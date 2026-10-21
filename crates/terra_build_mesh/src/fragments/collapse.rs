//! How what was left hanging comes down (D305), by rule for every structure, as masonry does
//! (research/2026-10-18-how-structures-collapse.md):
//!
//! - **Support goes from the bottom up.** Its blocks fall together, the cracks running up
//!   through it a moment apart (`burst`'s stagger), so the lowest courses land first and
//!   break, and what is over them comes down onto their heap ([`super::Rubble`]).
//! - **It tips only when its support gave way on one side and it is slender** ([`falls`]):
//!   a tower whose foot went evenly crumbles where it stood, its shell peeling outward as it
//!   goes, as an imploded building's walls do; a squat block slumps.
//! - **Tipping, it breaks up within a few degrees.** Masonry has almost no tensile strength:
//!   a tall body hinged at its foot bends hardest a third of the way up (the falling
//!   chimney), so it cracks there and at its courses within its material's angle
//!   ([`super::Look::cracks`]), each block flung on as it was going and turning as the whole
//!   was, so they part in the air rather than go over as one.

use super::{Breaks, Fragment, GRAVITY, Shape, unit};
use crate::fragments::Breaking;
use glam::{IVec3, Quat, Vec3};
use terra_build::cubes::Kind;
use terra_build::{CELL_M, Cube};
use terra_gen::hash;

/// A tall fall tipping before it breaks up ([`collapse`]): the edge of its foot it turns
/// about, the axis along that edge, how far over it has gone and how fast it is going over,
/// in radians and radians a second; where the fragment's middle was from that edge and how
/// it was turned when it stood; how tall what tips is, which sets how quickly it goes over;
/// and how far over this fragment cracks off it and flies on its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hinge {
    pub pivot: Vec3,
    pub axis: Vec3,
    pub angle: f32,
    pub rate: f32,
    pub from: Vec3,
    pub stood: Quat,
    pub tall: f32,
    pub lets_go: f32,
}

impl Hinge {
    /// Turns it over by a step as a tall body hinged at its foot falls (θ'' = 3g sin θ / 2L),
    /// returning where the fragment's middle is and how it is turned.
    pub(super) fn step(&mut self, dt: f32) -> (Vec3, Quat) {
        let pull = 1.5 * GRAVITY / self.tall.max(1.0);
        self.rate += pull * libm::sinf(self.angle) * dt;
        self.angle += self.rate * dt;
        let over = Quat::from_axis_angle(self.axis, self.angle);
        (self.pivot + over * self.from, over * self.stood)
    }

    /// How fast a point at `at` on what tips is moving.
    pub(super) fn velocity(&self, at: Vec3) -> Vec3 {
        (self.axis * self.rate).cross(at - self.pivot)
    }
}

/// A section of wall coming down whole, as one rigid body (castle film round 3: a fall of
/// separate blocks read as a picket fence, the old rigid tip as wonky): where its middle
/// was and how fast it was going when it set off, how it turns (the axis, as long as how
/// many radians a second), how long it has been going, and where the fragment's middle was
/// from its middle and how it was turned. Every fragment of a section carries the same
/// motion, so they go together with nothing shared between them, each letting go as it
/// touches ([`super::Fragment::step`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub middle: Vec3,
    pub velocity: Vec3,
    pub spin: Vec3,
    pub age: f32,
    pub from: Vec3,
    pub stood: Quat,
}

/// The longest a fragment goes with its section, in seconds, whatever it touches.
pub const BODY_MOST_S: f32 = 5.0;

impl Body {
    /// Where the fragment's middle is now, and how it is turned.
    pub(super) fn pose(&self) -> (Vec3, Quat) {
        let turned = Quat::from_scaled_axis(self.spin * self.age);
        (self.centre() + turned * self.from, turned * self.stood)
    }

    fn centre(&self) -> Vec3 {
        let t = self.age;
        self.middle + self.velocity * t - Vec3::Y * (0.5 * GRAVITY * t * t)
    }

    /// How fast a point at `at` on the section is going.
    pub(super) fn velocity(&self, at: Vec3) -> Vec3 {
        self.velocity - Vec3::Y * (GRAVITY * self.age) + self.spin.cross(at - self.centre())
    }

    /// Whether it is the same section as `other`: its fragments all set off as one.
    pub fn same(&self, other: &Body) -> bool {
        self.middle == other.middle && self.velocity == other.velocity && self.spin == other.spin
    }
}

