//! Block kinds drawn as their cubes' own surface (D197, research/block-modules.md): stone,
//! plaster and earth are not laid as models but as the outer faces of their cubes,
//! each a fine grid, rounded along every outside edge and displaced by their material's
//! height, so a wall of any shape, carved anywhere, is one surface with no seams.
//!
//! **Rounding.** Each cube is shrunk by the bevel on every side where nothing hides it,
//! and a point of the surface is moved onto the bevel's radius from the nearest point of
//! the nearest such shrunk box, over all the cubes round it: flat in the middle of a face,
//! a quarter-round along an outside edge, a rounded corner where three meet. Sides against
//! a neighbour are not shrunk, so faces of cubes side by side run on as one, and inside
//! corners stay sharp, as real ones are; the cube in an inside corner has the edge there
//! bitten out of its box a bevel round, so where two outside edges meet an inside one, a
//! window's jamb and sill at the wall's face, the three blend (the cubes opened by a ball). Taking the nearest over every cube round a point,
//! not only the face's own, makes it the same for every face that shares the point.
//!
//! **Relief.** The material's height, read where the texture is drawn (the same planar
//! mapping as its colour), pushes each point out along its rounded normal. Along an
//! outside edge the three axes' heights are blended by that normal, so the faces either
//! side of it meet. Near an inside edge, one with three of the four cubes round it full,
//! the relief fades out, as the faces there push different ways; how near is measured to
//! the edge itself, so every face at a point agrees.

use crate::mesh::{QuickMap, planar};
use crate::{CELL_M, Surface};
use glam::{IVec3, Vec3};
use std::collections::BTreeSet;
use std::hash::Hasher;
use std::sync::Arc;

mod slots;
pub use slots::draw_slots;

/// How a block kind's surface is made: from its kit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// The radius its outside edges are rounded to, in metres.
    pub bevel: f32,
    /// How far its relief stands out, from the deepest joint to the proudest stone.
    pub relief: f32,
    /// How finely each cube's face is divided.
    pub steps: u32,
    /// How many metres across and down one copy of its texture covers.
    pub metres: [f32; 2],
    /// Its tops, if they are drawn in a material of their own, as a stone floor's
    /// flagstones are: how many metres one copy of that covers, and how far its relief
    /// stands out.
    pub top: Option<([f32; 2], f32)>,
    /// Earth's packing (D236), if it was packed: lumps and lifts in its shape.
    pub packed: Option<Packed>,
}

/// Earth's packing in a surface's shape: its kit's `Packing`, and how tall its lifts are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Packed {
    pub packing: crate::kit::Packing,
    pub lift: f32,
}

impl Packed {
    /// How far the surface is pushed out at a point by its packing: lumps of two sizes, and
    /// each lift rounded out between its seams, a little flatter in its middle.
    fn at(&self, p: Vec3, outward: Vec3, stepping: f32) -> f32 {
        use crate::dress::smooth;
        let k = &self.packing;
        let lumps = if k.lumps > 0.0 {
            let size = k.lump.max(0.05);
            (smooth(p, size, 41) * 0.65 + smooth(p, size * 0.45, 43) * 0.35 - 0.5) * 2.0 * k.lumps
        } else {
            0.0
        };
        let ridge = if k.ridge > 0.0 {
            let y = crate::dress::lift_at(p, self.lift.max(0.05));
            let within = y - libm::floorf(y);
            let belly = 1.0 - (2.0 * within - 1.0) * (2.0 * within - 1.0);
            (libm::powf(belly, 0.6) - 0.6) * k.ridge
        } else {
            0.0
        };
        // Each lift a little proud or shy of the one under it, eased over its seam, out of
        // a wall's upright faces and round its arrises, not up off its top, and by `stepping`
        // of it, as near a cut it eases to nothing.
        let step = if k.step > 0.0 && stepping > 0.0 {
            let y = crate::dress::lift_at(p, self.lift.max(0.05));
            let layer = libm::floorf(y);
            let within = y - layer;
            let own = |l: f32| {
                (crate::layout::unit(crate::layout::hash(&[l as i32 as u32, 61])) - 0.5)
                    * 2.0
                    * k.step
            };
            let t = (within / 0.12).clamp(0.0, 1.0);
            let t = t * t * (3.0 - 2.0 * t);
            let upright = ((0.2 - outward.y.abs()) / 0.2).clamp(0.0, 1.0);
            (own(layer - 1.0) + (own(layer) - own(layer - 1.0)) * t) * upright * stepping
        } else {
            0.0
        };
        lumps + ridge + step
    }
}

/// Where a point of a surface is against the pieces set into it (`dress::Bed::at`).
pub type BedAt<'a> = &'a dyn Fn(Vec3) -> Option<(f32, f32, bool)>;

/// How far a point on a face's side may slide along it onto a hole's outline, as a share of
/// the fine grid's step: less than half, so it never passes a point another face's grid
/// put on the same side.
const RIM_SLIDE: f32 = 0.45;

/// How many times finer than its steps a face holed by damage is drawn: the points beside a
/// hole are put on its outline, so the grid need only be fine enough to find it.
pub const HOLED_FINER: usize = 2;

