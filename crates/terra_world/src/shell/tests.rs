use super::*;

#[test]
fn a_run_holds_to_the_budget_by_its_share_over_its_hitches_and_its_drift() {
    // Three thirds: steady at 8 ms, then three frames over, a hitch and a freeze, then 10.
    let mut frames = vec![8.0; 30];
    frames.extend([8.0, 20.0, 40.0, 8.0, 8.0, 100.0, 8.0, 8.0, 8.0, 8.0]);
    frames.extend(vec![10.0; 30]);
    let sustained = Sustained::of(&frames, None);
    assert_eq!((sustained.hitches, sustained.freezes), (2, 1));
    assert!((sustained.over_pct - 300.0 / 70.0).abs() < 1e-3);
    assert!((sustained.drift_pct - 25.0).abs() < 1e-3);
    // With the game's work measured, the drift is its.
    let game = vec![2.0; 70];
    assert_eq!(Sustained::of(&frames, Some(&game)).drift_pct, 0.0);
    assert_eq!(Sustained::of(&[], None).hitches, 0);
}

#[test]
fn a_route_is_its_spots_in_order_and_a_long_bench_its_frames() {
    let options = parse(&[
        "--route",
        "-58,-39",
        "--route",
        "10,-80",
        "--bench-frames",
        "3600",
    ])
    .unwrap();
    assert_eq!(options.route, [(-58.0, -39.0), (10.0, -80.0)]);
    assert_eq!(options.bench_frames, 3600);
    assert!(parse(&["--bench-frames", "5"]).is_err());
}

fn parse(args: &[&str]) -> Result<Options, String> {
    Options::from_args(args.iter().map(|a| a.to_string()))
}

#[test]
fn reads_the_workbench_options() {
    let options = parse(&[
        "--model",
        "a.glb",
        "--model",
        "b.glb",
        "--attach",
        "pack.glb@socket.back",
        "--sides",
        "30, 120",
        "--moments",
        "0.25,0.75",
        "--remote",
        "--background",
    ])
    .unwrap();
    assert_eq!(options.models, ["a.glb", "b.glb"]);
    assert_eq!(options.attaches, ["pack.glb@socket.back"]);
    assert_eq!(options.sides, [30.0, 120.0]);
    assert_eq!(options.moments, [0.25, 0.75]);
    assert!(options.remote);
    assert!(options.unattended() && !options.scripted() && !options.sound_on());
    assert!(parse(&["--sides", "front"]).is_err());
}

