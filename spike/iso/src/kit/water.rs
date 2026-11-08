//! The kit's one water (D634, iso-kit j9): a marsh's channels, a forest's pond, a ruin's
//! puddles and a street's wet asphalt are the same shader (`water.wesl`) on a biome's
//! `water` (biome.rs `Water`), nothing in here a biome's own.
//!
//! - The body absorbs by depth, per channel (Beer-Lambert: what is left of the bed's light
//!   after `d` metres is exp(-absorb d)), and scatters its own dark colour back, lit as the
//!   ground is; a thin film (`film`) is the same water a few millimetres deep.
//! - The shore: the face fades in over the bank's last centimetres (`shore`), its colour and
//!   its mirror together, with scum and floating leaves as data.
//! - The mirror is a planar reflection, as Unity's and Unreal's flat water (Unreal's "Planar
//!   Reflections" docs; Unity's MirrorReflection script): a second camera mirrored under the
//!   face renders the scene above it into a texture, which the face samples by projecting
//!   its own point through that camera, moved by the ripples' slope. At the ARPG camera's
//!   55 degrees Fresnel gives two to three percent, so night water is black except what is
//!   lit over it. Bevy 0.20's screen-space reflections are deferred and opaque only; this
//!   water is blended, and SSR could not mirror what stands off screen above the frame.
//! - Ripples: one tiling normal map (a sum of sines on whole wave numbers, Tessendorf's and
//!   GPU Gems 1 ch. 1's idea at its simplest) read at two scales drifting with the wind;
//!   rain's rings as an option.
//! - Highlights: each `Glint` light's and the key's (the moon's) reflection as a streak
//!   drawn long toward the eye (an anisotropic Beckmann lobe, as pool.rs's lane: Bruneton,
//!   Neyret and Holzschuch 2010; Zirr and Kaplanyan 2016 for its glitter), as night water
//!   shows a lamp's.
//!
//! Use: `plugin`; `spawn_face(.., &water, mesh)` with a mesh from `surface`; `Glint` on the
//! point lights it mirrors.

use super::biome::{Water, WaterKind};
use super::light::Light;
use super::weather::by_place;
use crate::camera::MainCamera;
use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{CameraProjection, ClearColorConfig, Exposure, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::diagnostic::FrameCount;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat,
};
use bevy::shader::ShaderRef;

pub type KitWater = ExtendedMaterial<StandardMaterial, WaterExt>;

/// How many lights a face mirrors.
pub const LIGHTS: usize = 6;
/// The faces' render layer: the main camera sees it, the mirror does not.
pub const LAYER: usize = 9;
/// The ripple map's side (texels).
const RIPPLE_SIZE: usize = 256;

/// The face's uniform (`water.wesl`'s `Look`).
#[derive(Clone, Copy, Debug, Default, Reflect, ShaderType)]
pub struct Look {
    /// The mirror camera's clip from world, to find a point in its texture.
    pub clip_from_world: Mat4,
    /// The colour the body scatters (linear rgb) and, w, the shore's width (m).
    pub body: Vec4,
    /// Absorption a metre (rgb) and, w, a thin film's depth (m, 0: none).
    pub absorb: Vec4,
    /// x the mirror's lift over Fresnel, y its distortion (m a unit slope), z 1 when the
    /// mirror's texture is bound, w its smear (a fraction of the frame, on a thin film).
    pub mirror: Vec4,
    /// x the fine tile (m), y the broad tile (m), z the slope, w the margin's calm.
    pub ripple: Vec4,
    /// xy the wind (m/s), z rain rings a m², w the time (s).
    pub wind: Vec4,
    /// Scum: its albedo (rgb) and, w, how much.
    pub scum: Vec4,
    /// Scum: x its reach (m deep), y its clumps (m); floating leaves: z a m², w a radius (m).
    pub float_: Vec4,
    /// The leaves' albedo (rgb) and, w, the lights in use.
    pub pads: Vec4,
    /// The streaks: x along, y across, z the glitter's cell (m), w its share.
    pub lane: Vec4,
    /// x the lights' gain, y the key's gain, z the key's calm, w its soft peak (nits).
    pub gains: Vec4,
    /// The key's direction (xyz) and, w, a glint's dash.
    pub toward: Vec4,
    /// The key's illuminance (lux, rgb).
    pub key: Vec4,
    /// Two a light: its place (xyz) and range (w); its colour times its lumens (rgb).
    pub lights: [Vec4; LIGHTS * 2],
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct WaterExt {
    #[uniform(100)]
    pub look: Look,
    #[texture(101)]
    #[sampler(102)]
    pub mirror: Option<Handle<Image>>,
    #[texture(103)]
    #[sampler(104)]
    pub ripples: Option<Handle<Image>>,
}

impl MaterialExtension for WaterExt {
    fn fragment_shader() -> ShaderRef {
        "embedded://iso/kit/water.wesl".into()
    }
}

/// A point light the water mirrors.
#[derive(Component)]
pub struct Glint;

/// A water face: its level, for its mirror.
#[derive(Component)]
pub struct Face {
    pub level: f32,
    /// The sky its mirror shows (nits, linear rgb).
    pub sky: Vec3,
}

/// The camera under the face that renders what it mirrors.
#[derive(Component)]
pub struct Mirror;

#[derive(Resource)]
struct Maps {
    ripples: Handle<Image>,
    mirror: Option<Handle<Image>>,
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "water.wesl");
    app.add_plugins(MaterialPlugin::<KitWater>::default())
        .add_systems(PreStartup, maps)
        .add_systems(
            PostUpdate,
            (follow, feed)
                .chain()
                .before(bevy::transform::TransformSystems::Propagate),
        );
}

