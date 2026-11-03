//! The ground's value noise read from a table (`assets/shaders/ground.wesl`, `noise`): a
//! random value at each point of a lattice that repeats every `LATTICE` cells on each axis,
//! its eight corners round a point fetched by two gathers rather than hashed one by one. The
//! noise is the same smooth trilinear value noise, its corners' values another draw of the
//! same dice. Unreal's Noise expression offers its texture-based functions for this cost,
//! Quilez's value noise reads its lattice from a picture ("value noise derivatives",
//! iquilezles.org) and the fire's gas reads its own from a volume (`fire::volume::noise`);
//! hashed, the ground's forty-odd noises a pixel were most of its shader (play-lake-fps j4).
//! A gather filters nothing, so the corners blend in full precision, not at the 8 bits of a
//! texture unit's linear weights, which band a noise stretched over many pixels.

use crate::planet::GroundMaterial;
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use terra_gen::tune::Tunable;

/// Cells a side before the lattice repeats, ground.wesl's `LATTICE`: a power of two, so the
/// shader's wrap of a cell into it is exact. 128 repeats the finest noise, five a metre, every
/// 26 m, and the broad blotches every few hundred.
pub(crate) const LATTICE: u32 = 128;

/// The lattice's own dice, apart from every other hash's.
const SEED: u32 = 0x6c61_7474; // "latt"

/// The table, made once as the app starts (`table`), for every ground material.
pub(crate) const TABLE: Handle<Image> =
    bevy::asset::uuid_handle!("3f0c8a52-6d1e-4b7a-9c2d-81e5f4a0b6c3");

/// Whether the ground reads its noise from the table (1) or hashes each corner (0), as the
/// probe A/Bs a lever (D609).
pub static FROM_TABLE: Tunable = Tunable::new(
    "ground.noise_table",
    0.0,
    0.0..=1.0,
    "1 reads the ground's value noise from its lattice in two gathers, 0 hashes its eight corners",
);

/// `FROM_TABLE` as the ground's uniform says it.
pub(crate) fn from_table() -> u32 {
    (FROM_TABLE.get() >= 0.5) as u32
}

/// The value at a lattice point, wrapped into the lattice, 0 to 255.
fn value(x: u32, y: u32, z: u32) -> u8 {
    let v = terra_gen::hash::unit3(SEED, x % LATTICE, y % LATTICE, z % LATTICE);
    (v * 255.0).round() as u8
}

/// The lattice as a picture, a layer a step in z, each a row and a column wider than the
/// lattice, its last the first again, so a gather at the lattice's last cell finds its
/// neighbours without the sampler wrapping. Its sampler is the ground's textures' (a gather
/// filters nothing).
pub(crate) fn table() -> Image {
    let side = LATTICE + 1;
    let mut data = Vec::with_capacity((side * side * LATTICE) as usize);
    for z in 0..LATTICE {
        for y in 0..side {
            for x in 0..side {
                data.push(value(x, y, z));
            }
        }
    }
    Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: LATTICE,
        },
        TextureDimension::D2,
        data,
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Tells the ground's materials when the tunable is set while the game runs; the ground's
/// material is otherwise made with its value (`GroundExtension::new`).
fn follow(mut seen: Local<Option<u32>>, mut materials: ResMut<Assets<GroundMaterial>>) {
    let now = from_table();
    if *seen == Some(now) {
        return;
    }
    *seen = Some(now);
    for (_, material) in materials.iter_mut() {
        material.extension.noise_from = now;
    }
}

fn make(mut images: ResMut<Assets<Image>>) {
    images.insert(&TABLE, table()).ok();
}

pub(crate) fn plugin(app: &mut App) {
    terra_gen::tune::register(&[&FROM_TABLE]);
    app.add_systems(Startup, make).add_systems(Update, follow);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(image: &Image, x: u32, y: u32, z: u32) -> u8 {
        let side = LATTICE + 1;
        image.data.as_ref().unwrap()[((z * side + y) * side + x) as usize]
    }

    #[test]
    fn the_table_holds_the_lattice_and_its_last_row_and_column_are_its_first() {
        let image = table();
        let side = LATTICE + 1;
        assert_eq!(image.texture_descriptor.size.width, side);
        assert_eq!(image.texture_descriptor.size.depth_or_array_layers, LATTICE);
        for z in [0, 1, LATTICE - 1] {
            for i in [0, 7, LATTICE - 1] {
                assert_eq!(at(&image, LATTICE, i, z), at(&image, 0, i, z));
                assert_eq!(at(&image, i, LATTICE, z), at(&image, i, 0, z));
            }
            assert_eq!(at(&image, LATTICE, LATTICE, z), at(&image, 0, 0, z));
        }
    }

    #[test]
    fn the_lattice_values_are_spread_evenly_and_unrelated_to_their_neighbours() {
        let n = LATTICE;
        let values: Vec<f64> = (0..n * n)
            .map(|i| f64::from(value(i % n, i / n, 3)) / 255.0)
            .collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        assert!((mean - 0.5).abs() < 0.01, "mean {mean}");
        let low = values.iter().filter(|&&v| v < 0.1).count() as f64 / values.len() as f64;
        assert!((low - 0.1).abs() < 0.01, "share under 0.1: {low}");
        // Neighbours along x and along z, as a 3D noise needs both unrelated.
        let along = |next: &dyn Fn(u32, u32) -> u8| {
            let mut sum = 0.0;
            for y in 0..n {
                for x in 0..n {
                    let a = f64::from(value(x, y, 3)) / 255.0 - 0.5;
                    let b = f64::from(next(x, y)) / 255.0 - 0.5;
                    sum += a * b;
                }
            }
            sum / f64::from(n * n) / (1.0 / 12.0)
        };
        let x = along(&|x, y| value(x + 1, y, 3));
        let z = along(&|x, y| value(x, y, 4));
        assert!(x.abs() < 0.03 && z.abs() < 0.03, "correlation x {x}, z {z}");
    }

    #[test]
    fn the_shader_wraps_by_the_same_lattice() {
        let source = include_str!("../../../assets/shaders/ground.wesl");
        assert_eq!(
            crate::far_air::constant(source, "LATTICE"),
            vec![LATTICE as f32]
        );
    }
}
