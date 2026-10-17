//! `cargo xtask check` (D37, D398): what has to pass before a commit and a push, each scoped
//! to what the change touches (`scope`), and the gates only it runs: the tools' tests, the
//! Blender scripts, and the shared crates kept free of Bevy (D28) and of the clock and the
//! system's randomness.
//!
//! Each stage runs quietly (D388, `stage`): a line when it passes, only what is needed to fix
//! it when it fails, everything in `target/check/<stage>.log`. `--verbose`, or
//! `TERRA_VERBOSE=1`, streams everything as it is said.

use crate::build::{KITS, artifacts_command, fuzz_args};
use crate::scope::{self, Scope};
use crate::stage::{self, Said, Step};
use crate::{output, root};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// The only crates allowed to depend on Bevy (D28): the applications, and the world on
/// screen that the game stands on (D198).
const BEVY_CRATES: [&str; 3] = ["terra_game", "terra_sandbox", "terra_world"];

/// Tests too slow for the check before a commit, by the exact name the test harness prints:
/// sweeps over every case of something, and tests that lay a town, a village or a stretch of
/// land, each over five seconds on the loaded 24-core machine (2026-10-17, D398). The check
/// before a commit skips them; the check before a push, and `cargo test`, run them. Each is
/// marked where it is written too. A test that slows past that joins them.
const SLOW_TESTS: &[&str] = &[
    // terra_build's drawing swept over every case: minutes each.
    "sections::tests::a_part_whose_key_is_as_it_was_draws_as_it_was_and_parts_drawn_alone_draw_as_with_the_rest",
    "sections::tests::a_rectangle_is_drawn_closed_in_every_state_it_is_drawn_in",
    "sections::tests::every_junction_case_is_drawn_without_a_fault",
    "sections::tests::kits_read_again_with_one_taken_out_added_or_reordered_lay_and_draw_without_fault",
    "sections::tests::a_section_drawn_again_from_how_it_was_drawn_is_drawn_as_afresh",
    "sections::tests::the_sections_merged_are_the_sum_of_the_pieces",
    "sections::tests::cubes_of_a_kind_meeting_only_along_an_edge_each_dress_their_own_side",
    "sections::tests::a_gable_is_cut_in_a_straight_line_to_the_roofs_deck_with_no_gap_under_it",
    "sections::tests::an_edit_laid_again_only_as_far_as_it_reaches_is_as_laid_whole",
    "sections::tests::whatever_changes_a_section_is_in_what_a_change_redraws",
    "roofing::tests::no_part_of_a_roof_lies_over_itself",
    "layout::runs::tests::a_roof_over_a_rooms_flat_top_is_closed_to_its_walls_whichever_way_it_rises",
    "layout::roofs::tests::an_eave_over_a_wall_running_on_from_the_room_is_flashed_against_it",
    "scenes::registry::tests::every_registered_scene_is_in_the_ratchet_and_every_family_before_a_push",
    "packs::tests::every_shipped_material_passes_and_a_real_scan_planted_wrong_fails",
    // One of each sort of the village's buildings laid and drawn whole.
    "village::tests::no_two_faces_of_the_villages_buildings_fight_for_the_same_pixels",
    // A town laid on its ground.
    "town::tests::builders_sites_stand_outside_every_building_facing_their_own",
    "town::tests::a_town_has_its_count_of_houses_a_market_a_forge_and_gates",
    "town::tests::every_door_opens_onto_a_street_or_the_square",
    "town::tests::nothing_built_stands_on_another_or_in_a_street_or_on_the_square",
    "town::tests::the_same_seed_and_ground_make_the_same_town_and_others_differ",
    "town::tests::the_path_graph_is_one_and_every_walk_on_it_is_clear",
    "town::tests::streets_follow_the_contour_rather_than_climb",
    "town::market::tests::some_stalls_hang_a_lantern_and_others_not",
    "town::market::tests::the_ground_beside_a_stall_shows_its_own_trades_goods",
    "town::market::tests::neighbouring_stalls_sell_different_things_and_seeds_differ",
    "town::market::tests::a_counters_goods_are_its_trades_and_lie_on_it_apart",
    "town::market::tests::neighbouring_stalls_wear_different_awnings",
    // terra_life's town lived through a day.
    "tests::walkers_go_round_a_visitor_standing_in_their_way",
    "tests::the_same_town_and_seed_live_the_same_day",
    "tests::a_town_started_settled_is_in_mid_flow_not_stood_still",
    // terra_land's places found on the land, and its snow and woods worked out over it.
    "town::tests::the_town_found_is_level_warm_wooded_round_and_under_peaks",
    "round::tests::round_a_mountain_lake_in_early_summer_its_shore_is_bare_and_wooded_and_its_peaks_white",
    "woods::tests::round_a_mountain_lake_spruce_holds_the_shade_and_pine_the_sun",
    "woods::tests::the_seasons_worked_out_ahead_are_the_seasons",
    "site::tests::the_shore_walk_keeps_to_dry_shore_below_the_trees",
    "site::tests::a_pinned_lake_is_the_one_found_there_and_none_where_none_fits",
    "site::tests::the_lake_found_meets_what_was_asked_of_it",
    "tundra::tests::the_tundra_found_is_past_the_trees_over_permafrost_with_the_forest_in_sight",
    "water::tests::a_lake_stands_at_its_level_over_its_own_ground_and_the_sea_elsewhere",
    "snow::tests::early_summer_snow_lies_on_the_heights_longest_facing_the_pole",
    "snow::tests::a_lee_hollow_keeps_its_snow_longer_than_open_ground_and_a_rise_less",
    // terra_voxel's mountains sampled finely across a range, and a planet's edits redrawn.
    "mountains::tests::the_mountains_never_jump",
    "mountains::tests::the_ground_is_smooth_within_a_voxel",
    "planet::tests::what_to_draw_again_holds_everything_the_edits_changed_near_the_surface",
];

