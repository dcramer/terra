//! The map: a village square of timber-framed houses about a well, market stalls, a
//! campfire, lamp posts and trees, built from the game's own materials and models; and the
//! light, at day, dusk or night.

use crate::mesh;
use crate::nav::Nav;
use bevy::image::ImageLoaderSettings;
use bevy::light::{CascadeShadowConfigBuilder, light_consts::lux};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

pub fn plugin(app: &mut App) {
    app.init_resource::<Nav>()
        .add_systems(Startup, build)
        .add_systems(Update, (light_the_hour, flicker));
}

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hour {
    Day,
    Dusk,
    Night,
}

impl Hour {
    pub fn next(self) -> Hour {
        match self {
            Hour::Day => Hour::Dusk,
            Hour::Dusk => Hour::Night,
            Hour::Night => Hour::Day,
        }
    }
}

#[derive(Component)]
struct Sun;

/// A light that burns at night: a lamp's flame or the fire.
#[derive(Component)]
struct Flame {
    lumens: f32,
    seed: f32,
    fire: bool,
}

#[derive(Resource)]
struct Glow {
    windows: Handle<StandardMaterial>,
    flames: Handle<StandardMaterial>,
}

/// The materials, each `color.jpg` with its `normal.jpg` and `arm.jpg` (occlusion,
/// roughness, metal) from `assets/materials/<name>`.
fn material(asset: &AssetServer, name: &str, tint: Color) -> StandardMaterial {
    let linear = |file: &str| {
        asset
            .load_builder()
            .with_settings(|s: &mut ImageLoaderSettings| s.is_srgb = false)
            .load(format!("materials/{name}/{file}"))
    };
    let arm: Handle<Image> = linear("arm.jpg");
    StandardMaterial {
        base_color: tint,
        base_color_texture: Some(asset.load(format!("materials/{name}/color.jpg"))),
        normal_map_texture: Some(linear("normal.jpg")),
        metallic_roughness_texture: Some(arm.clone()),
        occlusion_texture: Some(arm),
        perceptual_roughness: 1.0,
        metallic: 1.0,
        ..default()
    }
}

struct Kit<'a, 'w, 's> {
    commands: Commands<'w, 's>,
    asset: &'a AssetServer,
    meshes: ResMut<'w, Assets<Mesh>>,
    nav: ResMut<'w, Nav>,
}

impl Kit<'_, '_, '_> {
    fn model(&mut self, path: &str, at: Vec3, yaw: f32, scale: f32) -> Entity {
        self.commands
            .spawn((
                WorldAssetRoot(
                    self.asset
                        .load(GltfAssetLabel::Scene(0).from_asset(path.to_string())),
                ),
                Transform::from_translation(at)
                    .with_rotation(Quat::from_rotation_y(yaw))
                    .with_scale(Vec3::splat(scale)),
            ))
            .id()
    }

    /// A model that blocks walking over a `half`-sized footprint.
    fn prop(&mut self, path: &str, x: f32, z: f32, yaw: f32, half: Vec2) -> Entity {
        self.nav.block_rect(Vec2::new(x, z), half, yaw);
        self.model(path, Vec3::new(x, 0.0, z), yaw, 1.0)
    }

    fn block(
        &mut self,
        parent: Entity,
        mat: &Handle<StandardMaterial>,
        size: Vec3,
        at: Transform,
        tile: f32,
    ) {
        let mesh = self.meshes.add(mesh::block(size, tile));
        self.commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat.clone()),
            at,
            ChildOf(parent),
        ));
    }
}

struct Mats {
    stone: Handle<StandardMaterial>,
    plaster: [Handle<StandardMaterial>; 2],
    timber: Handle<StandardMaterial>,
    tiles: Handle<StandardMaterial>,
    thatch: Handle<StandardMaterial>,
    planks: Handle<StandardMaterial>,
    windows: Handle<StandardMaterial>,
}

#[derive(Clone, Copy)]
enum Roof {
    Tiles,
    Thatch,
}

