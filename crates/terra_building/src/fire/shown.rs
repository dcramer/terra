//! The blazes drawn, eased toward the fire's every frame (D377): the fire is stepped on its
//! tick, four a second, and gathered into blazes then; drawn as gathered, every blaze jumped
//! in height, heat and place four times a second, one came up full-grown where a cube caught
//! and was gone where one burnt out, and a level's cells joining into one broad sheet swapped
//! three flames for one at once (castle film r19: a peak at 4 Hz in how the flames changed
//! frame to frame, and one frame in ten a pop; "choppy as fuck"). Shipped fires do the same:
//! Far Cry 2's propagation is a grid of cells stepped on their own clock (Lévesque, 2008,
//! research/2026-10-15-dragons-and-fire.md), and the flames a cell shows are an effect that
//! grows as it catches and dies down as it burns out, never switched on and off with it.

use super::*;

/// A blaze's place: its cell of [`BLAZE_CUBES`] a side, and 1 if it is a broad one, so the
/// broad sheet a level's cells join into is not mistaken for the cell whose key it takes.
pub(super) type Key = [i32; 4];

/// How long a drawn blaze takes to come most of the way (1 - 1/e) to the fire's, in seconds:
/// long enough that a tick's step is a glide (at 0.4 s a 4 Hz step comes through at a tenth),
/// short enough that a blaze grows in as its cubes catch, not after.
pub(super) const EASES_S: f32 = 0.4;

/// How short a drawn blaze whose cubes burnt out is let go at, in metres.
const GONE_M: f32 = 0.02;

/// Eases the drawn blazes `dt` seconds on toward the fire's: one new to it grows in from
/// nothing where it stands, one gone from it dies down there and is let go.
pub(super) fn ease(shown: &mut BTreeMap<Key, Blaze>, fire: &BTreeMap<Key, Blaze>, dt: f32) {
    let k = 1.0 - libm::expf(-dt.max(0.0) / EASES_S);
    for (key, to) in fire {
        let now = shown.entry(*key).or_insert(Blaze { tall: 0.0, ..*to });
        now.at += (to.at - now.at) * k;
        now.tall += (to.tall - now.tall) * k;
        now.heat += (to.heat - now.heat) * k;
        now.wide += (to.wide - now.wide) * k;
        now.out = (now.out + (to.out - now.out) * k).normalize_or_zero();
        now.seed = to.seed;
    }
    shown.retain(|key, now| {
        if fire.contains_key(key) {
            return true;
        }
        now.tall -= now.tall * k;
        now.tall > GONE_M
    });
}

