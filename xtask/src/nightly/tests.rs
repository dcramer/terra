use super::*;

#[test]
fn a_probe_skips_what_does_not_build_and_stops_when_it_cannot_set_up() {
    assert_eq!(probe_exit(true, true, true), 0);
    assert_eq!(probe_exit(true, true, false), 1);
    assert_eq!(probe_exit(true, false, false), 125);
    assert_eq!(probe_exit(false, false, false), 128);
    assert_eq!(probe_exit(false, true, true), 128);
}

#[test]
fn every_stage_holds_in_its_probe_and_lowers_only_in_the_run() {
    for stage in STAGES {
        assert!(!stage.hold.contains(&"--lower"), "{}", stage.name);
        assert!(STAGES.iter().filter(|s| s.name == stage.name).count() == 1);
    }
    let lowering = STAGES.iter().filter(|s| s.run.contains(&"--lower")).count();
    assert_eq!(
        lowering, 6,
        "artifacts quick and full, fuzz, sizes, combat and controller lower"
    );
}

#[test]
fn the_last_run_is_read_from_its_reports_first_line() {
    let sha = "0123456789abcdef0123456789abcdef01234567";
    let passed = format!("passed at {sha} (2026-10-18 04:00 UTC): nothing to lower\n");
    assert_eq!(last_run(&passed), Some((true, sha)));
    let failed = format!("failed at {sha} (when): fuzz\n  fuzz first fails at …\n");
    assert_eq!(last_run(&failed), Some((false, sha)));
    assert_eq!(last_run(""), None);
    assert_eq!(last_run("nightly at noon"), None);
    assert_eq!(last_run("passed at main"), None);
}

#[test]
fn the_first_bad_commit_is_read_from_what_bisect_said() {
    let said = "running probe\nnightly --probe fuzz: 1\n\
                abcdef1234567890 is the first bad commit\ncommit abcdef1234567890\n";
    assert_eq!(
        first_bad(said),
        Some(Found::Commit("abcdef1234567890".into()))
    );
    let skipped = "There are only 'skip'ped commits left to test.\n\
                   The first bad commit could be any of:\n\
                   1111111aaaa\n2222222bbbb\nWe cannot bisect more!\n";
    assert_eq!(
        first_bad(skipped),
        Some(Found::OneOf(vec![
            "1111111aaaa".into(),
            "2222222bbbb".into()
        ]))
    );
    assert_eq!(first_bad("bisect run failed: exit code 128"), None);
}

#[test]
fn a_lowering_holds_on_a_moved_main_only_where_nothing_it_measured_changed() {
    let fuzz = format!("{ARTIFACTS}/fuzz/seed-2.tsv");
    let sheet = format!("{ARTIFACTS}/all/sheet.tsv");
    let since: BTreeSet<String> = [sheet.clone()].into_iter().collect();
    // Building unchanged: a file kept, unless the move changed that file itself.
    assert_eq!(holds(&fuzz, &since, false), Holds::Keep);
    assert_eq!(holds(&sheet, &since, false), Holds::Drop);
    // Building changed: every artifacts and fuzz count is measured again next run.
    assert_eq!(holds(&fuzz, &BTreeSet::new(), true), Holds::Drop);
    // Sizes entry by entry whatever moved; the indexes rendered again.
    assert_eq!(holds(SIZES, &since, true), Holds::Merge);
    assert_eq!(holds("docs/decisions.md", &since, true), Holds::Render);
    // The combat baseline: kept whatever building did, scored again when a move or the
    // baseline itself changed.
    assert_eq!(holds(COMBAT, &since, true), Holds::Keep);
    let moved: BTreeSet<String> = ["assets/data/moves/sword.shielded.ron".to_string()].into();
    assert_eq!(holds(COMBAT, &moved, false), Holds::Rescore);
    let raised: BTreeSet<String> = [COMBAT.to_string()].into();
    assert_eq!(holds(COMBAT, &raised, false), Holds::Rescore);
    // The controller's likewise, by what its course and film read: a move changes combat's
    // alone, a clip both.
    assert_eq!(holds(CONTROLLER, &moved, false), Holds::Keep);
    let clip: BTreeSet<String> = ["assets/models/character/base.glb".to_string()].into();
    assert_eq!(holds(CONTROLLER, &clip, false), Holds::Rescore);
    let walked: BTreeSet<String> = ["tools/controller/criteria.json".to_string()].into();
    assert_eq!(holds(CONTROLLER, &walked, false), Holds::Rescore);
    assert_eq!(holds(COMBAT, &walked, false), Holds::Keep);
}

/// A git command in a test's own repository, which must pass.
fn run_git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(dir).output();
    let out = out.expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A clone of `origin` that commits as a test.
fn clone(origin: &Path, to: &Path) {
    let (from, path) = (origin.to_string_lossy(), to.to_string_lossy());
    run_git(origin.parent().unwrap(), &["clone", "-q", &from, &path]);
    run_git(to, &["config", "user.name", "nightly test"]);
    run_git(to, &["config", "user.email", "nightly@test"]);
}

