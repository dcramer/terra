use super::*;

/// A stance's hold takes both arms over any clip; a carry pose takes only its own arm, and
/// only in a carried clip (an idle, a jog), never in a blow; with nothing carried in a pose
/// the clip has the arms.
#[test]
fn a_carry_pose_takes_its_own_arm_in_a_carried_clip_alone() {
    let torch: Carry = [None, Some(AnimationNodeIndex::new(7))];
    let both: Carry = [Some(AnimationNodeIndex::new(6)), torch[1]];
    assert_eq!(laid_arms("Jog_Fwd_Loop", false, &torch), Arms::Right);
    assert_eq!(laid_arms("Idle_Loop", false, &both), Arms::Both);
    assert_eq!(laid_arms("Fight_Slash", false, &torch), Arms::None);
    assert_eq!(laid_arms("Walk_Loop", true, &torch), Arms::Both);
    assert_eq!(laid_arms("Idle_Loop", false, &[None, None]), Arms::None);
}

/// A switch between the jog and the sprint (either way) goes on at the same step, on the
/// figure's own clips: the foot just planted in the one giving way is just planted in the
/// one taking over, as far on to the next plant. Restarted at 0 (as every switch was), the
/// sprint came in with the other foot down and the planted one slid (sprint 29.6 cm).
#[test]
fn a_switch_between_gaits_keeps_the_planted_foots_phase() {
    let json = crate::model_files::json(CLIPS);
    let markers: HashMap<String, Markers> =
        serde_json::from_value(crate::model_files::extra(&json, "markers").unwrap()).unwrap();
    let paces: HashMap<String, f32> =
        serde_json::from_value(crate::model_files::extra(&json, "paces").unwrap()).unwrap();
    // Every pair of gaits, the walk too (D585's one stride clock): the clock is each
    // clip's plants (`Markers::synced`), so one foot is down in both.
    for (from, to) in [
        ("Jog_Fwd_Loop", "Sprint_Loop"),
        ("Sprint_Loop", "Jog_Fwd_Loop"),
        ("Walk_Loop", "Jog_Fwd_Loop"),
        ("Jog_Fwd_Loop", "Walk_Loop"),
        ("Walk_Loop", "Sprint_Loop"),
        ("Sprint_Loop", "Walk_Loop"),
    ] {
        let (was, now) = (&markers[from], &markers[to]);
        // Each plant of the one giving way, just after it comes down (not a foot still
        // down over the loop's seam, a span at 0 beside one that wraps round its end).
        let downs = [&was.foot_down_l, &was.foot_down_r];
        for (side, spans) in downs.into_iter().enumerate() {
            let wraps = spans.iter().any(|span| span[1] < span[0]);
            for span in spans.iter().filter(|span| !(wraps && span[0] <= 1e-4)) {
                let t = span[0] + 0.02;
                let at = synced(&markers, &paces, from, t, to).unwrap();
                assert!(
                    now.lock(side, at) >= terra_motion::feet::FULL,
                    "{from} {t:.3} -> {to} {at:.3}: foot {side} is not planted"
                );
                let (a, b) = (was.phase_in_loop(t), now.phase_in_loop(at));
                let ((sa, pa), (sb, pb)) = (a.unwrap(), b.unwrap());
                assert!(sa == sb && (pa - pb).abs() < 1e-3, "{a:?} -> {b:?}");
            }
        }
    }
    // Into or out of a clip that is no stride loop, a clip starts at its start.
    assert_eq!(
        synced(&markers, &paces, "Idle_Loop", 0.3, "Jog_Fwd_Loop"),
        None
    );
}

