//! A square of grass as blades: where each grows, how tall, which way it bends and what
//! colour it is, after Ghost of Tsushima's procedural grass (Eric Wohllaib, GDC 2021).
//!
//! - **Blades, not tufts.** Each blade is placed on its own on a jittered grid, so a lawn of
//!   them covers the ground evenly instead of in spots.
//! - **Clumps.** Blades belong to the nearest of a scatter of clump points, as in Ghost of
//!   Tsushima: a clump shares a height, a facing and a colour, its blades lean out from its
//!   middle and shorten toward its edge. That is what makes a field read as grown, not
//!   sprinkled.
//! - **Each blade a curve.** A quadratic Bézier from the root, bent over by its own weight,
//!   tapering to a point, its normals leaning to either side across it so it shades round.
//! - **Rings of detail.** Near the camera a blade has three segments; farther off one, wider,
//!   and fewer of them. A far ring keeps a subset of the near ring's blades, chosen by each
//!   blade's own rank, so changing ring thins the grass rather than moving it.
//!
//! The vertex shader (`grass.wesl`) shrinks each blade into its root with distance, so the
//! grass never shows an edge; each vertex carries the way back to its root for that.

use super::{CoverSource, SQUARE_M};
use crate::planet::Planet;
use crate::start::Start;
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use std::f32::consts::TAU;
use terra_gen::Seed;

/// Blades are tried on a grid this fine, in metres: about 150 a square metre where the
/// grass is thickest.
const BLADE_SPACING_M: f32 = 0.08;
/// Clumps are scattered one to a cell this wide.
const CLUMP_M: f32 = 0.45;
/// What grows is sampled on a grid this fine and blended between, rather than asked of
/// every blade.
const SAMPLE_M: f32 = 0.5;
/// Ground within 30 m is measured for bareness in cells this wide.
pub const CELL_M: f32 = 0.25;
/// Cells to a side of a square.
pub const CELLS: usize = (SQUARE_M / CELL_M) as usize;

/// How a ring draws its blades: how many segments each has, how wide it is, and what share
/// of the near ring's blades it keeps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ring {
    pub segments: u32,
    pub width_m: f32,
    pub share: f32,
}

/// The rings of detail, nearest first, and where each ends, in metres from the camera.
pub const RINGS: [(Ring, f32); 3] = [
    (
        Ring {
            segments: 3,
            width_m: 0.034,
            share: 1.0,
        },
        16.0,
    ),
    (
        Ring {
            segments: 2,
            width_m: 0.048,
            share: 0.45,
        },
        32.0,
    ),
    (
        Ring {
            segments: 1,
            width_m: 0.07,
            share: 0.2,
        },
        f32::MAX,
    ),
];

/// A square built: its mesh and where it stands, if anything grows there, and which of its
/// cells anything grows in or a path crosses, for the bare-ground measure.
pub struct Built {
    pub mesh: Option<(Transform, Mesh)>,
    pub covered: Vec<bool>,
    pub path: Vec<bool>,
}

/// One clump: its middle, on the square's own map, and what its blades share.
#[derive(Clone, Copy)]
struct Clump {
    at: Vec2,
    height: f32,
    facing: f32,
    hue: f32,
    dry: f32,
}

