//! Fragments (D305; the developers on 2026-10-15: what is destroyed "needs to kind of..
//! 'explode' into voxels … a bunch of cubes that fall onto the ground and you cant interact
//! with that might fade when done", and "all the destroy thing needs to be performant AND
//! look sick"): what comes down breaks into chunky voxel-ish pieces of its own material, as
//! Enshrouded's building destruction reads and with Teardown's chunky debris for feel.
//!
//! - **How a kind breaks is data** ([`Breaks`], its kit's `toughness.breaks`, by its pattern
//!   unless its material says otherwise): stone into chunky irregular blocks, logs into
//!   lengths and splinters, wattle, plaster and roofs into small bits, earth crumbling. Each
//!   way of breaking is a table ([`Breaks::look`]), not code per material.
//! - **A cube destroyed bursts** at once, its pieces flung out from where it was hit,
//!   tumbling. **A cube left hanging falls whole**, a little after what gave way under it,
//!   the higher the later, and breaks where it lands ([`Fragment::shatter`]), so a wall or a
//!   tower comes down progressively rather than in one frame.
//! - **They move on simple physics of our own** ([`Fragment::step`]): gravity, a bounce off
//!   whatever is under them with friction, sliding and rolling off a ledge, glancing off a
//!   wall they are thrown into. The game says what is under a point (the terrain, and the
//!   base's cubes through [`Base::top_under`]) and fades them once they lie still.
//! - **Each piece is a few shapes** ([`Shape`]), a box bevelled to chips, drawn in its kind's
//!   material at any size and proportion, so the game draws them instanced.
//!
//! Without Bevy, and the same every time: everything follows from where each cube was.

use crate::{Material, Surfaces};
use glam::{IVec3, Quat, Vec3};
use terra_build::CELL_M;
use terra_build::cubes::{Cube, FULL, Kind};
use terra_build::kit::{Kit, Pattern};
use terra_gen::hash;

mod collapse;
mod fracture;
mod rubble;
pub use collapse::{BODY_MOST_S, Body, Falls, Hinge, collapse, falls};
pub use rubble::{Gone, REPOSE, Rubble, gone};

/// The most fragments a fall makes, and the most about at once, at the highest preset (a
/// castle's towers and walls coming down together make some thousands): the
/// game makes fewer at lower ones and fades the oldest first past it.
pub const MOST: usize = 5000;
/// The most cubes across a whole chunk's mesh wears its material for ([`Fragment::mesh`]):
/// a bigger one wears it stretched.
pub const MOST_DRAWN: u8 = 8;
/// How fast fragments fall, in metres a second a second.
pub const GRAVITY: f32 = 9.8;
/// What is kept of a fragment's speed off the floor when it bounces, and along the floor.
const BOUNCE: f32 = 0.32;
const FRICTION: f32 = 0.6;
/// Slower than this, in metres a second, a fragment on the floor lies still.
const SETTLE: f32 = 0.3;
/// How hard, in metres a second, a whole cube must land to break.
const BREAKS_AT: f32 = 1.8;
/// The steepest a fragment rests on, as a rise a metre, about 20 degrees: on anything
/// steeper, a roof's pitch, it slides, losing so much of its pull down to friction; and how
/// far either side of it the slope is read.
const SLIDES_AT: f32 = 0.36;
const SLIDE_FRICTION: f32 = 0.35;
const SLOPE_READ_M: f32 = 0.12;
/// How long after what gave way a cube left hanging starts to fall, for each cube it stood
/// above it, in seconds, and at most: the cracks running up through it as its support goes
/// from the bottom up, quickly, so its courses part by a hand's breadth as they fall rather
/// than a lattice opening (0.035 s a cube opened gaps of most of a cube at landing speed).
const STAGGER_S: f32 = 0.012;
const STAGGER_MOST_S: f32 = 0.3;
/// How much of what a landing's impulse at its lowest corner would turn a fragment it is
/// left with (a box's I = m s² / 6 about its middle), and the fastest it tumbles.
const TUMBLE: f32 = 0.5;
const MOST_SPIN: f32 = 14.0;

pub use terra_build::pieces::Breaks;

/// The shapes a piece is drawn as: a box with its edges bevelled, more for a rougher one.
/// Its size and proportions are the piece's own, so a few meshes draw every piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Shape {
    /// A whole cube falling before it breaks: barely bevelled, so the blocks of a wall falling
    /// together don't draw a dark groove at every joint.
    Whole,
    /// A chunk with its edges chipped off.
    Chunk,
    /// A rough lump, bevelled nearly round.
    Lump,
}

impl Shape {
    pub const ALL: [Shape; 3] = [Shape::Whole, Shape::Chunk, Shape::Lump];

    /// How much of its half width each edge is bevelled by.
    fn bevel(self) -> f32 {
        match self {
            Shape::Whole => 0.02,
            Shape::Chunk => 0.22,
            Shape::Lump => 0.42,
        }
    }
}

