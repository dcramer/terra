//! A crash capsule (D399): when the run panics, what an agent needs to understand it in one
//! short file, rather than hundreds of lines of backtrace and log. The hook writes
//! `shots/crash-<time>/` with `summary.txt` (at most 40 lines: the panic and where, only the
//! backtrace's frames in our own crates, the last lines the run said, the frame, seed, save
//! and command line, and a command that runs it again), the whole backtrace and log beside
//! it, and a copy of the save; then prints the summary's path as one line, last. The
//! default hook still runs first, so nothing it printed before is lost.
//!
//! As shipped games do with a crash reporter's minidump and its log, but as text an agent
//! reads: the capsule is the panic's place, our frames and the run's recent past.

use crate::record::{Line, Record};
use std::{
    fmt::Write as _,
    panic::PanicHookInfo,
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

/// The summary's length at most, so it is read in one look.
const MOST_LINES: usize = 40;
/// The run's last lines the summary shows at most, room allowing.
const MOST_SAID: usize = 30;
/// Our frames the summary shows at most; the rest are in `backtrace.txt`.
const MOST_FRAMES: usize = 8;
/// A summary line's length at most.
const WIDTH: usize = 160;
/// A save larger than this is not copied into the capsule.
const MOST_SAVE_BYTES: u64 = 64 << 20;

static RECORD: OnceLock<Record> = OnceLock::new();
static NOTES: Mutex<Vec<(&'static str, String)>> = Mutex::new(Vec::new());
static CRASHED: AtomicBool = AtomicBool::new(false);

/// Installs the hook, chained after the one already there. Call it first thing in `main`.
pub fn install() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        // Only the first panic: a panic in a system is followed by others as the app stops.
        if !CRASHED.swap(true, Ordering::SeqCst) {
            match write(info) {
                Ok(summary) => eprintln!("CRASH capsule: {}", summary.display()),
                Err(problem) => eprintln!("CRASH capsule not written: {problem}"),
            }
        }
    }));
}

/// Something about the run the capsule says, such as its seed or its save; set again, the
/// newer value wins.
pub fn note(key: &'static str, value: impl std::fmt::Display) {
    // Formatted before the lock is taken: a `Display` that panics would otherwise run the
    // hook with the lock held, and the hook's own lock would wait for ever.
    let value = value.to_string();
    let mut notes = NOTES.lock().unwrap_or_else(|p| p.into_inner());
    notes.retain(|(k, _)| *k != key);
    notes.push((key, value));
}

/// The record the capsule reads the run's last lines and frame from (`record::layer`).
pub(crate) fn watch(record: &Record) {
    let _ = RECORD.set(record.clone());
}

/// Everything a capsule is made of, gathered once the run has panicked.
pub struct Crash<'a> {
    pub message: &'a str,
    pub location: &'a str,
    pub thread: &'a str,
    pub frame: Option<u32>,
    pub notes: &'a [(&'static str, String)],
    /// The command line, the program first.
    pub args: &'a [String],
    /// `std::backtrace::Backtrace`'s text.
    pub backtrace: &'a str,
    /// The run's last lines, the oldest first, and how many it said in all.
    pub said: &'a [Line],
    pub said_in_all: u64,
    /// The save copied into the capsule, if there was one.
    pub save_copy: Option<&'a Path>,
}

