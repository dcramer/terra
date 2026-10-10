//! Construction: a base you shape, drawn from the rules in `terra_build`.
//!
//! Point the middle of the view at the base. G pushes the wall you are looking at
//! outward by a tile and H pulls it in. V cycles the tile you are looking at through
//! solid, window, wide window, tall window and door. Point at open ground and B adds a
//! room there.
//!
//! The base starts as a small example, built through the same operations the keys use.
//!
//! With the `play` experiment on, a player's builder tool drives this instead.

use crate::planet::Planet;
use bevy::{
    asset::RenderAssetUsages, light::NotShadowCaster, prelude::*,
    render::render_resource::PrimitiveTopology,
};
use terra_build::{
    Base, BuildOp, DOOR_WIDTH_M, Face, Panel, STOREY_M, Side, Surface, TILE_M, Walker,
};
use terra_core::Ground;

const REACH_M: f32 = 300.0;
/// The base's floor clears the highest ground under it by this much.
const FLOOR_CLEARANCE_M: f32 = 0.5;
/// A ramp runs this far out for every metre it has to climb.
const RAMP_RUN_PER_RISE: f32 = 2.4;
const RAMP_WIDTH_M: f32 = DOOR_WIDTH_M + 0.7;

/// Present when something else is driving construction, such as a player holding a
/// builder tool. The keys and camera here then stand aside.
#[derive(Resource)]
pub(super) struct DrivenByPlayer;

/// The one base under construction, and where it stands.
#[derive(Resource)]
pub(super) struct Site {
    pub base: Base,
    /// Places the base's own space on the planet.
    pub frame: Transform,
    pub operations: Vec<BuildOp>,
    /// A ramp from each ground-floor door down to the ground.
    ramps: Vec<Ramp>,
    ramp_holder: Entity,
    structure: Entity,
    glass: Entity,
    legs: Entity,
    leg_mesh: Handle<Mesh>,
    leg_material: Handle<StandardMaterial>,
    ramp_material: Handle<StandardMaterial>,
    camera_placed: bool,
}

#[derive(Component)]
struct Readout;

/// Marks that the construction systems are in place, so asking for them twice is safe.
#[derive(Resource)]
struct Added;

pub fn plugin(app: &mut App) {
    if app.world().contains_resource::<Added>() {
        return;
    }
    let free = not(resource_exists::<DrivenByPlayer>);
    app.insert_resource(Added)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                place_camera.run_if(free.clone()),
                edit.run_if(free.clone()),
                update_readout.run_if(free),
            )
                .chain(),
        );
}

/// A point just outside the example base's door, in the base's own space.
pub(super) const OUTSIDE_DOOR: Vec3 = Vec3::new(7.5, 0.0, 9.0);

/// A sloping walkway from a door's sill down to the ground, in the base's own space.
struct Ramp {
    /// The middle of the door's sill.
    top: Vec3,
    /// Level, pointing away from the door.
    outward: Vec3,
    length: f32,
    /// How far the far end is below the sill.
    drop: f32,
}

impl Ramp {
    /// The ramp as a wedge: its sloping top and a wall down each side.
    fn mesh(&self) -> Mesh {
        let half = self.outward.cross(Vec3::Y) * (RAMP_WIDTH_M * 0.5);
        let fall = self.outward * self.length - Vec3::Y * self.drop;
        let (near_left, near_right) = (self.top - half, self.top + half);
        let (far_left, far_right) = (near_left + fall, near_right + fall);
        let under = Vec3::Y * self.drop;
        let triangles = [
            [near_left, far_left, far_right],
            [near_left, far_right, near_right],
            [near_left, near_left - under, far_left],
            [near_right, far_right, near_right - under],
        ];
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        for [a, b, c] in triangles {
            let mut normal = (b - a).cross(c - a).normalize_or_zero();
            // The top should face up; the material is double sided, so the rest is free.
            if normal.y < 0.0 {
                normal = -normal;
            }
            positions.extend([a, b, c]);
            normals.extend([normal; 3]);
        }
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    }

    /// The height of the ramp's surface under a point, if the point is over the ramp.
    fn height_at(&self, point: Vec3) -> Option<f32> {
        let offset = point - self.top;
        let along = offset.dot(self.outward);
        let across = offset.dot(self.outward.cross(Vec3::Y));
        let on_ramp = (0.0..=self.length).contains(&along) && across.abs() <= RAMP_WIDTH_M * 0.5;
        on_ramp.then(|| self.top.y - self.drop * along / self.length)
    }
}