/// One kind of piece a way of breaking makes: its shape, its size in metres along its three
/// axes (the least and the most), and how many of it a cube makes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub shape: Shape,
    pub least: [f32; 3],
    pub most: [f32; 3],
    pub count: u32,
}

/// How a way of breaking looks: its pieces, how fast they are thrown and spun, and how much
/// dust it puts up, from 0 to 1; how many cubes a side, at least, a big fall of it comes
/// down in (masonry along its joints in blocks of a metre or more, timber as its members,
/// plaster and earth small); and how far over, in radians, a tipping fall of it cracks
/// apart (masonry, with almost no tensile strength, within a few degrees; timber bends).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub pieces: &'static [Piece],
    pub throw: f32,
    pub spin: f32,
    pub dust: f32,
    pub chunk: i32,
    pub cracks: f32,
}

const fn piece(shape: Shape, least: [f32; 3], most: [f32; 3], count: u32) -> Piece {
    Piece {
        shape,
        least,
        most,
        count,
    }
}

const CHUNKS: [Piece; 2] = [
    piece(Shape::Chunk, [0.16, 0.12, 0.14], [0.26, 0.2, 0.24], 5),
    piece(Shape::Chunk, [0.07, 0.06, 0.07], [0.12, 0.1, 0.12], 6),
];

const LENGTHS: [Piece; 2] = [
    piece(Shape::Chunk, [0.32, 0.1, 0.1], [0.5, 0.15, 0.15], 3),
    piece(Shape::Chunk, [0.15, 0.03, 0.04], [0.3, 0.05, 0.06], 5),
];

const BITS: [Piece; 2] = [
    piece(Shape::Chunk, [0.1, 0.03, 0.08], [0.18, 0.05, 0.14], 9),
    piece(Shape::Lump, [0.05, 0.04, 0.05], [0.08, 0.06, 0.08], 4),
];

const CRUMBS: [Piece; 2] = [
    piece(Shape::Lump, [0.1, 0.08, 0.1], [0.2, 0.15, 0.18], 5),
    piece(Shape::Lump, [0.04, 0.04, 0.04], [0.08, 0.07, 0.08], 8),
];

/// How a way of breaking looks and how many pieces it makes: the drawing's side of
/// [`Breaks`], which the rules name (D407).
pub trait Breaking {
    /// How it looks: the table every material breaking this way shares.
    fn look(self) -> Look;

    /// How many pieces a cube breaks into, all told.
    fn count(self) -> u32;
}

impl Breaking for Breaks {
    fn look(self) -> Look {
        match self {
            Breaks::Chunks => Look {
                pieces: &CHUNKS,
                throw: 1.0,
                spin: 6.0,
                dust: 1.0,
                chunk: 2,
                cracks: 0.12,
            },
            Breaks::Lengths => Look {
                pieces: &LENGTHS,
                throw: 0.8,
                spin: 8.0,
                dust: 0.45,
                chunk: 1,
                cracks: 0.35,
            },
            Breaks::Bits => Look {
                pieces: &BITS,
                throw: 1.2,
                spin: 10.0,
                dust: 0.8,
                chunk: 1,
                cracks: 0.08,
            },
            Breaks::Crumbs => Look {
                pieces: &CRUMBS,
                throw: 0.6,
                spin: 4.0,
                dust: 1.4,
                chunk: 1,
                cracks: 0.06,
            },
        }
    }

    fn count(self) -> u32 {
        self.look().pieces.iter().map(|piece| piece.count).sum()
    }
}

/// A fragment of what came down: of what kind and shape, where its middle is in the base's
/// own space, how it moves and turns, how big it is along its axes, whether it is a whole
/// cube yet to break, how long before it starts to fall, and whether it has come to rest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fragment {
    pub kind: Kind,
    pub shape: Shape,
    pub at: Vec3,
    pub velocity: Vec3,
    pub turn: Quat,
    /// Its spin: the axis, as long as how many radians a second.
    pub spin: Vec3,
    pub size: Vec3,
    pub wait: f32,
    pub still: bool,
    /// How full the cube was, for a whole one: what it breaks into.
    pub fill: f32,
    pub seed: u32,
    /// While it still leans with what it came down with, the hinge they tip over on.
    pub hinge: Option<Hinge>,
    /// While it still goes with the section of wall it came down in, that section's
    /// motion ([`Body`]).
    pub body: Option<Body>,
}

/// What happened to a fragment in a step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Landing {
    /// It hit the floor this hard, in metres a second, if it did.
    pub hit: Option<f32>,
    /// A whole cube hit hard enough to break.
    pub breaks: bool,
}

impl Fragment {
    /// Which mesh it is drawn with: its shape, and for a whole cube or block how many cubes
    /// across, so its material shows at its real size; and how big that mesh's texture is
    /// worn for, in metres.
    pub fn mesh(&self) -> (Shape, u8, f32) {
        match self.shape {
            Shape::Whole => {
                let across = (self.size.max_element() / CELL_M)
                    .round()
                    .clamp(1.0, f32::from(MOST_DRAWN));
                (self.shape, across as u8, across * CELL_M)
            }
            shape => (shape, 0, PIECE_M),
        }
    }

