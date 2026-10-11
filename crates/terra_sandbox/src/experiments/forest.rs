//! A whole planet of trees, every one of which can be chopped.
//!
//! The question this answers: can the world hold a very large number of things that are
//! each interactive? The approach is to never store them.
//!
//! - **Trees are computed, not kept.** The planet's surface is divided into cells. The
//!   trees in a cell are a function of the seed and the cell, so any cell can be produced
//!   on demand and thrown away again.
//! - **Only changes are remembered.** A chopped tree is one bit in its cell's mask.
//! - **Only what is near is drawn.** Cells around the camera are each baked into one
//!   mesh. Chopping a tree rebuilds its cell's mesh, which takes well under a millisecond.
//! - **Interaction needs no per-tree object.** Finding the tree you are pointing at is a
//!   search through the few cells around that point.

use crate::planet::{FACES, Planet};
use bevy::{
    asset::RenderAssetUsages, platform::collections::HashMap, prelude::*,
    render::render_resource::PrimitiveTopology,
};
use std::time::Instant;
use terra_core::{Ground, Harvested, KindId, ObjectId, Operation, Water, Yield};
use terra_gen::{Rng, Seed};

/// Cells along each edge of each of the planet's six faces.
const CELLS_PER_EDGE: u32 = 48;
const CELLS_PER_FACE: u32 = CELLS_PER_EDGE * CELLS_PER_EDGE;
const CELL_COUNT: u32 = 6 * CELLS_PER_FACE;
/// Trees tried in each cell. One bit of a `u128` each, so no more than 128.
const TREES_PER_CELL: u32 = 72;
/// Cells whose centre is within this distance of the camera are drawn.
const DRAW_WITHIN_M: f32 = 520.0;
const DROP_BEYOND_M: f32 = 600.0;
/// Cell meshes built per frame, so arriving somewhere new never stalls a frame.
const BUILDS_PER_FRAME: usize = 8;
const REACH_M: f32 = 400.0;
const GRAB_M: f32 = 6.0;
const TREE: KindId = KindId(1);

struct Tree {
    index: u32,
    base: Vec3,
    up: Vec3,
    height: f32,
    width: f32,
    turn: f32,
    shade: f32,
}

#[derive(Resource)]
struct Forest {
    seed: Seed,
    centres: Vec<Vec3>,
    /// One bit per chopped tree, by cell. The only state the forest has.
    chopped: HashMap<u32, u128>,
    drawn: HashMap<u32, Entity>,
    material: Handle<StandardMaterial>,
    operations: Vec<Operation>,
    total: Yield,
    build_ms: f32,
    builds: u32,
    reported: bool,
}

#[derive(Component)]
struct Readout;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (stream, chop, update_readout).chain());
}

/// The direction to a point within a cell, given as fractions (0 to 1) across and up it.
fn cell_direction(cell: u32, across: f32, up: f32) -> Vec3 {
    let (outward, face_across, face_up) = FACES[(cell / CELLS_PER_FACE) as usize];
    let in_face = cell % CELLS_PER_FACE;
    let (x, y) = (in_face % CELLS_PER_EDGE, in_face / CELLS_PER_EDGE);
    let a = (x as f32 + across) / CELLS_PER_EDGE as f32 * 2.0 - 1.0;
    let b = (y as f32 + up) / CELLS_PER_EDGE as f32 * 2.0 - 1.0;
    (outward + face_across * a + face_up * b).normalize()
}

/// The cell a direction falls in.
fn cell_at(direction: Vec3) -> u32 {
    let (face, (outward, face_across, face_up)) = FACES
        .iter()
        .copied()
        .enumerate()
        .max_by(|a, b| direction.dot(a.1.0).total_cmp(&direction.dot(b.1.0)))
        .unwrap();
    let depth = direction.dot(outward);
    let coordinate = |axis: Vec3| {
        let fraction = (direction.dot(axis) / depth + 1.0) * 0.5;
        ((fraction * CELLS_PER_EDGE as f32) as u32).min(CELLS_PER_EDGE - 1)
    };
    face as u32 * CELLS_PER_FACE + coordinate(face_up) * CELLS_PER_EDGE + coordinate(face_across)
}

/// The cell and the cells touching it. Across the edge of a face this finds them by
/// stepping a little way in each direction, so it works everywhere on the planet.
fn neighbourhood(direction: Vec3) -> impl Iterator<Item = u32> {
    let reach = 1.5 / CELLS_PER_EDGE as f32;
    let side = direction.any_orthonormal_vector();
    let other = direction.cross(side);
    let mut cells: Vec<u32> = [-1.0f32, 0.0, 1.0]
        .into_iter()
        .flat_map(|a| {
            [-1.0f32, 0.0, 1.0]
                .map(|b| cell_at((direction + (side * a + other * b) * reach).normalize()))
        })
        .collect();
    cells.sort_unstable();
    cells.dedup();
    cells.into_iter()
}

