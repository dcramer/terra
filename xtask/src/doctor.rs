//! `cargo xtask doctor`: what this machine is missing to build, check and run the game.

use crate::windows::{WINDOWS_TARGET, on_wsl};
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

    println!();
    if report.missing_required {
        println!("Some required items are missing. Fix them and run `cargo xtask doctor` again.");
    } else {
        println!("This machine is ready.");
    }
    !report.missing_required
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