impl Site {
    /// A point on the ground a little beyond the foot of the first door's ramp, facing
    /// which a walker would climb the ramp into the base. Returns the point and the
    /// direction to face, both in the base's own space.
    pub(super) fn approach(&self) -> (Vec3, Vec3) {
        match self.ramps.first() {
            Some(ramp) => (ramp.top + ramp.outward * (ramp.length + 2.5), -ramp.outward),
            None => (OUTSIDE_DOOR, Vec3::NEG_Z),
        }
    }

    /// The height of whatever the base offers to stand on at a point in its own space: a
    /// floor, a roof or a ramp, whichever is highest within a step of the feet.
    pub(super) fn support(&self, point: Vec3, walker: &Walker) -> Option<f32> {
        let ramp = self
            .ramps
            .iter()
            .filter_map(|ramp| ramp.height_at(point))
            .filter(|&height| height <= point.y + walker.step_up)
            .max_by(|a, b| a.total_cmp(b));
        match (self.base.support(point, walker), ramp) {
            (Some(floor), Some(ramp)) => Some(floor.max(ramp)),
            (floor, ramp) => floor.or(ramp),
        }
    }

    /// Applies an operation and redraws the base. Returns whether anything changed.
    pub(super) fn apply(
        &mut self,
        op: BuildOp,
        commands: &mut Commands,
        planet: &Planet,
        meshes: &mut Assets<Mesh>,
    ) -> bool {
        let changed = self.base.apply(op);
        if changed {
            self.operations.push(op);
            info!("Applied {op:?}");
            redraw(commands, self, planet, meshes);
        } else {
            info!("{op:?} changed nothing.");
        }
        changed
    }
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
    let mut frame =
        Transform::from_translation(anchor).looking_to(ahead.reject_from(anchor_up), anchor_up);
    frame.rotate_local_y(0.5);
    let mut base = Base::default();
    let operations = example();
    for &op in &operations {
        base.apply(op);
    }
    // Lift the base until its floor clears the highest ground under it. The stand-in
    // planet cannot be dug, so the base has to stay out of the ground's way; legs and
    // ramps make up the difference.
    let highest_ground = base
        .tiles()
        .filter(|(tile, _)| tile.y == 0)
        .flat_map(|(tile, _)| {
            [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (0.5, 0.5)].map(|(dx, dz)| {
                let on_floor = Vec3::new(
                    (tile.x as f32 + dx) * TILE_M,
                    0.0,
                    (tile.z as f32 + dz) * TILE_M,
                );
                -planet.ground.distance(frame.transform_point(on_floor))
            })
        })
        .fold(f32::MIN, f32::max);
    frame.translation += anchor_up * (highest_ground + FLOOR_CLEARANCE_M);

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
    let ramp_holder = commands
        .spawn((Name::new("Base ramps"), frame, Visibility::default()))
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
        ramps: Vec::new(),
        ramp_holder,
        ramp_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.62, 0.63, 0.67),
            perceptual_roughness: 0.9,
            cull_mode: None,
            double_sided: true,
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

    // A ramp from every ground-floor door down to the ground.
    commands
        .entity(site.ramp_holder)
        .despawn_related::<Children>();
    site.ramps.clear();
    let doors: Vec<Face> = site
        .base
        .hull()
        .filter(|&face| face.tile[1] == 0 && site.base.panel(face) == Panel::Door)
        .collect();
    for door in doors {
        let corners = Base::corners(door);
        let top = (corners[0] + corners[1]) * 0.5;
        let outward = door.side.offset().as_vec3();
        // Long enough for the climb, measured where the ramp starts and where it ends.
        let rise_here = planet
            .ground
            .distance(site.frame.transform_point(top))
            .max(0.0);
        let length = (rise_here * RAMP_RUN_PER_RISE).clamp(1.5, 16.0);
        let far_end = site.frame.transform_point(top + outward * length);
        let drop = planet.ground.distance(far_end).max(0.0) + 0.1;
        let ramp = Ramp {
            top,
            outward,
            length,
            drop,
        };
        commands.spawn((
            Mesh3d(meshes.add(ramp.mesh())),
            MeshMaterial3d(site.ramp_material.clone()),
            ChildOf(site.ramp_holder),
        ));
        site.ramps.push(ramp);
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
        site.apply(op, &mut commands, &planet, &mut meshes);
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
