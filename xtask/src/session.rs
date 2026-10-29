//! A played session's log (D448): while the game runs, every other run on the GPU that
//! tools/gpu has on file (D319: a file a run, its pid, command and start) is noted, since
//! they share the card with no queue and slow the game's frames; when the game exits, its
//! log gets a section saying which ran and how long beside it, is copied into the
//! checkout's `logs/` if the game ran from a copy elsewhere, and its path is printed last,
//! for the developer to hand to an agent.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How often the runs on file are read.
const EVERY: Duration = Duration::from_secs(3);
/// How many logs of a kind the checkout keeps, as the game does (terra_world::log_file).
const KEPT: usize = 20;
/// A command's length at most in the section.
const WIDTH: usize = 160;

/// A run on tools/gpu's file, as much as the section says of it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Run {
    pub(crate) class: String,
    /// The wrapper's pid, and the command's.
    pub(crate) pid: u64,
    pub(crate) child: Option<u64>,
    pub(crate) cmd: String,
    /// When it got the card, ms since 1970; none while it waits for it.
    pub(crate) started: Option<u64>,
}

/// A run as the session saw it: when first and last, and whether it ever had the card.
#[derive(Clone, Debug, PartialEq)]
struct Seen {
    run: Run,
    first: u64,
    last: u64,
}

/// The other runs a session saw, by their wrapper's pid.
#[derive(Default, Debug)]
pub(crate) struct Watch {
    seen: BTreeMap<u64, Seen>,
}

impl Watch {
    /// Notes the runs on file at `now`, ms since 1970, but those in `ours` (the processes
    /// this session runs under).
    pub(crate) fn sample(&mut self, runs: Vec<Run>, now: u64, ours: &[u64]) {
        for run in runs {
            if ours.contains(&run.pid) || run.child.is_some_and(|child| ours.contains(&child)) {
                continue;
            }
            let seen = self.seen.entry(run.pid).or_insert_with(|| Seen {
                run: run.clone(),
                first: now,
                last: now,
            });
            seen.last = now;
            // A run that waited gets its start once it has the card.
            seen.run.started = seen.run.started.or(run.started);
        }
    }

    /// The section for the log of a session from `start` to `end`, ms since 1970.
    pub(crate) fn section(&self, start: u64, end: u64) -> String {
        let mut out = format!(
            "\nOther GPU runs during this session (tools/gpu's runs, read every {} s): {}\n",
            EVERY.as_secs(),
            self.seen.len()
        );
        for seen in self.seen.values() {
            let Some(began) = seen.run.started else {
                out.push_str(&format!(
                    "  {} waited for the card from {} and never had it: {}\n",
                    seen.run.class,
                    clock(seen.first.max(start)),
                    shorten(&seen.run.cmd)
                ));
                continue;
            };
            let from = began.max(start);
            let to = seen.last.min(end).max(from);
            out.push_str(&format!(
                "  {} beside it for {} ({} to {} UTC{}): {}\n",
                seen.run.class,
                span(to - from),
                clock(from),
                clock(to),
                if began < start {
                    ", begun before the game"
                } else {
                    ""
                },
                shorten(&seen.run.cmd)
            ));
        }
        out
    }
}

/// A run's file, if it reads as one.
pub(crate) fn parse_run(text: &str) -> Option<Run> {
    let Json::Object(fields) = Json::parse(text)? else {
        return None;
    };
    let number = |key: &str| match fields.get(key) {
        Some(Json::Number(n)) => Some(*n as u64),
        _ => None,
    };
    let cmd = match fields.get("cmd") {
        Some(Json::Array(parts)) => parts
            .iter()
            .filter_map(|part| match part {
                Json::String(s) => Some(s.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    };
    Some(Run {
        class: match fields.get("class") {
            Some(Json::String(class)) => class.clone(),
            _ => "run".into(),
        },
        pid: number("pid")?,
        child: number("child"),
        cmd,
        started: number("started"),
    })
}

/// Where tools/gpu keeps its runs (tools/gpu/gpu.ts's `RUNS`).
fn runs_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TERRA_GPU_DIR") {
        return PathBuf::from(dir).join("runs");
    }
    cache_folder().join("gpu").join("runs")
}

/// Terra's folder in the user's cache, as the game names it (`terra_core::cooked::cache_folder`).
pub(crate) fn cache_folder() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_default()
        .join("terra")
}

