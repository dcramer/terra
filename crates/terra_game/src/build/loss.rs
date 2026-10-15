//! Loss and repair in the game (D193, D305; `terra_build::loss`, `terra_build::fragments`):
//! what comes down breaks into fragments of its own material that fly, tumble, bounce,
//! roll off ledges, settle and shrink away, with a puff of dust where it broke and where
//! big pieces land, and the view shaken when a big collapse is near; and, until attacks
//! come (D90), F9 harming the cube the hammer is aimed at by a quarter of its kind's health,
//! as the remote's `build.damage` does. Mending is the hammer's (`play::tool`), damage drawn
//! as its sections are.
//!
//! Cheap and bounded: fragments are drawn instanced, a few meshes a kind in the texture
//! arrays (D290) and so one draw a mesh; moved on a fixed step of our own; capped by the
//! preset, the oldest fading first past it, fewer made far off and at a low preset. Nothing
//! of them is interactive or collides with anyone.

use super::act::Building;
use super::arrays::{Arrays, BuildMaterial};
use super::draw;
use super::{Build, BuildLook, Built, Site};
use crate::planet::{MainCamera, Planet};
use crate::play::{Builder, LocalPlayer, Player, Shake};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
use terra_build::fragments::{self, Breaks, Fragment, Shape};
use terra_build::ops::Op;
use terra_build::{CELL_M, Cube, Kind};
use terra_world::quality::Quality;

/// The fixed step fragments move by, in seconds, and the most steps a frame.
const STEP_S: f32 = 1.0 / 60.0;
const MOST_STEPS: u32 = 3;
/// How long a fragment lies still before it shrinks away, at least and at most, how long
/// that takes, and how long one thrown off out of sight is kept at most.
const REST_S: [f32; 2] = [1.6, 3.4];
const FADE_S: f32 = 0.7;
/// How quickly the oldest shrink away when newer need the room.
const QUICK_FADE_S: f32 = 0.3;
const OLDEST_S: f32 = 14.0;
/// How long each kind's fragments are drawn out of sight when the game starts, so their
/// pipelines are compiled before anything comes down.
const WARM_S: f32 = 8.0;
/// A step of the fragments longer than this, in milliseconds, is said in the log.
const SLOW_MS: f32 = 2.0;
/// Past how many metres from the view a fall makes fewer fragments, and how many fewer.
const FAR_M: f32 = 60.0;
const FAR_SHARE: f32 = 0.4;
/// How far from a big collapse the view shakes, how many cubes make it shake its most, and
/// how far it shakes then, in metres.
const SHAKE_M: f32 = 40.0;
const SHAKE_CUBES: f32 = 300.0;
const SHAKE_REACH_M: f32 = 0.06;
/// How long a puff of dust lasts, in seconds, and how wide it grows, in metres.
const PUFF_S: f32 = 1.6;
const PUFF_M: [f32; 2] = [0.7, 2.6];

pub fn plugin(app: &mut App) {
    app.init_resource::<Shards>().add_systems(
        Update,
        (harm_aimed, burst, (step, puff))
            .chain()
            .after(super::layout::Drawing::Upload),
    );
}

/// What came down in an act, for its fragments: every cube as it was, and those harm
/// destroyed, which burst where they were; the rest hung from them and fall whole.
#[derive(Clone, Debug, Default)]
pub struct Fallen {
    pub fell: Vec<(IVec3, Cube)>,
    pub destroyed: BTreeSet<[i32; 3]>,
    /// Cubes harmed and still standing, as they are, with how many quarters of their health
    /// they had lost before and have now (D305): the stones, timber and shingles that came
    /// off them as they broke, falling from their open faces.
    pub chipped: Vec<(IVec3, Cube, u8, u8, Vec3)>,
}

/// Marks a fragment's entity.
#[derive(Component)]
struct Shard;

/// A puff of dust: how long it has been up, how wide it grows, and its own material, whose
/// alpha fades.
#[derive(Component)]
struct Puff {
    age: f32,
    wide: f32,
    rise: Vec3,
    material: Handle<StandardMaterial>,
    alpha: f32,
    /// Whether it is the puff's first sprite, which fades the material they share.
    owner: bool,
}