struct House {
    at: Vec2,
    yaw: f32,
    width: f32,
    depth: f32,
    height: f32,
    roof: Roof,
    plaster: usize,
    door: f32,
}

fn build(
    mut commands: Commands,
    asset: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    nav: ResMut<Nav>,
) {
    commands.insert_resource(Hour::Dusk);
    let wood_tint = Color::srgb(0.42, 0.33, 0.27);
    let mats = Mats {
        stone: materials.add(material(&asset, "stone_wall", Color::WHITE)),
        plaster: [
            materials.add(material(
                &asset,
                "white_stucco",
                Color::srgb(0.93, 0.88, 0.78),
            )),
            materials.add(material(
                &asset,
                "patterned_clay_plaster",
                Color::srgb(0.95, 0.9, 0.82),
            )),
        ],
        timber: materials.add(material(&asset, "rough_wood", wood_tint)),
        tiles: materials.add(material(
            &asset,
            "grey_roof_tiles_02",
            Color::srgb(0.85, 0.62, 0.5),
        )),
        thatch: materials.add(material(&asset, "riet_01", Color::WHITE)),
        planks: materials.add(material(
            &asset,
            "weathered_planks",
            Color::srgb(0.7, 0.62, 0.55),
        )),
        windows: materials.add(StandardMaterial {
            base_color: Color::srgb(0.05, 0.04, 0.03),
            emissive: LinearRgba::rgb(4.0, 1.8, 0.6),
            perceptual_roughness: 0.3,
            ..default()
        }),
    };
    let flames = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.7, 0.3),
        emissive: LinearRgba::rgb(30.0, 12.0, 3.0),
        unlit: true,
        ..default()
    });
    commands.insert_resource(Glow {
        windows: mats.windows.clone(),
        flames: flames.clone(),
    });

    // Ground: packed earth everywhere, cobbles over the square, fading at a ragged edge.
    let mut earth = material(&asset, "packed_earth", Color::srgb(0.8, 0.76, 0.7));
    earth.uv_transform = bevy::math::Affine2::from_scale(Vec2::splat(120.0 / 2.5));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(120.0, 120.0))),
        MeshMaterial3d(materials.add(earth)),
    ));
    let cobbles = material(&asset, "grey_stone_path", Color::srgb(0.85, 0.82, 0.78));
    let edge = |p: Vec2| {
        let a = p.to_angle();
        let r = 10.5 + 0.9 * (a * 3.0).sin() + 0.5 * (a * 7.0 + 1.3).sin() + 0.3 * (a * 13.0).cos();
        // The street to the south-east runs out of the square.
        let street = (p.x - p.y * -0.35 - 0.0).abs() < 2.2 && p.y > 0.0;
        if street { 1.0 } else { (r - p.length()) / 1.2 }
    };
    commands.spawn((
        Mesh3d(meshes.add(mesh::patch(24.0, 0.25, 1.8, edge))),
        MeshMaterial3d(materials.add(cobbles)),
        Transform::from_xyz(0.0, 0.015, 0.0),
    ));

    let mut kit = Kit {
        commands,
        asset: &asset,
        meshes,
        nav,
    };

    let houses = [
        House {
            at: Vec2::new(-8.5, -10.0),
            yaw: 0.0,
            width: 9.0,
            depth: 6.0,
            height: 4.6,
            roof: Roof::Tiles,
            plaster: 0,
            door: -1.5,
        },
        House {
            at: Vec2::new(4.5, -11.0),
            yaw: 0.0,
            width: 7.0,
            depth: 5.5,
            height: 4.0,
            roof: Roof::Thatch,
            plaster: 1,
            door: 1.2,
        },
        House {
            at: Vec2::new(14.5, -2.0),
            yaw: -FRAC_PI_2,
            width: 8.0,
            depth: 6.0,
            height: 4.4,
            roof: Roof::Tiles,
            plaster: 1,
            door: 0.5,
        },
        House {
            at: Vec2::new(-15.5, 1.5),
            yaw: FRAC_PI_2,
            width: 7.0,
            depth: 5.5,
            height: 4.0,
            roof: Roof::Thatch,
            plaster: 0,
            door: -1.0,
        },
    ];
    for house in &houses {
        build_house(&mut kit, &mats, &flames, house);
    }

    // The well in the middle, and the market about it.
    kit.prop("models/props/Well.glb", 0.0, 0.0, 0.3, Vec2::splat(0.85));
    kit.prop(
        "models/props/MarketStallBlue.glb",
        -4.5,
        -3.5,
        0.25,
        Vec2::new(1.3, 1.0),
    );
    kit.prop(
        "models/props/MarketStallOchre.glb",
        4.0,
        -4.5,
        -0.3,
        Vec2::new(1.3, 1.0),
    );
    kit.prop(
        "models/props/MarketStallGreen.glb",
        -6.0,
        3.0,
        1.9,
        Vec2::new(1.3, 1.0),
    );
    for (path, x, z, yaw) in [
        ("models/props/AppleCrate.glb", -3.0, -2.2, 0.4),
        ("models/props/RootCrate.glb", -2.6, -3.0, 1.2),
        ("models/props/Barrel.glb", 5.8, -3.2, 0.0),
        ("models/props/Barrel.glb", 6.3, -3.9, 1.0),
        ("models/props/Sack.glb", 2.6, -3.4, 0.7),
        ("models/props/MarketBasket.glb", 2.9, -2.8, 0.2),
        ("models/props/ClothBolts.glb", -7.3, 2.0, 0.5),
        ("models/props/Buckets.glb", 1.2, 1.3, 0.0),
        ("models/props/Trough.glb", 9.5, -5.5, 1.57),
        ("models/props/HandCart.glb", 9.0, 5.5, -0.6),
        ("models/props/Barrel.glb", -12.0, -6.0, 0.4),
        ("models/props/Barrel.glb", -11.3, -6.3, 2.0),
        ("models/props/BoltCrate.glb", -5.0, -6.3, 0.1),
        ("models/props/Workbench.glb", 8.0, -9.5, 0.0),
        ("models/props/ToolBucket.glb", 9.4, -9.3, 0.4),
        ("models/props/Log.glb", 11.5, -8.0, 0.8),
        ("models/props/Chest.glb", -13.0, -2.5, 1.57),
    ] {
        kit.prop(path, x, z, yaw, Vec2::splat(0.4));
    }

    // The campfire where the party rests, with logs to sit on.
    kit.prop(
        "models/props/Campfire.glb",
        6.5,
        8.0,
        0.0,
        Vec2::splat(0.75),
    );
    kit.prop(
        "models/props/Log.glb",
        6.4,
        10.0,
        0.1,
        Vec2::new(0.85, 0.25),
    );
    kit.prop("models/props/Log.glb", 8.6, 7.4, 1.8, Vec2::new(0.85, 0.25));
    kit.prop(
        "models/props/Log.glb",
        4.5,
        7.2,
        -1.2,
        Vec2::new(0.85, 0.25),
    );
    let fire = kit
        .commands
        .spawn((
            PointLight {
                color: Color::srgb(1.0, 0.55, 0.22),
                intensity: 60_000.0,
                range: 18.0,
                radius: 0.3,
                shadow_maps_enabled: true,
                ..default()
            },
            Transform::from_xyz(6.5, 0.7, 8.0),
            Flame {
                lumens: 60_000.0,
                seed: 0.0,
                fire: true,
            },
        ))
        .id();
    let _ = fire;

    // Lamp posts about the square.
    let post = mats.timber.clone();
    for (i, (x, z)) in [(-3.0, 5.5), (5.5, -1.0), (-8.0, -4.5), (10.5, 1.5)]
        .into_iter()
        .enumerate()
    {
        let root = kit
            .commands
            .spawn((Transform::from_xyz(x, 0.0, z), Visibility::default()))
            .id();
        kit.nav.block_circle(Vec2::new(x, z), 0.15);
        kit.block(
            root,
            &post,
            Vec3::new(0.16, 2.6, 0.16),
            Transform::from_xyz(0.0, 1.3, 0.0),
            0.5,
        );
        kit.block(
            root,
            &post,
            Vec3::new(0.6, 0.1, 0.1),
            Transform::from_xyz(0.25, 2.55, 0.0),
            0.5,
        );
        let lantern = kit.model(
            "models/props/Lantern.glb",
            Vec3::new(0.48, 1.97, 0.0),
            0.0,
            1.0,
        );
        kit.commands.entity(lantern).insert(ChildOf(root));
        flame_in(
            &mut kit,
            root,
            &flames,
            Vec3::new(0.48, 2.2, 0.0),
            i < 2,
            i as f32 * 1.7 + 0.5,
        );
    }

    // Trees about the edge, mostly behind the houses so they never hide the party.
    for (path, x, z, yaw, scale) in [
        ("models/nature/Oak.glb", -2.0, -24.0, 0.3, 0.75),
        ("models/nature/Maple.glb", 15.0, -17.0, 1.2, 0.65),
        ("models/nature/Willow.glb", -24.0, -12.0, 2.0, 0.8),
        ("models/nature/Yew.glb", 23.0, 6.0, 0.7, 0.55),
        ("models/nature/Oak.glb", -25.0, 9.0, 4.0, 0.6),
        ("models/nature/Maple.glb", 21.0, -8.0, 3.0, 0.55),
    ] {
        kit.nav.block_circle(Vec2::new(x, z), 0.8 * scale);
        kit.model(path, Vec3::new(x, 0.0, z), yaw, scale);
    }
    for (i, (x, z)) in [
        (-19.0, -4.0),
        (17.0, 11.0),
        (-3.0, 16.0),
        (20.0, -12.0),
        (-20.0, 17.0),
        (11.0, 15.0),
    ]
    .into_iter()
    .enumerate()
    {
        let path = format!("models/nature/Rock_Moss_{}.glb", i % 5 + 1);
        kit.nav.block_circle(Vec2::new(x, z), 1.0);
        kit.model(&path, Vec3::new(x, 0.0, z), i as f32 * 1.9, 0.8);
    }

    // Grass and ferns over the earth, never on the cobbles.
    let mut seed = 0x9e37_79b9u32;
    let mut rand = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32
    };
    let mut placed = 0;
    while placed < 700 {
        let p = Vec2::new(rand() * 56.0 - 28.0, rand() * 56.0 - 28.0);
        if edge(p) > -0.4
            || kit.nav.is_blocked(
                ((p + crate::nav::HALF) / crate::nav::CELL)
                    .floor()
                    .as_ivec2(),
            )
        {
            continue;
        }
        let path = match (rand() * 10.0) as u32 {
            0 => "models/nature/forest/fern.glb",
            1..=5 => "models/nature/Grass.glb",
            _ => "models/nature/Grass_2.glb",
        };
        let scale = 0.8 + rand() * 0.6;
        kit.model(path, Vec3::new(p.x, 0.0, p.y), rand() * 6.28, scale);
        placed += 1;
    }

    // The light: the sun (or moon), a sky's fill, and a little haze.
    kit.commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            soft_shadow_size: Some(1.5),
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 2,
            minimum_distance: 1.0,
            first_cascade_far_bound: 70.0,
            maximum_distance: 140.0,
            overlap_proportion: 0.2,
        }
        .build(),
        Transform::default(),
        Sun,
    ));
}

