use super::*;

pub(crate) fn timing(median_ms: f32, p99_ms: f32, worst_ms: f32) -> Timing {
    Timing {
        gpu: "Apple M4 Max".into(),
        size: "1920x1080 offscreen".into(),
        median_ms,
        p99_ms,
        worst_ms,
        budget_ms: 13.3,
        over_pct: 0.0,
        hitches: 0,
        drift_pct: 0.0,
        freezes: 0,
        from_start: false,
        load_s: None,
        device: "DiscreteGpu".into(),
        preset: "high".into(),
    }
}

#[test]
fn reads_how_a_run_held_to_the_budget() {
    let line = "SANDBOX against the 13.3 ms budget: 0.50% of frames over, 2 hitches over \
                    26.7 ms, the game's work in the last third -3.0% on the first";
    assert_eq!(parse_sustained(line), Some((13.3, 0.5, 2, 0, -3.0)));
    let line = "SANDBOX against the 13.3 ms budget: 0.50% of frames over, 2 hitches over \
                    26.7 ms, 1 freezes of 100 ms or more, the game's work in the last third \
                    -3.0% on the first";
    assert_eq!(parse_sustained(line), Some((13.3, 0.5, 2, 1, -3.0)));
    assert_eq!(parse_sustained("SANDBOX saved shot.png"), None);
}

#[test]
fn a_scene_holds_to_the_budget_unless_it_misses_a_gate() {
    assert!(timing(5.0, 9.0, 20.0).misses().is_empty());
    assert_eq!(timing(5.0, 14.0, 20.0).misses().len(), 1);
    // Judged against the budget the run said, not one of its own.
    let at_60 = Timing {
        budget_ms: 16.7,
        ..timing(5.0, 14.0, 20.0)
    };
    assert!(at_60.misses().is_empty());
    // A run that never said how it held is not taken to have held.
    let unsaid = Timing {
        budget_ms: 0.0,
        ..timing(5.0, 9.0, 20.0)
    };
    assert_eq!(unsaid.misses().len(), 1);
    let stutters = Timing {
        hitches: 1,
        drift_pct: 20.0,
        over_pct: 2.0,
        ..timing(5.0, 9.0, 40.0)
    };
    assert_eq!(stutters.misses().len(), 3);
}

#[test]
fn reads_the_sandboxs_frame_times() {
    let line = "SANDBOX frames on Apple M4 Max at 1920x1080 offscreen: median 7.51 ms, \
                    99th percentile 15.39 ms, worst 16.40 ms";
    let read = Timing {
        budget_ms: 0.0,
        device: String::new(),
        preset: String::new(),
        ..timing(7.51, 15.39, 16.40)
    };
    assert_eq!(parse_timing(line), Some(read));
    assert_eq!(parse_timing("SANDBOX saved shot.png"), None);
}

#[test]
fn runs_combine_as_the_middle_one_and_the_worst_frame() {
    let runs = [
        timing(9.0, 12.0, 30.0),
        timing(7.0, 10.0, 11.0),
        timing(8.0, 11.0, 12.0),
    ];
    assert_eq!(combine(&runs), Some(timing(8.0, 11.0, 30.0)));
    assert_eq!(combine(&[]), None);
}

#[test]
fn compares_with_the_last_result_on_the_same_kind_of_run() {
    let csv = format!(
        "{BENCHMARKS_HEADER}\n\
             2026-10-12,aaa,Apple M4 Max,1920x1080 offscreen,release,forest,6.00,7,8,3\n\
             2026-10-12,bbb,Apple M4 Max,1920x1080 offscreen,debug,forest,14.00,15,16,3\n\
             2026-10-13,ccc,Apple M4 Max,1920x1080 offscreen,release,forest,5.00,6,7,3\n\
             2026-10-13,ddd,Radeon,1920x1080 offscreen,release,forest,20.00,21,22,3\n"
    );
    let now = timing(5.5, 6.0, 7.0);
    assert_eq!(
        last_recorded(&csv, &now, "release", "forest"),
        Some((5.0, "ccc".to_string()))
    );
    assert_eq!(last_recorded(&csv, &now, "release", "yard"), None);
}

#[test]
fn the_integrated_gpu_is_held_to_its_median_not_the_budget() {
    let line = "SANDBOX drawn at the low preset on a IntegratedGpu GPU";
    assert_eq!(
        parse_drawn(line),
        Some(("low".to_string(), "IntegratedGpu".to_string()))
    );
    let integrated = |median_ms| Timing {
        device: "IntegratedGpu".into(),
        preset: "low".into(),
        hitches: 4,
        ..timing(median_ms, 45.0, 90.0)
    };
    // Over the 13.3 ms budget at every gate, but under 33.3 ms at the median: it holds.
    assert!(integrated(30.0).misses().is_empty());
    assert_eq!(integrated(34.0).misses().len(), 1);
    assert_eq!(integrated(30.0).judged_by(), "D239");
    assert_eq!(timing(5.0, 9.0, 20.0).judged_by(), "D25");
}

