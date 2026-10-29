//! Running on the real GPU from WSL (D185): the Windows build of a program, its assets
//! copied beside it, and `play` and `yard`, which use it wherever you are.

use crate::{Task, root, sweep};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

/// How long a staged copy (`windows_program`) is kept once nothing has run from it.
const COPY_IDLE: Duration = Duration::from_secs(24 * 60 * 60);
/// What the staged copies may hold together (D259): past it the longest idle go first, a day
/// or not. About twenty copies, as many as `doctor` lets pass.
const COPY_BUDGET: u64 = 16_000_000_000;
/// A copy run from this recently, or staged and about to start, is kept whatever the budget.
const COPY_FRESH: Duration = Duration::from_secs(10 * 60);

/// The tasks this module runs (D410).
pub(crate) const TASKS: &[Task] = &[
    Task {
        name: "play",
        takes: "[continue|new|creative|yard] [--build optimised|debug] [--profiling off|report|live] [--last] [--menu] [-- game args]",
        does: "the launcher (D575): in a terminal with nothing given, a menu of the build's choices, then the game on its title screen (D579); a word (continue, new, creative, yard) plays it straight in on the real GPU, wherever you are, as does no terminal (continue)",
        group: "Play and check",
        run: crate::launcher::play,
    },
    Task {
        name: "yard",
        takes: "[game args]",
        does: "play the yard, a flat test level for building: nothing on it, building free and instant, nothing saved (`play yard`)",
        group: "Play and check",
        run: yard,
    },
    Task {
        name: "win",
        takes: "-p <package> [--release] [--copy <name>] [-- args]",
        does: "under WSL, build for Windows and run it there, on the real GPU",
        group: "This machine",
        run: win,
    },
];

pub(crate) fn on_wsl() -> bool {
    std::fs::read_to_string("/proc/version").is_ok_and(|v| v.to_lowercase().contains("microsoft"))
}

pub(crate) const WINDOWS_TARGET: &str = "x86_64-pc-windows-gnu";

/// WSL has no hardware Vulkan, so performance is measured on a Windows build launched
/// from WSL. Windows cannot use a WSL folder as a working directory, so the program is
/// copied to the Windows temp folder and run from there.
pub(crate) fn win(args: &[String]) -> bool {
    if !on_wsl() {
        eprintln!(
            "`cargo xtask win` is only for WSL. Elsewhere, `cargo run` already uses the GPU."
        );
        return false;
    }
    let (ours, program_args) = match args.iter().position(|a| a == "--") {
        Some(i) => (&args[..i], &args[i + 1..]),
        None => (args, &[][..]),
    };
    let value = |name: &str| {
        ours.iter()
            .position(|a| a == name)
            .and_then(|i| ours.get(i + 1))
            .map(String::as_str)
    };
    let Some(package) = value("-p") else {
        eprintln!(
            "usage: cargo xtask win -p <package> [--release] [--copy <name>] [-- program args]"
        );
        return false;
    };
    let release = ours.iter().any(|a| a == "--release");
    Game {
        package,
        profile: if release { "release" } else { "dev" },
        features: None,
        // A named copy runs from its own folder, so test runs never collide with a copy
        // someone is playing: Windows will not replace a program that is running.
        copy: value("--copy"),
        args: program_args,
        output: None,
        played: false,
    }
    .run()
    .is_some_and(|(ended, _)| ended.ok())
}

/// A program built and run on the real GPU: under WSL the Windows build, in a copy of its own
/// in the Windows temp folder (`windows_program`); elsewhere the native build, run from the
/// checkout.
pub(crate) struct Game<'a> {
    pub(crate) package: &'a str,
    /// The cargo profile: `dev`, `release` or `bench`.
    pub(crate) profile: &'a str,
    /// Cargo features beyond the profile's own, such as `hitch-log` for the session report.
    pub(crate) features: Option<&'a str>,
    /// The copy's name under WSL, so runs never collide: Windows will not replace a program
    /// that is running.
    pub(crate) copy: Option<&'a str>,
    pub(crate) args: &'a [String],
    /// A file in the folder it runs in that takes its output, instead of the terminal.
    pub(crate) output: Option<&'a str>,
    /// A session someone plays: its log finished and brought home (`session`).
    pub(crate) played: bool,
}