/// Where the check's stages keep their logs, relative to the root.
const LOGS: &str = "target/check";

/// How long the check before a commit should take on a built tree, in seconds (D398): the
/// developers' "a few to few tens of seconds". It is not enforced; the check says when it is
/// over, so drift shows.
const COMMIT_BUDGET_SECS: u64 = 30;

/// How a stage runs: a command, a check made here, or not at all, and why.
enum How {
    Command(Command),
    Here(fn(&mut String) -> bool),
    Skip(&'static str),
}

/// Which gate is running: the one before a commit, or the one before a push.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tier {
    Commit,
    Push,
}

/// The gate (D37, D398). Before a commit (`--quick`), on the staged files, only what is fast:
/// formatting, the code-health gates, clippy and the quick tests of the crates changed, the
/// tests of the tools changed, the docs check and the shared crates' rules. Before a push
/// (no flag), on everything that differs from origin/main: clippy and every test of the crates
/// changed and every crate depending on them, the tools' tests, and the kit, foliage,
/// artifacts, fuzz and Blender checks when what they read changed. `--all` runs a tier over
/// everything. The stages run in order and the first to fail ends it.
pub(crate) fn check(args: &[String]) -> bool {
    let mut tier = Tier::Push;
    let mut all = false;
    for arg in args {
        match arg.as_str() {
            "--quick" => tier = Tier::Commit,
            "--all" => all = true,
            "--verbose" => stage::set_verbose(),
            _ => {
                eprintln!("usage: cargo xtask check [--quick] [--all] [--verbose]");
                return false;
            }
        }
    }
    let started = Instant::now();
    let scope = match (all, tier) {
        (true, _) => Scope::everything("--all"),
        (false, Tier::Commit) => scope::commit(),
        (false, Tier::Push) => scope::push(),
    };
    let name = match tier {
        Tier::Commit => "quick check",
        Tier::Push => "check",
    };
    println!("{name} over {}", scope.summary());
    let stages = stages(tier, &scope);
    let count = stages.len();
    let mut skipped = Vec::new();
    for (done, (step, how)) in stages.into_iter().enumerate() {
        let passed = match how {
            How::Command(command) => step.run(command),
            How::Here(check) => step.run_here(check),
            How::Skip(why) => {
                skipped.push(format!("{} ({why})", step.name()));
                true
            }
        };
        if !passed {
            println!(
                "✗ {name} failed at {} ({done} of {count} stages passed, {}); TERRA_VERBOSE=1 shows everything",
                step.name(),
                stage::duration(started.elapsed()),
            );
            return false;
        }
    }
    if !skipped.is_empty() {
        println!("- not run: {}", skipped.join(", "));
    }
    let took = started.elapsed();
    match tier {
        Tier::Commit => {
            let over = if took.as_secs() > COMMIT_BUDGET_SECS {
                ", OVER its budget: put the slow stage's work before the push (D398)"
            } else {
                ""
            };
            println!(
                "✓ quick check passed in {} (budget {COMMIT_BUDGET_SECS}s){over}. The push's \
                 check adds the crates depending on these, {} slow tests, the kit, foliage, \
                 artifacts, fuzz and Blender checks.",
                stage::duration(took),
                SLOW_TESTS.len()
            );
        }
        Tier::Push => println!("✓ check passed in {}", stage::duration(took)),
    }
    true
}