/// What a surface is displaced by: its material's height where its texture is at a point,
/// from 0 to 1, and its tops' material's, if they have one.
pub struct Heights<'a> {
    pub sides: &'a dyn Fn([f32; 2]) -> f32,
    pub top: &'a dyn Fn([f32; 2]) -> f32,
    /// Where a point of the surface is against a piece set into it, a lintel or a sill
    /// (`dress::Bed`): how far inside the piece's outline on the face it is, in metres (less
    /// than nothing outside), how far the piece's face stands out, and whether the surface
    /// laps over its edge there. Inside, the surface
    /// is sunk behind its deepest relief, so the piece can sit all but flush; round the
    /// outline it is built up a little over the piece's edge, so the piece is set in it.
    pub bed: Option<BedAt<'a>>,
    /// A number for the pieces bedded near a cube, two cubes round it, that `bed` reads:
    /// what a face kept from one drawing to the next ([`FaceCache`]) must have the same.
    pub bed_key: Option<&'a dyn Fn(IVec3) -> u64>,
    /// Where damage has taken its stones out (D305): how far behind its face a point is,
    /// from where it is before it is rounded and its face's normal; and, for a cube that
    /// is damaged or beside one, a number for its damage, which draws its faces finely
    /// enough to follow its stones and which a face kept from one drawing to the next must
    /// have the same; none for a cube with none.
    pub hole: Option<&'a dyn Fn(Vec3, Vec3) -> f32>,
    pub holed: Option<&'a dyn Fn(IVec3) -> Option<u64>>,
}

/// How far behind its deepest relief a wall's face is sunk where a piece is bedded in it.
const BED_DEPTH_M: f32 = 0.008;
/// How far either side of a bedded piece's outline the wall comes back to its own face, and
/// how far over the piece's face it is built up on the outline.
const BED_LAP_M: f32 = 0.035;
const BED_OVER_M: f32 = 0.012;

/// The most triangles a square metre of a block kind's surface open to the air may take,
/// on average, at the finest detail (D284): a cube's face, a quarter of a square metre, is
/// two flat, a dozen or so rounded along an open edge, a few dozen at a corner. Checked on a
/// wall here and on a stone house in `sections`' tests.
pub const NEAR_TRIANGLES_A_SQUARE_METRE: f32 = 64.0;

/// How far an inside corner's relief fades out from it, in metres.
const INSIDE_FADE_M: f32 = 0.06;

/// The faces, as an axis and a side, with the two axes across each.
const FACES: [(usize, f32); 6] = [
    (0, 1.0),
    (0, -1.0),
    (1, 1.0),
    (1, -1.0),
    (2, 1.0),
    (2, -1.0),
];

fn unit(axis: usize, side: f32) -> Vec3 {
    let mut v = Vec3::ZERO;
    v[axis] = side;
    v
}

fn step(axis: usize, side: i32) -> IVec3 {
    let mut v = IVec3::ZERO;
    v[axis] = side;
    v
}

/// The bit of a cube's edge along `axis` at its high side across the next axis (`da`) and
/// the one after (`db`), each 0 or 1, in a mask of its twelve edges.
fn bite_bit(axis: usize, da: i32, db: i32) -> u32 {
    axis as u32 * 4 + da as u32 + 2 * db as u32
}

/// A cube's box shrunk by the bevel (`draw_slots`), and the edges an inside edge runs along,
/// bitten out of it a bevel round: the surface is the bevel's radius out from the nearest
/// point of these. With the bites the shape is the cubes opened by a ball of the bevel
/// (eroded, then grown again, the morphological opening a bevel mesher's rounded box is):
/// every outside edge a quarter-round, an inside edge sharp, and where two outside edges
/// meet an inside one at a face, as a window's jamb meets its sill at the wall's face, a
/// rounded blend of the three rather than a corner of the cube's box swelling past them.
#[derive(Clone, Copy, Debug)]
struct Rounded {
    low: Vec3,
    high: Vec3,
    /// The cube's low corner.
    cube: Vec3,
    /// Its edges bitten (`bite_bit`).
    bites: u16,
}

impl Rounded {
    /// The nearest point of it to `p`.
    fn nearest(&self, p: Vec3, bevel: f32) -> Vec3 {
        let q = p.clamp(self.low, self.high);
        if self.bites == 0 {
            return q;
        }
        for axis in 0..3 {
            let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
            for (da, db) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                if self.bites & (1 << bite_bit(axis, da, db)) == 0 {
                    continue;
                }
                // Across the edge, in from it into the box each way.
                let corner = [
                    self.cube[a] + da as f32 * CELL_M,
                    self.cube[b] + db as f32 * CELL_M,
                ];
                let inward = [1.0 - 2.0 * da as f32, 1.0 - 2.0 * db as f32];
                let into = |v: Vec3| {
                    [
                        (v[a] - corner[0]) * inward[0],
                        (v[b] - corner[1]) * inward[1],
                    ]
                };
                let [u, v] = into(q);
                if u * u + v * v >= bevel * bevel {
                    continue;
                }
                // In the bite: the nearest point of its arc, the one toward `p` or either
                // end, or its middle where all are as near.
                let [pu, pv] = into(p);
                let (du, dv) = (pu.max(0.0), pv.max(0.0));
                let length = (du * du + dv * dv).sqrt();
                let half = core::f32::consts::FRAC_1_SQRT_2;
                let toward = (length > 1e-6).then(|| [du / length, dv / length]);
                let arc = |[cu, cv]: [f32; 2]| {
                    let mut out = q;
                    out[a] = corner[0] + inward[0] * bevel * cu;
                    out[b] = corner[1] + inward[1] * bevel * cv;
                    out
                };
                return toward
                    .into_iter()
                    .chain([[half, half], [1.0, 0.0], [0.0, 1.0]])
                    .map(arc)
                    .min_by(|x, y| x.distance_squared(p).total_cmp(&y.distance_squared(p)))
                    .unwrap_or(q);
            }
        }
        q
    }
}

