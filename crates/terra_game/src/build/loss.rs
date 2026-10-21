//! Loss and repair in the game (D193, D305; `terra_build::loss`, `terra_build_mesh::fragments`):
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
use super::{Build, BuildLook, Built, Fallen, Site};
use crate::planet::{MainCamera, Planet};
use crate::play::{Builder, LocalPlayer, Player, Shake};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
use terra_build::ops::Op;
use terra_build::{CELL_M, Cube, Kind};
use terra_build_mesh::fragments::{self, Breaking, Breaks, Fragment, Rubble, Shape};
use terra_world::quality::Quality;

mod dust;
mod heaps;
use dust::{billow_at, dust_at, landing, puff};
use heaps::draw_heaps;

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
/// From how big at its longest, in metres, a piece landing raises a billow of its own
/// (`dust::landing`) rather than a puff.
const BILLOW_FROM_M: f32 = 0.9;
/// How many billows the chunks of big falls raise where they land may be up at once, over
/// the small puffs' budget: a castle's towers and walls come down within seconds of each
/// other, and the small puffs of their many lesser falls had used up a shared budget, so
/// the big falls raised none.
const BILLOWS: usize = 180;
/// The least share of its pieces a chunk breaks into where it lands, however full the
/// budget.
const LEAST_SHARE: f32 = 0.3;

/// What a fragment raised where it landed: where, of what kind, how much dust (0 to 1 and
/// more), how big it is at its longest, in metres, and how it was going along the ground.
struct Landed {
    at: Vec3,
    kind: Kind,
    amount: f32,
    size: f32,
    along: Vec3,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Shards>().add_systems(
        Update,
        (harm_aimed, burst, (step, puff))
            .chain()
            .after(super::layout::Drawing::Upload),
    );
}

/// Marks a fragment's entity.
#[derive(Component)]
struct Shard;

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
    /// The dust's normals: each sprite shaded as a round billow is, lit on its sunward side.
    dust_normals: Option<Handle<Image>>,
    puffs: usize,
    /// The billows raised lately, which chunks landing near them join.
    raised: dust::Raised,
    /// The billows of big falls' surges still to come up: when, on the fragments' clock,
    /// where, of what colour, which way is up there, and the billow (`dust::surge`).
    surges: Vec<(f32, Vec3, Color, Vec3, dust::Billow)>,
    /// Each build's heap of what fell, which what comes down after lands on, and its clock.
    rubble: BTreeMap<Entity, Rubble>,
    /// Each build's heap as drawn: its entities, when, and how much was in it then.
    heaps: BTreeMap<Entity, (Vec<Entity>, f32, f32)>,
    clock_s: f32,
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
        Quality::High | Quality::Ultra => (fragments::MOST, 24, 1.0),
    }
}

/// How many fragments are drawn anew a frame at a preset: a big fall's are spread over a
/// few frames, while the sections it changed are laid again anyway.
fn spawn_budget(quality: Quality) -> usize {
    match quality {
        Quality::Low => 150,
        Quality::Medium => 300,
        Quality::High | Quality::Ultra => 500,
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

/// A rest a fragment lies still for before it shrinks away, its own, the same each time: a
/// big piece, a block or a slab, long enough for the heap it makes to be seen, as rubble
/// lies at a fallen wall's foot; a small one briefly. Every one fades in the end, and the
/// least worth first wherever the cap needs the room (`fade_least`).
fn rest_for(fragment: &Fragment) -> f32 {
    let unit = (fragment.seed % 1000) as f32 / 1000.0;
    let rest = if fragment.size.max_element() >= BIG_M {
        BIG_REST_S
    } else {
        REST_S
    };
    rest[0] + (rest[1] - rest[0]) * unit
}

/// How big a piece is, at its longest, to lie as long as a big one, and how long that is.
const BIG_M: f32 = 0.2;
const BIG_REST_S: [f32; 2] = [8.0, 10.0];

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
    planet: Res<Planet>,
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
        // What lay on what came down, its heap and the pieces at rest on it, comes down
        // with it, rather than hanging where it was (`fragments::gone`).
        let gone = fragments::gone(&fallen.fell);
        let mut woken = 0;
        if !gone.is_empty() {
            if let Some(heap) = shards.rubble.get_mut(&root) {
                heap.fell(&gone);
            }
            for live in shards.live.iter_mut().filter(|live| live.root == root) {
                if !live.fading() && live.fragment.wake(&gone) {
                    live.still_s = 0.0;
                    woken += 1;
                }
            }
        }
        let falls = fragments::falls(&fallen.fell, &destroyed);
        let mut made = fragments::burst(&fallen.fell, &destroyed, &breaks, share, most);
        // Down where it stood, or tipping the way its support gave way and cracking up as it
        // goes, by its support's rules (`fragments::collapse`).
        fragments::collapse(&mut made, &fallen.fell, &destroyed, &breaks);
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
        // A big fall's surge of dust, as it lands: from the ground at its foot, as wide as
        // it, however high what came down stood.
        if let Some((_, cube)) = fallen.fell.get(fallen.fell.len() / 2) {
            let colour = dust_of(&look, cube.kind);
            let ground = -planet.distance(frame.transform_point(middle.with_y(0.0)));
            dust::plan_surge(&mut shards, colour, &frame, middle, ground, &fallen.fell);
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
            "Build: fragments: {} cubes came down round ({:.0}, {:.0}, {:.0}) in its base at {:.2} s, {} destroyed, {falls:?}, {woken} at rest on it set falling, {count} fragments made in {:.1} ms, {} in all",
            fallen.fell.len(),
            middle.x,
            middle.y,
            middle.z,
            shards.clock_s,
            fallen.destroyed.len(),
            began.elapsed().as_secs_f32() * 1000.0,
            shards.live.len()
        );
    }
}

