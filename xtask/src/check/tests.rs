use super::*;

fn changed(names: &[&str]) -> Scope {
    Scope {
        files: names.len(),
        changed: names.iter().map(|n| n.to_string()).collect(),
        rust: true,
        ..Scope::default()
    }
}

fn names(stages: &[(Step, How)]) -> Vec<String> {
    stages
        .iter()
        .filter(|(_, how)| !matches!(how, How::Skip(_)))
        .map(|(step, _)| step.name().to_string())
        .collect()
}

/// The hook formats and lints the crates changed and runs nothing else (D37): no test, no
/// ratchet, no other crate's build.
#[test]
fn the_hook_formats_and_lints_only_the_crates_changed() {
    let stages = hook(&changed(&["terra_core"]));
    assert_eq!(names(&stages), ["fmt", "clippy"]);
    for (step, how) in &stages {
        let How::Command(command) = how else {
            panic!("{} does not run a command", step.name());
        };
        let args: Vec<String> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.iter().any(|a| a == "terra_core"), "{args:?}");
        assert!(!args.iter().any(|a| a == "--workspace"), "{args:?}");
    }
    // Docs alone: nothing to format or lint.
    let docs = Scope {
        files: 1,
        ..Scope::default()
    };
    assert!(names(&hook(&docs)).is_empty());
}

/// The full check runs every stage, the tests and the ratchets among them.
#[test]
fn the_full_check_runs_the_tests_and_the_ratchets() {
    let stages = full();
    let ran = names(&stages);
    for stage in ["fmt", "clippy", "tests", "tools tests", "kits", "foliage"] {
        assert!(ran.iter().any(|name| name == stage), "{stage}: {ran:?}");
    }
    // Blender's skips where there is none.
    let all: Vec<String> = stages
        .iter()
        .map(|(step, _)| step.name().to_string())
        .collect();
    for stage in ["artifacts", "fuzz", "docs", "Blender scripts"] {
        assert!(all.iter().any(|name| name == stage), "{stage}: {all:?}");
    }
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
