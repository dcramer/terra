//! The kit's effects (D634): a biome's `effects` name presets, each a file in
//! `spike/iso/biomes/effects/<name>.ron` (`Preset`), drawn by one shader (`effects.wesl`) on
//! a fixed pool of quads and lights made at startup, nothing spawned at run time.
//!
//! How the shipped ARPGs sell an effect, and what each part ports: a hit is a white-hot core
//! over bloom inside a tinted halo, a short-lived light that lights the ground and the
//! bodies round it (the light is what makes it read as real, not a sprite), sparks as
//! camera-facing streaks stretched along their motion (a shutter's length of their path,
//! Unreal's Niagara velocity-aligned sprites), and a puff of smoke lit by the lights at it
//! (Diablo IV's and Path of Exile 2's impacts: flash, light, sparks, dust). Smoke and steam
//! are soft particles (Lorach, "Soft Particles", NVIDIA 2007: alpha faded by the distance
//! to the depth behind), so they cut no hard line into the floor. A slash (slash.rs's
//! strip along the blade's own path) is a lit motion trail: a thin hot edge, a streaked
//! smear inside it fading back from the blade, a light riding it.
//!
//! A preset's light dies with it (`lamp_lm`); a biome's effects hold to `QUADS` and
//! `LIGHTS` (`Budget`, printed at startup); an unknown name stops the run naming the fix.

use super::biome::Biome;
use super::light::{Colour, Light};
use crate::forest::Rng;
use crate::ruin::effects::Stage;
use crate::ruin::effects::kit::{Cam, Clock};
use bevy::asset::embedded_asset;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use serde::Deserialize;

/// The most quads a biome's effects may hold, and lights (the slash's own counted).
pub const QUADS: usize = 400;
pub const LIGHTS: usize = 8;

/// The staged fight's effects, played where no biome names its own (the glade's view).
pub const FIGHT: [&str; 5] = ["slash", "hit-spark", "smoke-puff", "spell-burst", "bolt"];

/// One effect, as its file says.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    /// What sets it off: the knight's blow landing, a bolt striking, the swing itself
    /// (slash.rs draws it), or a loop of so many seconds (steam, embers).
    pub cue: Cue,
    /// Where it plays when the biome gives no place.
    pub at: Anchor,
    /// Its age in a still (s): the moment the still shows.
    pub still: f32,
    /// How long it lasts (s); nothing of it shows or lights past this.
    pub life: f32,
    #[serde(default)]
    pub flash: Option<Flash>,
    #[serde(default)]
    pub lamps: Vec<Lamp>,
    #[serde(default)]
    pub sparks: Option<Sparks>,
    #[serde(default)]
    pub puffs: Option<Puffs>,
    #[serde(default)]
    pub trail: Option<Trail>,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Cue {
    Blow,
    Bolt,
    Swing,
    Loop(f32),
}

/// The stage's places (ruin/effects/mod.rs `Stage`): where the blow bites, where the bolts
/// strike the bark, the hero's off hand, the ground under the blow.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Anchor {
    Strike,
    Bark,
    Hero,
    Ground,
}

/// The core and halo: nits, the quad's size (m), the core's share of it, the halo's
/// falloff, and its swell and fade (the flash's own e-fold, s).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Flash {
    pub core: Colour,
    pub core_nits: f32,
    pub rim: Colour,
    pub rim_nits: f32,
    pub size: f32,
    pub core_size: f32,
    pub falloff: f32,
    pub swell: f32,
    pub fall: f32,
}

/// A light: lumens, colour, height over the anchor (or over the ground under it), reach (m)
/// and e-fold (s).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Lamp {
    pub lm: f32,
    pub colour: Colour,
    pub up: f32,
    #[serde(default)]
    pub ground: bool,
    pub range: f32,
    pub fall: f32,
    /// How far it stands from the anchor toward the camera, along the ground (m): a hit's
    /// light at the bite lights the faces of knight and foe turned to each other, which the
    /// camera does not see (iso-kit j43's FOUND); set out toward the camera and up, it lights
    /// the faces it does, as the ARPGs cheat theirs (see `lamp_at`).
    #[serde(default)]
    pub toward: f32,
}