/// Sets so many fragments still not fading shrinking away, those that matter least first
/// ([`worth`]): as shipped destruction culls its debris (Frostbite's debris budget lets the
/// small pieces go before the big ones of a collapse), not the oldest whatever they are,
/// which let a burning castle's many small falls fade its keep's chunks as it fell.
fn fade_least(shards: &mut Shards, count: usize) {
    if count == 0 {
        return;
    }
    let mut order: Vec<usize> = (0..shards.live.len())
        .filter(|&i| !shards.live[i].fading())
        .collect();
    let by = |a: &usize, b: &usize| worth(&shards.live[*a]).total_cmp(&worth(&shards.live[*b]));
    if count < order.len() {
        order.select_nth_unstable_by(count, by);
    }
    for &i in order.iter().take(count) {
        // It goes on falling as it shrinks: stopped where it was, a falling piece hung in
        // the air as it went.
        let live = &mut shards.live[i];
        live.still_s = live.rest_s;
        live.fade_s = QUICK_FADE_S;
    }
}

/// How much a fragment matters to what is seen: the bigger the more, still falling or
/// tumbling more than lying still, and less the longer it has been about.
fn worth(live: &Live) -> f32 {
    let moving = if live.fragment.still { 1.0 } else { 3.0 };
    live.fragment.size.max_element() * moving / (1.0 + live.age_s / 4.0)
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
    warm(
        &mut commands,
        time.delta_secs(),
        &site,
        &look,
        arrays.as_deref(),
        &mut shards,
        &mut meshes,
    );
    if let Some(arrays) = arrays.as_mut() {
        let layers: Vec<u16> = shards.layers.iter().copied().collect();
        arrays.want(&layers, &assets, real.elapsed_secs());
    }
    let (most, most_puffs, preset) = budget(*quality);
    // Surges whose time has come up, or all of them once nothing falls to keep the clock.
    if !shards.surges.is_empty() {
        let now = shards.clock_s;
        let idle = shards.live.is_empty();
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut shards.surges)
            .into_iter()
            .partition(|(when, ..)| idle || *when <= now);
        shards.surges = later;
        for (_, at, colour, up, billow) in due {
            let assets = (&mut *meshes, &mut *materials, &mut *images);
            let most = most_puffs + BILLOWS + dust::SURGES;
            billow_at(
                &mut commands,
                &mut shards,
                assets,
                most,
                at,
                colour,
                up,
                billow,
            );
        }
    }
    if shards.live.is_empty() {
        return;
    }
    let began = Instant::now();
    let mut by_root: BTreeMap<Entity, &Build> = BTreeMap::new();
    by_root.insert(site.root, &site.build);
    for built in &builds {
        by_root.insert(built.root, &built.0);
    }
    let (steps, landed) = fall(
        &mut shards,
        &by_root,
        &planet,
        &look,
        time.delta_secs(),
        (most, preset),
    );
    keep_within(&mut shards, most);
    for Landed {
        at,
        kind,
        amount,
        size,
        along,
    } in landed
    {
        let colour = dust_of(&look, kind);
        let up = at.normalize_or(Vec3::Y);
        let assets = (&mut *meshes, &mut *materials, &mut *images);
        // A big chunk landing raises a billow as big as it, which rolls on the way it was
        // going and up, and lasts: a big fall's cloud comes up where and when its chunks
        // land, in lumps, as a collapse's does, not all at once over what still stands. As
        // big as its way of breaking's dust says: a burning hoarding's timber chunks, coming
        // down all the while, had raised so many they left none for the stone's.
        let dusty = breaks_of(&look)(kind).look().dust;
        if let Some(billow) = landing(size * dusty, along, up) {
            // Chunks of one fall landing together raise one cloud between them, not one
            // each, so the budget lasts to the next fall.
            if shards.raised.joins(at, time.elapsed_secs()) {
                continue;
            }
            let wide = billow.wide;
            let most = most_puffs + BILLOWS;
            let raised = billow_at(
                &mut commands,
                &mut shards,
                assets,
                most,
                at,
                colour,
                up,
                billow,
            );
            if raised {
                shards.raised.add(at, wide, time.elapsed_secs());
            }
        } else {
            dust_at(
                &mut commands,
                &mut shards,
                assets,
                most_puffs,
                at,
                colour,
                amount,
                up,
            );
        }
    }
    draw_where_they_are(
        &mut commands,
        &mut shards,
        &by_root,
        &mut transforms,
        &look,
        arrays.as_deref(),
        &mut meshes,
        spawn_budget(*quality),
    );
    draw_heaps(
        &mut commands,
        &mut shards,
        &by_root,
        &look,
        arrays.as_deref(),
        &mut meshes,
    );
    // A slow step said, so a trace can tell fragments from the rest of a frame.
    let took = began.elapsed().as_secs_f32() * 1000.0;
    if took > SLOW_MS {
        info!(
            "Build: fragments stepped: {} in {took:.1} ms ({steps} steps)",
            shards.live.len()
        );
    }
}

