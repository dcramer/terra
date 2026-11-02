//! One copy of the game a machine (D319). Several copies at once, a job filming three side by
//! side, slowed the developers' machine to a crawl ("thats gonna murder our system"), so the
//! game refuses to start while another copy runs, Windows and WSL counted as one machine, and
//! a deliberate switch, [`SWITCH`], allows exactly one more for a two-player test (a host and
//! a joining player on one machine). Nothing allows a third.
//!
//! It is a single-instance lock as shipped games and desktop programs take one (a named mutex
//! on Windows, a lock file elsewhere): a slot file per allowed copy, `instance-0.lock` and
//! `instance-1.lock`, held with the operating system's lock on the whole file (`flock` on
//! Linux, `LockFileEx` on Windows) for the run's life, so a crash frees it at once and a dead
//! copy's slot is simply taken. Beside each a record, `instance-<n>.txt`, says who holds it
//! (pid, system, worktree, command, since when) for the refusal (D556); it is a file of its own
//! because a Windows lock is mandatory and would stop another copy reading the slot file.
//!
//! The folder is the Windows one, `%LOCALAPPDATA%\terra`, seen from WSL through `/mnt/c`, so
//! both sides take the same slots; `TERRA_INSTANCE_DIR` moves it. A lock taken on one side of
//! the WSL line is not seen on the other (2026-11-02: a WSL copy found a running Windows copy
//! only by its beat, refused after the whole watch), so a record the other side wrote is
//! trusted only while it beats: its holder rewrites it every [`Timing::beat`], and a copy that
//! finds the slot's lock free but such a record watches it for [`Timing::settle`]; changed, the
//! copy is alive, unchanged, it is dead and the slot is taken. Clocks are never compared: WSL's
//! drifts from Windows' after the machine sleeps.
//!
//! The render box (tools/render-worker) is a machine kept for rendering and runs three copies
//! (D319): its daemon gives each slot of its own an instance folder of its own.

use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// The switch allowing one more copy, for a two-player test on one machine; named as Valve's
/// Source games name theirs (`-multirun`).
pub const SWITCH: &str = "--multirun";
/// The exit status of a refused start: sysexits' `EX_TEMPFAIL`, try again later.
pub const REFUSED: i32 = 75;
/// The words a refusal starts with, which the GPU wrapper (tools/gpu) looks for.
pub const REFUSAL: &str = "SANDBOX ✗ refused to start";
/// How many copies may run at once: one, and one more with the switch.
const SLOTS: usize = 2;

/// How often a holder rewrites its record, and how long a record from the other side of the
/// WSL line is watched for a change before its copy is taken for dead.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub beat: Duration,
    pub settle: Duration,
}

/// The game's: a beat a second, watched for two and a half.
pub const TIMING: Timing = Timing {
    beat: Duration::from_secs(1),
    settle: Duration::from_millis(2500),
};

/// Who holds a slot, as its record says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub pid: u32,
    /// `linux`, `windows`, `macos`: the side of the WSL line it runs on.
    pub os: String,
    pub worktree: String,
    pub command: String,
    /// Seconds since 1970 on the holder's clock.
    pub since: u64,
    /// Counts up each [`Timing::beat`] while the holder runs.
    pub beat: u64,
}

impl Record {
    /// This run's.
    pub fn this_run() -> Record {
        let mut args = std::env::args();
        let program = args
            .next()
            .map(|p| {
                Path::new(&p)
                    .file_name()
                    .map_or(p.clone(), |n| n.to_string_lossy().into_owned())
            })
            .unwrap_or_default();
        let command = std::iter::once(program)
            .chain(args.map(|a| if a.contains(' ') { format!("'{a}'") } else { a }))
            .collect::<Vec<_>>()
            .join(" ");
        Record {
            pid: std::process::id(),
            os: std::env::consts::OS.to_string(),
            // The Windows build runs from a staged copy; `cargo xtask win` names its worktree.
            worktree: std::env::var("TERRA_WORKTREE")
                .ok()
                .or_else(|| {
                    std::env::current_dir()
                        .ok()
                        .map(|d| d.display().to_string())
                })
                .unwrap_or_default(),
            command,
            since: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            beat: 0,
        }
    }

    fn text(&self) -> String {
        let one_line = |s: &str| s.replace(['\n', '\r'], " ");
        format!(
            "pid={}\nos={}\nworktree={}\ncommand={}\nsince={}\nbeat={}\n",
            self.pid,
            one_line(&self.os),
            one_line(&self.worktree),
            one_line(&self.command),
            self.since,
            self.beat
        )
    }

