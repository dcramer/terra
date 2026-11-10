//! A torch carried in hand: its two lights keep their light entities as the hand moves.

use super::*;

fn torch(base: f32) -> Vec<Lit> {
    let key = Lit {
        at: Vec3::new(0.0, base, 0.0),
        lumens: 85_000.0,
        color: Color::WHITE,
        range: torch::HELD_RANGE_M,
        radius: 0.45,
        glow: 1.0,
        kept: true,
        night_stops: 3.0,
        dusk_gain: 1.0,
        dusk_reach: 1.0,
        casts: true,
        still: None,
    };
    // The tip hangs 0.21 m over the key on one line, unshadowed.
    let tip = Lit {
        at: Vec3::new(0.0, base + 0.21, 0.0),
        lumens: 15_000.0,
        range: 3.5,
        casts: false,
        ..key.clone()
    };
    vec![key, tip]
}

fn was_of(shown: &[Option<Shown>]) -> Vec<Was> {
    shown
        .iter()
        .map(|s| Was {
            at: s.as_ref().map_or(Vec3::ZERO, |s| s.lit.at),
            on: s.is_some(),
            near: s.as_ref().is_some_and(|s| s.shadows || s.scatters),
            cast: s.as_ref().is_some_and(|s| s.shadows),
        })
        .collect()
}

/// Running, the hand lifts the torch more in a frame than half the 0.21 m between its key and
/// its tip, so the key came nearer the tip's last place than its own: matched by place alone
/// the two traded light entities, and the key sat unshadowed on the tip's while its shadows
/// faded back in over half a second (torch-light j2's log: a twin on 125 of a run's 1,540
/// frames).
#[test]
fn a_carried_torchs_key_keeps_its_light_and_shadows_as_the_hand_moves() {
    let eye = Some(Vec3::new(0.0, 1.8, 3.0));
    let off = Was {
        at: Vec3::ZERO,
        on: false,
        near: false,
        cast: false,
    };
    let first = choose(torch(1.5), eye, &[off; 4], 1);
    let slot_of = |shown: &[Option<Shown>], casts: bool| {
        shown
            .iter()
            .position(|s| s.as_ref().is_some_and(|s| s.lit.casts == casts))
            .expect("both of the torch's lights are shown")
    };
    let (key, tip) = (slot_of(&first, true), slot_of(&first, false));
    assert!(first[key].as_ref().is_some_and(|s| s.shadows));
    // Up, down, and along the line both ways, 0.15 m a frame: 9 m/s, a sprinting hand's swing.
    let mut was = was_of(&first);
    for (frame, base) in [1.65, 1.5, 1.35, 1.5, 1.65, 1.8].into_iter().enumerate() {
        let shown = choose(torch(base), eye, &was, 1);
        assert_eq!(slot_of(&shown, true), key, "frame {frame}: the key moved");
        assert_eq!(slot_of(&shown, false), tip, "frame {frame}: the tip moved");
        assert!(
            shown[key].as_ref().is_some_and(|s| s.shadows),
            "frame {frame}: the key lost its shadows"
        );
        was = was_of(&shown);
    }
}

/// A carried torch's key hangs over the holder's head, 0.3 m over where a staked one's would
/// on the same flame, so the holder's own shadow falls short (`torch::HELD_LIGHT_UP`); its tip
/// stays at the flames' tips.
#[test]
fn a_carried_torchs_key_hangs_over_its_holders_head() {
    let blaze = Blaze {
        at: Vec3::new(0.0, 2001.5, 0.0),
        tall: 0.3,
        heat: 0.6,
        seed: 0.3,
        wide: 0.1,
        out: Vec3::ZERO,
    };
    let hang = |share: f32| blaze.at + Vec3::Y * blaze.tall * share;
    let rig = |held| {
        let hearth = torch(0.0)[0].clone();
        torch::lights(&hearth, &blaze, 1.0, Vec3::ZERO, hang, |_| None, held)
    };
    let (held, staked) = (rig(Some(None)), rig(None));
    let over = held[0].at.y - staked[0].at.y;
    assert!((0.25..0.4).contains(&over), "{over} m over a staked key");
    assert!(held[0].casts && !held[1].casts);
    assert_eq!(held[1].at, staked[1].at);
}