/// Draws the cubes as one surface into `out`, in the base's own space, and their tops into
/// `tops` if the look gives them a material of their own. `hides` says which other cubes
/// hide a face against them; the cubes drawn always hide one another.
pub fn draw(
    cubes: &BTreeSet<[i32; 3]>,
    hides: &dyn Fn(IVec3) -> bool,
    look: &Look,
    heights: &Heights,
    colour: [f32; 3],
    out: &mut Surface,
    tops: &mut Surface,
) {
    let cuts = Cuts {
        of: &|_| false,
        at: &|_, _| [f32::MIN, f32::MAX],
        key: &|_| None,
    };
    draw_part(
        cubes,
        &|_| true,
        &|at| cubes.contains(&at.to_array()),
        hides,
        look,
        heights,
        &cuts,
        colour,
        out,
        tops,
        None,
    );
}

/// As [`draw`], with each cube's surface cut off above and below at heights over each
/// point of it, as a gable's is at its roof's underside and a slope's at its ramp: `cut`
/// gives them, lowest and highest, for a cube and a point of its face before it is rounded,
/// nothing of the face going lower or higher, so an upper half or an inverted slope is cut
/// from below (D231).
#[allow(clippy::too_many_arguments)]
pub fn draw_cut(
    cubes: &BTreeSet<[i32; 3]>,
    hides: &dyn Fn(IVec3) -> bool,
    look: &Look,
    heights: &Heights,
    cut: &dyn Fn(IVec3, Vec3) -> [f32; 2],
    colour: [f32; 3],
    out: &mut Surface,
    tops: &mut Surface,
) {
    let cuts = Cuts {
        of: &|_| true,
        at: cut,
        key: &|_| None,
    };
    draw_part(
        cubes,
        &|_| true,
        &|at| cubes.contains(&at.to_array()),
        hides,
        look,
        heights,
        &cuts,
        colour,
        out,
        tops,
        None,
    );
}

/// Faces of a surface drawn before, each with what it was worked out from, kept so the
/// surface drawn again after an edit works out only the faces near what changed, as a
/// voxel mesher keeps a chunk's mesh and remeshes only round an edit: a face's place
/// depends on the cubes two round it, and its edges' shading on three
/// ([`draw_part`]). Each face is filed under the number of the surface it is of.
#[derive(Clone, Debug, Default)]
pub struct FaceCache {
    faces: QuickMap<(u32, IVec3, u8), Arc<FaceData>>,
    /// How many of them were taken from the last.
    taken: usize,
}

impl FaceCache {
    /// How many faces it holds.
    pub fn len(&self) -> usize {
        self.faces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// How many of its faces were taken from the last drawing rather than worked out.
    pub fn taken(&self) -> usize {
        self.taken
    }

    /// Another's faces added, as a section's parts drawn apart are put together: a face
    /// both hold was worked out from the same cubes, so either will do.
    pub fn take_from(&mut self, other: FaceCache) {
        self.taken += other.taken;
        for (key, face) in other.faces {
            self.faces.entry(key).or_insert(face);
        }
    }

    /// The faces of the cubes `whose` says carried on from another, as they were, where
    /// this holds none of theirs: those of a section's parts not drawn again.
    pub fn carry_on(&mut self, from: &FaceCache, whose: &dyn Fn(IVec3) -> bool) {
        for (key, face) in &from.faces {
            if whose(key.1) {
                self.faces.entry(*key).or_insert_with(|| face.clone());
            }
        }
    }
}

/// A surface's faces as drawn last time, to take from, and this time, to keep: under a
/// number of the caller's, one for each surface it draws (a kit's, at a detail).
pub struct Caching<'a> {
    pub call: u32,
    pub old: Option<&'a FaceCache>,
    pub new: &'a mut FaceCache,
}

/// How cubes are cut (D231): `of` says whether a cube may be cut at all, so one that is
/// not, as most are, is drawn whole without asking where; `at` gives the heights a cube is
/// cut to over a point of its face, lowest and highest, as [`draw_cut`] takes them; and
/// `key` a number for everything a cube's cut is worked out from, if there is one, so the
/// faces round a cut cube are kept from one drawing to the next ([`FaceCache`]) while it is
/// cut the same.
pub struct Cuts<'a> {
    pub of: &'a dyn Fn(IVec3) -> bool,
    pub at: &'a dyn Fn(IVec3, Vec3) -> [f32; 2],
    pub key: &'a dyn Fn(IVec3) -> Option<u64>,
}

/// The cubes drawn and those round them, a few deep, in one box looked up by place rather
/// than hashed, as a terrain mesher keeps a chunk's voxels padded by its neighbours'.
struct Grid {
    low: IVec3,
    size: IVec3,
}

impl Grid {
    /// How far round the cubes it reaches: a face looks at the cubes one away, and the
    /// inside edges two.
    const PAD: i32 = 3;

    fn around(cubes: &BTreeSet<[i32; 3]>) -> Grid {
        let (low, high) = cubes
            .iter()
            .map(|&c| IVec3::from_array(c))
            .fold((IVec3::MAX, IVec3::MIN), |(l, h), c| (l.min(c), h.max(c)));
        let (low, high) = if cubes.is_empty() {
            (IVec3::ZERO, IVec3::ZERO)
        } else {
            (low, high)
        };
        let low = low - IVec3::splat(Self::PAD);
        Grid {
            low,
            size: high + IVec3::splat(Self::PAD + 1) - low,
        }
    }

    fn len(&self) -> usize {
        (self.size.x * self.size.y * self.size.z) as usize
    }

    fn index(&self, at: IVec3) -> Option<usize> {
        let d = at - self.low;
        (d.cmpge(IVec3::ZERO).all() && d.cmplt(self.size).all())
            .then(|| (d.x + self.size.x * (d.y + self.size.y * d.z)) as usize)
    }
}

/// A grid's values run together from its low corner: each cell's the XOR, and the sum, of
/// every value in the box from the low corner to it, so any box's are read from its
/// corners in eight steps, as a summed-area table is.
struct Running {
    xor: Vec<u64>,
    sum: Vec<u64>,
    size: IVec3,
}

