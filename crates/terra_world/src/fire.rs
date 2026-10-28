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
//! and a few point lights where the fire is greatest (`light`),
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
    pbr::ExtendedMaterial,
    prelude::*,
    render::{render_resource::Face, storage::ShaderBuffer},
};
pub use exposure::{FireExposure, LEANING_WEIGHT};
pub use hearth::{Body, Hung, Logs};

/// Something burning, as the layer above says it: where its flames stand from (their foot,
/// in the world), how tall they are in metres, how hot (0 to 1, a dying fire's embers to a
/// blaze's heart), and a number of its own so no two flicker alike.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blaze {
    pub at: Vec3,
    pub tall: f32,
    pub heat: f32,
    pub seed: f32,
    /// Its flame's width, metres: nought for as wide as a flame so tall is, more for a sheet.
    pub wide: f32,
    /// Which way what burns faces the open air, in the world, a unit vector: a burning wall's
    /// face, out of it; zero where no side is (a fire in the open, a hearth). Its light hangs
    /// out that way (`light`).
    pub out: Vec3,
}

/// A jet of flame breathed out: from where, which way, how far it reaches, how wide it
/// spreads (the tangent of its half angle), and whether it is on; and what it strikes: how
/// far off the face it meets is and which way that face looks, in the world (`face` 0 and
/// `normal` zero when it meets nothing), so its fire spreads over that face, not into it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jet {
    pub from: Vec3,
    pub way: Vec3,
    pub reach: f32,
    pub spread: f32,
    pub on: bool,
    pub face: f32,
    pub normal: Vec3,
}

/// No jet: nothing breathed in its slot yet.
const NO_JET: Jet = Jet {
    from: Vec3::ZERO,
    way: Vec3::Z,
    reach: 0.0,
    spread: 0.0,
    on: false,
    face: 0.0,
    normal: Vec3::ZERO,
};

/// Everything burning now, set by whatever owns the fires each frame: the blazes, the
/// largest first (those past the preset's slots are not drawn), and the jets; and the fires
/// kept where something [`Kindled`] stands, gathered here (`keep`), drawn first.
#[derive(Resource, Default, Debug)]
pub struct Blazes {
    pub blazes: Vec<Blaze>,
    pub jets: Vec<Jet>,
    kept: Vec<Blaze>,
    /// In `kept`'s order: their logs and bodies as the shader reads them (`Logs::packed`,
    /// `Body::told`), their lights (`Kindled::light`, `rig`) and where they hang (`Hung`).
    hearths: Vec<[Vec4; 2]>,
    bodies: Vec<f32>,
    lights: Vec<Color>,
    rigs: Vec<Rig>,
    hangs: Vec<Vec3>,
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
    /// The logs it burns on, which its flames lick up along (`hearth::Logs`).
    pub logs: Logs,
    /// How its flame body is drawn, its kind's choice (`hearth::Body`).
    pub body: Body,
    /// Its light's colour, its fuel's (wood's `HEARTH_LIGHT`, pitch's `TORCH_LIGHT`), and the
    /// lights it is lit by, its kind's (a hearth's, or a torch's short pool, `Rig`).
    pub light: Color,
    pub rig: Rig,
}

/// How many blazes are drawn at most, by preset.
pub fn slots(quality: Quality) -> usize {
    match quality {
        Quality::Low => 384,
        Quality::Medium => 1024,
        Quality::High | Quality::Ultra => 2048,
    }
}

