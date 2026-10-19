//! Fire as it is drawn (D377): flames, embers and smoke wherever something burns, a jet of
//! flame where something breathes fire, and the light they give. What burns, and where, is
//! the layer above's to say ([`Blazes`]); this draws it, as smoke (D325) is drawn: instanced
//! soft cards, moved on the GPU from each card's seed, the time and the wind, on our own
//! materials, with no particle library.
//!
//! As shipped games draw fire (research/2026-10-15-dragons-and-fire.md,
//! research/2026-10-16-real-time-fire-rendering.md): a body sheet to a blaze where it burns
//! and a few flamelets breaking off its top, eroded by noise scrolled up through them, their
//! colour glowing soot's at its temperature (1050 to 1650 K: deep red, orange, a yellow
//! heart, never white or pink by day), faded in where they meet what is behind; embers as
//! sparks stretched along their flight, rising with the plume and carried downwind; smoke
//! above as near-black lit cards paling as they climb, glowing from under where the fire is;
//! heat haze over the flames; and a few point lights where the fire is greatest, flickering
//! as a fire that size puffs, not one a flame.
//!
//! Every blaze is a slot of one buffer the GPU reads, so all flames are one draw, all
//! embers another, all smoke a third, however many burn; how many slots, by the preset.

use crate::adaptation::Adaptation;
use crate::climate::ClimateFields;
use crate::daylight::Clock;
use crate::footprint::Footprint;
use crate::quality::Quality;
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::{
        render_resource::{AsBindGroup, Face},
        storage::ShaderBuffer,
    },
    shader::ShaderRef,
};

/// Something burning, as the layer above says it: where its flames stand from (their foot,
/// in the world), how tall they are in metres, how hot (0 to 1, a dying fire's embers to a
/// blaze's heart), and a number of its own so no two flicker alike.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blaze {
    pub at: Vec3,
    pub tall: f32,
    pub heat: f32,
    pub seed: f32,
    /// How wide its flame is, in metres: nought for as wide as a flame so tall is, more for
    /// a sheet of fire over a broad burning surface.
    pub wide: f32,
}

/// A jet of flame breathed out: from where, which way, how far it reaches, how wide it
/// spreads (the tangent of its half angle), and whether it is on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jet {
    pub from: Vec3,
    pub way: Vec3,
    pub reach: f32,
    pub spread: f32,
    pub on: bool,
}

/// Everything burning now, set by whatever owns the fires each frame: the blazes, the
/// largest first (those past the preset's slots are not drawn), and the jets; and the fires
/// kept where something [`Kindled`] stands, gathered here (`keep`), drawn first.
#[derive(Resource, Default, Debug)]
pub struct Blazes {
    pub blazes: Vec<Blaze>,
    pub jets: Vec<Jet>,
    kept: Vec<Blaze>,
}

impl Blazes {
    /// Every blaze drawn: the kept fires, then the rest.
    pub fn all(&self) -> impl Iterator<Item = &Blaze> {
        self.kept.iter().chain(&self.blazes)
    }
}

/// A fire kept burning where this stands, as a campfire, a hearth or a brazier is: drawn as a
/// blaze of so tall and so wide flames and so hot (as [`Blaze`]) at its foot, with its embers,
/// smoke and flickering light, as long as it is there. A campfire's are knee-high flames
/// over a bed of embers, a thin smoke rising (research/2026-10-16-real-time-fire-rendering.md).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Kindled {
    pub tall: f32,
    pub wide: f32,
    pub heat: f32,
}

/// How a fire's light wavers at a time, for a light of its own number and so bright: as a
/// fire that size puffs (1.5 / sqrt(D) Hz, D its width, taken from its light), up to 4 Hz,
/// by a fifth either way for a small fire and less for a big one, whose many flames even
/// out; and how far it bobs up and down, in metres.
fn flicker(t: f32, k: f32, lumens: f32) -> (f32, f32) {
    let wide = (lumens / LUMENS_A_METRE).sqrt().clamp(0.5, 8.0);
    let hz = (1.5 / wide.sqrt()).min(4.0);
    let reach = 0.22 - 0.14 * ((wide - 0.5) / 7.5);
    let a = core::f32::consts::TAU * hz * t + k;
    let wave = 0.55 * libm::sinf(a)
        + 0.3 * libm::sinf(a * 2.31 + 1.3 * k)
        + 0.15 * libm::sinf(a * 4.7 + 0.4);
    (1.0 + reach * wave, 0.2 * libm::sinf(a * 0.7 + 2.0 * k))
}