    fn parse(text: &str) -> Option<Record> {
        let field = |key: &str| {
            text.lines()
                .find_map(|line| line.split_once('=').filter(|(k, _)| *k == key))
                .map(|(_, v)| v.to_string())
        };
        Some(Record {
            pid: field("pid")?.parse().ok()?,
            os: field("os")?,
            worktree: field("worktree")?,
            command: field("command")?,
            since: field("since")?.parse().ok()?,
            beat: field("beat")?.parse().ok()?,
        })
    }
}

/// A slot held: its lock, and the beat rewriting its record. The game keeps it for its life; a
/// test drops it to free the slot as a copy's death would, its record left behind.
pub struct Held {
    pub slot: usize,
    _lock: File,
    beating: Arc<AtomicBool>,
}

impl Drop for Held {
    fn drop(&mut self) {
        self.beating.store(false, Ordering::Relaxed);
    }
}

/// A start refused: who holds the slots it could have had.
#[derive(Debug)]
pub struct Refusal {
    /// A record a holder, or none where its record could not be read.
    pub holders: Vec<Option<Record>>,
    pub multirun: bool,
    pub folder: PathBuf,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.multirun {
            writeln!(
                f,
                "{REFUSAL}: two copies of the game run on this machine, as many as {SWITCH} \
                 allows (one copy a machine and one more for a two-player test, Windows and \
                 WSL counted as one, D319); nothing allows a third."
            )?;
        } else {
            writeln!(
                f,
                "{REFUSAL}: another copy of the game runs on this machine, and the game allows \
                 one copy a machine, Windows and WSL counted as one (D319)."
            )?;
        }
        for holder in &self.holders {
            match holder {
                Some(r) => writeln!(
                    f,
                    "  pid {} on {}, in {}, since {} UTC: {}",
                    r.pid,
                    r.os,
                    r.worktree,
                    clock(r.since),
                    r.command
                )?,
                None => writeln!(
                    f,
                    "  a copy whose record in {} could not be read",
                    self.folder.display()
                )?,
            }
        }
        if self.multirun {
            write!(
                f,
                "What to do: wait for one to end, or close one, then start this again."
            )
        } else {
            write!(
                f,
                "What to do: wait for it to end, or close it, then start this again; a script \
                 runs its copies one after another, never side by side. Only a two-player test \
                 on one machine starts its second copy with {SWITCH}, which allows exactly one \
                 more."
            )
        }
    }
}

/// `14:05:09`, the time of day in UTC of seconds since 1970.
fn clock(since: u64) -> String {
    let day = since % 86_400;
    format!("{:02}:{:02}:{:02}", day / 3600, day % 3600 / 60, day % 60)
}

/// The slot this run holds, for its life.
static HELD: OnceLock<Held> = OnceLock::new();

/// Takes this run's slot for its life, or says who holds the slots and exits with [`REFUSED`].
/// `multirun` is the switch. Where no folder can be used, it says so and the run goes on.
pub fn take(multirun: bool) {
    let me = Record::this_run();
    let mut tried = Vec::new();
    for folder in folders() {
        match take_in(&folder, multirun, &me, TIMING) {
            Ok(Ok(held)) => {
                let _ = HELD.set(held);
                return;
            }
            Ok(Err(refusal)) => {
                eprintln!("{refusal}");
                std::process::exit(REFUSED);
            }
            Err(error) => tried.push(format!("{}: {error}", folder.display())),
        }
    }
    println!(
        "SANDBOX ✗ no single-instance lock (one copy a machine, D319), so nothing stops a second \
         copy: {}; set TERRA_INSTANCE_DIR to a folder this run can write",
        tried.join("; ")
    );
}

/// Where the slots are tried, in order: `TERRA_INSTANCE_DIR`; else the Windows folder, from WSL
/// too, then this side's cache folder should that fail.
fn folders() -> Vec<PathBuf> {
    if let Some(dir) = std::env::var_os("TERRA_INSTANCE_DIR") {
        return vec![PathBuf::from(dir)];
    }
    let mut out = Vec::new();
    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        out.push(PathBuf::from(local).join("terra"));
    }
    if crate::shell::on_wsl()
        && let Some(windows) = windows_folder()
    {
        out.push(windows);
    }
    out.extend(crate::cache::folder());
    out
}

