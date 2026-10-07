//! Toolchain spike for the all-Rust stack (docs/tech.md). Answers: does Bevy plus the
//! `transvoxel` crate build and run from our machines, how long do builds take, and what
//! does meshing cost? It draws a small bumpy planet out of independently meshed blocks,
//! measures frame times, saves a screenshot and exits. Throwaway.

use bevy::{
    asset::RenderAssetUsages,
    mesh::Indices,
    prelude::*,
    render::{
        render_resource::PrimitiveTopology,
        renderer::RenderAdapterInfo,
        view::screenshot::{save_to_disk, Screenshot},
    },
    window::PresentMode,
};
use std::time::Instant;
use transvoxel::prelude::*;

const RADIUS: f32 = 200.0;
const BLOCK_SIZE: f32 = 64.0;
const SUBDIVISIONS: usize = 32;
const BLOCKS_PER_AXIS: i32 = 8;
const WARMUP_FRAMES: u32 = 120;
const MEASURED_FRAMES: usize = 600;

/// Positive inside the planet, negative outside.
fn density(x: f32, y: f32, z: f32) -> f32 {
    let len = (x * x + y * y + z * z).sqrt().max(1e-6);
    let (nx, ny, nz) = (x / len, y / len, z / len);
    // `libm` is pure Rust, so this gives the same answer, at the same speed, on every
    // platform. The system maths library does neither.
    let sin = libm::sinf;
    let hills = 9.0 * sin(5.0 * nx + 1.3) * sin(4.0 * ny + 0.4)
        + 6.0 * sin(9.0 * nz + 2.1) * sin(7.0 * nx)
        + 2.0 * sin(23.0 * ny + 12.0 * nz);
    RADIUS + hills - len
}

struct BlockMesh {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

fn mesh_block(origin: [f32; 3]) -> Option<BlockMesh> {
    let block = Block::new(origin, BLOCK_SIZE, SUBDIVISIONS);
    let builder = extract_from_field(
        &density,
        FieldCaching::CacheNothing,
        block,
        TransitionSide::none(),
        0.0,
        GenericMeshBuilder::new(),
    );
    let mesh = builder.build();
    if mesh.triangle_indices.is_empty() {
        return None;
    }
    let triples = |flat: &[f32]| flat.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
    Some(BlockMesh {
        positions: triples(&mesh.positions),
        normals: triples(&mesh.normals),
        indices: mesh.triangle_indices.iter().map(|&i| i as u32).collect(),
    })
}

#[derive(Resource, Default)]
struct Run {
    frame: u32,
    frame_ms: Vec<f32>,
    shot_taken: bool,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Terra Rust stack spike".into(),
                resolution: (1920u32, 1080u32).into(),
                present_mode: PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }))
        .init_resource::<Run>()
        .add_systems(Startup, setup)
        .add_systems(Update, (spin, measure))
        .run();
}

#[derive(Component)]
struct Planet;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Mesh every block on its own thread, as the real terrain system would.
    let half = BLOCKS_PER_AXIS / 2;
    let origins: Vec<[f32; 3]> = (-half..half)
        .flat_map(|x| (-half..half).flat_map(move |y| (-half..half).map(move |z| [x, y, z])))
        .map(|c| c.map(|v| v as f32 * BLOCK_SIZE))
        .collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let started = Instant::now();
    let block_meshes: Vec<BlockMesh> = std::thread::scope(|scope| {
        let handles: Vec<_> = origins
            .chunks(origins.len().div_ceil(threads))
            .map(|chunk| scope.spawn(move || chunk.iter().filter_map(|&o| mesh_block(o)).collect::<Vec<_>>()))
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let wall_ms = started.elapsed().as_secs_f32() * 1000.0;
    let triangles: usize = block_meshes.iter().map(|m| m.indices.len() / 3).sum();

    let single = Instant::now();
    let _ = mesh_block([0.0, RADIUS - BLOCK_SIZE / 2.0, 0.0]);
    println!(
        "SPIKE meshed {} blocks of {}^3 cells ({} with surface, {} triangles) in {:.0} ms on {} threads; one surface block alone takes {:.2} ms",
        origins.len(), SUBDIVISIONS, block_meshes.len(), triangles, wall_ms, threads,
        single.elapsed().as_secs_f32() * 1000.0
    );

    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.5, 0.4),
        perceptual_roughness: 0.95,
        ..default()
    });
    let planet = commands.spawn((Planet, Transform::default(), Visibility::default())).id();
    for block in block_meshes {
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, block.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, block.normals)
            .with_inserted_indices(Indices::U32(block.indices));
        commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(material.clone()), ChildOf(planet)));
    }

    commands.spawn((
        DirectionalLight { illuminance: 12_000.0, ..default() },
        Transform::from_xyz(1.0, 1.0, 0.6).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, RADIUS * 0.6, RADIUS * 2.6).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn spin(time: Res<Time>, mut planet: Single<&mut Transform, With<Planet>>) {
    planet.rotate_y(time.delta_secs() * 0.3);
}

fn measure(
    mut commands: Commands,
    mut run: ResMut<Run>,
    time: Res<Time<Real>>,
    adapter: Res<RenderAdapterInfo>,
    mut exit: MessageWriter<AppExit>,
) {
    run.frame += 1;
    if run.frame <= WARMUP_FRAMES {
        return;
    }
    if !run.shot_taken {
        run.shot_taken = true;
        std::fs::create_dir_all("shots").ok();
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk("shots/planet.png"));
    }
    run.frame_ms.push(time.delta_secs() * 1000.0);
    if run.frame_ms.len() == MEASURED_FRAMES {
        let mut sorted = run.frame_ms.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        println!(
            "SPIKE frames on {} ({:?}): median {:.2} ms, 99th percentile {:.2} ms, worst {:.2} ms",
            adapter.name, adapter.backend, sorted[sorted.len() / 2],
            sorted[sorted.len() * 99 / 100], sorted[sorted.len() - 1]
        );
        exit.write(AppExit::Success);
    }
}