/// The packages a tier lints and tests: before a commit those changed, before a push those
/// changed or reading a changed file and every package depending on them, with xtask, whose
/// tests read every Rust source, whenever Rust changed. `None` is the whole workspace.
fn packages(tier: Tier, scope: &Scope) -> Option<BTreeSet<String>> {
    if scope.all() {
        return None;
    }
    if tier == Tier::Commit {
        return Some(scope.changed.clone());
    }
    let mut asked: BTreeSet<String> = scope.changed.union(&scope.readers).cloned().collect();
    // Cargo's tree knows the crates, not the tasks: xtask depends on none of them.
    let tasks = asked.remove("xtask");
    let mut found = scope::with_dependents(&asked)?;
    if tasks || scope.rust || !found.is_empty() {
        found.insert("xtask".into());
    }
    Some(found)
}

/// The check's stages in the order they run, each with how to run it.
fn stages(tier: Tier, scope: &Scope) -> Vec<(Step, How)> {
    // CI only needs to know that everything compiles and the tests pass, so it skips
    // optimising dependencies, which is most of the time spent building Bevy.
    let ci: &[&str] = if std::env::var_os("CI").is_some() {
        &["--config", "profile.dev.package.\"*\".opt-level=0"]
    } else {
        &[]
    };
    // Cargo's own quiet modes: no progress, the compiler's messages a line each, the test
    // harness a dot a test. What they leave out is in the stage's log when streamed.
    let quiet: &[&str] = if stage::verbose() {
        &[]
    } else {
        &["-q", "--message-format=short"]
    };
    let cargo = |command: &str, rest: &[String]| {
        let mut cargo = Command::new("cargo");
        cargo.arg(command).args(ci).args(quiet).args(rest);
        cargo
    };
    let command = |program: &str, args: &[&str]| {
        let mut command = Command::new(program);
        command.args(args);
        command
    };
    let all = scope.all();
    let push = tier == Tier::Push;
    let packages = packages(tier, scope);
    let selected = match &packages {
        None => vec!["--workspace".to_string()],
        Some(names) => scope::package_args(names),
    };
    let rust = packages.as_ref().is_none_or(|names| !names.is_empty());
    let shown = selected.join(" ");
    let mut clippy = selected.clone();
    clippy.extend(["--all-targets", "--", "-D", "warnings"].map(String::from));
    let mut tests = selected.clone();
    if !push {
        // Before a commit: the crates' own tests, not their examples' builds or doc tests,
        // and not the slow ones.
        tests.extend(["--tests", "--", "--exact"].map(String::from));
        for name in SLOW_TESTS {
            tests.extend(["--skip".to_string(), name.to_string()]);
        }
    }
    let lint_and_test = |name: &str, how: How| -> (Step, How) {
        let rerun = match name {
            "clippy" => format!("cargo clippy {shown} --all-targets -- -D warnings"),
            _ => format!("cargo test {shown}"),
        };
        let step = Step::new(name, Said::Cargo, LOGS).rerun(rerun);
        if rust {
            (step, how)
        } else {
            (step, How::Skip("no crate changed"))
        }
    };
    // Building, or what it stands on, changed, or the tasks that run its checks did (xtask is
    // among the packages tested whenever any Rust changed, so its own change is asked of the scope).
    let builds = all
        || scope.changed.contains("xtask")
        || packages
            .as_ref()
            .is_some_and(|names| names.contains("terra_build"));
    let push_only = |step: Step, runs: bool, how: How, why: &'static str| -> (Step, How) {
        match (push, runs) {
            (false, _) => (step, How::Skip("before a push")),
            (true, false) => (step, How::Skip(why)),
            (true, true) => (step, how),
        }
    };
    let mut stages = vec![
        fmt(scope),
        (
            Step::new("sizes", Said::Printed, LOGS).rerun("cargo xtask sizes"),
            How::Here(crate::sizes::sizes_hold),
        ),
        (
            Step::new("one definition", Said::Printed, LOGS),
            How::Here(crate::primitives::one_definition),
        ),
        lint_and_test("clippy", How::Command(cargo("clippy", &clippy))),
        lint_and_test("tests", How::Command(cargo("test", &tests))),
        tools_tests(push || all, &scope.tools),
        push_only(
            Step::new("kits", Said::Cargo, LOGS).rerun("cargo xtask kits"),
            builds,
            How::Command(command("cargo", KITS)),
            "building unchanged",
        ),
        push_only(
            Step::new("foliage", Said::Printed, LOGS).rerun("cargo xtask foliage"),
            all || scope.assets || scope.tools.contains("textures"),
            How::Command(command("node", &["tools/textures/foliage.ts"])),
            "no assets or textures tool changed",
        ),
    ];
    let artifacts = artifacts_command(&["--quick".into(), "--ratchet".into()]);
    let fuzz = artifacts_command(&fuzz_args(&["--ratchet".into()]));
    for (name, rerun, (program, args)) in [
        (
            "artifacts",
            "cargo xtask artifacts --quick --ratchet",
            artifacts,
        ),
        ("fuzz", "cargo xtask fuzz --ratchet", fuzz),
    ] {
        let mut run = Command::new(program);
        run.args(args);
        stages.push(push_only(
            Step::new(name, Said::Cargo, LOGS).rerun(rerun),
            builds,
            How::Command(run),
            "building unchanged",
        ));
    }
    stages.push((
        Step::new("docs", Said::Printed, LOGS).rerun("node tools/docs/docs.ts"),
        How::Command(command("node", &["tools/docs/docs.ts"])),
    ));
    let (step, how) = blender_scripts();
    stages.push(push_only(
        step,
        all || scope.python || scope.tools.contains("blender"),
        how,
        "no Blender script changed",
    ));
    stages.push((
        Step::new("shared crates free of Bevy", Said::Printed, LOGS),
        if all || scope.manifests {
            How::Here(shared_crates_avoid_bevy)
        } else {
            How::Skip("no manifest changed")
        },
    ));
    stages.push((
        Step::new("shared crates repeatable", Said::Printed, LOGS),
        How::Here(shared_crates_are_repeatable),
    ));
    stages
}

