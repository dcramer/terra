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

/// As [`draw_part`], each kept cube's faces into the surface of the slot `keep` gives it,
/// as a section's near detail is drawn a part of it at a time (`sections::CLUSTER`).
#[allow(clippy::too_many_arguments)]
pub fn draw_slots(
    cubes: &BTreeSet<[i32; 3]>,
    keep: &dyn Fn(IVec3) -> Option<usize>,
    kin: &dyn Fn(IVec3) -> bool,
    hides: &dyn Fn(IVec3) -> bool,
    look: &Look,
    heights: &Heights,
    cuts: &Cuts,
    colour: [f32; 3],
    outs: &mut [Surface],
    tops: &mut [Surface],
    mut caching: Option<Caching>,
) {
    let cut = cuts.at;
    let grid = Grid::around(cubes);
    // Kept faces are known by everything round them for three cubes.
    let asked = if caching.is_some() { 3 } else { 1 };
    // Which cells are cubes drawn, and whether each cell round them is full, asked of each
    // once: `hides` reads the base.
    let mut member = vec![false; grid.len()];
    let mut fill = vec![UNKNOWN; grid.len()];
    for &cube in cubes {
        if let Some(i) = grid.index(IVec3::from_array(cube)) {
            member[i] = true;
            fill[i] = FULL;
        }
    }
    for &cube in cubes {
        let cube = IVec3::from_array(cube);
        for z in -asked..=asked {
            for y in -asked..=asked {
                for x in -asked..=asked {
                    let at = cube + IVec3::new(x, y, z);
                    if let Some(i) = grid.index(at)
                        && fill[i] == UNKNOWN
                    {
                        fill[i] = if hides(at) { FULL } else { EMPTY };
                    }
                }
            }
        }
    }
    // Whether each cube drawn is cut, and what its cut is from, asked of each once: `cuts`
    // reads the base.
    let mut cut_of = vec![false; grid.len()];
    let mut cut_keys: Vec<Option<u64>> = vec![None; grid.len()];
    for &cube in cubes {
        let cube = IVec3::from_array(cube);
        if let Some(i) = grid.index(cube)
            && (cuts.of)(cube)
        {
            cut_of[i] = true;
            cut_keys[i] = (cuts.key)(cube);
        }
    }
    let is_cut = |at: IVec3| match grid.index(at) {
        Some(i) => cut_of[i],
        None => (cuts.of)(at),
    };
    let is_member = |at: IVec3| grid.index(at).is_some_and(|i| member[i]);
    let full = |at: IVec3| match grid.index(at) {
        Some(i) if fill[i] != UNKNOWN => fill[i] == FULL,
        Some(i) => member[i] || hides(at),
        None => hides(at),
    };
    let n = look.steps.max(1) as usize;
    // Each cube shrunk by the bevel on every side nothing hides, but for a top cut away over
    // its middle, as a gable's top is at its roof or a slope's at its ramp: what is left
    // ends at the cut, sawn straight, not rounded off at every cube along it.
    let mut shrunk: Vec<(Vec3, Vec3)> = vec![(Vec3::ZERO, Vec3::ZERO); grid.len()];
    for &cube in cubes {
        let cube = IVec3::from_array(cube);
        let mut low = cube.as_vec3() * CELL_M;
        let mut high = low + Vec3::splat(CELL_M);
        let cut_top = is_cut(cube)
            && cut(cube, low + Vec3::new(CELL_M * 0.5, CELL_M, CELL_M * 0.5))[1] < high.y - 1e-4;
        for axis in 0..3 {
            if !full(cube - step(axis, 1)) {
                low[axis] += look.bevel;
            }
            if !full(cube + step(axis, 1)) && !(axis == 1 && cut_top) {
                high[axis] -= look.bevel;
            }
        }
        if let Some(i) = grid.index(cube) {
            shrunk[i] = (low, high);
        }
    }
    // Every inside edge: a cube's edge with three of the four cubes round it full, kept by
    // the cell its middle is in (a list a cell, linked through `edges`), so a point looks
    // only at those near it.
    let mut first_edge = vec![u32::MAX; grid.len()];
    let mut edges: Vec<(u32, (Vec3, Vec3))> = Vec::new();
    for &cube in cubes {
        let cube = IVec3::from_array(cube);
        for axis in 0..3 {
            let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
            for (da, db) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                // The edge along `axis` at this corner of the cube, and the four cubes round
                // it.
                let corner = cube + step(a, da) + step(b, db);
                let round = [(0, 0), (-1, 0), (0, -1), (-1, -1)]
                    .map(|(ea, eb)| full(corner + step(a, ea) + step(b, eb)));
                if round.iter().filter(|&&f| f).count() != 3 {
                    continue;
                }
                let from = corner.as_vec3() * CELL_M;
                let to = from + unit(axis, CELL_M);
                let cell = ((from + to) * 0.5 / CELL_M).floor().as_ivec3();
                let Some(slot) = grid.index(cell) else {
                    continue;
                };
                let mut at = first_edge[slot];
                let mut known = false;
                while at != u32::MAX {
                    let (next, edge) = edges[at as usize];
                    if edge == (from, to) {
                        known = true;
                        break;
                    }
                    at = next;
                }
                if !known {
                    edges.push((first_edge[slot], (from, to)));
                    first_edge[slot] = (edges.len() - 1) as u32;
                }
            }
        }
    }
    // The inside edges near a cube, which a point of its faces may be near: those further
    // off are further than the fade from it.
    let near_inside = |cube: IVec3, found: &mut Vec<(Vec3, Vec3)>| {
        found.clear();
        for z in -1..=2 {
            for y in -1..=2 {
                for x in -1..=2 {
                    let Some(slot) = grid.index(cube + IVec3::new(x, y, z)) else {
                        continue;
                    };
                    let mut at = first_edge[slot];
                    while at != u32::MAX {
                        let (next, edge) = edges[at as usize];
                        found.push(edge);
                        at = next;
                    }
                }
            }
        }
    };
    let fade = |p: Vec3, edges: &[(Vec3, Vec3)]| {
        edges
            .iter()
            .map(|&(from, to)| {
                let along = (to - from).normalize();
                let t = (p - from).dot(along).clamp(0.0, CELL_M);
                (p.distance(from + along * t) / INSIDE_FADE_M).min(1.0)
            })
            .fold(1.0_f32, f32::min)
    };
    // What a cube's faces are worked out from, as two numbers: where their points are, from
    // which cubes are drawn and full two round it, and how those drawn are cut; and their
    // normals along their rims, which take the faces beside them, from those three round
    // it. Each a box of cells' numbers XORed together (Zobrist hashing, as chess engines and
    // voxel meshers key a neighbourhood), read in a few steps from the grid's running XOR
    // rather than cell by cell: a cube's is asked of every cube with faces. A cube cut with
    // no number for its cut is not kept.
    let windows = (caching.is_some()).then(|| {
        let state = |i: usize| u64::from(member[i]) << 1 | u64::from(fill[i] == FULL);
        // By where the cell is, not its place in the grid, which an edit may move.
        let zobrist = |i: usize, what: u64| {
            let i = i as i32;
            let at = grid.low
                + IVec3::new(
                    i % grid.size.x,
                    i / grid.size.x % grid.size.y,
                    i / (grid.size.x * grid.size.y),
                );
            crate::layout::splitmix(
                u64::from(at.x as u32)
                    ^ u64::from(at.y as u32).rotate_left(21)
                    ^ u64::from(at.z as u32).rotate_left(42)
                    ^ what.rotate_left(60),
            )
        };
        let cells = grid.len();
        let states: Vec<u64> = (0..cells).map(|i| zobrist(i, state(i) + 1)).collect();
        let cuts: Vec<u64> = (0..cells)
            .map(|i| match (member[i] && cut_of[i], cut_keys[i]) {
                (true, Some(key)) => crate::layout::splitmix(zobrist(i, 7) ^ key),
                _ => 0,
            })
            .collect();
        let unkeyed: Vec<u64> = (0..cells)
            .map(|i| u64::from(member[i] && cut_of[i] && cut_keys[i].is_none()))
            .collect();
        [states, cuts, unkeyed].map(|values| Running::of(&grid, &values))
    });
    let signature = |cube: IVec3| -> Option<Signature> {
        let [states, cuts, unkeyed] = windows.as_ref()?;
        if unkeyed.sum(&grid, cube, 2) != 0 {
            return None;
        }
        let cut = cuts.xor(&grid, cube, 2);
        let beds = heights
            .bed_key
            .map_or(0, |bed_key| crate::layout::splitmix(bed_key(cube) ^ 0x5bed))
            ^ heights
                .holed
                .and_then(|holed| holed(cube))
                .map_or(0, |key| crate::layout::splitmix(key ^ 0x401e));
        Some(Signature {
            shape: states.xor(&grid, cube, 2) ^ cut ^ beds,
            whole: states.xor(&grid, cube, 3) ^ cut ^ beds ^ 0x3,
        })
    };
    // Where a cube's faces' points fall across it along an axis, as fractions of a cube: a
    // cube cut, or with a piece bedded near it, evenly at its steps, as its
    // shape there needs; any other only where its shape bends (`across`), its stone flat
    // between, its relief left to its maps. A cube's, whichever of its faces asks, so the
    // faces round a line agree on it.
    let even: Vec<f32> = (0..=n).map(|i| i as f32 / n as f32).collect();
    // Holed by damage, finer than its steps (`HOLED_FINER`), so a stone's hole is found.
    let fine: Vec<f32> = {
        let m = HOLED_FINER * n;
        (0..=m).map(|i| i as f32 / m as f32).collect()
    };
    let fine_step = CELL_M / (HOLED_FINER * n) as f32;
    // Round a bed, at half its steps: the wall's lap over a lintel's edge every 10 cm near.
    let half: Vec<f32> = {
        let m = n.div_ceil(2).max(1);
        (0..=m).map(|i| i as f32 / m as f32).collect()
    };
    // Where a cube's cut, a roof's underside or a shape's top, crosses its top or its foot
    // along an axis across it, as fractions of the cube, read along its four edges that
    // way: a cut runs straight across a cube, so points there and at its corners follow it
    // exactly (D284).
    let crossings = |c: IVec3, axis: usize| -> Vec<f32> {
        let low = c.as_vec3() * CELL_M;
        let other = if axis == 0 { 2 } else { 0 };
        let mut out = Vec::new();
        for o in [0.0, 1.0] {
            let at = |t: f32| {
                let mut p = low + Vec3::Y * CELL_M * 0.5;
                p[axis] += t * CELL_M;
                p[other] += o * CELL_M;
                cut(c, p)
            };
            // Read at every step, for where it bends (a gable's apex, a hip's crease) as
            // well as where it crosses.
            let samples: Vec<[f32; 2]> = (0..=n).map(|i| at(i as f32 / n as f32)).collect();
            for side in 0..2 {
                let h: Vec<f32> = samples.iter().map(|s| s[side]).collect();
                if h.iter().any(|v| v.abs() > 1e30) {
                    continue;
                }
                for i in 0..n {
                    let (from, to) = (h[i], h[i + 1]);
                    for level in [low.y, low.y + CELL_M] {
                        if (from - level) * (to - level) < 0.0 {
                            let t = (i as f32 + (level - from) / (to - from)) / n as f32;
                            // Not where a corner or another point already is, a sliver away.
                            if t > 0.02 && t < 0.98 {
                                out.push(t);
                            }
                        }
                    }
                    if i > 0 && (h[i - 1] - 2.0 * h[i] + h[i + 1]).abs() > 1e-4 {
                        out.push(i as f32 / n as f32);
                    }
                }
            }
        }
        out
    };
    // Whether a piece is bedded where it changes a cube's faces: each face read on a grid
    // a quarter of a cube apart, no point of it more than 9 cm from one,
    // first at its corners and middle to pass over the many faces nowhere near a piece.
    let bedded_near = |c: IVec3| {
        heights.bed.is_some_and(|bed| {
            let low = c.as_vec3() * CELL_M;
            let at = |axis: usize, side: f32, u: f32, v: f32| {
                let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
                let mut p = low;
                p[axis] += if side > 0.0 { CELL_M } else { 0.0 };
                p[a] += u * CELL_M;
                p[b] += v * CELL_M;
                p
            };
            let within = |p: Vec3, reach: f32| bed(p).is_some_and(|(inside, ..)| inside > -reach);
            FACES.iter().any(|&(axis, side)| {
                let near = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0], [0.5, 0.5]]
                    .iter()
                    .any(|&[u, v]| within(at(axis, side, u, v), BED_LAP_M + CELL_M * 0.5));
                near && (0..=4).any(|i| {
                    (0..=4).any(|j| {
                        let p = at(axis, side, i as f32 / 4.0, j as f32 / 4.0);
                        within(p, BED_LAP_M + 0.09)
                    })
                })
            })
        })
    };
    // Whether a cube that may be cut is: its cut passes through it somewhere over its top's
    // corners or middle.
    let really_cut = |c: IVec3| {
        is_cut(c) && {
            let low = c.as_vec3() * CELL_M;
            [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0], [0.5, 0.5]]
                .iter()
                .any(|&[u, v]| {
                    let p = low + Vec3::new(u, 0.5, v) * CELL_M;
                    let [under, over] = cut(c, p);
                    over < low.y + CELL_M - 1e-4 || under > low.y + 1e-4
                })
        }
    };
    // Along each edge of a cube where the two cubes across its faces are drawn into the same
    // surface and the one diagonally across it is not full, an inside edge's corner cube:
    // the edge bitten out of its shrunk box a bevel round, a bit an edge (`Rounded`), so the
    // box's corner, unshrunk both ways, does not hold the surface out past the inside edge,
    // and where the two outside edges either side of it meet a face, the three round into
    // one another. Not a cube cut, whose top stays on its cut as it is.
    let mut bitten: Vec<u16> = vec![0; grid.len()];
    for &cube in cubes {
        let cube = IVec3::from_array(cube);
        let Some(i) = grid.index(cube) else {
            continue;
        };
        if really_cut(cube) {
            continue;
        }
        for axis in 0..3 {
            let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
            for (da, db) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (across_a, across_b) = (step(a, 2 * da - 1), step(b, 2 * db - 1));
                if kin(cube + across_a) && kin(cube + across_b) && !full(cube + across_a + across_b)
                {
                    bitten[i] |= 1 << bite_bit(axis, da, db);
                }
            }
        }
    }
    let shrunk_at = |at: IVec3| {
        grid.index(at).filter(|&i| member[i]).map(|i| Rounded {
            low: shrunk[i].0,
            high: shrunk[i].1,
            cube: at.as_vec3() * CELL_M,
            bites: bitten[i],
        })
    };
    // Asked only of a cube itself, never of those round it, so a section and its neighbour,
    // each knowing the cubes a step past its edge and no further, agree on the cubes they
    // share. A side against a cut cube follows the cut's line between its corners, which a
    // slope's or a roof's underside is, straight across a cube.
    // Whether a cube's side toward another is open as far as where its points fall goes:
    // but against a cube drawn into the same surface. Read from `kin` alone, the same in
    // every call that draws into the surface; open where it is not quite (against another
    // kind's cube, say) only adds points where none are needed.
    let open = |c: IVec3| !kin(c);
    let worked_out = |c: IVec3| -> [Arc<[f32]>; 3] {
        let cut = really_cut(c);
        // Earth's packing, cut, follows its lumps up to the cut: evenly, as a bed's lap does.
        if look.packed.is_some() && cut {
            let even: Arc<[f32]> = even.clone().into();
            return [even.clone(), even.clone(), even];
        }
        if heights.holed.is_some_and(|holed| holed(c).is_some()) {
            let fine: Arc<[f32]> = fine.clone().into();
            return [fine.clone(), fine.clone(), fine];
        }
        if bedded_near(c) {
            let half: Arc<[f32]> = half.clone().into();
            return [half.clone(), half.clone(), half];
        }
        [0, 1, 2].map(|axis| {
            let mut out = across(open(c - step(axis, 1)), open(c + step(axis, 1)), look);
            if axis != 1 && cut {
                // The cut's crossings both ways across, each mirrored, and its middle: a
                // corner's hip or valley crosses its top on a diagonal, bending the cut
                // between its edges (`roofs::gable_cut`), and with the same points both ways
                // every square it crosses has its corners on it, to split along it.
                for t in crossings(c, 0).into_iter().chain(crossings(c, 2)) {
                    out.extend([t, 1.0 - t]);
                }
                out.push(0.5);
                out.sort_by(f32::total_cmp);
                out.dedup_by(|x, y| (*x - *y).abs() < 0.02);
            }
            out.into()
        })
    };
    // Each cube's, worked out once: the faces round a line each ask for the four cubes'.
    let known: std::cell::RefCell<QuickMap<IVec3, [Arc<[f32]>; 3]>> = Default::default();
    let grid_of = |c: IVec3, axis: usize| -> Arc<[f32]> {
        if let Some(found) = known.borrow().get(&c) {
            return found[axis].clone();
        }
        let made = worked_out(c);
        let out = made[axis].clone();
        known.borrow_mut().insert(c, made);
        out
    };
    let call = caching.as_ref().map_or(0, |caching| caching.call);
    let old = caching.as_ref().and_then(|caching| caching.old);
    // Each face drawn: a grid of points, placed once each, and its triangles' corners in it.
    let mut faces: Vec<Face> = Vec::new();
    let mut cube_edges: Vec<(Vec3, Vec3)> = Vec::new();
    let mut face_edges: Vec<(Vec3, Vec3)> = Vec::new();
    let mut round: Vec<Rounded> = Vec::with_capacity(27);
    let mut flat: Vec<Vec3> = Vec::new();
    for &cube in cubes {
        let cube = IVec3::from_array(cube);
        let slot = keep(cube);
        let keeping = slot.is_some();
        let slot = slot.unwrap_or(0);
        // A cube left out matters only where it meets one kept, a step away.
        if !keeping
            && !NEIGHBOURHOOD
                .iter()
                .any(|&offset| keep(cube + offset).is_some())
        {
            continue;
        }
        let low = cube.as_vec3() * CELL_M;
        // The inside edges round it, looked for once a face of it is worked out anew.
        let mut edges_found = false;
        let own_box = shrunk_at(cube).unwrap_or(Rounded {
            low,
            high: low + Vec3::splat(CELL_M),
            cube: low,
            bites: 0,
        });
        let cut_here = is_cut(cube);
        let signed = if caching.is_some() {
            signature(cube)
        } else {
            None
        };
        for (index, (axis, side)) in FACES.into_iter().enumerate() {
            let out_step = step(axis, side as i32);
            let beyond = cube + out_step;
            let normal = unit(axis, side);
            let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
            let plane = if side > 0.0 {
                low[axis] + CELL_M
            } else {
                low[axis]
            };
            let point = |i: usize, j: usize| {
                let mut p = Vec3::ZERO;
                p[axis] = plane;
                p[a] = low[a] + CELL_M * i as f32 / n as f32;
                p[b] = low[b] + CELL_M * j as f32 / n as f32;
                p
            };
            // Against anything full that is not one of these cubes cut, nothing shows.
            if full(beyond) && !(axis != 1 && is_member(beyond) && is_cut(beyond)) {
                continue;
            }
            // A face kept from last time was drawn from the same cuts round it, so shows as
            // it did: whether it shows against a cut neighbour is not asked again.
            // Its points as they were last time, if nothing two round it has changed, but its
            // rims' normals to be worked out again from the faces beside it.
            if let (Some(signed), Some(old)) = (signed, old)
                && let Some(data) = old.faces.get(&(call, cube, index as u8))
                && data.signature.is_some_and(|was| was.shape == signed.shape)
                && data.signature.is_some_and(|was| was.whole != signed.whole)
            {
                let mut data = FaceData::clone(data);
                data.signature = Some(signed);
                faces.push(Face {
                    data: Arc::new(data),
                    keeping,
                    slot,
                    cube,
                    index: index as u8,
                    fresh: true,
                    rims_only: true,
                });
                continue;
            }
            // As it was drawn last time, if nothing round it has changed since.
            if let (Some(signed), Some(old)) = (signed, old)
                && let Some(data) = old.faces.get(&(call, cube, index as u8))
                && data.signature == Some(signed)
            {
                faces.push(Face {
                    data: data.clone(),
                    keeping,
                    slot,
                    cube,
                    index: index as u8,
                    fresh: false,
                    rims_only: false,
                });
                continue;
            }
            // A side against another of the cubes drawn shows where that one is cut lower
            // than this one, as a slope's side shows over a half beside it: from the other's
            // cut up to this one's. Against anything else full, nothing shows; nor against
            // a cube not cut at all, which is never lower.
            let against_cut = axis != 1
                && is_member(beyond)
                && is_cut(beyond)
                && (0..=n).any(|i| {
                    (0..=n).any(|j| {
                        let p = point(i, j);
                        cut(beyond, p)[1] < cut(cube, p)[1].min(p.y) - 1e-4
                    })
                });
            if full(beyond) && !against_cut {
                continue;
            }
            // Where its points are across it, as fractions of a cube each way (`grid_of`),
            // and along each of its sides every point any face on that side's line has, so
            // faces meet point for point with no crack: each side's line is shared by the
            // four cubes round it, and all of them take the points of all four.
            let along_a = grid_of(cube, a);
            let along_b = grid_of(cube, b);
            let (na, nb) = (along_a.len() - 1, along_b.len() - 1);
            let line = |along: usize, over: usize, toward: i32| -> Vec<f32> {
                let mut all = Vec::new();
                for x in [
                    cube,
                    cube + step(over, toward),
                    beyond,
                    beyond + step(over, toward),
                ] {
                    if kin(x) {
                        all.extend(grid_of(x, along).iter().copied());
                    }
                }
                all.sort_by(f32::total_cmp);
                all.dedup_by(|x, y| (*x - *y).abs() < 1e-5);
                all
            };
            let missing = |line: Vec<f32>, own: &[f32]| -> Vec<f32> {
                line.into_iter()
                    .filter(|&t| own.iter().all(|&o| (o - t).abs() >= 1e-5))
                    .collect()
            };
            // The points each side lacks of its line's: along `a` on the sides at `b`'s low
            // and high edges, along `b` on those at `a`'s.
            let more = [
                missing(line(a, b, -1), &along_a),
                missing(line(a, b, 1), &along_a),
                missing(line(b, a, -1), &along_b),
                missing(line(b, a, 1), &along_b),
            ];
            let param = |u: f32, v: f32| {
                let mut p = Vec3::ZERO;
                p[axis] = plane;
                p[a] = low[a] + CELL_M * u;
                p[b] = low[b] + CELL_M * v;
                p
            };
            let point = |i: usize, j: usize| param(along_a[i], along_b[j]);
            // Only the boxes that may be nearest a point of this face: none further from
            // the face than its own cube's box can be, at most the bevel along every axis.
            let (face_low, face_high) = {
                let (mut l, mut h) = (low, low + Vec3::splat(CELL_M));
                l[axis] = plane;
                h[axis] = plane;
                (l, h)
            };
            let reach = look.bevel * look.bevel * 3.0 + 1e-6;
            round.clear();
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        let Some(rounded) = shrunk_at(cube + IVec3::new(x, y, z)) else {
                            continue;
                        };
                        let (l, h) = (rounded.low, rounded.high);
                        let gap = (l - face_high).max(face_low - h).max(Vec3::ZERO);
                        if gap.length_squared() <= reach {
                            round.push(rounded);
                        }
                    }
                }
            }
            if !edges_found {
                near_inside(cube, &mut cube_edges);
                edges_found = true;
            }
            // Only the inside edges that come within the fade of this face: the rest fade
            // nothing anywhere on it.
            face_edges.clear();
            face_edges.extend(cube_edges.iter().copied().filter(|&(from, to)| {
                let gap = (from.min(to) - face_high)
                    .max(face_low - from.max(to))
                    .max(Vec3::ZERO);
                gap.length() < INSIDE_FADE_M
            }));
            // Everything its points are placed from, as a number: where they fall across it,
            // the boxes they are rounded to, the inside edges that fade them, its cut and
            // its neighbour's, and the pieces bedded near. A face whose cubes round it
            // changed, but not anything it is placed from, as most two cubes from an edit,
            // keeps its points and has only its rims' normals worked out again.
            let placed = signed.map(|_| {
                let mut hash = crate::mesh::Corners::default();
                let mut floats = |values: &[f32]| {
                    hash.write_usize(values.len());
                    for v in values {
                        hash.write_u32(v.to_bits());
                    }
                };
                floats(&along_a);
                floats(&along_b);
                for more in &more {
                    floats(more);
                }
                for rounded in round.iter().chain([&own_box]) {
                    floats(&rounded.low.to_array());
                    floats(&rounded.high.to_array());
                    floats(&[f32::from(rounded.bites)]);
                }
                for (l, h) in &face_edges {
                    floats(&l.to_array());
                    floats(&h.to_array());
                }
                hash.write_u8(u8::from(against_cut));
                for c in [cube, beyond] {
                    if let Some(i) = grid.index(c)
                        && cut_of[i]
                    {
                        hash.write_u64(cut_keys[i].unwrap_or(u64::MAX));
                    }
                }
                if let Some(bed_key) = heights.bed_key {
                    hash.write_u64(bed_key(cube));
                }
                if let Some(key) = heights.holed.and_then(|holed| holed(cube)) {
                    hash.write_u64(key);
                }
                hash.finish()
            });
            if let (Some(signed), Some(old), Some(placed)) = (signed, old, placed)
                && let Some(data) = old.faces.get(&(call, cube, index as u8))
                && data.placed == Some(placed)
            {
                let mut data = FaceData::clone(data);
                data.signature = Some(signed);
                faces.push(Face {
                    data: Arc::new(data),
                    keeping,
                    slot,
                    cube,
                    index: index as u8,
                    fresh: true,
                    rims_only: true,
                });
                continue;
            }
            // A point further than the bevel from every edge of a face open to the air is
            // nearer its own cube's box than any other's: every other cube's is beyond the
            // face's edges, or behind it.
            let margin = look.bevel + 1e-3;
            let inner = |p: Vec3| {
                !against_cut
                    && p[a] - face_low[a] > margin
                    && face_high[a] - p[a] > margin
                    && p[b] - face_low[b] > margin
                    && face_high[b] - p[b] > margin
            };
            // Holed by damage: where the hole is, and each point beside it moved onto its
            // outline, so the hole's rim runs along its stones' edges rather than stepping
            // round the grid's squares, as a contour is marched (marching squares): along
            // each line of the grid from a point left standing to one gone, the place it
            // crosses the outline, found by halving. A point inside the face takes the
            // middle of its crossings, or the nearest where the middle falls in the hole;
            // one on the face's side slides along that side only, the same from the face
            // beside it, and only where the wall runs on flat past it, so faces still meet.
            let holing = axis != 1
                && heights.hole.is_some()
                && heights.holed.is_some_and(|holed| holed(cube).is_some());
            let hole_raw = |p: Vec3| heights.hole.map_or(0.0, |hole| hole(p, normal));
            // Read once at each point of the fine grid and one round it, which every point
            // and its neighbours fall on; between them, as asked.
            let fine_n = HOLED_FINER * n;
            let lattice: Vec<f32> = if holing {
                (0..(fine_n + 3) * (fine_n + 3))
                    .map(|k| {
                        let (i, j) = (k % (fine_n + 3), k / (fine_n + 3));
                        let mut p = Vec3::ZERO;
                        p[axis] = plane;
                        p[a] = low[a] + (i as f32 - 1.0) * fine_step;
                        p[b] = low[b] + (j as f32 - 1.0) * fine_step;
                        hole_raw(p)
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let on_lattice = |p: Vec3| -> Option<usize> {
                let (u, v) = ((p[a] - low[a]) / fine_step, (p[b] - low[b]) / fine_step);
                let (i, j) = (u.round(), v.round());
                (!lattice.is_empty()
                    && (u - i).abs() < 1e-3
                    && (v - j).abs() < 1e-3
                    && (-1.0..=fine_n as f32 + 1.0).contains(&i)
                    && (-1.0..=fine_n as f32 + 1.0).contains(&j))
                .then(|| (j as usize + 1) * (fine_n + 3) + i as usize + 1)
            };
            let hole_at = |p: Vec3| on_lattice(p).map_or_else(|| hole_raw(p), |at| lattice[at]);
            let sunk_at = |p: Vec3| hole_at(p) > 1e-4;
            // How far a point is pushed back: as deep as its hole where it has gone; and a
            // speck left standing in a hole, too small for the grid to draw but as a spike,
            // goes with it: a point of the grid with no more than two of the eight round it
            // standing.
            let depth_of = |p: Vec3| -> f32 {
                let here = hole_at(p);
                if here > 1e-4 || !holing || on_lattice(p).is_none() {
                    return here;
                }
                let (mut standing, mut deepest) = (0, 0.0f32);
                for (da, db) in [
                    (1, 0),
                    (1, 1),
                    (0, 1),
                    (-1, 1),
                    (-1, 0),
                    (-1, -1),
                    (0, -1),
                    (1, -1),
                ] {
                    let mut q = p;
                    q[a] += da as f32 * fine_step;
                    q[b] += db as f32 * fine_step;
                    let there = hole_at(q);
                    if there <= 1e-4 {
                        standing += 1;
                        if standing > 2 {
                            return here;
                        }
                    }
                    deepest = deepest.max(there);
                }
                deepest
            };
            let deep = |p: Vec3| depth_of(p) > 1e-4;
            let on_side = |p: Vec3, k: usize| -> Option<i32> {
                if (p[k] - face_low[k]).abs() < 1e-5 {
                    Some(-1)
                } else if (face_high[k] - p[k]).abs() < 1e-5 {
                    Some(1)
                } else {
                    None
                }
            };
            let runs_on = |k: usize, toward: i32| {
                kin(cube + step(k, toward)) && !full(beyond + step(k, toward))
            };
            let snap = |p: Vec3| -> Vec3 {
                if !holing || deep(p) {
                    return p;
                }
                let (ways, reach): (&[usize], f32) = match (on_side(p, a), on_side(p, b)) {
                    (None, None) => (&[a, b], 1.0),
                    (Some(toward), None) if runs_on(a, toward) => (&[b], RIM_SLIDE),
                    (None, Some(toward)) if runs_on(b, toward) => (&[a], RIM_SLIDE),
                    _ => return p,
                };
                let (mut sum, mut count, mut nearest) = (Vec3::ZERO, 0.0, None::<Vec3>);
                for &k in ways {
                    for sign in [-1.0f32, 1.0] {
                        let mut d = Vec3::ZERO;
                        d[k] = sign * fine_step;
                        if !deep(p + d) {
                            continue;
                        }
                        // Between the two, on the outline itself, a speck aside.
                        let (mut standing, mut gone) = (0.0f32, 1.0f32);
                        for _ in 0..4 {
                            let middle = (standing + gone) * 0.5;
                            if sunk_at(p + d * middle) {
                                gone = middle;
                            } else {
                                standing = middle;
                            }
                        }
                        let crossing = p + d * standing.min(reach);
                        sum += crossing;
                        count += 1.0;
                        if nearest
                            .is_none_or(|n| n.distance_squared(p) > crossing.distance_squared(p))
                        {
                            nearest = Some(crossing);
                        }
                    }
                }
                let Some(nearest) = nearest else {
                    return p;
                };
                let middle = sum / count;
                if sunk_at(middle) { nearest } else { middle }
            };
            let own = [own_box];
            let place = |p: Vec3| -> (Vec3, Vec3) {
                let boxes: &[Rounded] = if inner(p) { &own } else { &round };
                let nearest = boxes
                    .iter()
                    .map(|rounded| rounded.nearest(p, look.bevel))
                    .min_by(|a, b| a.distance_squared(p).total_cmp(&b.distance_squared(p)))
                    .unwrap_or(p);
                let away = p - nearest;
                let length = away.length();
                let (rounded, outward) = if length > 1e-6 {
                    let outward = away / length;
                    (nearest + outward * look.bevel, outward)
                } else {
                    (p, normal)
                };
                let faded = if face_edges.is_empty() {
                    1.0
                } else {
                    fade(p, &face_edges)
                };
                // A cube cut to a roof keeps to the roof's line: its lifts ease out of their steps
                // toward the cut.
                let stepping = if cut_here {
                    ((cut(cube, p)[1] - p.y - 0.05) / 0.15).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let packed = look
                    .packed
                    .map_or(0.0, |packed| packed.at(p, outward, stepping));
                // Flat but for its packing: its stone's relief is drawn from its maps.
                let relief = 0.0;
                let mut out = (relief + packed) * faded;
                // Sunk behind a piece set into it, a little deeper than its deepest relief,
                // and built up over its edges round its outline.
                if let Some((inside, face, lapped)) = heights.bed.and_then(|bed| bed(p))
                    && inside > -BED_LAP_M
                {
                    // How far over the piece's edge the wall is built here, by its own
                    // relief, so the edge it leaves is as uneven as the wall.
                    let high = (relief / look.relief.max(1e-4) + 0.5).clamp(0.0, 1.0);
                    let reach = BED_LAP_M * (0.25 + high);
                    let over = ((reach - inside) / 0.01).clamp(0.0, 1.0)
                        * (1.0 + inside / BED_LAP_M).clamp(0.0, 1.0);
                    let sunk = ((inside - reach) / 0.015).clamp(0.0, 1.0);
                    let deepest = -(look.relief * 0.5) - packed.abs() - BED_DEPTH_M;
                    out += (deepest - out) * sunk;
                    // Built up only where it laps over the edge: elsewhere a floor of
                    // nothing would undo the sinking, leaving the wall in the plane of a
                    // flush piece (a lintel's underside in its opening's head).
                    if lapped && over > 0.0 {
                        out = out.max(over * (face + BED_OVER_M));
                    }
                }
                // Pushed back where damage has taken its stones out.
                out -= if holing { depth_of(p) } else { hole_raw(p) };
                // Its normal the rounding's where it is no more than rounded, the same
                // worked out from any face; else from its triangles (`ZERO`).
                let exact = if out.abs() < 1e-7 {
                    outward
                } else {
                    Vec3::ZERO
                };
                (rounded + outward * out, exact)
            };
            let sunk = std::cell::RefCell::new(Vec::<bool>::new());
            // Wound so each triangle faces out along the face's normal.
            let flip = side < 0.0;
            let top = axis == 1 && side > 0.0 && look.top.is_some();
            let metres = match look.top {
                Some((metres, _)) if top => metres,
                _ => look.metres,
            };
            let mut face = FaceData {
                signature: signed,
                placed,
                points: Vec::with_capacity((na + 1) * (nb + 1)),
                rim: Vec::with_capacity((na + 1) * (nb + 1)),
                exact: Vec::with_capacity((na + 1) * (nb + 1)),
                uvs: Vec::with_capacity((na + 1) * (nb + 1)),
                triangles: Vec::with_capacity(na * nb * 2),
                top,
                local: Vec::new(),
                normals: Vec::new(),
                order: Vec::new(),
                indices: Vec::new(),
            };
            // Each point cut to the heights there, and where it would be unrounded, to
            // choose each square's diagonal by; a cube not cut is all there.
            let clipped = |p: Vec3| {
                if !cut_here {
                    return p.y;
                }
                let [low, high] = cut(cube, p);
                p.y.max(low).min(high)
            };
            flat.clear();
            // A point of the face added: placed, where it would be unrounded, and textured.
            let add = |p: Vec3, rim: bool, face: &mut FaceData, flat: &mut Vec<Vec3>| {
                let [low, high] = if cut_here {
                    cut(cube, p)
                } else {
                    [f32::MIN, f32::MAX]
                };
                // A side is cut before it is rounded and displaced, and textured where the
                // point ends up, so the texture runs on unstretched to the cut and points cut
                // to the same place stay one.
                let mut s = if axis == 1 || !cut_here {
                    p
                } else {
                    p.with_y(p.y.max(low).min(high))
                };
                if against_cut {
                    s.y = s.y.max(cut(beyond, p)[1].min(high));
                }
                let s = snap(s);
                let (mut q, mut exact) = place(s);
                sunk.borrow_mut().push(holing && deep(s));
                let placed = q;
                if cut_here {
                    q.y = q.y.max(low).min(high);
                    // And a side's where it ends up, so nothing a lump or the rounding moves
                    // along stands over the cut. Not a top's: its rounding and lumps, brought
                    // onto a sloping cut where they end up, fold over themselves in the cut;
                    // a top's point they would carry over the cut stays on it, as it was.
                    let [under, over] = cut(cube, q);
                    if axis != 1 {
                        q.y = q.y.max(under).min(over);
                    } else if q.y > over {
                        q = s.with_y(s.y.max(low).min(high));
                    } else if look.packed.is_some() {
                        q.y = q.y.max(under);
                    }
                }
                if against_cut {
                    q.y = q.y.max(cut(beyond, p)[1].min(high));
                }
                if q != placed {
                    exact = Vec3::ZERO;
                }
                face.points.push(q);
                face.exact.push(exact);
                face.rim.push(rim);
                flat.push(p.with_y(clipped(p)));
                face.uvs.push(planar(s, normal, metres));
                (face.points.len() - 1) as u32
            };
            for i in 0..=na {
                for j in 0..=nb {
                    let rim = i == 0 || j == 0 || i == na || j == nb;
                    add(point(i, j), rim, &mut face, &mut flat);
                }
            }
            // The points its sides lack of their lines', each with where it is along its side.
            let [bottom, top_side, left, right] = [0, 1, 2, 3].map(|s| {
                more[s]
                    .iter()
                    .map(|&t| {
                        let p = match s {
                            0 => param(t, 0.0),
                            1 => param(t, 1.0),
                            2 => param(0.0, t),
                            _ => param(1.0, t),
                        };
                        (t, add(p, true, &mut face, &mut flat))
                    })
                    .collect::<Vec<(f32, u32)>>()
            });
            let at = |i: usize, j: usize| (i * (nb + 1) + j) as u32;
            let within = |extra: &[(f32, u32)], from: f32, to: f32| -> Vec<(f32, u32)> {
                extra
                    .iter()
                    .copied()
                    .filter(|&(t, _)| t > from && t < to)
                    .collect()
            };
            for i in 0..na {
                for j in 0..nb {
                    // The points a cell on the face's rim takes from its neighbours' sides,
                    // round it in order: along its bottom, up its right, back along its top,
                    // down its left.
                    let (a0, a1, b0, b1) = (along_a[i], along_a[i + 1], along_b[j], along_b[j + 1]);
                    let mut ring: Vec<u32> = vec![at(i, j)];
                    if j == 0 {
                        ring.extend(within(&bottom, a0, a1).iter().map(|&(_, c)| c));
                    }
                    ring.push(at(i + 1, j));
                    if i + 1 == na {
                        ring.extend(within(&right, b0, b1).iter().map(|&(_, c)| c));
                    }
                    ring.push(at(i + 1, j + 1));
                    if j + 1 == nb {
                        ring.extend(within(&top_side, a0, a1).iter().rev().map(|&(_, c)| c));
                    }
                    ring.push(at(i, j + 1));
                    if i == 0 {
                        ring.extend(within(&left, b0, b1).iter().rev().map(|&(_, c)| c));
                    }
                    let triangle = |t: [u32; 3], face: &mut FaceData| {
                        let [x, y, z] = t;
                        let t = if flip { [x, z, y] } else { t };
                        let [pa, pb, pc] = t.map(|c| face.points[c as usize]);
                        let cross = (pb - pa).cross(pc - pa);
                        // Cut away to nothing, or folded back on itself where a cut or the
                        // rounding pinches a cell: drawn from inside, it would show dark. A
                        // smooth face's triangle turned more than 84° from its face is folded
                        // too, edge on in another face's plane (a foot's rounding pinched at
                        // an inner corner); its rounding's own steps turn no more than 75°.
                        // A hole's side, from its rim down to its floor, stands square to the
                        // face or near it, and is kept.
                        let side_of_hole = {
                            let sunk = sunk.borrow();
                            holing
                                && t.iter().any(|&c| sunk[c as usize])
                                && t.iter().any(|&c| !sunk[c as usize])
                        };
                        let folded = if look.packed.is_some() {
                            -0.5 * cross.length()
                        } else if side_of_hole {
                            -0.2 * cross.length()
                        } else {
                            0.1 * cross.length()
                        };
                        if cross.length_squared() < 1e-12 || cross.dot(normal) < folded {
                            return;
                        }
                        face.triangles.push(t);
                    };
                    if ring.len() > 4 {
                        // Its corners in the ring, and which of its sides took points.
                        let corner_at = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
                        let took = [
                            j == 0 && !within(&bottom, a0, a1).is_empty(),
                            i + 1 == na && !within(&right, b0, b1).is_empty(),
                            j + 1 == nb && !within(&top_side, a0, a1).is_empty(),
                            i == 0 && !within(&left, b0, b1).is_empty(),
                        ];
                        // A fan from a corner on neither side that took points, which every
                        // point round the cell sees across it, as a cell's taken points lie
                        // along its sides; failing one, from the cell's middle.
                        let apex = (0..4)
                            .find(|&k| !took[k] && !took[(k + 3) % 4])
                            .map(|k| corner_at[k]);
                        let (apex, open) = match apex {
                            Some(apex) => (apex, false),
                            None => {
                                let middle = add(
                                    param((a0 + a1) * 0.5, (b0 + b1) * 0.5),
                                    false,
                                    &mut face,
                                    &mut flat,
                                );
                                (middle, true)
                            }
                        };
                        let start = ring.iter().position(|&c| c == apex).unwrap_or(0);
                        let n_ring = ring.len();
                        for k in 0..n_ring {
                            let (x, y) =
                                (ring[(start + k) % n_ring], ring[(start + k + 1) % n_ring]);
                            if !open && (x == apex || y == apex) {
                                continue;
                            }
                            triangle([apex, x, y], &mut face);
                        }
                        continue;
                    }
                    let corners = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
                    // Split along the diagonal that follows the cut where it creases across
                    // the square, as along a corner slope's hip or valley: the one whose
                    // middle is nearer the cut surface at the square's middle.
                    let f = corners.map(|c| flat[c as usize]);
                    let middle = (f[0] + f[2]) * 0.5;
                    let middle = middle.with_y(clipped(point(i, j).lerp(point(i + 1, j + 1), 0.5)));
                    let along_02 = ((f[0] + f[2]) * 0.5).distance(middle);
                    let along_13 = ((f[1] + f[3]) * 0.5).distance(middle);
                    let mut split = if along_13 + 1e-5 < along_02 {
                        [[0, 1, 3], [1, 2, 3]]
                    } else {
                        [[0, 1, 2], [0, 2, 3]]
                    };
                    // Where the rounding has pinched the square into a dart, seen square to
                    // the face, along the diagonal from its reflex corner, which stays
                    // inside it: the other would lay one triangle over its neighbours and
                    // fold the other back.
                    let flat2 = corners.map(|c| {
                        let p = face.points[c as usize];
                        glam::Vec2::new(p[a], p[b])
                    });
                    let turn = |k: usize| {
                        let (p, q, r) = (flat2[(k + 3) % 4], flat2[k], flat2[(k + 1) % 4]);
                        (q - p).perp_dot(r - q)
                    };
                    let turns = [0, 1, 2, 3].map(turn);
                    let positive = turns.iter().filter(|t| **t > 0.0).count();
                    if positive == 1 || positive == 3 {
                        let odd = turns
                            .iter()
                            .position(|t| (*t > 0.0) == (positive == 1))
                            .unwrap_or(0);
                        split = if odd % 2 == 0 {
                            [[0, 1, 2], [0, 2, 3]]
                        } else {
                            [[0, 1, 3], [1, 2, 3]]
                        };
                    }
                    // Beside a hole, along the diagonal that keeps a corner on its own side
                    // of the outline: the rim runs between the two corners moved onto it.
                    if holing {
                        let sunk = sunk.borrow();
                        let gone = corners.map(|c| sunk[c as usize]);
                        let count = gone.iter().filter(|g| **g).count();
                        if count == 1 || count == 3 {
                            let odd = gone.iter().position(|g| *g == (count == 1)).unwrap_or(0);
                            split = if odd % 2 == 0 {
                                [[0, 1, 3], [1, 2, 3]]
                            } else {
                                [[0, 1, 2], [0, 2, 3]]
                            };
                        }
                    }
                    for [x, y, z] in split {
                        triangle([corners[x], corners[y], corners[z]], &mut face);
                    }
                }
            }
            faces.push(Face {
                data: Arc::new(face),
                keeping,
                slot,
                cube,
                index: index as u8,
                fresh: true,
                rims_only: false,
            });
        }
    }
    // Smooth across every edge the faces share: the normal at a point is the sum of its
    // triangles' around it. Inside a face that is its own triangles; on a face's edge,
    // where other faces meet it, every triangle with a corner there, found by place: each
    // face's own sum there is added to the place's, a face at a time. A face kept from last
    // time keeps its normals, as does every face round it, so only the places on the edges
    // of faces worked out anew are summed, from the faces beside them.
    let edge = |rim: &[bool], index: usize| rim[index];
    let key = |p: Vec3| ((p * 2048.0).round()).as_ivec3();
    for face in faces
        .iter_mut()
        .filter(|face| face.fresh && !face.rims_only)
    {
        let data = Arc::get_mut(&mut face.data).expect("a face worked out anew is its own");
        let mut local = vec![Vec3::ZERO; data.points.len()];
        for t in &data.triangles {
            let [pa, pb, pc] = t.map(|c| data.points[c as usize]);
            let normal = (pb - pa).cross(pc - pa);
            for &c in t {
                local[c as usize] += normal;
            }
        }
        data.local = local;
    }
    let mut slots: QuickMap<IVec3, u32> = QuickMap::default();
    let mut sums: Vec<Vec3> = Vec::new();
    let mut anew = vec![false; grid.len()];
    for face in faces.iter().filter(|face| face.fresh) {
        if let Some(i) = grid.index(face.cube) {
            anew[i] = true;
        }
        for (c, &p) in face.data.points.iter().enumerate() {
            if edge(&face.data.rim, c) {
                slots.entry(key(p)).or_insert_with(|| {
                    sums.push(Vec3::ZERO);
                    (sums.len() - 1) as u32
                });
            }
        }
    }
    let beside_anew = |cube: IVec3| {
        NEIGHBOURHOOD
            .iter()
            .any(|&offset| grid.index(cube + offset).is_some_and(|i| anew[i]))
    };
    for face in faces
        .iter()
        .filter(|face| face.fresh || beside_anew(face.cube))
    {
        let data = &face.data;
        for (c, &p) in data.points.iter().enumerate() {
            if edge(&data.rim, c)
                && let Some(&slot) = slots.get(&key(p))
            {
                sums[slot as usize] += data.local[c];
            }
        }
    }
    // Each face's normals, and its points in the order its triangles first come to them,
    // with its triangles' corners among them: what it puts into a surface sharing corners.
    for face in faces.iter_mut().filter(|face| face.fresh) {
        let data = Arc::get_mut(&mut face.data).expect("a face worked out anew is its own");
        let rim = data.rim.clone();
        let sum_at = |c: usize| {
            if edge(&rim, c) {
                sums[slots[&key(data.points[c])] as usize]
            } else {
                data.local[c]
            }
        };
        let mut normals = vec![Vec3::ZERO; data.points.len()];
        let mut index: Vec<u32> = vec![u32::MAX; data.points.len()];
        let (mut order, mut indices) = (Vec::new(), Vec::with_capacity(data.triangles.len() * 3));
        for t in &data.triangles {
            let [pa, pb, pc] = t.map(|c| data.points[c as usize]);
            let fallback = (pb - pa).cross(pc - pa).normalize_or_zero();
            for &c in t {
                let slot = &mut index[c as usize];
                if *slot == u32::MAX {
                    *slot = order.len() as u32;
                    order.push(c);
                    let exact = data.exact[c as usize];
                    normals[c as usize] = if exact != Vec3::ZERO {
                        exact
                    } else {
                        sum_at(c as usize).normalize_or(fallback)
                    };
                }
                indices.push(*slot);
            }
        }
        (data.normals, data.order, data.indices) = (normals, order, indices);
    }
    // Room for what is kept, at once.
    let mut room = vec![[0usize; 2]; outs.len() * 2];
    for face in faces.iter().filter(|face| face.keeping) {
        let room = &mut room[face.slot * 2 + usize::from(face.data.top)];
        room[0] += face.data.order.len();
        room[1] += face.data.indices.len();
    }
    for (slot, (out, top)) in outs.iter_mut().zip(tops.iter_mut()).enumerate() {
        for (into, [points, corners]) in [(out, room[slot * 2]), (top, room[slot * 2 + 1])] {
            if into.sharing {
                into.shared.reserve(points, corners);
            } else {
                into.reserve(corners);
            }
        }
    }
    for face in faces.iter().filter(|face| face.keeping) {
        let data = &face.data;
        let into = if data.top {
            &mut tops[face.slot]
        } else {
            &mut outs[face.slot]
        };
        if into.sharing {
            // Each point of the grid once, with the triangles' corners shared.
            let first = into.shared.positions.len() as u32;
            for &c in &data.order {
                let c = c as usize;
                into.shared
                    .corner(data.points[c], data.normals[c], data.uvs[c], colour);
            }
            into.shared
                .indices
                .extend(data.indices.iter().map(|&i| first + i));
            continue;
        }
        for t in &data.triangles {
            let corners = t.map(|c| data.points[c as usize]);
            let normals = t.map(|c| data.normals[c as usize]);
            into.textured(corners, normals, t.map(|c| data.uvs[c as usize]), colour);
        }
    }
    if let Some(caching) = caching.as_mut() {
        for face in faces
            .into_iter()
            .filter(|face| face.data.signature.is_some())
        {
            caching.new.taken += usize::from(!face.fresh);
            caching
                .new
                .faces
                .insert((caching.call, face.cube, face.index), face.data);
        }
    }
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
        assert!(
            !out.positions.chunks(3).any(|t| {
                side_on(t)
                    && t.iter().all(|p| (p.x - CELL_M).abs() < 0.05)
                    && t.iter().all(|p| p.y < CELL_M * 0.45)
            }),
            "a side inside the half"
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
