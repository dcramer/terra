//! `cargo xtask doctor`: what this machine is missing to build, check and run the game.

use std::path::PathBuf;
use std::time::Duration;

use crate::windows::{WINDOWS_TARGET, on_wsl};
use crate::worktrees::{age, gb};
use crate::{Task, output, root, seed};

/// The task this module runs (D410).
pub(crate) const TASKS: &[Task] = &[Task {
    name: "doctor",
    takes: "",
    does: "what this machine is missing",
    group: "This machine",
    run: |_| doctor(),
}];

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

    /// Something to look at rather than install: never makes the machine unready.
    fn look(&mut self, label: &str, fine: bool, fix: &str) {
        println!("  [{}] {label}", if fine { "ok     " } else { "look   " });
        if !fine {
            println!("            {fix}");
        }
    }
}

pub(crate) fn doctor() -> bool {
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
    git_setup(&mut report);
    let node = output("node", &["--version"]).unwrap_or_default();
    report.item(
        &format!("Node 22.18 or later, which runs the TypeScript tools (found: {node})"),
        node_runs_typescript(&node),
        true,
        "install Node 22.18 or later (https://nodejs.org)",
    );

    println!("\nThe shared seed build and the main checkout (D183)");
    let (current, state) = seed::status();
    report.item(
        &format!("the shared seed build: {state}"),
        current,
        false,
        "cargo xtask seed   (builds it or brings it up to date; --refresh runs its builds again)",
    );
    let (current, state) = seed::main_status();
    report.item(
        &format!("the main checkout: {state}"),
        current,
        false,
        "cargo xtask seed fast-forwards it once it is on main with no changes and nothing unpushed",
    );

    art_tools(&mut report);
    println!("\nProfiling");
    let tracy = crate::tracy::matches();
    report.look(
        tracy.as_ref().unwrap_or_else(|why| why),
        tracy.is_ok(),
        "Tracy's profiler and the game's client must be one version (D577)",
    );
    if os == "linux" {
        leftovers(&mut report, on_wsl);
    }

    println!();
    if report.missing_required {
        println!("Some required items are missing. Fix them and run `cargo xtask doctor` again.");
    } else {
        println!("This machine is ready.");
    }
    !report.missing_required
}

/// Blender, with the decoder for Tripo's models, and ImageMagick.
fn art_tools(report: &mut Report) {
    println!("\nArt tools");
    let pinned = std::fs::read_to_string(root().join("tools/blender/version")).unwrap_or_default();
    let pinned = pinned.trim();
    let blender = output("tools/blender/run.sh", &["--find"])
        .and_then(|path| output(&path, &["--version"]))
        .and_then(|v| v.lines().next().map(str::to_string))
        .unwrap_or_default();
    report.item(
        &format!("Blender {pinned}, for the model pipeline and its check (found: {blender})"),
        blender.starts_with(&format!("Blender {pinned}.")),
        false,
        &format!("install Blender {pinned} (https://www.blender.org/download/lts/); under WSL, the Windows one"),
    );
    // Tripo's models are meshopt-compressed; some Linux distributions' Blender, such as
    // Arch's, leave out the library its glTF importer decodes them with.
    if !blender.is_empty() {
        let meshopt = output("tools/blender/run.sh", &["--find"]).and_then(|path| {
            output(
                &path,
                &[
                    "--background",
                    "--factory-startup",
                    "--python-expr",
                    "from io_scene_gltf2.io.imp.gltf2_io_binary_meshopt import MeshoptDecoder as M\n\
                     try:\n    print('MESHOPT', M.find_library())\nexcept Exception as e:\n    print('NOMESHOPT', e)",
                ],
            )
        });
        let found = meshopt.as_deref().unwrap_or_default();
        let missing = found
            .lines()
            .find_map(|line| line.strip_prefix("NOMESHOPT Meshopt decoder library not found at "))
            .unwrap_or("its glTF add-on folder");
        report.item(
            "Blender opens Tripo's compressed models (meshopt)",
            found.lines().any(|line| line.starts_with("MESHOPT ")),
            false,
            &format!(
                "this Blender was built without libbf_intern_meshopt_bridge.so: copy it from the official \
                 Blender {pinned} for Linux (its lib folder) to {missing}, or install that build and set BLENDER"
            ),
        );
    }
    report.item(
        "ImageMagick, for painting item icons (tools/icons)",
        output("convert", &["-version"]).is_some(),
        false,
        "sudo apt install imagemagick (or brew install imagemagick)",
    );
}