/// Formatting, of the crates changed: it goes file by file, so they are all a change can
/// unsettle.
fn fmt(scope: &Scope) -> (Step, How) {
    let step = Step::new("fmt", Said::Fmt, LOGS).rerun("cargo fmt --all (it fixes them)");
    if !scope.all() && !scope.rust {
        return (step, How::Skip("no Rust changed"));
    }
    let mut fmt = Command::new("cargo");
    fmt.args(["fmt", "--check"]);
    if scope.all() || scope.changed.is_empty() {
        fmt.arg("--all");
    } else {
        fmt.args(scope::package_args(&scope.changed));
    }
    (step, How::Command(fmt))
}

/// The tools' tests (D121): every one, or before a commit those of the tools changed.
fn tools_tests(every: bool, changed: &BTreeSet<String>) -> (Step, How) {
    let step = Step::new("tools tests", Said::NodeTest, LOGS)
        .rerun("node --test $(git ls-files 'tools/**/*.test.ts' 'tools/**/*.test.mjs')");
    if !every && changed.is_empty() {
        return (step, How::Skip("no tool changed"));
    }
    let Some(files) = output(
        "git",
        &["ls-files", "tools/**/*.test.ts", "tools/**/*.test.mjs"],
    ) else {
        return (
            step,
            How::Here(|out| {
                out.push_str("git cannot list the tools' tests\n");
                false
            }),
        );
    };
    let chosen: Vec<&str> = files
        .lines()
        .filter(|file| {
            every
                || changed
                    .iter()
                    .any(|dir| file.starts_with(&format!("tools/{dir}/")))
        })
        .collect();
    if chosen.is_empty() {
        return (step, How::Skip("the tools changed have no tests"));
    }
    let mut node = Command::new("node");
    node.args(["--test", "--test-reporter=spec"]).args(chosen);
    (step, How::Command(node))
}