    /// How far its middle is above its lowest point, turned as it is: what rests on the
    /// floor.
    fn reach_down(&self) -> f32 {
        let half = self.size * 0.5;
        let (x, y, z) = (
            self.turn * Vec3::X * half.x,
            self.turn * Vec3::Y * half.y,
            self.turn * Vec3::Z * half.z,
        );
        // Bevelled boxes rest a little lower than their corners.
        (x.y.abs() + y.y.abs() + z.y.abs()) * 0.85
    }

    /// The spin an upward kick of `kick` metres a second at what it lands on gives it: at
    /// the middle of its lowest corners, so a box landing flat takes none and one landing on
    /// a corner or an edge turns over it.
    fn tumble(&self, kick: f32) -> Vec3 {
        let half = self.size * 0.5;
        let corners = (0..8).map(|i| {
            let sign = Vec3::new(
                if i & 1 == 0 { -1.0 } else { 1.0 },
                if i & 2 == 0 { -1.0 } else { 1.0 },
                if i & 4 == 0 { -1.0 } else { 1.0 },
            );
            self.turn * (half * sign)
        });
        let lowest = corners.clone().map(|c| c.y).fold(f32::MAX, f32::min);
        let near = 0.05 * self.size.max_element();
        let (sum, count) = corners
            .filter(|c| c.y < lowest + near)
            .fold((Vec3::ZERO, 0.0), |(sum, n), c| (sum + c, n + 1.0));
        let r = sum / count;
        let inertia = self.size.length_squared() / 18.0;
        r.cross(Vec3::Y * kick) / inertia.max(1e-4) * TUMBLE
    }