impl Game<'_> {
    /// Builds and runs it. Gives how it ended and the folder it ran in, or `None` when it
    /// could not be built or staged (said already).
    pub(crate) fn run(&self) -> Option<(Ended, PathBuf)> {
        let (program, run_dir) = if on_wsl() {
            windows_program(self.package, self.profile, self.features, self.copy)?
        } else {
            cook();
            let mut build = vec!["build", "--profile", self.profile, "-p", self.package];
            if self.profile == "dev" {
                build.extend(crate::tasks::sandbox::dev_features(self.package));
            }
            if let Some(features) = self.features {
                build.extend(["--features", features]);
            }
            if !crate::stage::build(&build) {
                return None;
            }
            let folder = root().join("target").join(profile_folder(self.profile));
            (folder.join(self.package), root())
        };
        println!(
            "\n> {} {}   (in {})",
            program.file_name().unwrap_or_default().to_string_lossy(),
            self.args.join(" "),
            run_dir.display()
        );
        let output = match self.output {
            Some(name) => match std::fs::File::create(run_dir.join(name)) {
                Ok(file) => Some(file),
                Err(why) => {
                    eprintln!("Could not write {} ({why}).", run_dir.join(name).display());
                    return None;
                }
            },
            None => None,
        };
        // The kits' files are read from the source's assets, through WSL's share, so one saved
        // while the copy runs is seen (WSLENV's `/p` gives Windows the path as it names it).
        let wslenv = std::env::var("WSLENV").unwrap_or_default();
        let wslenv = [wslenv.as_str(), "TERRA_SOURCE_ASSETS/p"]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(":");
        let ended = std::cell::Cell::new(Ended::NotStarted);
        let game = || {
            let mut command = Command::new(&program);
            command.args(self.args).current_dir(&run_dir);
            if on_wsl() {
                command
                    .env("TERRA_SOURCE_ASSETS", root().join("assets"))
                    .env("WSLENV", &wslenv);
            } else {
                // Run outside Cargo, a Bevy program looks for assets/ next to itself.
                command.env("BEVY_ASSET_ROOT", &run_dir);
            }
            if let Some(file) = &output
                && let (Ok(out), Ok(err)) = (file.try_clone(), file.try_clone())
            {
                command.stdout(out).stderr(err);
            }
            ended.set(match command.status() {
                Ok(status) if status.success() => Ended::Ok,
                Ok(status) => Ended::Failed(status.code()),
                Err(_) => Ended::NotStarted,
            });
            ended.get() == Ended::Ok
        };
        if self.played {
            crate::session::play(&run_dir.join("logs"), game);
        } else {
            game();
        }
        Some((ended.get(), run_dir))
    }
}

/// How a program run by `Game` ended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Ended {
    Ok,
    /// It ran and failed, with its exit code (`None`: ended by a signal).
    Failed(Option<i32>),
    /// It could not be started at all.
    NotStarted,
}

impl Ended {
    pub(crate) fn ok(self) -> bool {
        self == Ended::Ok
    }

    /// The exit code it failed with, if it ran and failed.
    pub(crate) fn code(self) -> Option<i32> {
        match self {
            Ended::Failed(code) => code,
            _ => None,
        }
    }

    /// In words, for an error: "exited with code 101 (a Rust panic)".
    pub(crate) fn words(self) -> String {
        match self {
            Ended::Ok => "exited cleanly".into(),
            Ended::NotStarted => "could not be started".into(),
            Ended::Failed(None) => "was killed by a signal".into(),
            Ended::Failed(Some(101)) => {
                "exited with code 101, a Rust panic: its output's `panicked at` line says where"
                    .into()
            }
            Ended::Failed(Some(code)) => format!("exited with code {code}"),
        }
    }
}

