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
//! heat haze over the flames; and a few point lights where the fire is greatest (`light`),
//! each fire flickering by one signal that its light, flames, embers and char all follow.
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
pub use exposure::{FireExposure, LEANING_WEIGHT};

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
/// How many jets are drawn.
const JETS: usize = 2;
/// A breath's flame sprites, as Monster Hunter World draws Zeno Jiva's (docs/fire.md,
/// "Composites"): puffs of burning fuel along its stream, each its own age, the fireball
/// rolling where it strikes and the blast when it first does, round the hot rod its hull
/// marches. Must match `BREATH_CARDS` in fire.wesl.
const BREATH_CARDS: usize = 150;
/// What a breath leaves in the air, as a flamethrower's stream does (research/
/// 2026-10-18-dragon-breath.md): black soot rolling off it and burning drops falling from it.
/// Each is let go where the stream is when it is born and then moves on its own, so it stays
/// where it was breathed as the breather flies on: how many are kept, and how many a second
/// a breath lets go.
const SOOT_PUFFS: usize = 320;
const DROPS: usize = 112;
const SOOT_A_SECOND: f32 = 55.0;
const DROPS_A_SECOND: f32 = 45.0;
/// How many burning drops a blast throws out where the stream first strikes, as debris.
const DEBRIS: usize = 36;
/// How fast a breath's stream flies from the jaw, metres a second; must match `JET_SPEED`
/// in fire.wesl, which says why.
const JET_SPEED_MPS: f32 = 24.0;
/// The least a fire's smoke drifts, metres a second.
const LEAST_DRAUGHT_MPS: f32 = 1.6;