    /// Moves it on by `dt` seconds. `floor` says the height of what is under a point, in
    /// metres, and `solid` whether a point is inside something built, which a fragment
    /// thrown into it glances off.
    pub fn step(
        &mut self,
        dt: f32,
        floor: &dyn Fn(Vec3) -> f32,
        solid: &dyn Fn(Vec3) -> bool,
    ) -> Landing {
        let mut landing = Landing::default();
        if self.still {
            return landing;
        }
        if self.wait > 0.0 {
            self.wait -= dt;
            return landing;
        }
        if let Some(mut body) = self.body {
            // With its section, as one rigid body, until it touches: then on its own, going
            // as the section was, to land as hard as that and break by it, while the rest
            // of the section comes on down onto it.
            body.age += dt;
            let (at, turn) = body.pose();
            self.at = at;
            self.turn = turn;
            self.velocity = body.velocity(at);
            let touched = self.at.y - self.reach_down() <= floor(self.at) || solid(self.at);
            if !touched && body.age < BODY_MOST_S {
                self.body = Some(body);
                return landing;
            }
            self.body = None;
            self.spin = body.spin;
        }
        if let Some(mut hinge) = self.hinge {
            let (at, turn) = hinge.step(dt);
            self.at = at;
            self.turn = turn;
            self.velocity = hinge.velocity(at);
            let grounded = self.at.y - self.reach_down() <= floor(self.at);
            if hinge.angle >= hinge.lets_go || grounded {
                // On its own now, flung on as it was going, still turning over as the whole
                // was, and as its crack threw it, so the blocks part rather than fly as one.
                self.hinge = None;
                let r = |salt: u32| unit(&[self.seed, salt]) - 0.5;
                let kick = Vec3::new(r(41), r(42), r(43)) * hinge.rate.max(0.3);
                self.spin = hinge.axis * hinge.rate + kick;
            } else {
                self.hinge = Some(hinge);
                return landing;
            }
        }
        self.velocity.y -= GRAVITY * dt;
        let moved = self.velocity.with_y(0.0) * dt;
        // Into a wall along the floor: off it, slowed.
        let ahead = self.at + moved;
        if moved != Vec3::ZERO && solid(ahead) && !solid(self.at) {
            let across = (ahead / CELL_M).floor() - (self.at / CELL_M).floor();
            if across.x != 0.0 {
                self.velocity.x *= -0.35;
            }
            if across.z != 0.0 {
                self.velocity.z *= -0.35;
            }
        }
        self.at += self.velocity * dt;
        self.turn = (Quat::from_scaled_axis(self.spin * dt) * self.turn).normalize();
        let under = floor(self.at);
        let down = self.reach_down();
        if self.at.y - down > under {
            return landing;
        }
        self.at.y = under + down;
        // On a slope steeper than it can rest on, a roof's, it slides down it and off its
        // eave rather than lying where it fell: read each way along the floor, so one on a
        // ridge slides off whichever side falls away most, not balanced between the two.
        let e = SLOPE_READ_M;
        let here = floor(self.at);
        let (way, steep) = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z]
            .into_iter()
            .map(|way| (way, (here - floor(self.at + way * e)) / e))
            .fold((Vec3::ZERO, 0.0f32), |best, (way, drop)| {
                if drop > best.1 { (way, drop) } else { best }
            });
        // Over an edge, an eave's, it goes on over it.
        let steep = steep.min(4.0);
        let sliding = steep > SLIDES_AT;
        if self.velocity.y < 0.0 {
            let hit = -self.velocity.y;
            landing.hit = Some(hit);
            landing.breaks = self.shape == Shape::Whole && hit > BREAKS_AT;
            // Following a slope down it, as it slides, is no fall onto it.
            let following = steep * self.velocity.with_y(0.0).length() + GRAVITY * dt * 2.0;
            if sliding && hit <= following {
                self.velocity.y = 0.0;
            } else {
                self.velocity.y = hit * BOUNCE;
                self.velocity.x *= FRICTION;
                self.velocity.z *= FRICTION;
                // Landing on a corner or an edge turns it over that corner (Δω = r × J / I):
                // a block tumbles off its heap rather than coming to rest as it fell.
                let tumble = self.tumble(hit * (1.0 + BOUNCE));
                // Rolling on along the floor rather than spinning in the air.
                let along = self.velocity.with_y(0.0);
                let roll = Vec3::Y.cross(along) / (down.max(0.03));
                self.spin = (self.spin * 0.4 + roll * 0.3 + tumble).clamp_length_max(MOST_SPIN);
            }
        }
        if sliding {
            let pull = GRAVITY * steep / (1.0 + steep * steep).sqrt();
            self.velocity += way * pull * (1.0 - SLIDE_FRICTION) * dt;
            return landing;
        }
        if self.velocity.length() < SETTLE {
            self.velocity = Vec3::ZERO;
            self.spin = Vec3::ZERO;
            self.still = true;
        }
        landing
    }

    /// The pieces a whole cube breaks into where it lands, as many as `share` of its way of
    /// breaking says, thrown up and out a little: what is left of its fall.
    pub fn shatter(&self, breaks: Breaks, share: f32) -> Vec<Fragment> {
        let mut out = Vec::new();
        // A chunk of cubes cracks where it lands hard into a few smaller chunks of uneven
        // size (`fracture::crack`), as masonry does, each tumbling on to break again where
        // it lands hard, with a spray of its way of breaking's pieces from the cracks; its
        // single cubes some whole, the rest at once into pieces.
        if self.size.max_element() > CELL_M * 1.2 {
            let count = (self.size / CELL_M).round().max(Vec3::ONE).as_ivec3();
            let cube = self.size / count.as_vec3();
            for (low, side) in fracture::crack(count, self.seed) {
                let part = cube * side.as_vec3();
                let offset = low.as_vec3() * cube + part * 0.5 - self.size * 0.5;
                let seed = hash::of(&[self.seed, (low.x | low.y << 4 | low.z << 8) as u32]);
                let r = |salt: u32| unit(&[seed, salt]);
                let at = self.at + self.turn * offset;
                let out_from = (self.turn * offset).normalize_or_zero();
                let whole = side != IVec3::ONE || r(7) < WHOLE_SHARE;
                if whole {
                    // The bigger, the less it is thrown: a big chunk rolls off the heap.
                    let heft = part.max_element().max(CELL_M).sqrt();
                    let away = self.velocity.with_y(0.0) * 0.5 + out_from * (0.6 + r(4)) / heft;
                    let axis = Vec3::new(r(1) - 0.5, r(2) - 0.5, r(3) - 0.5).normalize_or(Vec3::X);
                    out.push(Fragment {
                        at,
                        velocity: away + Vec3::Y * (0.4 + 0.8 * r(5)) / heft,
                        spin: axis * (2.0 + 3.0 * r(6)) / heft,
                        size: part * SHRINK,
                        wait: 0.0,
                        still: false,
                        seed,
                        hinge: None,
                        body: None,
                        ..*self
                    });
                }
                if !whole || side != IVec3::ONE {
                    // A cube's pieces, or from a crack a cube's worth at half.
                    let away = self.velocity.with_y(0.0) * 0.5 + out_from * (1.0 + r(4));
                    pieces(
                        self.kind,
                        breaks,
                        at - Vec3::splat(CELL_M * 0.5),
                        1.0,
                        seed,
                        if whole { share * 0.5 } else { share },
                        away,
                        0.7,
                        &mut out,
                    );
                }
            }
            return out;
        }
        pieces(
            self.kind,
            breaks,
            self.at - Vec3::splat(CELL_M * 0.5),
            self.fill,
            self.seed,
            share,
            self.velocity.with_y(0.0) * 0.5,
            0.7,
            &mut out,
        );
        out
    }
}

/// A number from 0 to 1 from some parts, the same everywhere.
fn unit(parts: &[u32]) -> f32 {
    (hash::of(parts) & 0xFFFF) as f32 / 65535.0
}

