//! The map: a clearing in an oak wood, a rocky ridge behind, a knoll and a pond, a camp
//! with its fire; and the light, at day, dusk or night.

use crate::nav::Nav;
use crate::terrain::{self, Land, WATER};
use bevy::image::ImageLoaderSettings;
use bevy::light::{CascadeShadowConfigBuilder, light_consts::lux};
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.init_resource::<Nav>()
        .add_systems(Startup, build)
        .add_systems(Update, (light_the_hour, flicker));
}

/// Where the camp's fire burns.
pub const FIRE: Vec2 = Vec2::new(0.0, 3.0);
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
    flames: Handle<StandardMaterial>,
}

/// The materials, each `color.jpg` with its `normal.jpg` and `arm.jpg` (occlusion,
/// roughness, metal) from `assets/materials/<name>`.
pub fn material(asset: &AssetServer, name: &str, tint: Color) -> StandardMaterial {
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

/// A model from the game's assets, set on the land.
pub fn model(commands: &mut Commands, asset: &AssetServer, path: &str, at: Vec3, yaw: f32, scale: f32) -> Entity {
    commands
        .spawn((
            WorldAssetRoot(asset.load(GltfAssetLabel::Scene(0).from_asset(path.to_string()))),
            Transform::from_translation(at)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(scale)),
        ))
        .id()
}

/// What lies on the ground at a point: grass, leaves, moss, rock; mud, roots.
fn cover(p: Vec2, n: Vec3, h: f32) -> ([f32; 4], [f32; 2]) {
    let woods = terrain::woods(p);
    let steep = terrain::smooth(0.86, 0.7, n.y);
    let patch = terrain::fbm(p * 0.18, 31);
    let wet = terrain::smooth(WATER + 0.45, WATER + 0.05, h) * terrain::smooth(-0.6, -0.2, terrain::pond(p));
    let rock = steep;
    let moss = (terrain::smooth(0.95, 0.82, n.y) * (1.0 - steep) + woods * terrain::smooth(0.55, 0.7, patch) * 0.8)
        .min(1.0);
    let bare = terrain::bare(p) * (1.0 - rock);
    // Trodden earth: the litter's dark soil, a little mud in it.
    let leaves = (woods * (1.0 - rock)).max(bare * 0.7);
    let wet = wet.max(bare * 0.12);
    let grass = (1.0 - woods * 0.85) * (1.0 - rock) * (1.0 - wet) * (1.0 - bare * 0.85);
    let roots = woods * terrain::smooth(0.6, 0.75, terrain::fbm(p * 0.3, 37)) * (1.0 - rock);
    ([grass, leaves, moss, rock], [wet, roots])
}

pub fn build(
    mut commands: Commands,
    asset: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut grounds: ResMut<Assets<terrain::GroundMaterial>>,
    land: Res<Land>,
    mut nav: ResMut<Nav>,
) {
    commands.insert_resource(Hour::Dusk);
    let flames = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.7, 0.3),
        emissive: LinearRgba::rgb(30.0, 12.0, 3.0),
        unlit: true,
        ..default()
    });
    commands.insert_resource(Glow {
        flames: flames.clone(),
    });

    // The land (the pond's water is water.rs's).
    commands.spawn((
        Mesh3d(meshes.add(land.mesh(cover))),
        MeshMaterial3d(grounds.add(terrain::ground_material(&asset))),
    ));
    nav.block_where(|p| land.slope(p) > 0.6 || land.height(p) < WATER + 0.08);

    // The camp: its fire with logs to sit on.
    let on = |p: Vec2| Vec3::new(p.x, land.height(p), p.y);
    nav.block_circle(FIRE, 0.75);
    model(&mut commands, &asset, "models/props/Campfire.glb", on(FIRE), 0.0, 1.0);
    for (dx, dz, yaw) in [(0.2, 2.0, 0.1), (2.1, -0.6, 1.8), (-1.9, -0.8, -1.2)] {
        let p = FIRE + Vec2::new(dx, dz);
        nav.block_rect(p, Vec2::new(0.85, 0.25), yaw);
        model(&mut commands, &asset, "models/props/Log.glb", on(p), yaw, 1.0);
    }
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.55, 0.22),
            intensity: 60_000.0,
            range: 18.0,
            radius: 0.3,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_translation(on(FIRE) + Vec3::Y * 0.7),
        Flame {
            lumens: 60_000.0,
            seed: 0.0,
            fire: true,
        },
    ));

    // The light: the sun (or moon), a sky's fill.
    commands.spawn((
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
        bevy::camera::visibility::RenderLayers::from_layers(&[0, crate::forest::SUN_ONLY]),
    ));
}

/// The sun's place and colour, the sky's fill and the lamps, for the hour.
fn light_the_hour(
    hour: Res<Hour>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), With<Sun>>,
    mut exposure: Query<&mut bevy::camera::Exposure>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    glow: Res<Glow>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut flames: Query<(&mut Flame, &mut Visibility)>,
    mut fog: Query<&mut bevy::pbr::DistanceFog>,
) {
    if !hour.is_changed() {
        return;
    }
    // Elevation, azimuth from the south toward the east, colour, lux, sky colour and nits,
    // window glow, and how bright the flames burn.
    let (el, az, color, lux_, sky, nits, _windows, lit) = match *hour {
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
    // The exposure: dusk and night are opened up a little, as a game grades them, so the
    // clearing is dim, not black.
    let ev = match *hour {
        Hour::Day => 9.0,
        Hour::Dusk => 8.2,
        Hour::Night => 7.4,
    };
    for mut e in &mut exposure {
        e.ev100 = ev;
    }
    let haze = match *hour {
        Hour::Day => Color::srgba(0.62, 0.7, 0.78, 0.55),
        Hour::Dusk => Color::srgba(0.42, 0.4, 0.5, 0.5),
        Hour::Night => Color::srgba(0.05, 0.07, 0.13, 0.6),
    };
    for mut f in &mut fog {
        f.color = haze;
    }
    ambient.color = sky;
    ambient.brightness = nits;
    clear.0 = sky.darker(0.4);
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