#[test]
fn busy_runs_are_left_out_when_enough_are_quiet() {
    let quiet = Load {
        cpu_pct: 10.0,
        gpu_pct: Some(1.0),
        load_pct: None,
    };
    let busy = Load {
        cpu_pct: 90.0,
        gpu_pct: Some(1.0),
        load_pct: None,
    };
    let run = |median, before| Run {
        timing: timing(median, 9.0, 12.0),
        before: Some(before),
        during: None,
    };
    let (used, was_busy) = chosen_runs(&[run(5.0, quiet), run(9.0, busy), run(5.2, quiet)]);
    assert_eq!(used.len(), 2);
    assert!(!was_busy);
    // Mostly busy: every run is used, and said to be busy.
    let (used, was_busy) = chosen_runs(&[run(5.0, quiet), run(9.0, busy), run(8.0, busy)]);
    assert_eq!(used.len(), 3);
    assert!(was_busy);
    // A run quiet before it but busy while it ran is busy.
    let during = Run {
        during: Some(During {
            mean_cpu_pct: 80.0,
            most_cpu_pct: 95.0,
        }),
        ..run(5.0, quiet)
    };
    assert!(during.busy());
}

#[test]
fn slower_is_past_the_runs_own_spread() {
    let runs = [
        timing(5.0, 9.0, 9.0),
        timing(6.0, 9.0, 9.0),
        timing(5.5, 9.0, 9.0),
    ];
    let (low, high, pct) = spread(&runs);
    assert_eq!((low, high), (5.0, 6.0));
    assert!((pct - 100.0 / 5.5).abs() < 0.01);
    // 12% slower than recorded: past 10%, but within an 18% spread, so not SLOWER.
    assert!(!slower(0.12, pct));
    assert!(slower(0.20, pct));
    assert!(slower(0.12, 4.0));
}

fn asked_from(args: &[&str]) -> Option<Asked> {
    asked(&args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>())
}

#[test]
fn tunables_are_passed_as_given_and_keyed_in_name_order() {
    let a = asked_from(&["lake", "--tune", "sky.b=0.250", "--tune", "sky.a=2"]).unwrap();
    // Sorted, and a number written the one way, so the same run is always the same key.
    assert_eq!(a.tunes, ["sky.a=2", "sky.b=0.25"]);
    assert_eq!(a.chosen.len(), 1);
    let b = asked_from(&["--tune", "sky.a=2.0", "lake", "--tune", "sky.b=.25"]).unwrap();
    assert_eq!(b.tunes, a.tunes);
    assert!(asked_from(&["lake"]).unwrap().tunes.is_empty());
}

#[test]
fn a_tune_that_is_not_an_assignment_or_set_twice_is_refused() {
    for bad in [
        &["--tune"][..],
        &["--tune", "sky.a"],
        &["--tune", "=1"],
        &["--tune", "sky.a="],
        &["--tune", "sky@a=1"],
        &["--tune", "sky.a=1,2"],
        &["--tune", "sky.a=1", "--tune", "sky.a=2"],
    ] {
        assert!(asked_from(bad).is_none(), "{bad:?} was taken");
    }
}

#[test]
fn a_tuned_run_has_a_key_of_its_own() {
    assert_eq!(result_key("lake", None, &[]), "lake");
    let tunes = ["sky.cloud.heaped_cover=0.25".to_string()];
    assert_eq!(
        result_key("lake", None, &tunes),
        "lake@tune:sky.cloud.heaped_cover=0.25"
    );
    assert_eq!(
        result_key("lake", Some("low"), &tunes),
        "lake@low@tune:sky.cloud.heaped_cover=0.25"
    );
    // Never compared with the scene's plain baseline.
    let csv = format!(
        "{BENCHMARKS_HEADER}\n\
             2026-10-12,aaa,Apple M4 Max,1920x1080 offscreen,release,lake,6.00,7,8,3\n"
    );
    let key = result_key("lake", None, &tunes);
    assert_eq!(
        last_recorded(&csv, &timing(5.0, 6.0, 7.0), "release", &key),
        None
    );
}

#[test]
fn a_tuned_key_is_recorded_only_when_new() {
    let tuned = "lake@tune:sky.cloud.heaped_cover=0.25";
    let csv = format!(
        "{BENCHMARKS_HEADER}\n\
             2026-10-12,aaa,Apple M4 Max,1920x1080 offscreen,release,lake,6.00,7,8,3\n\
             2026-10-13,bbb,Radeon,1920x1080 offscreen,release,{tuned},5.00,6,7,3\n"
    );
    let keys = [
        tuned.to_string(),
        "lake@tune:sky.cloud.heaped_cover=0.3".to_string(),
    ];
    assert_eq!(recorded_already(&csv, &keys), [tuned]);
    assert!(recorded_already(&format!("{BENCHMARKS_HEADER}\n"), &keys).is_empty());
}