/// A torch whose holder's body says where its key hangs (`carried::Carried`): the key stays
/// on that point of the body however the hand swings the flames, and turns and walks with the
/// body; the tip light stays on the flames. On the hand bone the key's step changed 16 mm a
/// frame while running, and every shadow it cast shook with it (torch-light j3).
#[test]
fn a_carried_torchs_key_moves_with_its_holders_body_not_their_hand() {
    use crate::fire::carried::Carried;
    let feet = Vec3::new(0.0, 2000.0, 0.0);
    let by = Carried {
        by: Entity::PLACEHOLDER,
        side: 1.0,
        key: Some([0.3, 2.1, 0.15]),
        fill: None,
    };
    let lights = |holder: Transform, hand: Vec3| {
        let blaze = Blaze {
            at: hand,
            tall: 0.3,
            heat: 0.6,
            seed: 0.3,
            wide: 0.1,
            out: Vec3::ZERO,
        };
        let hang = |share: f32| blaze.at + Vec3::Y * blaze.tall * share;
        let hearth = torch(0.0)[0].clone();
        let at = by.at(&GlobalTransform::from(holder));
        let held = Some(at.key);
        torch::lights(&hearth, &blaze, 1.0, Vec3::ZERO, hang, |_| None, held)
    };
    let stood = Transform::from_translation(feet);
    let hand = feet + Vec3::new(0.3, 1.4, -0.3);
    // The hand swings 0.2 m in a frame: the key does not move, the tip goes with the flames.
    let (a, b) = (
        lights(stood, hand),
        lights(stood, hand + Vec3::new(0.0, 0.15, 0.13)),
    );
    assert_eq!(a[0].at, b[0].at);
    assert!(a[0].casts && !a[1].casts);
    assert!((a[1].at.distance(b[1].at) - 0.2).abs() < 0.01);
    assert!(a[0].at.distance(feet + Vec3::new(0.3, 2.1, -0.15)) < 1e-3);
    // The body walks a metre and turns to face +x: the key goes with it, to its right still.
    let turned = Transform::from_translation(feet + Vec3::X).looking_to(Vec3::X, Vec3::Y);
    let c = lights(turned, hand);
    assert!(
        c[0].at.distance(feet + Vec3::X + Vec3::new(0.15, 2.1, 0.3)) < 1e-3,
        "{}",
        c[0].at - feet
    );
}

/// The lights of one hand torch's flames at `feet + (−0.39, 1.86, −0.43)` (where the left hand
/// holds it standing) over ten seconds at 60 frames a second, as `lit` makes them, with `rig`
/// and its holder's body's places if it is carried.
fn frames(rig: crate::fire::Rig, carried: bool) -> Vec<Vec<Lit>> {
    use crate::fire::carried::CarriedAt;
    let feet = Vec3::new(0.0, 2000.0, 0.0);
    let blazes = Blazes {
        kept: vec![Blaze {
            at: feet + Vec3::new(-0.39, 1.86, -0.43),
            tall: 0.3,
            heat: 0.6,
            seed: 0.3,
            wide: 0.1,
            out: Vec3::ZERO,
        }],
        rigs: vec![rig],
        carried: vec![if carried {
            CarriedAt {
                key: Some(feet + Vec3::new(-0.33, 2.25, -0.47)),
                fill: Some((feet + Vec3::new(0.0, 1.4, 1.0), 0.035)),
            }
        } else {
            CarriedAt::default()
        }],
        ..default()
    };
    (0..600)
        .map(|frame| lit(&blazes, frame as f32 / 60.0, crate::fire::BreathTier::Cards))
        .collect()
}

