//! Still dark water for any scene: a marsh's channels, a ruin's pool, a mere. Seen from an
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

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::diagnostic::FrameCount;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

pub type PoolMaterial = ExtendedMaterial<StandardMaterial, PoolExt>;

/// How many lights a pool mirrors.
pub const LIGHTS: usize = 6;

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

/// A point light the water mirrors.
#[derive(Component)]
pub struct Glint;

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

/// The face: a grid over `lo..hi` at `level`, each vertex's world xz in its uv and the
/// water's depth over `ground` in its second uv, with triangles only where some corner is
/// wet or nearly so.
pub fn surface(lo: Vec2, hi: Vec2, step: f32, level: f32, ground: impl Fn(Vec2) -> f32) -> Mesh {
    let nx = ((hi.x - lo.x) / step) as usize + 1;
    let nz = ((hi.y - lo.y) / step) as usize + 1;
    let mut pos = Vec::with_capacity(nx * nz);
    let mut uv = Vec::with_capacity(nx * nz);
    let mut uvb = Vec::with_capacity(nx * nz);
    for j in 0..nz {
        for i in 0..nx {
            let p = lo + Vec2::new(i as f32, j as f32) * step;
            pos.push([p.x, level, p.y]);
            uv.push([p.x, p.y]);
            uvb.push([level - ground(p), 0.0]);
        }
    }
    let mut idx: Vec<u32> = Vec::new();
    for j in 0..nz - 1 {
        for i in 0..nx - 1 {
            let a = j * nx + i;
            let (b, c, d) = (a + 1, a + nx, a + nx + 1);
            if [a, b, c, d].iter().all(|&k| uvb[k][0] < -0.05) {
                continue;
            }
            let [a, b, c, d] = [a, b, c, d].map(|k| k as u32);
            idx.extend_from_slice(&[a, d, b, a, c, d]);
        }
    }
    let n = pos.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, uvb)
    .with_inserted_indices(Indices::U32(idx))
}

/// Packs the lights as the shader reads them: the brightest `LIGHTS`, place and range,
/// then colour times lumens.
pub fn pack(mut lights: Vec<(Vec3, f32, LinearRgba, f32)>) -> (u32, [Vec4; LIGHTS * 2]) {
    lights.sort_by(|a, b| b.3.total_cmp(&a.3));
    let mut out = [Vec4::ZERO; LIGHTS * 2];
    let n = lights.len().min(LIGHTS);
    for (k, (at, range, c, lumens)) in lights.into_iter().take(n).enumerate() {
        out[k * 2] = at.extend(range);
        out[k * 2 + 1] = Vec4::new(c.red * lumens, c.green * lumens, c.blue * lumens, 0.0);
    }
    (n as u32, out)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The face covers the water and its margin and nothing drier.
    #[test]
    fn the_face_lies_only_over_water() {
        // A bowl: wet within 2 m of the middle.
        let ground = |p: Vec2| p.length() - 2.0;
        let m = surface(Vec2::splat(-5.0), Vec2::splat(5.0), 0.25, 0.0, ground);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(pos)) =
            m.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        let Some(Indices::U32(idx)) = m.indices() else {
            panic!("indices");
        };
        assert!(!idx.is_empty());
        for &k in idx {
            let p = Vec2::new(pos[k as usize][0], pos[k as usize][2]);
            assert!(p.length() < 2.5, "a triangle at {p} on dry land");
        }
    }

    /// The brightest lights are mirrored first, and no more than the shader reads.
    #[test]
    fn the_brightest_lights_are_kept() {
        let lights: Vec<_> = (0..LIGHTS + 3)
            .map(|k| (Vec3::X * k as f32, 5.0, LinearRgba::WHITE, k as f32 * 100.0))
            .collect();
        let (n, packed) = pack(lights);
        assert_eq!(n as usize, LIGHTS);
        assert_eq!(packed[0].x, (LIGHTS + 2) as f32);
        assert_eq!(packed[1].x, (LIGHTS + 2) as f32 * 100.0);
    }
}
