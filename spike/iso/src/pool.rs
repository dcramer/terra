//! Still dark water for the ruin's pool, its last user: the marsh's channels (iso-kit j10)
//! and the forest's pond (j28) are the kit's water (kit/water.rs). Seen from an
//! action RPG's camera, 55 degrees down, real water reflects only two to four percent of
//! what is above it, so it reads near black (Diablo IV's and V Rising's pools); what makes
//! it water is what it mirrors: a faint band of the sky, broken by ripples, and the narrow
//! glittering paths of the lights by it. So the lit material here is dark and gives no
//! specular of its own (a glossy face under a strong point light renders as one flat lit
//! disc: iso-art j10's pool), and the shader (shaders/pool.wesl) adds the mirror: the sky
//! by a Fresnel term, darkened near the banks as the bank's reflection would, and each
//! `Glint` light's reflection off the rippled face as a sharp normalised lobe, which the
//! ripples stretch into a streak toward the eye. The ripples are the wind's (noise drifting
//! downwind) and rings spreading from drips and rising things.
//!
//! Use: `pool::plugin`; a mesh from `surface` over the water's area at its level, its
//! depth carried per vertex; `material(look)`, or `with_edge(look, edge)` for a soft bank,
//! duckweed and lily pads and a broken sky (`Edge`), `with_sheen` for a moon's sheen
//! (`Sheen`), `with_body` for clear water by day (`Body`); `Glint` on the point lights to
//! mirror.

use bevy::asset::embedded_asset;
use bevy::diagnostic::FrameCount;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

pub use crate::kit::water::{Glint, LIGHTS, pack, surface};

pub type PoolMaterial = ExtendedMaterial<StandardMaterial, PoolExt>;

#[derive(Clone, Copy, Debug, Default, Reflect, ShaderType)]
pub struct Look {
    /// The water's own colour over the deep (linear rgb), and the depth (m) it is reached at.
    pub deep: Vec4,
    /// The shallows' colour over the mud, and how clear the shallows are (0..1).
    pub shallow: Vec4,
    /// The sky's radiance high up (nits, rgb) and how strongly the faked mirror is lifted
    /// over true Fresnel (a: 1 is physical).
    pub sky: Vec4,
    /// The sky's radiance at the horizon (nits) and, a: how far from the bank (m of depth)
    /// the bank's dark reflection reaches.
    pub horizon: Vec4,
    /// xy: the wind's direction; z: how steep the wind's ripples are; w: the time (s).
    pub wave: Vec4,
    /// x: lights in use; y: the glint lobe's sharpness (a Phong exponent: 600 is about a
    /// 3 degree highlight); z: the glints' gain; w: how many drip rings a square metre.
    pub look: Vec4,
    /// Two a light: its place (xyz) and range (w); its colour times its lumens (rgb).
    pub lights: [Vec4; LIGHTS * 2],
}

/// The water's margin, where it meets the bank: how softly it fades into the mud, what
/// floats there, and how the mirrored sky breaks up. All zero (the default) is the plain
/// margin: a 6 cm fade, nothing floating, an even sky.
#[derive(Clone, Copy, Debug, Default, Reflect, ShaderType)]
pub struct Edge {
    /// x: the depth (m) the water fades in over at the bank (0: 0.06); y: how much the
    /// mirrored sky varies in broad blots, the far bank's trees and clouds (0..1); z: the
    /// clouds' size (m, 0: 8).
    pub fade: Vec4,
    /// Duckweed: x how much of the margin it mats (0..1), y the depth (m) it reaches out to,
    /// z its patches' size (m); w lily pads a square metre of open shallows.
    pub weed: Vec4,
    /// The duckweed's albedo (linear rgb) and, w, a lily pad's radius (m).
    pub green: Vec4,
}

