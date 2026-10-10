//! Construction: a base you shape, drawn from the rules in `terra_build`.
//!
//! Point the middle of the view at the base. G pushes the wall you are looking at
//! outward by a tile and H pulls it in. V cycles the tile you are looking at through
//! solid, window, wide window, tall window and door. Point at open ground and B adds a
//! room there.
//!
//! The base starts as a small example, built through the same operations the keys use.

use crate::planet::Planet;
use bevy::{
    asset::RenderAssetUsages, light::NotShadowCaster, prelude::*,
    render::render_resource::PrimitiveTopology,
};
use terra_build::{Base, BuildOp, Face, Panel, STOREY_M, Side, Surface, TILE_M};
use terra_core::Ground;

const REACH_M: f32 = 300.0;
/// The base's floor sits this far above the ground at its anchor, on legs.
const FLOOR_ABOVE_GROUND_M: f32 = 0.8;

/// The one base under construction, and where it stands.
#[derive(Resource)]
struct Site {
    base: Base,
    /// Places the base's own space on the planet.
    frame: Transform,
    operations: Vec<BuildOp>,
    structure: Entity,
    glass: Entity,
    legs: Entity,
    leg_mesh: Handle<Mesh>,
    leg_material: Handle<StandardMaterial>,
    camera_placed: bool,
}

#[derive(Component)]
struct Readout;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (place_camera, edit, update_readout).chain());
}

/// The example base: two rooms joined by a corridor, a small upper storey, a run of
/// windows and a door.
fn example() -> Vec<BuildOp> {
    let wall = |x: i32, y: i32, z: i32, side: Side| Face::new(IVec3::new(x, y, z), side);
    vec![
        BuildOp::AddRoom {
            min: [-2, 0, -2],
            max: [1, 0, 0],
        },
        BuildOp::Grow(wall(1, 0, -1, Side::East)),
        BuildOp::AddRoom {
            min: [3, 0, -1],
            max: [4, 0, -1],
        },
        BuildOp::AddRoom {
            min: [5, 0, -2],
            max: [6, 0, 0],
        },
        BuildOp::AddRoom {
            min: [-1, 1, -2],
            max: [0, 1, -1],
        },
        BuildOp::SetPanel(wall(-1, 0, 0, Side::South), Panel::WIDE_WINDOW),
        BuildOp::SetPanel(wall(0, 0, 0, Side::South), Panel::WIDE_WINDOW),
        BuildOp::SetPanel(wall(1, 0, 0, Side::South), Panel::WIDE_WINDOW),
        BuildOp::SetPanel(wall(2, 0, 0, Side::South), Panel::Door),
        BuildOp::SetPanel(wall(-2, 0, -1, Side::West), Panel::WINDOW),
        BuildOp::SetPanel(wall(-1, 1, -1, Side::South), Panel::TALL_WINDOW),
        BuildOp::SetPanel(wall(0, 1, -1, Side::South), Panel::TALL_WINDOW),
        BuildOp::SetPanel(wall(4, 0, -1, Side::South), Panel::WINDOW),
        BuildOp::SetPanel(wall(6, 0, 0, Side::South), Panel::WINDOW),
        BuildOp::SetPanel(wall(6, 0, -1, Side::East), Panel::TALL_WINDOW),
    ]
}

fn setup(
    mut commands: Commands,
    planet: Res<Planet>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Stand the base on the ground a short walk ahead of where the camera starts.
    let start = planet.start();
    let up = start.normalize();
    let ahead = up.cross(Vec3::X).normalize();
    let anchor = planet.surface((start + ahead * 60.0).normalize());
    let anchor_up = anchor.normalize();
    let mut frame = Transform::from_translation(anchor + anchor_up * FLOOR_ABOVE_GROUND_M)
        .looking_to(ahead.reject_from(anchor_up), anchor_up);
    frame.rotate_local_y(0.5);

    let mut base = Base::default();
    let operations = example();
    for &op in &operations {
        base.apply(op);
    }

    // Colour comes from each triangle, so one material serves the whole structure.
    let structure_material = materials.add(StandardMaterial {
        perceptual_roughness: 0.85,
        ..default()
    });
    let glass_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.7, 0.9, 1.0, 0.28),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.08,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let structure = commands
        .spawn((Name::new("Base"), frame, MeshMaterial3d(structure_material)))
        .id();
    let glass = commands
        .spawn((
            Name::new("Base glass"),
            frame,
            MeshMaterial3d(glass_material),
            NotShadowCaster,
        ))
        .id();
    let legs = commands
        .spawn((
            Name::new("Base legs"),
            Transform::default(),
            Visibility::default(),
        ))
        .id();

    let mut site = Site {
        base,
        frame,
        operations,
        structure,
        glass,
        legs,
        leg_mesh: meshes.add(Cuboid::new(0.35, 1.0, 0.35)),
        leg_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 0.41, 0.45),
            perceptual_roughness: 0.9,
            ..default()
        }),
        camera_placed: false,
    };
    redraw(&mut commands, &mut site, &planet, &mut meshes);
    commands.insert_resource(site);

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
    // A mark in the middle of the view, which is what the build keys aim with.
    commands.spawn((
        Text::new("+"),
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            margin: UiRect {
                left: px(-5),
                top: px(-12),
                ..default()
            },
            ..default()
        },
    ));
}

