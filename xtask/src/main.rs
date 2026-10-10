//! Project tasks. Run as `cargo xtask <task>`.
//!
//!   check    Everything a change must pass: formatting, lints, tests, and the rule
//!            that shared-state crates do not depend on Bevy. CI runs exactly this.
//!   doctor   Checks this machine has what it needs to build and run the project.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

/// The only crate allowed to depend on Bevy (D28).
const BEVY_CRATE: &str = "terra_game";

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let ok = match task.as_str() {
        "check" => check(),
        "doctor" => doctor(),
        _ => {
            eprintln!("usage: cargo xtask <check|doctor>");
            false
        }
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
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

fn check() -> bool {
    run("cargo", &["fmt", "--all", "--check"])
        && run(
            "cargo",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        )
        && run("cargo", &["test", "--workspace"])
        && shared_crates_avoid_bevy()
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
        if name == BEVY_CRATE || !entry.path().join("Cargo.toml").exists() {
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
    let on_wsl = std::fs::read_to_string("/proc/version")
        .is_ok_and(|v| v.to_lowercase().contains("microsoft"));
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
        rustc.contains(&pinned),
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
            targets.contains("x86_64-pc-windows-gnu"),
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

    println!();
    if report.missing_required {
        println!("Some required items are missing. Fix them and run `cargo xtask doctor` again.");
    } else {
        println!("This machine is ready.");
    }
    !report.missing_required
}