/// Where a light set `toward` metres out from `base` toward the camera stands: along the
/// ground (the camera's facing laid flat), so the light's height is the preset's `up`. An
/// ARPG's hit and projectile lights are placed for the camera, not for the contact (Diablo
/// III's and Path of Exile's hit flashes light the near side of the bodies fighting; Last
/// Epoch's skills offset their lights the same way): with no shadows, a light behind the
/// bodies only rims what the camera sees.
pub fn lamp_at(base: Vec3, toward: f32, cam: Quat) -> Vec3 {
    let f = cam * Vec3::Z;
    base + Vec3::new(f.x, 0.0, f.z).normalize_or(Vec3::Z) * toward
}

/// Streaks: how many, speed (m/s) and life (s) ranges, thickness (m), the cone about the
/// way out (0 a line, 1 a sphere), gravity (m/s², negative rises), the shutter (s of path a
/// streak shows), and the colour hot at birth and cool at death (nits).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Sparks {
    pub count: usize,
    pub speed: (f32, f32),
    pub life: (f32, f32),
    pub width: f32,
    pub spread: f32,
    pub gravity: f32,
    pub shutter: f32,
    pub hot: Colour,
    pub hot_nits: f32,
    pub cool: Colour,
    pub cool_nits: f32,
    /// Where they come from about the anchor (m, a cube's half-size).
    #[serde(default)]
    pub area: f32,
    /// The time over which they are thrown (s): a spray, not one ring of streaks all the same
    /// way out (iso-kit j43: born together, a still showed a starburst).
    #[serde(default)]
    pub delay: f32,
}

/// Puffs of smoke or steam: how many, size at birth and death (m), life range (s), how
/// fast they rise and spread (m/s), their albedo and density, the delay over which they
/// are born (s), their own glow at birth (hot gas, nits) and the noise's share.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Puffs {
    pub count: usize,
    pub size: (f32, f32),
    pub life: (f32, f32),
    pub rise: f32,
    pub spread: f32,
    pub albedo: Colour,
    pub density: f32,
    pub delay: f32,
    pub glow: Colour,
    pub glow_nits: f32,
    pub grain: f32,
    /// The distance over which it fades into what is behind it (m).
    pub soft: f32,
}

/// A trail's look (slash.rs): the edge's and the smear's nits, where across the edge lies
/// (0 the hilt, 1 the tip), its width, the smear's share, and the tail's exponent.
/// A `streak` (spell.rs's bolts) is a projectile's trail instead: the hot line down its
/// middle (`width` its share of the half-width), the smear a halo about it narrowing down
/// the trail; `edge` is the slash's alone.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Trail {
    pub core: Colour,
    pub core_nits: f32,
    pub rim: Colour,
    pub rim_nits: f32,
    #[serde(default)]
    pub edge: f32,
    pub width: f32,
    pub smear: f32,
    pub tail: f32,
    pub soft: f32,
    #[serde(default)]
    pub streak: bool,
}

impl Preset {
    pub fn load(name: &str) -> Result<Preset, String> {
        let path = super::dir().join("effects").join(format!("{name}.ron"));
        if !path.exists() {
            return Err(format!(
                "effects: no preset \"{name}\" ({}). A biome's `effects` name presets in \
                 spike/iso/biomes/effects/ (D634): use one of {}, or write the file \
                 (references/kit.md in the iso-art skill, the effects section)",
                path.display(),
                names().join(", ")
            ));
        }
        super::read(&path, "effects.rs")
    }
    /// The quads and lights it holds.
    pub fn budget(&self) -> Budget {
        Budget {
            quads: self.flash.map_or(0, |_| 1)
                + self.sparks.map_or(0, |s| s.count)
                + self.puffs.map_or(0, |p| p.count)
                + self.trail.map_or(0, |_| 1),
            lights: self.lamps.len(),
        }
    }
}

/// The presets there are.
pub fn names() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(super::dir().join("effects"))
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let p = f.ok()?.path();
            (p.extension()? == "ron").then(|| p.file_stem()?.to_str().map(String::from))?
        })
        .collect();
    v.sort();
    v
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Budget {
    pub quads: usize,
    pub lights: usize,
}

/// A biome's effects: each preset by name with the place the biome gives it.
#[derive(Resource, Default, Clone)]
pub struct Effects(pub Vec<(String, Preset, Option<Vec2>)>);