/// What is left of the light through `depth` metres of the body, per channel (Beer-Lambert).
pub fn transmittance(depth: f32, absorb: Vec3) -> Vec3 {
    let d = depth.max(0.0);
    Vec3::new(
        (-absorb.x * d).exp(),
        (-absorb.y * d).exp(),
        (-absorb.z * d).exp(),
    )
}

/// How much of the face shows at a depth (0 at the bank's line, 1 past `width`): its body's
/// colour and its mirror fade in together.
pub fn shore(depth: f32, width: f32) -> f32 {
    let t = (depth / width.max(1e-4)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How much of the bed the body hides at a depth (its alpha): the shore's fade times what
/// the body absorbs, the mean over the channels (one alpha a pixel: the bed's own tint
/// through the water is the scatter colour's to give). `water.wesl` does the same.
pub fn opacity(depth: f32, absorb: Vec3, width: f32) -> f32 {
    let t = transmittance(depth, absorb);
    shore(depth, width) * (1.0 - (t.x + t.y + t.z) / 3.0)
}

fn linear(c: super::light::Colour) -> Vec3 {
    let l = c.color().to_linear();
    Vec3::new(l.red, l.green, l.blue)
}

/// The face's look from a biome's water, its wind and its light (the key and the sky).
pub fn look(water: &Water, wind: Vec2, light: Option<&Light>) -> (Look, Vec3) {
    let film = match water.kind {
        WaterKind::Wet | WaterKind::Puddles if water.film > 0.0 => water.film,
        WaterKind::Wet => 0.002,
        _ => water.film,
    };
    let (a, s) = (water.absorb, water.streaks);
    let scum = water
        .scum
        .map_or(Vec4::ZERO, |c| linear(c.colour).extend(c.amount));
    let (reach, size) = water.scum.map_or((0.0, 1.0), |c| (c.reach, c.size));
    let (count, radius, green) = water.pads.map_or((0.0, 0.2, Vec3::ZERO), |p| {
        (p.count, p.radius, linear(p.colour))
    });
    let (toward, key) = light.map_or((Vec3::Y, Vec3::ZERO), |l| {
        (
            l.key.toward.vector().normalize(),
            linear(l.key.colour) * l.key.lux,
        )
    });
    let sky = match (water.mirror.sky, water.mirror.nits, light) {
        (Some(c), n, _) => linear(c) * n.unwrap_or(1.0),
        (None, n, Some(l)) => linear(l.sky.colour) * n.unwrap_or(l.sky.nits),
        (None, n, None) => Vec3::splat(n.unwrap_or(0.0)),
    };
    let look = Look {
        clip_from_world: Mat4::IDENTITY,
        body: water
            .deep
            .map_or(Vec3::splat(0.004), linear)
            .extend(water.shore),
        absorb: Vec4::new(a.0, a.1, a.2, film),
        mirror: Vec4::new(
            water.mirror.lift,
            water.mirror.distort,
            0.0,
            water.mirror.smear,
        ),
        ripple: Vec4::new(
            water.ripples.fine,
            water.ripples.broad,
            water.ripples.steep,
            water.ripples.calm,
        ),
        wind: Vec4::new(wind.x, wind.y, water.rain, 0.0),
        scum,
        float_: Vec4::new(reach, size, count, radius),
        pads: green.extend(0.0),
        lane: Vec4::new(s.along, s.across, s.cell, s.glitter),
        gains: Vec4::new(s.gain, s.key, s.calm, s.peak),
        toward: toward.extend(s.dash),
        key: key.extend(0.0),
        lights: default(),
    };
    (look, sky)
}

/// The water's material: the body dark, rough and with no specular of its own (the shader
/// mirrors), premultiplied, so the mirror adds over a body that fades.
pub fn material(look: Look) -> KitWater {
    ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::linear_rgb(look.body.x, look.body.y, look.body.z),
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            alpha_mode: AlphaMode::Premultiplied,
            ..default()
        },
        extension: WaterExt {
            look,
            mirror: None,
            ripples: None,
        },
    }
}

