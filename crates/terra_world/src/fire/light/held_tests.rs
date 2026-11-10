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
    let (held, staked) = (rig(true), rig(false));
    let over = held[0].at.y - staked[0].at.y;
    assert!((0.25..0.4).contains(&over), "{over} m over a staked key");
    assert!(held[0].casts && !held[1].casts);
    assert_eq!(held[1].at, staked[1].at);
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