/// How many cards a blaze has of each: flames, embers and smoke. A heat haze card, the
/// opaque scene drawn shimmering over the flames, was dropped: too faint to see, all it showed
/// was a grey smudge round a campfire's tips (fire round 8).
const FLAME_CARDS: usize = 3;
const EMBER_CARDS: usize = 5;
/// A blaze's smoke is so many overlapping billows let go close behind one another, so its
/// column is one smoke (at four it was a file of separate puffs, castle round 5); must match
/// `BLAZE_PUFFS` in fire_smoke.wesl.
const SMOKE_CARDS: usize = 8;
/// The fires kept where something kindled stands, a campfire or a hearth, are drawn as a
/// campfire is in footage and in the games that draw one well (The Witcher 3, Red Dead
/// Redemption 2, Kingdom Come): several tongues of flame of their own heights over a bed of
/// glowing coals among the logs, a few sparks, a thin grey wisp of smoke warm only at its
/// foot, and a light of their own. At most so many of them; must match `HEARTHS` in
/// fire.wesl and fire_smoke.wesl. Each has so many flame cards (its ground, coals, body's
/// columns, tongues on the logs, glow inside its stack and round its flames; `HEARTH_CARDS`
/// in fire.wesl) and so many puffs of smoke, close enough in time to rise as one column.
const HEARTHS: usize = 16;
const HEARTH_FLAME_CARDS: usize = 44;
const HEARTH_SMOKE_CARDS: usize = 72;
/// A hearth's sparks, thirty or more seen at once, most dying low (fire review, round 2:
/// 30 to 60 small points, not a file of a few streaks).
const HEARTH_EMBER_CARDS: usize = 48;
/// How many jets are drawn.
const JETS: usize = 2;
/// A breath's flame sprites, as Monster Hunter World draws Zeno Jiva's (docs/fire.md,
/// "Composites"): puffs of burning fuel along its stream, each its own age, the fireball
/// rolling where it strikes, the blast when it first does and the soot it leaves on what it
/// strikes, round the hot rod its hull marches. Must match `BREATH_CARDS` in fire.wesl.
const BREATH_CARDS: usize = 46;
/// What a breath leaves in the air, as a flamethrower's stream does (research/
/// 2026-10-18-dragon-breath.md): black soot rolling off it and burning drops falling from it.
/// Each is let go where the stream is when it is born and then moves on its own, so it stays
/// where it was breathed as the breather flies on: how many are kept, and how many a second
/// a breath lets go.
const SOOT_PUFFS: usize = 320;
const DROPS: usize = 112;
const SOOT_A_SECOND: f32 = 10.0;
const DROPS_A_SECOND: f32 = 45.0;
/// How many burning drops a blast throws out where the stream first strikes, as debris.
const DEBRIS: usize = 36;
/// How fast a breath's stream flies from the jaw, metres a second; must match `JET_SPEED`
/// in fire.wesl, which says why.
const JET_SPEED_MPS: f32 = 24.0;
/// The least a fire's smoke drifts, metres a second.
const LEAST_DRAUGHT_MPS: f32 = 1.6;
/// How fast the flames move against the clock (D561): every flame's loop, noise and tongues, the
/// wisps they shed, their light's flicker and sway, the haze over them and the crawl of the
/// embers on what burns, so they stay in step; sparks, smoke and the jets keep the clock's. The
/// developers picked 0.65 from a film of 1, 0.8, 0.65 and 0.5 (desk q-fire-pace). Must match
/// `FLAME_PACE` in fire.wesl and `FLAME_PACE` in tools/fire/motion.ts.
pub const PACE: f32 = 0.65;

/// The time the flames move by at `now` on the cards' clock (`Time`'s): `PACE` of it.
fn flame_time(now: f32) -> f32 {
    now * PACE
}

/// The fire materials: one a kind of card (the shader's `kind`), then the hearths' mask.
const KINDS: usize = 9;
/// The hearths' and torches' flames drawn again, into the picture's alpha only (`film`).
const MASK: usize = 6;
/// The breaths' hulls drawn again, into the alpha only: a marched breath's flame through the
/// film as a hearth's (`volume`).
const JET_MASK: usize = 7;
/// A marched breath's embers once off, drawn as the breath's sprites (5) but after its hull,
/// so its smoke does not cover them: as Niagara's sparks are their own emitter, sorted apart
/// from the volume they rise from. Told by the fraction of its kind (`fire.wesl`'s
/// `breath_ember`). Must match `BREATH_EMBER_CARDS` in fire.wesl.
const JET_EMBERS: usize = 8;
const BREATH_EMBER_CARDS: usize = 160;