/// Builds a package for Windows in a cargo profile (`dev`, `release` or `bench`) and copies
/// it, with the assets, to a folder of its own in the Windows temp folder. Windows cannot use
/// a WSL folder as a working directory. The textures are cooked first (`cook`), so every
/// Windows run (`play`, `win`, `bench`) draws as a player's game does, whatever ran before.
/// Gives the program and its folder.
pub(crate) fn windows_program(
    package: &str,
    profile: &str,
    features: Option<&str>,
    copy: Option<&str>,
) -> Option<(PathBuf, PathBuf)> {
    cook();
    let mut build = vec![
        "build",
        "--target",
        WINDOWS_TARGET,
        "--profile",
        profile,
        "-p",
        package,
    ];
    if profile == "dev" {
        build.extend(crate::tasks::sandbox::dev_features(package));
    }
    if let Some(features) = features {
        build.extend(["--features", features]);
    }
    if !crate::stage::build(&build) {
        return None;
    }

    let exe_name = format!("{package}.exe");
    let built = root()
        .join("target")
        .join(WINDOWS_TARGET)
        .join(profile_folder(profile))
        .join(&exe_name);
    let folder = match copy {
        Some(name) => format!("{package}-{name}"),
        None => package.to_string(),
    };
    let Some(run_dir) = windows_temp().map(|temp| temp.join("terra").join(folder)) else {
        eprintln!("Could not find the Windows temp folder. Is Windows interop enabled?");
        return None;
    };
    let program = run_dir.join(&exe_name);
    let staged = std::fs::create_dir_all(&run_dir)
        .and_then(|()| copy_changed(&built, &program))
        // Not copied, it is touched: the sweep reads its time as the copy's last run.
        .and_then(|copied| match copied {
            true => Ok(()),
            false => std::fs::File::options()
                .write(true)
                .open(&program)
                .and_then(|file| file.set_modified(SystemTime::now())),
        })
        // A Bevy program looks for its assets next to itself.
        .and_then(|()| copy_dir(&root().join("assets"), &run_dir.join("assets")));
    if let Err(error) = staged {
        eprintln!(
            "Could not copy the program to {}: {error}",
            run_dir.display()
        );
        return None;
    }
    Some((run_dir.join(&exe_name), run_dir))
}

/// The folder under `target/` a cargo profile builds into: Cargo's own profiles share `debug`
/// and `release` (`test` builds with `dev`, `bench` with `release`).
pub(crate) fn profile_folder(profile: &str) -> &str {
    match profile {
        "dev" | "test" => "debug",
        "release" | "bench" => "release",
        custom => custom,
    }
}

/// Plays the sandbox with `game`'s arguments as a person would, what the launcher chose
/// (`launcher`). Under WSL the Windows build runs, in a copy of its own, so a scripted test
/// running at the same time does not get in the way. Its log, with the other GPU runs beside
/// it, ends in the checkout's `logs/` (`session`). The optimised build is the release
/// profile, the default (D581): the debug build loaded in 25 s, not 15, and drew a frame in
/// 13.5 ms, not 10. Gives whether it ended well, and its exit code if it failed.
pub(crate) fn start(build: crate::launcher::Build, game: Vec<String>) -> (bool, Option<i32>) {
    let ended = Game {
        package: "terra_sandbox",
        profile: build.profile(),
        features: None,
        copy: Some("play"),
        args: &game,
        output: None,
        played: true,
    }
    .run()
    .map(|(ended, _)| ended);
    (ended.is_some_and(Ended::ok), ended.and_then(Ended::code))
}

