//! Weathering a building from its own shape (research/2026-10-25-wfc-weathering.md), a
//! spike: does wave function collapse over weathering motifs make a building read as aged
//! better than the masks shipped games use?
//!
//! Both paths start from the same **masks**, worked out from the cubes (`masks`): outside
//! edges for wear, crevices and the underside of ledges for grime, faces looking up and the
//! wall's foot for moss and damp, the zone under sills for streaks, the zone over openings
//! for soot and lost stones, all scaled by the building's age. Then the layers a material
//! blends (crack, loss, moss, streak; grime, wear, lichen, damp; soot) are drawn:
//!
//! - [`Mode::Masks`], classic procedural weathering: each motif layer is its mask cut by
//!   noise at a threshold, as Substance's generators and most engines' building shaders do.
//! - [`Mode::Wfc`]: the motif layers are solved by the overlapping model of wave function
//!   collapse (`wfc`, after Maxim Gumin's, MIT) over symbol grids learned from a few example
//!   patches (`motifs`), each symbol weighed and banned cell by cell by the masks: cracks
//!   that run on and branch, stones lost in clusters, moss that spreads in patches, streaks
//!   that run down from a sill.
//!
//! The output is a map per exposed half-metre face, [`TILE`] texels a side, in three RGBA
//! layers of one atlas, and a lookup from each cube's face to its place in the atlas, which
//! the building material reads by where a point is in the base (`shaders/build.wesl`).

use glam::IVec3;

pub mod compress;
mod faces;
mod masks;
mod motifs;
mod raster;
#[cfg(test)]
mod tests;
pub mod wfc;

pub use faces::{FACES, Face};
pub use masks::Masks;

/// Texels a side of each face's map: a half-metre face at 1.6 cm a texel.
pub const TILE: usize = 32;

/// How many RGBA layers the atlas has: crack, loss, moss, streak; grime, wear, lichen,
/// damp; soot.
pub const LAYERS: usize = 3;

/// The length of a cube's side, in metres (`terra_build::CELL_M`).
pub const CELL_M: f32 = 0.5;

/// What a cube is, as weathering sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stuff {
    /// Nothing, or an opening: weather reaches what faces it.
    Air,
    /// Laid stones: cracks along their joints, stones lost, spalled faces.
    Stone,
    /// Logs or hewn timber: grime, moss, streaks, never cracks or lost stones.
    Timber,
    /// A roof's covering: moss and lichen on what looks up.
    Roof,
}

impl Stuff {
    pub fn solid(self) -> bool {
        self != Stuff::Air
    }
}

/// Which way of weathering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Every motif layer its mask cut by noise.
    Masks,
    /// The motif layers solved by wave function collapse, weighed by the masks.
    Wfc,
}

/// What to weather, and how.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub mode: Mode,
    /// How long the building has stood, in years.
    pub age: f32,
    pub seed: u32,
    /// How many times a plane's solve is started again after a contradiction.
    pub tries: u32,
}

/// A building weathered: its maps and where each face's is.
#[derive(Clone, Debug, Default)]
pub struct Weathered {
    /// The least cube of the box the lookup covers, and its size in cubes.
    pub low: IVec3,
    pub dims: IVec3,
    /// For each cube of the box and each of its six faces, its map's slot and one, or 0 for
    /// none: `(dims.x * 6)` across, `(dims.y * dims.z)` down, the cube `(x, y, z)`'s face `f`
    /// at `(x * 6 + f, z * dims.y + y)`.
    pub lookup: Vec<u32>,
    /// How many maps are in a row of the atlas, and how many there are.
    pub per_row: u32,
    pub slots: u32,
    /// The atlas's layers, RGBA8, `per_row * TILE` texels a side; each face's map with its
    /// first row at the top of the face.
    pub atlas: [Vec<u8>; LAYERS],
    pub stats: Stats,
}

impl Weathered {
    /// The atlas's side in texels.
    pub fn side(&self) -> usize {
        self.per_row as usize * TILE
    }

    /// The bytes the maps and the lookup take on the GPU.
    pub fn bytes(&self) -> usize {
        self.atlas.iter().map(Vec::len).sum::<usize>() + self.lookup.len() * 4
    }
}

/// How the solves went.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Exposed faces weathered, and the planes they fall in.
    pub faces: usize,
    pub planes: usize,
    /// Solves (a plane and a motif layer each), those done at the first try, those done
    /// at all, and the tries they took.
    pub solves: usize,
    pub first_try: usize,
    pub solved: usize,
    pub tries: usize,
    /// Grid cells solved, over every solve.
    pub cells: usize,
}

/// Weathers the building whose cubes `stuff` gives, inside the box `low..=high`.
pub fn weather(
    stuff: &(dyn Fn(IVec3) -> Stuff + Sync),
    low: IVec3,
    high: IVec3,
    settings: &Settings,
) -> Weathered {
    let planes = faces::planes(stuff, low, high);
    let ground = low.y;
    let drawn = terra_gen::par::map(&planes, |plane| {
        raster::plane(stuff, plane, ground, settings)
    });
    raster::pack(&planes, drawn, low, high)
}