/// The pieces a cube with its low corner at `low` breaks into, `share` of its way's count,
/// each where in the cube it came from, thrown out from its middle, by `away` more and up by
/// `up` of its way's throw, tumbling.
#[allow(clippy::too_many_arguments)]
fn pieces(
    kind: Kind,
    breaks: Breaks,
    low: Vec3,
    fill: f32,
    seed: u32,
    share: f32,
    away: Vec3,
    up: f32,
    out: &mut Vec<Fragment>,
) {
    let look = breaks.look();
    let mut index = 0u32;
    for piece in look.pieces {
        let count = ((piece.count as f32 * share).round() as u32).max(u32::from(share > 0.0));
        for _ in 0..count {
            index += 1;
            let r = |salt: u32| unit(&[seed, index, salt]);
            let size = Vec3::new(
                piece.least[0] + (piece.most[0] - piece.least[0]) * r(1),
                piece.least[1] + (piece.most[1] - piece.least[1]) * r(2),
                piece.least[2] + (piece.most[2] - piece.least[2]) * r(3),
            );
            // Where in the cube it came from: in what it filled.
            let inside = Vec3::new(
                0.15 + 0.7 * r(4),
                (0.15 + 0.7 * r(5)) * fill,
                0.15 + 0.7 * r(6),
            );
            let from_middle = inside - Vec3::new(0.5, fill * 0.5, 0.5);
            let axis = Vec3::new(r(7) - 0.5, r(8) - 0.5, r(9) - 0.5).normalize_or(Vec3::Y);
            let velocity = (from_middle * 3.0 + Vec3::new(r(10) - 0.5, up + r(11), r(12) - 0.5))
                * look.throw
                + away;
            out.push(Fragment {
                kind,
                shape: piece.shape,
                at: low + inside * CELL_M,
                velocity,
                turn: Quat::from_scaled_axis(axis * r(13) * 3.0),
                spin: axis * look.spin * (0.4 + r(14)),
                size,
                wait: 0.0,
                still: false,
                fill: 1.0,
                seed: hash::of(&[seed, index]),
                hinge: None,
                body: None,
            });
        }
    }
}