/// Cooks the textures (D188, `cargo xtask cook`) so the game played draws them cooked: read
/// from the source, every model's and material's picture was mipmapped on the CPU at load
/// (5.1 s of the developer's 25 to 30 s) and went to the GPU uncompressed, as nothing ran the
/// cook. Under WSL the Windows build reads its own cache (`%LOCALAPPDATA%\terra\cooked`,
/// `terra_core::cooked::cache_folder`), so what is cooked is mirrored there. What is cooked
/// already is kept on both sides, so a launch with nothing new takes about a second. A cook
/// that fails is said and the game plays from the source, as on a fresh checkout. Bench's
/// numbers depended on whether `play` had filled the cache until bench and `win` cooked too.
pub(crate) fn cook() {
    if !crate::stage::build(&["run", "-q", "--release", "-p", "terra_bake"]) {
        eprintln!(
            "  The game plays with its textures read as made, slower to load; `cargo xtask cook` shows why the cook failed (D188)."
        );
        return;
    }
    if !on_wsl() {
        return;
    }
    let from = crate::session::cache_folder().join("cooked");
    let Some(to) = cooked_folder() else {
        eprintln!(
            "✗ cook: could not find the Windows %LOCALAPPDATA% to copy the cooked textures to; the Windows build reads them as made. Is Windows interop enabled?"
        );
        return;
    };
    match mirror_cooked(&from, &to) {
        Ok(0) => {}
        Ok(copied) => println!("cook: {copied} cooked files copied to {}", to.display()),
        Err(error) => eprintln!(
            "✗ cook: copying the cooked textures to {} failed ({error}); the Windows build reads those missing as made. Run `cargo xtask cook` and this command again.",
            to.display()
        ),
    }
}

/// Where the game run from here reads its cooked textures (`terra_core::cooked::folder`):
/// under WSL the Windows build's `%LOCALAPPDATA%\terra\cooked`, otherwise the cache's own.
pub(crate) fn cooked_folder() -> Option<PathBuf> {
    if on_wsl() {
        windows_folder("LOCALAPPDATA").map(|local| local.join("terra").join("cooked"))
    } else {
        Some(crate::session::cache_folder().join("cooked"))
    }
}

/// Copies to `to` each cooked file of `from` it lacks; says how many. A file's name is the
/// key of the bytes it was cooked from (`terra_core::cooked`), so one there is never stale and
/// none is removed: other checkouts' keys stay. Each is written beside its name and renamed,
/// so a game or a second copy running at once never reads half a file.
fn mirror_cooked(from: &Path, to: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(to)?;
    let mut copied = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let target = to.join(&name);
        if !entry.file_type()?.is_file()
            || name.to_string_lossy().ends_with(".part")
            || target
                .metadata()
                .is_ok_and(|t| t.len() == entry.metadata().map_or(0, |m| m.len()))
        {
            continue;
        }
        let part = to.join(format!("{}.part", name.to_string_lossy()));
        std::fs::copy(entry.path(), &part)?;
        std::fs::rename(&part, &target)?;
        copied += 1;
    }
    Ok(copied)
}

/// Plays the yard, the flat test level for working on building: the launcher's `play yard`.
pub(crate) fn yard(args: &[String]) -> bool {
    let mut ours = vec!["yard".to_string()];
    ours.extend(args.iter().cloned());
    crate::launcher::play(&ours)
}

/// Deletes the staged copies in the Windows temp folder nothing has run from for a day, then
/// the longest idle while together they hold more than `COPY_BUDGET` (D259): each is a whole
/// program and its assets, about 750 MB, named by view and worktree, so most are never run
/// again, and agents stage some 150 a day, which filled the Windows disk inside the day. What
/// it freed.
pub(crate) fn sweep_copies(now: SystemTime) -> u64 {
    if !on_wsl() {
        return 0;
    }
    let Some(staged) = windows_temp().map(|temp| temp.join("terra")) else {
        return 0;
    };
    sweep_staged(&staged, now, COPY_BUDGET)
}