/// A fragment moving or lying: the build whose space it is in, its entity, how long it has
/// lain still and how long it may before it shrinks away, how long since it was made.
struct Live {
    fragment: Fragment,
    root: Entity,
    /// Its entity, once it is drawn: `PLACEHOLDER` until then.
    entity: Entity,
    still_s: f32,
    rest_s: f32,
    /// How long it takes to shrink away: quicker when the oldest make room.
    fade_s: f32,
    age_s: f32,
}

impl Live {
    fn new(fragment: Fragment, root: Entity) -> Live {
        Live {
            fragment,
            root,
            entity: Entity::PLACEHOLDER,
            still_s: 0.0,
            rest_s: rest_for(&fragment),
            fade_s: FADE_S,
            age_s: 0.0,
        }
    }

    /// Whether it is shrinking away, or gone.
    fn fading(&self) -> bool {
        self.still_s >= self.rest_s
    }

    fn gone(&self) -> bool {
        self.still_s >= self.rest_s + self.fade_s
    }
}

/// The material a fragment's mesh is drawn in: the texture arrays, or its kind's own where
/// the arrays have no layer for it.
#[derive(Clone)]
enum Drawn {
    Arrays(Handle<BuildMaterial>),
    Own(Handle<StandardMaterial>),
}

/// The fragments, their meshes by kind and shape, the layers they wear, and the dust.
#[derive(Resource, Default)]
struct Shards {
    live: Vec<Live>,
    meshes: BTreeMap<(Kind, Shape, u8), Option<(Handle<Mesh>, Drawn)>>,
    layers: BTreeSet<u16>,
    puff: Option<Handle<Mesh>>,
    dust: Option<Handle<Image>>,
    puffs: usize,
    behind: f32,
    /// Whether every kind's meshes have been made, ahead of anything coming down, and drawn
    /// out of sight for a while so their pipelines are ready.
    warmed: bool,
    warming: Vec<Entity>,
    warm_s: f32,
}

/// How much of a cube's burst comes off it as it is harmed and still stands, a quarter of
/// its health lost (D305).
const CHIPPED_SHARE: f32 = 0.35;

/// How many fragments and puffs may be at once, and how much of a full burst to make, at a
/// preset.
fn budget(quality: Quality) -> (usize, usize, f32) {
    match quality {
        Quality::Low => (700, 8, 0.45),
        Quality::Medium => (1400, 16, 0.75),
        Quality::High => (fragments::MOST, 24, 1.0),
    }
}

/// How many fragments are drawn anew a frame at a preset: a big fall's are spread over a
/// few frames, while the sections it changed are laid again anyway.
fn spawn_budget(quality: Quality) -> usize {
    match quality {
        Quality::Low => 150,
        Quality::Medium => 300,
        Quality::High => 500,
    }
}

/// How a kind breaks, from its kit: chunks unless it says.
fn breaks_of(look: &BuildLook) -> impl Fn(Kind) -> Breaks + '_ {
    move |kind| {
        look.kits
            .as_deref()
            .and_then(|kits| kits.of(kind))
            .map_or(Breaks::Chunks, |kit| kit.toughness.breaks)
    }
}

/// The colour a kind's dust is: its material's light colour, greyed a little.
fn dust_of(look: &BuildLook, kind: Kind) -> Color {
    let light = look
        .kits
        .as_deref()
        .and_then(|kits| kits.of(kind))
        .map_or([0.6, 0.55, 0.5], |kit| kit.colours.light);
    let grey = (light[0] + light[1] + light[2]) / 3.0;
    let mix = |c: f32| c * 0.6 + grey * 0.4;
    Color::srgb(mix(light[0]), mix(light[1]), mix(light[2]))
}

/// The mesh and material a kind's fragments of a shape and size are drawn with
/// (`Fragment::mesh`), made the first time.
fn mesh_for(
    shards: &mut Shards,
    look: &BuildLook,
    arrays: Option<&Arrays>,
    meshes: &mut Assets<Mesh>,
    kind: Kind,
    (shape, across, drawn_m): (Shape, u8, f32),
) -> Option<(Handle<Mesh>, Drawn)> {
    if let Some(made) = shards.meshes.get(&(kind, shape, across)) {
        return made.clone();
    }
    let made = (|| {
        let kit = look.kits.as_deref()?.of(kind)?;
        let surfaces: BTreeMap<_, _> = fragments::surfaces(kit, shape, drawn_m)
            .0
            .into_iter()
            .map(|(material, surface)| ((kind, material), surface))
            .collect();
        let mut used = BTreeSet::new();
        let (in_arrays, rest) = draw::in_arrays(
            &surfaces,
            &look.layers,
            Vec3::ZERO,
            &BTreeMap::new(),
            &mut used,
        );
        if let (Some(mesh), Some(arrays)) = (in_arrays, arrays) {
            shards.layers.extend(used);
            return Some((meshes.add(mesh), Drawn::Arrays(arrays.material.clone())));
        }
        let (kind, material, mesh) = rest.into_iter().next()?;
        let own = look
            .materials
            .get(kind, material)
            .or_else(|| look.materials.get(Kind::PLAIN, material))?;
        Some((meshes.add(mesh), Drawn::Own(own.clone())))
    })();
    shards.meshes.insert((kind, shape, across), made.clone());
    made
}

