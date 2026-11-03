use super::plan::{Frame, Gate, Lever, Plan, Row, Rules, Seen, TOTAL, Verdict, Why, rows, table};
use super::{Probe, apply, gpu, hide, start};
use bevy::prelude::*;

const RULES: Rules = Rules {
    wait: 10,
    idle: 5,
    timeout: 2000,
    drawn_share: 0.995,
};

#[test]
fn a_plan_reads_its_variants_and_levers() {
    let plan =
        Plan::parse("old: fog.light_span=0; new: fog.light_span=1, hide=WindExtension").unwrap();
    assert_eq!(plan.variants.len(), 2);
    assert_eq!(
        plan.variants[0].levers,
        [Lever::Tune("fog.light_span".into(), 0.0)]
    );
    assert_eq!(
        plan.variants[1].levers,
        [
            Lever::Tune("fog.light_span".into(), 1.0),
            Lever::Hide("WindExtension".into())
        ]
    );
    assert_eq!(plan.tunables(), ["fog.light_span"]);
    // A base with no levers is a variant too.
    assert!(
        Plan::parse("base:; lit: hide=PointLight").unwrap().variants[0]
            .levers
            .is_empty()
    );
    for bad in [
        "",
        "old fog=1",
        "a: x=1; a: x=2",
        "a: hide=",
        "a: fog.x=many",
    ] {
        let error = Plan::parse(bad).unwrap_err();
        assert!(error.contains("D609"), "`{bad}` said {error}");
    }
}

/// j19's levers, `tune.render.spread_aerial_view_lut=0`, failed every load as naming no
/// tunable; the refusal now says to drop the prefix and gives the lever to write.
#[test]
fn a_tune_prefixed_lever_is_refused_with_the_bare_name() {
    let error = Plan::parse("thread: tune.render.spread_aerial_view_lut=0").unwrap_err();
    assert!(error.contains("drop the prefix"), "said {error}");
    assert!(
        error.contains("`render.spread_aerial_view_lut=0`"),
        "said {error}"
    );
    assert!(error.contains("D609"), "said {error}");
    // The bare name parses.
    assert_eq!(
        Plan::parse("thread: render.spread_aerial_view_lut=0")
            .unwrap()
            .variants[0]
            .levers,
        [Lever::Tune("render.spread_aerial_view_lut".into(), 0.0)]
    );
}

#[test]
fn rounds_alternate_the_variants() {
    // A variant an argument, as `--probe` is repeated.
    let mut plan = Plan::parse("a:").unwrap();
    plan.add("b: hide=X").unwrap();
    assert!(plan.add("a: hide=Y").unwrap_err().contains("twice"));
    assert_eq!(plan.phases(2), [(0, 0), (1, 0), (0, 1), (1, 1)]);
}

/// j10's ground: after a shader swap its pipeline compiles for 500 frames, the ground
/// undrawn meanwhile; then it is drawn. Only the frames after count.
#[test]
fn the_gate_counts_nothing_until_the_pipelines_are_built_and_all_is_drawn() {
    let mut gate = Gate::default();
    let most = 1.0e6;
    let mut counted = 0;
    for frame in 1..=1000u32 {
        let compiling = usize::from(frame <= 500);
        let vertices = if frame <= 500 { 0.6e6 } else { 1.0e6 };
        let seen = Seen {
            compiling,
            vertices: Some(vertices),
        };
        match gate.frame(&RULES, seen, most) {
            Verdict::Count => {
                assert!(frame >= 500 + RULES.idle, "frame {frame} counted too soon");
                counted += 1;
            }
            Verdict::Wait(Why::Switched) => assert!(frame <= RULES.wait),
            Verdict::Wait(Why::Compiling(_)) => assert!(frame < 500 + RULES.idle),
            other => panic!("frame {frame}: {other:?}"),
        }
    }
    // Frames 505 to 1000: the pipelines idle five frames in a row from 501.
    assert_eq!(counted, 1000 - 500 - RULES.idle + 1);
}

