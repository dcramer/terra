//! Measures how long meshing takes, so the speed the terrain brief asks for (docs/terrain.md:
//! a 32-cubed chunk in no more than 6 ms) can be checked on any machine:
//!
//!   cargo run --release -p terra_voxel
//!   cargo xtask win -p terra_voxel --release

use std::time::Instant;
use terra_core::{Operation, PlanetSpec, Seed};
use terra_voxel::{GeneratedPlanet, Sides, VoxelPlanet, chunk_at, chunk_size_m, mesh_chunk};

fn main() {
    let mut planet = VoxelPlanet::new(GeneratedPlanet::new(
        PlanetSpec {
            seed: Seed(7),
            radius_m: 2000.0,
        },
        150.0,
        Some(-20.0),
    ));
    // Chunks along the surface, where meshing has work to do.
    let surface: Vec<_> = (0..40)
        .map(|i| {
            let direction = glam::Vec3::new(i as f32 * 0.004, 1.0, i as f32 * 0.003).normalize();
            direction * (2000.0 + planet.field().height_m(direction))
        })
        .collect();
    let chunks: Vec<_> = surface.iter().map(|&point| chunk_at(point, 0)).collect();
    let time = |planet: &VoxelPlanet<GeneratedPlanet>, finer: Sides| {
        let start = Instant::now();
        let triangles: usize = chunks
            .iter()
            .map(|&id| mesh_chunk(planet, id, finer).triangle_count())
            .sum();
        let each = start.elapsed().as_secs_f64() * 1000.0 / chunks.len() as f64;
        (each, triangles / chunks.len())
    };
    let (plain, triangles) = time(&planet, Sides::none());
    println!(
        "Meshing a {}-metre chunk of 32 cubed voxels: {plain:.2} ms, {triangles} triangles",
        chunk_size_m(0)
    );
    let (seams, _) = time(&planet, Sides::LOW_X.with(Sides::HIGH_Z));
    println!("With two transition faces: {seams:.2} ms");
    // Edits make every sample look at those nearby.
    for &centre in &surface {
        planet.apply(&Operation::Dig {
            centre,
            radius: 2.5,
        });
    }
    let (dug, _) = time(&planet, Sides::none());
    println!("After 40 digs, one in each: {dug:.2} ms");
    // The mesher alone, on a ground that costs nothing to ask about: a smooth sphere.
    struct Sphere;
    impl terra_voxel::Field for Sphere {
        type Memo = ();

        fn distance_in(&self, point: glam::Vec3, _: &mut ()) -> f32 {
            point.length() - 2003.0
        }

        fn sample_in(&self, point: glam::Vec3, memo: &mut ()) -> terra_voxel::Sample {
            terra_voxel::Sample {
                distance: self.distance_in(point, memo),
                material: terra_voxel::material::STONE,
            }
        }
    }
    let start = Instant::now();
    for &id in &chunks {
        mesh_chunk(&Sphere, id, Sides::none());
    }
    let sphere = start.elapsed().as_secs_f64() * 1000.0 / chunks.len() as f64;
    println!("The mesher alone, on a plain sphere: {sphere:.2} ms");
    println!(
        "Budget: 6 ms (docs/terrain.md). {}",
        if plain <= 6.0 {
            "Within it."
        } else {
            "Over it."
        }
    );
}