/// The Blender scripts (D121) compile and find what they import, checked inside the pinned
/// Blender without running them. A machine with no Blender skips this.
fn blender_scripts() -> (Step, How) {
    let step = Step::new("Blender scripts", Said::Printed, LOGS)
        .rerun("tools/blender/run.sh tools/blender/check.py $(git ls-files 'tools/**/*.py')");
    if output("tools/blender/run.sh", &["--find"]).is_none() {
        return (step, How::Skip("no Blender here (cargo xtask doctor)"));
    }
    let Some(files) = output("git", &["ls-files", "tools/**/*.py"]) else {
        return (
            step,
            How::Here(|out| {
                out.push_str("git cannot list them\n");
                false
            }),
        );
    };
    let mut run = Command::new("tools/blender/run.sh");
    run.arg("tools/blender/check.py").args(files.lines());
    (step, How::Command(run))
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
fn shared_crates_avoid_bevy(out: &mut String) -> bool {
    let _ = writeln!(out, "shared-state crates must not depend on Bevy (D28)");
    let Some(crates) = shared_crates() else {
        let _ = writeln!(out, "  cannot list the crates");
        return false;
    };
    let mut ok = true;
    for (name, _) in crates {
        let tree = output(
            "cargo",
            &["tree", "-p", &name, "-e", "normal", "--prefix", "none"],
        );
        let verdict = bevy_verdict(tree.as_deref());
        let _ = writeln!(out, "  {name}: {verdict}");
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

fn shared_crates_are_repeatable(out: &mut String) -> bool {
    let _ = writeln!(
        out,
        "shared-state crates must not read the clock or the system's randomness"
    );
    let Some(crates) = shared_crates() else {
        let _ = writeln!(out, "  cannot list the crates");
        return false;
    };
    let mut ok = true;
    for (name, path) in crates {
        let mut found = Vec::new();
        if !sources(&path.join("src"), &mut found) || found.is_empty() {
            let _ = writeln!(out, "  {name}: cannot read its sources");
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
                let _ = writeln!(out, "  {at}: cannot read it");
                clean = false;
                continue;
            };
            for (number, what) in unrepeatable(&text) {
                let _ = writeln!(out, "  {at}:{number}: {what}");
                clean = false;
            }
        }
        if clean {
            let _ = writeln!(out, "  {name}: ok");
        }
        ok &= clean;
    }
    ok
}

/// Every Rust file under a folder. False if any folder in it could not be read.
pub(crate) fn sources(folder: &Path, found: &mut Vec<PathBuf>) -> bool {
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