fn runs_on_file(dir: &Path) -> Vec<Run> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()? == "json")
                .then(|| std::fs::read_to_string(&path).ok())
                .flatten()
                .and_then(|text| parse_run(&text))
        })
        .collect()
}

/// This process and its forebears, so a session run under tools/gpu does not count itself.
fn ours() -> Vec<u64> {
    let mut found = vec![u64::from(std::process::id())];
    while let Some(&pid) = found.last() {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
        // The parent is the second field after the name in brackets.
        let parent = stat
            .rsplit(')')
            .next()
            .and_then(|rest| rest.split_whitespace().nth(1))
            .and_then(|p| p.parse::<u64>().ok());
        match parent {
            Some(parent) if parent > 1 && !found.contains(&parent) => found.push(parent),
            _ => break,
        }
    }
    found
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Watches the runs on file until `stop` is set; gives what it saw.
pub(crate) fn watch(stop: Arc<AtomicBool>) -> JoinHandle<Watch> {
    std::thread::spawn(move || {
        let dir = runs_dir();
        let ours = ours();
        let mut watch = Watch::default();
        while !stop.load(Ordering::Relaxed) {
            watch.sample(runs_on_file(&dir), now_ms(), &ours);
            let mut waited = Duration::ZERO;
            while waited < EVERY && !stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(200));
                waited += Duration::from_millis(200);
            }
        }
        watch.sample(runs_on_file(&dir), now_ms(), &ours);
        watch
    })
}

/// The newest log of the play experiment in `folder` written since `start`, ms since
/// 1970: not another run's an agent made in the same checkout meanwhile.
pub(crate) fn newest_log(folder: &Path, start: u64) -> Option<PathBuf> {
    let since = UNIX_EPOCH + Duration::from_millis(start.saturating_sub(2_000));
    std::fs::read_dir(folder)
        .ok()?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let modified = entry.metadata().ok()?.modified().ok()?;
            let path = entry.path();
            let played = path.file_name()?.to_str()?.get(15..)?.starts_with("-play");
            (path.extension()? == "log" && played && modified >= since).then_some((modified, path))
        })
        .max()
        .map(|(_, path)| path)
}

/// Ends a session that ran from `start` to now: adds the section to the game's newest log
/// in `logs` and copies it to `home` unless it is there already; gives the log's path.
pub(crate) fn finish(watch: &Watch, start: u64, logs: &Path, home: &Path) -> Option<PathBuf> {
    use std::io::Write as _;
    let log = newest_log(logs, start)?;
    let section = watch.section(start, now_ms());
    std::fs::OpenOptions::new()
        .append(true)
        .open(&log)
        .and_then(|mut file| file.write_all(section.as_bytes()))
        .ok()?;
    if logs == home {
        return Some(log);
    }
    std::fs::create_dir_all(home).ok()?;
    let name = log.file_name()?.to_str()?.to_string();
    let copy = home.join(&name);
    std::fs::copy(&log, &copy).ok()?;
    // The checkout keeps as many of a kind as the game does (terra_world::log_file::KEPT).
    let names: Vec<String> = std::fs::read_dir(home)
        .ok()?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .collect();
    for old in older(&names, &name, KEPT) {
        let _ = std::fs::remove_file(home.join(old));
    }
    Some(copy)
}

/// The logs of the same kind as `newest` (its name past the time, `YYYYMMDD-HHMMSS-`) beyond
/// the newest `kept`.
fn older(names: &[String], newest: &str, kept: usize) -> Vec<String> {
    let kind = |name: &str| {
        name.get(16..).map(|kind| {
            kind.split('~')
                .next()
                .unwrap_or(kind)
                .trim_end_matches(".log")
                .to_string()
        })
    };
    let mut same: Vec<&String> = names
        .iter()
        .filter(|name| name.len() > 16 && kind(name) == kind(newest))
        .collect();
    same.sort();
    same.reverse();
    same.into_iter().skip(kept).cloned().collect()
}