/// Where a fragment is in the world: its build's frame over where it is in the build's
/// space, shrunk as it fades.
fn placed(frame: &Transform, live: &Live) -> Transform {
    let fade = ((live.still_s - live.rest_s) / live.fade_s).clamp(0.0, 1.0);
    let shrink = 1.0 - fade * fade * (3.0 - 2.0 * fade);
    let sink = Vec3::Y * live.fragment.size.y * 0.35 * fade;
    frame.mul_transform(
        Transform::from_translation(live.fragment.at - sink)
            .with_rotation(live.fragment.turn)
            .with_scale(live.fragment.size * shrink.max(0.001)),
    )
}

/// Spawns a fragment's entity where it is.
fn spawn(commands: &mut Commands, (mesh, drawn): (Handle<Mesh>, Drawn), at: Transform) -> Entity {
    let mut entity = commands.spawn((Name::new("Fragment"), Shard, Mesh3d(mesh), at));
    match drawn {
        Drawn::Arrays(material) => entity.insert(MeshMaterial3d(material)),
        Drawn::Own(material) => entity.insert(MeshMaterial3d(material)),
    };
    entity.id()
}

/// A rest a fragment lies still for before it shrinks away, its own, the same each time.
fn rest_for(fragment: &Fragment) -> f32 {
    let unit = (fragment.seed % 1000) as f32 / 1000.0;
    REST_S[0] + (REST_S[1] - REST_S[0]) * unit
}