/// What a fall comes to: the pieces of what was destroyed, bursting now, and the cubes left
/// hanging, whole, each waiting its turn to fall; within `most` fragments.
///
/// `destroyed` are the cubes harm broke, the rest of `fell` those left hanging; `breaks`
/// says how each kind breaks; `share` is how much of a full burst to make (less far off and
/// at a low preset).
pub fn burst(
    fell: &[(IVec3, Cube)],
    destroyed: &dyn Fn(IVec3) -> bool,
    breaks: &dyn Fn(Kind) -> Breaks,
    share: f32,
    most: usize,
) -> Vec<Fragment> {
    if fell.is_empty() || most == 0 {
        return Vec::new();
    }
    type Fell<'a> = Vec<&'a (IVec3, Cube)>;
    let (broken, hanging): (Fell, Fell) = fell.iter().partition(|(at, _)| destroyed(*at));
    // A roof left hanging is a skin, not a block: it sheds its covering where it hangs, a
    // little after what gave way, rather than falling as cubes.
    let (shed, hanging): (Fell, Fell) = hanging
        .into_iter()
        .partition(|entry| entry.1.kind.is_roof());
    // Where it was hit: the middle of what broke, else of all of it.
    let of = |cubes: &[&(IVec3, Cube)]| {
        cubes
            .iter()
            .map(|(at, _)| (at.as_vec3() + 0.5) * CELL_M)
            .sum::<Vec3>()
            / cubes.len().max(1) as f32
    };
    let impact = if broken.is_empty() {
        of(&hanging)
    } else {
        of(&broken)
    };
    // What broke gets the budget first, at most half of it; the hanging cubes the rest, one
    // fragment each, every so many if there are more than that.
    let per = broken
        .iter()
        .chain(&shed)
        .map(|(_, cube)| breaks(cube.kind).count() as usize)
        .max()
        .unwrap_or(1);
    let room = (most / 2).max(most.saturating_sub(hanging.len()));
    let share = share.min(room as f32 / ((broken.len() + shed.len()) * per).max(1) as f32);
    let mut out = Vec::new();
    for (at, cube) in &broken {
        let low = at.as_vec3() * CELL_M;
        let seed = hash::of(&[at.x as u32, at.y as u32, at.z as u32]);
        let away = (low + CELL_M * 0.5 - impact).normalize_or_zero() * 1.5;
        let fill = f32::from(cube.level()) / f32::from(FULL);
        pieces(
            cube.kind,
            breaks(cube.kind),
            low,
            fill,
            seed,
            share,
            away,
            1.2,
            &mut out,
        );
    }
    let lowest_shed = shed.iter().map(|(at, _)| at.y).min().unwrap_or(0);
    for (at, cube) in &shed {
        let low = at.as_vec3() * CELL_M;
        let seed = hash::of(&[at.x as u32, at.y as u32, at.z as u32]);
        let away = (low + CELL_M * 0.5 - impact)
            .with_y(0.0)
            .normalize_or_zero()
            * 0.4;
        let first = out.len();
        pieces(
            cube.kind,
            breaks(cube.kind),
            low,
            1.0,
            seed,
            share,
            away,
            0.2,
            &mut out,
        );
        let wait =
            ((at.y - lowest_shed) as f32 * STAGGER_S + 0.1 * unit(&[seed, 99])).min(STAGGER_MOST_S);
        for piece in &mut out[first..] {
            piece.wait = wait;
            piece.velocity *= 0.5;
        }
    }
    out.truncate(most);
    // The cubes left hanging fall whole, in chunks of uneven size where there are many
    // (`fracture`): a tower comes down as solid chunks of masonry, not a lattice of every
    // so many cubes nor a grid of equal blocks.
    let left = most - out.len();
    let lowest = hanging.iter().map(|(at, _)| at.y).min().unwrap_or(0);
    let fallen: std::collections::BTreeSet<[i32; 3]> =
        fell.iter().map(|(at, _)| at.to_array()).collect();
    let blocks = chunks_of(&hanging, left, breaks);
    let every = blocks.len().div_ceil(left.max(1)).max(1);
    for (index, block) in blocks.iter().enumerate() {
        if index % every != 0 || out.len() >= most {
            continue;
        }
        let (low, high) = block.iter().fold(
            (IVec3::splat(i32::MAX), IVec3::splat(i32::MIN)),
            |(low, high), (at, _)| (low.min(*at), high.max(*at)),
        );
        let (at, cube) = block[0];
        let seed = hash::of(&[at.x as u32, at.y as u32, at.z as u32]);
        let r = |salt: u32| unit(&[seed, salt]);
        // As full as its cubes are, a single cube part in standing on its foot.
        let fill = if block.len() == 1 {
            f32::from(cube.level()) / f32::from(FULL)
        } else {
            1.0
        };
        let cubes = (high - low + IVec3::ONE).as_vec3();
        let foot = low.as_vec3() * CELL_M;
        let size = Vec3::new(cubes.x, cubes.y * fill, cubes.z) * CELL_M;
        // Its grain, if it has one, along its first axis (`surfaces`): turned along the
        // wall it fell from, the way the block or the cubes beside it run.
        let beside = |step: IVec3| {
            block
                .iter()
                .filter(|(at, _)| {
                    fallen.contains(&(*at + step).to_array())
                        || fallen.contains(&(*at - step).to_array())
                })
                .count()
        };
        let along_z =
            cubes.z > cubes.x || (cubes.z == cubes.x && beside(IVec3::Z) > beside(IVec3::X));
        let (turn, size) = if along_z {
            (
                Quat::from_rotation_y(core::f32::consts::FRAC_PI_2),
                Vec3::new(size.z, size.y, size.x),
            )
        } else {
            (Quat::IDENTITY, size)
        };
        let middle = foot + size * 0.5;
        let away = (middle - impact).with_y(0.0).normalize_or_zero();
        let axis = Vec3::new(r(1) - 0.5, 0.2 * r(2), r(3) - 0.5).normalize_or(Vec3::X);
        out.push(Fragment {
            kind: cube.kind,
            shape: Shape::Whole,
            at: middle,
            // The higher, the further it topples out from where it gave way.
            velocity: away * (0.3 + 0.6 * r(4) + (0.07 * (low.y - lowest) as f32).min(1.5)),
            turn,
            spin: axis * (0.6 + 1.6 * r(5)) / cubes.max_element().sqrt(),
            // A hair smaller than its cubes, never fighting those still drawn ([`SHRINK`]).
            size: size * SHRINK,
            wait: ((low.y - lowest) as f32 * STAGGER_S + 0.08 * r(6)).min(STAGGER_MOST_S),
            still: false,
            fill,
            seed,
            hinge: None,
            body: None,
        });
    }
    out
}