/// Writes `files` and commits them; the commit.
fn commit(dir: &Path, files: &[(&str, &str)]) -> String {
    for (path, text) in files {
        let file = dir.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }
    run_git(dir, &["add", "-A"]);
    run_git(dir, &["commit", "-q", "-m", "change"]);
    run_git(dir, &["rev-parse", "HEAD"])
}

#[test]
fn a_lowering_raced_by_main_is_scored_again_where_main_changed_what_the_scorecard_reads() {
    let dir = std::env::temp_dir().join(format!("terra-nightly-race-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (origin, src, other, logs) = (
        dir.join("origin.git"),
        dir.join("src"),
        dir.join("other"),
        dir.join("logs"),
    );
    std::fs::create_dir_all(&logs).unwrap();
    run_git(&dir, &["init", "-q", "--bare", "-b", "main", "origin.git"]);
    clone(&origin, &src);
    let camera = "crates/terra_walker/src/camera.rs";
    let sky = "crates/terra_world/src/sky.rs";
    let row = |v: &str| format!("id\tvalue\twhy\nfeet\t{v}\tseeded\n");
    let tip = commit(
        &src,
        &[
            (CONTROLLER, &row("0.5")),
            (COMBAT, &row("0.5")),
            (camera, "// the camera\n"),
            (sky, "// the sky\n"),
        ],
    );
    run_git(&src, &["push", "-q", "origin", "HEAD:main"]);
    // While the run measures at `tip`, main moves: the walker's camera, which the
    // controller's film reads and combat's scorecard does not.
    clone(&origin, &other);
    commit(&other, &[(camera, "// the camera, moved\n")]);
    run_git(&other, &["push", "-q", "origin", "HEAD:main"]);
    // The run lowered both baselines at `tip`.
    std::fs::write(src.join(CONTROLLER), row("0.3")).unwrap();
    std::fs::write(src.join(COMBAT), row("0.2")).unwrap();
    let mut asked = Vec::new();
    let said = land(&src, &logs, &tip, &mut |path| {
        asked.push(path.to_string());
        std::fs::write(src.join(path), row("0.4")).is_ok()
    });
    assert!(said.starts_with("lowered and pushed"), "{said}");
    assert!(said.contains("scored again"), "{said}");
    // Combat's carried as measured, the controller's as scored again on the new main,
    // the new main's camera kept.
    assert_eq!(asked, [CONTROLLER]);
    let at = |path: &str| run_git(&other, &["show", &format!("origin/main:{path}")]);
    run_git(&other, &["fetch", "-q"]);
    assert_eq!(at(COMBAT), row("0.2").trim());
    assert_eq!(at(CONTROLLER), row("0.4").trim());
    assert_eq!(at(camera), "// the camera, moved");

    // Again, the scorecard failing on the new main: its baseline is main's, the other carried.
    let tip = run_git(&src, &["rev-parse", "HEAD"]);
    run_git(&other, &["pull", "-q", "--rebase", "origin", "main"]);
    commit(&other, &[(camera, "// the camera, moved again\n")]);
    run_git(&other, &["push", "-q", "origin", "HEAD:main"]);
    std::fs::write(src.join(CONTROLLER), row("0.1")).unwrap();
    std::fs::write(src.join(COMBAT), row("0.1")).unwrap();
    let said = land(&src, &logs, &tip, &mut |path| {
        let _ = std::fs::write(src.join(path), row("0.0"));
        false
    });
    assert!(said.contains("its scorecard fails"), "{said}");
    run_git(&other, &["fetch", "-q"]);
    assert_eq!(at(COMBAT), row("0.1").trim());
    assert_eq!(at(CONTROLLER), row("0.4").trim());

    // A move outside both scorecards' reads carries both as measured, nothing scored again.
    let tip = run_git(&src, &["rev-parse", "HEAD"]);
    run_git(&other, &["pull", "-q", "--rebase", "origin", "main"]);
    commit(&other, &[(sky, "// the sky, moved\n")]);
    run_git(&other, &["push", "-q", "origin", "HEAD:main"]);
    std::fs::write(src.join(CONTROLLER), row("0.05")).unwrap();
    let said = land(&src, &logs, &tip, &mut |_| panic!("nothing to score again"));
    assert!(said.starts_with("lowered and pushed"), "{said}");
    run_git(&other, &["fetch", "-q"]);
    assert_eq!(at(CONTROLLER), row("0.05").trim());
    assert_eq!(at(sky), "// the sky, moved");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failure_reports_its_first_stage_first_and_the_hook_reads_one_block() {
    let outcomes = [
        Outcome {
            name: "clippy",
            ok: true,
            lines: vec![],
        },
        Outcome {
            name: "fuzz",
            ok: false,
            lines: vec!["seed 7: z-fighting 3, was 0".into()],
        },
        Outcome {
            name: "docs",
            ok: false,
            lines: vec!["docs: 1 faults".into()],
        },
    ];
    let sha = "0123456789abcdef";
    let text = failure(sha, "now", "first fails at 0123456789 x", &outcomes);
    let block = text.split("\n\n").next().unwrap();
    assert!(block.starts_with(&format!("failed at {sha} (now): fuzz, docs\n")));
    assert!(block.contains("  fuzz first fails at 0123456789 x\n    seed 7"));
    assert!(!block.contains("docs: 1 faults"));
    assert_eq!(last_run(&text), Some((false, sha)));
}