/// Spawns a face of a biome's water on `mesh` (from `surface`), on the water's layer.
pub fn spawn_face(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    waters: &mut Assets<KitWater>,
    water: &Water,
    wind: Vec2,
    light: Option<&Light>,
    mesh: Mesh,
) -> Entity {
    let (look, sky) = look(water, wind, light);
    commands
        .spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(waters.add(material(look))),
            bevy::light::NotShadowCaster,
            RenderLayers::layer(LAYER),
            Face {
                level: water.level,
                sky,
            },
        ))
        .id()
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

/// Marks a face's pools (puddles on a track): each vertex's `share` (0 to 1) of the second
/// uv's y, which water.wesl reads as the least share it mirrors (over Fresnel's 2%) and
/// clears of scum and floating leaves, so a shallow puddle shows the sky and what stands
/// over it to a soft rim while the open water keeps its true Fresnel. 0 everywhere else:
/// a face never marked is unchanged.
pub fn pooled(mesh: &mut Mesh, share: impl Fn(Vec2) -> f32) {
    let Some(bevy::mesh::VertexAttributeValues::Float32x2(uv)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
    else {
        return;
    };
    let shares: Vec<f32> = uv
        .iter()
        .map(|p| share(Vec2::new(p[0], p[1])).clamp(0.0, 1.0))
        .collect();
    if let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvb)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_UV_1)
    {
        for (v, s) in uvb.iter_mut().zip(shares) {
            v[1] = s;
        }
    }
}

/// The `LIGHTS` that shine brightest at the eye (lumens over the distance squared), when
/// more glint than the shader reads: a street's lamps far down the road give way to the
/// signs in the frame.
pub fn seen(
    mut lights: Vec<(Vec3, f32, LinearRgba, f32)>,
    eye: Vec3,
) -> Vec<(Vec3, f32, LinearRgba, f32)> {
    if lights.len() > LIGHTS {
        let at_eye = |l: &(Vec3, f32, LinearRgba, f32)| l.3 / l.0.distance_squared(eye).max(1.0);
        lights.sort_by(|a, b| {
            at_eye(b)
                .total_cmp(&at_eye(a))
                .then_with(|| by_place(a.0, b.0))
        });
        lights.truncate(LIGHTS);
    }
    lights
}

/// Packs the lights as the shader reads them: the brightest `LIGHTS`, place and range,
/// then colour times lumens.
pub fn pack(mut lights: Vec<(Vec3, f32, LinearRgba, f32)>) -> (u32, [Vec4; LIGHTS * 2]) {
    lights.sort_by(|a, b| b.3.total_cmp(&a.3).then_with(|| by_place(a.0, b.0)));
    let mut out = [Vec4::ZERO; LIGHTS * 2];
    let n = lights.len().min(LIGHTS);
    for (k, (at, range, c, lumens)) in lights.into_iter().take(n).enumerate() {
        out[k * 2] = at.extend(range);
        out[k * 2 + 1] = Vec4::new(c.red * lumens, c.green * lumens, c.blue * lumens, 0.0);
    }
    (n as u32, out)
}