impl Running {
    fn of(grid: &Grid, values: &[u64]) -> Running {
        // One cell more each way, so the box's corners below the grid read zero.
        let size = grid.size + IVec3::ONE;
        let at = |x: i32, y: i32, z: i32| (x + size.x * (y + size.y * z)) as usize;
        let cells = (size.x * size.y * size.z) as usize;
        let (mut xor, mut sum) = (vec![0u64; cells], vec![0u64; cells]);
        for z in 1..size.z {
            for y in 1..size.y {
                for x in 1..size.x {
                    let v = values
                        [((x - 1) + grid.size.x * ((y - 1) + grid.size.y * (z - 1))) as usize];
                    let corners = [
                        at(x - 1, y, z),
                        at(x, y - 1, z),
                        at(x, y, z - 1),
                        at(x - 1, y - 1, z),
                        at(x - 1, y, z - 1),
                        at(x, y - 1, z - 1),
                        at(x - 1, y - 1, z - 1),
                    ];
                    let mut x_ = v;
                    for c in corners {
                        x_ ^= xor[c];
                    }
                    xor[at(x, y, z)] = x_;
                    let s = v
                        .wrapping_add(sum[corners[0]])
                        .wrapping_add(sum[corners[1]])
                        .wrapping_add(sum[corners[2]])
                        .wrapping_sub(sum[corners[3]])
                        .wrapping_sub(sum[corners[4]])
                        .wrapping_sub(sum[corners[5]])
                        .wrapping_add(sum[corners[6]]);
                    sum[at(x, y, z)] = s;
                }
            }
        }
        Running { xor, sum, size }
    }

    /// The box `reach` round a cube, as corners into the running tables: its high corner
    /// and its low corner less one, each cell inside the grid's.
    fn corners(&self, grid: &Grid, cube: IVec3, reach: i32) -> [(usize, bool); 8] {
        let low = (cube - IVec3::splat(reach) - grid.low).clamp(IVec3::ZERO, grid.size);
        let high = (cube + IVec3::splat(reach + 1) - grid.low).clamp(IVec3::ZERO, grid.size);
        let at = |x: i32, y: i32, z: i32| (x + self.size.x * (y + self.size.y * z)) as usize;
        let mut out = [(0, true); 8];
        for (k, out) in out.iter_mut().enumerate() {
            let (hx, hy, hz) = (k & 1 == 0, k & 2 == 0, k & 4 == 0);
            let x = if hx { high.x } else { low.x };
            let y = if hy { high.y } else { low.y };
            let z = if hz { high.z } else { low.z };
            // Added where an even number of its coordinates are the low corner's.
            let lows = u32::from(!hx) + u32::from(!hy) + u32::from(!hz);
            *out = (at(x, y, z), lows % 2 == 0);
        }
        out
    }

    fn xor(&self, grid: &Grid, cube: IVec3, reach: i32) -> u64 {
        self.corners(grid, cube, reach)
            .iter()
            .fold(0, |acc, &(i, _)| acc ^ self.xor[i])
    }

    fn sum(&self, grid: &Grid, cube: IVec3, reach: i32) -> u64 {
        self.corners(grid, cube, reach)
            .iter()
            .fold(0u64, |acc, &(i, add)| {
                if add {
                    acc.wrapping_add(self.sum[i])
                } else {
                    acc.wrapping_sub(self.sum[i])
                }
            })
    }
}

/// Whether a cube of the grid is full: not yet asked, no, or yes.
const UNKNOWN: u8 = 0;
const EMPTY: u8 = 1;
const FULL: u8 = 2;

/// As [`draw_cut`], keeping only the faces of the cubes `keep` says: the rest are worked
/// out, so the edges and shading where they meet the kept ones come out as they would
/// with all of them drawn, and left out. A section drawn with the cubes a step beyond its
/// edge meets its neighbour's surface with no seam (docs/building-plan.md, drawing).
/// `kin` says which cubes are drawn into the same surface, in this call or another (a
/// section's whole cubes and its cut ones are drawn apart), so faces meeting across calls
/// put their points in the same places along the line they share.
///
/// The cubes, whether those round them are full, their shrunk boxes and the inside edges
/// are kept in a [`Grid`]; a point further than the bevel from its face's edges is nearest
/// its own cube's box alone, and the inside edges a face is not near are not looked at, so
/// most of a face is placed with no search at all.
#[allow(clippy::too_many_arguments)]
pub fn draw_part(
    cubes: &BTreeSet<[i32; 3]>,
    keep: &dyn Fn(IVec3) -> bool,
    kin: &dyn Fn(IVec3) -> bool,
    hides: &dyn Fn(IVec3) -> bool,
    look: &Look,
    heights: &Heights,
    cuts: &Cuts,
    colour: [f32; 3],
    out: &mut Surface,
    tops: &mut Surface,
    caching: Option<Caching>,
) {
    draw_slots(
        cubes,
        &|at| keep(at).then_some(0),
        kin,
        hides,
        look,
        heights,
        cuts,
        colour,
        std::slice::from_mut(out),
        std::slice::from_mut(tops),
        caching,
    );
}

