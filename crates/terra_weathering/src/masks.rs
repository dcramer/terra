//! Where weathering goes, from the building's own shape, as shipped games bake it from a
//! mesh's curvature, occlusion and height (Substance's generators, Unreal's mesh distance
//! fields): worked out per face from its neighbouring cubes, then read at any point on it.

use crate::faces::Face;
use crate::{CELL_M, Stuff};
use glam::IVec3;
use terra_gen::smoothstep;

/// How far an outside edge's wear reaches in, in metres.
const EDGE_M: f32 = 0.12;
/// How far an inside corner's grime reaches out, in metres.
const CORNER_M: f32 = 0.35;
/// How far a ledge's shadow of grime reaches down, in metres.
const LEDGE_M: f32 = 0.3;
/// How far under a sill streaks run, in metres.
const STREAK_M: f32 = 2.2;
/// How far over an opening soot and loosened stones reach, in metres.
const ABOVE_M: f32 = 1.2;
/// How high the splash and damp climb from the ground, in metres.
const SPLASH_M: f32 = 1.1;
/// How many cubes up and down an opening or a ledge is looked for.
const LOOK: i32 = 5;

/// What a face's side meets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    /// More of the same face.
    On,
    /// Air: the surface turns away, an outside edge (a corner, a jamb, a wall's end).
    Outside,
    /// A wall standing out in front: the surface turns toward it, a crevice.
    Inside,
}

/// What a face's neighbourhood is, worked out once per face.
#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub stuff: Stuff,
    /// Up, down, along and back along the face: +v, -v, +u, -u.
    sides: [Side; 4],
    /// How many cubes up the opening over it is (its sill), or the ledge over it.
    sill: Option<i32>,
    ledge: Option<i32>,
    /// How many cubes up the top of its wall is, where the wall ends level under a roof:
    /// an eave's wall head, never a gable's stepped verge.
    top: Option<i32>,
    /// How many cubes down the opening under it is (its head).
    head: Option<i32>,
    /// Whether something stands over the air in front of it: an eave, a floor above.
    sheltered: bool,
    /// Cubes from the ground to its foot.
    height: i32,
    up: bool,
}

/// The masks at a point of a face, each 0 to 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Masks {
    /// Near an outside edge: wear, chips.
    pub edge: f32,
    /// In a crevice, an inside corner or under a ledge: grime.
    pub cavity: f32,
    /// Looking up: moss, lichen.
    pub up: f32,
    /// Under a sill or a level wall head under a roof, where water runs down: streaks.
    pub below: f32,
    /// Over an opening: soot, loosened stones.
    pub above: f32,
    /// Near the ground: splash, damp, moss.
    pub ground: f32,
    /// Under an eave: dry, dusty.
    pub sheltered: f32,
}

/// Whether a cube is a wall's: laid stone or timber, not a roof's covering.
fn walling(s: Stuff) -> bool {
    matches!(s, Stuff::Stone | Stuff::Timber)
}

/// Whether the cube `head` is the top of a wall facing `n`: wall, its face open, and no wall
/// over it.
fn is_head(stuff: &dyn Fn(IVec3) -> Stuff, n: IVec3, head: IVec3) -> bool {
    walling(stuff(head)) && !walling(stuff(head + IVec3::Y)) && !stuff(head + n).solid()
}

/// How many cubes up from the foot of `at` the top of its wall is, up through open wall
/// alone, where water running off a roof wets the wall head: only where the wall ends level
/// under a roof (its covering on the wall or an eave over the air in front), as along an
/// eave. A gable's head steps with the roof's slope, so its sloped verge is no source; nor is
/// a wall broken by an opening first (that is a sill's), or one open to the sky.
fn wall_top(stuff: &dyn Fn(IVec3) -> Stuff, face: &Face, at: IVec3) -> Option<i32> {
    let n = face.normal;
    let k = (0..LOOK).find(|&k| !walling(stuff(at + IVec3::Y * (k + 1))))?;
    let head = at + IVec3::Y * k;
    let open = |q: IVec3| walling(stuff(q)) && !stuff(q + n).solid();
    if !(0..=k).all(|j| open(at + IVec3::Y * j)) || !is_head(stuff, n, head) {
        return None;
    }
    let over = head + IVec3::Y;
    if stuff(over) != Stuff::Roof && !stuff(over + n).solid() {
        return None;
    }
    // Level: along it either way to its end, every head at the same height. A step up is wall
    // standing on along it, a step down air with open wall under it.
    let level = [face.u, -face.u].into_iter().all(|d| {
        for i in 1..=LOOK {
            let q = head + d * i;
            if is_head(stuff, n, q) {
                continue;
            }
            let end = if stuff(q).solid() {
                // Another wall standing out in front, an inside corner: its end.
                walling(stuff(q)) && stuff(q + n).solid()
            } else {
                !open(q - IVec3::Y)
            };
            return end;
        }
        true
    });
    level.then_some(k + 1)
}

/// The first `k` in `1..=LOOK` for which `hit` holds.
fn first(hit: impl Fn(i32) -> bool) -> Option<i32> {
    (1..=LOOK).find(|&k| hit(k))
}