/// The ripples' slopes over one tile, `n` texels a side: a sum of sines on whole wave
/// numbers (so it tiles), their directions spread about +x (the shader turns +x downwind),
/// each's height falling as its wave number grows; scaled into -1..1.
pub fn ripple_slopes(n: usize) -> Vec<Vec2> {
    use std::f32::consts::TAU;
    let mut waves = Vec::new();
    for i in 0..40 {
        let g = (i as f32 * 0.618_034).fract();
        let k = 2.0 + (i as f32 * 0.37).fract() * 14.0;
        let a = (g - 0.5) * 2.4;
        let w = Vec2::new((k * a.cos()).round(), (k * a.sin()).round());
        if w == Vec2::ZERO {
            continue;
        }
        let amp = 1.0 / w.length().powf(1.6);
        waves.push((w, amp, (i as f32 * 0.754_877).fract() * TAU));
    }
    let mut out = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            let uv = Vec2::new(i as f32, j as f32) / n as f32;
            let mut s = Vec2::ZERO;
            for (w, amp, phase) in &waves {
                s += *w * TAU * amp * (TAU * w.dot(uv) + phase).cos();
            }
            out.push(s);
        }
    }
    let top = out
        .iter()
        .fold(1e-6f32, |m, s| m.max(s.x.abs()).max(s.y.abs()));
    out.iter().map(|s| *s / top).collect()
}