/// How fast the pieces of a section cracked where it first landed hard part, in metres a
/// second: enough that its joints open as it comes down, little enough that they come down
/// where the section did.
const CRACKS_APART_M_S: f32 = 0.35;

impl Fragment {
    /// What it comes down as when the section of wall it goes with first lands hard,
    /// `section` (the falling panel: the first of it to hit stops, and the mortar holding the
    /// rest to it fails at once): if it goes with that section, it lets go and cracks along
    /// its courses into its cubes and blocks of a few (`fracture::crack`), each going on as
    /// it was, its joints opening, to land on its own and break where it lands hard. A
    /// section going on as one box after its first block struck read as a crisp slab landing
    /// whole (castle film round 7, 24.2 s). With `room` for fewer pieces than it cracks into,
    /// it lets go whole. None when it goes with no such section.
    pub fn cracks_off(&self, section: &Body, room: usize) -> Option<Vec<Fragment>> {
        let body = self.body.filter(|body| body.same(section))?;
        let going = Fragment {
            body: None,
            velocity: body.velocity(self.at),
            spin: body.spin,
            ..*self
        };
        let count = (self.size / CELL_M).round().max(Vec3::ONE).as_ivec3();
        let cracks = super::fracture::crack(count, self.seed);
        if cracks.len() < 2 || cracks.len() > room {
            return Some(vec![going]);
        }
        let cube = self.size / count.as_vec3();
        let pieces = cracks.into_iter().map(|(low, side)| {
            let part = cube * side.as_vec3();
            let offset = self.turn * (low.as_vec3() * cube + part * 0.5 - self.size * 0.5);
            let seed = hash::of(&[self.seed, (low.x | low.y << 4 | low.z << 8) as u32, 31]);
            let r = |salt: u32| unit(&[seed, salt]) - 0.5;
            let at = self.at + offset;
            let apart = offset.normalize_or_zero() * CRACKS_APART_M_S * (1.0 + r(1));
            Fragment {
                at,
                velocity: body.velocity(at) + apart,
                spin: body.spin + Vec3::new(r(2), r(3), r(4)),
                size: part * super::drawn::SHRINK,
                seed,
                sawn: [
                    self.sawn[0] && low.x == 0,
                    self.sawn[1] && low.x + side.x == count.x,
                ],
                ..going
            }
        });
        Some(pieces.collect())
    }
}

/// How big, at most, the sections a crumbling fall comes down in are, in cubes along a wall
/// and up it: a few metres of wall, which hold together as they drop and break where they
/// land, as a demolished building's walls come down in slabs.
const SECTION_ALONG: f32 = 9.0;
const SECTION_UP: f32 = 7.0;
/// And for a big fall, at least this share of its width along it and of its height up it:
/// a keep's top in sections of a few metres came down as dozens of small slabs sinking into
/// their own heap, crumbling in place (castle film round 5), where a real one's upper walls
/// go as a few big pieces that lean, drop and break again where they land.
const SECTION_SHARE_ALONG: f32 = 0.55;
const SECTION_SHARE_UP: f32 = 0.4;

/// How far over a section of a crumbling fall turns by the time it lands, in radians, when
/// it drops from high enough ([`TIPS_FROM_M`]), varying by half either way; the most it
/// turns a second; how far its foot must drop for it to tip all that way, in metres (the
/// lowest sections, crushed under the rest, only sink); and the share of sections that go
/// over inward, into what they stood round, rather than out.
const TIPS_OVER: f32 = 1.15;
const TIPS_MOST: f32 = 2.2;
const TIPS_FROM_M: f32 = 4.0;
const TIPS_IN: f32 = 0.1;
/// How fast a section going over outward sets off out from what it stood on, in metres a
/// second: its foot slides off the broken wall under it as its top leans out. Turning about
/// its middle alone, a keep's sections went into the walls still standing under them and
/// broke up there, crumbling straight down (castle film round 6, 40 degrees by landing).
const TIPS_OUT_M_S: f32 = 0.9;
/// How far out from a fall's middle a section must be, in metres, to set off that fast.
const TIPS_OUT_WIDE_M: f32 = 4.0;