/// Free space under which the disks are reported: a worktree's build is several GB, the seed
/// tens, and on 2026-10-26 117 worktrees had filled the disk to 90%.
const DISK_FREE_GB: u64 = 50;
/// A game, Blender or build running this long was left behind by a session that ended.
const LONG_RUN: Duration = Duration::from_secs(12 * 60 * 60);

/// What sessions left behind (D259): the disks' free space, staged Windows copies, processes
/// whose folder was deleted under them (a polling loop in a removed worktree) and runs going
/// for hours. It reports; it kills and deletes nothing.
fn leftovers(report: &mut Report, on_wsl: bool) {
    println!("\nWhat sessions left behind (D259)");
    let prune = "cargo xtask worktrees shows what each worktree holds; --prune clears the ones agents are done with, \
                 and staged Windows copies idle a day or past 16 GB together";
    let mut disks = vec![("the repository's disk", root())];
    if on_wsl {
        disks.push(("Windows' disk (C:)", PathBuf::from("/mnt/c")));
    }
    for (name, disk) in disks {
        let df = output("df", &["-Pk", &disk.to_string_lossy()]).and_then(|text| disk_space(&text));
        let Some((total, free)) = df else { continue };
        report.look(
            &format!("{name}: {} free of {}", gb(free), gb(total)),
            free >= DISK_FREE_GB * 1_000_000_000 && free * 10 >= total,
            prune,
        );
    }
    if on_wsl {
        let copies = crate::windows::windows_temp()
            .and_then(|temp| std::fs::read_dir(temp.join("terra")).ok())
            .map_or(0, Iterator::count);
        report.look(
            &format!("staged Windows copies of the game: {copies} (about 750 MB each)"),
            copies <= 20,
            prune,
        );
    }
    let found = processes();
    for (kind, label, why) in [
        (
            Leftover::FolderGone,
            "no process left in a deleted folder",
            "their folder is gone; nothing reads what they make",
        ),
        (
            Leftover::LongRun,
            "no game, Blender or build running over 12 hours",
            "if nothing waits on them",
        ),
    ] {
        let these: Vec<&Process> = found.iter().filter(|p| leftover(p) == Some(kind)).collect();
        let mut fix = these
            .iter()
            .map(|p| format!("{} {}: {}\n            ", p.pid, age(p.age), p.command))
            .collect::<String>();
        let pids: Vec<String> = these.iter().map(|p| p.pid.to_string()).collect();
        fix.push_str(&format!("kill {}   ({why})", pids.join(" ")));
        report.look(label, these.is_empty(), &fix);
    }
}

/// A `df -Pk` listing's size and free space, in bytes.
fn disk_space(df: &str) -> Option<(u64, u64)> {
    let fields: Vec<&str> = df.lines().nth(1)?.split_whitespace().collect();
    let kb = |i: usize| fields.get(i)?.parse::<u64>().ok().map(|k| k * 1024);
    Some((kb(1)?, kb(3)?))
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Leftover {
    FolderGone,
    LongRun,
}

/// One of this user's processes, as /proc shows it: its program (the first argument, which may
/// hold spaces, as `Program Files` does) and its whole command line.
struct Process {
    pid: u32,
    age: Duration,
    folder_gone: bool,
    program: String,
    command: String,
}

/// Whether a process was left behind, and how. Claude Code sessions are the developer's to
/// close, and this run and its parents are young.
fn leftover(p: &Process) -> Option<Leftover> {
    let program = p.program.as_str();
    if program.rsplit('/').next() == Some("claude") {
        return None;
    }
    if p.folder_gone {
        return Some(Leftover::FolderGone);
    }
    let run = ["terra_", "blender", "cargo", "rustc", "xtask"]
        .iter()
        .any(|name| program.to_lowercase().contains(name));
    (run && p.age > LONG_RUN).then_some(Leftover::LongRun)
}

/// This user's processes (others' have no readable folder), from /proc.
fn processes() -> Vec<Process> {
    let uptime = std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|text| text.split_whitespace().next()?.parse::<f64>().ok());
    let Some(uptime) = uptime else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        let dir = entry.path();
        let (Ok(folder), Ok(stat), Ok(cmdline)) = (
            std::fs::read_link(dir.join("cwd")),
            std::fs::read_to_string(dir.join("stat")),
            std::fs::read(dir.join("cmdline")),
        ) else {
            continue;
        };
        let Some(started) = started_s(&stat) else {
            continue;
        };
        let command: String = String::from_utf8_lossy(&cmdline)
            .replace('\0', " ")
            .trim()
            .chars()
            .take(100)
            .collect();
        found.push(Process {
            pid,
            age: Duration::from_secs_f64((uptime - started).max(0.0)),
            folder_gone: folder.to_string_lossy().ends_with(" (deleted)"),
            program: String::from_utf8_lossy(cmdline.split(|b| *b == 0).next().unwrap_or_default())
                .into_owned(),
            command,
        });
    }
    found
}