/// Where a fire material's draw is sorted among the transparent ones (StandardMaterial's
/// `depth_bias`: the higher, the later; every card mesh stands at the origin, so this alone
/// orders them). Drawn after the smoke, which is sorted with it by the same middle: smoke
/// drawn over its own flames greyed and dimmed their yellow hearts to olive (castle film r5
/// and r6). A fire's flames show through the smoke over it. A breath is drawn after the
/// flames too, as it is opaque and stands nearer the eye than the walls it strikes: tied with
/// them, a burning tower's flame cards behind its fireball were drawn over it, a lighter patch
/// with their straight edges, and smoke behind it browned it (castle film r11). Its sprites go
/// first, its hot rod last, so the rod shows through their soot. A mask after what it masks:
/// tied with the hearth's flames, the hearths' mask was drawn first, and a flame card blended
/// over it took the share back as it covered (`ONE_MINUS_SRC_ALPHA`), so where the campfire
/// burned hottest it went through the camera's curve as the world and read pink-white (matte
/// 0 under its brightest streaks, campfire halo round).
fn drawn_bias(material: usize) -> f32 {
    let kind = drawn_kind(material);
    if material == JET_MASK {
        3.0
    } else if material == JET_EMBERS {
        2.75
    } else if kind == 2.0 {
        2.5
    } else if kind == 5.0 {
        2.0
    } else if material == MASK {
        1.5
    } else {
        1.0
    }
}

/// The kind of card a fire material's shader draws: the mask draws the flames' (0).
fn drawn_kind(material: usize) -> f32 {
    if material == JET_MASK {
        2.0
    } else if material == MASK {
        0.0
    } else if material == JET_EMBERS {
        5.25
    } else {
        material as f32
    }
}

use smoke::{FireSmokeExtension, FireSmokeUniform};

pub type FireSmokeMaterial = ExtendedMaterial<StandardMaterial, FireSmokeExtension>;