/// How many blazes are drawn at most, by preset.
pub fn slots(quality: Quality) -> usize {
    match quality {
        Quality::Low => 384,
        Quality::Medium => 1024,
        Quality::High => 2048,
    }
}

/// How many cards a blaze has of each: flames, embers, smoke and heat haze.
const FLAME_CARDS: usize = 3;
const EMBER_CARDS: usize = 5;
const SMOKE_CARDS: usize = 4;
const HAZE_CARDS: usize = 1;
/// The fires kept where something kindled stands, a campfire or a hearth, are drawn as a
/// campfire is in footage and in the games that draw one well (The Witcher 3, Red Dead
/// Redemption 2, Kingdom Come): several tongues of flame of their own heights over a bed of
/// glowing coals among the logs, a few sparks, a thin grey wisp of smoke warm only at its
/// foot, and a light of their own. At most so many of them; must match `HEARTHS` in
/// fire.wesl and fire_smoke.wesl. Each has so many flame cards (its coals, then its tongues)
/// and so many puffs of smoke, close enough in time to rise as one column.
const HEARTHS: usize = 16;
const HEARTH_FLAME_CARDS: usize = 6;
const HEARTH_SMOKE_CARDS: usize = 24;
/// A hearth's sparks, ten or so a second rising from its coals, a few seen at once.
const HEARTH_EMBER_CARDS: usize = 16;
/// A hearth's light, lumens for a fire a metre tall at full heat, and how far it reaches: a
/// campfire lights the faces round it and a ring of the ground, not the trees across a lake.
const HEARTH_LUMENS_A_METRE: f32 = 280_000.0;
const HEARTH_RANGE_M: f32 = 14.0;
/// How many cards a jet is, and how many jets are drawn.
const JETS: usize = 2;
/// What a breath leaves in the air, as a flamethrower's stream does (research/
/// 2026-10-18-dragon-breath.md): black soot rolling off it and burning drops falling from it.
/// Each is let go where the stream is when it is born and then moves on its own, so it stays
/// where it was breathed as the breather flies on: how many are kept, and how many a second
/// a breath lets go.
const SOOT_PUFFS: usize = 320;
const DROPS: usize = 112;
const SOOT_A_SECOND: f32 = 55.0;
const DROPS_A_SECOND: f32 = 45.0;
/// How many point lights the fires are lit by, and how far round each its fire is gathered,
/// in metres.
const LIGHTS: usize = 12;
const LIGHT_CELL_M: f32 = 9.0;
/// How far out from the middle of all that burns a gathering's light hangs, metres.
const LIGHT_OUT_M: f32 = 3.0;
/// A blaze's light, in lumens for a blaze a metre tall at full heat, how far it reaches, and
/// the most one light gives: still some times a real fire's, for the film, but no longer so
/// much that sunlit stone round a fire goes salmon pink; a breath's brighter, as every game
/// that draws one well lights the ground under it. Eight lights at 3e7 left the walls round a
/// burning keep unlit by it in daylight (castle film r7).
const LUMENS_A_METRE: f32 = 500_000.0;
const MOST_LUMENS: f32 = 4.0e7;
/// How far a blaze's light and a jet's reach, metres: at 55 every light tinted every sunlit wall
/// of the burg a uniform salmon (r8's first probe), where firelight by day shows only close
/// round the fire.
const LIGHT_RANGE_M: f32 = 22.0;
const JET_RANGE_M: f32 = 30.0;
/// A jet's light just past the mouth, on the breather's head and neck, along its stream and
/// where it strikes: at 1e7 the dragon and the wall it struck showed none of it by day (castle
/// film r7). Its colour the stream's yellow, a blaze's the orange of burning timber.
const JET_LUMENS: f32 = 5.0e7;
const JET_LIGHT: Color = Color::srgb(1.0, 0.72, 0.38);
const BLAZE_LIGHT: Color = Color::srgb(1.0, 0.62, 0.3);
/// A campfire's, wood's flame about 1,800 K: an orange, not the yellow the grass turned a
/// timber blaze's (lake film r5).
const HEARTH_LIGHT: Color = Color::srgb(1.0, 0.5, 0.18);
/// The least a fire's smoke drifts, metres a second.
const LEAST_DRAUGHT_MPS: f32 = 1.6;

/// What a fire's material is told. Field order must match `fire.wesl`.
#[derive(Clone, Copy, Debug, Default, Reflect, bevy::render::render_resource::ShaderType)]
pub struct FireUniform {
    /// x which cards (0 flames, 1 embers, 2 jets), y how many slots are filled, z the time
    /// the cards move by, w unused.
    kind: Vec4,
    /// The wind where the fire is, metres a second in the world, and its strength.
    wind: Vec4,
    /// Each jet: from, and its reach; its way, and its spread; whether it is on (1) or since
    /// when off, and when it came on.
    jets: [Vec4; JETS * 3],
}