fn to_mesh(surface: &Surface) -> Option<Mesh> {
    if surface.positions.is_empty() {
        return None;
    }
    let colours: Vec<[f32; 4]> = surface
        .colours
        .iter()
        .map(|c| [c[0], c[1], c[2], 1.0])
        .collect();
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, surface.positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, surface.normals.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours),
    )
}

/// Rebuilds everything drawn for the base from its current shape.
fn redraw(commands: &mut Commands, site: &mut Site, planet: &Planet, meshes: &mut Assets<Mesh>) {
    let mesh = site.base.mesh();
    for (entity, surface) in [(site.structure, &mesh.structure), (site.glass, &mesh.glass)] {
        match to_mesh(surface) {
            Some(mesh) => commands.entity(entity).insert(Mesh3d(meshes.add(mesh))),
            None => commands.entity(entity).remove::<Mesh3d>(),
        };
    }

    // A leg under every ground-floor tile that stands clear of the ground.
    commands.entity(site.legs).despawn_related::<Children>();
    for face in site.base.hull().filter(|face| face.side == Side::Down) {
        let tile = face.tile().as_vec3();
        let under = Vec3::new(
            (tile.x + 0.5) * TILE_M,
            tile.y * STOREY_M,
            (tile.z + 0.5) * TILE_M,
        );
        let point = site.frame.transform_point(under);
        let gap = planet.ground.distance(point);
        if tile.y == 0.0 && gap > 0.15 {
            let length = gap + 0.6;
            let down = -site.frame.up();
            commands.spawn((
                Mesh3d(site.leg_mesh.clone()),
                MeshMaterial3d(site.leg_material.clone()),
                Transform::from_translation(point + down * (length * 0.5))
                    .with_rotation(site.frame.rotation)
                    .with_scale(Vec3::new(1.0, length, 1.0)),
                ChildOf(site.legs),
            ));
        }
    }
}

/// Moves the camera to a good view of the base, once.
fn place_camera(mut site: ResMut<Site>, mut camera: Single<&mut Transform, With<Camera3d>>) {
    if site.camera_placed {
        return;
    }
    site.camera_placed = true;
    let frame = site.frame;
    let eye = frame.transform_point(Vec3::new(4.0, 6.5, 26.0));
    let target = frame.transform_point(Vec3::new(6.0, 2.0, -3.0));
    **camera = Transform::from_translation(eye).looking_at(target, *frame.up());
}

fn edit(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    planet: Res<Planet>,
    mut site: ResMut<Site>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut gizmos: Gizmos,
) {
    let site = &mut *site;
    // Aim with the middle of the view, in the base's own space.
    let to_base = site.frame.compute_affine().inverse();
    let origin = to_base.transform_point3(camera.translation());
    let direction = to_base.transform_vector3(*camera.forward()).normalize();
    let aimed = site.base.raycast(origin, direction, REACH_M);

    // Show what a key press would act on: the whole wall faintly, the one tile brightly.
    if let Some(hit) = aimed {
        let outline = |gizmos: &mut Gizmos, face: Face, colour: Color| {
            let corners = Base::corners(face).map(|corner| site.frame.transform_point(corner));
            gizmos.linestrip(
                [corners[0], corners[1], corners[2], corners[3], corners[0]],
                colour,
            );
        };
        for face in site.base.wall(hit.face) {
            outline(&mut gizmos, face, Color::srgba(1.0, 1.0, 1.0, 0.35));
        }
        outline(&mut gizmos, hit.face, Color::srgb(1.0, 0.85, 0.2));
    }

    let op = if let Some(hit) = aimed {
        if keys.just_pressed(KeyCode::KeyG) {
            Some(BuildOp::Grow(hit.face))
        } else if keys.just_pressed(KeyCode::KeyH) {
            Some(BuildOp::Shrink(hit.face))
        } else if keys.just_pressed(KeyCode::KeyV) {
            Some(BuildOp::SetPanel(
                hit.face,
                site.base.panel(hit.face).next(),
            ))
        } else {
            None
        }
    } else if keys.just_pressed(KeyCode::KeyB) {
        // A new room, three tiles square, on the ground being looked at.
        planet
            .ground
            .raycast(camera.translation(), *camera.forward(), REACH_M)
            .map(|ground| {
                let local = to_base.transform_point3(ground.point);
                let (x, z) = (
                    (local.x / TILE_M).floor() as i32,
                    (local.z / TILE_M).floor() as i32,
                );
                BuildOp::AddRoom {
                    min: [x - 1, 0, z - 1],
                    max: [x + 1, 0, z + 1],
                }
            })
    } else {
        None
    };

    if let Some(op) = op {
        if site.base.apply(op) {
            site.operations.push(op);
            info!("Applied {op:?}");
            redraw(&mut commands, site, &planet, &mut meshes);
        } else {
            info!("{op:?} changed nothing.");
        }
    }
}

fn update_readout(site: Res<Site>, mut readout: Single<&mut Text, With<Readout>>) {
    let windows = site
        .base
        .hull()
        .filter(|&face| matches!(site.base.panel(face), Panel::Window { .. }))
        .count();
    readout.0 = format!(
        "Base: {} tiles, {} hull faces, {} window tiles, {} operations\n\
         Aim with the + mark. G grow wall, H shrink wall, V window or door, B new room on open ground.",
        site.base.tile_count(),
        site.base.hull().count(),
        windows,
        site.operations.len()
    );
}
