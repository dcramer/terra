//! Project tasks, and the one place every tool is started from. Run as `cargo xtask <task>`;
//! `cargo xtask tools` lists them all, from `TASKS` below. A new tool gets a line there.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

/// A task: its name, what it takes, what it does, the group it is listed under, and how
/// it runs, given the arguments after its name.
struct Task {
    name: &'static str,
    takes: &'static str,
    does: &'static str,
    group: &'static str,
    run: fn(&[String]) -> bool,
}

/// Every task, in the order `tools` lists them. The TypeScript and Blender tools are started
/// here too, so nobody has to remember where each one lives; each has its own README.
const TASKS: &[Task] = &[
    Task {
        name: "play",
        takes: "[--fresh] [game args]",
        does: "play the tech demo on the real GPU, wherever you are; --fresh starts a new game",
        group: "Play and check",
        run: play,
    },
    Task {
        name: "loop",
        takes: "",
        does: "play the demo's loop by script and check every step happened (D69)",
        group: "Play and check",
        run: |_| run("node", &["tools/sandbox/loop.ts"]),
    },
    Task {
        name: "check",
        takes: "",
        does: "what every change must pass: formatting, lints, tests, the dependency rules",
        group: "Play and check",
        run: |_| check(),
    },
    Task {
        name: "sandbox",
        takes: "[args]",
        does: "the sandbox, the test environment; --list for its experiments (crates/terra_sandbox)",
        group: "Look at things",
        run: |args| cargo_run("terra_sandbox", &[], args),
    },
    Task {
        name: "workbench",
        takes: "[args]",
        does: "models in the game's renderer: --model, --prop Forge|all, --hold, --sheet",
        group: "Look at things",
        run: |args| cargo_run("terra_sandbox", &["--experiment", "workbench"], args),
    },
    Task {
        name: "remote",
        takes: "<command> [args]",
        does: "drive a running sandbox: start, warp, give, look, shot… (tools/sandbox)",
        group: "Look at things",
        run: |args| node("tools/sandbox/remote.ts", args),
    },
    Task {
        name: "models",
        takes: "[--port 8765]",
        does: "the model gallery in a browser: every model, its size, problems and a page each (tools/models)",
        group: "Look at things",
        run: |args| node("tools/models/serve.ts", args),
    },
    Task {
        name: "props",
        takes: "<step> [prop…]",
        does: "make props: concept, model, refine, all; fronts to see every model from the front (tools/props)",
        group: "Make art",
        run: |args| node("tools/props/props.ts", args),
    },
    Task {
        name: "tripo",
        takes: "<command> [args]",
        does: "Tripo on our Studio subscription: image, model, rig, animate, balance… (tools/tripo)",
        group: "Make art",
        run: |args| node("tools/tripo/tripo.ts", args),
    },
    Task {
        name: "bodies",
        takes: "",
        does: "rebuild every character asset the game loads from art/ (tools/characters)",
        group: "Make art",
        run: |_| run("tools/characters/build_bodies.sh", &[]),
    },
    Task {
        name: "blender",
        takes: "<script.py> [args]",
        does: "run a script in Blender, headless (tools/blender)",
        group: "Make art",
        run: |args| run_owned("tools/blender/run.sh", args),
    },
    Task {
        name: "icons",
        takes: "[args]",
        does: "bake every item's icon from its look into assets/icons (D129)",
        group: "Make art",
        run: icons,
    },
    Task {
        name: "paint-icons",
        takes: "<item…> | --unpainted",
        does: "paint item icons in the art style (tools/icons)",
        group: "Make art",
        run: |args| node("tools/icons/paint.ts", args),
    },
    Task {
        name: "doctor",
        takes: "",
        does: "what this machine is missing",
        group: "This machine",
        run: |_| doctor(),
    },
    Task {
        name: "win",
        takes: "-p <package> [--release] [--copy <name>] [-- args]",
        does: "under WSL, build for Windows and run it there, on the real GPU",
        group: "This machine",
        run: win,
    },
    Task {
        name: "tools",
        takes: "",
        does: "this list",
        group: "This machine",
        run: |_| {
            println!("{}", usage());
            true
        },
    },
];

