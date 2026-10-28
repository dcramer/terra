use super::*;

#[test]
fn our_units_are_told_from_dependencies() {
    let ours = vec!["terra_core".to_string(), "xtask".to_string()];
    assert!(is_ours("terra_core-29bd634e92f54282", &ours));
    assert!(is_ours("libterra_core-29bd634e92f54282.rlib", &ours));
    assert!(is_ours("terra_core-29bd634e92f54282.d", &ours));
    assert!(is_ours("xtask-0123456789abcdef", &ours));
    assert!(!is_ours("libbevy_ecs-c6abf7f208cd6fe1.rlib", &ours));
    assert!(!is_ours("wayland-client-1a2b3c4d5e6f7a8b", &ours));
    assert!(!is_ours("libterra_core_extra-29bd634e92f54282.rlib", &ours));
}

#[test]
fn only_outputs_replaced_whole_are_linked() {
    assert!(linkable(Path::new(
        "deps/libbevy_ecs-c6abf7f208cd6fe1.rlib"
    )));
    assert!(linkable(Path::new(
        "deps/libbevy_ecs-c6abf7f208cd6fe1.rmeta"
    )));
    assert!(linkable(Path::new(
        "deps/libserde_derive-5dfc8b2696cad4ae.so"
    )));
    assert!(!linkable(Path::new("deps/bevy_ecs-c6abf7f208cd6fe1.d")));
    assert!(!linkable(Path::new(
        ".fingerprint/bevy_ecs-c6abf7f208cd6fe1/lib-bevy_ecs"
    )));
    assert!(!linkable(Path::new("build/blake3-729e203be1ad83f9/output")));
}

#[test]
fn the_seed_is_keyed_by_what_decides_how_dependencies_build() {
    let listing = "100644 blob aaa\tCargo.toml\n\
        100644 blob bbb\tCargo.lock\n\
        100644 blob ccc\tcrates/terra_core/Cargo.toml\n\
        100644 blob ddd\tcrates/terra_core/src/lib.rs\n\
        100644 blob eee\tspike/old/Cargo.toml\n\
        100644 blob fff\ttools/sandbox/Cargo.toml\n\
        100644 blob ggg\t.cargo/config.toml\n\
        100644 blob hhh\trust-toolchain.toml\n\
        100644 blob iii\txtask/Cargo.toml\n\
        100644 blob jjj\tdocs/Cargo.lock.md\n";
    assert_eq!(
        manifest_lines(listing),
        "aaa Cargo.toml\nbbb Cargo.lock\nccc crates/terra_core/Cargo.toml\n\
         ggg .cargo/config.toml\nhhh rust-toolchain.toml\niii xtask/Cargo.toml"
    );
}

/// The scorecards run `cargo test -p <crate> --lib` for each crate their criteria name;
/// each must be a member whose own test build the seed holds, or a fresh worktree's first
/// run builds that crate's dependencies again.
#[test]
fn the_seed_builds_every_crate_the_scorecards_test() {
    let builds = seed_builds(&workspace_packages(&root()).unwrap());
    let mut named = 0;
    for criteria in [
        "tools/combat/criteria.json",
        "tools/controller/criteria.json",
    ] {
        let text = std::fs::read_to_string(root().join(criteria)).unwrap();
        for part in text.split("\"crate\"").skip(1) {
            let krate = part.split('"').nth(1).unwrap();
            let wanted = ["test", "-p", krate, "--no-run"].map(String::from);
            assert!(
                builds.iter().any(|b| *b == wanted),
                "{criteria} tests {krate}, which the seed does not build"
            );
            named += 1;
        }
    }
    assert!(named > 0, "no crate found in the scorecards' criteria");
    // The workspace's own builds come first, so each member's reuses what they made.
    assert_eq!(
        builds[0],
        BUILDS[0].iter().map(|s| s.to_string()).collect::<Vec<_>>()
    );
}

#[test]
fn finds_this_workspace_packages() {
    let names = workspace_packages(&root()).unwrap();
    assert!(names.iter().any(|n| n == "xtask"));
    assert!(names.iter().any(|n| n == "terra_game"));
}
