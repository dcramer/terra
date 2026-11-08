//! A heavy blow's and a killing blow's push (strike.rs): the struck body slides away from the
//! blow, quick then easing out, and stops at what it meets, as the ARPGs' knockback does
//! (Path of Exile's moves a hit monster a set distance from the attacker over a fraction of a
//! second, less for a heavier one; Diablo IV's corpses are thrown away from the killing blow).
//! Where we depart: no ragdoll. A killing blow turns the body over its first moments so its
//! death clip falls away from the blow (each clip's fall measured from its model, `Weight`),
//! then slides it a little less far. How far is the body's weight, data per foe kind
//! (the biome's `foes[].weight`); the slide goes round the props and bodies as a step does
//! (play.rs `step_to`), so a body never ends inside a tree.

use super::footprint::{self, Blocking, Footprint};
use super::{Ground, step_to};
use bevy::prelude::*;

/// A struck body's heft: its weight (kg) and which way its death clip falls, as an angle
/// from its front toward its left (+x of its model, radians; `None` keeps its facing).
#[derive(Component, Clone, Copy)]
pub struct Weight {
    pub kg: f32,
    pub falls: Option<f32>,
}

/// The weight a push's distance is given for (kg).
pub const MASS: f32 = 120.0;
/// A heavy blow's and a killing blow's slide at `MASS` (m), the range any weight's is kept to,
/// and how long each takes (s): within the mob's stagger (demo/mobs.rs, 0.6 s).
const SHOVE: (f32, (f32, f32), f32) = (0.9, (0.4, 1.0), 0.3);
const THROW: (f32, (f32, f32), f32) = (0.5, (0.25, 0.6), 0.45);
/// How long a killing blow's turn takes (s): the death clip's first moments, before it falls.
const TURN: f32 = 0.2;
/// The longest piece a slide is stepped in (m), so it cannot pass through a thin footprint.
const PIECE: f32 = 0.05;

/// A slide under way: its way, length and time, how far it has gone, and a killing blow's
/// turn (from, to).
#[derive(Clone, Copy, Debug)]
pub struct Knock {
    way: Vec2,
    length: f32,
    secs: f32,
    at: f32,
    gone: f32,
    turn: Option<(Quat, Quat)>,
}