/// The fires' drawing: their buffer, their materials, and their lights.
#[derive(Resource)]
struct Drawn {
    slots: Handle<ShaderBuffer>,
    /// The haze in front of the cards (`fogged`).
    haze: Handle<ShaderBuffer>,
    fire: [Handle<FireMaterial>; KINDS],
    /// A material for each of `fogged::SMOKES`.
    smoke: [Handle<FireSmokeMaterial>; fogged::SMOKES.len()],
    puffs: Puffs,
    lights: Vec<Entity>,
    /// Whether each jet is on, since when it is on or off, and when it last came on, by the
    /// cards' clock.
    jets: [(bool, f32, f32); JETS],
    /// Each jet as it last burned: once it is off, or its breather gone, it is drawn from
    /// there, so the last of its fire burns out where it was and the soot it left on the face
    /// it struck stays on that face (`breath_stain`), not following the jaw away.
    kept: [Jet; JETS],
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
                        // Soot rises only off where it strikes, a few big puffs a second, as
                        // shipped flamethrowers' smoke is: off its last third at 30 a second,
                        // a sooty cloud hung over the stream (breath r29's review).
                        (0.95 + 0.05 * self.roll(), 1.0, 9.0)
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
    fogged::plugin(app);
    order::plugin(app);
    haze::plugin(app);
    film::plugin(app);
    matte::plugin(app);
    glow::plugin(app);
    volume::plugin(app);
    app.init_resource::<Blazes>()
        .add_plugins((
            MaterialPlugin::<FireMaterial>::default(),
            MaterialPlugin::<FireSmokeMaterial>::default(),
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, scorch)
        .add_systems(
            PostUpdate,
            (hearth::keep, blow, show, sim::tell, light::light).chain(),
        );
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

/// The cards of every kind, each a mesh of its own drawn wherever the fires are.
fn spawn_cards(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    count: usize,
    fire: &[Handle<FireMaterial>; KINDS],
    smoke: &[Handle<FireSmokeMaterial>; fogged::SMOKES.len()],
) {
    // Drawn wherever the fires are: the cards are placed on the GPU, so no bounds would hold.
    // All stand at the origin, so their sort ties are broken by their rank, in this order.
    let mut rank = 0..;
    let mut spawn = |name: &str, mesh: Mesh, material: Handle<FireMaterial>| {
        commands.spawn((
            Name::new(name.to_string()),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            NoFrustumCulling,
            NotShadowCaster,
            fogged::AfterFog,
            order::Rank(rank.next().unwrap()),
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
        "Fire: hearth flames' mask",
        cards(HEARTHS.min(count), HEARTH_FLAME_CARDS, 1.0),
        fire[MASK].clone(),
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
    spawn("Fire: jets", volume::hulls(JETS, 0.0), fire[2].clone());
    spawn(
        "Fire: jets' mask",
        volume::hulls(JETS, 0.0),
        fire[JET_MASK].clone(),
    );
    // Each hearth's flame body marched as one field (D466, docs/fire-module.md).
    spawn(
        "Fire: hearth fields",
        volume::hulls(HEARTHS.min(count), 1.0),
        fire[2].clone(),
    );
    spawn(
        "Fire: hearth fields' mask",
        volume::hulls(HEARTHS.min(count), 1.0),
        fire[JET_MASK].clone(),
    );
    spawn(
        "Fire: jets' sprites",
        cards(JETS, BREATH_CARDS, 0.0),
        fire[5].clone(),
    );
    spawn(
        "Fire: jets' embers",
        cards(JETS, BREATH_EMBER_CARDS, 0.0),
        fire[JET_EMBERS].clone(),
    );
    spawn("Fire: drops", cards(DROPS, 1, 0.0), fire[4].clone());
    // The torches' smoke is the hearths' cards again under its own material, each mesh
    // drawing its own hearths' (`fogged::SMOKES`).
    for (name, mesh, which) in [
        ("Fire: smoke", cards(count, SMOKE_CARDS, 0.0), 0),
        (
            "Fire: hearth smoke",
            cards(HEARTHS.min(count), HEARTH_SMOKE_CARDS, 1.0),
            0,
        ),
        ("Fire: soot", cards(SOOT_PUFFS, 1, 0.0), 1),
        (
            "Fire: torch smoke",
            cards(HEARTHS.min(count), HEARTH_SMOKE_CARDS, 1.0),
            2,
        ),
    ] {
        let mut card = commands.spawn((
            Name::new(name),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(smoke[which].clone()),
            Transform::IDENTITY,
            NoFrustumCulling,
            NotShadowCaster,
            order::Rank(rank.next().unwrap()),
        ));
        if fogged::SMOKES[which].after_fog() {
            card.insert(fogged::AfterFog);
        }
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
    let smoke_books = books::read_smoke();
    let near = images.add(smoke_books.near);
    let far = images.add(smoke_books.far);
    let flames = images.add(books.flames);
    let motions = images.add(books.motions);
    let ramp = images.add(books.ramp);
    let noise = images.add(volume::noise());
    let sim = images.add(sim::unread());
    // Nothing let go yet: born an hour ago, long gone.
    let haze = buffers.add(ShaderBuffer::from(vec![[0.0f32; 4]; fogged::HAZE_VEC4S]));
    let puffs = Puffs {
        buffer: buffers.add(ShaderBuffer::from(
            [[0.0f32, 0.0, 0.0, -3600.0], [0.0; 4]].repeat(SOOT_PUFFS + DROPS),
        )),
        data: [[0.0f32, 0.0, 0.0, -3600.0], [0.0; 4]].repeat(SOOT_PUFFS + DROPS),
        next: [0, 0],
        owed: [0.0, 0.0],
        luck: 0,
    };
    let fire = std::array::from_fn::<_, KINDS, _>(|i| {
        let kind = drawn_kind(i);
        fires.add(FireMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                // Flames are light over what they cover, premultiplied (docs/fire.md, "The
                // colour ramp"): `ONE, ONE_MINUS_SRC_ALPHA`.
                alpha_mode: AlphaMode::Premultiplied,
                unlit: true,
                depth_bias: drawn_bias(i),
                // A breath's hull is drawn from its far side, so it shows with the eye in it;
                // its sprites, turned any way, from both.
                cull_mode: if kind == 2.0 {
                    Some(Face::Front)
                } else if kind == 5.0 || i == JET_EMBERS {
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
                mask: i == MASK || i == JET_MASK,
                depth: kind == 5.0 && i != MASK,
                slots: slots.clone(),
                flames: flames.clone(),
                motions: motions.clone(),
                ramp: ramp.clone(),
                noise: noise.clone(),
                sim: sim.clone(),
                puffs: puffs.buffer.clone(),
                adaptation: adaptation.0.clone(),
                haze: haze.clone(),
                haze_density: None,
            },
        })
    });
    let smoke = fogged::SMOKES.map(|which| {
        let kind = which.kind;
        smokes.add(FireSmokeMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 1.0,
                reflectance: 0.0,
                ..default()
            },
            extension: FireSmokeExtension {
                uniform: FireSmokeUniform {
                    kind: Vec4::new(kind, 0.0, if which.torches { 1.0 } else { 0.0 }, 0.0),
                    sets: smoke_books.light_scales,
                    ..default()
                },
                slots: slots.clone(),
                puffs: puffs.buffer.clone(),
                near: near.clone(),
                far: far.clone(),
                haze: haze.clone(),
                haze_density: None,
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
        haze,
        fire,
        smoke,
        puffs,
        lights,
        jets: [(false, -100.0, -100.0); JETS],
        kept: [NO_JET; JETS],
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
    (open_sky, indoor_sky): (
        Option<Res<crate::skylight::OpenSky>>,
        Option<Res<crate::scene::IndoorSky>>,
    ),
    environments: Query<&bevy::light::EnvironmentMapLight, With<Camera3d>>,
    tier: Res<volume::BreathTier>,
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
            data.push([
                blaze.heat,
                blaze.seed,
                blaze.wide,
                light::glow(blaze, flame_time(now)),
            ]);
        }
        data.resize(slots(*quality) * 2, [0.0; 4]);
        *buffer = ShaderBuffer::from(data);
    }
    let mut jets = [Vec4::ZERO; JETS * 4];
    let mut debris = Vec::new();
    for i in 0..JETS {
        let live = blazes.jets.get(i).copied().filter(|jet| jet.on);
        let state = &mut drawn.jets[i];
        let on = live.is_some();
        if on != state.0 {
            let came_on = if on { now } else { state.2 };
            *state = (on, now, came_on);
        }
        let state = *state;
        if let Some(jet) = live {
            drawn.kept[i] = jet;
        }
        let jet = Jet {
            on,
            ..drawn.kept[i]
        };
        jets[i * 4] = jet.from.extend(jet.reach);
        jets[i * 4 + 1] = jet.way.normalize_or(Vec3::Z).extend(jet.spread);
        jets[i * 4 + 2] = Vec4::new(f32::from(u8::from(state.0)), state.1, state.2, 0.0);
        jets[i * 4 + 3] = jet.normal.normalize_or_zero().extend(jet.face);
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
                Vec4::new(drawn_kind(kind), count as f32, now, kept as f32);
            material.extension.uniform.wind = wind;
            material.extension.uniform.jets = jets;
            material.extension.uniform.exposure = exposure;
            material.extension.uniform.volume = tier.packed();
            for (i, logs) in blazes.hearths.iter().take(HEARTHS).enumerate() {
                material.extension.uniform.hearths[i * 2] = logs[0];
                material.extension.uniform.hearths[i * 2 + 1] = logs[1];
            }
            for (i, body) in blazes.bodies.iter().take(HEARTHS).enumerate() {
                material.extension.uniform.bodies[i / 4][i % 4] = *body;
            }
        }
    }
    // The smoke is lit by the open sky round it as the shading's light gives it, in a room
    // as far as its openings let it in (D485).
    let air_lit = indoor_sky.map_or(1.0, |s| s.0);
    let (sky, sky_frame) = smoke::sky(open_sky.as_deref(), environments.iter().next(), air_lit);
    for (which, handle) in fogged::SMOKES.iter().zip(&drawn.smoke) {
        if let Some(mut material) = smokes.get_mut(handle) {
            material.extension.uniform.wind = drawn.wind.extend(count as f32);
            let torches = if which.torches { 1.0 } else { 0.0 };
            material.extension.uniform.kind = Vec4::new(which.kind, now, torches, kept as f32);
            material.extension.uniform.sky = sky;
            material.extension.uniform.sky_frame = sky_frame;
        }
    }
}

mod books;
#[cfg(test)]
mod burnout;
mod film;
mod fog;
mod fogged;
mod glow;
mod haze;
mod hearth;
mod light;
mod material;
mod matte;
mod order;
mod seen;
mod sim;
mod smoke;
mod volume;
pub use matte::with_matte;
pub use volume::BreathTier;

pub use glow::{Glow, GlowMaterial};
pub use light::{FireGlows, GLOWS, HEARTH_LIGHT, Rig, TORCH_LIGHT};
pub use material::{FireExtension, FireKey, FireMaterial, FireUniform};

mod exposure;
#[cfg(test)]
mod tests;