/// A body its mover says jogs but that is held still is shown standing; one that goes as
/// fast strides at its speed, a step's rise adds nothing, and a body put far away in a
/// frame strides no faster than it moves.
#[test]
fn the_figure_strides_as_fast_as_the_shown_body_goes_and_no_faster() {
    let (dt, up, jog) = (1.0 / 60.0, Vec3::Y, Vec3::X * crate::movement::JOG_SPEED);
    let at = Vec3::new(3.0, 2000.0, 1.0);
    let shown = |to: Vec3| shown_velocity(jog, Some(at), to, up, dt);
    let held = shown(at);
    assert_eq!(
        doing(held.length(), 0.0, &Stance::default(), false, None),
        Doing::Idle
    );
    let going = shown(at + jog * dt);
    assert!((going - jog).length() < 1e-3, "{going}");
    assert_eq!(
        doing(going.length(), 0.0, &Stance::default(), false, None),
        Doing::Jog
    );
    let stepping = shown(at + jog * dt + up * 0.3);
    assert!((stepping - jog).length() < 1e-3, "{stepping}");
    let put = shown(at + Vec3::Z * 50.0);
    assert!((put - jog).length() < 1e-3, "{put}");
    let half = shown(at + jog * dt * 0.5);
    assert!((half - jog * 0.5).length() < 1e-3, "{half}");
    // The first frame, and a paused one, take the mover's word.
    assert_eq!(shown_velocity(jog, None, at, up, dt), jog);
    assert_eq!(shown_velocity(jog, Some(at), at, up, 0.0), jog);
}

#[test]
fn a_mixed_clip_takes_its_share_of_its_mains_weight_and_stops_with_it() {
    let mut graph = AnimationGraph::new();
    let main = graph.add_clip(Handle::default(), 1.0, graph.root);
    let held = graph.add_clip(Handle::default(), 1.0, graph.root);
    let mut app = App::new();
    app.add_systems(Update, mix);
    let mut player = AnimationPlayer::default();
    // The transitions have given the main 0.8 (a clip giving way has the rest).
    player.start(main).set_weight(0.8);
    player.start(held);
    let figure = app
        .world_mut()
        .spawn((
            player,
            Animated {
                player: Entity::PLACEHOLDER,
                moves: Arc::default(),
                hands: Vec::new(),
                doing: Doing::Acting,
                playing: None,
                applied: 0,
                holds: Vec::new(),
                carry: [None; 2],
                swung: [0.0; 2],
                swings: Vec::new(),
                mixed: vec![Mixed {
                    main,
                    mix: held,
                    share: 0.25,
                }],
                shown: None,
            },
        ))
        .id();
    app.update();
    let weight = |app: &App, node| {
        let player = app.world().get::<AnimationPlayer>(figure).unwrap();
        player.animation(node).map(|a| a.weight())
    };
    assert!((weight(&app, main).unwrap() - 0.6).abs() < 1e-6);
    assert!((weight(&app, held).unwrap() - 0.2).abs() < 1e-6);
    // The main stopped (its fade done): the mixed clip stops and is forgotten.
    app.world_mut()
        .get_mut::<AnimationPlayer>(figure)
        .unwrap()
        .stop(main);
    app.update();
    assert_eq!(weight(&app, held), None);
    assert!(
        app.world()
            .get::<Animated>(figure)
            .unwrap()
            .mixed
            .is_empty()
    );
}

/// A stride's pace is metres a second in its model file's own world, through its armature's
/// scale (`tools/characters/pace.py`), and the figure's animation player sits on that
/// armature's node, whose world scale carries it again: a figure at its model's size going
/// at its jog's or sprint's own pace plays it at its own speed, unwarped, and one twice the
/// size at twice the speed the same. By the player's world scale, base.glb's 1.078 armature
/// played them at 0.93.
#[test]
fn a_figure_going_at_its_strides_pace_plays_it_at_its_own_speed_whatever_its_rigs_scale() {
    use crate::model_files::{json, pace, root_scale};
    let rig = root_scale(&json(crate::appearance::BASE));
    assert!(rig > 1.05, "the case this covers: an armature scaled {rig}");
    let clips = json(CLIPS);
    for clip in [Doing::Jog.clip(), Doing::Sprint.clip()] {
        let pace = pace(&clips, clip);
        for size in [1.0, 2.0] {
            // The figure turned to face -Z and sized, its model's armature under it.
            let own = Transform::from_scale(Vec3::splat(rig));
            let figure = Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI))
                .with_scale(Vec3::splat(size));
            let place = GlobalTransform::from(figure) * own;
            let shown = stride(size * pace, pace, &place, &own);
            assert!(
                (shown.rate - 1.0).abs() < 0.02 && (shown.warp - 1.0).abs() < 0.02,
                "{clip} at {} m/s, {size} times its model's size: {shown:?}",
                size * pace
            );
        }
    }
}
