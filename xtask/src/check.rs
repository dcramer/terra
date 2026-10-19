//! `cargo xtask check` (D37, D398): what has to pass before a commit and a push, each scoped
//! to what the change touches (`scope`), and the full check CI runs on every push to main; and
//! the gates only it runs: the tools' tests, the Blender scripts, and the shared crates kept
//! free of Bevy (D28) and of the clock and the system's randomness.
//!
//! Each stage runs quietly (D388, `stage`): a line when it passes, only what is needed to fix
//! it when it fails, everything in `target/check/<stage>.log`. `--verbose`, or
//! `TERRA_VERBOSE=1`, streams everything as it is said.

use crate::build::{KITS, artifacts_command, fuzz_args};
use crate::scope::{self, Scope};
use crate::stage::{self, Said, Step};
use crate::{Task, land, output, root, slot, stamp};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// The task this module runs (D410).
pub(crate) const TASKS: &[Task] = &[Task {
    name: "check",
    takes: "[--quick | --full] [--all] [--pushing] [--verbose]",
    does: "what every change must pass, scoped to what it touches (D37, D398): before each push, formatting, lints and the fast tests of the crates changed since main; --quick, before each commit, the same on the staged files in a 30 s budget; --full adds the crates depending on them, the slow tests, and the kit, foliage, artifact, fuzz and Blender checks that can see it; --all is the full check over everything, as CI runs it on main",
    group: "Play and check",
    run: check,
}];

/// The only crates allowed to depend on Bevy (D28): the applications, and the world on
/// screen that the game stands on (D198).
const BEVY_CRATES: [&str; 3] = ["terra_game", "terra_sandbox", "terra_world"];

/// Tests too slow for the check before a commit, by the exact name the test harness prints:
/// sweeps over every case of something, and tests that lay a town, a village or a stretch of
/// land, each over five seconds on the loaded 24-core machine (2026-10-17, D398). The checks
/// before a commit and a push skip them; the full check (CI on main, D37) and `cargo test` run
/// them. Each is
/// marked where it is written too. A test that slows past that joins them.
const SLOW_TESTS: &[&str] = &[
    // terra_build's drawing swept over every case: minutes each.
    "sections::tests::keys::a_part_whose_key_is_as_it_was_draws_as_it_was_and_parts_drawn_alone_draw_as_with_the_rest",
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

/// A test the check names when it runs longer, in seconds (D406): the full check waits on its
/// slowest test, so one over this sets how long CI's run on main takes.
const SLOW_TEST_SECS: u64 = 120;

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

/// Which gate is running: the one before a commit, the one before a push, or the full check
/// CI runs on main (D37).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tier {
    Commit,
    Push,
    Full,
}

/// Why a stage only the full check runs is not run here.
const ON_MAIN: &str = "the full check's, run by CI on main";

/// Whether a tier reads and writes the stamp (D413): the push's alone, since a stamp says its
/// tree passed that tier, and a full check narrowed by a fast push's stamp would leave out
/// what the push never ran.
fn stamps(tier: Tier, all: bool) -> bool {
    tier == Tier::Push && !all
}