#[test]
fn reads_the_options() {
    let options = parse(&[
        "--experiment",
        "scatter",
        "--radius",
        "500",
        "--shot",
        "out/a.png",
        "--press",
        "F@60",
    ])
    .unwrap();
    assert_eq!(options.experiments, ["scatter"]);
    assert_eq!(options.radius_m, 500.0);
    assert_eq!(options.shot, Some(PathBuf::from("out/a.png")));
    let files = parse(&["--bone-audit", "a.jsonl", "--fight-log", "b.jsonl"]).unwrap();
    assert_eq!(files.bone_audit, Some(PathBuf::from("a.jsonl")));
    assert_eq!(files.fight_log, Some(PathBuf::from("b.jsonl")));
    assert!(
        parse(&["--bone-audit"])
            .unwrap_err()
            .contains("needs a value")
    );
    assert_eq!(options.presses, [(KeyCode::KeyF, 60, 61)]);
    let digit = parse(&["--press", "3@10"]).unwrap();
    assert_eq!(digit.presses, [(KeyCode::Digit3, 10, 11)]);
    let held = parse(&["--press", "w@40-90"]).unwrap();
    assert_eq!(held.presses, [(KeyCode::KeyW, 40, 90)]);
    let function = parse(&["--press", "F3@10"]).unwrap();
    assert_eq!(function.presses, [(KeyCode::F3, 10, 11)]);
    let escape = parse(&["--press", "esc@10"]).unwrap();
    assert_eq!(escape.presses, [(KeyCode::Escape, 10, 11)]);
    assert!(parse(&["--press", "F13@10"]).is_err());
    assert!(options.scripted());
    assert!(!parse(&[]).unwrap().scripted());
    let sequence = parse(&["--shots", "out/walk@300-302,600,301"]).unwrap();
    let shots = sequence.shots.unwrap();
    assert_eq!(shots.frames, [300, 301, 302, 600]);
    let every_other = parse(&["--shots", "out/film@10-16/2"])
        .unwrap()
        .shots
        .unwrap();
    assert_eq!(every_other.frames, [10, 12, 14, 16]);
    assert_eq!(shots.path(301), PathBuf::from("out/walk-00301.png"));
    // A shot's water mask: the lakes masked from the frame after it, pictured on the
    // last masked frame, and drawn as water again on the next; never on the shot itself.
    let masked = parse(&["--shots", "out/look@100,200", "--water-mask"]).unwrap();
    let looks = masked.shots.as_ref().unwrap();
    assert!(masked.water_mask);
    let matted = parse(&["--shots", "out/fire@330", "--flame-matte"]).unwrap();
    assert!(matted.flame_matte);
    assert_eq!(
        matted.shots.as_ref().unwrap().matte_path(330),
        PathBuf::from("out/fire-00330.matte.png")
    );
    assert_eq!(
        looks.water_path(100),
        PathBuf::from("out/look-00100.water.png")
    );
    assert_eq!(looks.water_masking(100), (false, None));
    assert_eq!(looks.water_masking(101), (true, None));
    assert_eq!(
        looks.water_masking(100 + WATER_MASK_FRAMES),
        (true, Some(100))
    );
    assert_eq!(looks.water_masking(101 + WATER_MASK_FRAMES), (false, None));
    assert_eq!(
        looks.water_masking(200 + WATER_MASK_FRAMES),
        (true, Some(200))
    );
    assert!(sequence.shot.is_none() && parse(&["--shots", "out/walk@300-302"]).unwrap().scripted());
    assert!(
        parse(&["--shots", "out/walk"])
            .unwrap_err()
            .contains("--shots")
    );
    assert!(
        parse(&["--shots", "out/walk@x"])
            .unwrap_err()
            .contains("--shots")
    );
    let fixed = parse(&["--vantage", "4,3,20,6,2,0"]).unwrap();
    assert_eq!(
        fixed.vantage,
        Some((Vec3::new(4.0, 3.0, 20.0), Vec3::new(6.0, 2.0, 0.0)))
    );
    assert!(parse(&["--vantage", "1,2"]).unwrap_err().contains("six"));
    let moving = parse(&["--vantage", "1,2,3,4,5,6", "--vantage", "7,8,9,1,2,3@300"]).unwrap();
    assert_eq!(
        moving.vantage,
        Some((Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, 5.0, 6.0)))
    );
    assert_eq!(moving.vantages.len(), 2);
    assert_eq!(moving.vantages[1].0, 300);
    // Gliding, halfway between them halfway through, held at either end.
    let at = |frame| glided(&moving.vantages, frame).unwrap().0;
    assert_eq!(at(150), Vec3::new(4.0, 5.0, 6.0));
    assert_eq!(at(400), Vec3::new(7.0, 8.0, 9.0));
    assert!(parse(&["--glide", "--town", "300"]).unwrap().glide);
    assert!(
        parse(&["--vantage", "1,2,3,4,5,6@x"])
            .unwrap_err()
            .contains("six")
    );
    let placed = parse(&["--at", "-40,30", "--face", "-45, 31"]).unwrap();
    assert_eq!(placed.start_at, Some((-40.0, 30.0)));
    assert_eq!(placed.start_facing, Some((-45.0, 31.0)));
    assert!(
        parse(&["--face", "north"])
            .unwrap_err()
            .contains("east and south")
    );
    let warped = parse(&["--warp", "66,-62:66,-64.4@900"]).unwrap();
    assert_eq!(
        warped.warps,
        [Warp {
            frame: 900,
            at: (66.0, -62.0),
            facing: (66.0, -64.4)
        }]
    );
    assert!(
        parse(&["--warp", "66,-62@900"])
            .unwrap_err()
            .contains("like")
    );
}