/// Plays a session: runs `game`, which writes its logs to `logs`, watching the other GPU
/// runs meanwhile; then finishes its log into the checkout's `logs/` and prints its path,
/// last. Gives whether the game succeeded.
pub(crate) fn play(logs: &Path, game: impl FnOnce() -> bool) -> bool {
    let start = now_ms();
    let stop = Arc::new(AtomicBool::new(false));
    let watching = watch(stop.clone());
    let ok = game();
    stop.store(true, Ordering::Relaxed);
    let seen = watching.join().unwrap_or_default();
    match finish(&seen, start, logs, &crate::root().join("logs")) {
        Some(log) => println!("Log: {}", log.display()),
        None => eprintln!("No log of this session in {}", logs.display()),
    }
    ok
}

/// A moment, ms since 1970, as its time of day in UTC, `HH:MM:SS`.
fn clock(ms: u64) -> String {
    let s = ms / 1000 % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// A length of time, ms, as `1h02m`, `3m12s` or `9s`.
fn span(ms: u64) -> String {
    let s = ms / 1000;
    match s {
        0..60 => format!("{s}s"),
        60..3600 => format!("{}m{:02}s", s / 60, s % 60),
        _ => format!("{}h{:02}m", s / 3600, s / 60 % 60),
    }
}

fn shorten(text: &str) -> String {
    if text.chars().count() <= WIDTH {
        return text.to_string();
    }
    text.chars().take(WIDTH - 1).chain(['…']).collect()
}

/// JSON, as much as a run's file needs: xtask has no dependencies.
#[derive(Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

impl Json {
    fn parse(text: &str) -> Option<Json> {
        let mut chars = text.chars().peekable();
        let value = Self::value(&mut chars)?;
        Self::space(&mut chars);
        chars.peek().is_none().then_some(value)
    }

    fn space(chars: &mut std::iter::Peekable<std::str::Chars>) {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
    }

    fn value(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<Json> {
        Self::space(chars);
        match *chars.peek()? {
            '{' => {
                chars.next();
                let mut fields = BTreeMap::new();
                loop {
                    Self::space(chars);
                    match chars.next()? {
                        '}' if fields.is_empty() => return Some(Json::Object(fields)),
                        '"' => {}
                        _ => return None,
                    }
                    let key = Self::string(chars)?;
                    Self::space(chars);
                    (chars.next()? == ':').then_some(())?;
                    fields.insert(key, Self::value(chars)?);
                    Self::space(chars);
                    match chars.next()? {
                        ',' => {}
                        '}' => return Some(Json::Object(fields)),
                        _ => return None,
                    }
                }
            }
            '[' => {
                chars.next();
                let mut items = Vec::new();
                Self::space(chars);
                if chars.peek() == Some(&']') {
                    chars.next();
                    return Some(Json::Array(items));
                }
                loop {
                    items.push(Self::value(chars)?);
                    Self::space(chars);
                    match chars.next()? {
                        ',' => {}
                        ']' => return Some(Json::Array(items)),
                        _ => return None,
                    }
                }
            }
            '"' => {
                chars.next();
                Self::string(chars).map(Json::String)
            }
            _ => {
                let mut word = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || matches!(c, '-' | '+' | '.') {
                        word.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                match word.as_str() {
                    "null" => Some(Json::Null),
                    "true" => Some(Json::Bool(true)),
                    "false" => Some(Json::Bool(false)),
                    number => number.parse().ok().map(Json::Number),
                }
            }
        }
    }

    /// A string's rest, after its opening quote.
    fn string(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<String> {
        let mut out = String::new();
        loop {
            match chars.next()? {
                '"' => return Some(out),
                '\\' => match chars.next()? {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    'b' => out.push('\u{8}'),
                    'f' => out.push('\u{c}'),
                    'u' => {
                        let hex: String = (0..4).filter_map(|_| chars.next()).collect();
                        let code = u32::from_str_radix(&hex, 16).ok()?;
                        out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                    }
                    other => out.push(other),
                },
                c => out.push(c),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-24 15:30:00 UTC, in ms.
    const T: u64 = 1_792_855_800_000;

    fn run(pid: u64, class: &str, started: Option<u64>) -> Run {
        let text = format!(
            r#"{{"class":"{class}","pid":{pid},"child":{},"cmd":["cargo","xtask","sandbox","--shot","a \"b\".png"],"cwd":"/w","asked":{T}{}}}"#,
            pid + 1,
            started.map_or(String::new(), |s| format!(
                r#","started":{s},"progress":{s}"#
            ))
        );
        parse_run(&text).unwrap()
    }

    #[test]
    fn reads_a_run_file_as_tools_gpu_writes_it() {
        let read = run(41, "film", Some(T + 5));
        assert_eq!(read.pid, 41);
        assert_eq!(read.child, Some(42));
        assert_eq!(read.class, "film");
        assert_eq!(read.cmd, r#"cargo xtask sandbox --shot a "b".png"#);
        assert_eq!(read.started, Some(T + 5));
        assert_eq!(run(7, "bench", None).started, None);
        assert_eq!(parse_run("{\"pid\": 3, \"cmd\": []}").unwrap().class, "run");
        assert!(parse_run("not json").is_none());
        assert!(parse_run(r#"{"pid":3"#).is_none());
    }

    #[test]
    fn says_which_other_runs_shared_the_card_and_for_how_long() {
        let s = 1000;
        let mut watch = Watch::default();
        // A film begun a minute before the game, gone after 3m12s; a bench that waited the
        // whole session; a shot under which this session itself runs.
        let film = run(10, "film", Some(T - 60 * s));
        let bench = run(20, "bench", None);
        let ours = run(30, "still", Some(T));
        for at in (0..=192).step_by(3) {
            watch.sample(
                vec![film.clone(), bench.clone(), ours.clone()],
                T + at * s,
                &[31],
            );
        }
        for at in (195..=600).step_by(3) {
            watch.sample(vec![bench.clone(), ours.clone()], T + at * s, &[31]);
        }
        let section = watch.section(T, T + 600 * s);
        let lines: Vec<&str> = section.lines().collect();
        assert_eq!(
            lines[1],
            "Other GPU runs during this session (tools/gpu's runs, read every 3 s): 2"
        );
        assert_eq!(
            lines[2],
            r#"  film beside it for 3m12s (15:30:00 to 15:33:12 UTC, begun before the game): cargo xtask sandbox --shot a "b".png"#
        );
        assert!(lines[3].starts_with("  bench waited for the card from 15:30:00 and never had it"));
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn a_session_alone_says_so() {
        assert!(
            Watch::default()
                .section(T, T + 1000)
                .ends_with("read every 3 s): 0\n")
        );
    }

    #[test]
    fn the_checkout_keeps_the_newest_of_a_kind() {
        let mut names: Vec<String> = (1..=22)
            .map(|day| format!("202610{day:02}-120000-play-yard.log"))
            .collect();
        names.push("20201001-120000-play.log".into());
        names.push("20201001-120000-play-yard~2.log".into());
        let gone = older(&names, "20261022-120000-play-yard.log", 20);
        assert_eq!(
            gone,
            [
                "20261002-120000-play-yard.log",
                "20261001-120000-play-yard.log",
                "20201001-120000-play-yard~2.log"
            ]
        );
    }

    #[test]
    fn the_log_is_finished_and_copied_home() {
        let dir = std::env::temp_dir().join(format!("terra-session-test-{}", std::process::id()));
        let (logs, home) = (dir.join("copy/logs"), dir.join("home/logs"));
        std::fs::create_dir_all(&logs).unwrap();
        let start = now_ms();
        std::fs::write(
            logs.join("20261024-153000-play-yard.log"),
            "the game's lines\n",
        )
        .unwrap();
        let copied = finish(&Watch::default(), start, &logs, &home).unwrap();
        let text = std::fs::read_to_string(&copied).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(copied, home.join("20261024-153000-play-yard.log"));
        assert!(text.starts_with("the game's lines\n\nOther GPU runs during this session"));
    }
}