/// The gate (D37, D398). Before a commit (`--quick`), on the staged files, only what is fast:
/// formatting, the code-health gates, clippy and the quick tests of the crates changed, the
/// tests of the tools changed, the docs check and the shared crates' rules. Before a push
/// (no flag), the same over everything that differs from origin/main, with the crates reading
/// a changed file. The full check (`--full`) adds every crate depending on them, the slow
/// tests, every tool's tests, and the kit, foliage, artifacts, fuzz and Blender checks when
/// what they read changed. `--all` runs a tier over everything, the full one unless `--quick`
/// says otherwise: what CI runs on every push to main. The stages run in order and the first
/// to fail ends it.
pub(crate) fn check(args: &[String]) -> bool {
    let mut tier = Tier::Push;
    let mut all = false;
    let mut pushing = false;
    for arg in args {
        match arg.as_str() {
            "--quick" => tier = Tier::Commit,
            "--full" => tier = Tier::Full,
            "--pushing" => pushing = true,
            "--all" => all = true,
            "--verbose" => stage::set_verbose(),
            _ => {
                eprintln!(
                    "usage: cargo xtask check [--quick | --full] [--all] [--pushing] [--verbose]"
                );
                return false;
            }
        }
    }
    if all && tier == Tier::Push {
        tier = Tier::Full;
    }
    let started = Instant::now();
    // The hook's check of a push to main holds the turn to land (D413), or stops before
    // checking what git would refuse.
    let _turn = if pushing && tier != Tier::Commit {
        match land::hold_for_push() {
            Ok(turn) => turn,
            Err(why) => {
                println!("✗ check stopped before checking: {why}");
                return false;
            }
        }
    } else {
        None
    };
    let scope = match (all, tier) {
        (true, _) => Scope::everything("--all"),
        (false, Tier::Commit) => scope::commit(),
        (false, Tier::Push | Tier::Full) => scope::push(),
    };
    let name = match tier {
        Tier::Commit => "quick check",
        Tier::Push => "check",
        Tier::Full => "full check",
    };
    println!("{name} over {}", scope.summary());
    let push = tier != Tier::Commit;
    let stamped = stamps(tier, all);
    // The tree read, to stamp if it passes, and what changed since one stamped (D413).
    let tree = if stamped { stamp::clean_tree() } else { None };
    let mut plan = Plan::of(tier, &scope);
    if stamped && let Some((passed, since)) = stamp::since() {
        if since.all() {
            println!(
                "  passed on {passed} before; running it all again: {}",
                since.summary()
            );
        } else {
            println!(
                "  passed on {passed} before; running only what changed since: {}",
                since.summary()
            );
            plan = plan.and(&Plan::since(&since));
        }
    }
    // A machine-wide slot while it builds and tests (D413); nothing heavy, no wait. A push
    // holding the turn to land goes first, since every landing waits on it.
    let _slot = if push && plan.heavy() && !pushing {
        slot::take()
    } else {
        None
    };
    let stages = stages(tier, &scope, &plan);
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
                 check runs it over everything since main; CI's full check on main adds the \
                 crates depending on these, {} slow tests, the kit, foliage, artifacts, fuzz \
                 and Blender checks (D37).",
                stage::duration(took),
                SLOW_TESTS.len()
            );
        }
        Tier::Full => println!("✓ full check passed in {}", stage::duration(took)),
        Tier::Push => {
            // Stamped only if what was read is still what it was when the check began.
            if let Some(tree) = tree
                && stamp::clean_tree().as_ref() == Some(&tree)
            {
                stamp::write(&tree);
            }
            println!(
                "✓ check passed in {}; CI runs the full check on main (D37)",
                stage::duration(took)
            );
        }
    }
    true
}

/// Why a stage that the push holds is not run: it passed on the stamped tree and nothing it
/// reads changed since (D413).
const UNCHANGED: &str = "passed before, unchanged since";

/// Whether a scoped stage runs, or why not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Run {
    Yes,
    No(&'static str),
}

impl Run {
    fn when(runs: bool, why_not: &'static str) -> Self {
        if runs { Run::Yes } else { Run::No(why_not) }
    }

    /// Runs only if it runs over both: the push holds it and what it reads changed since the
    /// stamp.
    fn and(self, since: Run) -> Self {
        match (self, since) {
            (Run::Yes, Run::Yes) => Run::Yes,
            (Run::Yes, Run::No(_)) => Run::No(UNCHANGED),
            (no, _) => no,
        }
    }
}

/// What a tier runs over a scope: the packages it lints and tests, and the scoped stages.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Plan {
    /// The packages linted and tested; `None` the whole workspace.
    packages: Option<BTreeSet<String>>,
    /// Why none is, when none is.
    idle: &'static str,
    /// The kit, artifacts and fuzz checks.
    builds: Run,
    foliage: Run,
    blender: Run,
    /// The shared crates' manifests checked free of Bevy.
    manifests: Run,
}

/// The packages depending on these, with these (`scope::with_dependents`); `None` when they
/// cannot be found.
type Dependents = fn(&BTreeSet<String>) -> Option<BTreeSet<String>>;

impl Plan {
    fn of(tier: Tier, scope: &Scope) -> Self {
        Plan::of_with(tier, scope, scope::with_dependents)
    }

    fn of_with(tier: Tier, scope: &Scope, dependents: Dependents) -> Self {
        let all = scope.all();
        let packages = packages(tier, scope, dependents);
        // Building, or what it stands on, changed, or the tasks that run its checks did (xtask
        // is among the packages tested whenever any Rust changed, so its own change is asked
        // of the scope).
        let builds = all
            || scope.changed.contains("xtask")
            || packages
                .as_ref()
                .is_none_or(|names| names.contains("terra_build"));
        Plan {
            packages,
            idle: "no crate changed",
            builds: Run::when(builds, "building unchanged"),
            foliage: Run::when(
                all || scope.assets || scope.tools.contains("textures"),
                "no assets or textures tool changed",
            ),
            blender: Run::when(
                all || scope.python || scope.tools.contains("blender"),
                "no Blender script changed",
            ),
            manifests: Run::when(all || scope.manifests, "no manifest changed"),
        }
    }