#[test]
fn explains_bad_options() {
    assert!(parse(&["--nope"]).unwrap_err().contains("Unknown option"));
    assert!(parse(&["--radius"]).unwrap_err().contains("needs a value"));
    assert!(parse(&["--radius", "big"]).unwrap_err().contains("number"));
    assert!(
        parse(&["--press", "ff@3"])
            .unwrap_err()
            .contains("like f@60")
    );
    assert!(parse(&["--press", "f"]).unwrap_err().contains("like f@60"));
    // A key let go before it is pressed, or on the same frame.
    assert!(parse(&["--press", "w@90-40"]).is_err());
    assert!(parse(&["--press", "w@40-40"]).is_err());
}

#[test]
fn reads_the_play_options_and_refuses_what_is_out_of_range() {
    let play = parse(&[
        "--time", "22", "--give", "stone:10", "--give", "spear:1", "--level", "yard", "--walls",
        "plain",
    ])
    .unwrap();
    assert_eq!(play.time, Some(22.0));
    assert_eq!(
        play.gives,
        [("stone".to_string(), 10), ("spear".to_string(), 1)]
    );
    assert!(play.yard());
    assert_eq!(play.walls.as_deref(), Some("plain"));
    assert!(!parse(&["--level", "world"]).unwrap().yard());
    // The hour runs from 0 to 24, both ends included.
    assert_eq!(parse(&["--time", "0"]).unwrap().time, Some(0.0));
    assert_eq!(parse(&["--time", "24"]).unwrap().time, Some(24.0));
    for hour in ["24.5", "-1", "night"] {
        assert!(parse(&["--time", hour]).unwrap_err().contains("0 to 24"));
    }
    for give in ["stone", "stone:lots", "stone:-1"] {
        assert!(parse(&["--give", give]).unwrap_err().contains("stone:10"));
    }
    assert!(
        parse(&["--level", "moon"])
            .unwrap_err()
            .contains("world or yard")
    );
    assert!(
        parse(&["--walls", "stone"])
            .unwrap_err()
            .contains("logs or plain")
    );
}

#[test]
fn a_date_is_a_day_of_the_clock_or_of_a_calendar() {
    let year = terra_climate::Year {
        days: 365.0,
        tilt_deg: 23.44,
        start: 0.6768,
    };
    assert_eq!(parse(&["--date", "40"]).unwrap().date, Some(Date::Day(40)));
    assert_eq!(Date::Day(-3).day(&year), -3);
    // Midwinter's day starts the year, its middle half a day on.
    assert_eq!(
        parse(&["--date", "12-21"]).unwrap().date,
        Some(Date::OfYear(0.5 / 365.0))
    );
    let date = |text: &str| parse(&["--date", text]).unwrap().date.unwrap();
    // Counted another way: the clock starts on the 25th of August, the 247th day after
    // midwinter (10 in December, 31 in January and so on), 0.6768 of the way round.
    assert_eq!(date("8-25").day(&year), 0);
    assert_eq!(date("10-15").day(&year), 51);
    assert_eq!(date("1-15").day(&year), 143);
    // And a date a year on from the start comes before the year turns again.
    for text in ["8-24", "3-1", "12-21", "12-20"] {
        let day = date(text).day(&year);
        assert!((0..365).contains(&day), "{text}: {day}");
        let Date::OfYear(share) = date(text) else {
            unreachable!()
        };
        let gap = (year.phase(day) as f64 - share).rem_euclid(1.0);
        assert!(gap.min(1.0 - gap) < 1.5 / 365.0, "{text}: {day}");
    }
    for wrong in ["13-1", "2-30", "oct", "10-", "0-5"] {
        assert!(parse(&["--date", wrong]).unwrap_err().contains("10-15"));
    }
}