#[test]
fn the_gate_holds_a_frame_that_draws_less_than_its_variant_has() {
    let mut gate = Gate::default();
    let seen = |vertices| Seen {
        compiling: 0,
        vertices: Some(vertices),
    };
    for _ in 0..RULES.wait + RULES.idle {
        gate.frame(&RULES, seen(1.0e6), 1.0e6);
    }
    assert_eq!(gate.frame(&RULES, seen(1.0e6), 1.0e6), Verdict::Count);
    assert!(matches!(
        gate.frame(&RULES, seen(0.9e6), 1.0e6),
        Verdict::Wait(Why::Undrawn { .. })
    ));
    // Without pipeline statistics only the pipelines are judged.
    let blind = Seen {
        compiling: 0,
        vertices: None,
    };
    assert_eq!(gate.frame(&RULES, blind, 1.0e6), Verdict::Count);
}

#[test]
fn a_phase_that_never_settles_times_out() {
    let mut gate = Gate::default();
    let compiling = Seen {
        compiling: 3,
        vertices: None,
    };
    let verdicts: Vec<Verdict> = (0..=RULES.timeout)
        .map(|_| gate.frame(&RULES, compiling, 0.0))
        .collect();
    assert_eq!(verdicts.last(), Some(&Verdict::TimedOut(Why::Compiling(3))));
    assert!(
        !verdicts[..verdicts.len() - 1]
            .iter()
            .any(|v| matches!(v, Verdict::TimedOut(_)))
    );
}

fn frame(total: f64, fog: f64, vertices: f64) -> Frame {
    let mut f = Frame::default();
    f.passes.insert(TOTAL.into(), total);
    f.passes.insert("volumetric_fog".into(), fog);
    f.vertices = Some(vertices);
    f
}

#[test]
fn the_table_says_a_saving_past_the_noise_and_drops_undrawn_frames() {
    let rounds = |total: f64, fog: f64| -> Vec<Vec<Frame>> {
        vec![
            (0..9)
                .map(|i| frame(total + 0.01 * f64::from(i % 3), fog, 1.0e6))
                .collect(),
            (0..9)
                .map(|i| frame(total + 0.02 * f64::from(i % 3), fog, 1.0e6))
                .collect(),
        ]
    };
    let mut old = rounds(7.6, 0.5);
    // A frame counted before the round's ground was drawn: cheap, and dropped by the table
    // because the variant drew more on its other frames.
    old[0].push(frame(3.0, 0.5, 0.5e6));
    let new = rounds(7.4, 0.3);
    let rows = rows(&[old, new], 0.995, 4);
    assert_eq!(rows[0].pass, TOTAL);
    assert!((rows[0].medians[0] - 7.61).abs() < 0.011, "{:?}", rows[0]);
    let lines = table(&["old", "new"], &rows);
    assert!(lines[1].contains("new saves 0.200"), "{}", lines[1]);
    // A difference within the A/A spread is noise.
    let row = Row {
        pass: "x".into(),
        medians: vec![1.0, 0.98],
        noise: vec![0.05, 0.01],
    };
    assert!(table(&["a", "b"], &[row])[1].contains("b within noise"));
}

/// A pass outside the costliest whose difference is past the noise floor is listed after them
/// (D614's atmosphere_luts, 0.18 -> 0.15 ms, fell outside the 14 costliest and went unseen);
/// one outside them within the noise is not.
#[test]
fn the_table_lists_a_cheap_pass_that_changed_past_the_noise() {
    let variant = |luts: f64| -> Vec<Vec<Frame>> {
        (0..3)
            .map(|round| {
                let wobble = 0.001 * f64::from(round);
                (0..5)
                    .map(|_| {
                        let mut f = Frame::default();
                        for (pass, ms) in [
                            (TOTAL, 7.0),
                            ("main_opaque", 3.0),
                            ("shadows", 2.0),
                            ("bloom", 0.5),
                            ("atmosphere_luts", luts),
                            ("tonemapping", 0.05),
                        ] {
                            f.passes.insert(pass.into(), ms + wobble);
                        }
                        f
                    })
                    .collect()
            })
            .collect()
    };
    let rows = rows(&[variant(0.18), variant(0.15)], 0.995, 2);
    let passes: Vec<&str> = rows.iter().map(|r| r.pass.as_str()).collect();
    assert_eq!(passes, [TOTAL, "main_opaque", "shadows", "atmosphere_luts"]);
    let lines = table(&["every", "kept"], &rows);
    assert!(lines[4].contains("kept saves 0.030"), "{}", lines[4]);
}