/// Flames, embers and jets: drawn unlit, their light added to what is behind, on Bevy's own
/// material as smoke is, its shaders ours.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[bind_group_data(FireKey)]
pub struct FireExtension {
    #[uniform(100)]
    uniform: FireUniform,
    /// Whether these are the breaths' hulls, marched through as volumes (`hulls`).
    volume: bool,
    #[storage(101, read_only)]
    slots: Handle<ShaderBuffer>,
    /// The flames' flipbook (`tools/textures/flames.ts`): a real fire filmed at night, frame by
    /// frame its red and green, soot and cover, linear.
    #[texture(102)]
    #[sampler(103)]
    flipbook: Handle<Image>,
    /// The breaths' soot and drops (`Puffs`).
    #[storage(104, read_only)]
    puffs: Handle<ShaderBuffer>,
    /// The exposure the eye is adapted to (`adaptation`), so a flame's hottest cores clip
    /// white on the screen whatever the adaptation does.
    #[storage(105, read_only)]
    adaptation: Handle<ShaderBuffer>,
}

/// The flames' flipbook.
const FLIPBOOK: &str = "textures/fire/flames.png";

/// Which pipeline a fire material is drawn with: a breath's hull is not depth tested, since
/// its far side stands behind the wall it strikes; its shader ends each ray at what the
/// depth prepass holds instead.
#[repr(C)]
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct FireKey {
    volume: bool,
}

impl From<&FireExtension> for FireKey {
    fn from(fire: &FireExtension) -> Self {
        FireKey {
            volume: fire.volume,
        }
    }
}

impl MaterialExtension for FireExtension {
    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        if key.bind_group_data.volume
            && let Some(depth) = descriptor.depth_stencil.as_mut()
        {
            depth.depth_compare = Some(bevy::render::render_resource::CompareFunction::Always);
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }

    fn vertex_shader() -> ShaderRef {
        "shaders/fire.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/fire.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/fire.wesl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/fire.wesl".into()
    }
}

pub type FireMaterial = ExtendedMaterial<StandardMaterial, FireExtension>;

/// What the smoke over a fire is told. Field order must match `fire_smoke.wesl`.
#[derive(Clone, Copy, Debug, Default, Reflect, bevy::render::render_resource::ShaderType)]
pub struct FireSmokeUniform {
    /// The wind, and how many slots are filled.
    wind: Vec4,
    /// x which smoke (0 over the blazes, 1 a breath's soot), y the time the puffs were let
    /// go by.
    kind: Vec4,
}

/// The smoke over a fire, or a breath's soot.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct FireSmokeExtension {
    #[uniform(100)]
    uniform: FireSmokeUniform,
    #[storage(101, read_only)]
    slots: Handle<ShaderBuffer>,
    #[storage(102, read_only)]
    puffs: Handle<ShaderBuffer>,
}

impl MaterialExtension for FireSmokeExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/fire_smoke.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/fire_smoke.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/fire_smoke.wesl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/fire_smoke.wesl".into()
    }
}

pub type FireSmokeMaterial = ExtendedMaterial<StandardMaterial, FireSmokeExtension>;

/// The fires' drawing: their buffer, their materials, and their lights.
#[derive(Resource)]
struct Drawn {
    slots: Handle<ShaderBuffer>,
    fire: [Handle<FireMaterial>; 5],
    smoke: [Handle<FireSmokeMaterial>; 2],
    puffs: Puffs,
    lights: Vec<Entity>,
    /// When each jet came on or went off, by the cards' clock.
    jets: [(bool, f32); JETS],
    wind: Vec3,
    wind_due: f32,
}

/// The soot and drops breaths have let go, in two rings the GPU reads: each two vec4s, where
/// it was let go and when, then how fast it was going and its luck. The soot first, then
/// the drops.
struct Puffs {
    buffer: Handle<ShaderBuffer>,
    data: Vec<[f32; 4]>,
    next: [usize; 2],
    /// What is owed of each, a fraction of one, carried to the next frame.
    owed: [f32; 2],
    luck: u32,
}