fn build_house(kit: &mut Kit, mats: &Mats, flames: &Handle<StandardMaterial>, h: &House) {
    let (w, d, ht) = (h.width, h.depth, h.height);
    kit.nav
        .block_rect(h.at, Vec2::new(w / 2.0 + 0.1, d / 2.0 + 0.1), h.yaw);
    let root = kit
        .commands
        .spawn((
            Transform::from_xyz(h.at.x, 0.0, h.at.y).with_rotation(Quat::from_rotation_y(h.yaw)),
            Visibility::default(),
        ))
        .id();
    let plinth = 0.7;
    kit.block(
        root,
        &mats.stone,
        Vec3::new(w + 0.12, plinth, d + 0.12),
        Transform::from_xyz(0.0, plinth / 2.0, 0.0),
        2.0,
    );
    let wall = ht - plinth;
    kit.block(
        root,
        &mats.plaster[h.plaster],
        Vec3::new(w, wall, d),
        Transform::from_xyz(0.0, plinth + wall / 2.0, 0.0),
        2.0,
    );

    // The timber frame: corner and door posts, a sill and a wall plate, and braces.
    let t = 0.22;
    let timber = mats.timber.clone();
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        kit.block(
            root,
            &timber,
            Vec3::new(t, wall, t),
            Transform::from_xyz(sx * (w / 2.0), plinth + wall / 2.0, sz * (d / 2.0)),
            1.0,
        );
    }
    for (y, size) in [
        (plinth + 0.08, 0.16),
        (ht - 0.1, 0.22),
        (plinth + wall * 0.52, 0.14),
    ] {
        kit.block(
            root,
            &timber,
            Vec3::new(w + 0.06, size, 0.12),
            Transform::from_xyz(0.0, y, d / 2.0 + 0.04),
            1.0,
        );
        kit.block(
            root,
            &timber,
            Vec3::new(w + 0.06, size, 0.12),
            Transform::from_xyz(0.0, y, -d / 2.0 - 0.04),
            1.0,
        );
        kit.block(
            root,
            &timber,
            Vec3::new(0.12, size, d + 0.06),
            Transform::from_xyz(w / 2.0 + 0.04, y, 0.0),
            1.0,
        );
        kit.block(
            root,
            &timber,
            Vec3::new(0.12, size, d + 0.06),
            Transform::from_xyz(-w / 2.0 - 0.04, y, 0.0),
            1.0,
        );
    }
    let bays = (w / 2.2).round().max(2.0) as i32;
    for i in 1..bays {
        let x = -w / 2.0 + w * i as f32 / bays as f32;
        for z in [d / 2.0 + 0.05, -d / 2.0 - 0.05] {
            kit.block(
                root,
                &timber,
                Vec3::new(0.16, wall, 0.1),
                Transform::from_xyz(x, plinth + wall / 2.0, z),
                1.0,
            );
        }
    }
    // Braces in the end bays of the front, leaning in from the corners.
    let bay = w / bays as f32;
    let rise = wall * 0.48;
    let len = (bay * bay + rise * rise).sqrt();
    let lean = (bay / rise).atan();
    for (x, s) in [(-w / 2.0 + bay / 2.0, -1.0), (w / 2.0 - bay / 2.0, 1.0)] {
        let tr = Transform::from_xyz(x, plinth + wall * 0.76, d / 2.0 + 0.05)
            .with_rotation(Quat::from_rotation_z(s * lean));
        kit.block(root, &timber, Vec3::new(0.13, len, 0.08), tr, 1.0);
    }

    // The roof: two slopes over the long side, gables at the ends, a chimney.
    let pitch = if matches!(h.roof, Roof::Thatch) {
        48f32
    } else {
        40f32
    }
    .to_radians();
    let over = 0.45;
    let run = d / 2.0 + over;
    let roof_rise = run * pitch.tan();
    let slope = run / pitch.cos();
    let (mat, thick, tile) = match h.roof {
        Roof::Tiles => (&mats.tiles, 0.14, 1.5),
        Roof::Thatch => (&mats.thatch, 0.38, 2.5),
    };
    let gable_rise = (d / 2.0) * pitch.tan();
    let ridge = ht + gable_rise;
    for s in [-1.0f32, 1.0] {
        // From the ridge down to the eave, lifted by half its thickness off the gables.
        let normal = Vec3::new(0.0, pitch.cos(), s * pitch.sin());
        let centre = Vec3::new(0.0, ridge - roof_rise / 2.0, s * run / 2.0) + normal * thick / 2.0;
        let tr =
            Transform::from_translation(centre).with_rotation(Quat::from_rotation_x(s * pitch));
        kit.block(root, mat, Vec3::new(w + 2.0 * over, thick, slope), tr, tile);
    }
    kit.block(
        root,
        mat,
        Vec3::new(w + 2.0 * over, thick * 1.2, thick * 2.2),
        Transform::from_xyz(0.0, ridge + thick * 0.9, 0.0),
        tile,
    );
    let gable = kit.meshes.add(mesh::gable(d, gable_rise, 2.0));
    for s in [-1.0f32, 1.0] {
        kit.commands.spawn((
            Mesh3d(gable.clone()),
            MeshMaterial3d(mats.plaster[h.plaster].clone()),
            Transform::from_xyz(s * w / 2.0, ht, 0.0)
                .with_rotation(Quat::from_rotation_y(FRAC_PI_2)),
            ChildOf(root),
        ));
    }
    kit.block(
        root,
        &mats.stone,
        Vec3::new(0.75, 2.6, 0.75),
        Transform::from_xyz(w * 0.28, ht + gable_rise * 0.55, -d * 0.22),
        1.0,
    );

    // The door, a step, windows with shutters thrown open, and a lamp by the door.
    let door = kit.model(
        "models/openings/PlankDoor.glb",
        Vec3::new(h.door, 0.0, d / 2.0 + 0.07),
        0.0,
        1.05,
    );
    kit.commands.entity(door).insert(ChildOf(root));
    kit.block(
        root,
        &timber,
        Vec3::new(1.15, 2.2, 0.06),
        Transform::from_xyz(h.door, 1.1, d / 2.0 + 0.04),
        1.0,
    );
    kit.block(
        root,
        &mats.stone,
        Vec3::new(1.4, 0.18, 0.6),
        Transform::from_xyz(h.door, 0.09, d / 2.0 + 0.36),
        1.0,
    );
    let pane = kit.meshes.add(mesh::block(Vec3::new(0.9, 0.85, 0.05), 1.0));
    for i in 0..bays {
        let x = -w / 2.0 + bay * (i as f32 + 0.5);
        if (x - h.door).abs() < 1.0 {
            continue;
        }
        for (z, yaw) in [(d / 2.0 + 0.03, 0.0), (-d / 2.0 - 0.03, PI)] {
            let y = plinth + wall * 0.25 + 0.45;
            kit.commands.spawn((
                Mesh3d(pane.clone()),
                MeshMaterial3d(mats.windows.clone()),
                Transform::from_xyz(x, y, z),
                ChildOf(root),
            ));
            let sign = if z > 0.0 { 1.0 } else { -1.0 };
            for side in [-1.0f32, 1.0] {
                let shutter = kit.model(
                    "models/openings/BoardShutter.glb",
                    Vec3::new(x + side * 0.7, y - 0.5, z + sign * 0.02),
                    yaw,
                    0.95,
                );
                kit.commands.entity(shutter).insert(ChildOf(root));
            }
            // Upstairs windows, under the eaves.
            kit.commands.spawn((
                Mesh3d(pane.clone()),
                MeshMaterial3d(mats.windows.clone()),
                Transform::from_xyz(x, plinth + wall * 0.76, z)
                    .with_scale(Vec3::new(0.8, 0.8, 1.0)),
                ChildOf(root),
            ));
        }
    }
    let lamp = kit.model(
        "models/props/Lantern.glb",
        Vec3::new(h.door + 0.85, 2.05, d / 2.0 + 0.2),
        0.0,
        0.8,
    );
    kit.commands.entity(lamp).insert(ChildOf(root));
    flame_in(
        kit,
        root,
        flames,
        Vec3::new(h.door + 0.85, 2.25, d / 2.0 + 0.25),
        false,
        h.at.x,
    );
    let _ = &mats.planks;
}