impl Context {
    pub fn of(stuff: &dyn Fn(IVec3) -> Stuff, face: &Face, at: IVec3, ground: i32) -> Context {
        let n = face.normal;
        let side = |d: IVec3| {
            let next = at + d;
            if !stuff(next).solid() {
                Side::Outside
            } else if stuff(next + n).solid() {
                Side::Inside
            } else {
                Side::On
            }
        };
        let wall = face.normal.y == 0;
        let solid_over =
            |from: IVec3, k: i32| (k..k + 2 * LOOK).any(|j| stuff(from + IVec3::Y * j).solid());
        let up = |k: i32| at + IVec3::Y * k;
        // An opening over it: air in the wall over it, up through wall alone, with wall (not a
        // roof's covering, as over a gable's step) again over that.
        let sill = (1..=LOOK).find(|&k| !walling(stuff(up(k)))).filter(|&k| {
            wall && !stuff(up(k)).solid() && (k + 1..=k + LOOK).any(|j| walling(stuff(up(j))))
        });
        let top = if wall {
            wall_top(stuff, face, at)
        } else {
            None
        };
        // An opening under it: air in the wall under it, above the ground.
        let head = first(|k| wall && at.y - k >= ground && !stuff(at - IVec3::Y * k).solid());
        let ledge = first(|k| wall && stuff(at + n + IVec3::Y * k).solid());
        Context {
            stuff: stuff(at),
            sides: [side(face.v), side(-face.v), side(face.u), side(-face.u)],
            sill,
            ledge,
            head,
            top,
            sheltered: solid_over(at + n, 1),
            height: at.y - ground,
            up: face.normal.y > 0,
        }
    }

    /// The masks at a point of the face, `(fu, fv)` from 0 to 1 along and up it.
    pub fn at(&self, fu: f32, fv: f32) -> Masks {
        // Distances to each side, in metres: up, down, along, back.
        let to = [
            (1.0 - fv) * CELL_M,
            fv * CELL_M,
            (1.0 - fu) * CELL_M,
            fu * CELL_M,
        ];
        let ramp = |d: f32, reach: f32| (1.0 - d / reach).clamp(0.0, 1.0);
        let mut edge: f32 = 0.0;
        let mut cavity: f32 = 0.0;
        for (side, d) in self.sides.iter().zip(to) {
            match side {
                Side::Outside => edge = edge.max(ramp(d, EDGE_M)),
                Side::Inside => cavity = cavity.max(ramp(d, CORNER_M)),
                Side::On => {}
            }
        }
        if self.ledge == Some(1) {
            cavity = cavity.max(ramp(to[0], LEDGE_M));
        }
        // Metres from the point up to a sill or a level wall head, and down to an opening's head.
        let rise = |k: i32| (k as f32 - fv) * CELL_M;
        let under = self
            .sill
            .into_iter()
            .chain(self.top)
            .map(rise)
            .fold(f32::MAX, f32::min);
        let below = if under < f32::MAX {
            ramp(under, STREAK_M)
        } else {
            0.0
        };
        let above = self
            .head
            .map_or(0.0, |k| ramp((k as f32 - 1.0 + fv) * CELL_M, ABOVE_M));
        let height = (self.height as f32 + fv) * CELL_M;
        let ground = if self.up { 0.0 } else { ramp(height, SPLASH_M) };
        let sheltered = if self.sheltered { 1.0 } else { 0.0 };
        Masks {
            edge,
            cavity,
            up: if self.up { 1.0 } else { 0.0 },
            below: below * (1.0 - 0.6 * sheltered),
            above,
            ground,
            sheltered,
        }
    }
}

/// How far along weathering is at an age in years, 0 to 1, for a process that takes about
/// `years` to show well.
pub fn aged(age: f32, years: f32) -> f32 {
    1.0 - libm::expf(-age.max(0.0) / years)
}

/// The motif layers' masks: where each may go and how strongly, 0 to 1, by the masks, what
/// the face is of and the building's age.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wants {
    pub crack: f32,
    pub loss: f32,
    pub moss: f32,
    pub lichen: f32,
    pub streak: f32,
    pub soot: f32,
}

impl Wants {
    pub fn of(m: &Masks, stuff: Stuff, age: f32) -> Wants {
        let stone = stuff == Stuff::Stone;
        let wall = m.up < 0.5;
        // Stress: over openings and at corners, where a wall settles and frost works in.
        let stress = (0.25 + 0.75 * m.above.max(m.edge * 0.8)).min(1.0);
        let pick = |on: bool, v: f32| if on { v.clamp(0.0, 1.0) } else { 0.0 };
        Wants {
            crack: pick(stone && wall, stress * smoothstep(4.0, 60.0, age)),
            loss: pick(
                stone && wall,
                m.edge.max(m.above * 0.9) * smoothstep(15.0, 120.0, age),
            ),
            moss: pick(
                true,
                m.up.max(m.ground * 0.9).max(m.cavity * 0.6)
                    * (1.0 - 0.75 * m.sheltered)
                    * aged(age, 25.0),
            ),
            lichen: pick(
                stuff != Stuff::Timber,
                (0.35 + 0.4 * m.up) * (1.0 - m.ground) * aged(age, 30.0),
            ),
            streak: pick(wall, m.below * aged(age, 8.0)),
            soot: pick(wall, m.above * aged(age, 20.0)),
        }
    }
}
