//! Running on the real GPU from WSL (D185): the Windows build of a program, its assets
//! copied beside it, and `play` and `yard`, which use it wherever you are.

use crate::{Task, root, run};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The tasks this module runs (D410).
pub(crate) const TASKS: &[Task] = &[
    Task {
        name: "play",
        takes: "[--fresh] [game args]",
        does: "play the tech demo on the real GPU, wherever you are; --fresh starts a new game",
        group: "Play and check",
        run: play,
    },
    Task {
        name: "yard",
        takes: "[game args]",
        does: "play the yard, a flat test level for building: nothing on it, building free and instant, nothing saved",
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
    let release = ours.iter().any(|a| a == "--release");
    // A named copy runs from its own folder, so test runs never collide with a copy
    // someone is playing: Windows will not replace a program that is running.
    let copy = ours
        .iter()
        .position(|a| a == "--copy")
        .and_then(|i| ours.get(i + 1));
    let Some(package) = ours
        .iter()
        .position(|a| a == "-p")
        .and_then(|i| ours.get(i + 1))
    else {
        eprintln!(
            "usage: cargo xtask win -p <package> [--release] [--copy <name>] [-- program args]"
        );
        return false;
    };

    let profile = if release { "release" } else { "dev" };
    let Some((program, run_dir)) = windows_program(package, profile, copy.map(String::as_str))
    else {
        return false;
    };
    let exe_name = format!("{package}.exe");
    println!(
        "\n> {exe_name} {}   (in {})",
        program_args.join(" "),
        run_dir.display()
    );
    // The kits' files are read from the source's assets, through WSL's share, so one saved
    // while the copy runs is seen (WSLENV's `/p` gives Windows the path as it names it).
    let wslenv = std::env::var("WSLENV").unwrap_or_default();
    let wslenv = [wslenv.as_str(), "TERRA_SOURCE_ASSETS/p"]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(":");
    Command::new(program)
        .args(program_args)
        .current_dir(&run_dir)
        .env("TERRA_SOURCE_ASSETS", root().join("assets"))
        .env("WSLENV", wslenv)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Builds a package for Windows in a cargo profile (`dev`, `release` or `bench`) and copies
/// it, with the assets, to a folder of its own in the Windows temp folder. Windows cannot use
/// a WSL folder as a working directory. Gives the program and its folder.
pub(crate) fn windows_program(
    package: &str,
    profile: &str,
    copy: Option<&str>,
) -> Option<(PathBuf, PathBuf)> {
    let build = [
        "build",
        "--target",
        WINDOWS_TARGET,
        "--profile",
        profile,
        "-p",
        package,
    ];
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
    let staged = std::fs::create_dir_all(&run_dir)
        .and_then(|()| std::fs::copy(&built, run_dir.join(&exe_name)).map(|_| ()))
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

/// Plays the tech demo as a person would: the sandbox's play experiment, with its save
/// kept between runs. Under WSL the Windows build runs, in a copy of its own, so a
/// scripted test running at the same time does not get in the way.
pub(crate) fn play(args: &[String]) -> bool {
    let mut game: Vec<String> = vec!["--experiment".into(), "play".into()];
    game.extend(args.iter().cloned());
    if on_wsl() {
        let mut ours: Vec<String> = ["-p", "terra_sandbox", "--copy", "play", "--"]
            .map(String::from)
            .to_vec();
        ours.extend(game);
        return win(&ours);
    }
    let mut cargo = vec!["run", "-p", "terra_sandbox", "--"];
    cargo.extend(game.iter().map(String::as_str));
    run("cargo", &cargo)
}

/// Plays the yard, the flat test level for working on building (`--level yard`).
pub(crate) fn yard(args: &[String]) -> bool {
    let mut game: Vec<String> = vec!["--level".into(), "yard".into()];
    game.extend(args.iter().cloned());
    play(&game)
}

/// The Windows temp folder, as a path WSL can use.
fn windows_temp() -> Option<PathBuf> {
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
    let windows_path = run_from("cmd.exe", &["/c", "echo %TEMP%"])?;
    run_from("wslpath", &["-u", &windows_path]).map(PathBuf::from)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    if !from.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