/// Every tree a cell holds, chopped or not. The same seed and cell always give the same
/// trees, which is what lets nothing be stored.
fn trees_in(planet: &Planet, seed: Seed, cell: u32) -> Vec<Tree> {
    let cell_seed = seed.index(cell as u64);
    (0..TREES_PER_CELL)
        .filter_map(|index| {
            let mut rng: Rng = cell_seed.index(index as u64).rng();
            let direction = cell_direction(cell, rng.unit(), rng.unit());
            let base = planet.surface(direction);
            // No trees in the sea.
            if planet
                .ground
                .level(direction)
                .is_some_and(|sea| base.length() < sea + 1.0)
            {
                return None;
            }
            Some(Tree {
                index,
                base,
                up: direction,
                height: rng.range(5.0, 11.0),
                width: rng.range(1.4, 2.4),
                turn: rng.range(0.0, std::f32::consts::TAU),
                shade: rng.unit(),
            })
        })
        .collect()
}

fn tree_id(cell: u32, index: u32) -> ObjectId {
    ObjectId((cell as u64) << 8 | index as u64)
}

/// One mesh holding every standing tree in a cell, or nothing if none are standing.
fn cell_mesh(trees: &[Tree], chopped: u128) -> Option<Mesh> {
    let mut shape = Shape::default();
    for tree in trees.iter().filter(|tree| chopped & (1 << tree.index) == 0) {
        shape.pine(tree);
    }
    if shape.positions.is_empty() {
        return None;
    }
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, shape.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, shape.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, shape.colours),
    )
}

/// Triangles with flat faces and a colour each, with no shared corners.
#[derive(Default)]
struct Shape {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    colours: Vec<[f32; 4]>,
}

impl Shape {
    fn triangle(&mut self, a: Vec3, b: Vec3, c: Vec3, colour: [f32; 4]) {
        let normal = (b - a).cross(c - a).normalize_or_zero();
        self.positions.extend([a, b, c]);
        self.normals.extend([normal; 3]);
        self.colours.extend([colour; 3]);
    }

    /// A ring of `sides` points around an axis.
    fn ring(
        centre: Vec3,
        across: Vec3,
        along: Vec3,
        radius: f32,
        sides: usize,
        turn: f32,
    ) -> Vec<Vec3> {
        (0..sides)
            .map(|i| {
                let around =
                    Vec2::from_angle(turn + i as f32 / sides as f32 * std::f32::consts::TAU);
                centre + (across * around.x + along * around.y) * radius
            })
            .collect()
    }

    /// A low-polygon pine: a tapered trunk and three stacked cones, lit as flat faces.
    fn pine(&mut self, tree: &Tree) {
        let up = tree.up;
        let across = up.any_orthonormal_vector();
        let along = up.cross(across);
        // Start a little under the ground so the trunk never shows a gap.
        let foot = tree.base - up * 0.6;
        let trunk_top = tree.base + up * tree.height * 0.45;
        let bark = [0.36, 0.24, 0.17, 1.0];
        let low = Self::ring(foot, across, along, tree.width * 0.14, 5, tree.turn);
        let high = Self::ring(trunk_top, across, along, tree.width * 0.08, 5, tree.turn);
        for i in 0..5 {
            let j = (i + 1) % 5;
            self.triangle(low[i], low[j], high[j], bark);
            self.triangle(low[i], high[j], high[i], bark);
        }
        // Greens vary a little from tree to tree and get lighter toward the top.
        for tier in 0..3 {
            let t = tier as f32 / 2.0;
            let bottom = tree.base + up * tree.height * (0.22 + 0.24 * t);
            let tip = tree.base + up * tree.height * (0.62 + 0.19 * t);
            let radius = tree.width * (1.0 - 0.3 * t);
            let green = [
                0.07 + 0.05 * tree.shade + 0.03 * t,
                0.2 + 0.09 * tree.shade + 0.06 * t,
                0.11 + 0.04 * tree.shade,
                1.0,
            ];
            let skirt = Self::ring(
                bottom,
                across,
                along,
                radius,
                6,
                tree.turn + tier as f32 * 0.5,
            );
            for i in 0..6 {
                let j = (i + 1) % 6;
                self.triangle(skirt[i], skirt[j], tip, green);
                self.triangle(skirt[j], skirt[i], bottom, green);
            }
        }
    }
}