#[test]
fn a_phase_run_clocked_down_is_warned_of() {
    let clocks = |mhz: f32, pstate: &str| {
        gpu::summarise(&[gpu::Reading {
            mhz,
            max_mhz: 2100.0,
            pstate: pstate.into(),
            busy: 98.0,
            celsius: 64.0,
        }])
    };
    let warned = gpu::clocked_down(&[
        ("old #1".into(), clocks(1905.0, "P0")),
        ("new #1".into(), clocks(1890.0, "P0")),
        ("old #2".into(), clocks(1350.0, "P0")),
        ("new #2".into(), clocks(1900.0, "P2")),
        ("unknown".into(), None),
    ]);
    assert_eq!(warned.len(), 2, "{warned:?}");
    assert!(warned[0].starts_with("old #2 ran clocked down, 1350 MHz P0 against old #1's 1905"));
    assert!(warned[1].starts_with("new #2 ran clocked down"));
    let steady = [
        ("a".into(), clocks(1900.0, "P0")),
        ("b".into(), clocks(1800.0, "P0")),
    ];
    assert!(gpu::clocked_down(&steady).is_empty());
    let read = gpu::parse("1905, 2100, P0, 98, 64").unwrap();
    assert_eq!(
        (read.mhz, read.max_mhz, read.pstate.as_str()),
        (1905.0, 2100.0, "P0")
    );
    assert_eq!(gpu::parse("[N/A], 2100, P0, 98, 64"), None);
}

/// A world with one standard-material mesh, in view or not.
fn scene(in_view: bool) -> (World, Entity) {
    let mut world = World::new();
    let mesh = world
        .spawn((
            Mesh3d(Handle::default()),
            MeshMaterial3d::<StandardMaterial>(Handle::default()),
            Visibility::Inherited,
            if in_view {
                ViewVisibility::VISIBLE
            } else {
                ViewVisibility::HIDDEN
            },
        ))
        .id();
    (world, mesh)
}

fn probe(text: &str) -> Probe {
    let mut probe = Probe::new(Plan::parse(text).unwrap());
    probe.phases = probe.plan.phases(1);
    probe.at = 1;
    probe
}

/// play-lake-fps j12 hid `WindMaterial`, which no mesh is drawn with (the flora's is
/// `WindExtension`), and timed the scene unchanged as the scene without it.
#[test]
fn a_hide_matching_nothing_fails_naming_what_is_drawn() {
    let (mut world, _) = scene(true);
    let mut wrong = probe("base:; nowind: hide=WindMaterial");
    let error = apply(&mut world, &mut wrong).unwrap_err();
    assert!(
        error.contains("hide=WindMaterial matched no mesh or light"),
        "{error}"
    );
    assert!(
        error.contains("MeshMaterial3d<StandardMaterial> 1/1"),
        "{error}"
    );

    // Matching only meshes out of view measures nothing either.
    let (mut world, _) = scene(false);
    let error = apply(&mut world, &mut probe("base:; plain: hide=Standard")).unwrap_err();
    assert!(error.contains("matched 1 meshes, none in view"), "{error}");

    // One that matches is hidden while its phase lasts.
    let (mut world, mesh) = scene(true);
    let mut right = probe("base:; plain: hide=Standard");
    apply(&mut world, &mut right).unwrap();
    hide(&mut world, &mut right);
    assert_eq!(world.get::<Visibility>(mesh), Some(&Visibility::Hidden));

    // A light is never culled, so its view visibility stays unset: shown is in view (the
    // lake's first probe failed `hide=PointLight` as matching 24, none in view).
    let light = world
        .spawn((
            PointLight::default(),
            Visibility::Inherited,
            InheritedVisibility::VISIBLE,
            ViewVisibility::HIDDEN,
        ))
        .id();
    let mut lights = probe("base:; unlit: hide=PointLight");
    apply(&mut world, &mut lights).unwrap();
    hide(&mut world, &mut lights);
    assert_eq!(world.get::<Visibility>(light), Some(&Visibility::Hidden));
}

#[test]
fn a_lever_naming_no_tunable_fails() {
    let mut probe = probe("a: probe.no_such_lever=1");
    let error = start(&mut probe).unwrap_err();
    assert!(
        error.contains("no tunable is called probe.no_such_lever"),
        "{error}"
    );
    let mut probe = Probe::new(Plan::parse("a: probe.rounds=99").unwrap());
    assert!(start(&mut probe).unwrap_err().contains("out of its range"));
}
