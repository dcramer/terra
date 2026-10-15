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

use crate::cubes::{Cube, FULL, Kind};
use crate::kit::{Kit, Pattern};
use crate::layout::hash;
use crate::{Base, CELL_M, Material, Surfaces};
use glam::{IVec3, Quat, Vec3};

/// The most fragments a fall makes, and the most about at once, at the highest preset: the
/// game makes fewer at lower ones and fades the oldest first past it.
pub const MOST: usize = 2000;
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
/// above it, in seconds, and at most.
const STAGGER_S: f32 = 0.035;
const STAGGER_MOST_S: f32 = 0.6;

/// How a kind breaks (D305): data on its kit, by its pattern unless its material says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Breaks {
    /// Chunky irregular blocks: stone, brick.
    #[default]
    Chunks,
    /// Lengths and splinters: logs, timber.
    Lengths,
    /// Small flat bits: wattle and plaster, a roof's covering.
    Bits,
    /// Crumbling lumps: packed earth.
    Crumbs,
}

/// The shapes a piece is drawn as: a box with its edges bevelled, more for a rougher one.
/// Its size and proportions are the piece's own, so a few meshes draw every piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Shape {
    /// A whole cube falling before it breaks: barely bevelled.
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
            Shape::Whole => 0.05,
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
/// dust it puts up, from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub pieces: &'static [Piece],
    pub throw: f32,
    pub spin: f32,
    pub dust: f32,
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

impl Breaks {
    /// How it looks: the table every material breaking this way shares.
    pub fn look(self) -> Look {
        match self {
            Breaks::Chunks => Look {
                pieces: &CHUNKS,
                throw: 1.0,
                spin: 6.0,
                dust: 1.0,
            },
            Breaks::Lengths => Look {
                pieces: &LENGTHS,
                throw: 0.8,
                spin: 8.0,
                dust: 0.45,
            },
            Breaks::Bits => Look {
                pieces: &BITS,
                throw: 1.2,
                spin: 10.0,
                dust: 0.8,
            },
            Breaks::Crumbs => Look {
                pieces: &CRUMBS,
                throw: 0.6,
                spin: 4.0,
                dust: 1.4,
            },
        }
    }

