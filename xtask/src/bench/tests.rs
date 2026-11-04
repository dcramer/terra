use super::*;
use crate::load::{Clock, During, Load};

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
        hitch_at: None,
        // A scene held to every scene's play budgets and no load of its own; no settle phase,
        // which these runs do not say (`phases`' tests hold it).
        budget: budgets::Scene {
            settle: budgets::Limits::default(),
            ..budgets::of("world").unwrap()
        },
        device: "DiscreteGpu".into(),
        preset: "high".into(),
        ticks: ticks::Said::default(),
        phases: phases::Said::default(),
        longest: hitches::Longest::default(),
    }
}

/// A scene whose ticks run past their time misses its target, however its frames held.
#[test]
fn a_scene_misses_with_ticks_slower_than_their_time() {
    let mut slow = timing(5.0, 9.0, 20.0);
    assert!(slow.ticks.read(
        "SANDBOX ticks, in ms a tick: median 12.00, 99th percentile 38.00, worst 40.00, of 600 frames' ticks, against the 15.6 ms of a tick"
    ));
    assert_eq!(slow.misses().len(), 1);
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
    // Judged against the budgets' file (D607), whatever budget the run said.
    let at_60 = Timing {
        budget_ms: 16.7,
        ..timing(5.0, 14.0, 20.0)
    };
    assert_eq!(at_60.misses().len(), 1);
    assert!(
        at_60.misses()[0].contains("budgets.ron"),
        "{:?}",
        at_60.misses()
    );
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
    // A scene that hitched too often names the system its runs' warnings found longest, or
    // the command that will; never a log the bench did not keep.
    let hitched = |timing: &Timing| {
        timing
            .misses()
            .into_iter()
            .find(|miss| miss.starts_with("1 hitches, past"))
            .unwrap()
    };
    assert!(hitched(&stutters).contains("cargo xtask profile walk"));
    let mut longest = hitches::Longest::default();
    longest.read("WARN terra_world::debug: 9 frames took over 27 ms since the last report, the worst 55.0 ms; the longest in 6 of 9: forest::stream.");
    let named = Timing {
        longest,
        ..stutters
    };
    let said = hitched(&named);
    assert!(
        said.ends_with("the longest system was forest::stream in 6 of the 9 hitch frames the recorder explained"),
        "{said}"
    );
}

#[test]
fn reads_the_sandboxs_frame_times() {
    let line = "SANDBOX frames on Apple M4 Max at 1920x1080 offscreen: median 7.51 ms, \
                    99th percentile 15.39 ms, worst 16.40 ms";
    let read = Timing {
        budget_ms: 0.0,
        device: String::new(),
        preset: String::new(),
        budget: budgets::Scene::default(),
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
fn a_result_taken_on_a_busy_machine_is_no_baseline() {
    let csv = format!(
        "{BENCHMARKS_HEADER}\n\
             2026-11-03,aaa,RTX 4090,1920x1080 offscreen,release,forest,6.00,7,8,3,0,0,0,,0,0\n\
             2026-11-03,bbb,RTX 4090,1920x1080 offscreen,release,forest,9.00,12,40,3,0,20,0,,0,1\n"
    );
    let mut now = timing(6.0, 7.0, 8.0);
    now.gpu = "RTX 4090".to_string();
    assert_eq!(
        last_recorded(&csv, &now, "release", "forest"),
        Some((6.0, "aaa".to_string()))
    );
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
            clock: None,
        }),
        ..run(5.0, quiet)
    };
    assert!(during.busy());
}