/// Bursts what came down in each build since last looked into fragments, with dust where it
/// broke and the view shaken if a big collapse is near.
#[allow(clippy::too_many_arguments)]
fn burst(
    mut commands: Commands,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
    look: Res<BuildLook>,
    quality: Res<Quality>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    mut shards: ResMut<Shards>,
    mut meshes: ResMut<Assets<Mesh>>,
    (mut materials, mut images): (ResMut<Assets<StandardMaterial>>, ResMut<Assets<Image>>),
    mut shake: ResMut<Shake>,
) {
    let (most, most_puffs, preset) = budget(*quality);
    let eye = camera.map(|camera| camera.translation());
    let mut events: Vec<(Entity, Transform, Fallen)> = Vec::new();
    if !site.fallen.is_empty() {
        let site = site.bypass_change_detection();
        let (root, frame) = (site.root, site.frame);
        for fallen in site.fallen.drain(..) {
            events.push((root, frame, fallen));
        }
    }
    for mut built in &mut builds {
        if built.fallen.is_empty() {
            continue;
        }
        let built = built.bypass_change_detection();
        let (root, frame) = (built.root, built.frame);
        for fallen in built.fallen.drain(..) {
            events.push((root, frame, fallen));
        }
    }
    let breaks = breaks_of(&look);
    for (root, frame, fallen) in events {
        let began = Instant::now();
        let middle = fallen
            .fell
            .iter()
            .map(|(at, _)| *at)
            .chain(fallen.chipped.iter().map(|chip| chip.0))
            .map(|at| (at.as_vec3() + 0.5) * CELL_M)
            .sum::<Vec3>()
            / (fallen.fell.len() + fallen.chipped.len()).max(1) as f32;
        let world = frame.transform_point(middle);
        let distance = eye.map_or(0.0, |eye| eye.distance(world));
        let share = preset * if distance > FAR_M { FAR_SHARE } else { 1.0 };
        let destroyed = |at: IVec3| fallen.destroyed.contains(&at.to_array());
        let mut made = fragments::burst(&fallen.fell, &destroyed, &breaks, share, most);
        // What came off a cube still standing: a few pieces a quarter of its health lost,
        // out of its open face, where its stones and shingles were.
        for (at, cube, from, to, out) in &fallen.chipped {
            let lost = f32::from(to.saturating_sub(*from)) / 4.0;
            let pieces = fragments::burst(
                &[(*at, *cube)],
                &|_| true,
                &breaks,
                share * CHIPPED_SHARE * lost,
                most.saturating_sub(made.len()),
            );
            made.extend(pieces.into_iter().map(|mut piece| {
                // Off its face, not inside it.
                let centre = (at.as_vec3() + 0.5) * CELL_M;
                let across = piece.at - centre;
                piece.at = centre + across - *out * across.dot(*out)
                    + *out * (CELL_M * 0.5 + piece.size.max_element() * 0.5);
                piece.velocity += *out * 0.8;
                piece
            }));
        }
        let count = made.len();
        // Moving at once, drawn a few hundred a frame (`step`), so a big fall's entities are
        // not all made in one frame.
        shards
            .live
            .extend(made.into_iter().map(|fragment| Live::new(fragment, root)));
        // Dust where it broke: a puff for every few cubes destroyed, more for what crumbles.
        let broke: Vec<&(IVec3, Cube)> = fallen
            .fell
            .iter()
            .filter(|(at, _)| destroyed(*at))
            .collect();
        let puffs = broke.len().div_ceil(3).min(most_puffs / 2).max(1);
        let every = broke.len().div_ceil(puffs).max(1);
        for (at, cube) in broke.iter().step_by(every) {
            let dust = breaks(cube.kind).look().dust;
            let colour = dust_of(&look, cube.kind);
            let at = frame.transform_point((at.as_vec3() + 0.5) * CELL_M);
            dust_at(
                &mut commands,
                &mut shards,
                (&mut meshes, &mut materials, &mut images),
                most_puffs,
                at,
                colour,
                dust,
                at.normalize_or(Vec3::Y),
            );
        }
        // A big collapse near shakes the view.
        if let Some(eye) = eye
            && fallen.fell.len() >= 12
        {
            let near = 1.0 - (eye.distance(world) / SHAKE_M).clamp(0.0, 1.0);
            let big = (fallen.fell.len() as f32 / SHAKE_CUBES).clamp(0.15, 1.0);
            if near > 0.0 {
                shake.kick(SHAKE_REACH_M * near * big, 0.35 + 0.5 * big);
            }
        }
        info!(
            "Build: fragments: {} cubes came down, {} destroyed, {count} fragments made in {:.1} ms, {} in all",
            fallen.fell.len(),
            fallen.destroyed.len(),
            began.elapsed().as_secs_f32() * 1000.0,
            shards.live.len()
        );
    }
}

/// Sets the oldest fragments still not fading shrinking away, so many of them.
fn fade_oldest(shards: &mut Shards, count: usize) {
    if count == 0 {
        return;
    }
    let mut order: Vec<usize> = (0..shards.live.len())
        .filter(|&i| !shards.live[i].fading())
        .collect();
    order.sort_by(|&a, &b| shards.live[b].age_s.total_cmp(&shards.live[a].age_s));
    for &i in order.iter().take(count) {
        let live = &mut shards.live[i];
        live.fragment.still = true;
        live.still_s = live.rest_s;
        live.fade_s = QUICK_FADE_S;
    }
}