/// The summary: at most `MOST_LINES` lines.
pub fn summary(crash: &Crash) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut message = crash.message.lines();
    let first = message.next().unwrap_or("");
    lines.push(format!(
        "panicked at {}: {first}",
        short_path(crash.location)
    ));
    let more = message.count();
    if more > 0 {
        lines.push(format!("  (+{more} lines of message in backtrace.txt)"));
    }
    let mut about = vec![format!("thread '{}'", crash.thread)];
    about.push(match crash.frame {
        Some(frame) => format!("frame {frame}"),
        None => "before the first frame".into(),
    });
    for (key, value) in crash.notes {
        about.push(format!("{key} {value}"));
    }
    if crash.save_copy.is_some() {
        about.push("save copied as save.ron".into());
    }
    lines.push(about.join(", "));
    lines.push(format!("ran:   {}", command_line(crash.args)));
    lines.push(format!("again: {}", again(crash.args, crash.save_copy)));
    if crash.args.iter().any(|a| a == "--remote") {
        lines.push("  (driven over the remote: the script's calls are not in this)".into());
    }

    let ours = our_frames(crash.backtrace);
    if ours.is_empty() {
        lines.push("no frames in our crates (no debug info?); see backtrace.txt".into());
    } else {
        lines.push("our frames, innermost first:".into());
        for frame in ours.iter().take(MOST_FRAMES) {
            lines.push(format!("  {frame}"));
        }
        if ours.len() > MOST_FRAMES {
            lines.push(format!(
                "  … {} more in backtrace.txt",
                ours.len() - MOST_FRAMES
            ));
        }
    }

    // The last lines said, as many as there is room for.
    let room = MOST_LINES
        .saturating_sub(lines.len() + 2)
        .min(MOST_SAID)
        .min(crash.said.len());
    let shown = &crash.said[crash.said.len() - room..];
    lines.push(format!(
        "last {} of {} lines said (all kept in log.txt):",
        shown.len(),
        crash.said_in_all
    ));
    for line in shown {
        let source = line.source.rsplit("::").next().unwrap_or(&line.source);
        lines.push(format!(
            "  f{} {} {source}: {}",
            line.frame,
            line.level,
            line.text.replace('\t', " ")
        ));
    }
    lines.push("beside it: backtrace.txt, log.txt".into());
    let mut out = String::new();
    for line in lines.iter().take(MOST_LINES) {
        out.push_str(&shorten(line.lines().next().unwrap_or("")));
        out.push('\n');
    }
    out
}

/// The backtrace's frames in our own crates, as `crates/…/file.rs:line name`, innermost
/// first, leaving out the hook's own. A frame is ours by its file, under our `crates/`, or
/// failing a file by its name, since the dev profile's line tables name a function without
/// its path (`build`, `{closure#0}`).
fn our_frames(backtrace: &str) -> Vec<String> {
    let mut frames: Vec<(String, Option<String>)> = Vec::new();
    for line in backtrace.lines() {
        let line = line.trim();
        if let Some(at) = line.strip_prefix("at ") {
            if let Some(last) = frames.last_mut()
                && last.1.is_none()
            {
                last.1 = Some(at.to_string());
            }
            continue;
        }
        // `12: name`, or an inlined name under it without a number.
        let name = match line.split_once(": ") {
            Some((number, name)) if number.chars().all(|c| c.is_ascii_digit()) => name,
            _ => line,
        };
        if !name.is_empty() {
            frames.push((name.to_string(), None));
        }
    }
    let in_ours = |at: &str| {
        at.contains("crates/terra_") && !at.contains("/registry/") && !at.contains("/rustc/")
    };
    frames
        .into_iter()
        .filter(|(name, at)| {
            let bare = name.strip_prefix('<').unwrap_or(name);
            let ours = match at {
                Some(at) => in_ours(at),
                None => bare.starts_with("terra_"),
            };
            ours && !bare.starts_with("terra_world::crash::")
                && !at
                    .as_deref()
                    .is_some_and(|at| at.contains("terra_world/src/crash.rs"))
        })
        .map(|(name, at)| {
            // Generic arguments say little and run long: `add_to_app<…>` as `add_to_app`.
            let name = match name.find('<') {
                Some(cut) if cut > 0 => &name[..cut],
                _ => &name,
            };
            match at {
                Some(at) => format!("{} {name}", place(&at)),
                None => name.to_string(),
            }
        })
        .collect()
}

/// A source file's path from where it is someone's: `…/crates/terra_x/src/y.rs:12:5` as
/// `crates/terra_x/src/y.rs:12:5`, a dependency's `…/registry/src/<index>/bevy_x-0.20.0/…`
/// from `bevy_x-0.20.0/`.
fn short_path(at: &str) -> &str {
    if let Some(start) = at.find("/registry/src/") {
        let rest = &at[start + "/registry/src/".len()..];
        return rest.split_once('/').map_or(rest, |(_, path)| path);
    }
    at.find("crates/").map_or(at, |start| &at[start..])
}