/// A run whose every hitch ended in a collapse of the machine's clock (from 0.1 s before its
/// fall to 1 s after) is the machine's (D185), so it is busy and another stands in; one hitch
/// outside them is the game's, and so is a hitch the run did not say the time of.
#[test]
fn a_hitch_in_a_clock_collapse_is_the_machines() {
    // The run started at 1000 s since the epoch; its clock fell 25 s and 40 s in.
    let start = 1000.0;
    let run = |hitch_at: Option<&[f64]>| Run {
        timing: Timing {
            hitches: hitch_at.map_or(2, |at| at.len() as u32),
            hitch_at: hitch_at.map(<[f64]>::to_vec),
            ..timing(5.0, 9.0, 40.0)
        },
        before: None,
        during: Some(During {
            mean_cpu_pct: 5.0,
            most_cpu_pct: 10.0,
            clock: Some(Clock {
                median_mhz: 5300.0,
                lowest_mhz: 900.0,
                at_s: vec![25.0, 40.0],
                start_epoch_s: start,
            }),
        }),
    };
    let inside = [start + 25.05, start + 25.6, start + 39.95];
    assert_eq!(run(Some(&inside)).hitches_split(), Some((3, 0)));
    assert!(run(Some(&inside)).busy());
    let said = run(Some(&inside)).clock_said().unwrap();
    assert!(said.contains("fell to 900 MHz 2 times") && said.contains("another stands in"));
    // One a second and a half after a fall, or before it: the game's, so the run is not busy.
    let late = [start + 25.05, start + 26.5];
    assert_eq!(run(Some(&late)).hitches_split(), Some((1, 1)));
    assert!(!run(Some(&late)).busy());
    assert!(!run(Some(&[start + 24.8])).busy());
    assert!(
        run(Some(&[start + 24.8]))
            .clock_said()
            .unwrap()
            .contains("the game's")
    );
    // No hitches, or hitches with no times said: not the machine's.
    assert!(!run(Some(&[])).busy());
    assert!(!run(None).busy());
    assert!(run(None).clock_said().unwrap().contains("did not say"));
    let (used, was_busy) = chosen_runs(&[run(Some(&[])), run(Some(&inside)), run(Some(&late))]);
    assert_eq!((used.len(), was_busy), (2, false));
    // The sandbox's line.
    let line =
        "SANDBOX hitches ended at, seconds since the Unix epoch: 1762171234.51 1762171240.02";
    assert_eq!(
        runs::parse_hitch_times(line),
        Some(vec![1762171234.51, 1762171240.02])
    );
    let none = "SANDBOX hitches ended at, seconds since the Unix epoch: none";
    assert_eq!(runs::parse_hitch_times(none), Some(vec![]));
    assert_eq!(runs::parse_hitch_times("SANDBOX frames on"), None);
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
fn a_picture_size_is_passed_on_and_a_bad_one_refused() {
    let a = asked_from(&["lake", "--picture", "1901x1550"]).unwrap();
    assert_eq!(a.picture.as_deref(), Some("1901x1550"));
    assert!(asked_from(&["lake"]).unwrap().picture.is_none());
    for bad in ["1901", "1901x", "x1550", "1901x50", "big", "1901 x 1550"] {
        assert!(asked_from(&["--picture", bad]).is_none(), "{bad} was taken");
    }
    assert!(asked_from(&["--picture"]).is_none());
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

/// A settled scene's load goes in its row's `load_s` and its table's line, read against the
/// last one recorded, so a slower load has a baseline as a slower median does.
#[test]
fn a_settled_scenes_load_is_recorded_and_said_against_the_last() {
    let mut town = timing(9.7, 12.2, 16.8);
    town.load_s = Some(76.8);
    let key = ["2026-11-03", "abc", "release", "town-dusk"];
    let row = csv_row(key, &town, 3, false);
    let cells: Vec<&str> = row.split(',').collect();
    let header: Vec<&str> = BENCHMARKS_HEADER.split(',').collect();
    assert_eq!(cells.len(), header.len());
    let load = header
        .iter()
        .position(|h| *h == "load_s")
        .expect("a load column");
    assert_eq!(cells[load], "76.8");
    let csv = format!("{BENCHMARKS_HEADER}\n{row}\n");
    town.load_s = Some(40.0);
    assert_eq!(
        play::load_said(&csv, &town, "release", "town-dusk", SLOWER_BY),
        "loaded in 40.0 s, -48% on abc"
    );
    town.load_s = None;
    let unsaid = csv_row(key, &town, 3, false);
    assert_eq!(unsaid.split(',').nth(load), Some(""));
}

/// A result taken on a busy machine says so in its row's last column, `busy`, which the perf
/// row reads to record it without judging it (D610); a quiet one says 0.
#[test]
fn a_busy_result_is_marked_in_its_row() {
    let forest = timing(8.9, 12.0, 57.1);
    let header: Vec<&str> = BENCHMARKS_HEADER.split(',').collect();
    assert_eq!(header.last(), Some(&"busy"));
    let key = ["2026-11-03", "abc", "release", "forest-run"];
    let busy = csv_row(key, &forest, 20, true);
    let quiet = csv_row(key, &forest, 10, false);
    for (row, want) in [(&busy, "1"), (&quiet, "0")] {
        let cells: Vec<&str> = row.split(',').collect();
        assert_eq!(cells.len(), header.len());
        assert_eq!(cells.last(), Some(&want));
    }
}