/// The whole blocks among `hanging` grouped into the sections they come down in: by the face
/// of the fall they are on (the way out from its middle), cut every few metres along it and
/// up it at offsets of its own, and only blocks touching one another, as mortar holds them.
fn sections(
    made: &[Fragment],
    hanging: &[usize],
    middle: Vec3,
    (along, up): (f32, f32),
    seed: u32,
) -> Vec<Vec<usize>> {
    let r = |salt: u32| unit(&[seed, salt]);
    let along_m = along * CELL_M;
    let up_m = up * CELL_M;
    let mut keyed: std::collections::BTreeMap<(u8, i32, i32), Vec<usize>> = Default::default();
    for &i in hanging {
        let out = (made[i].at - middle).with_y(0.0);
        // Its face: the way it lies from the middle, along x or z.
        let face = if out.x.abs() >= out.z.abs() {
            u8::from(out.x >= 0.0)
        } else {
            2 + u8::from(out.z >= 0.0)
        };
        let along = if face < 2 { made[i].at.z } else { made[i].at.x };
        let shift = |salt: u32| r(salt + u32::from(face) * 8);
        let a = ((along / along_m) + shift(1)).floor() as i32;
        let u = ((made[i].at.y / up_m) + shift(2)).floor() as i32;
        keyed.entry((face, a, u)).or_default().push(i);
    }
    // Within each, those touching one another: a gap of air, a window or a wall's far side,
    // parts them.
    let touch = |a: &Fragment, b: &Fragment| {
        let gap = (a.at - b.at).abs() - ((a.turn * a.size).abs() + (b.turn * b.size).abs()) * 0.5;
        gap.max_element() < CELL_M * 0.3
    };
    let mut out = Vec::new();
    for (_, members) in keyed {
        let mut left = members;
        while let Some(first) = left.pop() {
            let mut body = vec![first];
            let mut next = 0;
            while next < body.len() {
                let at = body[next];
                let (near, far): (Vec<usize>, Vec<usize>) =
                    left.into_iter().partition(|&j| touch(&made[at], &made[j]));
                body.extend(near);
                left = far;
                next += 1;
            }
            out.push(body);
        }
    }
    out
}

/// How what was left hanging comes down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Falls {
    /// Where it stood, onto its own heap.
    InPlace,
    /// Over the way its support gave way, on the level.
    Tips { way: Vec3 },
}

/// From how many cubes tall, and how many in all, a fall can tip; how much taller than deep
/// along the way it would go it must be (a squat block slumps); how far off its foot's
/// middle what gave way under it must be, as a share of its foot's half depth that way, to
/// count as one-sided; and how many cubes under its lowest what gave way may be.
const TIPS_TALL: i32 = 8;
const TIPS_FROM: usize = 120;
const SLENDER: f32 = 1.5;
const UNEVEN: f32 = 0.3;
const FOOT_CUBES: i32 = 2;

/// How what was left hanging of `fell` comes down, from where its support gave way:
/// `destroyed` says which of it harm broke.
pub fn falls(fell: &[(IVec3, Cube)], destroyed: &dyn Fn(IVec3) -> bool) -> Falls {
    let hanging: Vec<IVec3> = fell
        .iter()
        .filter(|(at, cube)| !destroyed(*at) && !cube.kind.is_roof())
        .map(|(at, _)| *at)
        .collect();
    if hanging.len() < TIPS_FROM {
        return Falls::InPlace;
    }
    let (low, high) = bounds(&hanging);
    let tall = high.y - low.y + 1;
    if tall < TIPS_TALL {
        return Falls::InPlace;
    }
    // Its foot, and what gave way under it.
    let level = |at: &IVec3| Vec3::new(at.x as f32 + 0.5, 0.0, at.z as f32 + 0.5);
    let foot: Vec<Vec3> = hanging
        .iter()
        .filter(|at| at.y < low.y + FOOT_CUBES)
        .map(level)
        .collect();
    let gave: Vec<Vec3> = fell
        .iter()
        .filter(|(at, _)| destroyed(*at) && at.y < low.y + FOOT_CUBES && at.y >= low.y - FOOT_CUBES)
        .filter(|(at, _)| {
            at.x >= low.x - 1 && at.x <= high.x + 1 && at.z >= low.z - 1 && at.z <= high.z + 1
        })
        .map(|(at, _)| level(at))
        .collect();
    if gave.is_empty() || foot.is_empty() {
        return Falls::InPlace;
    }
    let middle = |points: &[Vec3]| points.iter().sum::<Vec3>() / points.len() as f32;
    let off = middle(&gave) - middle(&foot);
    let Some(way) = off.try_normalize() else {
        return Falls::InPlace;
    };
    let along = |way: Vec3| {
        let (least, most) = foot.iter().fold((f32::MAX, f32::MIN), |(least, most), at| {
            (least.min(at.dot(way)), most.max(at.dot(way)))
        });
        most - least + 1.0
    };
    let deep = along(way);
    if off.length() < UNEVEN * deep * 0.5 || (tall as f32) < SLENDER * deep {
        return Falls::InPlace;
    }
    Falls::Tips { way }
}