/// A moon (or a low sun) the water mirrors as a broad sheen over much of the frame, the
/// ripples breaking a sharper glitter out of it (vr-pond-slash's moonlit pond). All zero (the
/// default) mirrors none.
#[derive(Clone, Copy, Debug, Default, Reflect, ShaderType)]
pub struct Sheen {
    /// The direction toward it (xyz) and, w, how broad the sheen is (a Phong exponent: 8
    /// spreads it from the top of an ARPG frame to the middle).
    pub toward: Vec4,
    /// Its radiance (nits, rgb), lifted as the sky is, and, w, the glitter's share (0..1).
    pub colour: Vec4,
    /// Its lane, and every glinting light's streak (zero: the lights' round Phong lobe): the
    /// ripples' slopes as an anisotropic Beckmann spread, x along the line from the face to
    /// the eye and y across it (an art cheat: at 55 degrees down a true glitter path is
    /// nearly round; drawn long toward the eye it reads as the moon's lane, as ARPGs paint
    /// it); z the glitter's facet cell (m); w the glitter's share of the lane (0..1).
    pub lane: Vec4,
    /// The moon's illuminance on the water (lux, rgb: its colour times its lux) and, w, the
    /// lane's gain over physical (0: no lane).
    pub moon: Vec4,
    /// How soft the moon's lane is (all zero: as the ripples and the lobe make it): x its
    /// peak (nits, its brightest channel, reached softly as x / (1 + x / peak), so the lane
    /// never outshines a fight's effects); y how much of the ripples' tilt it sees taken
    /// away (0..1: 0.5 halves it, so broad ripples do not break it into blotches); z the
    /// glints' stretch across it (a facet's dot drawn z times as long as it is tall: the
    /// stacked dashes of a painted moon's lane, not round speckles).
    pub soft: Vec4,
}

/// Clear water by day (the forest's pond against le-river-beam's river): the bed showing
/// through the deep, and the breeze's fine ripples the sun's glitter breaks into many small
/// sharp glints. All zero (the default) is the dark still water of the marsh and the ruin.
#[derive(Clone, Copy, Debug, Default, Reflect, ShaderType)]
pub struct Body {
    /// The fine ripples: x their height (m, 0: none), y their wavelength (m), z the size of
    /// the calm and ruffled patches that break the glitter up (m), w how calm the calm ones
    /// are (0..1).
    pub ripple: Vec4,
    /// x: the deep's opacity (0: 0.97, near black); the bed shows through the rest, and the
    /// mirror is added over it undimmed.
    pub clear: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct PoolExt {
    #[uniform(100)]
    pub look: Look,
    #[uniform(101)]
    pub edge: Edge,
    #[uniform(102)]
    pub sheen: Sheen,
    #[uniform(103)]
    pub body: Body,
}

const SHADER: &str = "embedded://iso/shaders/pool.wesl";

impl MaterialExtension for PoolExt {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "shaders/pool.wesl");
    app.add_plugins(MaterialPlugin::<PoolMaterial>::default())
        .add_systems(PostUpdate, mirror);
}

/// The water's material: dark, rough and without specular of its own (the shader mirrors).
pub fn material(look: Look) -> PoolMaterial {
    with_edge(look, Edge::default())
}

/// The water's material with a margin of its own: a soft bank, duckweed and lily pads.
pub fn with_edge(look: Look, edge: Edge) -> PoolMaterial {
    with_sheen(look, edge, Sheen::default())
}

/// The water's material with its margin and a moon's sheen on it.
pub fn with_sheen(look: Look, edge: Edge, sheen: Sheen) -> PoolMaterial {
    with_body(look, edge, sheen, Body::default())
}

/// The water's material, clear by day with fine ripples (`Body`).
pub fn with_body(look: Look, edge: Edge, sheen: Sheen, body: Body) -> PoolMaterial {
    ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::linear_rgb(look.deep.x, look.deep.y, look.deep.z),
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            alpha_mode: AlphaMode::Blend,
            ..default()
        },
        extension: PoolExt {
            look,
            edge,
            sheen,
            body,
        },
    }
}

/// Each frame, the glinting lights as they now stand and burn (they flicker and drift),
/// and the time, at a fixed 30 a frame so a still and a film are the same water.
fn mirror(
    frame: Res<FrameCount>,
    lights: Query<(&GlobalTransform, &PointLight), With<Glint>>,
    mut pools: ResMut<Assets<PoolMaterial>>,
) {
    let found: Vec<_> = lights
        .iter()
        .map(|(t, l)| (t.translation(), l.range, l.color.to_linear(), l.intensity))
        .collect();
    let (n, packed) = pack(found);
    for (_, mut m) in pools.iter_mut() {
        m.extension.look.look.x = n as f32;
        m.extension.look.lights = packed;
        m.extension.look.wave.w = frame.0 as f32 / crate::FILM_FPS as f32;
    }
}