fn maps(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let data: Vec<u8> = ripple_slopes(RIPPLE_SIZE)
        .into_iter()
        .flat_map(|s| {
            let b = |x: f32| ((x * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
            [b(s.x), b(s.y), 128, 255]
        })
        .collect();
    let mut image = Image::new(
        Extent3d {
            width: RIPPLE_SIZE as u32,
            height: RIPPLE_SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    let mut sampler = ImageSamplerDescriptor::linear();
    sampler.set_address_mode(ImageAddressMode::Repeat);
    image.sampler = ImageSampler::Descriptor(sampler);
    commands.insert_resource(Maps {
        ripples: images.add(image),
        mirror: None,
    });
}

/// The mirror: the main camera reflected in the first face's plane, rendering what stands
/// over the water into a half-size HDR texture before the main camera draws, at its
/// exposure and with no tone curve; the faces live on their own layer it does not see.
#[allow(clippy::type_complexity)]
fn follow(
    mut commands: Commands,
    args: Res<crate::Args>,
    mut maps: ResMut<Maps>,
    mut images: ResMut<Assets<Image>>,
    faces: Query<&Face>,
    main: Query<
        (
            Entity,
            &Transform,
            &Projection,
            Option<&Exposure>,
            Option<&RenderLayers>,
        ),
        (With<MainCamera>, Without<Mirror>),
    >,
    mut mirror: Query<
        (
            &mut Transform,
            &mut Projection,
            &mut Exposure,
            &mut Tonemapping,
            &mut Camera,
        ),
        With<Mirror>,
    >,
) {
    let (Some(face), Ok((cam, t, proj, exposure, layers))) = (faces.iter().next(), main.single())
    else {
        return;
    };
    let exposure = exposure.copied().unwrap_or_default();
    let layers = layers.cloned().unwrap_or_default();
    if !layers.intersects(&RenderLayers::layer(LAYER)) {
        commands.entity(cam).insert(layers.clone().with(LAYER));
    }
    let flip = |v: Vec3| Vec3::new(v.x, -v.y, v.z);
    let mut at = t.translation;
    at.y = 2.0 * face.level - at.y;
    let placed = Transform::from_translation(at).looking_to(flip(*t.forward()), flip(*t.up()));
    let sky = face.sky * exposure.exposure();
    let clear = ClearColorConfig::Custom(Color::linear_rgb(sky.x, sky.y, sky.z));
    let Ok((mut mt, mut mp, mut me, mut tone, mut camera)) = mirror.single_mut() else {
        let (w, h) = if args.shot.is_some() {
            ((args.size.0 / 2).max(1), (args.size.1 / 2).max(1))
        } else {
            (960, 540)
        };
        let image = images.add(Image::new_target_texture(
            w,
            h,
            TextureFormat::Rgba16Float,
            None,
        ));
        maps.mirror = Some(image.clone());
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: -1,
                clear_color: clear,
                ..default()
            },
            RenderTarget::Image(image.into()),
            proj.clone(),
            placed,
            bevy::camera::Hdr,
            Msaa::Off,
            // The fight's marks (Bevy's ForwardDecal) and the kit's soft effects read the
            // depth prepass; without it the decal's shader fails for this view.
            bevy::core_pipeline::prepass::DepthPrepass,
            Tonemapping::None,
            exposure,
            layers.without(LAYER),
            Mirror,
        ));
        return;
    };
    *mt = placed;
    *mp = proj.clone();
    if me.ev100 != exposure.ev100 {
        *me = exposure;
    }
    if *tone != Tonemapping::None {
        *tone = Tonemapping::None;
    }
    camera.clear_color = clear;
}

/// Each frame, every face's mirror camera matrix, the glinting lights (point and spot, each
/// mirrored as a point) as they now stand and burn (they flicker and drift), and the time,
/// at a fixed 30 a frame so a still and a film are the same water.
#[allow(clippy::type_complexity)]
fn feed(
    frame: Res<FrameCount>,
    maps: Res<Maps>,
    lights: Query<(&GlobalTransform, &PointLight), With<Glint>>,
    spots: Query<(&GlobalTransform, &SpotLight), With<Glint>>,
    eye: Query<&GlobalTransform, (With<MainCamera>, Without<Mirror>)>,
    mirror: Query<(&Transform, &Projection), With<Mirror>>,
    mut waters: ResMut<Assets<KitWater>>,
) {
    let found: Vec<_> = lights
        .iter()
        .map(|(t, l)| (t.translation(), l.range, l.color.to_linear(), l.intensity))
        .chain(
            spots
                .iter()
                .map(|(t, l)| (t.translation(), l.range, l.color.to_linear(), l.intensity)),
        )
        .collect();
    let eye = eye.single().map_or(Vec3::ZERO, |t| t.translation());
    let (n, packed) = pack(seen(found, eye));
    let clip = mirror
        .single()
        .ok()
        .map(|(t, p)| p.get_clip_from_view() * t.to_matrix().inverse());
    for (_, mut m) in waters.iter_mut() {
        let e = &mut m.extension;
        e.look.pads.w = n as f32;
        e.look.lights = packed;
        e.look.wind.w = frame.0 as f32 / crate::FILM_FPS as f32;
        e.ripples = Some(maps.ripples.clone());
        if let (Some(c), Some(image)) = (clip, &maps.mirror) {
            e.look.clip_from_world = c;
            e.look.mirror.z = 1.0;
            e.mirror = Some(image.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The body hides the bed by depth as Beer-Lambert says: nothing at the bank, half where
    /// each channel has lost half its light (ln 2 / absorb deep, past the shore), near all
    /// in the deep; the more a channel absorbs, the sooner it is dark.
    #[test]
    fn the_body_darkens_by_depth_as_light_is_absorbed() {
        let k = 4.0;
        let half = std::f32::consts::LN_2 / k;
        assert_eq!(opacity(0.0, Vec3::splat(k), 0.05), 0.0);
        assert_eq!(
            opacity(-0.1, Vec3::splat(k), 0.05),
            0.0,
            "dry land shows nothing"
        );
        assert!((opacity(half, Vec3::splat(k), 0.05) - 0.5).abs() < 1e-4);
        assert!(opacity(1.5, Vec3::splat(k), 0.05) > 0.99);
        let mut last = 0.0;
        for i in 1..60 {
            let a = opacity(i as f32 * 0.02, Vec3::new(9.0, 6.0, 5.0), 0.08);
            assert!(a >= last, "lighter deeper at {}", i as f32 * 0.02);
            last = a;
        }
        let t = transmittance(0.2, Vec3::new(9.0, 6.0, 3.0));
        assert!(t.x < t.y && t.y < t.z, "red is lost first: {t}");
    }

    /// The shore fades the face in over its width from the bank's line, smoothly.
    #[test]
    fn the_shore_fades_in_over_its_width() {
        assert_eq!(shore(0.0, 0.08), 0.0);
        assert_eq!(shore(-0.02, 0.08), 0.0);
        assert_eq!(shore(0.08, 0.08), 1.0);
        assert_eq!(shore(0.5, 0.08), 1.0);
        assert!((shore(0.04, 0.08) - 0.5).abs() < 1e-6);
        let mut last = 0.0;
        for i in 0..=20 {
            let s = shore(i as f32 * 0.005, 0.08);
            assert!(s >= last);
            last = s;
        }
    }

    /// The ripple map tiles: its last column runs into its first, and its last row into its
    /// first, as neighbours do (a map that did not tile jumps by about its range, 2, at the
    /// seam, many times the largest step between neighbours at this sampling).
    #[test]
    fn the_ripple_map_tiles() {
        let n = 128;
        let s = ripple_slopes(n);
        let step = (0..n * n - 1)
            .filter(|k| (k + 1) % n != 0)
            .map(|k| s[k].distance(s[k + 1]))
            .fold(0.0f32, f32::max);
        let rise = (0..n * (n - 1))
            .map(|k| s[k].distance(s[k + n]))
            .fold(0.0f32, f32::max);
        assert!(
            step.max(rise) < 0.4,
            "too coarse to judge a seam: {step}, {rise}"
        );
        for j in 0..n {
            let seam = s[j * n + n - 1].distance(s[j * n]);
            assert!(seam <= step * 1.5, "row {j}: seam {seam} over {step}");
            let seam = s[(n - 1) * n + j].distance(s[j]);
            assert!(seam <= rise * 1.5, "column {j}: seam {seam} over {rise}");
        }
    }

    /// The face covers the water and its margin and nothing drier.
    #[test]
    fn the_face_lies_only_over_water() {
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

    /// Past what the shader reads, a dim light by the eye outranks a bright one far off;
    /// at or under it every light is kept.
    #[test]
    fn the_lights_seen_brightest_are_kept() {
        let far = |k: usize| (Vec3::X * (100.0 + k as f32), 9.0, LinearRgba::WHITE, 1e5);
        let near = (Vec3::new(3.0, 4.0, 0.0), 9.0, LinearRgba::RED, 1e3);
        let mut lights: Vec<_> = (0..LIGHTS).map(far).collect();
        assert_eq!(seen(lights.clone(), Vec3::ZERO).len(), LIGHTS);
        lights.push(near);
        let kept = seen(lights, Vec3::ZERO);
        assert_eq!(kept.len(), LIGHTS);
        assert!(
            kept.iter().any(|l| l.2 == LinearRgba::RED),
            "the near light was dropped"
        );
        assert!(
            kept.iter().all(|l| l.0.x < 100.0 + LIGHTS as f32 - 1.0),
            "the farthest light was kept"
        );
    }

    /// A street's identical lamps, as many to each side of the eye, are kept and packed the
    /// same whatever order the query gave them in: the puddle mirrors the same lamps each run.
    #[test]
    fn equal_lights_rank_the_same_in_any_order() {
        let lamp = |k: usize| {
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            (
                Vec3::new(side * 6.0, 5.0, (k / 2) as f32 * 4.0),
                12.0,
                LinearRgba::WHITE,
                9e3,
            )
        };
        let street: Vec<_> = (0..LIGHTS * 2 + 3).map(lamp).collect();
        let reversed: Vec<_> = street.iter().rev().copied().collect();
        let place =
            |l: Vec<(Vec3, f32, LinearRgba, f32)>| l.iter().map(|l| l.0).collect::<Vec<_>>();
        let (a, b) = (
            seen(street.clone(), Vec3::ZERO),
            seen(reversed.clone(), Vec3::ZERO),
        );
        assert_eq!(
            place(a),
            place(b),
            "seen kept other lamps for another query order"
        );
        assert_eq!(
            pack(street).1,
            pack(reversed).1,
            "pack ranked equal lamps by query order"
        );
    }

    /// A biome's water reaches the face: a still night pond and a street's thin film.
    #[test]
    fn a_biomes_water_reaches_the_face() {
        let marsh = crate::kit::biome::Biome::load("marsh").unwrap();
        let w = marsh.water.expect("the marsh has water");
        let (l, _) = look(&w, Vec2::X, None);
        assert_eq!(l.absorb.w, 0.0, "a pond is deep, not a film");
        assert_eq!(w.level, crate::marsh::LEVEL);
        let film: Water = crate::kit::parse("(kind: Wet)", "w.ron", "biome.rs").unwrap();
        assert!(look(&film, Vec2::X, None).0.absorb.w > 0.0);
        assert_eq!(film.scum, None, "no scum by default");
    }
}