/// How fast, at most, in metres a second, a block crumbling in place is thrown out from the
/// middle of what fell, at its shell and at its top, as an imploded wall peels outward.
const PEELS_OUT: f32 = 1.4;
/// How far over a tipping fall starts and how fast it is going, in radians and radians a
/// second; how much its blocks' crack angles vary, as a share either way.
const TIP_START: f32 = 0.03;
const TIP_KICK: f32 = 0.15;
const CRACKS_VARY: f32 = 0.3;

/// What was left hanging, as `burst` made it, set coming down as its support says
/// ([`falls`]): crumbling where it stood, or tipping and cracking up within its
/// material's angle. `breaks` says how each kind breaks.
pub fn collapse(
    made: &mut [Fragment],
    fell: &[(IVec3, Cube)],
    destroyed: &dyn Fn(IVec3) -> bool,
    breaks: &dyn Fn(Kind) -> Breaks,
) {
    let hanging: Vec<usize> = (0..made.len())
        .filter(|&i| made[i].shape == Shape::Whole && made[i].hinge.is_none())
        .collect();
    if hanging.len() < 2 {
        return;
    }
    let middle = hanging.iter().map(|&i| made[i].at).sum::<Vec3>() / hanging.len() as f32;
    let bottom = hanging
        .iter()
        .map(|&i| made[i].at.y - made[i].size.y * 0.5)
        .fold(f32::MAX, f32::min);
    let top = hanging
        .iter()
        .map(|&i| made[i].at.y + made[i].size.y * 0.5)
        .fold(f32::MIN, f32::max);
    let tall = (top - bottom).max(CELL_M);
    match falls(fell, destroyed) {
        Falls::InPlace => {
            // How far out its shell reaches from its middle, on the level.
            let reach = hanging
                .iter()
                .map(|&i| (made[i].at - middle).with_y(0.0).length())
                .fold(CELL_M, f32::max);
            // Its shell leans out as slabs, each face turning outward about its foot (v = ω h,
            // one ω for the face): neighbouring blocks go and turn together and part only as
            // they land, as a demolished wall's face folds out whole. A share of each block's
            // own throw and spin, as `burst` made them, scattered them into a spray of tiles
            // in the castle film's falls (2026-10-21).
            for &i in &hanging {
                let piece = &mut made[i];
                let out = (piece.at - middle).with_y(0.0);
                let way = out.normalize_or_zero();
                let shell = (out.length() / reach).clamp(0.0, 1.0);
                let height = ((piece.at.y - bottom) / tall).clamp(0.0, 1.0);
                let lean = PEELS_OUT * shell / tall;
                piece.velocity = way * lean * height * tall;
                piece.spin = Vec3::Y.cross(way) * lean;
            }
            // Its blocks come down in sections, each as one body, all setting off as its
            // lowest does: big for a big fall, and those that drop far tipping over about
            // their foot as they go, mostly outward, so the top of a keep leans out in a
            // few big pieces, drops and breaks again where it lands rather than sinking
            // into its heap (castle film round 5; demolition footage, Red Faction:
            // Guerrilla's walls).
            let seed = hash::of(&[hanging.len() as u32, bottom.to_bits(), 5]);
            let wide = hanging
                .iter()
                .fold((Vec3::MAX, Vec3::MIN), |(low, high), &i| {
                    (low.min(made[i].at), high.max(made[i].at))
                });
            let wide = ((wide.1 - wide.0).with_y(0.0).max_element() + CELL_M) / CELL_M;
            let size = (
                SECTION_ALONG.max(wide * SECTION_SHARE_ALONG),
                SECTION_UP.max(tall / CELL_M * SECTION_SHARE_UP),
            );
            for (n, section) in sections(made, &hanging, middle, size, seed)
                .into_iter()
                .enumerate()
            {
                if section.len() < 2 {
                    continue;
                }
                let count = section.len() as f32;
                let centre = section.iter().map(|&i| made[i].at).sum::<Vec3>() / count;
                let (velocity, spin) = tips(made, &section, centre, middle, bottom, seed, n);
                let wait = section
                    .iter()
                    .map(|&i| made[i].wait)
                    .fold(f32::MAX, f32::min);
                for &i in &section {
                    let piece = &mut made[i];
                    piece.wait = wait;
                    piece.body = Some(Body {
                        middle: centre,
                        velocity,
                        spin,
                        age: 0.0,
                        from: piece.at - centre,
                        stood: piece.turn,
                    });
                }
            }
        }
        Falls::Tips { way } => {
            // The edge of its foot on that side: as far that way as any of it reaches.
            let reach = hanging
                .iter()
                .map(|&i| (made[i].at - middle).dot(way) + made[i].size.max_element() * 0.5)
                .fold(0.0f32, f32::max);
            let pivot = Vec3::new(middle.x, bottom, middle.z) + way * reach;
            let axis = way.cross(Vec3::NEG_Y).normalize();
            let seed = hash::of(&[hanging.len() as u32, bottom.to_bits()]);
            let start = TIP_START * (0.7 + 0.6 * unit(&[seed, 2]));
            for &i in &hanging {
                let piece = &mut made[i];
                let height = ((piece.at.y - bottom) / tall).clamp(0.0, 1.0);
                // It bends hardest a third of the way up: what is over that cracks off
                // first, the stub under it a little later.
                let cracks = breaks(piece.kind).look().cracks;
                let stub = if height < 1.0 / 3.0 { 1.6 } else { 1.0 };
                let vary = 1.0 + CRACKS_VARY * (2.0 * unit(&[piece.seed, 77]) - 1.0);
                piece.hinge = Some(Hinge {
                    pivot,
                    axis,
                    angle: start,
                    rate: TIP_KICK,
                    from: piece.at - pivot,
                    stood: piece.turn,
                    tall,
                    lets_go: start + cracks * stub * vary,
                });
                // All of it goes together, at once.
                piece.wait = 0.0;
                piece.velocity = Vec3::ZERO;
                piece.spin = Vec3::ZERO;
            }
        }
    }
}

