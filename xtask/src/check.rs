//! `cargo xtask check` (D37): what has to pass before a commit and a push, and the gates
//! only it runs: the tools' tests, the Blender scripts, and the shared crates kept free of
//! Bevy (D28) and of the clock and the system's randomness.

use crate::build::{artifacts, kits};
use crate::{output, root, run};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The only crates allowed to depend on Bevy (D28): the applications, and the world on
/// screen that the game stands on (D198).
const BEVY_CRATES: [&str; 3] = ["terra_game", "terra_sandbox", "terra_world"];

/// Tests too slow to run before every commit, each a sweep over every case of something, by
/// the exact name the test harness prints. The quick check skips them; the full check, and
/// `cargo test`, run them. Each is marked where it is written too.
const SLOW_TESTS: &[&str] = &[
    // terra_build's drawing swept over every case: about a minute each on one thread.
    "sections::tests::a_part_whose_key_is_as_it_was_draws_as_it_was_and_parts_drawn_alone_draw_as_with_the_rest",
    "sections::tests::a_rectangle_is_drawn_closed_in_every_state_it_is_drawn_in",
    "sections::tests::every_junction_case_is_drawn_without_a_fault",
];

/// The gate (D37). The full check, run by git before each push: formatting, lints, every test,
/// and the checks of the kits, the foliage, the meshes' artifacts (a ratchet), the docs, the
/// Blender scripts and the shared crates. `--quick`, run before each commit, leaves out the slow
/// sweeps, the artifacts ratchet (a release build) and the Blender scripts' check.
pub(crate) fn check(args: &[String]) -> bool {
    let quick = match args {
        [] => false,
        [flag] if flag == "--quick" => true,
        _ => {
            eprintln!("usage: cargo xtask check [--quick]");
            return false;
        }
    };
    // CI only needs to know that everything compiles and the tests pass, so it skips
    // optimising dependencies, which is most of the time spent building Bevy.
    let ci: &[&str] = if std::env::var_os("CI").is_some() {
        &["--config", "profile.dev.package.\"*\".opt-level=0"]
    } else {
        &[]
    };
    let cargo = |command: &'static str, rest: &[&'static str]| {
        run("cargo", &[&[command], ci, rest].concat())
    };
    let mut tests = vec!["--workspace"];
    if quick {
        tests.extend(["--", "--exact"]);
        for name in SLOW_TESTS {
            tests.extend(["--skip", name]);
        }
    }
    let passed = run("cargo", &["fmt", "--all", "--check"])
        && cargo(
            "clippy",
            &["--workspace", "--all-targets", "--", "-D", "warnings"],
        )
        && cargo("test", &tests)
        && tools_tests()
        && kits()
        && run("node", &["tools/textures/foliage.ts"])
        && (quick || artifacts(&["--quick".into(), "--ratchet".into()]))
        && run("node", &["tools/docs/docs.ts"])
        && (quick || blender_scripts())
        && shared_crates_avoid_bevy()
        && shared_crates_are_repeatable();
    if passed && quick {
        println!(
            "\nThe quick check passed. The full check (`cargo xtask check`), which git runs \
             before a push, adds {} slow tests, the artifacts ratchet and the Blender scripts' check.",
            SLOW_TESTS.len()
        );
    }
    passed
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

/// The Blender scripts (D121) compile and find what they import, checked inside the pinned
/// Blender without running them. A machine with no Blender skips this.
fn blender_scripts() -> bool {
    println!("\n> Blender scripts (tools/blender/check.py)");
    if output("tools/blender/run.sh", &["--find"]).is_none() {
        println!("  skipped: no Blender here (cargo xtask doctor)");
        return true;
    }
    let Some(files) = output("git", &["ls-files", "tools/**/*.py"]) else {
        println!("  git cannot list them");
        return false;
    };
    Command::new("tools/blender/run.sh")
        .arg("tools/blender/check.py")
        .args(files.lines())
        .current_dir(root())
        .status()
        .is_ok_and(|status| status.success())
}

/// The crates that must not depend on Bevy, by name and folder, or `None` when the crates
/// folder cannot be read. A check that cannot list them fails rather than passing.
fn shared_crates() -> Option<Vec<(String, PathBuf)>> {
    let entries = std::fs::read_dir(root().join("crates")).ok()?;
    let mut found: Vec<(String, PathBuf)> = entries
        .flatten()
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().to_string(),
                entry.path(),
            )
        })
        .filter(|(name, path)| {
            !BEVY_CRATES.contains(&name.as_str()) && path.join("Cargo.toml").exists()
        })
        .collect();
    found.sort();
    (!found.is_empty()).then_some(found)
}

/// What `cargo tree` says of a crate's dependencies, or `None` when it gave no answer:
/// "ok" only when it answered and Bevy is not among them.
fn bevy_verdict(tree: Option<&str>) -> &'static str {
    match tree {
        None => "CARGO TREE FAILED",
        Some(tree) if tree.lines().any(|line| line.starts_with("bevy")) => "DEPENDS ON BEVY",
        Some(_) => "ok",
    }
}