/// The cubes left hanging cut into the chunks they fall as (`fracture::chunks`), each way of
/// breaking at its own size ([`Look::chunk`]) once the fall is big, bigger where there would
/// be more than `left`, in order up the fall, so that past the budget it is every so many,
/// evenly.
fn chunks_of<'a>(
    hanging: &[&'a (IVec3, Cube)],
    left: usize,
    breaks: &dyn Fn(Kind) -> Breaks,
) -> Vec<Vec<&'a (IVec3, Cube)>> {
    let lowest = hanging.iter().map(|(at, _)| at.y).min().unwrap_or(0);
    let seed = hash::of(&[hanging.len() as u32, lowest as u32]);
    let mut ways: Vec<(Breaks, Vec<&'a (IVec3, Cube)>)> = Vec::new();
    for &entry in hanging {
        let way = breaks(entry.1.kind);
        match ways.iter_mut().find(|(of, _)| *of == way) {
            Some((_, cubes)) => cubes.push(entry),
            None => ways.push((way, vec![entry])),
        }
    }
    let cut = |cubes: &[&'a (IVec3, Cube)], across: i32| -> Vec<Vec<&'a (IVec3, Cube)>> {
        // A small fall cube by cube.
        let below = if across == 1 { 2 } else { i32::MAX };
        let places: Vec<IVec3> = cubes.iter().map(|(at, _)| *at).collect();
        fracture::chunks(&places, across, below, seed)
            .into_iter()
            .map(|chunk| chunk.into_iter().map(|i| cubes[i]).collect())
            .collect()
    };
    let small = hanging.len() < CHUNKS_FROM;
    let mut more = match hanging.len() {
        0..BIG_CHUNKS_FROM => 0,
        BIG_CHUNKS_FROM..HUGE_CHUNKS_FROM => 1,
        _ => 2,
    };
    let mut blocks = Vec::new();
    loop {
        blocks.clear();
        for (way, cubes) in &ways {
            let least = if small { 1 } else { way.look().chunk };
            blocks.extend(cut(cubes, (least + more).min(MOST_ACROSS)));
        }
        if blocks.len() <= left || more >= MOST_ACROSS {
            break;
        }
        more += 1;
    }
    blocks.sort_by_key(|block| {
        let at = block[0].0;
        (at.y, at.x, at.z)
    });
    blocks
}

/// How many of the cubes a block bursts into where it first lands stay whole, to break
/// where they land again (`Fragment::shatter`).
const WHOLE_SHARE: f32 = 0.4;

/// The most the least side of the chunks cubes left hanging fall as may grow to, to stay in
/// the budget; from how many cubes left hanging they fall in chunks of their way of
/// breaking's size, and from how many in chunks one and two cubes bigger than that: a big
/// fall comes down in a few big chunks, which tip and tumble and break again where they land.
const MOST_ACROSS: i32 = 6;
const CHUNKS_FROM: usize = 300;
const BIG_CHUNKS_FROM: usize = 2000;
const HUGE_CHUNKS_FROM: usize = 4000;

/// The material a kind's fragments are made of: its walls' own; for timber, the wood its
/// members split to, squared, rather than the bark that wraps them.
pub fn material(kit: &Kit) -> Material {
    match kit.pattern {
        Pattern::Logs if kit.textures.contains_key(&Material::Hewn) => Material::Hewn,
        Pattern::Logs => Material::Log,
        Pattern::Masonry => Material::Masonry,
        Pattern::Roof if kit.textures.contains_key(&Material::Covering) => Material::Covering,
        Pattern::Roof => Material::Board,
    }
}

/// How big a piece usually is, in metres: the size its mesh wears its texture at.
const PIECE_M: f32 = 0.25;
/// How much darker a fragment is drawn than its material's face: broken stone in the dirt
/// and shadow of its fall, not the dressed face of a wall.
/// Whole blocks are the wall's own face moving, and keep its shade: darkened, a wall read as
/// another stone the moment it broke (the castle film's review, 2026-10-21).
const FRAGMENT_SHADE: f32 = 0.72;
/// A whole block's beds, top and bottom, and its broken arrises are where it split from the
/// courses round it: broken stone, as a standing wall's break is drawn
/// (`sections::laying::worn_within`), its sides the wall's face. A tower's blocks lit from
/// above in their face's colour read as a grid of flat peach slabs (castle film round 4).
fn split_shade(shape: Shape, normal: Vec3, colour: [f32; 3]) -> [f32; 3] {
    let split =
        shape == Shape::Whole && (normal.y.abs() > 0.7 || normal.abs().max_element() < 0.95);
    if split {
        colour.map(|c| c * BED_SHADE)
    } else {
        colour
    }
}

/// How dark a whole block's beds and broken edges are drawn beside its face: as a standing
/// cube beside a break, worn two quarters (`mesh::wear`'s darkening at half lost).
const BED_SHADE: f32 = 0.72;
/// How much smaller than its cubes a whole block is drawn, so it never fights the cubes drawn
/// where they were for the frames before their sections are drawn again; no more, as gaps of
/// 0.97 read as a lattice of dark seams while the blocks fell together.
const SHRINK: f32 = 0.99;

/// A shape of fragment, a metre across, in a kind's material: a box with its twelve edges
/// bevelled and its eight corners cut, which the game scales to each piece; its texture as it
/// would be on one `drawn_m` across ([`Fragment::mesh`]).
///
/// Timber's is a piece of the log it came from, its grain along its own first axis (the
/// game turns it along the wall it fell from, `Fragment::turn`): its ends across the grain
/// the log's sawn end, rings and all (`LogEnd`); its two sides the face of the log, bark
/// and weathered wood; its top and bottom, and its bevels, where it was split from the rest,
/// fresh pale wood (the face's worn set, `Texture::worn`), its grain running along it.
pub fn surfaces(kit: &Kit, shape: Shape, drawn_m: f32) -> Surfaces {
    let material = material(kit);
    let metres = kit
        .textures
        .get(&material)
        .map_or([1.0, 1.0], |texture| texture.metres)
        .map(|m| m / drawn_m.max(0.05));
    let timber = kit.pattern == Pattern::Logs && kit.textures.contains_key(&Material::LogEnd);
    let colour = if material == Material::Masonry {
        kit.tint
    } else {
        [1.0; 3]
    }
    .map(|channel| {
        channel
            * if timber {
                TIMBER_SHADE
            } else if shape == Shape::Whole {
                1.0
            } else {
                FRAGMENT_SHADE
            }
    });
    let end = crate::timber::end_cell(kit, 0x51ce);
    let mut surfaces = Surfaces::sharing();
    let mut wood = Surfaces::sharing();
    let half = 0.5;
    let b = half * shape.bevel();
    let inner = half - b;
    // Every corner of a bevelled box is a point of the cube's corner pulled in along two
    // axes; each face, edge and corner a flat piece through such points.
    let point = |sign: Vec3, out: [bool; 3]| -> Vec3 {
        Vec3::new(
            sign.x * if out[0] { half } else { inner },
            sign.y * if out[1] { half } else { inner },
            sign.z * if out[2] { half } else { inner },
        )
    };
    let mut add = |corners: &[Vec3]| {
        let middle = corners.iter().copied().sum::<Vec3>() / corners.len() as f32;
        for i in 1..corners.len() - 1 {
            let (mut a, mut c) = (corners[0], corners[i + 1]);
            let bb = corners[i];
            let mut normal = (bb - a)
                .cross(c - a)
                .normalize_or(middle.normalize_or(Vec3::Y));
            if normal.dot(middle) < 0.0 {
                std::mem::swap(&mut a, &mut c);
                normal = -normal;
            }
            let triangle = [a, bb, c];
            if !timber {
                let uvs = triangle.map(|p| crate::mesh::planar(p, normal, metres));
                let colour = split_shade(shape, normal, colour);
                surfaces
                    .of(material)
                    .textured(triangle, [normal; 3], uvs, colour);
                continue;
            }
            if normal.x.abs() > 0.9 {
                // Its sawn end: the end's picture over it, one of the kit's.
                let (corner, size) = end;
                let uvs = triangle.map(|p| {
                    [
                        corner[0] + size * (0.5 + p.z * 0.94),
                        corner[1] + size * (0.5 - p.y * 0.94),
                    ]
                });
                surfaces
                    .of(Material::LogEnd)
                    .textured(triangle, [normal; 3], uvs, colour);
                continue;
            }
            // Along the grain: across it by whichever way the face looks least, along it by
            // its first axis.
            let across = if normal.y.abs() > normal.z.abs() {
                2
            } else {
                1
            };
            let uvs = triangle.map(|p| [p[across] / metres[0], p.x / metres[1]]);
            let side = normal.z.abs() > 0.9;
            let (wear, shade) = if side { (0.25, 1.0) } else { (1.0, FRESH_WOOD) };
            let surface = wood.of(material);
            surface.textured(
                triangle,
                [normal; 3],
                uvs,
                colour.map(|c| (c * shade).min(1.0)),
            );
            crate::mesh::worn(&mut surface.wear, surface.positions.len() - 3);
            surface.wear.extend([wear; 3]);
        }
    };
    let signs = [-1.0f32, 1.0];
    // Faces.
    for axis in 0..3 {
        for s in signs {
            let mut quad = Vec::new();
            for (a, b2) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let mut sign = Vec3::ZERO;
                sign[axis] = s;
                sign[(axis + 1) % 3] = a;
                sign[(axis + 2) % 3] = b2;
                let mut out = [false; 3];
                out[axis] = true;
                quad.push(point(sign, out));
            }
            add(&quad);
        }
    }
    // Edges: along each axis, at each of its four corners round it.
    for axis in 0..3 {
        let (p, q) = ((axis + 1) % 3, (axis + 2) % 3);
        for sp in signs {
            for sq in signs {
                let mut quad = Vec::new();
                for (along, outer) in [(-1.0, p), (1.0, p), (1.0, q), (-1.0, q)] {
                    let mut sign = Vec3::ZERO;
                    sign[axis] = along;
                    sign[p] = sp;
                    sign[q] = sq;
                    let mut out = [false; 3];
                    out[outer] = true;
                    quad.push(point(sign, out));
                }
                add(&quad);
            }
        }
    }
    // Corners.
    for sx in signs {
        for sy in signs {
            for sz in signs {
                let sign = Vec3::new(sx, sy, sz);
                add(&[0, 1, 2].map(|axis| {
                    let mut out = [false; 3];
                    out[axis] = true;
                    point(sign, out)
                }));
            }
        }
    }
    for (material, surface) in wood.0 {
        surfaces.of(material).append(surface);
    }
    surfaces
}

/// How much darker timber's fragments are drawn than its face, less than stone's: split wood
/// is pale, and its fresh faces paler again than its weathered ones.
const TIMBER_SHADE: f32 = 0.9;
const FRESH_WOOD: f32 = 1.2;

#[cfg(test)]
mod tests;