    /// What the full check would run over what changed since the stamp, as a change's scope,
    /// except that xtask among it is the tasks' data, not the code that runs building's
    /// checks: a change to that narrows nothing (`stamp::since`). It is the full check's, with
    /// the crates depending on what changed, though the push tests only the crates changed: a
    /// crate the push holds whose dependency the rebase changed is among it only as a
    /// dependent, and must be tested again.
    fn since(scope: &Scope) -> Self {
        Plan::since_with(scope, scope::with_dependents)
    }

    fn since_with(scope: &Scope, dependents: Dependents) -> Self {
        let mut plan = Plan::of_with(Tier::Full, scope, dependents);
        if plan.builds == Run::Yes
            && !scope.all()
            && plan
                .packages
                .as_ref()
                .is_some_and(|names| !names.contains("terra_build"))
        {
            plan.builds = Run::No("building unchanged");
        }
        plan
    }

    /// What runs over both this plan and one of what changed since the stamp: the packages
    /// in both, each stage only if both run it.
    fn and(self, since: &Plan) -> Self {
        let had = self.lints();
        let packages = match (self.packages, &since.packages) {
            (None, None) => None,
            (None, Some(names)) => Some(names.clone()),
            (Some(names), None) => Some(names),
            (Some(names), Some(changed)) => Some(names.intersection(changed).cloned().collect()),
        };
        let mut plan = Plan {
            packages,
            idle: self.idle,
            builds: self.builds.and(since.builds),
            foliage: self.foliage.and(since.foliage),
            blender: self.blender.and(since.blender),
            manifests: self.manifests.and(since.manifests),
        };
        if had && !plan.lints() {
            plan.idle = UNCHANGED;
        }
        plan
    }

    /// Whether any package is linted and tested.
    fn lints(&self) -> bool {
        self.packages.as_ref().is_none_or(|names| !names.is_empty())
    }

    /// Whether it builds or tests anything, so takes a slot on the machine.
    fn heavy(&self) -> bool {
        self.lints() || self.builds == Run::Yes
    }
}

/// The packages a tier lints and tests: before a commit those changed, before a push those
/// changed or reading a changed file, and in the full check those and every package depending
/// on them, with xtask, whose tests read every Rust source, whenever Rust changed. `None` is
/// the whole workspace.
fn packages(tier: Tier, scope: &Scope, dependents: Dependents) -> Option<BTreeSet<String>> {
    if scope.all() {
        return None;
    }
    let mut asked: BTreeSet<String> = scope.changed.union(&scope.readers).cloned().collect();
    match tier {
        Tier::Commit => return Some(scope.changed.clone()),
        Tier::Push => return Some(asked),
        Tier::Full => {}
    }
    // Cargo's tree knows the crates, not the tasks: xtask depends on none of them.
    let tasks = asked.remove("xtask");
    let mut found = dependents(&asked)?;
    if tasks || scope.rust || !found.is_empty() {
        found.insert("xtask".into());
    }
    Some(found)
}