    /// How many pieces a cube breaks into, all told.
    pub fn count(self) -> u32 {
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
                let across = (self.size.max_element() / CELL_M).round().clamp(1.0, 4.0);
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
        if self.velocity.y < 0.0 {
            let hit = -self.velocity.y;
            landing.hit = Some(hit);
            landing.breaks = self.shape == Shape::Whole && hit > BREAKS_AT;
            self.velocity.y = hit * BOUNCE;
            self.velocity.x *= FRICTION;
            self.velocity.z *= FRICTION;
            // Rolling on along the floor rather than spinning in the air.
            let along = self.velocity.with_y(0.0);
            let roll = Vec3::Y.cross(along) / (down.max(0.03));
            self.spin = self.spin * 0.4 + roll * 0.6;
        }
        // On a slope steeper than it can rest on, a roof's, it slides down it and off its
        // eave rather than lying where it fell.
        let e = SLOPE_READ_M;
        let slope = Vec3::new(
            (floor(self.at + Vec3::X * e) - floor(self.at - Vec3::X * e)) / (2.0 * e),
            0.0,
            (floor(self.at + Vec3::Z * e) - floor(self.at - Vec3::Z * e)) / (2.0 * e),
        );
        let steep = slope.length();
        if steep > SLIDES_AT && steep < 20.0 {
            let down = -slope / steep;
            let pull = GRAVITY * steep / (1.0 + steep * steep).sqrt();
            self.velocity += down * pull * (1.0 - SLIDE_FRICTION) * dt;
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
        // A block of cubes bursts where it first lands into the cubes it was made of, as
        // voxels do: some of them whole, to tumble on and break where they land again, the
        // rest at once into its way of breaking's pieces.
        if self.size.max_element() > CELL_M * 1.2 {
            let count = (self.size / CELL_M).round().max(Vec3::ONE).as_ivec3();
            let part = self.size / count.as_vec3();
            for x in 0..count.x {
                for y in 0..count.y {
                    for z in 0..count.z {
                        let offset = (Vec3::new(x as f32, y as f32, z as f32) + 0.5) * part
                            - self.size * 0.5;
                        let seed = hash(&[self.seed, (x | y << 3 | z << 6) as u32]);
                        let r = |salt: u32| unit(&[seed, salt]);
                        let at = self.at + self.turn * offset;
                        let out_from = (self.turn * offset).normalize_or_zero();
                        let away = self.velocity.with_y(0.0) * 0.5 + out_from * (1.0 + r(4));
                        if r(7) < WHOLE_SHARE {
                            let axis =
                                Vec3::new(r(1) - 0.5, r(2) - 0.5, r(3) - 0.5).normalize_or(Vec3::X);
                            out.push(Fragment {
                                at,
                                velocity: away + Vec3::Y * (0.8 + r(5)),
                                spin: axis * (2.0 + 3.0 * r(6)),
                                size: part * 0.97,
                                wait: 0.0,
                                still: false,
                                seed,
                                ..*self
                            });
                        } else {
                            pieces(
                                self.kind,
                                breaks,
                                at - Vec3::splat(CELL_M * 0.5),
                                1.0,
                                seed,
                                share,
                                away,
                                0.7,
                                &mut out,
                            );
                        }
                    }
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
    (hash(parts) & 0xFFFF) as f32 / 65535.0
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
                seed: hash(&[seed, index]),
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
        let seed = hash(&[at.x as u32, at.y as u32, at.z as u32]);
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
        let seed = hash(&[at.x as u32, at.y as u32, at.z as u32]);
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
    // The cubes left hanging fall whole, a block of them together where there are more
    // than there is room for: a tower comes down as chunks a metre or two across, solid,
    // rather than as a lattice of every so many cubes.
    let left = most - out.len();
    let lowest = hanging.iter().map(|(at, _)| at.y).min().unwrap_or(0);
    let fallen: std::collections::BTreeSet<[i32; 3]> =
        fell.iter().map(|(at, _)| at.to_array()).collect();
    let mut across = 1;
    let mut blocks = group(&hanging, across);
    while blocks.len() > left && across < MOST_ACROSS {
        across += 1;
        blocks = group(&hanging, across);
    }
    let every = blocks.len().div_ceil(left.max(1)).max(1);
    for (index, block) in blocks.values().enumerate() {
        if index % every != 0 || out.len() >= most {
            continue;
        }
        let (low, high) = block.iter().fold(
            (IVec3::splat(i32::MAX), IVec3::splat(i32::MIN)),
            |(low, high), (at, _)| (low.min(*at), high.max(*at)),
        );
        let (at, cube) = block[0];
        let seed = hash(&[at.x as u32, at.y as u32, at.z as u32]);
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
            // A hair smaller than its cubes, so it never fights the cubes drawn where they
            // were for the frames before their sections are drawn again.
            size: size * 0.97,
            wait: ((low.y - lowest) as f32 * STAGGER_S + 0.08 * r(6)).min(STAGGER_MOST_S),
            still: false,
            fill,
            seed,
        });
    }
    out
}

/// How many of the cubes a block bursts into where it first lands stay whole, to break
/// where they land again (`Fragment::shatter`).
const WHOLE_SHARE: f32 = 0.4;

/// The most cubes along each side a block of cubes left hanging falls as one.
const MOST_ACROSS: i32 = 4;

/// Cubes grouped in blocks so many a side, by the block each is in.
fn group<'a>(
    cubes: &[&'a (IVec3, Cube)],
    across: i32,
) -> std::collections::BTreeMap<[i32; 3], Vec<&'a (IVec3, Cube)>> {
    let mut blocks: std::collections::BTreeMap<[i32; 3], Vec<&(IVec3, Cube)>> =
        std::collections::BTreeMap::new();
    for cube in cubes {
        let key = cube.0.div_euclid(IVec3::splat(across)).to_array();
        blocks.entry(key).or_default().push(*cube);
    }
    blocks
}

impl Base {
    /// The height of the top of whatever is built under a point, in metres in the base's own
    /// space: the highest cube with something in it at or below it in its column, by its
    /// shape and how far in it is, within a storey and a half; none if nothing is.
    pub fn top_under(&self, at: Vec3) -> Option<f32> {
        let cell = (at / CELL_M).floor().as_ivec3();
        let (u, w) = (at.x / CELL_M - cell.x as f32, at.z / CELL_M - cell.z as f32);
        for y in (cell.y - 9..=cell.y).rev() {
            let place = IVec3::new(cell.x, y, cell.z);
            let Some(cube) = self.cube(place).filter(Cube::is_in) else {
                continue;
            };
            let fill = f32::from(cube.level()) / f32::from(FULL);
            let top = cube.shape.top(u, w).min(fill);
            return Some((y as f32 + top) * CELL_M);
        }
        None
    }

    /// Whether a point is inside a cube with something in it, under its top.
    pub fn solid_at(&self, at: Vec3) -> bool {
        let cell = (at / CELL_M).floor().as_ivec3();
        let Some(cube) = self.cube(cell).filter(Cube::is_in) else {
            return false;
        };
        let (u, v, w) = (
            at.x / CELL_M - cell.x as f32,
            at.y / CELL_M - cell.y as f32,
            at.z / CELL_M - cell.z as f32,
        );
        v <= cube.shape.top(u, w) && v >= cube.shape.bottom(u, w)
    }
}

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
const FRAGMENT_SHADE: f32 = 0.72;

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
    .map(|channel| channel * if timber { TIMBER_SHADE } else { FRAGMENT_SHADE });
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
mod tests {
    use super::*;
    use crate::{Fill, Shape as CubeShape};

    fn cube(shape: CubeShape) -> Cube {
        Cube {
            shape,
            ..Cube::new(Kind(2), Fill::Solid)
        }
    }

    fn chunks(_: Kind) -> Breaks {
        Breaks::Chunks
    }

    fn flat(_: Vec3) -> f32 {
        0.0
    }

    fn open(_: Vec3) -> bool {
        false
    }

    #[test]
    fn what_broke_bursts_and_what_hung_falls_whole_later_the_higher_it_was() {
        let wall: Vec<(IVec3, Cube)> = (0..4)
            .flat_map(|y| (0..3).map(move |x| (IVec3::new(x, y, 0), cube(CubeShape::Whole))))
            .collect();
        let foot = |at: IVec3| at.y == 0;
        let parts = burst(&wall, &foot, &chunks, 1.0, 2000);
        let (broken, whole): (Vec<&Fragment>, Vec<&Fragment>) =
            parts.iter().partition(|part| part.shape != Shape::Whole);
        assert_eq!(broken.len(), 3 * Breaks::Chunks.count() as usize);
        assert_eq!(whole.len(), 9);
        assert!(broken.iter().all(|part| part.wait == 0.0));
        let wait = |y: f32| {
            whole
                .iter()
                .filter(|part| (part.at.y - y).abs() < 0.01)
                .map(|part| part.wait)
                .fold(0.0, f32::max)
        };
        assert!(wait(1.75) > wait(0.75), "the top falls after what gave way");
        assert_eq!(
            parts,
            burst(&wall, &foot, &chunks, 1.0, 2000),
            "the same every time"
        );
    }

    #[test]
    fn a_tower_coming_down_stays_within_the_budget_and_falls_solid() {
        let tower: Vec<(IVec3, Cube)> = (0..20)
            .flat_map(|y| (0..15).flat_map(move |x| (0..15).map(move |z| IVec3::new(x, y, z))))
            .map(|at| (at, cube(CubeShape::Whole)))
            .collect();
        let ring = |at: IVec3| at.y == 0;
        let parts = burst(&tower, &ring, &chunks, 1.0, 2000);
        assert!(parts.len() <= 2000, "{}", parts.len());
        let high = parts.iter().map(|part| part.at.y).fold(0.0, f32::max);
        assert!(high > 9.0, "its top comes down too: {high}");
        // What hung comes down in blocks that between them fill all of it, not as a
        // lattice of every so many cubes.
        let whole: f32 = parts
            .iter()
            .filter(|part| part.shape == Shape::Whole)
            .map(|part| part.size.x * part.size.y * part.size.z)
            .sum();
        let hung = (19 * 15 * 15) as f32 * CELL_M * CELL_M * CELL_M;
        assert!(whole > hung * 0.88, "{whole} of {hung}");
    }

    #[test]
    fn a_block_landing_bursts_into_its_cubes_some_whole_and_the_rest_into_pieces() {
        // A block two metres a side, four cubes, falling as one.
        let block: Vec<(IVec3, Cube)> = (0..4)
            .flat_map(|y| (0..4).flat_map(move |x| (0..4).map(move |z| IVec3::new(x, 12 + y, z))))
            .map(|at| (at, cube(CubeShape::Whole)))
            .collect();
        let parts = burst(&block, &|_| false, &chunks, 1.0, 2);
        assert_eq!(parts.len(), 1, "four to a side, as one");
        let burst = parts[0].shatter(Breaks::Chunks, 1.0);
        // Nothing left bigger than a cube where it first lands.
        assert!(burst.iter().all(|part| part.size.max_element() < CELL_M));
        let whole: Vec<_> = burst
            .iter()
            .filter(|part| part.shape == Shape::Whole)
            .collect();
        assert!(
            whole.len() > 10 && whole.len() < 54,
            "{} whole of 64",
            whole.len()
        );
        assert!(burst.len() > 64 * 2, "the rest in pieces: {}", burst.len());
        let pieces = whole[0].shatter(Breaks::Chunks, 1.0);
        assert!(pieces.iter().all(|piece| piece.shape != Shape::Whole));
    }

    #[test]
    fn a_timber_fragment_shows_end_grain_across_its_grain_bark_on_its_sides_and_fresh_wood_where_split()
     {
        let mut kit = Kit::default();
        for (material, path) in [(Material::Log, "log"), (Material::LogEnd, "ends")] {
            kit.textures.insert(
                material,
                crate::kit::Texture {
                    path: path.into(),
                    metres: [1.0, 1.0],
                    depth: 0.0,
                    reflectance: crate::kit::matte(),
                    strips: None,
                    cells: 1,
                    worn: Some("wood".into()),
                },
            );
        }
        let made = surfaces(&kit, Shape::Whole, CELL_M);
        let ends = &made.0[&Material::LogEnd];
        // Its ends, across its grain, its first axis: the sawn end, all of its picture.
        assert!(ends.normals.iter().all(|n| n.x.abs() > 0.9));
        assert_eq!(ends.normals.iter().filter(|n| n.x > 0.0).count(), 6);
        let (low, high) = ends
            .uvs
            .iter()
            .fold(([1.0f32; 2], [0.0f32; 2]), |(l, h), uv| {
                (
                    [l[0].min(uv[0]), l[1].min(uv[1])],
                    [h[0].max(uv[0]), h[1].max(uv[1])],
                )
            });
        assert!(
            low[0] > 0.0 && high[0] < 1.0 && high[0] - low[0] > 0.8,
            "{low:?} {high:?}"
        );
        // Along it, its texture runs with the grain: its second texture axis along the first.
        let face = &made.0[&Material::Log];
        for (corners, uvs) in face.positions.chunks(3).zip(face.uvs.chunks(3)) {
            for (p, uv) in corners.iter().zip(uvs) {
                assert!((uv[1] - p.x * CELL_M).abs() < 1e-4, "{p} at {uv:?}");
            }
        }
        // Its sides bark, a little weathered; its top and bottom, split, bare fresh wood.
        for ((n, wear), colour) in face.normals.iter().zip(&face.wear).zip(&face.colours) {
            if n.z.abs() > 0.9 {
                assert!(*wear < 0.5);
            } else if n.y.abs() > 0.9 {
                assert!(*wear > 0.9 && colour[0] > TIMBER_SHADE);
            }
        }
    }

    #[test]
    fn a_chip_landing_on_a_roof_slides_down_it_and_off_the_eave() {
        // A roof pitched at 35 degrees down to its eave at x = 2 m, 1.6 m up; the ground
        // past it.
        let roof = |p: Vec3| if p.x < 2.0 { 3.0 - p.x * 0.7 } else { 0.0 };
        let fell = [(IVec3::new(1, 8, 0), cube(CubeShape::Whole))];
        let mut chip = burst(&fell, &|_| true, &chunks, 1.0, 100)[0];
        chip.at = Vec3::new(0.5, 4.0, 0.25);
        chip.velocity = Vec3::ZERO;
        for _ in 0..1200 {
            chip.step(1.0 / 60.0, &roof, &open);
        }
        assert!(chip.still, "it comes to rest");
        assert!(
            chip.at.x > 2.0 && chip.at.y < 0.4,
            "on the ground past the eave: {}",
            chip.at
        );
        // On level ground it lies where it fell.
        let mut lying = burst(&fell, &|_| true, &chunks, 1.0, 100)[0];
        lying.at = Vec3::new(0.5, 1.0, 0.25);
        lying.velocity = Vec3::ZERO;
        for _ in 0..600 {
            lying.step(1.0 / 60.0, &flat, &open);
        }
        assert!(
            lying.still && (lying.at.x - 0.5).abs() < 0.3,
            "{}",
            lying.at
        );
    }

    #[test]
    fn a_whole_cube_breaks_where_it_lands_hard_and_its_pieces_settle() {
        let fell = [(IVec3::new(0, 8, 0), cube(CubeShape::Whole))];
        let mut whole = burst(&fell, &|_| false, &chunks, 1.0, 100)[0];
        let mut broke = None;
        for _ in 0..600 {
            let landing = whole.step(1.0 / 60.0, &flat, &open);
            if landing.breaks {
                broke = Some(whole.shatter(Breaks::Chunks, 1.0));
                break;
            }
        }
        let mut pieces = broke.expect("it breaks where it lands");
        assert_eq!(pieces.len(), Breaks::Chunks.count() as usize);
        for _ in 0..900 {
            for piece in &mut pieces {
                piece.step(1.0 / 60.0, &flat, &open);
                assert!(piece.at.y + 1e-3 >= piece.reach_down(), "through the floor");
            }
        }
        assert!(pieces.iter().all(|piece| piece.still), "they come to rest");
        assert!(pieces.iter().all(|piece| piece.at.y < 0.4), "on the ground");
    }

    #[test]
    fn a_piece_rolls_off_a_ledge_and_glances_off_a_wall() {
        let ledge = |at: Vec3| if at.x < 1.0 { 1.0 } else { 0.0 };
        let mut piece = burst(
            &[(IVec3::new(1, 2, 0), cube(CubeShape::Whole))],
            &|_| true,
            &chunks,
            1.0,
            100,
        )[0];
        piece.at = Vec3::new(0.9, 1.1, 0.25);
        piece.velocity = Vec3::new(1.5, 0.0, 0.0);
        for _ in 0..300 {
            piece.step(1.0 / 60.0, &ledge, &open);
        }
        assert!(
            piece.at.x > 1.0 && piece.at.y < 0.4,
            "off the ledge: {piece:?}"
        );
        let wall = |at: Vec3| at.x > 2.0;
        piece.at = Vec3::new(1.8, 0.5, 0.25);
        piece.velocity = Vec3::new(3.0, 0.5, 0.0);
        piece.still = false;
        for _ in 0..60 {
            piece.step(1.0 / 60.0, &flat, &wall);
        }
        assert!(piece.at.x < 2.0, "it glanced off: {}", piece.at);
    }

    #[test]
    fn a_roof_left_hanging_sheds_its_covering_rather_than_falling_as_cubes() {
        let roof: Vec<(IVec3, Cube)> = (0..3)
            .map(|x| {
                (
                    IVec3::new(x, 6 + x, 0),
                    Cube::new(Kind::FIRST_ROOF, crate::Fill::Solid),
                )
            })
            .collect();
        let parts = burst(&roof, &|_| false, &|_| Breaks::Bits, 1.0, 2000);
        assert!(!parts.is_empty());
        assert!(parts.iter().all(|part| part.shape != Shape::Whole));
        let latest = parts.iter().map(|part| part.wait).fold(0.0, f32::max);
        assert!(latest > 0.0, "the higher a moment later");
    }

    #[test]
    fn each_way_of_breaking_is_its_own_table() {
        let sizes = |breaks: Breaks| breaks.look().pieces[0].most;
        // Logs break into lengths, longer than they are thick; stone into chunks.
        let length = sizes(Breaks::Lengths);
        assert!(length[0] > 2.0 * length[1]);
        let chunk = sizes(Breaks::Chunks);
        assert!(chunk[0] < 2.0 * chunk[1]);
        assert!(Breaks::Crumbs.look().dust > Breaks::Lengths.look().dust);
    }

    #[test]
    fn what_is_built_under_a_point_is_found_by_its_shape() {
        let mut base = Base::default();
        base.apply(crate::BuildOp::Fill {
            min: [0, 0, 0],
            max: [0, 2, 0],
            cube: Some(Cube::new(Kind(2), Fill::Solid)),
        });
        base.apply(crate::BuildOp::Fill {
            min: [1, 0, 0],
            max: [1, 0, 0],
            cube: Some(cube(CubeShape::Half)),
        });
        assert_eq!(base.top_under(Vec3::new(0.25, 4.0, 0.25)), Some(1.5));
        assert_eq!(base.top_under(Vec3::new(0.75, 1.0, 0.25)), Some(0.25));
        assert_eq!(base.top_under(Vec3::new(3.0, 1.0, 0.25)), None);
        assert!(base.solid_at(Vec3::new(0.25, 0.6, 0.25)));
        assert!(!base.solid_at(Vec3::new(0.75, 0.4, 0.25)));
    }

    #[test]
    fn a_fragment_is_a_closed_bevelled_box_of_its_kinds_material() {
        let stone = Kit {
            id: 2,
            pattern: Pattern::Masonry,
            ..Kit::default()
        };
        for shape in Shape::ALL {
            let drawn = surfaces(&stone, shape, 0.5);
            let surface = &drawn.0[&Material::Masonry];
            // Six faces and twelve edges of two triangles, eight corners of one.
            assert_eq!(surface.triangle_count(), 6 * 2 + 12 * 2 + 8);
            for triangle in surface.positions.chunks(3) {
                let normal = (triangle[1] - triangle[0]).cross(triangle[2] - triangle[0]);
                let middle = (triangle[0] + triangle[1] + triangle[2]) / 3.0;
                assert!(normal.dot(middle) > 0.0, "{shape:?} faces in");
                assert!(triangle.iter().all(|p| p.abs().max_element() <= 0.5 + 1e-5));
            }
        }
        assert_eq!(material(&Kit::default()), Material::Log);
    }
}