/// A frame's place without its column: `crates/terra_x/src/y.rs:12`.
fn place(at: &str) -> String {
    let at = short_path(at);
    match at.rsplit_once(':') {
        Some((head, column))
            if column.chars().all(|c| c.is_ascii_digit()) && head.contains(':') =>
        {
            head.to_string()
        }
        _ => at.to_string(),
    }
}

/// The command line as a shell would take it back.
fn command_line(args: &[String]) -> String {
    args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ")
}

fn quote(arg: &str) -> String {
    let plain = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=,@+%".contains(c));
    if plain {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

/// A command that runs the program again with the same options, through Cargo with the same
/// build type. A run that did not start fresh starts from the copy of its save, which is the
/// save as last written before the crash, not as it was when the run began.
fn again(args: &[String], save_copy: Option<&Path>) -> String {
    let Some((program, options)) = args.split_first() else {
        return String::new();
    };
    let package = Path::new(program)
        .file_stem()
        .map_or_else(|| "?".into(), |s| s.to_string_lossy().into_owned());
    let release = Path::new(program)
        .components()
        .any(|c| c.as_os_str() == "release");
    let mut options = options.to_vec();
    if let Some(copy) = save_copy
        && !options.iter().any(|o| o == "--fresh")
    {
        let copy = copy.to_string_lossy().into_owned();
        match options.iter().position(|o| o == "--save") {
            Some(at) if at + 1 < options.len() => options[at + 1] = copy,
            _ => options.extend(["--save".to_string(), copy]),
        }
    }
    let mut out = format!(
        "cargo run{} -p {package} --",
        if release { " --release" } else { "" }
    );
    for option in &options {
        write!(out, " {}", quote(option)).ok();
    }
    out
}

fn shorten(line: &str) -> String {
    if line.chars().count() <= WIDTH {
        return line.to_string();
    }
    let mut cut: String = line.chars().take(WIDTH - 1).collect();
    cut.push('…');
    cut
}

/// Seconds since 1970 as `YYYYMMDD-HHMMSS`, in UTC (Howard Hinnant's `civil_from_days`).
pub fn stamp(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let of_day = seconds % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        of_day / 3_600,
        of_day / 60 % 60,
        of_day % 60
    )
}

/// The capsule's folder: `shots/` of the checkout the run's assets are in.
fn folder() -> PathBuf {
    let assets = crate::data::assets();
    let root = assets.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let base = root.join("shots").join(format!("crash-{}", stamp(seconds)));
    let mut dir = base.clone();
    let mut n = 2;
    while dir.exists() {
        dir = PathBuf::from(format!("{}-{n}", base.display()));
        n += 1;
    }
    dir
}

/// Writes the capsule for a panic and gives the summary's path.
fn write(info: &PanicHookInfo) -> std::io::Result<PathBuf> {
    let payload = info.payload();
    let message = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("(a panic without text)");
    let location = info
        .location()
        .map_or_else(|| "?".into(), |l| l.to_string());
    let thread = std::thread::current();
    let thread = thread.name().unwrap_or("unnamed");
    let backtrace = std::backtrace::Backtrace::force_capture().to_string();
    let args: Vec<String> = std::env::args().collect();
    let notes = NOTES.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let (said, said_in_all, frame) = match RECORD.get() {
        Some(record) => {
            let (lines, next) = record.everything();
            (lines, next, Some(record.frame()))
        }
        None => (Vec::new(), 0, None),
    };

    let dir = folder();
    std::fs::create_dir_all(&dir)?;
    let save = notes
        .iter()
        .find(|(k, _)| *k == "save")
        .map(|(_, v)| PathBuf::from(v));
    let save_copy = save
        .filter(|s| s.metadata().is_ok_and(|m| m.len() <= MOST_SAVE_BYTES))
        .and_then(|s| {
            let copy = dir.join("save.ron");
            std::fs::copy(s, &copy).ok().map(|_| copy)
        });
    let crash = Crash {
        message,
        location: &location,
        thread,
        frame,
        notes: &notes,
        args: &args,
        backtrace: &backtrace,
        said: &said,
        said_in_all,
        save_copy: save_copy.as_deref(),
    };
    std::fs::write(
        dir.join("backtrace.txt"),
        format!("panicked at {location} on thread '{thread}':\n{message}\n\n{backtrace}"),
    )?;
    let mut log = String::new();
    for line in &said {
        writeln!(
            log,
            "{} f{} {} {}: {}",
            line.number, line.frame, line.level, line.source, line.text
        )
        .ok();
    }
    std::fs::write(dir.join("log.txt"), log)?;
    let path = dir.join("summary.txt");
    std::fs::write(&path, summary(&crash))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::log::Level;

    /// A backtrace as std writes one: the hook's frames, ours with an inlined one under it,
    /// Bevy's between, the runtime's, and ours as the dev profile's line tables name them,
    /// without their paths.
    const BACKTRACE: &str = "   0: std::backtrace_rs::backtrace::libunwind::trace
             at /rustc/abc/library/std/src/../../backtrace/src/backtrace/libunwind.rs:117:9
   1: std::backtrace::Backtrace::force_capture
             at /rustc/abc/library/std/src/backtrace.rs:312:9
   2: terra_world::crash::write
             at /home/me/terra/crates/terra_world/src/crash.rs:250:21
   3: core::panicking::panic_bounds_check
             at /rustc/abc/library/core/src/panicking.rs:280:5
   4: terra_game::play::combat::hit
             at /home/me/terra/crates/terra_game/src/play/combat.rs:120:9
      terra_game::play::combat::swing
             at /home/me/terra/crates/terra_game/src/play/combat.rs:88:5
   5: bevy_ecs::system::function_system::FunctionSystem::run
             at /home/me/.cargo/registry/src/bevy_ecs-0.20.0/src/system/function_system.rs:700:9
   6: <terra_game::play::Plugin as bevy_app::Plugin>::build
             at /home/me/terra/crates/terra_game/src/play/mod.rs:40:1
   7: terra_sandbox::main
             at ./crates/terra_sandbox/src/main.rs:90:5
   8: {closure#0}
             at ./crates/terra_world/src/crash.rs:46:19
   9: build
             at ./crates/terra_world/src/shell.rs:952:14
  10: add_to_app<terra_world::shell::ShellPlugin>
             at /home/me/.cargo/registry/src/index.crates.io-1949/bevy_app-0.20.0/src/plugin.rs:148:21
  11: __libc_start_main";

    fn said(count: u64) -> Vec<Line> {
        (0..count)
            .map(|n| Line {
                number: n,
                frame: 100 + n as u32,
                level: Level::INFO,
                source: "terra_game::play::combat".into(),
                text: format!("line {n}"),
            })
            .collect()
    }

    fn args(text: &str) -> Vec<String> {
        text.split(' ').map(String::from).collect()
    }

    /// A value whose formatting reports whether the notes could be taken meanwhile, as the
    /// hook takes them when that formatting panics.
    struct Probe;
    impl std::fmt::Display for Probe {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            let free = NOTES.try_lock().is_ok();
            write!(f, "{}", if free { "free" } else { "held" })
        }
    }

    #[test]
    fn a_note_is_formatted_before_the_notes_are_locked() {
        note("probe", Probe);
        let notes = NOTES.lock().unwrap_or_else(|p| p.into_inner()).clone();
        let probe = notes
            .iter()
            .find(|(k, _)| *k == "probe")
            .map(|(_, v)| v.as_str());
        assert_eq!(probe, Some("free"));
    }

    #[test]
    fn a_summary_is_short_and_keeps_what_matters() {
        let said = said(500);
        let args = args("/x/target/debug/terra_sandbox --experiment play --save my.ron");
        let notes = [("seed", "1".to_string()), ("save", "my.ron".to_string())];
        let copy = PathBuf::from("/x/shots/crash-1/save.ron");
        let text = summary(&Crash {
            message: "index out of bounds: the len is 3 but the index is 7\nand more",
            location: "crates/terra_game/src/play/combat.rs:120:9",
            thread: "Compute Task Pool (1)",
            frame: Some(1203),
            notes: &notes,
            args: &args,
            backtrace: BACKTRACE,
            said: &said[480..],
            said_in_all: 500,
            save_copy: Some(&copy),
        });
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.len() <= 40, "{} lines:\n{text}", lines.len());
        assert_eq!(
            lines[0],
            "panicked at crates/terra_game/src/play/combat.rs:120:9: index out of bounds: the len is 3 but the index is 7"
        );
        assert!(text.contains("frame 1203, seed 1, save my.ron"), "{text}");
        assert!(text.contains(
            "again: cargo run -p terra_sandbox -- --experiment play --save /x/shots/crash-1/save.ron"
        ));
        // Ours only, innermost first, the inlined one kept and the hook's own left out.
        let frames: Vec<&str> = lines
            .iter()
            .skip_while(|l| !l.starts_with("our frames"))
            .skip(1)
            .take_while(|l| !l.starts_with("last "))
            .map(|l| l.trim())
            .collect();
        assert_eq!(
            frames,
            [
                "crates/terra_game/src/play/combat.rs:120 terra_game::play::combat::hit",
                "crates/terra_game/src/play/combat.rs:88 terra_game::play::combat::swing",
                "crates/terra_game/src/play/mod.rs:40 <terra_game::play::Plugin as bevy_app::Plugin>::build",
                "crates/terra_sandbox/src/main.rs:90 terra_sandbox::main",
                "crates/terra_world/src/shell.rs:952 build",
            ]
        );
        // Every one of the 20 lines given fits, the newest last.
        assert!(text.contains("last 20 of 500 lines said"));
        assert_eq!(lines[lines.len() - 2], "  f599 INFO combat: line 499");
        assert!(text.contains("  f580 INFO combat: line 480"));
    }

    #[test]
    fn only_the_newest_lines_said_fit() {
        let said = said(500);
        let args = args("target/release/terra_sandbox --experiment play --fresh --remote");
        let text = summary(&Crash {
            message: "boom",
            location: "a.rs:1:1",
            thread: "main",
            frame: None,
            notes: &[],
            args: &args,
            backtrace: BACKTRACE,
            said: &said,
            said_in_all: 500,
            save_copy: Some(Path::new("/c/save.ron")),
        });
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 40, "{text}");
        assert!(text.contains("before the first frame"));
        // A fresh run starts from nothing again, so its save is not swapped in.
        assert!(text.contains(
            "again: cargo run --release -p terra_sandbox -- --experiment play --fresh --remote\n"
        ));
        assert!(text.contains("driven over the remote"));
        // 40 lines less the 13 others: the 27 newest of the run's lines.
        assert!(text.contains("last 27 of 500 lines said"), "{text}");
        assert!(text.contains("line 499\n"));
        assert!(text.contains("line 473\n"));
        assert!(!text.contains("line 472\n"));
    }

    #[test]
    fn a_backtrace_without_our_frames_says_where_to_look() {
        let text = summary(&Crash {
            message: "boom",
            location: "/root/.cargo/registry/src/index.crates.io-1949/bevy_winit-0.20.0/src/lib.rs:137:14",
            thread: "main",
            frame: Some(3),
            notes: &[],
            args: &args("terra_sandbox"),
            backtrace: "   0: std::rt::lang_start\n   1: main",
            said: &[],
            said_in_all: 0,
            save_copy: None,
        });
        assert!(
            text.starts_with("panicked at bevy_winit-0.20.0/src/lib.rs:137:14: boom\n"),
            "{text}"
        );
        assert!(text.contains("no frames in our crates"), "{text}");
        assert!(text.contains("last 0 of 0 lines said"));
    }

    #[test]
    fn an_argument_a_shell_would_split_is_quoted() {
        assert_eq!(
            command_line(&args("terra_sandbox --at 1,2")),
            "terra_sandbox --at 1,2"
        );
        assert_eq!(
            command_line(&["--shot".into(), "my shot's.png".into()]),
            r"--shot 'my shot'\''s.png'"
        );
    }

    #[test]
    fn a_time_is_stamped_as_its_utc_date() {
        assert_eq!(stamp(0), "19700101-000000");
        // The leap day of 2000, a century's leap year: 951,782,400 s, plus 1 h 2 min 3 s.
        assert_eq!(stamp(951_782_400 + 3_723), "20000229-010203");
        assert_eq!(stamp(951_782_400 + 86_400), "20000301-000000");
    }
}
