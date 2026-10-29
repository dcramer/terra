//! The run's log in a file, every run (D448), as shipped games keep one (Unreal's
//! `Saved/Logs`, the previous runs kept beside it): every line the log keeps, written as it
//! is said, so a run that crashes or is killed still leaves it, and an agent can read a
//! session the developer played afterwards to say why its frames were slow.
//!
//! The files are in `logs/` beside the run's assets (the checkout's root, or the folder a
//! Windows copy runs from, which `cargo xtask play` copies back), named by when the run
//! began, in UTC, and what it ran: `20261024-153000-play-yard.log`. The newest
//! [`KEPT`] of each kind are kept, so the scripted runs agents make never push out the
//! sessions someone played.

use crate::record::Record;
use bevy::log::{
    BoxedLayer,
    tracing::{Event, Subscriber},
    tracing_subscriber::{Layer, layer::Context},
};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

/// How many logs of a kind are kept, the newest; older ones are deleted.
pub const KEPT: usize = 20;

/// The folder the logs are in: `logs/` beside the run's assets.
pub fn folder() -> PathBuf {
    let assets = crate::data::assets();
    let root = assets.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    root.join("logs")
}

/// What a run ran, for its file's name: its experiments, and its level if not the world.
pub fn kind(experiments: &[String], yard: bool) -> String {
    let mut kind = if experiments.is_empty() {
        "world".to_string()
    } else {
        experiments.join("+")
    };
    if yard {
        kind.push_str("-yard");
    }
    kind.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '_') {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// A log's file name: when it began and what it ran.
pub fn name(seconds: u64, kind: &str) -> String {
    format!("{}-{kind}.log", crate::crash::stamp(seconds))
}

/// The kind a log's file name says, if it is one of ours.
fn kind_of(name: &str) -> Option<&str> {
    let rest = name.strip_suffix(".log")?;
    // `YYYYMMDD-HHMMSS-`, then the kind, perhaps with the `~<n>` of a second run that second.
    let stamped = rest.get(..16)?;
    let looks = stamped.bytes().enumerate().all(|(i, b)| match i {
        8 | 15 => b == b'-',
        _ => b.is_ascii_digit(),
    });
    let kind = &rest[16..];
    let kind = match kind.rsplit_once('~') {
        Some((kind, n)) if n.bytes().all(|b| b.is_ascii_digit()) => kind,
        _ => kind,
    };
    looks.then_some(kind)
}

/// Of the files `names`, the logs of `kind` to delete so that, with one more, `kept` remain: the
/// oldest, by the time in their names.
pub fn to_delete(names: &[String], kind: &str, kept: usize) -> Vec<String> {
    let mut same: Vec<&String> = names
        .iter()
        .filter(|name| kind_of(name) == Some(kind))
        .collect();
    same.sort();
    let over = (same.len() + 1).saturating_sub(kept);
    same.into_iter().take(over).cloned().collect()
}

/// Makes the folder, deletes the oldest logs of the kind and opens a new one; gives it and
/// its path.
pub fn open(
    folder: &Path,
    seconds: u64,
    kind: &str,
    kept: usize,
) -> std::io::Result<(File, PathBuf)> {
    std::fs::create_dir_all(folder)?;
    let names: Vec<String> = std::fs::read_dir(folder)?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .collect();
    for old in to_delete(&names, kind, kept) {
        let _ = std::fs::remove_file(folder.join(old));
    }
    let base = name(seconds, kind);
    let mut path = folder.join(&base);
    let mut n = 2;
    // Two runs begun in one second: the second's kind gets `~2`, still read as the same kind.
    while path.exists() {
        path = folder.join(base.replace(".log", &format!("~{n}.log")));
        n += 1;
    }
    let file = File::create_new(&path)?;
    Ok((file, path))
}

/// The time of day of a moment, in ms since 1970, as `HH:MM:SS.mmm` in UTC.
pub fn clock(ms: u128) -> String {
    let of_day = ms % 86_400_000;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        of_day / 3_600_000,
        of_day / 60_000 % 60,
        of_day / 1_000 % 60,
        of_day % 1_000
    )
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

/// The log's layer, writing to a new file in [`folder`], or none if it could not be made
/// (said once on stderr: the run goes on without it).
pub(crate) fn layer(record: &Record, kind: &str) -> Option<BoxedLayer> {
    let seconds = (now_ms() / 1000) as u64;
    let (mut file, path) = match open(&folder(), seconds, kind, KEPT) {
        Ok(opened) => opened,
        Err(problem) => {
            eprintln!("No log file in {}: {problem}", folder().display());
            return None;
        }
    };
    let args: Vec<String> = std::env::args().collect();
    let _ = writeln!(
        file,
        "Terra log of {}, times UTC; a line is its time, frame, level, source and text.\n\
         Command: {}",
        crate::crash::stamp(seconds),
        args.join(" ")
    );
    crate::crash::note("log", path.display());
    Some(Box::new(FileWriter {
        file: Mutex::new(file),
        record: record.clone(),
    }))
}

struct FileWriter {
    file: Mutex<File>,
    /// For the frame each line was said on.
    record: Record,
}

impl<S: Subscriber> Layer<S> for FileWriter {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        if crate::record::frame_mark(event) {
            return;
        }
        let metadata = event.metadata();
        let line = format!(
            "{} f{} {} {}: {}\n",
            clock(now_ms()),
            self.record.frame(),
            metadata.level(),
            metadata.target(),
            crate::record::said(event)
        );
        // Unbuffered: each line is the system's as soon as it is said, so a crash or a
        // kill loses none of it.
        let mut file = self.file.lock().unwrap_or_else(|p| p.into_inner());
        let _ = file.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_log_is_named_by_when_it_began_and_what_it_ran() {
        let kind = kind(&["play".into()], true);
        assert_eq!(kind, "play-yard");
        // 2026-10-24 15:30:00 UTC.
        assert_eq!(name(1_792_855_800, &kind), "20261024-153000-play-yard.log");
        assert_eq!(kind_of("20261024-153000-play-yard.log"), Some("play-yard"));
        assert_eq!(
            kind_of("20261024-153000-play-yard~2.log"),
            Some("play-yard")
        );
        assert_eq!(kind_of("notes.log"), None);
        assert_eq!(super::kind(&[], false), "world");
        assert_eq!(
            super::kind(&["walk".into(), "a b".into()], false),
            "walk+a_b"
        );
    }

    #[test]
    fn only_the_oldest_of_the_same_kind_are_deleted() {
        let mut names: Vec<String> = (0..25)
            .map(|i| format!("202610{:02}-120000-play.log", i + 1))
            .collect();
        names.push("20200101-000000-walk.log".into());
        names.push("readme.txt".into());
        let gone = to_delete(&names, "play", 20);
        // Five of the oldest, and one more for the log about to be made.
        assert_eq!(gone.len(), 6);
        assert_eq!(gone[0], "20261001-120000-play.log");
        assert_eq!(gone[5], "20261006-120000-play.log");
        assert!(to_delete(&names, "walk", 20).is_empty());
    }

    #[test]
    fn opening_keeps_the_newest_and_never_overwrites() {
        let folder = std::env::temp_dir().join(format!("terra-log-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        for second in 0..5 {
            open(&folder, 1_792_855_800 + second, "play", 3).unwrap();
        }
        // Begun in the same second as the last: a second file beside it.
        let (_, again) = open(&folder, 1_792_855_804, "play", 3).unwrap();
        let mut left: Vec<String> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        std::fs::remove_dir_all(&folder).unwrap();
        assert_eq!(
            left,
            [
                "20261024-153003-play.log",
                "20261024-153004-play.log",
                "20261024-153004-play~2.log"
            ]
        );
        assert!(again.ends_with("20261024-153004-play~2.log"));
    }

    #[test]
    fn a_moment_is_its_time_of_day() {
        assert_eq!(clock(1_792_855_800_000 + 61_007), "15:31:01.007");
    }
}
