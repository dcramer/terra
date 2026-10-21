//! Wakes behind what swims: a V of ripples trailing a duck or a swimmer across a lake, drawn as
//! the rings where fish rise are (`rises`), a slope laid on the water's own waves in the
//! lake's shader (`lake.wesl`, `wake_slopes`), so the wake bends the water's reflection as a
//! real one does. Its arms open at Kelvin's half angle, 19.47 degrees, the angle every body
//! moving steadily over deep water trails (Lord Kelvin, 1887), as games draw a boat's or a
//! bird's wake analytically or from a trail of decals.
//!
//! What swims is the layer above's: it tells [`Wakes`] each frame where its swimmers are, how
//! they head and how fast, and where each was a wake's length back along its way, and the
//! nearest few to the eye are kept. The shader bends each wake's V through that point, so a
//! wake follows the swimmer's turns as a trail of ripples does, rather than trailing straight
//! behind its heading of the moment (lake film r15: ruler-straight wakes behind turning ducks).

use bevy::prelude::*;

/// How many wakes the lake's shader holds at once.
pub const WAKES: usize = 8;
/// Slower than this, metres a second, a swimmer leaves no wake worth drawing.
const STILL_MPS: f32 = 0.05;

/// The wakes being drawn now: for each, where its swimmer is and how fast it goes (w, metres a
/// second), which way it heads, level, and how wide it is (w, metres), and where it was a
/// wake's length back (w 1, or 0 when it left no trail to bend the wake by). None when the
/// speed is 0.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Wakes {
    pub at: [Vec4; WAKES],
    pub way: [Vec4; WAKES],
    pub was: [Vec4; WAKES],
}

/// One swimmer, as the layer above tells it: where, its velocity, its width, and where it was
/// about [`reach_m`] back along its way, if it has come that far.
#[derive(Clone, Copy, Debug)]
pub struct Swimmer {
    pub at: Vec3,
    pub velocity: Vec3,
    pub wide_m: f32,
    pub was: Option<Vec3>,
}

/// How far behind a swimmer its wake reaches, metres, by its width and speed: the shader's
/// `reach` in `wake_slopes`, which fades the wake out over its last 60%.
pub fn reach_m(wide_m: f32, speed_mps: f32) -> f32 {
    wide_m.max(0.05) * 18.0 * (speed_mps / 0.4).clamp(0.5, 2.0)
}

impl Wakes {
    /// The wakes of the swimmers nearest the eye that move, the still ones left out; what
    /// was there before is gone.
    pub fn of(swimmers: impl IntoIterator<Item = Swimmer>, eye: Vec3) -> Self {
        let mut moving: Vec<Swimmer> = swimmers
            .into_iter()
            .filter(|swimmer| swimmer.velocity.length() >= STILL_MPS)
            .collect();
        moving.sort_by(|a, b| {
            a.at.distance_squared(eye)
                .total_cmp(&b.at.distance_squared(eye))
        });
        let mut wakes = Self::default();
        for (slot, swimmer) in moving.into_iter().take(WAKES).enumerate() {
            let speed = swimmer.velocity.length();
            wakes.at[slot] = swimmer.at.extend(speed);
            wakes.way[slot] = (swimmer.velocity / speed).extend(swimmer.wide_m);
            wakes.was[slot] = swimmer.was.map_or(Vec4::ZERO, |was| was.extend(1.0));
        }
        wakes
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Wakes>();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn swimmer(east: f32, speed: f32) -> Swimmer {
        Swimmer {
            at: Vec3::new(east, 0.0, 0.0),
            velocity: Vec3::new(0.0, 0.0, speed),
            wide_m: 0.3,
            was: Some(Vec3::new(east, 0.0, -4.0)),
        }
    }

    #[test]
    fn the_nearest_moving_swimmers_leave_wakes_and_the_still_ones_none() {
        // Twelve paddling and one at rest nearest of all.
        let swimmers = (1..=12)
            .map(|east| swimmer(east as f32 * 5.0, 0.35))
            .chain([swimmer(0.5, 0.0)]);
        let wakes = Wakes::of(swimmers, Vec3::ZERO);
        let kept: Vec<f32> = wakes.at.iter().map(|at| at.x).collect();
        assert_eq!(
            kept,
            (1..=WAKES)
                .map(|east| east as f32 * 5.0)
                .collect::<Vec<_>>()
        );
        // Each heads the way it goes, at its speed, as wide as it is.
        assert!(wakes.at.iter().all(|at| (at.w - 0.35).abs() < 1e-6));
        assert!(
            wakes
                .way
                .iter()
                .all(|way| way.truncate().abs_diff_eq(Vec3::Z, 1e-6) && way.w == 0.3)
        );
        // Each bends through where it was.
        assert!(
            wakes
                .was
                .iter()
                .zip(&wakes.at)
                .all(|(was, at)| *was == Vec4::new(at.x, 0.0, -4.0, 1.0))
        );
        // None at all from a lake of still ducks.
        assert_eq!(
            Wakes::of([swimmer(1.0, 0.01)], Vec3::ZERO),
            Wakes::default()
        );
    }
}