/// How a section of a crumbling fall sets off, its middle at `centre`: turning over about
/// its foot, the way out from the fall's `middle` along x or z (inward for a few), as far
/// by the time it lands as its drop from over the fall's `bottom` lets it, its middle going
/// that way as a body turning about its foot does (v = ω h / 2), so its foot stays where it
/// was as its top goes over.
fn tips(
    made: &[Fragment],
    section: &[usize],
    centre: Vec3,
    middle: Vec3,
    bottom: f32,
    seed: u32,
    n: usize,
) -> (Vec3, Vec3) {
    let low = section
        .iter()
        .map(|&i| made[i].at.y - made[i].size.y * 0.5)
        .fold(f32::MAX, f32::min);
    let high = section
        .iter()
        .map(|&i| made[i].at.y + made[i].size.y * 0.5)
        .fold(f32::MIN, f32::max);
    let out = (centre - middle).with_y(0.0);
    let face = if out.x.abs() >= out.z.abs() {
        Vec3::X * out.x.signum()
    } else {
        Vec3::Z * out.z.signum()
    };
    let r = |salt: u32| unit(&[seed, n as u32, salt]);
    let way = if r(1) < TIPS_IN { -face } else { face };
    let drop = (low - bottom).max(0.0);
    let lands_s = libm::sqrtf(2.0 * drop / GRAVITY).max(0.35);
    let over = TIPS_OVER * (0.5 + r(2)) * (drop / TIPS_FROM_M).min(1.0);
    let rate = (over / lands_s).min(TIPS_MOST);
    // The wider the fall the more: a slender tower's sections only lean (its heap stays at
    // its foot), a keep's walls slide off the storey under them.
    let wide = (out.length() / TIPS_OUT_WIDE_M).min(1.0);
    let kick = if way == face {
        TIPS_OUT_M_S * (drop / TIPS_FROM_M).min(1.0) * wide
    } else {
        0.0
    };
    (
        way * (rate * (high - low) * 0.5 + kick),
        Vec3::Y.cross(way) * rate,
    )
}

fn bounds(cubes: &[IVec3]) -> (IVec3, IVec3) {
    cubes.iter().fold(
        (IVec3::splat(i32::MAX), IVec3::splat(i32::MIN)),
        |(low, high), at| (low.min(*at), high.max(*at)),
    )
}

#[cfg(test)]
#[path = "collapse_tests.rs"]
mod tests;