impl Effects {
    /// The biome's list (the staged fight's where there is none), each preset read, and
    /// the budget held.
    pub fn load(biome: Option<&Biome>) -> Result<Effects, String> {
        let list: Vec<(String, Option<Vec2>)> = match biome {
            Some(b) => b
                .effects
                .iter()
                .map(|e| (e.preset.clone(), e.at.map(Vec2::from)))
                .collect(),
            None => FIGHT.iter().map(|n| (n.to_string(), None)).collect(),
        };
        let mut out = Vec::new();
        for (name, at) in list {
            out.push((name.clone(), Preset::load(&name)?, at));
        }
        let fx = Effects(out);
        let b = fx.budget();
        if b.quads > QUADS || b.lights > LIGHTS {
            return Err(format!(
                "effects: {} quads and {} lights, over the kit's {QUADS} and {LIGHTS} (kit/effects.rs, \
                 D634): fewer sparks or puffs, or a lamp fewer, in the presets ({})",
                b.quads,
                b.lights,
                fx.0.iter()
                    .map(|e| e.0.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        Ok(fx)
    }
    pub fn get(&self, name: &str) -> Option<&Preset> {
        self.0.iter().find(|e| e.0 == name).map(|e| &e.1)
    }
    pub fn budget(&self) -> Budget {
        self.0.iter().fold(Budget::default(), |a, e| {
            let b = e.1.budget();
            Budget {
                quads: a.quads + b.quads,
                lights: a.lights + b.lights,
            }
        })
    }
}

/// When the fight's cues last went off (the clock's time): hit.rs's blow, spell.rs's bolt.
#[derive(Resource)]
pub struct Cues {
    pub blow: f32,
    pub bolt: f32,
}

impl Default for Cues {
    fn default() -> Self {
        Cues {
            blow: -10.0,
            bolt: -10.0,
        }
    }
}

/// An effect's age at the clock: the still's chosen moment, else the time since its cue.
pub fn age(p: &Preset, clock: &Clock, cues: &Cues) -> f32 {
    if clock.still {
        return p.still;
    }
    match p.cue {
        Cue::Blow => clock.t - cues.blow,
        Cue::Bolt => clock.t - cues.bolt,
        Cue::Swing => -1.0,
        Cue::Loop(period) => clock.t.rem_euclid(period.max(0.01)),
    }
}

/// A lamp's lumens at `age`: dark before its cue and past the effect's life, else dying.
pub fn lamp_lm(l: &Lamp, age: f32, life: f32) -> f32 {
    if !(0.0..life).contains(&age) {
        return 0.0;
    }
    // Faded to nothing by the end of its life, not cut: the e-fold times a window.
    let w = 1.0 - (age / life).powi(4);
    l.lm * (-age / l.fall.max(0.001)).exp() * w
}

/// Where a streak is and how it moves `a` seconds after it left `from` at `v0`.
pub fn spark_at(from: Vec3, v0: Vec3, gravity: f32, a: f32) -> (Vec3, Vec3) {
    let g = Vec3::NEG_Y * gravity;
    (from + v0 * a + 0.5 * g * a * a, v0 + g * a)
}

/// A streak's light at `k` of its life (nits, linear rgb), as hot metal cools: `hot` at
/// birth (white-yellow), `cool` by mid-life (orange), then dimming to nothing (dark red), not
/// a straight fade from white (iso-kit j43).
pub fn spark_glow(s: &Sparks, k: f32) -> Vec3 {
    let smooth = |a: f32, b: f32, x: f32| {
        let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let c = nits(s.hot, s.hot_nits).lerp(nits(s.cool, s.cool_nits), smooth(0.0, 0.5, k));
    // The red end of the glow lasts longest: blue and green go first.
    let dark = smooth(0.4, 1.0, k);
    c * Vec3::new(1.0, 1.0 - 0.6 * dark, 1.0 - 0.8 * dark) * (1.0 - dark)
}

/// The illuminance (lux, linear rgb) the lights leave at `p`: each point light's lumens over
/// 4π d², within its reach, plus the key's and the sky's (a puff scatters half of each). A
/// puff is a volume `r` in radius, not a point: a light at or inside it lights it as one about
/// `r` away (d² + r², the mean over the puff to first order), so smoke round a hit's light
/// glows warm instead of burning out to a flat disc.
pub fn lit_at(p: Vec3, r: f32, points: &[(Vec3, Vec3, f32, f32)], light: Option<&Light>) -> Vec3 {
    let mut e = Vec3::ZERO;
    for &(at, colour, lm, range) in points {
        let d2 = (at.distance_squared(p) + r * r).max(0.09);
        if d2 < range * range && lm > 0.0 {
            e += colour * lm / (4.0 * std::f32::consts::PI * d2);
        }
    }
    if let Some(l) = light {
        let key = l.key.colour.color().to_linear();
        let sky = l.sky.colour.color().to_linear();
        e += 0.5 * l.key.lux * Vec3::new(key.red, key.green, key.blue);
        e += std::f32::consts::PI * l.sky.nits * Vec3::new(sky.red, sky.green, sky.blue);
    }
    e
}

fn nits(c: Colour, n: f32) -> Vec3 {
    let l = c.color().to_linear();
    Vec3::new(l.red, l.green, l.blue) * n
}

/// The shader's uniform (`effects.wesl`'s `Look`).
#[derive(ShaderType, Clone, Copy, Debug, Default, Reflect)]
pub struct Look {
    pub core: Vec4,
    pub rim: Vec4,
    pub shape: Vec4,
    pub lit: Vec4,
}

impl Look {
    /// A flash's core in its halo at `strength` (0 to 1): the kit's flashes, and a scene's
    /// steady glow (marsh/wisps.rs), so the quad's falloff is soft, never a disc's edge.
    pub fn flash(f: &Flash, strength: f32) -> Look {
        Look {
            core: nits(f.core, f.core_nits).extend(f.core_size),
            rim: nits(f.rim, f.rim_nits).extend(f.falloff),
            shape: Vec4::new(strength, 0.15, 0.0, 0.0),
            lit: Vec4::ZERO,
        }
    }
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct Glow {
    #[uniform(0)]
    pub look: Look,
}

impl Material for Glow {
    fn fragment_shader() -> ShaderRef {
        "embedded://iso/kit/effects.wesl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Premultiplied
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

impl Glow {
    /// A trail's material from its preset (slash.rs, spell.rs): effects.wesl's mode 2, or
    /// mode 3 for a streak.
    pub fn trail(t: &Trail, seed: f32) -> Glow {
        let mode = if t.streak { 3.0 } else { 2.0 };
        Glow {
            look: Look {
                core: nits(t.core, t.core_nits).extend(t.edge),
                rim: nits(t.rim, t.rim_nits).extend(t.width),
                shape: Vec4::new(1.0, t.soft, mode, t.smear),
                lit: Vec4::new(t.tail, 0.0, 0.0, seed),
            },
        }
    }
}

/// One of an effect's quads.
#[derive(Component)]
struct Quad {
    effect: usize,
    glow: Handle<Glow>,
    kind: Kind,
}

enum Kind {
    Flash,
    Spark {
        dir: Vec3,
        speed: f32,
        life: f32,
        from: Vec3,
        delay: f32,
        phase: f32,
    },
    Puff {
        off: Vec3,
        drift: Vec3,
        life: f32,
        delay: f32,
        phase: f32,
    },
}

/// One of an effect's lights.
#[derive(Component)]
struct Lit {
    effect: usize,
    lamp: usize,
    /// Where it stands before it is set out toward the camera (`lamp_at`, each frame).
    base: Vec3,
}

/// Each played effect's anchor and the way its sparks fly.
#[derive(Resource, Default)]
struct Places(Vec<(Vec3, Vec3)>);

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "effects.wesl");
    app.add_plugins(MaterialPlugin::<Glow>::default())
        .init_resource::<Cues>()
        .init_resource::<Places>()
        .add_systems(PreStartup, load)
        .add_systems(Startup, spawn)
        .add_systems(PostUpdate, play.before(TransformSystems::Propagate));
}

fn load(mut commands: Commands, biome: Option<Res<Biome>>) {
    let fx = Effects::load(biome.as_deref()).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    let b = fx.budget();
    println!(
        "effects: {} · {} quads of {QUADS}, {} lights of {LIGHTS}",
        fx.0.iter()
            .map(|e| e.0.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        b.quads,
        b.lights
    );
    commands.insert_resource(fx);
}

fn anchor(
    stage: &Stage,
    land: Option<&crate::terrain::Land>,
    a: Anchor,
    at: Option<Vec2>,
) -> (Vec3, Vec3) {
    let toward = (stage.foe - stage.knight).normalize_or(Vec2::X);
    let back = (stage.hero() - stage.bark()).normalize_or(Vec3::Y);
    if let Some(p) = at {
        return (stage.ground(land, p), Vec3::Y);
    }
    match a {
        Anchor::Strike => (stage.strike(), toward.extend(0.0).xzy() + Vec3::Y * 0.45),
        // Where the bolts strike (spell.rs's flights end 0.35 m short of the bark).
        Anchor::Bark => (stage.bark() + back * 0.35, back + Vec3::Y * 0.3),
        Anchor::Hero => (stage.hero(), Vec3::Y),
        Anchor::Ground => (stage.ground(land, stage.strike().xz()), Vec3::Y),
    }
}

fn spawn(
    mut commands: Commands,
    fx: Res<Effects>,
    stage: Res<Stage>,
    land: Option<Res<crate::terrain::Land>>,
    mut places: ResMut<Places>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut glows: ResMut<Assets<Glow>>,
) {
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    for (i, (name, p, at)) in fx.0.iter().enumerate() {
        let (here, out) = anchor(&stage, land.as_deref(), p.at, *at);
        places.0.push((here, out));
        let mut rng = Rng::new(0x5eed ^ (i as u32 * 7919) ^ name.len() as u32);
        let mut quad_of = |commands: &mut Commands, kind: Kind| {
            let glow = glows.add(Glow::default());
            commands.spawn((
                Mesh3d(quad.clone()),
                MeshMaterial3d(glow.clone()),
                Transform::from_scale(Vec3::ZERO),
                bevy::light::NotShadowCaster,
                Quad {
                    effect: i,
                    glow,
                    kind,
                },
            ));
        };
        // A trail's flash is its head's, drawn by its scene where the head is (spell.rs's
        // bolts), as its lamps are.
        if p.flash.is_some() && p.trail.is_none() {
            quad_of(&mut commands, Kind::Flash);
        }
        if let Some(s) = p.sparks {
            for j in 0..s.count {
                let r = Vec3::new(
                    rng.range(-1.0, 1.0),
                    rng.range(-1.0, 1.0),
                    rng.range(-1.0, 1.0),
                );
                let dir = (out.normalize() + r * s.spread).normalize_or(Vec3::Y);
                let from = here
                    + Vec3::new(
                        rng.range(-1.0, 1.0),
                        rng.range(-1.0, 1.0),
                        rng.range(-1.0, 1.0),
                    ) * s.area;
                let kind = Kind::Spark {
                    dir,
                    speed: rng.range(s.speed.0, s.speed.1),
                    life: rng.range(s.life.0, s.life.1),
                    from,
                    delay: rng.range(0.0, s.delay),
                    phase: j as f32 / s.count.max(1) as f32,
                };
                quad_of(&mut commands, kind);
            }
        }
        if let Some(f) = p.puffs {
            for j in 0..f.count {
                let off = Vec3::new(
                    rng.range(-1.0, 1.0),
                    rng.range(-0.3, 0.6),
                    rng.range(-1.0, 1.0),
                ) * f.size.0
                    * 0.6;
                let drift = Vec3::new(rng.range(-1.0, 1.0), 0.0, rng.range(-1.0, 1.0)) * f.spread;
                let kind = Kind::Puff {
                    off,
                    drift,
                    life: rng.range(f.life.0, f.life.1),
                    delay: rng.range(0.0, f.delay),
                    phase: j as f32 / f.count.max(1) as f32,
                };
                quad_of(&mut commands, kind);
            }
        }
        // A trail's lamps ride the trail its scene draws (slash.rs lights the slash's along its
        // edge): the kit spawns none for it, so no dark light is left standing.
        if p.trail.is_some() {
            continue;
        }
        for (j, l) in p.lamps.iter().enumerate() {
            let base = if l.ground {
                stage.ground(land.as_deref(), here.xz())
            } else {
                here
            };
            commands.spawn((
                PointLight {
                    color: l.colour.color(),
                    intensity: 0.0,
                    range: l.range,
                    radius: 0.1,
                    ..default()
                },
                Transform::from_translation(base + Vec3::Y * l.up),
                Lit {
                    effect: i,
                    lamp: j,
                    base: base + Vec3::Y * l.up,
                },
            ));
        }
    }
}

/// A particle's age within a looping effect: spread through the period by its phase.
fn looped(p: &Preset, age: f32, phase: f32) -> f32 {
    match p.cue {
        Cue::Loop(period) => (age + phase * period).rem_euclid(period.max(0.01)),
        _ => age,
    }
}

#[allow(clippy::too_many_arguments)]
fn play(
    fx: Res<Effects>,
    places: Res<Places>,
    clock: Res<Clock>,
    cues: Res<Cues>,
    cam: Res<Cam>,
    light: Option<Res<Light>>,
    mut lamps: Query<(&Lit, &mut PointLight, &mut Transform), Without<Quad>>,
    others: Query<(&PointLight, &GlobalTransform), Without<Lit>>,
    mut quads: Query<(&Quad, &mut Transform)>,
    mut glows: ResMut<Assets<Glow>>,
) {
    let ages: Vec<f32> = fx.0.iter().map(|e| age(&e.1, &clock, &cues)).collect();
    let mut points: Vec<(Vec3, Vec3, f32, f32)> = Vec::new();
    for (lit, mut pl, mut tr) in &mut lamps {
        let p = &fx.0[lit.effect].1;
        let l = &p.lamps[lit.lamp];
        pl.intensity = lamp_lm(l, ages[lit.effect], p.life);
        tr.translation = lamp_at(lit.base, l.toward, cam.rot);
        let c = pl.color.to_linear();
        points.push((
            tr.translation,
            Vec3::new(c.red, c.green, c.blue),
            pl.intensity,
            pl.range,
        ));
    }
    for (pl, g) in &others {
        let c = pl.color.to_linear();
        points.push((
            g.translation(),
            Vec3::new(c.red, c.green, c.blue),
            pl.intensity,
            pl.range,
        ));
    }
    let facing = cam.rot * Vec3::Z;
    for (q, mut tr) in &mut quads {
        let p = &fx.0[q.effect].1;
        let (here, _) = places.0[q.effect];
        let a = ages[q.effect];
        let mut look = Look::default();
        let mut shown = false;
        match q.kind {
            Kind::Flash => {
                let f = p.flash.unwrap();
                if (0.0..p.life).contains(&a) {
                    let k = (a / p.life).min(1.0);
                    let fade = (-a / f.fall.max(0.001)).exp() * (1.0 - k * k);
                    tr.translation = here;
                    tr.rotation = cam.rot;
                    tr.scale = Vec3::splat(
                        f.size * (1.0 + f.swell * (1.0 - (-a / f.fall.max(0.001)).exp())),
                    );
                    look = Look::flash(&f, fade);
                    shown = true;
                }
            }
            Kind::Spark {
                dir,
                speed,
                life,
                from,
                delay,
                phase,
            } => {
                let s = p.sparks.unwrap();
                let a = looped(p, a, phase) - delay;
                if (0.0..life).contains(&a) && (0.0..p.life).contains(&ages[q.effect]) {
                    let k = a / life;
                    let (at, v) = spark_at(from, dir * speed, s.gravity, a);
                    let along = v - facing * v.dot(facing);
                    let x = along.normalize_or(cam.rot * Vec3::X);
                    let y = facing.cross(x);
                    tr.translation = at - v * s.shutter * 0.5;
                    tr.rotation = Quat::from_mat3(&Mat3::from_cols(x, y, facing));
                    tr.scale = Vec3::new(
                        along.length() * s.shutter + s.width * 2.0,
                        s.width * 2.0,
                        1.0,
                    );
                    let hot = spark_glow(&s, k);
                    // A streak is all core: hot along its middle, its halo the bloom's.
                    look.core = hot.extend(0.45);
                    look.rim = (hot * 0.25).extend(3.0);
                    look.shape = Vec4::new(1.0, 0.05, 0.0, 0.0);
                    shown = true;
                }
            }
            Kind::Puff {
                off,
                drift,
                life,
                delay,
                phase,
            } => {
                let f = p.puffs.unwrap();
                let a = looped(p, a, phase) - delay;
                if (0.0..life).contains(&a) && (0.0..p.life + f.delay).contains(&(ages[q.effect])) {
                    let k = a / life;
                    let at = here + off + drift * a + Vec3::Y * f.rise * a;
                    tr.translation = at;
                    tr.rotation =
                        cam.rot * Quat::from_rotation_z(phase * std::f32::consts::TAU + a * 0.6);
                    tr.scale = Vec3::splat(
                        f.size.0 + (f.size.1 - f.size.0) * (1.0 - (1.0 - k) * (1.0 - k)),
                    );
                    let alb = nits(f.albedo, 1.0);
                    let e = lit_at(at, tr.scale.x * 0.5, &points, light.as_deref());
                    let lit = alb * e / std::f32::consts::PI
                        + nits(f.glow, f.glow_nits) * (1.0 - k).powi(3);
                    let born = (a / 0.06).min(1.0);
                    look.lit = lit.extend(phase * 17.0);
                    look.shape =
                        Vec4::new(f.density * born * (1.0 - k).powf(1.5), f.soft, 1.0, f.grain);
                    shown = true;
                }
            }
        }
        if !shown {
            tr.scale = Vec3::ZERO;
            continue;
        }
        if let Some(mut g) = glows.get_mut(&q.glow) {
            g.look = look;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lamp() -> Lamp {
        Lamp {
            lm: 1000.0,
            colour: Colour::Srgb(1.0, 1.0, 1.0),
            up: 0.5,
            ground: true,
            range: 3.0,
            fall: 0.1,
            toward: 0.0,
        }
    }

    /// A hit's light set out toward the camera lights the faces of the two bodies the camera
    /// sees more than one at the bite, and the faces turned from it less: the irradiance on
    /// each body's near and far side, knight and foe a couple of metres apart across the view
    /// (the ruin's stage, its camera at 55 degrees down, 20 round).
    #[test]
    fn a_hit_light_set_toward_the_camera_lights_what_it_sees() {
        let (pitch, yaw) = (55f32.to_radians(), 20f32.to_radians());
        let cam = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-pitch);
        let to_eye = cam * Vec3::Z;
        assert!(to_eye.y > 0.5, "the camera looks down");
        let near = Vec3::new(to_eye.x, 0.0, to_eye.z).normalize();
        let bite = Vec3::new(1.0, 1.5, -0.7);
        let bodies = [Vec3::new(0.0, 1.2, 0.0), Vec3::new(2.0, 1.3, -1.4)];
        // A face's irradiance from a point light (Lambert over r^2).
        let e = |light: Vec3, at: Vec3, n: Vec3| {
            let d = light - at;
            (d.normalize().dot(n)).max(0.0) / d.length_squared()
        };
        let set = lamp_at(bite + Vec3::Y * 0.5, 1.0, cam);
        assert!(
            (set.y - 2.0).abs() < 1e-5,
            "set out along the ground, not down the view"
        );
        for b in bodies {
            let (seen, hidden) = (near, -near);
            assert!(e(set, b + seen * 0.25, seen) > 2.0 * e(bite, b + seen * 0.25, seen));
            assert!(e(set, b - seen * 0.25, hidden) <= e(bite, b - seen * 0.25, hidden));
        }
    }

    /// A preset's light is dark before its cue, lights at once, and dies with the effect:
    /// nothing past its life, however slow its own fade.
    #[test]
    fn a_presets_light_dies_with_it() {
        let slow = Lamp {
            fall: 100.0,
            ..lamp()
        };
        assert_eq!(lamp_lm(&slow, -0.01, 0.4), 0.0);
        assert!(lamp_lm(&slow, 0.0, 0.4) > 999.0);
        assert!(lamp_lm(&slow, 0.39, 0.4) < 0.1 * 1000.0);
        assert_eq!(lamp_lm(&slow, 0.4, 0.4), 0.0);
        assert_eq!(lamp_lm(&slow, 5.0, 0.4), 0.0);
        // And it only ever dims.
        let mut last = f32::MAX;
        for i in 0..40 {
            let l = lamp_lm(&lamp(), i as f32 * 0.01, 0.4);
            assert!(l <= last, "brighter at {i}0 ms");
            last = l;
        }
    }

    /// The bolts' trail is a streak (a hot line in a halo, effects.wesl's mode 3), the slash's
    /// the edged smear (mode 2): a bolt drawn as the slash would be a crescent again.
    #[test]
    fn a_bolt_trails_a_streak_and_a_slash_an_edge() {
        let mode = |name: &str| {
            let p = Preset::load(name).unwrap();
            Glow::trail(&p.trail.unwrap(), 0.0).look.shape.z
        };
        assert_eq!(mode("bolt"), 3.0);
        assert_eq!(mode("slash"), 2.0);
    }

    /// Every preset parses, and every biome's effects load within the kit's budget.
    #[test]
    fn budgets_hold() {
        for name in names() {
            Preset::load(&name).unwrap_or_else(|e| panic!("{e}"));
        }
        for f in std::fs::read_dir(crate::kit::dir()).unwrap() {
            let p = f.unwrap().path();
            if p.extension().is_some_and(|e| e == "ron") {
                let b = Biome::load(p.file_stem().unwrap().to_str().unwrap()).unwrap();
                let fx = Effects::load(Some(&b)).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
                let n = fx.budget();
                assert!(
                    n.quads <= QUADS && n.lights <= LIGHTS,
                    "{}: {n:?}",
                    p.display()
                );
            }
        }
        let fight = Effects::load(None).unwrap();
        for name in FIGHT {
            assert!(fight.get(name).is_some(), "the staged fight's {name}");
        }
    }

    /// A name with no file fails naming the folder and the presets there are.
    #[test]
    fn an_unknown_preset_names_the_fix() {
        let e = Preset::load("no-such-effect").unwrap_err();
        assert!(
            e.contains("spike/iso/biomes/effects/") && e.contains("hit-spark"),
            "{e}"
        );
    }

    /// A streak flies out along its way and falls under gravity; embers (gravity under
    /// zero) rise.
    #[test]
    fn sparks_fly_and_fall() {
        let (p, v) = spark_at(Vec3::ZERO, Vec3::X * 4.0, 9.8, 0.5);
        assert!((p.x - 2.0).abs() < 1e-5 && p.y < -1.2 && v.y < -4.8);
        let (p, _) = spark_at(Vec3::ZERO, Vec3::ZERO, -1.0, 1.0);
        assert!(p.y > 0.49);
    }

    /// A spark cools as hot metal does: only ever dimmer, white-yellow at birth, orange (its
    /// green under half its red) by mid-life, and dark at death, not a white streak cut off.
    #[test]
    fn sparks_cool_to_dark() {
        let s = Preset::load("hit-spark").unwrap().sparks.unwrap();
        let born = spark_glow(&s, 0.0);
        assert!(born.y > 0.6 * born.x, "born {born}");
        let mid = spark_glow(&s, 0.5);
        assert!(mid.y < 0.5 * mid.x, "mid-life {mid}");
        assert!(spark_glow(&s, 1.0).max_element() < 1e-3);
        let mut last = f32::MAX;
        for i in 0..=20 {
            let l = spark_glow(&s, i as f32 / 20.0).element_sum();
            assert!(l <= last, "brighter at {i}/20");
            last = l;
        }
    }

    /// A puff under a light is lit by it as a lux meter would read (lm / 4π d²), and a light
    /// out of reach or dark leaves nothing.
    #[test]
    fn smoke_is_lit_by_the_lights_at_it() {
        let lm = 4.0 * std::f32::consts::PI * 100.0;
        let e = lit_at(Vec3::ZERO, 0.0, &[(Vec3::Y, Vec3::ONE, lm, 5.0)], None);
        assert!((e.x - 100.0).abs() < 0.01, "{e}");
        assert_eq!(
            lit_at(
                Vec3::ZERO,
                0.0,
                &[(Vec3::Y * 6.0, Vec3::ONE, lm, 5.0)],
                None
            ),
            Vec3::ZERO
        );
        assert_eq!(
            lit_at(Vec3::ZERO, 0.0, &[(Vec3::Y, Vec3::ONE, 0.0, 5.0)], None),
            Vec3::ZERO
        );
        // A light inside a puff a metre in radius lights it as one a metre off, not as one
        // at its heart: what is near the light is not burned out.
        let inside = lit_at(Vec3::ZERO, 1.0, &[(Vec3::ZERO, Vec3::ONE, lm, 5.0)], None);
        assert!((inside.x - 100.0).abs() < 0.01, "{inside}");
    }

    /// A still shows each effect at its chosen moment; a film plays it from its cue, and a
    /// loop round its period.
    #[test]
    fn a_still_holds_the_chosen_moment() {
        let p = Preset::load("hit-spark").unwrap();
        let cues = Cues {
            blow: 2.0,
            bolt: 0.0,
        };
        assert_eq!(
            age(
                &p,
                &Clock {
                    t: 9.0,
                    still: true
                },
                &cues
            ),
            p.still
        );
        assert!(
            (age(
                &p,
                &Clock {
                    t: 2.25,
                    still: false
                },
                &cues
            ) - 0.25)
                .abs()
                < 1e-5
        );
        let s = Preset {
            cue: Cue::Loop(2.0),
            ..p
        };
        assert!(
            (age(
                &s,
                &Clock {
                    t: 5.5,
                    still: false
                },
                &cues
            ) - 1.5)
                .abs()
                < 1e-5
        );
    }
}