/// A cheap, well-mixed hash of a grid cell and a stream, as a number in [0, 1); seeded so
/// two worlds differ. Cover is only seen, so it need not be the same on every machine
/// (docs/biomes.md, three tiers), but it is anyway.
fn hash(seed: u64, i: i32, j: i32, stream: u64) -> f32 {
    let mut x = seed
        ^ (i as u32 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (j as u32 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ stream.wrapping_mul(0x1656_67B1_9E37_79F9);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

fn clump(seed: u64, i: i32, j: i32) -> Clump {
    let h = |stream| hash(seed, i, j, stream);
    Clump {
        at: (Vec2::new(i as f32 + h(1), j as f32 + h(2))) * CLUMP_M,
        height: 0.65 + 0.7 * h(3),
        facing: h(4) * TAU,
        hue: h(5) * 2.0 - 1.0,
        dry: h(6) * 2.0 - 1.0,
    }
}

/// What grows on the sample grid: the ground's rise above the square's middle, how much
/// grass, how lush, how much path, litter and moss, blended between the samples.
struct Samples {
    corner: Vec2,
    side: usize,
    values: Vec<[f32; 6]>,
}

impl Samples {
    fn at(&self, at: Vec2) -> [f32; 6] {
        let cell = ((at - self.corner) / SAMPLE_M)
            .clamp(Vec2::ZERO, Vec2::splat((self.side - 1) as f32 - 0.001));
        let (i, j) = (cell.x as usize, cell.y as usize);
        let (fx, fy) = (cell.x.fract(), cell.y.fract());
        let get = |a: usize, b: usize| self.values[b * self.side + a];
        let mut out = [0.0; 6];
        for (k, value) in out.iter_mut().enumerate() {
            let top = get(i, j)[k] * (1.0 - fx) + get(i + 1, j)[k] * fx;
            let bottom = get(i, j + 1)[k] * (1.0 - fx) + get(i + 1, j + 1)[k] * fx;
            *value = top * (1.0 - fy) + bottom * fy;
        }
        out
    }
}

/// Builds the square `key` (squares of [`SQUARE_M`] east and south of the start) at a ring
/// of detail.
pub fn square(
    source: &dyn CoverSource,
    planet: &Planet,
    start: &Start,
    seed: Seed,
    key: (i32, i32),
    ring: Ring,
) -> Built {
    let middle = (Vec2::new(key.0 as f32, key.1 as f32) + 0.5) * SQUARE_M;
    let half = SQUARE_M / 2.0;
    let mut built = Built {
        mesh: None,
        covered: vec![false; CELLS * CELLS],
        path: vec![false; CELLS * CELLS],
    };
    if !source.reaches(middle, half) {
        return built;
    }
    let centre = planet.ground_at(start.at(middle.x, middle.y));
    let up = centre.normalize();
    let east_axis = start.rotation * Vec3::X;
    let east = (east_axis - up * east_axis.dot(up)).normalize();
    let south = east.cross(up);

    let side = (SQUARE_M / SAMPLE_M) as usize + 1;
    let corner = middle - Vec2::splat(half);
    let mut values = Vec::with_capacity(side * side);
    for b in 0..side {
        for a in 0..side {
            let at = corner + Vec2::new(a as f32, b as f32) * SAMPLE_M;
            let ground = planet.ground_at(start.at(at.x, at.y));
            let growing = source.growing(planet, ground, at);
            values.push([
                (ground - centre).dot(up),
                growing.grass,
                growing.lush,
                if growing.path { 1.0 } else { 0.0 },
                growing.litter,
                growing.moss,
            ]);
        }
    }
    let samples = Samples {
        corner,
        side,
        values,
    };

    let seed = seed.derive("grass blades").rng().next_u64();
    let clumps_seed = seed ^ 0x5DEE_CE66;
    let first = ((corner / CLUMP_M).floor() - 1.0).as_ivec2();
    let across = (SQUARE_M / CLUMP_M).ceil() as i32 + 3;
    let clumps: Vec<Clump> = (0..across)
        .flat_map(|b| (0..across).map(move |a| (a, b)))
        .map(|(a, b)| clump(clumps_seed, first.x + a, first.y + b))
        .collect();
    let nearest_clump = |at: Vec2| -> (Clump, f32) {
        let cell = (at / CLUMP_M).floor().as_ivec2() - first;
        let mut best = (clumps[0], f32::MAX);
        for dj in -1..=1 {
            for di in -1..=1 {
                let (a, b) = (cell.x + di, cell.y + dj);
                if a < 0 || b < 0 || a >= across || b >= across {
                    continue;
                }
                let c = clumps[(b * across + a) as usize];
                let d = c.at.distance_squared(at);
                if d < best.1 {
                    best = (c, d);
                }
            }
        }
        (best.0, best.1.sqrt() / CLUMP_M)
    };

    // Vertex colours are linear, as the renderer works.
    let linear =
        |r, g, b| Vec3::from_array(Color::srgb(r, g, b).to_linear().to_f32_array_no_alpha());
    let green = linear(0.30, 0.45, 0.14);
    let deep = linear(0.20, 0.36, 0.10);
    let dry = linear(0.58, 0.53, 0.28);

    let mut mesh = BladeMesh::default();
    let steps = (SQUARE_M / BLADE_SPACING_M) as i32;
    let first_blade = (corner / BLADE_SPACING_M).round().as_ivec2();
    for b in 0..steps {
        for a in 0..steps {
            let (gi, gj) = (first_blade.x + a, first_blade.y + b);
            let h = |stream| hash(seed, gi, gj, stream);
            let at = Vec2::new(gi as f32 + h(1), gj as f32 + h(2)) * BLADE_SPACING_M;
            let [rise, grass, lush, path, litter, moss] = samples.at(at);
            let cell = (((at - corner) / CELL_M).floor().as_ivec2())
                .clamp(IVec2::ZERO, IVec2::splat(CELLS as i32 - 1));
            let index = cell.y as usize * CELLS + cell.x as usize;
            if path > 0.5 {
                built.path[index] = true;
            }
            // Fallen leaves and moss cover the ground too (docs/biomes.md, ground cover).
            if litter + moss > 0.5 {
                built.covered[index] = true;
            }
            // Its rank decides whether it grows at all, and in which rings: the far rings
            // keep the lowest ranks, a subset of the near ring.
            let rank = h(3);
            if rank > grass || grass < 0.02 {
                continue;
            }
            // Measured as grown at full detail: a far ring draws fewer blades, wider, over
            // the same ground.
            built.covered[index] = true;
            if rank > grass * ring.share {
                continue;
            }
            let (clump, from_middle) = nearest_clump(at);
            let tall = (0.12 + 0.42 * libm::powf(grass, 0.7) * (0.6 + 0.4 * lush))
                * clump.height
                * (1.0 - 0.35 * from_middle.min(1.0))
                * (0.8 + 0.25 * h(4));
            let away = (at - clump.at).normalize_or(Vec2::X);
            let facing = clump.facing + (h(5) - 0.5) * 1.8;
            let bend_way =
                (Vec2::new(libm::cosf(facing), libm::sinf(facing)) + away * 0.8).normalize_or(away);
            let dryness =
                ((1.0 - lush) * 0.45 + clump.dry * 0.2 + (h(6) - 0.5) * 0.25).clamp(0.0, 1.0);
            let tip =
                deep.lerp(green, 0.5 + 0.5 * clump.hue).lerp(dry, dryness) * (0.85 + 0.3 * h(7));
            let local = Vec3::new(at.x - middle.x, rise, at.y - middle.y);
            mesh.blade(
                local,
                Vec3::new(bend_way.x, 0.0, bend_way.y),
                tall,
                0.15 + 0.4 * h(8),
                ring.width_m * (0.8 + 0.4 * h(9)),
                tip,
                ring.segments,
            );
        }
    }
    if !mesh.indices.is_empty() {
        let place = Transform {
            translation: centre,
            rotation: Quat::from_mat3(&Mat3::from_cols(east, up, south)),
            scale: Vec3::ONE,
        };
        built.mesh = Some((place, mesh.mesh()));
    }
    built
}

/// The blades of a square, as the grass shader reads them.
#[derive(Default)]
struct BladeMesh {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    /// From the vertex back to its blade's root, across: the root is this far east and
    /// south of it.
    to_root: Vec<[f32; 2]>,
    /// How it moves in the wind: with the blade, and on its own.
    sway: Vec<[f32; 2]>,
    /// Its colour, and in alpha how far it stands above its root.
    colours: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl BladeMesh {
    /// A blade from `root`, `tall` metres, bending toward `way` by `bend` of its height,
    /// `width` across at the foot, coloured `tip` at its tip and darker at the root.
    fn blade(
        &mut self,
        root: Vec3,
        way: Vec3,
        tall: f32,
        bend: f32,
        width: f32,
        tip: Vec3,
        segments: u32,
    ) {
        let side = way.cross(Vec3::Y).normalize_or(Vec3::X);
        let control = root + Vec3::Y * tall * 0.65;
        let end = root + Vec3::Y * tall * (1.0 - 0.4 * bend) + way * tall * bend;
        let curve =
            |t: f32| root * (1.0 - t) * (1.0 - t) + control * 2.0 * t * (1.0 - t) + end * t * t;
        let tangent = |t: f32| {
            ((control - root) * 2.0 * (1.0 - t) + (end - control) * 2.0 * t).normalize_or(Vec3::Y)
        };
        let base = self.positions.len() as u32;
        let root_colour = tip * 0.32;
        let mut push = |point: Vec3, normal: Vec3, t: f32| {
            self.positions.push(point.to_array());
            self.normals.push(normal.to_array());
            self.to_root.push([root.x - point.x, root.z - point.z]);
            self.sway.push([t * t * 0.5, t * 0.6]);
            let colour = root_colour.lerp(tip, libm::powf(t, 0.8));
            self.colours
                .push([colour.x, colour.y, colour.z, point.y - root.y]);
        };
        for step in 0..segments {
            let t = step as f32 / segments as f32;
            let point = curve(t);
            // Facing out of the blade's bend, leaning mostly up so a field shades as one
            // surface, and to either side across it so a blade shades round.
            let face = side.cross(tangent(t)).normalize_or(Vec3::Y);
            let lit = (Vec3::Y * 0.6 + face * 0.4).normalize();
            let half = width * 0.5 * libm::powf(1.0 - t, 0.8);
            push(point - side * half, (lit - side * 0.3).normalize(), t);
            push(point + side * half, (lit + side * 0.3).normalize(), t);
        }
        let face = side.cross(tangent(1.0)).normalize_or(Vec3::Y);
        push(curve(1.0), (Vec3::Y * 0.6 + face * 0.4).normalize(), 1.0);
        for step in 0..segments {
            let a = base + step * 2;
            if step + 1 < segments {
                self.indices.extend([a, a + 1, a + 2, a + 1, a + 3, a + 2]);
            } else {
                self.indices.extend([a, a + 1, a + 2]);
            }
        }
    }

    fn mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.to_root)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.sway)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colours)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}