/// Every kind's fragments made ready before anything comes down, and the layers they wear
/// kept resident, so the first fall is drawn in its material from its first frame; those
/// drawn to warm up gone once they have been.
fn warm(
    commands: &mut Commands,
    delta_s: f32,
    site: &Site,
    look: &BuildLook,
    arrays: Option<&Arrays>,
    shards: &mut ResMut<Shards>,
    meshes: &mut ResMut<Assets<Mesh>>,
) {
    if !shards.warmed
        && let Some(kits) = look.kits.clone()
    {
        shards.warmed = true;
        // Each drawn once, out of sight under the ground and never culled, so its pipelines
        // are compiled before the first fall rather than leaving a hole for its first frames.
        let under = site.frame.transform_point(Vec3::new(0.0, -80.0, 0.0));
        let sizes = [(Shape::Chunk, 0, 0.25), (Shape::Lump, 0, 0.25)]
            .into_iter()
            .chain(
                (1..=fragments::MOST_DRAWN)
                    .map(|across| (Shape::Whole, across, f32::from(across) * CELL_M)),
            );
        let sizes: Vec<_> = sizes.collect();
        for kit in &kits.0 {
            for &size in &sizes {
                let made = mesh_for(shards, look, arrays, meshes, kit.kind(), size);
                if let Some(drawn) = made {
                    let at = Transform::from_translation(under).with_scale(Vec3::splat(0.01));
                    let entity = spawn(commands, drawn, at);
                    commands.entity(entity).insert(NoFrustumCulling);
                    shards.warming.push(entity);
                }
            }
        }
    }
    if !shards.warming.is_empty() {
        shards.warm_s += delta_s;
        if shards.warm_s > WARM_S {
            for entity in shards.warming.drain(..) {
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Every fragment moved on by as many fixed steps as the time behind asks, each landing on
/// the terrain and the base's cubes; a whole cube landing hard broken into pieces, as many
/// as `most` and the `preset` share leave room for. How many steps were taken, and where
/// something landed hard enough to raise dust: the place, its kind and how much.
fn fall(
    shards: &mut Shards,
    by_root: &BTreeMap<Entity, &Build>,
    planet: &Planet,
    look: &BuildLook,
    delta_s: f32,
    (most, preset): (usize, f32),
) -> (u32, Vec<Landed>) {
    let breaks = breaks_of(look);
    shards.behind = (shards.behind + delta_s).min(STEP_S * MOST_STEPS as f32);
    let mut steps = 0;
    let mut born: Vec<Live> = Vec::new();
    let mut landed: Vec<Landed> = Vec::new();
    while shards.behind >= STEP_S && steps < MOST_STEPS {
        shards.behind -= STEP_S;
        shards.clock_s += STEP_S;
        let now = shards.clock_s;
        steps += 1;
        let mut laid: Vec<(Entity, Fragment)> = Vec::new();
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
            let fading = live.fading();
            if fading {
                live.still_s += STEP_S;
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
            let heap = shards.rubble.get(&root);
            // On the heap of what fell before, else on the ground or what is built.
            let floor = |at: Vec3| -> f32 {
                let under = under(&mut cells.borrow_mut(), (root, *build), planet, look, at);
                heap.and_then(|heap| heap.top(at, now))
                    .map_or(under, |top| top.max(under))
            };
            let solid = |at: Vec3| base.solid_at(at);
            let landing = live.fragment.step(STEP_S, &floor, &solid);
            // What it raises where it lands: its place, its kind, how much, how big it is
            // and the way it was going, in the world.
            let raised = |fragment: &Fragment, amount: f32| Landed {
                at: frame.transform_point(fragment.at),
                kind: fragment.kind,
                amount,
                size: fragment.size.max_element(),
                along: frame.rotation * fragment.velocity.with_y(0.0),
            };
            if landing.breaks && !fading {
                // What it breaks into lies where it landed, and raises the heap there.
                laid.push((root, live.fragment));
                let kind = live.fragment.kind;
                let breaking = breaks(kind);
                // Some of its pieces however full the budget is: the least worth make room
                // for them (`fade_least`), so a chunk never cracks into holes.
                let share = (room as f32 / breaking.count() as f32).max(LEAST_SHARE);
                let pieces = live.fragment.shatter(breaking, preset.min(share));
                room = room.saturating_sub(pieces.len());
                landed.push(raised(&live.fragment, breaking.look().dust * 0.6));
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
                landed.push(raised(&live.fragment, 0.35));
            }
        }
        let terrain = std::cell::RefCell::new(&mut terrain);
        for (root, fragment) in laid {
            let Some(build) = by_root.get(&root) else {
                continue;
            };
            let floor =
                |at: Vec3| under(&mut terrain.borrow_mut(), (root, *build), planet, look, at);
            shards
                .rubble
                .entry(root)
                .or_default()
                .lay(&fragment, &floor, now);
        }
    }
    // What broke on landing: moving from now, drawn at the end with the rest.
    shards.live.extend(born);
    (steps, landed)
}

/// What a point in a build's space stands on, the heap aside: what is built under it, a
/// roof's covering as drawn over its cubes by its thickness, else the ground, its height
/// read once a column (`terrain`).
fn under(
    terrain: &mut BTreeMap<(Entity, [i32; 2]), f32>,
    (root, build): (Entity, &Build),
    planet: &Planet,
    look: &BuildLook,
    at: Vec3,
) -> f32 {
    let column = [
        (at.x / CELL_M).floor() as i32,
        (at.z / CELL_M).floor() as i32,
    ];
    let ground = *terrain.entry((root, column)).or_insert_with(|| {
        let middle = Vec3::new(
            (column[0] as f32 + 0.5) * CELL_M,
            0.0,
            (column[1] as f32 + 0.5) * CELL_M,
        );
        -planet.distance(build.frame.transform_point(middle))
    });
    build.base.top_under_of(at).map_or(ground, |(top, kind)| {
        let skin = if kind.is_roof() {
            look.kits
                .as_deref()
                .and_then(|kits| kits.of(kind))
                .map_or(0.0, |kit| kit.roof.thick + kit.roof.relief * 0.5)
        } else {
            0.0
        };
        (top + skin).max(ground)
    })
}

/// Never far past the most: those of least worth make room ([`fade_least`]), and past twice
/// as many, those of least worth not yet drawn are let go at once.
fn keep_within(shards: &mut Shards, most: usize) {
    let over = shards
        .live
        .iter()
        .filter(|live| !live.fading())
        .count()
        .saturating_sub(most);
    fade_least(shards, over);
    let past = shards.live.len().saturating_sub(most * 2);
    if past > 0 {
        let mut undrawn: Vec<usize> = (0..shards.live.len())
            .filter(|&i| shards.live[i].entity == Entity::PLACEHOLDER)
            .collect();
        let by = |a: &usize, b: &usize| worth(&shards.live[*a]).total_cmp(&worth(&shards.live[*b]));
        if past < undrawn.len() {
            undrawn.select_nth_unstable_by(past, by);
        }
        let gone: BTreeSet<usize> = undrawn.into_iter().take(past).collect();
        let mut index = 0;
        shards.live.retain(|_| {
            index += 1;
            !gone.contains(&(index - 1))
        });
    }
}

/// Each where it is now, those not yet drawn drawn `spawns` a frame; those shrunk away gone.
#[allow(clippy::too_many_arguments)]
fn draw_where_they_are(
    commands: &mut Commands,
    shards: &mut Shards,
    by_root: &BTreeMap<Entity, &Build>,
    transforms: &mut Query<&mut Transform, With<Shard>>,
    look: &BuildLook,
    arrays: Option<&Arrays>,
    meshes: &mut ResMut<Assets<Mesh>>,
    mut spawns: usize,
) {
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
                    shards,
                    look,
                    arrays,
                    meshes,
                    live.fragment.kind,
                    live.fragment.mesh(),
                )
            {
                spawns -= 1;
                live.entity = spawn(commands, drawn, placed(&build.frame, &live));
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
    building.act_later(None, "damage", vec![op]);
}

/// How many fragments are about, for the remote's state.
pub(super) fn live(world: &World) -> usize {
    world
        .get_resource::<Shards>()
        .map_or(0, |shards| shards.live.len())
}

#[cfg(test)]
mod tests;