impl Puffs {
    fn roll(&mut self) -> f32 {
        self.luck = self.luck.wrapping_add(1);
        (terra_gen::hash::lowbias32(self.luck) >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Let go of what each breath sheds this frame, `step` seconds long, at time `now`.
    fn shed(&mut self, jets: &[Jet], step: f32, now: f32) {
        for (ring, (rate, size, from)) in [
            (SOOT_A_SECOND, SOOT_PUFFS, 0),
            (DROPS_A_SECOND, DROPS, SOOT_PUFFS),
        ]
        .into_iter()
        .enumerate()
        {
            for jet in jets.iter().filter(|jet| jet.on && jet.reach > 0.0) {
                self.owed[ring] += rate * step;
                while self.owed[ring] >= 1.0 {
                    self.owed[ring] -= 1.0;
                    let way = jet.way.normalize_or(Vec3::Z);
                    let side = way.any_orthonormal_vector();
                    let lift = way.cross(side);
                    let a = self.roll() * core::f32::consts::TAU;
                    let (along, out, speed) = if ring == 0 {
                        // Soot rolls off the stream's outer two thirds and most where it
                        // strikes, carried on a while by the stream.
                        (0.3 + 0.7 * self.roll().sqrt(), 1.0, 9.0)
                    } else {
                        // Drops fall from its underside, from near the mouth to its end,
                        // still going most of the stream's speed.
                        (0.12 + 0.7 * self.roll(), 0.6, 15.0)
                    };
                    let d = jet.reach * along;
                    let r = jet.spread * d * out * self.roll().sqrt();
                    let at = jet.from + way * d + (side * libm::cosf(a) + lift * libm::sinf(a)) * r;
                    let wobble = Vec3::new(self.roll(), self.roll(), self.roll()) - 0.5;
                    let going = way * speed * (0.6 + 0.4 * self.roll()) + wobble * 3.0;
                    let luck = self.roll();
                    let i = from + self.next[ring];
                    self.next[ring] = (self.next[ring] + 1) % size;
                    self.data[i * 2] = at.extend(now).to_array();
                    self.data[i * 2 + 1] = going.extend(luck).to_array();
                }
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    seen::plugin(app);
    app.init_resource::<Blazes>()
        .add_plugins((
            MaterialPlugin::<FireMaterial>::default(),
            MaterialPlugin::<FireSmokeMaterial>::default(),
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, scorch)
        .add_systems(PostUpdate, (keep, blow, show, light).chain());
}

/// A mesh of cards, `count` of them, every corner at the origin: the first texture
/// coordinate the corner, the second which card and of which slot, the card's place among
/// the slot's counted `from` on (1 for a hearth's, so the shaders tell them apart).
fn cards(slots: usize, each: usize, from: f32) -> Mesh {
    let count = slots * each;
    let mut positions = Vec::with_capacity(count * 4);
    let mut normals = Vec::with_capacity(count * 4);
    let mut corners = Vec::with_capacity(count * 4);
    let mut ids = Vec::with_capacity(count * 4);
    let mut indices = Vec::with_capacity(count * 6);
    for slot in 0..slots {
        for card in 0..each {
            let first = positions.len() as u32;
            for corner in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
                positions.push([0.0f32, 0.0, 0.0]);
                normals.push([0.0f32, 1.0, 0.0]);
                corners.push(corner);
                ids.push([slot as f32, from + card as f32 / each as f32]);
            }
            indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, corners)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, ids)
    .with_inserted_indices(Indices::U32(indices))
}

/// The breaths' hulls: a box for each jet, its corners 0 to 1 on each axis (its position),
/// which the shader stretches round the jet's stream and fireball and marches through
/// (`fire.wesl`, `breath`), its back faces drawn so it shows with the eye inside it too.
fn hulls(slots: usize) -> Mesh {
    let mut positions = Vec::with_capacity(slots * 24);
    let mut normals = Vec::with_capacity(slots * 24);
    let mut ids = Vec::with_capacity(slots * 24);
    let mut indices = Vec::with_capacity(slots * 36);
    for slot in 0..slots {
        for axis in 0..3 {
            for side in [0.0f32, 1.0] {
                // (u, v, axis) turn as x, y, z do, so the face runs anticlockwise seen from
                // its outside when it is at 1, and is turned over when it is at 0.
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                let first = positions.len() as u32;
                for (a, b) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                    let mut p = [0.0f32; 3];
                    p[axis] = side;
                    p[u] = a;
                    p[v] = b;
                    let mut n = [0.0f32; 3];
                    n[axis] = side * 2.0 - 1.0;
                    positions.push(p);
                    normals.push(n);
                    ids.push([slot as f32, 0.0]);
                }
                if side > 0.5 {
                    indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
                } else {
                    indices.extend([first, first + 2, first + 1, first, first + 3, first + 2]);
                }
            }
        }
    }
    let corners = vec![[0.0f32, 0.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, corners)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, ids)
    .with_inserted_indices(Indices::U32(indices))
}

/// The cards of every kind, each a mesh of its own drawn wherever the fires are.
fn spawn_cards(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    count: usize,
    fire: &[Handle<FireMaterial>; 5],
    smoke: &[Handle<FireSmokeMaterial>; 2],
) {
    // Drawn wherever the fires are: the cards are placed on the GPU, so no bounds would hold.
    let mut spawn = |name: &str, mesh: Mesh, material: Handle<FireMaterial>| {
        commands.spawn((
            Name::new(name.to_string()),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            NoFrustumCulling,
            NotShadowCaster,
        ));
    };
    spawn(
        "Fire: flames",
        cards(count, FLAME_CARDS, 0.0),
        fire[0].clone(),
    );
    spawn(
        "Fire: hearth flames",
        cards(HEARTHS.min(count), HEARTH_FLAME_CARDS, 1.0),
        fire[0].clone(),
    );
    spawn(
        "Fire: embers",
        cards(count, EMBER_CARDS, 0.0),
        fire[1].clone(),
    );
    spawn(
        "Fire: hearth embers",
        cards(HEARTHS.min(count), HEARTH_EMBER_CARDS, 1.0),
        fire[1].clone(),
    );
    spawn("Fire: jets", hulls(JETS), fire[2].clone());
    spawn("Fire: haze", cards(count, HAZE_CARDS, 0.0), fire[3].clone());
    spawn("Fire: drops", cards(DROPS, 1, 0.0), fire[4].clone());
    for (name, mesh, material) in [
        (
            "Fire: smoke",
            cards(count, SMOKE_CARDS, 0.0),
            smoke[0].clone(),
        ),
        (
            "Fire: hearth smoke",
            cards(HEARTHS.min(count), HEARTH_SMOKE_CARDS, 1.0),
            smoke[0].clone(),
        ),
        ("Fire: soot", cards(SOOT_PUFFS, 1, 0.0), smoke[1].clone()),
    ] {
        commands.spawn((
            Name::new(name),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            NoFrustumCulling,
            NotShadowCaster,
        ));
    }
}

fn setup(
    mut commands: Commands,
    quality: Res<Quality>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    assets: Res<AssetServer>,
    adaptation: Res<Adaptation>,
    mut fires: ResMut<Assets<FireMaterial>>,
    mut smokes: ResMut<Assets<FireSmokeMaterial>>,
) {
    let count = slots(*quality);
    let slots = buffers.add(ShaderBuffer::from(vec![[0.0f32; 4]; count * 2]));
    // Heat, not colour: read as it is stored.
    let flipbook: Handle<Image> = assets
        .load_builder()
        .with_settings(|settings: &mut bevy::image::ImageLoaderSettings| {
            settings.is_srgb = false;
        })
        .load(FLIPBOOK);
    // Nothing let go yet: born an hour ago, long gone.
    let puffs = Puffs {
        buffer: buffers.add(ShaderBuffer::from(
            [[0.0f32, 0.0, 0.0, -3600.0], [0.0; 4]].repeat(SOOT_PUFFS + DROPS),
        )),
        data: [[0.0f32, 0.0, 0.0, -3600.0], [0.0; 4]].repeat(SOOT_PUFFS + DROPS),
        next: [0, 0],
        owed: [0.0, 0.0],
        luck: 0,
    };
    let fire = [0.0, 1.0, 2.0, 3.0, 4.0].map(|kind| {
        // The haze reads the opaque scene as Bevy copies it for transmissive materials, so it
        // is drawn among them, blended over it.
        let haze = kind == 3.0;
        fires.add(FireMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                alpha_mode: if haze {
                    AlphaMode::Blend
                } else {
                    AlphaMode::Add
                },
                specular_transmission: if haze { 1.0 } else { 0.0 },
                unlit: true,
                // Drawn after the smoke, which is sorted with it by the same middle: smoke
                // drawn over its own flames greyed and dimmed their yellow hearts to olive
                // (castle film r5 and r6). A fire's flames show through the smoke over it.
                // A breath is drawn after the flames too, as it is opaque and stands nearer
                // the eye than the walls it strikes: tied with them, a burning tower's flame
                // cards behind its fireball were drawn over it, a lighter patch with their
                // straight edges, and smoke behind it browned it (castle film r11).
                depth_bias: if haze {
                    0.0
                } else if kind == 2.0 {
                    2.0
                } else {
                    1.0
                },
                // A breath's hull is drawn from its far side, so it shows with the eye in it.
                cull_mode: if kind == 2.0 {
                    Some(Face::Front)
                } else {
                    Some(Face::Back)
                },
                ..default()
            },
            extension: FireExtension {
                uniform: FireUniform {
                    kind: Vec4::new(kind, 0.0, 0.0, 0.0),
                    ..default()
                },
                volume: kind == 2.0,
                slots: slots.clone(),
                flipbook: flipbook.clone(),
                puffs: puffs.buffer.clone(),
                adaptation: adaptation.0.clone(),
            },
        })
    });
    let smoke = [0.0, 1.0].map(|kind| {
        smokes.add(FireSmokeMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 1.0,
                reflectance: 0.0,
                diffuse_transmission: 0.1,
                ..default()
            },
            extension: FireSmokeExtension {
                uniform: FireSmokeUniform {
                    kind: Vec4::new(kind, 0.0, 0.0, 0.0),
                    ..default()
                },
                slots: slots.clone(),
                puffs: puffs.buffer.clone(),
            },
        })
    });
    spawn_cards(&mut commands, &mut meshes, count, &fire, &smoke);
    let lights = (0..LIGHTS)
        .map(|i| {
            commands
                .spawn((
                    Name::new(format!("Fire: light {i}")),
                    PointLight {
                        color: BLAZE_LIGHT,
                        intensity: 0.0,
                        range: LIGHT_RANGE_M,
                        radius: 1.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::IDENTITY,
                    Visibility::Hidden,
                ))
                .id()
        })
        .collect();
    commands.insert_resource(Drawn {
        slots,
        fire,
        smoke,
        puffs,
        lights,
        jets: [(false, -100.0); JETS],
        wind: Vec3::ZERO,
        wind_due: 0.0,
    });
}

/// How far round a kept fire nothing grows, past half its flames' width, in metres, and the
/// cells it is cleared in: a campfire's ring of stones and the scorched, trodden earth round
/// it, a metre or so out. With the grass grown up to the stones and through its flames, the
/// flames were see-through where each blade stood just behind them, faded as in front of a
/// wall (soft particles; lake film r7). The ring itself reaches about as far as half its
/// flames' width and more (a troll's campfire's stones 0.85 m out, its flames 0.9 m wide): at
/// 0.8 m past them the blades, leaning in, still reached the stones (lake film r8), where a
/// kept camp's ground is bare a stride round its ring (a 10-foot cleared circle, as the US
/// Forest Service asks of a campfire).
const SCORCHED_M: f32 = 1.6;
const SCORCHED_CELL_M: f32 = 0.25;

/// Clears the ground round each fire newly kindled, as what is built clears it ([`Footprint`]),
/// so the grass and the plants keep out.
fn scorch(
    mut commands: Commands,
    kindled: Query<(Entity, &Kindled, &Transform), (Added<Kindled>, Without<Footprint>)>,
) {
    for (entity, fire, at) in &kindled {
        commands.entity(entity).insert(scorched(fire, at));
    }
}

/// The ground a fire clears, round its foot.
fn scorched(fire: &Kindled, at: &Transform) -> Footprint {
    let radius = fire.wide * 0.5 + SCORCHED_M;
    let reach = (radius / SCORCHED_CELL_M).ceil() as i32;
    let ground = (-reach..reach)
        .flat_map(|x| (-reach..reach).map(move |z| IVec2::new(x, z)))
        .filter(|cell| ((cell.as_vec2() + 0.5) * SCORCHED_CELL_M).length() <= radius)
        .collect();
    Footprint {
        frame: Transform::from_translation(at.translation).with_rotation(at.rotation),
        cell_m: SCORCHED_CELL_M,
        ground,
        standing: Default::default(),
    }
}

/// The fires kept where something kindled stands, each with a number of its own from its
/// place, so no two flicker alike.
fn keep(kindled: Query<(&Kindled, &GlobalTransform)>, mut blazes: ResMut<Blazes>) {
    if kindled.is_empty() && blazes.kept.is_empty() {
        return;
    }
    let kept: Vec<Blaze> = kindled
        .iter()
        .map(|(fire, at)| {
            let at = at.translation();
            Blaze {
                at,
                tall: fire.tall,
                heat: fire.heat,
                seed: (at.x * 0.731 + at.z * 0.377).rem_euclid(1.0),
                wide: fire.wide,
            }
        })
        .collect();
    if kept != blazes.kept {
        blazes.kept = kept;
    }
}

/// The wind where the fires are, as the climate has it, looked up now and then.
fn blow(
    time: Res<Time>,
    clock: Res<Clock>,
    climate: Option<Res<ClimateFields>>,
    blazes: Res<Blazes>,
    mut drawn: ResMut<Drawn>,
) {
    drawn.wind_due -= time.delta_secs();
    let Some(here) = blazes.all().next().map(|b| b.at) else {
        return;
    };
    if drawn.wind_due > 0.0 {
        return;
    }
    drawn.wind_due = 3.0;
    let up = here.normalize_or(Vec3::Y);
    let (speed, from, north) = match &climate {
        Some(climate) => {
            let wind = climate.fields.at(climate.place(here), clock.time()).wind;
            (wind.speed_m_s, wind.from, climate.frame.north(here))
        }
        None => (
            1.5,
            0.8,
            Vec3::Y
                .reject_from(up)
                .normalize_or(up.any_orthonormal_vector()),
        ),
    };
    let east = north.cross(up);
    let blowing_to = -(north * libm::cosf(from) + east * libm::sinf(from));
    // A fire makes its own draught and the air is never quite still over one: its smoke
    // leans at least as a light air carries it.
    drawn.wind = blowing_to * speed.clamp(LEAST_DRAUGHT_MPS, 10.0);
}

/// The blazes into the buffer, the hottest and tallest first, and the materials told how
/// many there are, the wind and the jets.
fn show(
    time: Res<Time>,
    quality: Res<Quality>,
    blazes: Res<Blazes>,
    mut drawn: ResMut<Drawn>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut fires: ResMut<Assets<FireMaterial>>,
    mut smokes: ResMut<Assets<FireSmokeMaterial>>,
) {
    let now = time.elapsed_secs_wrapped();
    let count = blazes.all().count().min(slots(*quality));
    if let Some(mut buffer) = buffers.get_mut(&drawn.slots) {
        let mut data: Vec<[f32; 4]> = Vec::with_capacity(slots(*quality) * 2);
        // The kept fires first, marked so they stand just where they are.
        let kept = blazes.kept.len();
        for (i, blaze) in blazes.all().take(count).enumerate() {
            data.push([blaze.at.x, blaze.at.y, blaze.at.z, blaze.tall]);
            data.push([
                blaze.heat,
                blaze.seed,
                blaze.wide,
                f32::from(u8::from(i < kept)),
            ]);
        }
        data.resize(slots(*quality) * 2, [0.0; 4]);
        *buffer = ShaderBuffer::from(data);
    }
    let mut jets = [Vec4::ZERO; JETS * 3];
    for (i, state) in drawn.jets.iter_mut().enumerate() {
        let jet = blazes.jets.get(i).copied();
        let on = jet.is_some_and(|jet| jet.on);
        if on != state.0 {
            *state = (on, now);
        }
        let jet = jet.unwrap_or(Jet {
            from: Vec3::ZERO,
            way: Vec3::Z,
            reach: 0.0,
            spread: 0.0,
            on: false,
        });
        jets[i * 3] = jet.from.extend(jet.reach);
        jets[i * 3 + 1] = jet.way.normalize_or(Vec3::Z).extend(jet.spread);
        jets[i * 3 + 2] = Vec4::new(f32::from(u8::from(state.0)), state.1, 0.0, 0.0);
    }
    drawn.puffs.shed(&blazes.jets, time.delta_secs(), now);
    if let Some(mut buffer) = buffers.get_mut(&drawn.puffs.buffer) {
        *buffer = ShaderBuffer::from(drawn.puffs.data.clone());
    }
    let wind = drawn.wind.extend(drawn.wind.length());
    for (kind, handle) in drawn.fire.iter().enumerate() {
        if let Some(mut material) = fires.get_mut(handle) {
            material.extension.uniform.kind = Vec4::new(kind as f32, count as f32, now, 0.0);
            material.extension.uniform.wind = wind;
            material.extension.uniform.jets = jets;
        }
    }
    for (kind, handle) in drawn.smoke.iter().enumerate() {
        if let Some(mut material) = smokes.get_mut(handle) {
            material.extension.uniform.wind = drawn.wind.extend(count as f32);
            material.extension.uniform.kind = Vec4::new(kind as f32, now, 0.0, 0.0);
        }
    }
}

/// One of the fires' lights.
struct Lit {
    at: Vec3,
    lumens: f32,
    color: Color,
    range: f32,
    lift: f32,
    flickers: f32,
}

/// The fires' light: each of a few lights where the most fire is, gathered over cells of a
/// few metres, at the cell's fire's middle and as bright as it burns, wavering; and a light
/// at each jet's mouth and where it reaches.
fn light(
    time: Res<Time>,
    blazes: Res<Blazes>,
    drawn: Res<Drawn>,
    mut lights: Query<(&mut PointLight, &mut Transform, &mut Visibility)>,
) {
    let t = time.elapsed_secs_wrapped();
    let lit = lit(&blazes);
    for (i, &entity) in drawn.lights.iter().enumerate() {
        let Ok((mut light, mut transform, mut visibility)) = lights.get_mut(entity) else {
            continue;
        };
        match lit.get(i) {
            Some(lit) => {
                let up = lit.at.normalize_or(Vec3::Y);
                let (waver, bob) = flicker(t, i as f32 * 1.7, lit.flickers);
                light.intensity = lit.lumens * waver;
                light.range = lit.range;
                light.color = lit.color;
                transform.translation = lit.at + up * (lit.lift + bob * lit.lift / 1.5);
                *visibility = Visibility::Inherited;
            }
            None => {
                light.intensity = 0.0;
                *visibility = Visibility::Hidden;
            }
        }
    }
}

/// The lights the fires want, the brightest first after the jets' and the hearths'.
fn lit(blazes: &Blazes) -> Vec<Lit> {
    let mut cells: std::collections::BTreeMap<[i64; 3], (Vec3, f32)> =
        std::collections::BTreeMap::new();
    let hearths = blazes.kept.len().min(HEARTHS);
    for blaze in blazes.all().skip(hearths) {
        let weight = blaze.tall * blaze.heat;
        let key = (blaze.at / LIGHT_CELL_M).floor().as_i64vec3().to_array();
        let cell = cells.entry(key).or_insert((Vec3::ZERO, 0.0));
        cell.0 += blaze.at * weight;
        cell.1 += weight;
    }
    // Each light: where, how many lumens, how far it reaches, how high over where it is it
    // hangs, and how big a fire it flickers as.
    let mut lit: Vec<Lit> = blazes
        .jets
        .iter()
        .filter(|jet| jet.on)
        .flat_map(|jet| {
            let way = jet.way.normalize_or(Vec3::Z);
            [0.06, 0.4, 0.85].map(|along| Lit {
                at: jet.from + way * jet.reach * along,
                lumens: JET_LUMENS,
                color: JET_LIGHT,
                range: JET_RANGE_M,
                lift: 1.5,
                flickers: JET_LUMENS,
            })
        })
        .collect();
    // A hearth's light hangs in its flames just clear of its logs' ends, so it lights the
    // ground round it and the faces of those sat by it, and flickers as its tongues do: level
    // with the ends (0.6 of its height) they were the brightest thing by it (lake film r6).
    lit.extend(blazes.kept.iter().take(hearths).map(|blaze| {
        let weight = blaze.tall * blaze.heat;
        Lit {
            at: blaze.at,
            lumens: weight * HEARTH_LUMENS_A_METRE,
            color: HEARTH_LIGHT,
            range: HEARTH_RANGE_M,
            lift: blaze.tall * 0.8,
            flickers: weight * LUMENS_A_METRE * 0.25,
        }
    }));
    // Each gathering's light hangs out from the middle of all that burns, on the outside of
    // the walls its flames stand on: at the gathering's own middle, inside a burning building,
    // it lit the inner faces and none of the stone the eye sees (castle film r8).
    let (sum, weight) = cells
        .values()
        .fold((Vec3::ZERO, 0.0), |(s, w), (cs, cw)| (s + *cs, w + cw));
    let middle = if weight > 0.0 {
        sum / weight
    } else {
        Vec3::ZERO
    };
    let outward = |at: Vec3| {
        let up = at.normalize_or(Vec3::Y);
        let away = at - middle;
        let flat = away - up * away.dot(up);
        if flat.length() > 1.0 {
            flat.normalize() * LIGHT_OUT_M
        } else {
            Vec3::ZERO
        }
    };
    let mut gathered: Vec<Lit> = cells
        .into_values()
        .filter(|(_, weight)| *weight > 0.0)
        .map(|(sum, weight)| {
            let lumens = (weight * LUMENS_A_METRE).min(MOST_LUMENS);
            Lit {
                at: sum / weight + outward(sum / weight),
                lumens,
                color: BLAZE_LIGHT,
                range: LIGHT_RANGE_M,
                lift: 1.5,
                flickers: lumens,
            }
        })
        .collect();
    gathered.sort_by(|a, b| b.lumens.total_cmp(&a.lumens));
    lit.extend(gathered);
    lit
}

mod seen;

#[cfg(test)]
mod tests;