/// `%LOCALAPPDATA%\terra` as WSL sees it, asked of Windows once and remembered in this side's
/// cache folder, as asking takes a Windows program's start.
fn windows_folder() -> Option<PathBuf> {
    let note = crate::cache::folder()?.join("windows-local-app-data");
    if let Ok(text) = std::fs::read_to_string(&note) {
        let local = PathBuf::from(text.trim());
        if local.is_dir() {
            return Some(local.join("terra"));
        }
    }
    let run = |program: &str, args: &[&str]| {
        let out = std::process::Command::new(program)
            .args(args)
            // Windows programs complain when started from a WSL folder.
            .current_dir("/mnt/c")
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let windows = run("cmd.exe", &["/c", "echo %LOCALAPPDATA%"])?;
    let local = PathBuf::from(run("wslpath", &["-u", &windows])?);
    if !local.is_dir() {
        return None;
    }
    let _ = std::fs::write(&note, local.to_string_lossy().as_bytes());
    Some(local.join("terra"))
}

/// What a slot was found to be.
enum Slot {
    Free(File),
    Held(Option<Record>),
}

/// Takes a slot in `folder` for `me`: the first without the switch, or the first free of the
/// two with it, refused while the slots it may have are held, and without the switch while the
/// second is too. An error is the folder's, not a refusal.
pub fn take_in(
    folder: &Path,
    multirun: bool,
    me: &Record,
    timing: Timing,
) -> io::Result<Result<Held, Refusal>> {
    std::fs::create_dir_all(folder)?;
    let mut holders = Vec::new();
    let mine = if multirun { SLOTS } else { 1 };
    for slot in 0..mine {
        match look(folder, slot, &me.os, timing)? {
            Slot::Held(record) => holders.push(record),
            Slot::Free(lock) => {
                // Without the switch, a second copy running (one that passed it) still counts.
                if !multirun && let Slot::Held(record) = look(folder, SLOTS - 1, &me.os, timing)? {
                    holders.push(record);
                    break;
                }
                return Ok(Ok(hold(folder, slot, lock, me, timing.beat)));
            }
        }
    }
    Ok(Err(Refusal {
        holders,
        multirun,
        folder: folder.to_path_buf(),
    }))
}

fn lock_path(folder: &Path, slot: usize) -> PathBuf {
    folder.join(format!("instance-{slot}.lock"))
}

fn record_path(folder: &Path, slot: usize) -> PathBuf {
    folder.join(format!("instance-{slot}.txt"))
}

/// Whether a slot is free, and if so its lock, taken.
fn look(folder: &Path, slot: usize, os: &str, timing: Timing) -> io::Result<Slot> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path(folder, slot))?;
    let record = record_path(folder, slot);
    // A record read as its holder rewrites it may come out empty or cut short: read again.
    let read = || {
        let mut text = std::fs::read_to_string(&record).ok();
        for _ in 0..4 {
            if text.as_deref().is_none_or(|t| Record::parse(t).is_some()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
            text = std::fs::read_to_string(&record).ok();
        }
        text
    };
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            return Ok(Slot::Held(read().as_deref().and_then(Record::parse)));
        }
        Err(TryLockError::Error(error)) => return Err(error),
    }
    // Free on this side. A record from the other side of the WSL line may be a live copy whose
    // lock this side does not see: alive while it beats.
    let before = read();
    if let Some(other) = before
        .as_deref()
        .and_then(Record::parse)
        .filter(|r| r.os != os)
    {
        std::thread::sleep(timing.settle);
        if read() != before {
            return Ok(Slot::Held(
                read().as_deref().and_then(Record::parse).or(Some(other)),
            ));
        }
    }
    Ok(Slot::Free(lock))
}

/// Holds a slot whose lock is taken: writes the record and keeps it beating.
fn hold(folder: &Path, slot: usize, lock: File, me: &Record, beat: Duration) -> Held {
    let path = record_path(folder, slot);
    let _ = std::fs::write(&path, me.text());
    let beating = Arc::new(AtomicBool::new(true));
    let still = beating.clone();
    let mut record = me.clone();
    let _ = std::thread::Builder::new()
        .name("single-instance beat".into())
        .spawn(move || {
            loop {
                std::thread::sleep(beat);
                if !still.load(Ordering::Relaxed) {
                    return;
                }
                record.beat += 1;
                let _ = std::fs::write(&path, record.text());
            }
        });
    Held {
        slot,
        _lock: lock,
        beating,
    }
}

#[cfg(test)]
mod tests;