/// When a process started, in seconds after boot, from its /proc stat line: the 22nd field, in
/// clock ticks of 1/100 s (Linux's USER_HZ), counted after the command's closing bracket since
/// the command may hold spaces.
fn started_s(stat: &str) -> Option<f64> {
    let after = &stat[stat.rfind(')')? + 1..];
    let ticks: f64 = after.split_whitespace().nth(19)?.parse().ok()?;
    Some(ticks / 100.0)
}

/// What git is set up to do for the repository: the hooks (D37) and the rendered
/// indexes' merge driver (D409), both set by any `cargo xtask` task.
fn git_setup(report: &mut Report) {
    report.item(
        "git runs the formatter before each commit and the formatter and clippy before each push",
        output("git", &["config", "--get", "core.hooksPath"]).as_deref() == Some(".githooks"),
        true,
        "git config core.hooksPath .githooks   (any cargo xtask task sets it)",
    );
    report.item(
        "git merges the rendered indexes row by row (D409)",
        output("git", &["config", "--get", "merge.rendered.driver"]).as_deref()
            == Some(crate::MERGE_DRIVER),
        true,
        "any cargo xtask task sets merge.rendered.driver",
    );
    report.item(
        "git-lfs turns the binary assets' pointers into files on checkout and back on commit (D525)",
        output("git", &["lfs", "version"]).is_some()
            && output("git", &["config", "--get", "filter.lfs.process"]).is_some(),
        true,
        "install git-lfs (`mise use -g git-lfs@latest`, or https://git-lfs.com), then `git lfs install --skip-repo` and `git lfs pull`",
    );
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
    use super::{
        Duration, Leftover, Process, disk_space, leftover, node_runs_typescript, started_s,
    };

    #[test]
    fn a_df_listing_gives_size_and_free_bytes() {
        let df = "Filesystem 1024-blocks Used Available Capacity Mounted on\n\
                  /dev/sdd 1055762868 950186460 52 90% /\n";
        assert_eq!(disk_space(df), Some((1055762868 * 1024, 52 * 1024)));
        assert_eq!(disk_space("Filesystem\n"), None);
    }

    #[test]
    fn a_start_is_read_after_the_commands_bracket() {
        // pid (comm) state, then fields 4 to 21, then starttime (22nd) in ticks.
        let stat = "42 (zsh -c until x) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 123456 0 0";
        assert_eq!(started_s(stat), Some(1234.56));
    }

    #[test]
    fn leftovers_are_runs_in_gone_folders_or_long_runs_never_a_session() {
        let process = |command: &str, hours: u64, folder_gone: bool| Process {
            pid: 1,
            age: Duration::from_secs(hours * 3600),
            folder_gone,
            program: command.split(" -").next().unwrap_or_default().into(),
            command: command.into(),
        };
        let polling = "/usr/bin/zsh -c source /home/u/.claude/shell-snapshots/s.sh && until test -f x; do sleep 3; done";
        assert_eq!(
            leftover(&process(polling, 1, true)),
            Some(Leftover::FolderGone)
        );
        let game =
            "/mnt/c/Users/u/AppData/Local/Temp/terra/terra_sandbox-duel/terra_sandbox.exe --duel";
        assert_eq!(leftover(&process(game, 13, false)), Some(Leftover::LongRun));
        assert_eq!(leftover(&process(game, 2, false)), None);
        assert_eq!(
            leftover(&process(
                "/mnt/c/Program Files/Blender/blender.exe -b",
                30,
                false
            )),
            Some(Leftover::LongRun)
        );
        assert_eq!(
            leftover(&process("/home/u/.local/bin/claude --resume", 90, true)),
            None
        );
        assert_eq!(leftover(&process("/usr/bin/zsh", 90, false)), None);
    }

    #[test]
    fn node_runs_typescript_from_22_18() {
        assert!(node_runs_typescript("v22.18.0"));
        assert!(node_runs_typescript("v24.14.0"));
        assert!(!node_runs_typescript("v22.17.1"));
        assert!(!node_runs_typescript("v20.19.0"));
        assert!(!node_runs_typescript(""));
    }
}