/// A carried torch lights its holder too: a third light on the point of their body it is
/// given, behind them, a few hundredths of the torch's light, casting nothing, steady while
/// the key flickers, with no glow in the air and no twin, reaching no farther than the body.
/// A staked or wall torch, a carried one whose holder gives no place, and a hearth have two
/// lights, as before.
#[test]
fn a_carried_torch_has_a_dim_steady_fill_on_its_holder_and_no_other_fire_has() {
    use crate::fire::Rig;
    let feet = Vec3::new(0.0, 2000.0, 0.0);
    let held = frames(Rig::Held, true);
    let key: f32 = held.iter().map(|l| l[0].lumens).sum::<f32>() / held.len() as f32;
    let flickers = held.iter().any(|l| (l[0].lumens / key - 1.0).abs() > 0.02);
    assert!(flickers, "the key does not flicker: the test shows nothing");
    for lights in &held {
        assert_eq!(lights.len(), 3);
        let fill = &lights[2];
        assert!(!fill.casts && !fill.kept && fill.still.is_none());
        assert_eq!(fill.at, feet + Vec3::new(0.0, 1.4, 1.0));
        assert!((fill.lumens / held[0][2].lumens - 1.0).abs() < 1e-5);
        assert!(fill.range < 3.0 && fill.color == lights[0].color);
    }
    // The share it is given, of the whole torch's light (its key's and its tip's).
    let whole: f32 = held.iter().map(|l| l[0].lumens + l[1].lumens).sum::<f32>() / 600.0;
    let share = held[0][2].lumens / whole;
    assert!((share - 0.035).abs() < 0.003, "the fill is {share}");
    for (rig, carried) in [
        (Rig::Held, false),
        (Rig::Torch, false),
        (Rig::Hearth, false),
    ] {
        let lights = frames(rig, carried);
        assert!(lights.iter().all(|l| l.len() == 2), "{rig:?}");
    }
    // Its key and tip are what they were without the fill.
    let alone = frames(Rig::Held, false);
    assert_eq!(held[7][0].lumens, alone[7][0].lumens);
    assert_eq!(held[7][1].at, alone[7][1].at);
}

/// With its holder standing still a carried torch's key moves only by its flames' sway, under
/// 5 mm a frame at 60 frames a second: on the hand bone it moved with the arm, its step
/// changing 16 mm a frame while running (torch-light j3). Over its flames it would go with a
/// hand swung 0.2 m, as the test above shows.
#[test]
fn a_carried_torchs_key_on_a_still_body_moves_under_five_millimetres_a_frame() {
    let held = frames(crate::fire::Rig::Held, true);
    let steps: Vec<f32> = held
        .windows(2)
        .map(|w| w[0][0].at.distance(w[1][0].at) * 1000.0)
        .collect();
    let most = steps.iter().copied().fold(0.0_f32, f32::max);
    let mean = steps.iter().sum::<f32>() / steps.len() as f32;
    assert!(most > 0.0, "the key does not sway: the test shows nothing");
    assert!(
        mean < 5.0 && most < 10.0,
        "the key steps {mean} mm a frame on the mean, {most} at most"
    );
    println!("key step mean {mean} mm, most {most} mm");
}

/// Two shadow-casting lights far apart are still matched by place: the nearer entity wins
/// whichever cast last frame.
#[test]
fn lights_metres_apart_are_matched_by_place_whichever_cast() {
    let lit = |x: f32| Lit {
        at: Vec3::new(x, 0.0, 0.0),
        ..torch(0.0)[0].clone()
    };
    let was = |x: f32, cast: bool| Was {
        at: Vec3::new(x, 0.0, 0.0),
        on: true,
        near: cast,
        cast,
    };
    // The brighter-weighed light (nearer the eye) is at 3 m and did not cast; the entity at
    // 0 m did. It stays on its own entity, 3 m from the caster's.
    let shown = choose(
        vec![lit(3.0), lit(0.0)],
        Some(Vec3::new(4.0, 0.0, 0.0)),
        &[was(0.0, true), was(3.0, false)],
        1,
    );
    assert_eq!(shown[1].as_ref().map(|s| s.lit.at.x), Some(3.0));
    assert_eq!(shown[0].as_ref().map(|s| s.lit.at.x), Some(0.0));
}