/// `sweep_copies` on a folder of copies, with its budget.
fn sweep_staged(staged: &Path, now: SystemTime, budget: u64) -> u64 {
    let mut copies: Vec<(Duration, u64, PathBuf)> = children(staged)
        .into_iter()
        .map(|copy| {
            let idle = now.duration_since(last_run(&copy)).unwrap_or_default();
            (idle, sweep::unlinked(&copy), copy)
        })
        .collect();
    copies.sort_by_key(|(idle, _, _)| std::cmp::Reverse(*idle));
    let mut held: u64 = copies.iter().map(|(_, bytes, _)| bytes).sum();
    let mut freed = 0;
    for (idle, bytes, copy) in copies {
        let over = held > budget && idle >= COPY_FRESH;
        if (idle >= COPY_IDLE || over) && delete_copy(&copy) {
            held -= bytes;
            freed += bytes;
        }
    }
    freed
}

/// Deletes a staged copy, its program first: Windows will not delete a program that is running
/// (through WSL, an I/O error), so a copy in use is kept whole.
fn delete_copy(copy: &Path) -> bool {
    children(copy)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|e| e == "exe"))
        .all(|program| std::fs::remove_file(program).is_ok())
        && std::fs::remove_dir_all(copy).is_ok()
}

/// When a staged copy was last run from: its program is copied in or touched at every launch, and a run
/// writes its logs and shots beside it.
fn last_run(copy: &Path) -> SystemTime {
    let mut paths = vec![copy.to_path_buf()];
    for path in children(copy) {
        if path.ends_with("logs") || path.ends_with("shots") {
            paths.extend(children(&path));
        }
        paths.push(path);
    }
    paths
        .iter()
        .filter_map(|p| p.metadata().and_then(|m| m.modified()).ok())
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn children(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default()
}

/// The Windows temp folder, as a path WSL can use.
pub(crate) fn windows_temp() -> Option<PathBuf> {
    windows_folder("TEMP")
}

/// A folder Windows names in an environment variable (`TEMP`, `LOCALAPPDATA`), as a path WSL
/// can use.
pub(crate) fn windows_folder(variable: &str) -> Option<PathBuf> {
    let run_from = |program: &str, args: &[&str]| {
        let out = Command::new(program)
            .args(args)
            // Windows programs complain when started from a WSL folder.
            .current_dir("/mnt/c")
            .stderr(Stdio::null())
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let windows_path = run_from("cmd.exe", &["/c", &format!("echo %{variable}%")])?;
    run_from("wslpath", &["-u", &windows_path]).map(PathBuf::from)
}

/// Mirrors `from` into `to`: what `from` has is copied over, and what it no longer has is
/// removed, so a data file renamed or deleted since the last run is not still read there (a
/// folder of move sets read whole failed on one renamed away, combat round 26).
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    if !from.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(to)? {
        let entry = entry?;
        if !from.join(entry.file_name()).exists() {
            if entry.path().is_dir() {
                std::fs::remove_dir_all(entry.path())?;
            } else {
                std::fs::remove_file(entry.path())?;
            }
        }
    }
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            copy_changed(&entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Copies `from` to `to` unless `to` already holds it: the same size, written since `from`
/// was. Says whether it copied. The program (730 MB in a dev build) and the assets (500 MB)
/// went over WSL's share whole at every launch, about 20 s before the game started; most
/// launches change neither. A copy is written after its source, so a source saved since, or
/// put back by git (which writes the time of the checkout), is newer and copied again.
fn copy_changed(from: &Path, to: &Path) -> std::io::Result<bool> {
    let source = from.metadata()?;
    if let Ok(copy) = to.metadata()
        && copy.len() == source.len()
        && copy.modified()? >= source.modified()?
    {
        return Ok(false);
    }
    std::fs::copy(from, to)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A copy of `bytes` whose last run was `idle` ago.
    fn stage(staged: &Path, name: &str, bytes: usize, idle: Duration, now: SystemTime) {
        let copy = staged.join(name);
        std::fs::create_dir_all(copy.join("logs")).unwrap();
        std::fs::write(copy.join("terra_sandbox.exe"), vec![0u8; bytes]).unwrap();
        let then = now - idle;
        for path in [copy.join("terra_sandbox.exe"), copy.join("logs"), copy] {
            std::fs::File::open(&path)
                .unwrap()
                .set_modified(then)
                .unwrap();
        }
    }

    #[test]
    fn copies_past_the_budget_go_longest_idle_first_and_a_fresh_one_stays() {
        let staged = std::env::temp_dir().join(format!("xtask-copies-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&staged);
        let now = SystemTime::now();
        let hour = Duration::from_secs(60 * 60);
        let size = 1000;
        stage(&staged, "day", size, 25 * hour, now);
        stage(&staged, "two-hours", size, 2 * hour, now);
        stage(&staged, "one-hour", size, hour, now);
        stage(&staged, "fresh", size, Duration::from_secs(60), now);
        let left = || {
            let mut names: Vec<String> = children(&staged)
                .iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        };
        // The day's goes on age alone; the next longest idle brings the four down to two.
        assert_eq!(sweep_staged(&staged, now, 2 * size as u64), 2 * size as u64);
        assert_eq!(left(), ["fresh", "one-hour"]);
        // With no room at all, every copy but one run this minute goes.
        assert_eq!(sweep_staged(&staged, now, 0), size as u64);
        assert_eq!(left(), ["fresh"]);
        std::fs::remove_dir_all(&staged).unwrap();
    }

    #[test]
    fn a_copy_is_made_again_only_when_its_source_changed() {
        let dir = std::env::temp_dir().join(format!("xtask-copy-changed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (from, to) = (dir.join("from"), dir.join("to"));
        let write = |text: &str, age: u64| {
            std::fs::write(&from, text).unwrap();
            let then = SystemTime::now() - Duration::from_secs(age);
            std::fs::File::options()
                .write(true)
                .open(&from)
                .unwrap()
                .set_modified(then)
                .unwrap();
        };
        write("one", 60);
        assert!(copy_changed(&from, &to).unwrap(), "no copy yet");
        assert!(!copy_changed(&from, &to).unwrap(), "unchanged since");
        // Saved since the copy, at the same size: copied again.
        write("two", 0);
        std::fs::File::options()
            .write(true)
            .open(&to)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(30))
            .unwrap();
        assert!(copy_changed(&from, &to).unwrap(), "saved since the copy");
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "two");
        // Another size, though older than the copy (put back from an older file): copied.
        write("three", 3600);
        assert!(copy_changed(&from, &to).unwrap(), "another size");
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "three");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_cooked_files_missing_are_copied_and_the_rest_kept() {
        let dir = std::env::temp_dir().join(format!("xtask-mirror-cooked-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (from, to) = (dir.join("linux"), dir.join("windows"));
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(&to).unwrap();
        std::fs::write(from.join("a.ktx2"), "cooked a").unwrap();
        std::fs::write(from.join("b.glb"), "cooked b").unwrap();
        // A file the cook is still writing is not copied.
        std::fs::write(from.join("c.ktx2.part"), "half").unwrap();
        // Another checkout's key, and one cut short by a copy that stopped.
        std::fs::write(to.join("other.ktx2"), "theirs").unwrap();
        std::fs::write(to.join("b.glb"), "cut").unwrap();
        assert_eq!(mirror_cooked(&from, &to).unwrap(), 2, "a missing, b short");
        assert_eq!(
            std::fs::read_to_string(to.join("a.ktx2")).unwrap(),
            "cooked a"
        );
        assert_eq!(
            std::fs::read_to_string(to.join("b.glb")).unwrap(),
            "cooked b"
        );
        assert!(
            to.join("other.ktx2").exists(),
            "another checkout's key stays"
        );
        assert!(!to.join("c.ktx2.part").exists() && !to.join("c.ktx2").exists());
        assert_eq!(mirror_cooked(&from, &to).unwrap(), 0, "nothing new");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