fn setup(
    mut commands: Commands,
    planet: Res<Planet>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let centres = (0..CELL_COUNT)
        .map(|cell| planet.surface(cell_direction(cell, 0.5, 0.5)))
        .collect();
    commands.insert_resource(Forest {
        seed: planet.ground.spec().seed.derive("forest"),
        centres,
        chopped: HashMap::default(),
        drawn: HashMap::default(),
        // Colour comes from each triangle, so one material serves every tree.
        material: materials.add(StandardMaterial {
            perceptual_roughness: 1.0,
            ..default()
        }),
        operations: Vec::new(),
        total: Yield::default(),
        build_ms: 0.0,
        builds: 0,
        reported: false,
    });
    commands.spawn((
        Readout,
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(12),
            left: px(12),
            ..default()
        },
    ));
}

/// Keeps the cells around the camera drawn, and lets go of the ones left behind.
fn stream(
    mut commands: Commands,
    planet: Res<Planet>,
    mut forest: ResMut<Forest>,
    camera: Single<&Transform, With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let eye = camera.translation;
    let forest = &mut *forest;

    forest.drawn.retain(|cell, entity| {
        let keep = forest.centres[*cell as usize].distance(eye) < DROP_BEYOND_M;
        if !keep {
            commands.entity(*entity).despawn();
        }
        keep
    });

    let mut wanted: Vec<(f32, u32)> = (0..CELL_COUNT)
        .filter(|cell| !forest.drawn.contains_key(cell))
        .map(|cell| (forest.centres[cell as usize].distance(eye), cell))
        .filter(|(distance, _)| *distance < DRAW_WITHIN_M)
        .collect();
    wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
    for &(_, cell) in wanted.iter().take(BUILDS_PER_FRAME) {
        let started = Instant::now();
        let chopped = forest.chopped.get(&cell).copied().unwrap_or(0);
        let mesh = cell_mesh(&trees_in(&planet, forest.seed, cell), chopped);
        forest.build_ms += started.elapsed().as_secs_f32() * 1000.0;
        forest.builds += 1;
        // A cell with nothing standing still counts as drawn, so it is not rebuilt.
        let mut entity = commands.spawn(MeshMaterial3d(forest.material.clone()));
        if let Some(mesh) = mesh {
            entity.insert(Mesh3d(meshes.add(mesh)));
        }
        let entity = entity.id();
        forest.drawn.insert(cell, entity);
    }

    if wanted.is_empty() && !forest.drawn.is_empty() && !forest.reported {
        forest.reported = true;
        let standing: u32 = forest
            .drawn
            .keys()
            .map(|&cell| trees_in(&planet, forest.seed, cell).len() as u32)
            .sum();
        println!(
            "FOREST up to {} trees over the planet in {} cells; {} standing in the {} cells drawn; \
             building a cell takes {:.2} ms on average; remembering every tree as chopped would take {} KB",
            CELL_COUNT * TREES_PER_CELL,
            CELL_COUNT,
            standing,
            forest.drawn.len(),
            forest.build_ms / forest.builds as f32,
            CELL_COUNT as usize * size_of::<u128>() / 1024
        );
    }
}

