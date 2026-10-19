use super::*;

fn plan(packages: Option<&[&str]>, builds: bool) -> Plan {
    Plan {
        packages: packages.map(|names| names.iter().map(|n| n.to_string()).collect()),
        idle: "no crate changed",
        builds: Run::when(builds, "building unchanged"),
        foliage: Run::No("no assets or textures tool changed"),
        blender: Run::Yes,
        manifests: Run::Yes,
    }
}

/// After a rebase a stage runs again only if the push holds it and what it reads changed
/// since the stamp (D413); a package outside the push is not the push's to test.
#[test]
fn a_stamped_push_runs_only_what_it_holds_and_the_rebase_changed() {
    // The push changed terra_core, which building stands on; the rebase brought a change to
    // building alone.
    let push = plan(
        Some(&["terra_core", "terra_build", "terra_game", "xtask"]),
        true,
    );
    let since = plan(Some(&["terra_build", "terra_game", "xtask"]), true);
    let run = push.clone().and(&since);
    assert_eq!(
        run.packages,
        plan(Some(&["terra_build", "terra_game", "xtask"]), true).packages
    );
    assert_eq!(run.builds, Run::Yes);
    // A rebase that brought only docs: nothing heavy is run again, and it says why.
    let docs = Plan {
        blender: Run::No("no Blender script changed"),
        ..plan(Some(&[]), false)
    };
    let run = push.clone().and(&docs);
    assert!(!run.heavy());
    assert_eq!(
        (run.idle, run.builds, run.blender),
        (UNCHANGED, Run::No(UNCHANGED), Run::No(UNCHANGED))
    );
    // What the push does not hold stays out with its own reason, whatever the rebase did.
    assert_eq!(run.foliage, Run::No("no assets or textures tool changed"));
    // The whole workspace on either side is the other side's packages; on both, all of it.
    assert_eq!(plan(None, true).and(&since).packages, since.packages);
    assert_eq!(push.clone().and(&plan(None, true)).packages, push.packages);
    assert_eq!(plan(None, true).and(&plan(None, true)).packages, None);
}

/// A dependency graph to test with: building stands on terra_core.
fn dependents(names: &BTreeSet<String>) -> Option<BTreeSet<String>> {
    let mut found = names.clone();
    if names.contains("terra_core") {
        found.insert("terra_build".into());
    }
    Some(found)
}

fn changed(names: &[&str]) -> Scope {
    Scope {
        files: names.len(),
        changed: names.iter().map(|n| n.to_string()).collect(),
        rust: true,
        ..Scope::default()
    }
}

/// The push tests the crates changed and none of the slow tests, and leaves the stages
/// that need the dependents or building's drawing to the full check on main (D37), even
/// where the full check would run them.
#[test]
fn the_push_tests_only_the_crates_changed_and_leaves_the_rest_to_main() {
    let scope = changed(&["terra_core"]);
    let push = Plan::of_with(Tier::Push, &scope, dependents);
    assert_eq!(
        push.packages,
        Some(BTreeSet::from(["terra_core".to_string()]))
    );
    let full = Plan::of_with(Tier::Full, &scope, dependents);
    let names = full.packages.clone().unwrap_or_default();
    assert!(names.contains("terra_build") && names.contains("xtask"));
    assert_eq!(full.builds, Run::Yes);
    let mut seen = 0;
    for (step, how) in stages(Tier::Push, &scope, &full) {
        match (step.name(), how) {
            ("kits" | "foliage" | "artifacts" | "fuzz" | "Blender scripts", how) => {
                assert!(
                    matches!(how, How::Skip(why) if why == ON_MAIN),
                    "{} runs before a push",
                    step.name()
                );
                seen += 1;
            }
            ("tests", How::Command(tests)) => {
                let args: Vec<String> = tests
                    .get_args()
                    .map(|a| a.to_string_lossy().into_owned())
                    .collect();
                for slow in SLOW_TESTS {
                    assert!(args.iter().any(|a| a == slow), "{slow} runs before a push");
                }
                seen += 1;
            }
            _ => {}
        }
    }
    assert_eq!(seen, 6);
}

/// After a rebase that changed terra_core, building, which the push holds, is tested again
/// though terra_core is not among the push's crates: what changed since the stamp keeps the
/// crates depending on it (D413).
#[test]
fn a_rebase_changing_a_dependency_tests_again_the_crate_the_push_holds() {
    let push = Plan::of_with(Tier::Push, &changed(&["terra_build"]), dependents);
    let since = Plan::since_with(&changed(&["terra_core"]), dependents);
    let run = push.and(&since);
    assert!(
        run.packages
            .is_some_and(|names| names.contains("terra_build"))
    );
}

/// A stamp says its tree passed the push's check: the full check neither narrows to one
/// nor writes one, so a fast push never stands in for it.
#[test]
fn only_the_push_reads_and_writes_the_stamp() {
    assert!(stamps(Tier::Push, false));
    assert!(!stamps(Tier::Full, false));
    assert!(!stamps(Tier::Push, true));
    assert!(!stamps(Tier::Commit, false));
}

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