/// The check's stages in the order they run, each with how to run it.
fn stages(tier: Tier, scope: &Scope, plan: &Plan) -> Vec<(Step, How)> {
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
        let mut cargo = niced("cargo");
        cargo.arg(command).args(ci).args(quiet).args(rest);
        cargo
    };
    let command = |program: &str, args: &[&str]| {
        let mut command = niced(program);
        command.args(args);
        command
    };
    let full = tier == Tier::Full;
    let selected = match &plan.packages {
        None => vec!["--workspace".to_string()],
        Some(names) => scope::package_args(names),
    };
    let shown = selected.join(" ");
    let mut clippy = selected.clone();
    clippy.extend(["--all-targets", "--", "-D", "warnings"].map(String::from));
    let mut tests = selected.clone();
    if !full {
        // Before a commit or a push: the crates' own tests, not their examples' builds or doc
        // tests, and not the slow ones.
        tests.extend(["--tests", "--", "--exact"].map(String::from));
        for name in SLOW_TESTS {
            tests.extend(["--skip".to_string(), name.to_string()]);
        }
    } else {
        tests.push("--".into());
    }
    // A line a test, to the log alone, so the slow ones are timed (`flag_tests_over`).
    tests.push("--format=pretty".into());
    let test_command = cargo("test", &tests);
    let lint_and_test = |name: &str, how: How| -> (Step, How) {
        let rerun = match name {
            "clippy" => format!("cargo clippy {shown} --all-targets -- -D warnings"),
            _ => format!("cargo test {shown}"),
        };
        let step = Step::new(name, Said::Cargo, LOGS)
            .rerun(rerun)
            .flag_tests_over(SLOW_TEST_SECS);
        if plan.lints() {
            (step, how)
        } else {
            (step, How::Skip(plan.idle))
        }
    };
    // A stage only the full check runs; how it runs is asked only when it does.
    let full_only = |step: Step, run: Run, how: &dyn Fn() -> (Step, How)| -> (Step, How) {
        match (full, run) {
            (false, _) => (step, How::Skip(ON_MAIN)),
            (true, Run::No(why)) => (step, How::Skip(why)),
            (true, Run::Yes) => how(),
        }
    };
    let mut stages = vec![
        fmt(scope),
        (
            Step::new("sizes", Said::Printed, LOGS).rerun("cargo xtask sizes"),
            How::Here(crate::sizes::sizes_hold),
        ),
        baselines(tier != Tier::Commit),
        (
            Step::new("one definition", Said::Printed, LOGS),
            How::Here(crate::primitives::one_definition),
        ),
        lint_and_test("clippy", How::Command(cargo("clippy", &clippy))),
        lint_and_test("tests", How::Command(test_command)),
        tools_tests(full || scope.all(), &scope.tools),
    ];
    let kits = || Step::new("kits", Said::Cargo, LOGS).rerun("cargo xtask kits");
    stages.push(full_only(kits(), plan.builds, &|| {
        (kits(), How::Command(command("cargo", KITS)))
    }));
    let foliage = || Step::new("foliage", Said::Printed, LOGS).rerun("cargo xtask foliage");
    stages.push(full_only(foliage(), plan.foliage, &|| {
        let node = command("node", &["tools/textures/foliage.ts"]);
        (foliage(), How::Command(node))
    }));
    for (name, rerun, args) in [
        (
            "artifacts",
            "cargo xtask artifacts --quick --ratchet",
            vec!["--quick".to_string(), "--ratchet".into()],
        ),
        (
            "fuzz",
            "cargo xtask fuzz --ratchet",
            fuzz_args(&["--ratchet".into()]),
        ),
    ] {
        let step = || Step::new(name, Said::Cargo, LOGS).rerun(rerun);
        stages.push(full_only(step(), plan.builds, &|| {
            let (program, args) = artifacts_command(&args);
            let mut run = niced(program);
            run.args(args);
            (step(), How::Command(run))
        }));
    }
    stages.push((
        Step::new("docs", Said::Printed, LOGS).rerun("node tools/docs/docs.ts"),
        How::Command(command("node", &["tools/docs/docs.ts"])),
    ));
    stages.push(full_only(blender_step(), plan.blender, &blender_scripts));
    stages.push((
        Step::new("shared crates free of Bevy", Said::Printed, LOGS),
        match plan.manifests {
            Run::Yes => How::Here(shared_crates_avoid_bevy),
            Run::No(why) => How::Skip(why),
        },
    ));
    stages.push((
        Step::new("shared crates repeatable", Said::Printed, LOGS),
        How::Here(shared_crates_are_repeatable),
    ));
    stages
}

/// A command run at a lower priority, as everything the check runs is (D406): its builds and
/// tests take every core, and renders, films and the developer's game started beside it stay
/// responsive. `nice` is on Linux and macOS; elsewhere it runs as it is.
pub(crate) fn niced(program: &str) -> Command {
    if cfg!(unix) {
        let mut command = Command::new("nice");
        command.args(["-n", "10", program]);
        command
    } else {
        Command::new(program)
    }
}

/// Formatting, of the crates changed: it goes file by file, so they are all a change can
/// unsettle.
/// The baselines (D414): against HEAD before a commit, the merge-base before a push.
fn baselines(push: bool) -> (Step, How) {
    let step = Step::new("baselines", Said::Printed, LOGS);
    if push {
        (step, How::Here(crate::baselines::hold_before_push))
    } else {
        (step, How::Here(crate::baselines::hold_before_commit))
    }
}

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
    let step = blender_step();
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

fn blender_step() -> Step {
    Step::new("Blender scripts", Said::Printed, LOGS)
        .rerun("tools/blender/run.sh tools/blender/check.py $(git ls-files 'tools/**/*.py')")
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

/// The gate's tiers and plans: what each runs over a change, and what a stamp narrows.
#[cfg(test)]
#[path = "check/tests.rs"]
mod gate_tests;