/// What a fire's material is told. Field order must match `fire.wesl`.
#[derive(Clone, Copy, Debug, Default, Reflect, bevy::render::render_resource::ShaderType)]
pub struct FireUniform {
    /// x which cards (0 flames, 1 embers, 2 jets' rods, 3 haze, 4 drops, 5 jets' sprites), y
    /// how many slots are filled, z the time the cards move by, w how many of the first slots
    /// are kept fires.
    kind: Vec4,
    /// The wind where the fire is, metres a second in the world, and its strength.
    wind: Vec4,
    /// Each jet: from, and its reach; its way, and its spread; whether it is on (1), since
    /// when it is on or off, and when it last came on.
    jets: [Vec4; JETS * 3],
    /// The exposure weight and the dusk it is drawn as tuned at (`FireExposure::packed`).
    exposure: Vec4,
    /// Each flipbook set's rate, metres, motion scale and whether it blends along its vectors
    /// (`books::Sidecar::told`), in the order of `books::SETS`.
    sets: [Vec4; books::SETS.len()],
    /// x the colour ramp's scale, y its rows.
    ramp: Vec4,
    /// How high up its frame each set's flame tips reach (`books::Sidecar::reach`).
    reach: Vec4,
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
    /// The flames' flipbooks (`books`): a layer a set, its heat, cover, soot and distortion.
    #[texture(102, dimension = "2d_array")]
    #[sampler(103)]
    flames: Handle<Image>,
    /// The breaths' soot and drops (`Puffs`).
    #[storage(104, read_only)]
    puffs: Handle<ShaderBuffer>,
    /// The exposure the eye is adapted to (`adaptation`), so a flame's hottest cores clip
    /// white on the screen whatever the adaptation does.
    #[storage(105, read_only)]
    adaptation: Handle<ShaderBuffer>,
    /// Each set's motion to its next frame, at half size, loaded unfiltered.
    #[texture(106, dimension = "2d_array")]
    motions: Handle<Image>,
    /// The colour ramp every flame's heat is coloured through.
    #[texture(107)]
    #[sampler(108)]
    ramp: Handle<Image>,
}

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
    /// go by, w how many of the first slots are kept fires.
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
    fire: [Handle<FireMaterial>; 6],
    smoke: [Handle<FireSmokeMaterial>; 2],
    puffs: Puffs,
    lights: Vec<Entity>,
    /// Whether each jet is on, since when it is on or off, and when it last came on, by the
    /// cards' clock.
    jets: [(bool, f32, f32); JETS],
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

    /// Throws a blast's debris out where a stream struck, `at`, the world's up there `up`:
    /// burning drops flung out and up, falling as the stream's do.
    fn throw(&mut self, at: Vec3, up: Vec3, now: f32) {
        let side = up.any_orthonormal_vector();
        let other = up.cross(side);
        for _ in 0..DEBRIS {
            let a = self.roll() * core::f32::consts::TAU;
            let rise = 0.2 + 0.8 * self.roll();
            let speed = 7.0 + 9.0 * self.roll();
            let out = (side * libm::cosf(a) + other * libm::sinf(a)) * (1.0 - rise * 0.6);
            let going = (out + up * rise).normalize_or(up) * speed;
            let luck = self.roll();
            let i = SOOT_PUFFS + self.next[1];
            self.next[1] = (self.next[1] + 1) % DROPS;
            self.data[i * 2] = at.extend(now).to_array();
            self.data[i * 2 + 1] = going.extend(luck).to_array();
        }
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
    light::plugin(app);
    fog::plugin(app);
    app.init_resource::<Blazes>()
        .add_plugins((
            MaterialPlugin::<FireMaterial>::default(),
            MaterialPlugin::<FireSmokeMaterial>::default(),
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, scorch)
        .add_systems(PostUpdate, (keep, blow, show, light::light).chain());
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
    fire: &[Handle<FireMaterial>; 6],
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
    spawn(
        "Fire: jets' sprites",
        cards(JETS, BREATH_CARDS, 0.0),
        fire[5].clone(),
    );
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
    mut images: ResMut<Assets<Image>>,
    adaptation: Res<Adaptation>,
    mut fires: ResMut<Assets<FireMaterial>>,
    mut smokes: ResMut<Assets<FireSmokeMaterial>>,
) {
    let count = slots(*quality);
    let slots = buffers.add(ShaderBuffer::from(vec![[0.0f32; 4]; count * 2]));
    let books = books::read();
    let flames = images.add(books.flames);
    let motions = images.add(books.motions);
    let ramp = images.add(books.ramp);
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
    let fire = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0].map(|kind| {
        // The haze reads the opaque scene as Bevy copies it for transmissive materials, so it
        // is drawn among them, blended over it.
        let haze = kind == 3.0;
        fires.add(FireMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                // Flames are light over what they cover, premultiplied (docs/fire.md, "The
                // colour ramp"): `ONE, ONE_MINUS_SRC_ALPHA`.
                alpha_mode: if haze {
                    AlphaMode::Blend
                } else {
                    AlphaMode::Premultiplied
                },
                specular_transmission: if haze { 1.0 } else { 0.0 },
                unlit: true,
                // Drawn after the smoke, which is sorted with it by the same middle: smoke
                // drawn over its own flames greyed and dimmed their yellow hearts to olive
                // (castle film r5 and r6). A fire's flames show through the smoke over it.
                // A breath is drawn after the flames too, as it is opaque and stands nearer
                // the eye than the walls it strikes: tied with them, a burning tower's flame
                // cards behind its fireball were drawn over it, a lighter patch with their
                // straight edges, and smoke behind it browned it (castle film r11). Its
                // sprites go first, its hot rod last, so the rod shows through their soot.
                depth_bias: if haze {
                    0.0
                } else if kind == 2.0 {
                    2.5
                } else if kind == 5.0 {
                    2.0
                } else {
                    1.0
                },
                // A breath's hull is drawn from its far side, so it shows with the eye in it;
                // its sprites, turned any way, from both.
                cull_mode: if kind == 2.0 {
                    Some(Face::Front)
                } else if kind == 5.0 {
                    None
                } else {
                    Some(Face::Back)
                },
                ..default()
            },
            extension: FireExtension {
                uniform: FireUniform {
                    kind: Vec4::new(kind, 0.0, 0.0, 0.0),
                    sets: books.sets,
                    ramp: books.ramp_scale,
                    reach: books.reach,
                    ..default()
                },
                volume: kind == 2.0,
                slots: slots.clone(),
                flames: flames.clone(),
                motions: motions.clone(),
                ramp: ramp.clone(),
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
    let lights = (0..light::LIGHTS)
        .map(|i| {
            commands
                .spawn((
                    Name::new(format!("Fire: light {i}")),
                    PointLight {
                        color: Color::WHITE,
                        intensity: 0.0,
                        range: 1.0,
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
        jets: [(false, -100.0, -100.0); JETS],
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
    realm: Option<Res<FireExposure>>,
) {
    let now = time.elapsed_secs_wrapped();
    let exposure = realm.as_deref().copied().unwrap_or_default().packed();
    let count = blazes.all().count().min(slots(*quality));
    // The kept fires come first, told how many so they stand just where they are.
    let kept = blazes.kept.len().min(count);
    if let Some(mut buffer) = buffers.get_mut(&drawn.slots) {
        let mut data: Vec<[f32; 4]> = Vec::with_capacity(slots(*quality) * 2);
        for blaze in blazes.all().take(count) {
            data.push([blaze.at.x, blaze.at.y, blaze.at.z, blaze.tall]);
            // Its flicker now, which its flames, embers and light all burn by (`light::glow`).
            data.push([blaze.heat, blaze.seed, blaze.wide, light::glow(blaze, now)]);
        }
        data.resize(slots(*quality) * 2, [0.0; 4]);
        *buffer = ShaderBuffer::from(data);
    }
    let mut jets = [Vec4::ZERO; JETS * 3];
    let mut debris = Vec::new();
    for (i, state) in drawn.jets.iter_mut().enumerate() {
        let jet = blazes.jets.get(i).copied();
        let on = jet.is_some_and(|jet| jet.on);
        if on != state.0 {
            let came_on = if on { now } else { state.2 };
            *state = (on, now, came_on);
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
        jets[i * 3 + 2] = Vec4::new(f32::from(u8::from(state.0)), state.1, state.2, 0.0);
        // The blast where its front first strikes throws its debris.
        let struck = state.2 + jet.reach / JET_SPEED_MPS;
        if state.0 && jet.reach > 0.0 && (now - time.delta_secs()..now).contains(&struck) {
            let up = jet.from.normalize_or(Vec3::Y);
            let at = jet.from + jet.way.normalize_or(Vec3::Z) * (jet.reach - 1.0);
            debris.push((at, up));
        }
    }
    for (at, up) in debris {
        drawn.puffs.throw(at, up, now);
    }
    drawn.puffs.shed(&blazes.jets, time.delta_secs(), now);
    if let Some(mut buffer) = buffers.get_mut(&drawn.puffs.buffer) {
        *buffer = ShaderBuffer::from(drawn.puffs.data.clone());
    }
    let wind = drawn.wind.extend(drawn.wind.length());
    for (kind, handle) in drawn.fire.iter().enumerate() {
        if let Some(mut material) = fires.get_mut(handle) {
            material.extension.uniform.kind =
                Vec4::new(kind as f32, count as f32, now, kept as f32);
            material.extension.uniform.wind = wind;
            material.extension.uniform.jets = jets;
            material.extension.uniform.exposure = exposure;
        }
    }
    for (kind, handle) in drawn.smoke.iter().enumerate() {
        if let Some(mut material) = smokes.get_mut(handle) {
            material.extension.uniform.wind = drawn.wind.extend(count as f32);
            material.extension.uniform.kind = Vec4::new(kind as f32, now, 0.0, kept as f32);
        }
    }
}

mod books;
mod fog;
mod light;
mod seen;

pub use light::{FireGlows, GLOWS};

mod exposure;
#[cfg(test)]
mod tests;
