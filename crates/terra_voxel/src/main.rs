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
    // The whole tree of chunks around one viewer on the ground, and meshing all of it.
    let detail = terra_voxel::Detail::for_planet(2000.0, 150.0);
    let viewer = surface[0] + surface[0].normalize() * 1.7;
    let start = Instant::now();
    let tree = terra_voxel::chunks_around(&[viewer], &detail, |id| {
        terra_voxel::may_hold_surface(&planet, id)
    });
    let choosing = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let meshed: Vec<usize> = tree
        .iter()
        .map(|&(id, finer)| mesh_chunk(&planet, id, finer).triangle_count())
        .collect();
    let meshing = start.elapsed().as_secs_f64();
    let with_surface = meshed.iter().filter(|&&triangles| triangles > 0).count();
    println!(
        "Around a viewer: {} chunks chosen in {choosing:.1} ms, {with_surface} with surface, {} triangles, all meshed on one thread in {meshing:.2} s",
        tree.len(),
        meshed.iter().sum::<usize>()
    );
    mountains();
    println!(
        "Budget: 6 ms (docs/terrain.md). {}",
        if plain <= 6.0 {
            "Within it."
        } else {
            "Over it."
        }
    );
}

/// The same in mountains on the 30 km world (D322): chunks down a steep flank, and the whole
/// tree round a viewer in a valley, out to the horizon.
fn mountains() {
    let radius = 30_000.0;
    let ground = GeneratedPlanet::new(
        PlanetSpec {
            seed: Seed(1),
            radius_m: radius,
        },
        150.0,
        Some(-20.0),
    )
    .with_mountains(terra_voxel::Mountains::START);
    let range = ground.mountains().expect("mountains").clone();
    // A place well inside a range: the first of a spiral of directions that is.
    let inside = (0..20_000)
        .map(|i| {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / 20_000.0;
            let around = i as f32 * 2.399_963;
            let flat = (1.0 - y * y).sqrt();
            glam::Vec3::new(flat * libm::cosf(around), y, flat * libm::sinf(around))
        })
        .find(|&d| range.inside(d * radius) >= 1.0)
        .expect("a range somewhere");
    // What one point of the mountains costs to work out, and one ask of the ground.
    let across = inside.any_orthonormal_vector();
    let points: Vec<_> = (0..10_000)
        .map(|i| (inside + across * (i as f32 * 0.3 / radius)).normalize())
        .collect();
    let start = Instant::now();
    let total: f32 = points.iter().map(|&d| range.rise(d, radius).height_m).sum();
    let rise_us = start.elapsed().as_secs_f64() * 1e6 / points.len() as f64;
    let start = Instant::now();
    let mut memo = terra_voxel::Field::memo_for(&ground, 0.5);
    let asked: f32 = points
        .iter()
        .map(|&d| terra_voxel::Field::sample_in(&ground, d * (radius + 1500.0), &mut memo).distance)
        .sum();
    let sample_us = start.elapsed().as_secs_f64() * 1e6 / points.len() as f64;
    println!(
        "In mountains: a point of them worked out in {rise_us:.2} µs, the ground asked in {sample_us:.2} µs ({:.0})",
        total + asked
    );
    let planet = VoxelPlanet::new(ground);
    let surface: Vec<_> = (0..40)
        .map(|i| {
            let d = (inside + across * (i as f32 * 25.0 / radius)).normalize();
            d * (radius + planet.field().height_m(d))
        })
        .collect();
    let chunks: Vec<_> = surface.iter().map(|&point| chunk_at(point, 0)).collect();
    let start = Instant::now();
    let triangles: usize = chunks
        .iter()
        .map(|&id| mesh_chunk(&planet, id, Sides::none()).triangle_count())
        .sum();
    let each = start.elapsed().as_secs_f64() * 1000.0 / chunks.len() as f64;
    println!(
        "In mountains, a 30 km world: a 32-metre chunk in {each:.2} ms, {} triangles",
        triangles / chunks.len()
    );
    let detail = terra_voxel::Detail::for_planet(radius, planet.field().reach_m());
    let viewer = surface[0] + surface[0].normalize() * 1.7;
    let start = Instant::now();
    let tree = terra_voxel::chunks_around(&[viewer], &detail, |id| {
        terra_voxel::may_hold_surface(&planet, id)
    });
    let choosing = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let meshed: Vec<usize> = tree
        .iter()
        .map(|&(id, finer)| mesh_chunk(&planet, id, finer).triangle_count())
        .collect();
    let meshing = start.elapsed().as_secs_f64();
    println!(
        "Around a viewer there: {} chunks chosen in {choosing:.1} ms, {} with surface, {} triangles, all meshed on one thread in {meshing:.2} s",
        tree.len(),
        meshed.iter().filter(|&&t| t > 0).count(),
        meshed.iter().sum::<usize>()
    );
}