fn chop(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    planet: Res<Planet>,
    mut forest: ResMut<Forest>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let forest = &mut *forest;
    let mut rebuild: Vec<u32> = Vec::new();

    // K: chop everything in view, to see what a cleared forest costs.
    if keys.just_pressed(KeyCode::KeyK) {
        let started = Instant::now();
        let mut felled = 0;
        for &cell in forest.drawn.keys() {
            let mask = forest.chopped.entry(cell).or_insert(0);
            for tree in trees_in(&planet, forest.seed, cell) {
                if *mask & (1 << tree.index) == 0 {
                    *mask |= 1 << tree.index;
                    forest.operations.push(Operation::RemoveObject {
                        id: tree_id(cell, tree.index),
                    });
                    felled += 1;
                }
            }
            rebuild.push(cell);
        }
        forest.total.add(Harvested::Object(TREE), felled as f32);
        println!(
            "FOREST chopped {felled} trees in {} cells in {:.1} ms",
            rebuild.len(),
            started.elapsed().as_secs_f32() * 1000.0
        );
    }

    let (camera, camera_transform) = *camera;
    let ray = if keys.just_pressed(KeyCode::KeyF) {
        Some(Ray3d::new(
            camera_transform.translation(),
            camera_transform.forward(),
        ))
    } else if mouse.just_pressed(MouseButton::Left) {
        window
            .cursor_position()
            .and_then(|cursor| camera.viewport_to_world(camera_transform, cursor).ok())
    } else {
        None
    };
    if let Some(hit) =
        ray.and_then(|ray| planet.ground.raycast(ray.origin, *ray.direction, REACH_M))
    {
        // Only the cells around the point looked at are searched, never the whole forest.
        let nearest = neighbourhood(hit.point.normalize())
            .flat_map(|cell| {
                let chopped = forest.chopped.get(&cell).copied().unwrap_or(0);
                trees_in(&planet, forest.seed, cell)
                    .into_iter()
                    .filter(move |tree| chopped & (1 << tree.index) == 0)
                    .map(move |tree| (cell, tree.index, tree.base.distance(hit.point)))
            })
            .filter(|(_, _, distance)| *distance < GRAB_M)
            .min_by(|a, b| a.2.total_cmp(&b.2));
        match nearest {
            Some((cell, index, _)) => {
                *forest.chopped.entry(cell).or_insert(0) |= 1 << index;
                forest.operations.push(Operation::RemoveObject {
                    id: tree_id(cell, index),
                });
                forest.total.add(Harvested::Object(TREE), 1.0);
                rebuild.push(cell);
                info!("Chopped tree {index} in cell {cell}.");
            }
            None => info!("No tree near where you are pointing."),
        }
    }

    for cell in rebuild {
        if let Some(&entity) = forest.drawn.get(&cell) {
            match cell_mesh(&trees_in(&planet, forest.seed, cell), forest.chopped[&cell]) {
                Some(mesh) => commands.entity(entity).insert(Mesh3d(meshes.add(mesh))),
                None => commands.entity(entity).remove::<Mesh3d>(),
            };
        }
    }
}

fn update_readout(forest: Res<Forest>, mut readout: Single<&mut Text, With<Readout>>) {
    readout.0 = format!(
        "Forest: {} cells drawn, {} trees chopped ({} operations)\nLeft click or F chops a tree. K chops every tree in view.",
        forest.drawn.len(),
        forest.total.amount(Harvested::Object(TREE)),
        forest.operations.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::PlanetSpec;

    fn planet() -> Planet {
        let spec = PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        };
        Planet::new(spec)
    }

    #[test]
    fn every_direction_belongs_to_the_cell_that_contains_it() {
        let mut rng = Seed(4).rng();
        for _ in 0..5_000 {
            let cell = rng.below(CELL_COUNT);
            let (a, b) = (rng.range(0.02, 0.98), rng.range(0.02, 0.98));
            assert_eq!(cell_at(cell_direction(cell, a, b)), cell);
        }
        for _ in 0..5_000 {
            let direction = rng.unit_vector();
            assert!(cell_at(direction) < CELL_COUNT);
            assert!(neighbourhood(direction).any(|cell| cell == cell_at(direction)));
        }
    }

    #[test]
    fn a_cell_always_holds_the_same_trees_on_the_ground_inside_it() {
        let planet = planet();
        let seed = Seed(1).derive("forest");
        let mut any_trees = false;
        for cell in (0..CELL_COUNT).step_by(97) {
            let trees = trees_in(&planet, seed, cell);
            let again = trees_in(&planet, seed, cell);
            assert_eq!(trees.len(), again.len());
            for (tree, same) in trees.iter().zip(&again) {
                assert_eq!((tree.index, tree.base), (same.index, same.base));
                assert_eq!(cell_at(tree.up), cell);
                assert!(planet.ground.distance(tree.base).abs() < 0.01);
                assert!(tree.index < TREES_PER_CELL);
            }
            any_trees |= !trees.is_empty();
        }
        assert!(any_trees);
    }

    #[test]
    fn chopped_trees_are_left_out_of_the_mesh() {
        let planet = planet();
        let seed = Seed(1).derive("forest");
        let cell = (0..CELL_COUNT)
            .find(|&cell| trees_in(&planet, seed, cell).len() > 2)
            .unwrap();
        let trees = trees_in(&planet, seed, cell);
        let triangles = |mesh: Option<Mesh>| mesh.map_or(0, |mesh| mesh.count_vertices() / 3);
        let whole = triangles(cell_mesh(&trees, 0));
        let one_gone = triangles(cell_mesh(&trees, 1 << trees[0].index));
        let all_gone = triangles(cell_mesh(&trees, u128::MAX));
        assert_eq!(whole, trees.len() * 46);
        assert_eq!(one_gone, whole - 46);
        assert_eq!(all_gone, 0);
    }
}