/// Anything every player must agree on lives in crates that know nothing about Bevy.
fn shared_crates_avoid_bevy() -> bool {
    println!("\n> shared-state crates must not depend on Bevy (D28)");
    let Some(crates) = shared_crates() else {
        println!("  cannot list the crates");
        return false;
    };
    let mut ok = true;
    for (name, _) in crates {
        let tree = output(
            "cargo",
            &["tree", "-p", &name, "-e", "normal", "--prefix", "none"],
        );
        let verdict = bevy_verdict(tree.as_deref());
        println!("  {name}: {verdict}");
        ok &= verdict == "ok";
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

/// The lines of a source that use something in [`UNREPEATABLE`], numbered from 1.
fn unrepeatable(text: &str) -> Vec<(usize, &'static str)> {
    text.lines()
        .enumerate()
        .filter_map(|(number, line)| {
            let what = UNREPEATABLE.iter().find(|what| line.contains(*what))?;
            Some((number + 1, *what))
        })
        .collect()
}

fn shared_crates_are_repeatable() -> bool {
    println!("\n> shared-state crates must not read the clock or the system's randomness");
    let Some(crates) = shared_crates() else {
        println!("  cannot list the crates");
        return false;
    };
    let mut ok = true;
    for (name, path) in crates {
        let mut found = Vec::new();
        if !sources(&path.join("src"), &mut found) || found.is_empty() {
            println!("  {name}: cannot read its sources");
            ok = false;
            continue;
        }
        let mut clean = true;
        for file in found {
            // A benchmark's own program may time itself.
            if file.ends_with("main.rs") || file.components().any(|part| part.as_os_str() == "bin")
            {
                continue;
            }
            let at = file.strip_prefix(root()).unwrap_or(&file).display();
            let Ok(text) = std::fs::read_to_string(&file) else {
                println!("  {at}: cannot read it");
                clean = false;
                continue;
            };
            for (number, what) in unrepeatable(&text) {
                println!("  {at}:{number}: {what}");
                clean = false;
            }
        }
        if clean {
            println!("  {name}: ok");
        }
        ok &= clean;
    }
    ok
}

/// Every Rust file under a folder. False if any folder in it could not be read.
fn sources(folder: &Path, found: &mut Vec<PathBuf>) -> bool {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return false;
    };
    let mut ok = true;
    for entry in entries {
        let Ok(entry) = entry else {
            ok = false;
            continue;
        };
        let path = entry.path();
        if path.is_dir() {
            ok &= sources(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    ok
}

#[cfg(test)]
mod task_tests {
    use super::*;

    /// A slow test renamed, or its mark left behind, would be run by the quick check again
    /// without anyone noticing, or skipped with no sign at the test.
    #[test]
    fn every_slow_test_is_written_and_marked_where_it_is() {
        let mut files = Vec::new();
        assert!(sources(&root().join("crates"), &mut files));
        let texts: Vec<String> = files
            .iter()
            .filter_map(|f| std::fs::read_to_string(f).ok())
            .collect();
        assert!(!SLOW_TESTS.is_empty());
        for name in SLOW_TESTS {
            let function = name.rsplit("::").next().unwrap();
            let marked = texts.iter().any(|text| {
                let lines: Vec<&str> = text.lines().collect();
                lines.iter().enumerate().any(|(i, line)| {
                    line.trim_start().starts_with(&format!("fn {function}("))
                        && lines[i.saturating_sub(4)..i]
                            .iter()
                            .any(|l| l.contains("SLOW_TESTS"))
                })
            });
            assert!(
                marked,
                "{name}: no test of that name marked `SLOW_TESTS` above it"
            );
        }
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;

    #[test]
    fn only_an_answer_without_bevy_passes() {
        let tree = "terra_gen v0.1.0\nglam v0.30.0\nlibm v0.2.15";
        assert_eq!(bevy_verdict(Some(tree)), "ok");
        let with = "terra_gen v0.1.0\nglam v0.30.0\nbevy_math v0.17.0";
        assert_ne!(bevy_verdict(Some(with)), "ok");
        // No answer from cargo is a failure, not a pass.
        assert_ne!(bevy_verdict(None), "ok");
    }

    #[test]
    fn the_clock_and_the_systems_randomness_are_found_by_line() {
        let text = "let a = 1;\nlet t = Instant::now();\nlet b = 2;\nlet r = rand::random::<u8>();";
        assert_eq!(
            unrepeatable(text),
            vec![(2, "Instant::now"), (4, "rand::random")]
        );
        assert!(unrepeatable("let seed = Seed(1).rng();").is_empty());
    }

    #[test]
    fn the_shared_crates_are_found_and_the_bevy_ones_are_not() {
        let crates = shared_crates().expect("the crates folder is readable");
        let names: Vec<&str> = crates.iter().map(|(name, _)| name.as_str()).collect();
        assert!(names.contains(&"terra_core") && names.contains(&"terra_voxel"));
        assert!(BEVY_CRATES.iter().all(|bevy| !names.contains(bevy)));
    }
}