/// The drawn blazes the largest first, as the drawing takes them (those past its slots are
/// not drawn).
pub(super) fn largest_first(shown: &BTreeMap<Key, Blaze>) -> Vec<Blaze> {
    let mut out: Vec<Blaze> = shown.values().copied().collect();
    out.sort_by(|a, b| (b.tall * b.heat).total_cmp(&(a.tall * a.heat)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_build::{Base, BuildOp, Cube, Fill};

    const FRAME_S: f32 = 1.0 / 60.0;

    fn blaze(tall: f32) -> Blaze {
        Blaze {
            at: Vec3::new(1.0, 2.0, 3.0),
            tall,
            heat: 0.8,
            seed: 0.3,
            wide: 0.0,
            out: Vec3::Z,
        }
    }

    #[test]
    fn a_blaze_grows_in_where_it_catches_and_dies_down_where_it_burns_out() {
        let key = [0, 0, 0, 0];
        let fire = BTreeMap::from([(key, blaze(3.0))]);
        let mut shown = BTreeMap::new();
        let (mut last, mut most) = (0.0f32, 0.0f32);
        for frame in 1..=120 {
            ease(&mut shown, &fire, FRAME_S);
            let tall = shown[&key].tall;
            most = most.max(tall - last);
            last = tall;
            if frame == 1 {
                assert!(
                    tall < 0.15,
                    "a blaze came up {tall:.2} m tall on its first frame"
                );
            }
        }
        assert!(last > 2.9, "two seconds on, it stands {last:.2} m of 3");
        assert!(most < 0.15, "it grew {most:.2} m in a frame");
        // Burnt out: it dies down where it stood, a frame at a time, and is let go.
        let out = BTreeMap::new();
        let mut frames = 0;
        while let Some(now) = shown.get(&key) {
            let before = now.tall;
            ease(&mut shown, &out, FRAME_S);
            let after = shown.get(&key).map_or(0.0, |b| b.tall);
            assert!(
                before - after < 0.15,
                "it fell {:.2} m in a frame",
                before - after
            );
            frames += 1;
            assert!(
                frames < 300,
                "a burnt-out blaze still stands after five seconds"
            );
        }
        assert!(frames > 30, "a burnt-out blaze was gone in {frames} frames");
    }

    #[test]
    fn a_ticks_steps_come_through_as_a_glide() {
        // The fire's blaze stepping between 2 and 3 m each tick, four a second: drawn, its
        // height changes by a small part of a step a frame, never the step.
        let key = [0, 0, 0, 0];
        let mut shown = BTreeMap::from([(key, blaze(2.5))]);
        let mut most = 0.0f32;
        let (mut low, mut high) = (f32::MAX, f32::MIN);
        for frame in 0..600 {
            let tall = if ((frame as f32 * FRAME_S / TICK_S) as u32).is_multiple_of(2) {
                2.0
            } else {
                3.0
            };
            let before = shown[&key].tall;
            ease(&mut shown, &BTreeMap::from([(key, blaze(tall))]), FRAME_S);
            let now = shown[&key].tall;
            most = most.max((now - before).abs());
            if frame > 300 {
                (low, high) = (low.min(now), high.max(now));
            }
        }
        assert!(
            most < 0.1,
            "a 1 m step came through as {most:.2} m in a frame"
        );
        assert!(
            high - low < 0.5,
            "it swung {:.2} m with the ticks",
            high - low
        );
    }

    /// A timber wall lit at a corner, its fire stepped on its tick and drawn every frame: the
    /// fire's whole flame (each blaze's height by its heat, summed) as drawn and as gathered,
    /// frame by frame, over ten seconds.
    fn wall_burning() -> (Vec<f32>, Vec<f32>) {
        // As the coursed logs' kit has them burn.
        let logs = Kind(1);
        let burns = |kind: Kind| {
            (kind == logs).then_some(Burns {
                catches: 1.5,
                fuel_s: 20.0,
                heat: 1.0,
            })
        };
        let base = Base::from_shortcuts(&[BuildOp::Fill {
            min: [0, 0, 0],
            max: [23, 11, 0],
            cube: Some(Cube::new(logs, Fill::Solid)),
        }]);
        let mut fire = Fire::default();
        fire.light(&base, &burns, IVec3::new(0, 0, 0));
        fire.light(&base, &burns, IVec3::new(1, 0, 0));
        let (mut gathered, mut shown) = (BTreeMap::new(), BTreeMap::new());
        let (mut owed, mut drawn, mut fed) = (0.0, Vec::new(), Vec::new());
        let whole = |blazes: &BTreeMap<Key, Blaze>| blazes.values().map(|b| b.tall * b.heat).sum();
        for _ in 0..600 {
            owed += FRAME_S;
            while owed >= TICK_S {
                owed -= TICK_S;
                fire.step(&base, &burns, &|_: Kind| 400u16, Vec3::ZERO);
                gathered = gather(&fire, &base, &Transform::IDENTITY);
            }
            ease(&mut shown, &gathered, FRAME_S);
            drawn.push(whole(&shown));
            fed.push(whole(&gathered));
        }
        (drawn, fed)
    }

    #[test]
    fn a_fire_spreading_over_a_wall_is_drawn_advancing_frame_by_frame() {
        let (drawn, gathered) = wall_burning();
        let jump = |all: &[f32]| {
            all.windows(2)
                .map(|w| (w[1] - w[0]).abs() / w[1].max(w[0]).max(1.0))
                .fold(0.0f32, f32::max)
        };
        let (smooth, stepped) = (jump(&drawn), jump(&gathered));
        let end = *drawn.last().unwrap();
        assert!(
            end > 3.0,
            "the wall's fire never spread: {end:.1} m of flame at ten seconds"
        );
        assert!(
            stepped > 0.05,
            "the fire's own ticks jumped only {stepped:.3}: nothing to ease"
        );
        assert!(
            smooth < 0.03 && smooth < stepped / 5.0,
            "drawn, the flame changed by {smooth:.3} of itself in a frame (gathered, {stepped:.3})"
        );
        // It keeps advancing: each second more of it burns than the second before, until
        // most of the wall is alight.
        let seconds: Vec<f32> = drawn
            .chunks(60)
            .map(|s| s.iter().sum::<f32>() / 60.0)
            .collect();
        let grows = seconds.windows(2).take(5).all(|w| w[1] > w[0]);
        assert!(grows, "the drawn fire stalled while it spread: {seconds:?}");
    }
}