/// Where a face's points fall across it one way, as fractions of a cube from its low edge:
/// at both edges; across the rounding of each edge open to the air, in as many steps as
/// its detail rounds it in; and between them none, the face flat, unless its kind is packed
/// in lifts and lumps (earth, D236), whose shape the points must follow, then every
/// quarter of a cube near and every half a few metres off. As terrain and voxel renderers
/// spend triangles only where the silhouette bends and leave a flat face's detail to its
/// normal and height maps.
fn across(low_open: bool, high_open: bool, look: &Look) -> Vec<f32> {
    let (rounding, packing) = if look.steps >= crate::sections::SURFACE_STEPS {
        (3, 4)
    } else if look.steps >= 4 {
        (2, 2)
    } else {
        (1, 1)
    };
    let bevel = (look.bevel / CELL_M).min(0.45);
    let mut out = vec![0.0, 1.0];
    if bevel > 0.0 {
        for k in 1..=rounding {
            let t = bevel * k as f32 / rounding as f32;
            if low_open {
                out.push(t);
            }
            if high_open {
                out.push(1.0 - t);
            }
        }
    }
    if look.packed.is_some() {
        let (from, to) = (
            if low_open { bevel } else { 0.0 },
            if high_open { 1.0 - bevel } else { 1.0 },
        );
        out.extend(
            (1..packing)
                .map(|k| k as f32 / packing as f32)
                .filter(|&t| t > from + 0.02 && t < to - 0.02),
        );
    }
    out.sort_by(f32::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
    out
}

/// A cube and those round them.
const NEIGHBOURHOOD: [IVec3; 27] = {
    let mut out = [IVec3::ZERO; 27];
    let mut i = 0;
    while i < 27 {
        out[i] = IVec3::new(i as i32 % 3 - 1, i as i32 / 3 % 3 - 1, i as i32 / 9 - 1);
        i += 1;
    }
    out
};

/// What a face was worked out from ([`FaceCache`]): where its points are, from the cubes
/// two round its cube; and all of it, its rims' normals too, from those three round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Signature {
    shape: u64,
    whole: u64,
}

/// A cube's face as it is drawn, whether it is kept, its cube and which of its faces it is,
/// and whether it was worked out anew rather than kept from last time.
struct Face {
    data: Arc<FaceData>,
    keeping: bool,
    /// The surface it goes into, if it is kept.
    slot: usize,
    cube: IVec3,
    index: u8,
    fresh: bool,
    /// Worked out anew only along its rims: its points kept from last time.
    rims_only: bool,
}

/// A face worked out: what it was worked out from ([`FaceCache`]), its grid of points,
/// placed, where each falls on its texture, its triangles as three points of the grid each,
/// whether it is a top in a material of its own; each point's own triangles' normals
/// summed and its normal; and its points in the order its triangles come to them, with its
/// triangles' corners among them.
#[derive(Clone, Debug)]
pub struct FaceData {
    signature: Option<Signature>,
    /// Everything its points were placed from, as a number.
    placed: Option<u64>,
    points: Vec<Vec3>,
    uvs: Vec<[f32; 2]>,
    triangles: Vec<[u32; 3]>,
    /// Whether each point is on its rim, where other faces meet it.
    rim: Vec<bool>,
    /// Each point's normal where the rounding alone gives it, `ZERO` where its triangles do.
    exact: Vec<Vec3>,
    top: bool,
    local: Vec<Vec3>,
    normals: Vec<Vec3>,
    order: Vec<u32>,
    indices: Vec<u32>,
    /// Each point's share sunk into a hole damage left (D305), 0 or 1, which the texture
    /// arrays' shader draws as the hollow behind: its wear (`Surface::wear`). Empty for
    /// none.
    sunk: Vec<f32>,
}

/// How far the surface is pushed out at a point: its materials' heights there, their three
/// planar mappings blended by which way the surface faces, so the faces either side of an
/// outside edge agree; the tops' own material where they face up and have one.
fn displaced(p: Vec3, outward: Vec3, look: &Look, heights: &Heights) -> f32 {
    let weights = outward.abs();
    let weights = weights * weights * weights * weights;
    let total = weights.x + weights.y + weights.z;
    (0..3)
        .filter(|&axis| weights[axis] > 1e-4)
        .map(|axis| {
            let (height, metres, relief) = match look.top {
                Some((metres, relief)) if axis == 1 && outward.y > 0.0 => {
                    (heights.top, metres, relief)
                }
                _ => (heights.sides, look.metres, look.relief),
            };
            let at = height(planar(p, unit(axis, 1.0), metres));
            weights[axis] / total * (at - 0.5) * relief
        })
        .sum()
}