fn flame_in(
    kit: &mut Kit,
    parent: Entity,
    flames: &Handle<StandardMaterial>,
    at: Vec3,
    shadows: bool,
    seed: f32,
) {
    let ball = kit.meshes.add(Sphere::new(0.035).mesh().uv(8, 6));
    kit.commands.spawn((
        Mesh3d(ball),
        MeshMaterial3d(flames.clone()),
        Transform::from_translation(at - Vec3::Y * 0.12),
        ChildOf(parent),
    ));
    kit.commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.68, 0.38),
            intensity: 9_000.0,
            range: 9.0,
            radius: 0.05,
            shadow_maps_enabled: shadows,
            ..default()
        },
        Transform::from_translation(at - Vec3::Y * 0.12),
        Flame {
            lumens: 9_000.0,
            seed,
            fire: false,
        },
        ChildOf(parent),
    ));
}

/// The sun's place and colour, the sky's fill and the lamps, for the hour.
fn light_the_hour(
    hour: Res<Hour>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), With<Sun>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    glow: Res<Glow>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut flames: Query<(&mut Flame, &mut Visibility)>,
) {
    if !hour.is_changed() {
        return;
    }
    // Elevation, azimuth from the south toward the east, colour, lux, sky colour and nits,
    // window glow, and how bright the flames burn.
    let (el, az, color, lux_, sky, nits, windows, lit) = match *hour {
        Hour::Day => (
            52f32,
            -40f32,
            Color::srgb(1.0, 0.96, 0.9),
            lux::AMBIENT_DAYLIGHT * 0.12,
            Color::srgb(0.75, 0.85, 1.0),
            450.0,
            0.0f32,
            0.0f32,
        ),
        Hour::Dusk => (
            16.0,
            -58.0,
            Color::srgb(1.0, 0.64, 0.4),
            5_000.0,
            Color::srgb(0.55, 0.62, 0.9),
            900.0,
            1.0,
            1.0,
        ),
        Hour::Night => (
            48.0,
            30.0,
            Color::srgb(0.55, 0.65, 1.0),
            300.0,
            Color::srgb(0.3, 0.38, 0.7),
            160.0,
            1.6,
            1.4,
        ),
    };
    if let Ok((mut light, mut tr)) = sun.single_mut() {
        light.color = color;
        light.illuminance = lux_;
        let (el, az) = (el.to_radians(), az.to_radians());
        let toward = Vec3::new(el.cos() * az.sin(), el.sin(), el.cos() * az.cos());
        *tr = Transform::default().looking_to(-toward, Vec3::Y);
    }
    ambient.color = sky;
    ambient.brightness = nits;
    clear.0 = sky.darker(0.4);
    if let Some(mut m) = materials.get_mut(&glow.windows) {
        m.emissive = LinearRgba::rgb(4.0, 1.8, 0.6) * windows;
    }
    if let Some(mut m) = materials.get_mut(&glow.flames) {
        m.emissive = LinearRgba::rgb(30.0, 12.0, 3.0) * lit.max(0.15);
    }
    for (mut flame, mut vis) in &mut flames {
        let base = if flame.fire { 60_000.0 } else { 9_000.0 };
        flame.lumens = base * lit;
        *vis = if lit > 0.0 || flame.fire {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if flame.fire && lit == 0.0 {
            flame.lumens = base * 0.4;
        }
    }
}

/// Flames waver.
fn flicker(time: Res<Time>, mut flames: Query<(&Flame, &mut PointLight)>) {
    let t = time.elapsed_secs();
    for (flame, mut light) in &mut flames {
        let speed = if flame.fire { 1.0 } else { 0.5 };
        let w = (t * 13.0 * speed + flame.seed).sin() * 0.06
            + (t * 7.3 * speed + flame.seed * 2.1).sin() * 0.08
            + (t * 23.0 * speed + flame.seed * 0.7).sin() * 0.04;
        light.intensity = flame.lumens * (1.0 + w);
    }
}