/// A soft round of dust, its alpha falling off from its middle, a little uneven: what each
/// sprite of a puff is drawn with.
fn dust_image() -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    const SIDE: u32 = 64;
    let mut data = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (u, v) = (
                (x as f32 + 0.5) / SIDE as f32 * 2.0 - 1.0,
                (y as f32 + 0.5) / SIDE as f32 * 2.0 - 1.0,
            );
            let r = (u * u + v * v).sqrt();
            let lump = 0.85 + 0.15 * libm::sinf(u * 9.0 + libm::cosf(v * 7.0) * 2.0);
            let alpha = ((1.0 - r).max(0.0)).powi(2) * lump;
            data.extend([255, 255, 255, (alpha.clamp(0.0, 1.0) * 255.0) as u8]);
        }
    }
    Image::new(
        Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// A puff of dust at a place in the world, of a colour, as big as `amount` says: a few soft
/// sprites facing the view, which billow out, rise a little and thin away.
#[allow(clippy::too_many_arguments)]
fn dust_at(
    commands: &mut Commands,
    shards: &mut Shards,
    (meshes, materials, images): (
        &mut Assets<Mesh>,
        &mut Assets<StandardMaterial>,
        &mut Assets<Image>,
    ),
    most: usize,
    at: Vec3,
    colour: Color,
    amount: f32,
    up: Vec3,
) {
    if shards.puffs >= most || amount <= 0.0 {
        return;
    }
    shards.puffs += 1;
    let quad = shards
        .puff
        .get_or_insert_with(|| meshes.add(Rectangle::new(1.0, 1.0)))
        .clone();
    let image = shards
        .dust
        .get_or_insert_with(|| images.add(dust_image()))
        .clone();
    let alpha = (0.6 * amount).clamp(0.3, 0.8);
    let linear = colour.to_linear();
    let material = materials.add(StandardMaterial {
        base_color: Color::LinearRgba(linear).with_alpha(alpha),
        base_color_texture: Some(image),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let wide = PUFF_M[0] + (PUFF_M[1] - PUFF_M[0]) * (amount / 1.4).min(1.0);
    let seed = shards.puffs as u32 ^ (at.x.to_bits() >> 8) ^ (at.z.to_bits() >> 4);
    for sprite in 0..5u32 {
        let r = |salt: u32| {
            let h = (seed ^ sprite.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA77))
                .wrapping_mul(0x2C1B_3C6D);
            ((h >> 8) & 0xFFFF) as f32 / 65535.0
        };
        let drift = Vec3::new(r(1) - 0.5, 0.0, r(2) - 0.5) * 1.2 + up * (0.25 + 0.35 * r(3));
        commands.spawn((
            Name::new("Dust"),
            Puff {
                age: 0.0,
                wide: wide * (0.6 + 0.6 * r(4)),
                rise: drift,
                material: material.clone(),
                alpha,
                owner: sprite == 0,
            },
            Mesh3d(quad.clone()),
            MeshMaterial3d(material.clone()),
            NotShadowCaster,
            Transform::from_translation(at + drift * 0.2).with_scale(Vec3::splat(PUFF_M[0] * 0.5)),
        ));
    }
}

/// Moves every fragment on by the fixed step as many times as the frame's time asks: what
/// lies under each is the terrain and the base's cubes; a whole cube landing hard breaks,
/// with dust; one lying still long enough shrinks away and is gone.
#[allow(clippy::too_many_arguments)]
fn step(
    mut commands: Commands,
    time: Res<Time>,
    site: Res<Site>,
    builds: Query<&Built>,
    look: Res<BuildLook>,
    planet: Res<Planet>,
    quality: Res<Quality>,
    mut arrays: Option<ResMut<Arrays>>,
    assets: Res<AssetServer>,
    real: Res<Time<Real>>,
    mut shards: ResMut<Shards>,
    mut transforms: Query<&mut Transform, With<Shard>>,
    mut meshes: ResMut<Assets<Mesh>>,
    (mut materials, mut images): (ResMut<Assets<StandardMaterial>>, ResMut<Assets<Image>>),
) {
    // Every kind's fragments made ready before anything comes down, and the layers they
    // wear kept resident, so the first fall is drawn in its material from its first frame.
    if !shards.warmed
        && let Some(kits) = look.kits.clone()
    {
        shards.warmed = true;
        // Each drawn once, out of sight under the ground and never culled, so its pipelines
        // are compiled before the first fall rather than leaving a hole for its first frames.
        let under = site.frame.transform_point(Vec3::new(0.0, -80.0, 0.0));
        let sizes = [(Shape::Chunk, 0, 0.25), (Shape::Lump, 0, 0.25)]
            .into_iter()
            .chain((1..=4u8).map(|across| (Shape::Whole, across, f32::from(across) * CELL_M)));
        let sizes: Vec<_> = sizes.collect();
        for kit in &kits.0 {
            for &size in &sizes {
                let made = mesh_for(
                    &mut shards,
                    &look,
                    arrays.as_deref(),
                    &mut meshes,
                    kit.kind(),
                    size,
                );
                if let Some(drawn) = made {
                    let at = Transform::from_translation(under).with_scale(Vec3::splat(0.01));
                    let entity = spawn(&mut commands, drawn, at);
                    commands.entity(entity).insert(NoFrustumCulling);
                    shards.warming.push(entity);
                }
            }
        }
    }
    if !shards.warming.is_empty() {
        shards.warm_s += time.delta_secs();
        if shards.warm_s > WARM_S {
            for entity in shards.warming.drain(..) {
                commands.entity(entity).despawn();
            }
        }
    }
    if let Some(arrays) = arrays.as_mut() {
        let layers: Vec<u16> = shards.layers.iter().copied().collect();
        arrays.want(&layers, &assets, real.elapsed_secs());
    }
    if shards.live.is_empty() {
        return;
    }
    let began = Instant::now();
    let (most, most_puffs, preset) = budget(*quality);
    let mut by_root: BTreeMap<Entity, &Build> = BTreeMap::new();
    by_root.insert(site.root, &site.build);
    for built in &builds {
        by_root.insert(built.root, &built.0);
    }
    let breaks = breaks_of(&look);
    shards.behind = (shards.behind + time.delta_secs()).min(STEP_S * MOST_STEPS as f32);
    let mut steps = 0;
    let mut born: Vec<Live> = Vec::new();
    let mut landed: Vec<(Vec3, Kind, f32)> = Vec::new();
    while shards.behind >= STEP_S && steps < MOST_STEPS {
        shards.behind -= STEP_S;
        steps += 1;
        // The terrain under each column fragments are over, once a step.
        let mut terrain: BTreeMap<(Entity, [i32; 2]), f32> = BTreeMap::new();
        let mut room = most.saturating_sub(shards.live.len());
        for live in &mut shards.live {
            live.age_s += STEP_S;
            let Some(build) = by_root.get(&live.root) else {
                live.still_s = live.rest_s + live.fade_s;
                continue;
            };
            if live.fragment.still {
                live.still_s += STEP_S;
                continue;
            }
            if live.age_s > OLDEST_S {
                live.fragment.still = true;
                live.still_s = live.rest_s;
                continue;
            }
            let frame = build.frame;
            let base = &build.base;
            let cells = std::cell::RefCell::new(&mut terrain);
            let root = live.root;
            let floor = |at: Vec3| -> f32 {
                let column = [
                    (at.x / CELL_M).floor() as i32,
                    (at.z / CELL_M).floor() as i32,
                ];
                let ground = *cells.borrow_mut().entry((root, column)).or_insert_with(|| {
                    let middle = Vec3::new(
                        (column[0] as f32 + 0.5) * CELL_M,
                        0.0,
                        (column[1] as f32 + 0.5) * CELL_M,
                    );
                    -planet.distance(frame.transform_point(middle))
                });
                base.top_under(at).map_or(ground, |top| top.max(ground))
            };
            let solid = |at: Vec3| base.solid_at(at);
            let landing = live.fragment.step(STEP_S, &floor, &solid);
            if landing.breaks {
                let kind = live.fragment.kind;
                let breaking = breaks(kind);
                let pieces = live
                    .fragment
                    .shatter(breaking, preset.min(room as f32 / breaking.count() as f32));
                room = room.saturating_sub(pieces.len());
                landed.push((
                    frame.transform_point(live.fragment.at),
                    kind,
                    breaking.look().dust * 0.6,
                ));
                if !pieces.is_empty() {
                    // What it broke into goes on; the whole is gone at once, hidden
                    // inside its pieces.
                    live.still_s = live.rest_s + live.fade_s;
                    live.fragment.still = true;
                    let root = live.root;
                    born.extend(pieces.into_iter().map(|fragment| Live::new(fragment, root)));
                }
            } else if landing.hit.is_some_and(|hit| hit > 3.0)
                && live.fragment.size.max_element() > 0.2
            {
                landed.push((
                    frame.transform_point(live.fragment.at),
                    live.fragment.kind,
                    0.35,
                ));
            }
        }
    }
    // What broke on landing: moving from now, drawn at the end with the rest.
    shards.live.extend(born);
    // Never far past the most: the oldest make room, and past half again as many the
    // oldest of those not yet drawn are let go at once.
    let over = shards
        .live
        .iter()
        .filter(|live| !live.fading())
        .count()
        .saturating_sub(most);
    fade_oldest(&mut shards, over);
    while shards.live.len() > most + most / 2 {
        match shards
            .live
            .iter()
            .position(|live| live.entity == Entity::PLACEHOLDER)
        {
            Some(at) => {
                shards.live.remove(at);
            }
            None => break,
        }
    }
    for (at, kind, amount) in landed {
        let colour = dust_of(&look, kind);
        dust_at(
            &mut commands,
            &mut shards,
            (&mut meshes, &mut materials, &mut images),
            most_puffs,
            at,
            colour,
            amount,
            at.normalize_or(Vec3::Y),
        );
    }
    // Each where it is now, those not yet drawn drawn a few hundred a frame; those shrunk
    // away gone.
    let mut spawns = spawn_budget(*quality);
    let mut live_now = std::mem::take(&mut shards.live);
    let mut kept = Vec::with_capacity(live_now.len());
    for mut live in live_now.drain(..) {
        if live.gone() {
            if live.entity != Entity::PLACEHOLDER
                && let Ok(mut entity) = commands.get_entity(live.entity)
            {
                entity.despawn();
            }
            continue;
        }
        if live.entity == Entity::PLACEHOLDER {
            if spawns > 0
                && let Some(build) = by_root.get(&live.root)
                && let Some(drawn) = mesh_for(
                    &mut shards,
                    &look,
                    arrays.as_deref(),
                    &mut meshes,
                    live.fragment.kind,
                    live.fragment.mesh(),
                )
            {
                spawns -= 1;
                live.entity = spawn(&mut commands, drawn, placed(&build.frame, &live));
            }
            kept.push(live);
            continue;
        }
        if let (Some(build), Ok(mut transform)) =
            (by_root.get(&live.root), transforms.get_mut(live.entity))
        {
            let now = placed(&build.frame, &live);
            // Touched for its first moments even while it waits to fall, so what it is
            // drawn with from the frame before, for its motion, is never left unset.
            if *transform != now || live.age_s < 1.0 {
                *transform = now;
            }
        }
        kept.push(live);
    }
    shards.live = kept;
    // A slow step said, so a trace can tell fragments from the rest of a frame.
    let took = began.elapsed().as_secs_f32() * 1000.0;
    if took > SLOW_MS {
        info!(
            "Build: fragments stepped: {} in {took:.1} ms ({steps} steps)",
            shards.live.len()
        );
    }
}

/// Dust billows out, rises and thins away, each sprite facing the view.
fn puff(
    mut commands: Commands,
    time: Res<Time>,
    mut shards: ResMut<Shards>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    mut puffs: Query<(Entity, &mut Puff, &mut Transform)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let eye = camera.map(|camera| camera.translation());
    for (entity, mut puff, mut transform) in &mut puffs {
        puff.age += time.delta_secs();
        let through = (puff.age / PUFF_S).min(1.0);
        if through >= 1.0 {
            commands.entity(entity).despawn();
            if puff.owner {
                materials.remove(&puff.material);
                shards.puffs = shards.puffs.saturating_sub(1);
            }
            continue;
        }
        let grow = 1.0 - (1.0 - through) * (1.0 - through);
        transform.scale = Vec3::splat(PUFF_M[0] * 0.5 + (puff.wide - PUFF_M[0] * 0.5) * grow);
        transform.translation += puff.rise * time.delta_secs() * (1.0 - through);
        if let Some(eye) = eye {
            let at = transform.translation;
            let up = at.normalize_or(Vec3::Y);
            transform.look_at(at * 2.0 - eye, up);
        }
        if puff.owner
            && let Some(mut material) = materials.get_mut(&puff.material)
        {
            // Thick at once, then thinning away.
            let rise = (through / 0.12).min(1.0);
            let alpha = puff.alpha * rise * (1.0 - through) * (1.0 - through);
            material.base_color = material.base_color.with_alpha(alpha);
        }
    }
}

/// F9 harms the cube the hammer is aimed at by a quarter of its kind's health, as an attack
/// will (D90): a debug key, until attacks come.
fn harm_aimed(
    keys: Res<ButtonInput<KeyCode>>,
    builders: Query<&Builder, (With<Player>, With<LocalPlayer>)>,
    mut building: Building,
) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }
    let Some(at) = builders.iter().find_map(Builder::aimed) else {
        return;
    };
    let Some(cube) = building.site.base.cube(at) else {
        return;
    };
    let health = building.rules.rules.toughness(cube.kind).health;
    let op = Op::Damage {
        cubes: vec![at.to_array()],
        amount: health.div_ceil(4),
    };
    let _ = building.act(None, "damage", &[op]);
}

/// How many fragments are about, for the remote's state.
pub(super) fn live(world: &World) -> usize {
    world
        .get_resource::<Shards>()
        .map_or(0, |shards| shards.live.len())
}