/// A box of a block kind's material from one corner to the other, as its cubes' surface is
/// drawn: rounded along every edge by the bevel and displaced by its material, each face
/// divided about as finely as a cube's; its top in its tops' material if it has one. Door
/// steps are drawn so.
pub fn slab(
    min: Vec3,
    max: Vec3,
    look: &Look,
    heights: &Heights,
    colour: [f32; 3],
    out: &mut Surface,
    tops: &mut Surface,
) {
    let size = max - min;
    let bevel = look.bevel.min(size.min_element() * 0.45);
    let (low, high) = (min + Vec3::splat(bevel), max - Vec3::splat(bevel));
    let cell = CELL_M / look.steps.max(1) as f32;
    for (axis, side) in FACES {
        let normal = unit(axis, side);
        let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
        let (na, nb) = (
            ((size[a] / cell).ceil() as usize).max(1),
            ((size[b] / cell).ceil() as usize).max(1),
        );
        let plane = if side > 0.0 { max[axis] } else { min[axis] };
        let point = |i: usize, j: usize| {
            let mut p = Vec3::ZERO;
            p[axis] = plane;
            p[a] = min[a] + size[a] * i as f32 / na as f32;
            p[b] = min[b] + size[b] * j as f32 / nb as f32;
            p
        };
        let place = |p: Vec3| -> (Vec3, Vec3) {
            let nearest = p.clamp(low, high);
            let outward = (p - nearest).normalize_or(normal);
            let rounded = nearest + outward * bevel;
            (
                rounded + outward * displaced(p, outward, look, heights),
                outward,
            )
        };
        let top = axis == 1 && side > 0.0 && look.top.is_some();
        let metres = match look.top {
            Some((metres, _)) if top => metres,
            _ => look.metres,
        };
        let into = if top { &mut *tops } else { &mut *out };
        let flip = side < 0.0;
        for i in 0..na {
            for j in 0..nb {
                let corners = [
                    point(i, j),
                    point(i + 1, j),
                    point(i + 1, j + 1),
                    point(i, j + 1),
                ];
                let placed = corners.map(place);
                let mapped = corners.map(|p| planar(p, normal, metres));
                for [x, y, z] in [[0, 1, 2], [0, 2, 3]] {
                    let (y, z) = if flip { (z, y) } else { (y, z) };
                    into.textured(
                        [placed[x].0, placed[y].0, placed[z].0],
                        [placed[x].1, placed[y].1, placed[z].1],
                        [mapped[x], mapped[y], mapped[z]],
                        colour,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look() -> Look {
        Look {
            bevel: 0.04,
            relief: 0.05,
            steps: 8,
            metres: [2.0, 2.0],
            top: None,
            packed: None,
        }
    }

    fn flat(_: [f32; 2]) -> f32 {
        0.5
    }

    fn bumpy(uv: [f32; 2]) -> f32 {
        0.5 + 0.5 * libm::sinf(uv[0] * 40.0) * libm::cosf(uv[1] * 37.0)
    }

    fn drawn(cubes: &[[i32; 3]], height: &dyn Fn([f32; 2]) -> f32) -> Surface {
        drawn_as(cubes, &look(), height)
    }

    fn drawn_as(cubes: &[[i32; 3]], look: &Look, height: &dyn Fn([f32; 2]) -> f32) -> Surface {
        let (out, tops) = drawn_with_tops(cubes, look, height);
        assert!(tops.positions.is_empty());
        out
    }

    fn drawn_with_tops(
        cubes: &[[i32; 3]],
        look: &Look,
        height: &dyn Fn([f32; 2]) -> f32,
    ) -> (Surface, Surface) {
        let set: BTreeSet<[i32; 3]> = cubes.iter().copied().collect();
        let (mut out, mut tops) = (Surface::default(), Surface::default());
        let heights = Heights {
            sides: height,
            top: &flat,
            bed: None,
            bed_key: None,
            hole: None,
            holed: None,
        };
        draw(
            &set,
            &|_| false,
            look,
            &heights,
            [1.0; 3],
            &mut out,
            &mut tops,
        );
        (out, tops)
    }

    #[test]
    fn a_cube_shapes_only_the_faces_of_the_cubes_beside_it() {
        // An L of wall two cubes thick and three high, with a buttress: outside and inside
        // corners, bevels and their fades. Each cube taken out in turn, every face of the
        // cubes two or more from it comes out the same, to the bit, whatever the bevel:
        // what `sections::cluster_keys` keys a part by.
        let mut wall: Vec<[i32; 3]> = Vec::new();
        for y in 0..3 {
            for x in 0..6 {
                wall.extend([[x, y, 0], [x, y, 1]]);
            }
            for z in 2..6 {
                wall.extend([[0, y, z], [1, y, z]]);
            }
            wall.push([3, y, 2]);
        }
        let heights = Heights {
            sides: &bumpy,
            top: &flat,
            bed: None,
            bed_key: None,
            hole: None,
            holed: None,
        };
        let cuts = Cuts {
            of: &|_| false,
            at: &|_, _| [f32::MIN, f32::MAX],
            key: &|_| None,
        };
        let soft = Look {
            bevel: 0.12,
            ..look()
        };
        let mut compared = 0;
        for look in [look(), soft] {
            for &gone in &wall {
                let gone = IVec3::from_array(gone);
                let far = |at: IVec3| (at - gone).abs().max_element() >= 2;
                let drawn = |cubes: &BTreeSet<[i32; 3]>| {
                    let (mut out, mut tops) = (Surface::default(), Surface::default());
                    draw_part(
                        cubes,
                        &far,
                        &|at| cubes.contains(&at.to_array()),
                        &|_| false,
                        &look,
                        &heights,
                        &cuts,
                        [1.0; 3],
                        &mut out,
                        &mut tops,
                        None,
                    );
                    out
                };
                let all: BTreeSet<[i32; 3]> = wall.iter().copied().collect();
                let mut less = all.clone();
                less.remove(&gone.to_array());
                let (with, without) = (drawn(&all), drawn(&less));
                assert!(with.triangle_count() > 0);
                assert!(with == without, "{gone} reaches a face two cubes off");
                compared += 1;
            }
        }
        assert_eq!(compared, wall.len() * 2);
    }

    /// Both surfaces as one, for checking it is closed.
    fn joined(a: Surface, b: &Surface) -> Surface {
        let mut a = a;
        a.positions.extend(&b.positions);
        a.normals.extend(&b.normals);
        a.colours.extend(&b.colours);
        a.uvs.extend(&b.uvs);
        a
    }

    #[test]
    fn a_floor_of_stone_has_its_tops_in_their_own_material_and_stays_closed() {
        let floor: Vec<[i32; 3]> = (0..3)
            .flat_map(|x| (0..2).map(move |z| [x, 0, z]))
            .collect();
        let flagged = Look {
            top: Some(([1.0, 1.0], 0.01)),
            ..look()
        };
        let (sides, tops) = drawn_with_tops(&floor, &flagged, &bumpy);
        assert!(!tops.positions.is_empty() && !sides.positions.is_empty());
        // The tops face up, and none of the sides do but along the rounded edges.
        assert!(tops.normals.iter().all(|n| n.y > 0.3));
        let flat_sides = sides.normals.iter().filter(|n| n.y > 0.99).count();
        assert_eq!(flat_sides, 0);
        assert!(closed(&joined(sides, &tops)));
    }

    #[test]
    fn a_slab_is_closed_rounded_and_its_own_size() {
        let (mut out, mut tops) = (Surface::default(), Surface::default());
        let heights = Heights {
            sides: &bumpy,
            top: &flat,
            bed: None,
            bed_key: None,
            hole: None,
            holed: None,
        };
        let (min, max) = (Vec3::new(0.0, -0.3, 0.0), Vec3::new(1.5, 0.12, 0.32));
        slab(min, max, &look(), &heights, [1.0; 3], &mut out, &mut tops);
        assert!(closed(&out));
        let half = look().relief * 0.5 + 1e-4;
        assert!(
            out.positions
                .iter()
                .all(|p| p.cmpge(min - half).all() && p.cmple(max + half).all())
        );
        // Its corner is rounded off.
        assert!(out.positions.iter().all(|p| p.distance(max) > 0.01));
    }

    /// There are triangles, and every edge of every one is shared by exactly one other
    /// triangle: no cracks.
    fn closed(surface: &Surface) -> bool {
        let key = |p: Vec3| ((p * 2048.0).round()).as_ivec3().to_array();
        let mut edges: std::collections::HashMap<([i32; 3], [i32; 3]), i32> = Default::default();
        for t in surface.positions.chunks(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (a, b) = (key(a), key(b));
                if a == b {
                    continue;
                }
                let k = if a < b { (a, b) } else { (b, a) };
                *edges.entry(k).or_default() += 1;
            }
        }
        !edges.is_empty() && edges.values().all(|&count| count == 2)
    }

    #[test]
    fn a_jamb_meeting_its_sill_rounds_into_it_at_the_walls_face() {
        // A wall a cube thick with an opening's corner cut from it, as a window's jamb
        // meets its sill: the cube under the jamb, beside the sill, has both its faces
        // there against full cubes, and its box's corner, unshrunk, would hold the wall's
        // face out flat to the very corner where the jamb's and the sill's rounded edges
        // meet the inside edge between them, a point the faces round it fold over. Rounded
        // a bevel as broad as earth's.
        let look = Look {
            bevel: 0.12,
            relief: 0.0,
            steps: 5,
            ..look()
        };
        let surface = drawn_as(&[[0, 0, 0], [0, 1, 0], [0, 0, 1]], &look, &flat);
        assert!(closed(&surface));
        for x in [0.0, 0.5] {
            let corner = Vec3::new(x, 0.5, 0.5);
            let nearest = surface
                .positions
                .iter()
                .map(|p| p.distance(corner))
                .fold(f32::MAX, f32::min);
            assert!(nearest > 0.02, "a point left at the corner {corner}");
        }
    }

    #[test]
    fn a_cube_is_closed_and_rounded() {
        let s = drawn(&[[0, 0, 0]], &flat);
        assert!(closed(&s));
        // Its corner is rounded: no point reaches the box's corner.
        let corner = Vec3::splat(CELL_M);
        assert!(s.positions.iter().all(|p| p.distance(corner) > 0.01));
        // Its faces' middles are where the cube's faces are.
        assert!(s.positions.iter().any(|p| (p.x - CELL_M).abs() < 1e-5));
    }

    #[test]
    fn a_side_shows_over_a_lower_cut_beside_it() {
        // A half, and east of it a slope rising east: the slope's west side shows from the
        // half's top up to the slope's.
        let set: BTreeSet<[i32; 3]> = [[0, 0, 0], [1, 0, 0]].into_iter().collect();
        let cut = |cube: IVec3, at: Vec3| {
            if cube.x == 0 {
                [f32::MIN, CELL_M * 0.5]
            } else {
                [f32::MIN, at.x - CELL_M]
            }
        };
        let (mut out, mut tops) = (Surface::default(), Surface::default());
        let heights = Heights {
            sides: &flat,
            top: &flat,
            bed: None,
            bed_key: None,
            hole: None,
            holed: None,
        };
        draw_cut(
            &set,
            &|_| false,
            &look(),
            &heights,
            &cut,
            [1.0; 3],
            &mut out,
            &mut tops,
        );
        // A triangle of a side looking east or west, not of a top or underside beside it.
        let side_on =
            |t: &[Vec3]| (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero().x.abs() > 0.5;
        let between = out.positions.chunks(3).any(|t| {
            side_on(t)
                && t.iter().all(|p| (p.x - CELL_M).abs() < 0.05)
                && t.iter().any(|p| p.y > CELL_M * 0.55)
        });
        assert!(
            !between,
            "the slope's side is no higher than its top at its foot"
        );
        // Against a half, a whole cube's own side shows from the half's top up.
        let whole =
            |cube: IVec3, _: Vec3| [f32::MIN, if cube.x == 0 { CELL_M * 0.5 } else { f32::MAX }];
        let (mut out, mut tops) = (Surface::default(), Surface::default());
        draw_cut(
            &set,
            &|_| false,
            &look(),
            &heights,
            &whole,
            [1.0; 3],
            &mut out,
            &mut tops,
        );
        let side = out.positions.chunks(3).any(|t| {
            t.iter().all(|p| (p.x - CELL_M).abs() < 0.05) && t.iter().all(|p| p.y > CELL_M * 0.49)
        });
        assert!(side, "no side over the half");
        // Drawn whole, its part under the half's top inside the half and never seen: brought
        // up onto a sloping cut between its points, a side folded over itself in its plane.
        assert!(
            out.positions.chunks(3).any(|t| {
                side_on(t)
                    && t.iter().all(|p| (p.x - CELL_M).abs() < 0.05)
                    && t.iter().all(|p| p.y < CELL_M * 0.45)
            }),
            "the side drawn whole"
        );
    }

    #[test]
    fn cubes_side_by_side_are_one_surface_with_no_face_between() {
        let s = drawn(&[[0, 0, 0], [1, 0, 0]], &bumpy);
        assert!(closed(&s));
        // Nothing drawn on the plane between them, away from the outer faces.
        assert!(!s.positions.iter().any(|p| (p.x - CELL_M).abs() < 1e-4
            && p.y > 0.1
            && p.y < 0.4
            && p.z > 0.1
            && p.z < 0.4));
    }

    #[test]
    fn an_l_and_a_carved_wall_stay_closed_with_relief() {
        let ell = [
            [0, 0, 0],
            [1, 0, 0],
            [0, 0, 1],
            [0, 1, 0],
            [1, 1, 0],
            [0, 1, 1],
        ];
        assert!(closed(&drawn(&ell, &bumpy)));
        let mut wall: Vec<[i32; 3]> = (0..4)
            .flat_map(|x| (0..3).map(move |y| [x, y, 0]))
            .collect();
        wall.retain(|c| *c != [1, 1, 0]);
        assert!(closed(&drawn(&wall, &bumpy)));
    }

    #[test]
    fn packed_earths_lifts_each_stand_a_little_proud_or_shy_of_the_next() {
        let packed = Packed {
            packing: crate::kit::Packing {
                lumps: 0.0,
                lump: 0.5,
                ridge: 0.0,
                step: 0.03,
            },
            lift: 0.5,
        };
        // The middle of each of eight lifts up a line on a wall: within the step either way,
        // and not all alike, so the wall's outline steps.
        let middles: Vec<f32> = (0..8)
            .map(|lift| {
                let mut p = Vec3::new(0.3, 0.0, 0.0);
                // Up the line to where the lift's middle is, as the seams wander.
                p.y = (0..200)
                    .map(|i| i as f32 * 0.025)
                    .find(|&y| {
                        let at = crate::dress::lift_at(Vec3::new(0.3, y, 0.0), 0.5);
                        libm::floorf(at) as i32 == lift && at - libm::floorf(at) > 0.5
                    })
                    .unwrap();
                packed.at(p, Vec3::X, 1.0)
            })
            .collect();
        assert!(
            middles.iter().all(|m| m.abs() <= 0.03 + 1e-5),
            "{middles:?}"
        );
        let (low, high) = middles
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        assert!(high - low > 0.02, "{middles:?}");
    }

    #[test]
    fn packed_earth_stays_closed_and_its_lifts_stand_out_between_their_seams() {
        let packing = crate::kit::Packing {
            lumps: 0.0,
            lump: 0.5,
            ridge: 0.04,
            step: 0.0,
        };
        let packed = Packed { packing, lift: 0.5 };
        // Up a line on the wall, the lifts' middles stand out the ridge's depth beyond the
        // seams, and no further.
        let along: Vec<f32> = (0..400)
            .map(|i| packed.at(Vec3::new(0.3, i as f32 * 0.01, 0.0), Vec3::X, 1.0))
            .collect();
        let (low, high) = along
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        assert!((high - low - 0.04).abs() < 0.004, "{low} {high}");
        // With lumps too, a carved wall's surface stays one closed surface.
        let look = Look {
            packed: Some(Packed {
                packing: crate::kit::Packing {
                    lumps: 0.03,
                    ..packing
                },
                lift: 0.5,
            }),
            ..look()
        };
        let mut wall: Vec<[i32; 3]> = (0..4)
            .flat_map(|x| (0..3).map(move |y| [x, y, 0]))
            .collect();
        wall.retain(|c| *c != [1, 1, 0]);
        let surface = drawn_as(&wall, &look, &bumpy);
        assert!(closed(&surface));
        // And its face is no longer flat: it bulges out and sinks in by centimetres.
        let front: Vec<f32> = surface
            .positions
            .iter()
            .filter(|p| p[2] > CELL_M * 0.5)
            .map(|p| p[2])
            .collect();
        let spread = front.iter().fold(f32::MIN, |a, &b| a.max(b))
            - front.iter().fold(f32::MAX, |a, &b| a.min(b));
        assert!(spread > 0.06, "{spread}");
    }

    #[test]
    fn the_stones_relief_is_left_to_the_maps_and_a_flat_face_is_two_triangles() {
        // An L two cubes high: its relief moves nothing, the height its maps' to draw.
        let ell = [
            [0, 0, 0],
            [1, 0, 0],
            [0, 0, 1],
            [0, 1, 0],
            [1, 1, 0],
            [0, 1, 1],
        ];
        let (a, b) = (drawn(&ell, &flat), drawn(&ell, &bumpy));
        assert_eq!(a.positions, b.positions);
        // In a wall eight cubes long, four high and one thick, a face in its middle is flat
        // and one square: two triangles; the rounding is along its open edges alone.
        let wall: Vec<[i32; 3]> = (0..8)
            .flat_map(|x| (0..4).map(move |y| [x, y, 0]))
            .collect();
        let near = Look {
            steps: crate::sections::SURFACE_STEPS,
            ..look()
        };
        let surface = drawn_as(&wall, &near, &flat);
        let middle = surface
            .positions
            .chunks(3)
            .filter(|t| {
                t.iter().all(|p| {
                    (p.z - CELL_M).abs() < 1e-5
                        && (CELL_M * 3.0..=CELL_M * 4.0).contains(&p.x)
                        && (CELL_M..=CELL_M * 2.0).contains(&p.y)
                })
            })
            .count();
        assert_eq!(middle, 2);
        // About as many triangles a square metre as its rims need: the budget near (D284).
        let area = (2 * 8 * 4 + 2 * 8 + 2 * 4) as f32 * CELL_M * CELL_M;
        let per_metre = surface.triangle_count() as f32 / area;
        assert!(
            per_metre <= NEAR_TRIANGLES_A_SQUARE_METRE,
            "{per_metre} a square metre"
        );
        // Rounded along its edges, still more than flat.
        assert!(per_metre > 8.0, "{per_metre} a square metre");
    }
}