/// The slide's share done at `t` of its time: fast, then easing out (a cubic's).
pub fn ease(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

/// A slide's length for a body of `kg`: inverse to its weight, kept to its range.
fn length((at_mass, (lo, hi), _): (f32, (f32, f32), f32), kg: f32) -> f32 {
    (at_mass * MASS / kg.max(1.0)).clamp(lo, hi)
}

/// The facing that lays a death clip falling `falls` from its front along `away`.
pub fn fall_facing(away: Vec2, falls: f32) -> Quat {
    Quat::from_rotation_y(libm::atan2f(away.x, away.y) - falls)
}

impl Knock {
    /// A heavy blow from `from` on a body at `at`, at time `now`.
    pub fn heavy(from: Vec2, at: Vec2, w: Weight, now: f32) -> Self {
        Self::new(from, at, SHOVE, w.kg, now, None)
    }

    /// A killing blow: the slide, and the turn from `facing` that lays its fall away.
    pub fn death(from: Vec2, at: Vec2, w: Weight, now: f32, facing: Quat) -> Self {
        let away = (at - from).normalize_or(Vec2::X);
        let turn = w.falls.map(|f| (facing, fall_facing(away, f)));
        Self::new(from, at, THROW, w.kg, now, turn)
    }

    fn new(
        from: Vec2,
        at: Vec2,
        kind: (f32, (f32, f32), f32),
        kg: f32,
        now: f32,
        turn: Option<(Quat, Quat)>,
    ) -> Self {
        Knock {
            way: (at - from).normalize_or(Vec2::X),
            length: length(kind, kg),
            secs: kind.2,
            at: now,
            gone: 0.0,
            turn,
        }
    }

    /// Whether it still moves its body at `now` (a steered mob is left to it, demo/mobs.rs).
    pub fn sliding(&self, now: f32) -> bool {
        self.gone < self.length && now < self.at + self.secs.max(TURN)
    }

    /// Its body's place at `now` from `here`, stepped in pieces by `step` (where a step from a
    /// place lands, `None` where it cannot): the slide ends where a piece is refused.
    pub fn advance(
        &mut self,
        now: f32,
        mut here: Vec2,
        step: impl Fn(Vec2, Vec2) -> Option<Vec2>,
    ) -> Vec2 {
        let want = self.length * ease((now - self.at) / self.secs);
        while self.gone < want - 1e-5 {
            let piece = (want - self.gone).min(PIECE);
            match step(here, self.way * piece) {
                Some(q) => here = q,
                None => {
                    self.gone = self.length;
                    break;
                }
            }
            self.gone += piece;
        }
        here
    }

    /// A killing blow's facing at `now`, while it turns and after.
    pub fn facing(&self, now: f32) -> Option<Quat> {
        self.turn
            .map(|(a, b)| a.slerp(b, ease((now - self.at) / TURN)))
    }
}

/// Each sliding struck body moved on, round the ground's edges, the nav grid and every other
/// body's and prop's footprint grown by its own radius; a killing blow's turn laid on it.
pub(super) fn slide(
    time: Res<Time>,
    ground: Option<Res<Ground>>,
    nav: Option<Res<crate::nav::Nav>>,
    mut bodies: Query<(Entity, &mut super::strike::Struck, &mut Transform)>,
    footprints: Query<(Entity, &Footprint, &GlobalTransform), Blocking>,
) {
    let Some(ground) = ground else {
        return;
    };
    let now = time.elapsed_secs();
    for (body, mut s, mut tr) in &mut bodies {
        let Some(k) = s.knock.as_mut() else {
            continue;
        };
        if let Some(f) = k.facing(now) {
            tr.rotation = f;
        }
        if !k.sliding(now) {
            continue;
        }
        // The others' circles, grown by how much wider it is than the hero step_to sizes for.
        let own = footprints
            .get(body)
            .ok()
            .and_then(|(_, f, g)| f.0.first().map(|c| c.1 * g.scale().x))
            .unwrap_or(footprint::BODY);
        let circles: Vec<(Vec2, f32)> = footprints
            .iter()
            .filter(|(e, ..)| *e != body)
            .flat_map(|(_, f, g)| footprint::placed(f, g))
            .map(|(c, r)| (c, r + own - footprint::BODY))
            .collect();
        let here = tr.translation.xz();
        let to = k.advance(now, here, |h, s| {
            step_to(&ground, nav.as_deref(), &circles, h, s)
        });
        tr.translation = to.extend((ground.floor)(to)).xzy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open() -> Ground {
        Ground {
            floor: Box::new(|_| 0.0),
            open: Box::new(|_| true),
        }
    }

    /// Its body's place each frame of a slide from the blow at `from` on a body at `at`.
    fn run(mut k: Knock, at: Vec2, circles: &[(Vec2, f32)]) -> Vec<Vec2> {
        let g = open();
        let mut here = at;
        (0..=60)
            .map(|i| {
                here = k.advance(i as f32 / 60.0, here, |h, s| {
                    step_to(&g, None, circles, h, s)
                });
                here
            })
            .collect()
    }

    const MAN: Weight = Weight {
        kg: MASS,
        falls: None,
    };

    /// A heavy blow from the south-west slides its body north-east, away from the hero, its
    /// whole length (0.5 to 1 m) on open ground, fast then slowing; a heavier body slides
    /// less far, a lighter one no further than the range.
    #[test]
    fn a_heavy_blow_slides_its_body_away() {
        let (from, at) = (Vec2::new(-1.0, -1.0), Vec2::ZERO);
        let path = run(Knock::heavy(from, at, MAN, 0.0), at, &[]);
        let end = *path.last().unwrap();
        let away = (at - from).normalize();
        assert!(
            end.normalize().dot(away) > 0.999,
            "slid {end}, not away along {away}"
        );
        assert!(
            (0.5..=1.0).contains(&end.length()),
            "slid {} m",
            end.length()
        );
        // Over half its length in its first third: eased out.
        assert!(
            path[6].length() > 0.5 * end.length(),
            "{} of {}",
            path[6].length(),
            end.length()
        );
        let heavy = Weight {
            kg: 2.0 * MASS,
            ..MAN
        };
        let light = Weight {
            kg: 0.2 * MASS,
            ..MAN
        };
        let far = |w| {
            run(Knock::heavy(from, at, w, 0.0), at, &[])
                .last()
                .unwrap()
                .length()
        };
        assert!(
            far(heavy) < end.length() - 0.1,
            "a heavier body slides less"
        );
        assert!(far(light) <= SHOVE.1.1 + 1e-4, "kept to its range");
    }

    /// A tree's footprint half a metre behind the body stops the slide at its edge: the body
    /// never ends inside it (the planted fault: a slide that ignored footprints ends 0.9 m on,
    /// in the trunk), and it is no further than with nothing there.
    #[test]
    fn a_slide_is_blocked_by_a_footprint() {
        let (from, at) = (Vec2::new(-1.0, 0.0), Vec2::ZERO);
        let tree = (Vec2::new(0.8, 0.0), 0.3);
        let free = *run(Knock::heavy(from, at, MAN, 0.0), at, &[])
            .last()
            .unwrap();
        let path = run(Knock::heavy(from, at, MAN, 0.0), at, &[tree]);
        let near = path
            .iter()
            .map(|p| p.distance(tree.0))
            .fold(f32::MAX, f32::min);
        assert!(
            near >= tree.1 + footprint::BODY - 1e-3,
            "{near} m from the trunk's middle"
        );
        assert!(
            free.distance(tree.0) < tree.1,
            "the planted fault: unblocked it ends in the trunk"
        );
        assert!(path.last().unwrap().x < free.x - 0.2, "stopped short");
    }

    /// A killing blow turns the body so its death clip's fall, measured from its front, lies
    /// along the blow's way: a clip falling to its left (a quarter turn from its front) struck
    /// from the west by the hero it faced falls east, not north as it stood; the turn is done
    /// in its first moments and held.
    #[test]
    fn a_killing_blow_lays_the_fall_away_from_it() {
        use core::f32::consts::FRAC_PI_2;
        let w = Weight {
            falls: Some(FRAC_PI_2 * 1.05),
            ..MAN
        };
        let (from, at) = (Vec2::new(-1.0, 0.0), Vec2::ZERO);
        let k = Knock::death(from, at, w, 0.0, Quat::from_rotation_y(-FRAC_PI_2));
        let fall = |q: Quat| {
            let f = w.falls.unwrap();
            (q * Vec3::new(libm::sinf(f), 0.0, libm::cosf(f))).xz()
        };
        assert!(fall(k.facing(TURN).unwrap()).dot(Vec2::X) > 0.999);
        assert_eq!(k.facing(2.0), k.facing(TURN));
        assert!(
            fall(k.facing(0.0).unwrap()).dot(Vec2::X) < 0.5,
            "it starts as it stood"
        );
        let end = *run(k, at, &[]).last().unwrap();
        assert!(end.x > 0.2 && end.y.abs() < 1e-3, "slid {end}");
        assert!(
            Knock::death(from, at, MAN, 0.0, Quat::IDENTITY)
                .facing(1.0)
                .is_none(),
            "no fall measured: no turn"
        );
    }
}