/// Every task, by group, for `tools` and for a task that is not known.
fn usage() -> String {
    let mut text = String::from("usage: cargo xtask <task> [args]\n");
    let width = TASKS
        .iter()
        .map(|task| task.name.len() + task.takes.len() + 1)
        .max()
        .unwrap_or(0)
        .min(34);
    let mut group = "";
    for task in TASKS {
        if task.group != group {
            group = task.group;
            text.push_str(&format!("\n{group}\n"));
        }
        let call = format!("{} {}", task.name, task.takes);
        let call = call.trim_end();
        if call.len() > width {
            text.push_str(&format!("  {call}\n  {:width$}  {}\n", "", task.does));
        } else {
            text.push_str(&format!("  {call:width$}  {}\n", task.does));
        }
    }
    text
}

/// The only crates allowed to depend on Bevy (D28): the applications.
const BEVY_CRATES: [&str; 2] = ["terra_game", "terra_sandbox"];

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_default();
    let rest: Vec<String> = args.collect();
    let ok = match TASKS
        .iter()
        .find(|task| task.name == name || (name == "help" && task.name == "tools"))
    {
        Some(task) => (task.run)(&rest),
        None => {
            eprintln!("{}", usage());
            false
        }
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// A TypeScript tool, run by Node from the root.
fn node(script: &str, args: &[String]) -> bool {
    let mut all = vec![script.to_string()];
    all.extend(args.iter().cloned());
    run_owned("node", &all)
}

/// A package's program, with some arguments first.
fn cargo_run(package: &str, first: &[&str], args: &[String]) -> bool {
    let mut all = vec!["run", "-p", package, "--"];
    all.extend(first);
    all.extend(args.iter().map(String::as_str));
    run("cargo", &all)
}

fn run_owned(program: &str, args: &[String]) -> bool {
    run(
        program,
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run(program: &str, args: &[&str]) -> bool {
    println!("\n> {program} {}", args.join(" "));
    Command::new(program)
        .args(args)
        .current_dir(root())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(root())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Renders every item's icon and writes them into assets/icons. It opens a window
/// for the few seconds it takes; under WSL the CPU renderer is enough.
fn icons(args: &[String]) -> bool {
    let mut all = vec!["run", "-p", "terra_sandbox", "--", "--experiment", "icons"];
    all.extend(args.iter().map(String::as_str));
    run("cargo", &all)
}

fn check() -> bool {
    // CI only needs to know that everything compiles and the tests pass, so it skips
    // optimising dependencies, which is most of the time spent building Bevy.
    let quick: &[&str] = if std::env::var_os("CI").is_some() {
        &["--config", "profile.dev.package.\"*\".opt-level=0"]
    } else {
        &[]
    };
    let cargo = |command: &'static str, rest: &[&'static str]| {
        run("cargo", &[&[command], quick, rest].concat())
    };
    run("cargo", &["fmt", "--all", "--check"])
        && cargo(
            "clippy",
            &["--workspace", "--all-targets", "--", "-D", "warnings"],
        )
        && cargo("test", &["--workspace"])
        && tools_tests()
        && shared_crates_avoid_bevy()
        && shared_crates_are_repeatable()
}

/// The tests of the TypeScript tools (D121), which Node runs as they are.
fn tools_tests() -> bool {
    let Some(files) = output(
        "git",
        &["ls-files", "tools/**/*.test.ts", "tools/**/*.test.mjs"],
    ) else {
        println!("\n> tools tests: git cannot list them");
        return false;
    };
    let files: Vec<&str> = files.lines().collect();
    run("node", &[&["--test"], files.as_slice()].concat())
}

/// Anything every player must agree on lives in crates that know nothing about Bevy.
fn shared_crates_avoid_bevy() -> bool {
    println!("\n> shared-state crates must not depend on Bevy (D28)");
    let mut ok = true;
    let Ok(entries) = std::fs::read_dir(root().join("crates")) else {
        return true;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if BEVY_CRATES.contains(&name.as_str()) || !entry.path().join("Cargo.toml").exists() {
            continue;
        }
        let tree = output(
            "cargo",
            &["tree", "-p", &name, "-e", "normal", "--prefix", "none"],
        );
        let uses_bevy = tree.is_some_and(|t| t.lines().any(|line| line.starts_with("bevy")));
        println!(
            "  {name}: {}",
            if uses_bevy { "DEPENDS ON BEVY" } else { "ok" }
        );
        ok &= !uses_bevy;
    }
    ok
}

/// What makes a run of shared code come out differently from the same start: the clock and
/// the operating system's randomness. Shared code takes time and seeds as arguments.
const UNREPEATABLE: [&str; 6] = [
    "Instant::now",
    "SystemTime::now",
    "thread_rng",
    "rand::random",
    "RandomState::new",
    "getrandom",
];

fn shared_crates_are_repeatable() -> bool {
    println!("\n> shared-state crates must not read the clock or the system's randomness");
    let mut ok = true;
    let Ok(entries) = std::fs::read_dir(root().join("crates")) else {
        return true;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if BEVY_CRATES.contains(&name.as_str()) || !entry.path().join("Cargo.toml").exists() {
            continue;
        }
        let mut found = Vec::new();
        sources(&entry.path().join("src"), &mut found);
        let mut clean = true;
        for file in found {
            // A benchmark's own program may time itself.
            if file.ends_with("main.rs") || file.components().any(|part| part.as_os_str() == "bin")
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            for (number, line) in text.lines().enumerate() {
                if let Some(what) = UNREPEATABLE.iter().find(|what| line.contains(*what)) {
                    let at = file.strip_prefix(root()).unwrap_or(&file).display();
                    println!("  {at}:{}: {what}", number + 1);
                    clean = false;
                }
            }
        }
        if clean {
            println!("  {name}: ok");
        }
        ok &= clean;
    }
    ok
}

/// Every Rust file under a folder.
fn sources(folder: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
}

fn on_wsl() -> bool {
    std::fs::read_to_string("/proc/version").is_ok_and(|v| v.to_lowercase().contains("microsoft"))
}

const WINDOWS_TARGET: &str = "x86_64-pc-windows-gnu";

/// WSL has no hardware Vulkan, so performance is measured on a Windows build launched
/// from WSL. Windows cannot use a WSL folder as a working directory, so the program is
/// copied to the Windows temp folder and run from there.
fn win(args: &[String]) -> bool {
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

    let mut build = vec!["build", "--target", WINDOWS_TARGET, "-p", package.as_str()];
    if release {
        build.push("--release");
    }
    if !run("cargo", &build) {
        return false;
    }

    let profile = if release { "release" } else { "debug" };
    let exe_name = format!("{package}.exe");
    let built = root()
        .join("target")
        .join(WINDOWS_TARGET)
        .join(profile)
        .join(&exe_name);
    let folder = match copy {
        Some(name) => format!("{package}-{name}"),
        None => package.clone(),
    };
    let Some(run_dir) = windows_temp().map(|temp| temp.join("terra").join(folder)) else {
        eprintln!("Could not find the Windows temp folder. Is Windows interop enabled?");
        return false;
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
        return false;
    }

    println!(
        "\n> {exe_name} {}   (in {})",
        program_args.join(" "),
        run_dir.display()
    );
    Command::new(run_dir.join(&exe_name))
        .args(program_args)
        .current_dir(&run_dir)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Plays the tech demo as a person would: the sandbox's play experiment, with its save
/// kept between runs. Under WSL the Windows build runs, in a copy of its own, so a
/// scripted test running at the same time does not get in the way.
fn play(args: &[String]) -> bool {
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

struct Report {
    missing_required: bool,
}

impl Report {
    fn item(&mut self, label: &str, found: bool, required: bool, fix: &str) {
        let state = match (found, required) {
            (true, _) => "ok     ",
            (false, true) => "MISSING",
            (false, false) => "absent ",
        };
        println!("  [{state}] {label}");
        if !found {
            println!("            {fix}");
            self.missing_required |= required;
        }
    }
}

fn doctor() -> bool {
    let mut report = Report {
        missing_required: false,
    };
    let on_wsl = on_wsl();
    let os = std::env::consts::OS;
    println!("Platform: {os}{}", if on_wsl { " under WSL" } else { "" });

    println!("\nRust");
    let pinned = std::fs::read_to_string(root().join("rust-toolchain.toml")).unwrap_or_default();
    let pinned = pinned
        .lines()
        .find_map(|l| l.strip_prefix("channel = "))
        .map(|v| v.trim_matches('"').to_string())
        .unwrap_or_default();
    let rustc = output("rustc", &["--version"]).unwrap_or_default();
    report.item(
        &format!("compiler {pinned} (found: {rustc})"),
        !pinned.is_empty() && rustc.contains(&pinned),
        true,
        "rustup toolchain install   (run in the repo; it reads rust-toolchain.toml)",
    );
    let components = output("rustup", &["component", "list", "--installed"]).unwrap_or_default();
    for component in ["rustfmt", "clippy"] {
        report.item(
            component,
            components.contains(component),
            true,
            "rustup toolchain install",
        );
    }

    if os == "linux" {
        println!("\nLinux libraries Bevy needs to build");
        let pkg_config = output("pkg-config", &["--version"]).is_some();
        report.item(
            "pkg-config",
            pkg_config,
            true,
            "sudo apt install pkg-config",
        );
        for (library, package) in [
            ("alsa", "libasound2-dev"),
            ("libudev", "libudev-dev"),
            ("wayland-client", "libwayland-dev"),
            ("xkbcommon", "libxkbcommon-dev"),
        ] {
            let found = pkg_config && output("pkg-config", &["--exists", library]).is_some();
            report.item(library, found, true, &format!("sudo apt install {package}"));
        }
    }

    if os == "macos" {
        println!("\nmacOS");
        report.item(
            "Xcode command line tools",
            output("xcode-select", &["-p"]).is_some(),
            true,
            "xcode-select --install",
        );
    }

    if on_wsl {
        println!("\nWSL: building and running the Windows version (needed to use the real GPU)");
        let targets = output("rustup", &["target", "list", "--installed"]).unwrap_or_default();
        report.item(
            "Rust target x86_64-pc-windows-gnu",
            targets.contains(WINDOWS_TARGET),
            true,
            "rustup target add x86_64-pc-windows-gnu",
        );
        report.item(
            "MinGW cross-compiler",
            output("x86_64-w64-mingw32-gcc", &["--version"]).is_some(),
            true,
            "sudo apt install mingw-w64",
        );
        report.item(
            "Windows programs can be launched from WSL",
            output("cmd.exe", &["/c", "ver"]).is_some(),
            true,
            "enable Windows interop in /etc/wsl.conf ([interop] enabled=true)",
        );
        println!(
            "\n  WSL has no hardware Vulkan. The Linux build renders on the CPU, which is right\n  \
             for screenshots and wrong for performance. Measure on the Windows build."
        );
    }

    println!("\nTools");
    let node = output("node", &["--version"]).unwrap_or_default();
    report.item(
        &format!("Node 22.18 or later, which runs the TypeScript tools (found: {node})"),
        node_runs_typescript(&node),
        true,
        "install Node 22.18 or later (https://nodejs.org)",
    );

    println!("\nArt tools");
    report.item(
        "ImageMagick, for painting item icons (tools/icons)",
        output("convert", &["-version"]).is_some(),
        false,
        "sudo apt install imagemagick (or brew install imagemagick)",
    );

    println!();
    if report.missing_required {
        println!("Some required items are missing. Fix them and run `cargo xtask doctor` again.");
    } else {
        println!("This machine is ready.");
    }
    !report.missing_required
}

/// Whether a `node --version` is 22.18 or later, the first to run TypeScript as it is.
fn node_runs_typescript(version: &str) -> bool {
    let mut parts = version
        .trim_start_matches('v')
        .split('.')
        .map(|p| p.parse::<u32>().unwrap_or(0));
    let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    major > 22 || (major == 22 && minor >= 18)
}

#[cfg(test)]
mod tests {
    use super::node_runs_typescript;

    #[test]
    fn node_runs_typescript_from_22_18() {
        assert!(node_runs_typescript("v22.18.0"));
        assert!(node_runs_typescript("v24.14.0"));
        assert!(!node_runs_typescript("v22.17.1"));
        assert!(!node_runs_typescript("v20.19.0"));
        assert!(!node_runs_typescript(""));
    }
}

#[cfg(test)]
mod task_tests {
    use super::*;

    #[test]
    fn every_task_has_one_name_and_is_listed() {
        let text = usage();
        for task in TASKS {
            assert_eq!(
                TASKS.iter().filter(|other| other.name == task.name).count(),
                1,
                "{} twice",
                task.name
            );
            assert!(
                text.contains(&format!("  {}", task.name)),
                "{} not listed",
                task.name
            );
        }
    }
}
