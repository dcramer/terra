//! The kit's weather (iso-kit j19, D634): a biome's `weather` as rain falling and mist lying.
//!
//! Rain is streaks, as films and games show it: each drop a thin camera-facing quad
//! stretched along its velocity by a shutter's length, falling through a box over the
//! layout's floor and wrapping, faint, lit only by the lamps and signs near it, so it shows
//! in their cones and not in the dark between (the drop scatters what lights it; the moon's
//! and the sky's share is a trace). Where a drop meets the floor a splash's crown flicks up
//! and fades, and the kit's water draws rain's rings (`water.rain`). One mesh of quads moved
//! in weather.wesl's vertex shader, so nothing is spawned while it rains; the budget prints.
//!
//! Mist is height-falloff fog in a lit volume (Bevy's volumetric fog, the raymarched
//! froxel-free kind Unreal's local fog volumes and Frostbite's height fog are): a 3D density
//! over the floor, thickest at the land and water and falling by `e` every `height` metres
//! above them, thicker over open water and in hollows, drifting in banks on the wind, lit by
//! the moon and every lamp in it (each light carries `VolumetricLight`). Never sheets lying
//! on the water (traps.md: the ruin's mist cards lost twice as a milky haze).

use super::biome::Biome;
use super::light::{Colour, Light};
use crate::camera::MainCamera;
use crate::terrain::{fbm, smooth};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::{FogVolume, VolumetricFog, VolumetricLight};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
    TextureDimension, TextureFormat,
};
use bevy::shader::ShaderRef;
use serde::Deserialize;

/// How rain looks where it falls.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Drops {
    /// How fast a drop falls (m/s; a 2 mm drop's terminal speed is about 6.5).
    pub fall: f32,
    /// The shutter (s): a streak is the drop's travel in it.
    pub shutter: f32,
    /// A streak's width (m).
    pub width: f32,
    /// The share of the light at a drop it sends to the eye (a streak's nits are the lux at
    /// it times this over π).
    pub bright: f32,
    /// How many splashes for each streak's share (1: the kit's count at full rain).
    pub splashes: f32,
}

impl Default for Drops {
    fn default() -> Self {
        Self {
            fall: 7.0,
            shutter: 0.04,
            width: 0.012,
            bright: 0.02,
            splashes: 1.0,
        }
    }
}

/// How the mist lies.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Layer {
    /// Its density falls by e every `height` metres over the land or the water.
    pub height: f32,
    /// How many times thicker over open water than over land.
    pub water: f32,
    /// How much thicker in a hollow half a metre under the land round it (0: no more).
    pub hollows: f32,
    /// How much of it gathers in banks drifting on the wind (0: an even layer).
    pub drift: f32,
    /// The banks' size (m).
    pub banks: f32,
    /// Extinction (a metre) at the ground at `mist: 1`.
    pub thick: f32,
    /// The colour it scatters back.
    pub colour: Colour,
    /// The scattering's lean toward the light (Henyey-Greenstein g, 0 to 1).
    pub forward: f32,
    /// Whether the key (the moon or sun) lights it, or only the lamps in it.
    pub keylit: bool,
    /// Whether the lamps (every point and spot light: fires, lanterns, wisps, the effects'
    /// flashes) light it. Off, a fire keeps its crisp warm pool, no bloom halo in the mist
    /// round it (the marsh lost its blind pair twice to that, round 261107-0126).
    pub lamps: bool,
    /// The in-scatter's gain over the physical (Bevy's `FogVolume::light_intensity`): moonlit
    /// at 1 a thin mist seen from above is no brighter than dark ground, so it only veils.
    pub glow: f32,
    /// Clearings (x, z, radius m): none at the centre, rising to the full mist at the radius,
    /// so it lies thicker away from a camp fire.
    pub clear: Vec<(f32, f32, f32)>,
}

impl Default for Layer {
    fn default() -> Self {
        Self {
            height: 0.7,
            water: 1.8,
            hollows: 1.0,
            drift: 0.6,
            banks: 7.0,
            thick: 0.12,
            colour: Colour::Srgb(0.7, 0.75, 0.8),
            forward: 0.3,
            keylit: true,
            lamps: true,
            glow: 1.0,
            clear: Vec::new(),
        }
    }
}

/// The streaks and splashes at full rain, and the lamps a drop is lit by.
pub const STREAKS: usize = 8000;
pub const SPLASHES: usize = 6000;
pub const LAMPS: usize = 16;

/// The layout's floor under the weather: the ground's height on a grid over its span, and
/// the water's level where it stands. A layout inserts one at Startup (the street's strew,
/// the marsh's land); none, a flat floor at 0 round the origin.
#[derive(Resource, Clone, Debug)]
pub struct Floor {
    pub lo: Vec2,
    pub step: f32,
    pub n: (usize, usize),
    pub ground: Vec<f32>,
    pub water: Option<f32>,
}

impl Floor {
    pub fn sample(
        lo: Vec2,
        hi: Vec2,
        step: f32,
        water: Option<f32>,
        height: impl Fn(Vec2) -> f32,
    ) -> Floor {
        let n = (
            ((hi.x - lo.x) / step).ceil() as usize + 1,
            ((hi.y - lo.y) / step).ceil() as usize + 1,
        );
        let mut ground = Vec::with_capacity(n.0 * n.1);
        for j in 0..n.1 {
            for i in 0..n.0 {
                ground.push(height(lo + Vec2::new(i as f32, j as f32) * step));
            }
        }
        Floor {
            lo,
            step,
            n,
            ground,
            water,
        }
    }
    pub fn hi(&self) -> Vec2 {
        self.lo + Vec2::new((self.n.0 - 1) as f32, (self.n.1 - 1) as f32) * self.step
    }
    fn cell(&self, p: Vec2) -> usize {
        let q = ((p - self.lo) / self.step).round();
        let i = (q.x.max(0.0) as usize).min(self.n.0 - 1);
        let j = (q.y.max(0.0) as usize).min(self.n.1 - 1);
        j * self.n.0 + i
    }
    pub fn ground(&self, p: Vec2) -> f32 {
        self.ground[self.cell(p)]
    }
    /// The top of what is there: the water's face over a bed under it, else the ground.
    pub fn surface(&self, p: Vec2) -> f32 {
        let g = self.ground(p);
        self.water.map_or(g, |w| g.max(w))
    }
    pub fn wet(&self, p: Vec2) -> bool {
        self.water.is_some_and(|w| self.ground(p) < w)
    }
    /// The mean surface within `r` m of each cell (a box blur), against which a hollow is low.
    fn broad(&self, r: f32) -> Vec<f32> {
        let k = (r / self.step).round().max(1.0) as isize;
        let (w, h) = (self.n.0 as isize, self.n.1 as isize);
        let surf: Vec<f32> = (0..self.ground.len())
            .map(|c| self.water.map_or(self.ground[c], |l| self.ground[c].max(l)))
            .collect();
        let pass = |src: &[f32], dx: isize, dy: isize| -> Vec<f32> {
            let mut out = vec![0.0; src.len()];
            for y in 0..h {
                for x in 0..w {
                    let (mut s, mut c) = (0.0, 0.0);
                    for t in -k..=k {
                        let (xx, yy) = (x + t * dx, y + t * dy);
                        if xx >= 0 && yy >= 0 && xx < w && yy < h {
                            s += src[(yy * w + xx) as usize];
                            c += 1.0;
                        }
                    }
                    out[(y * w + x) as usize] = s / c;
                }
            }
            out
        };
        pass(&pass(&surf, 1, 0), 0, 1)
    }
}

/// One lamp a drop is lit by, as weather.wesl reads it: `at` xyz and range, `colour` rgb
/// candela (lumens over 4π) and w 1 for a spot, `dir` the spot's axis and its outer cone's
/// cosine, `cone` x the inner's.
#[derive(ShaderType, Clone, Copy, Debug, Default, Reflect)]
pub struct Lamp {
    pub at: Vec4,
    pub colour: Vec4,
    pub dir: Vec4,
    pub cone: Vec4,
}

impl Lamp {
    pub fn point(at: Vec3, colour: Vec3, lumens: f32, range: f32) -> Lamp {
        let cd = colour * lumens / (4.0 * std::f32::consts::PI);
        Lamp {
            at: at.extend(range),
            colour: cd.extend(0.0),
            dir: Vec4::ZERO,
            cone: Vec4::ZERO,
        }
    }
    pub fn spot(
        at: Vec3,
        colour: Vec3,
        lumens: f32,
        range: f32,
        axis: Vec3,
        inner: f32,
        outer: f32,
    ) -> Lamp {
        let mut l = Lamp::point(at, colour, lumens, range);
        l.colour.w = 1.0;
        l.dir = axis.normalize().extend(outer.cos());
        l.cone = Vec4::new(inner.cos(), 0.0, 0.0, 0.0);
        l
    }
}

/// The illuminance (lux, linear rgb) the lamps leave on a drop at `p`, as weather.wesl's
/// `lit` (the two kept the same): inverse square within each lamp's range, Bevy's windowing
/// at the range's end, a spot's cone smooth from its outer edge to its inner; plus the
/// trace `ambient` the key and sky leave.
pub fn lit(p: Vec3, lamps: &[Lamp], ambient: Vec3) -> Vec3 {
    let mut e = ambient;
    for l in lamps {
        let d = l.at.truncate() - p;
        let d2 = d.length_squared().max(0.04);
        let r2 = l.at.w * l.at.w;
        let f = (1.0 - (d2 / r2).powi(2)).clamp(0.0, 1.0).powi(2);
        let mut s = 1.0;
        if l.colour.w > 0.5 {
            let c = (-d.normalize()).dot(l.dir.truncate());
            s = smooth(l.dir.w, l.cone.x, c).powi(2);
        }
        e += l.colour.truncate() * f * s / d2;
    }
    e
}

/// The key's and the sky's share on a drop: a drop is lit from all round, and what reaches
/// the eye off it is what its few lamps send; the open sky's light only greys it a trace.
fn ambient(light: Option<&Light>) -> Vec3 {
    let Some(l) = light else { return Vec3::ZERO };
    let k = l.key.colour.color().to_linear();
    let s = l.sky.colour.color().to_linear();
    0.05 * l.key.lux * Vec3::new(k.red, k.green, k.blue)
        + 0.05 * std::f32::consts::PI * l.sky.nits * Vec3::new(s.red, s.green, s.blue)
}

/// How many streaks and splashes `rain` (0 to 1) draws.
pub fn counts(rain: f32, drops: &Drops) -> (usize, usize) {
    let r = rain.clamp(0.0, 1.0);
    let streaks = (r * STREAKS as f32).round() as usize;
    let splashes = ((r * drops.splashes).clamp(0.0, 2.0) * SPLASHES as f32).round() as usize;
    (streaks, splashes)
}

/// weather.wesl's uniform, field for field.
#[derive(ShaderType, Clone, Copy, Debug, Default, Reflect)]
pub struct Look {
    /// xyz the rain box's low corner, w the time (s).
    pub lo: Vec4,
    /// xyz its size, w the fall speed (m/s).
    pub size: Vec4,
    /// xy the wind (m/s), z the shutter (s), w a streak's width (m).
    pub wind: Vec4,
    /// x the drops' brightness, y the splashes' size, z how many lamps, w a splash's life.
    pub drops: Vec4,
    /// xy the floor's low corner, z its step, w the water's level (-1e4: none).
    pub floor: Vec4,
    /// rgb the trace of key and sky on a drop (lux).
    pub ambient: Vec4,
    pub lamps: [Lamp; LAMPS],
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct Rain {
    #[uniform(0)]
    pub look: Look,
    /// The floor's heights (r) and water (g: 1 where it stands), one texel a cell.
    #[texture(1, sample_type = "float", filterable = false)]
    pub floor: Handle<Image>,
}

impl Material for Rain {
    fn vertex_shader() -> ShaderRef {
        "embedded://iso/kit/weather.wesl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "embedded://iso/kit/weather.wesl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
    fn enable_shadows() -> bool {
        false
    }
    fn enable_prepass() -> bool {
        false
    }
    /// Both faces: a quad turned in the vertex shader faces the eye either way round, and a
    /// splash lying on the floor faced down and was culled (iso-kit j20, round 261107-0054).
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

pub fn plugin(app: &mut App) {
    bevy::asset::embedded_asset!(app, "weather.wesl");
    app.add_plugins(MaterialPlugin::<Rain>::default())
        .add_systems(PostStartup, spawn)
        .add_systems(Update, (light_rain, drift, volumetric_lights));
}

/// The mist's volume and its static shape (the density before the drift), kept to drift it.
#[derive(Component)]
struct Mist {
    image: Handle<Image>,
    dims: (usize, usize, usize),
    lo: Vec3,
    cell: Vec3,
    shape: Vec<f32>,
    layer: Layer,
    wind: Vec2,
}

#[derive(Component)]
struct Falling(Handle<Rain>);

fn flat() -> Floor {
    Floor::sample(Vec2::splat(-24.0), Vec2::splat(24.0), 0.5, None, |_| 0.0)
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    mut commands: Commands,
    biome: Option<Res<Biome>>,
    floor: Option<Res<Floor>>,
    camera: Query<Entity, With<MainCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut rains: ResMut<Assets<Rain>>,
) {
    let Some(w) = biome.as_ref().and_then(|b| b.weather.clone()) else {
        return;
    };
    if w.rain <= 0.0 && w.mist <= 0.0 {
        return;
    }
    let floor = floor.map_or_else(flat, |f| f.clone());
    let wind = Vec2::new(w.wind.0, w.wind.1);
    let (lo, hi) = (floor.lo, floor.hi());
    let (low, top) = floor
        .ground
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), &g| {
            let s = floor.water.map_or(g, |l| g.max(l));
            (a.min(s), b.max(s))
        });
    let mut said = Vec::new();
    if w.rain > 0.0 {
        let (streaks, splashes) = counts(w.rain, &w.drops);
        let mut texels = Vec::with_capacity(floor.ground.len() * 8);
        for c in 0..floor.ground.len() {
            let p =
                floor.lo + Vec2::new((c % floor.n.0) as f32, (c / floor.n.0) as f32) * floor.step;
            texels.extend_from_slice(&floor.surface(p).to_le_bytes());
            texels.extend_from_slice(&(if floor.wet(p) { 1.0f32 } else { 0.0 }).to_le_bytes());
        }
        let map = images.add(Image::new(
            Extent3d {
                width: floor.n.0 as u32,
                height: floor.n.1 as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            texels,
            TextureFormat::Rg32Float,
            RenderAssetUsages::RENDER_WORLD,
        ));
        let above = 9.0;
        let look = Look {
            lo: Vec3::new(lo.x, low, lo.y).extend(0.0),
            size: Vec3::new(hi.x - lo.x, top - low + above, hi.y - lo.y).extend(w.drops.fall),
            wind: Vec4::new(wind.x, wind.y, w.drops.shutter, w.drops.width),
            drops: Vec4::new(w.drops.bright, 1.0, 0.0, 0.35),
            floor: Vec4::new(
                floor.lo.x,
                floor.lo.y,
                floor.step,
                floor.water.unwrap_or(-1e4),
            ),
            ..default()
        };
        let rain = rains.add(Rain { look, floor: map });
        commands.spawn((
            Mesh3d(meshes.add(quads(streaks, splashes))),
            MeshMaterial3d(rain.clone()),
            Falling(rain),
            Transform::IDENTITY,
            NoFrustumCulling,
        ));
        said.push(format!(
            "rain {streaks} streaks + {splashes} splashes ({} triangles, {LAMPS} lamps at most)",
            2 * (streaks + splashes)
        ));
    }
    if w.mist > 0.0 {
        let l = w.layer.clone();
        let reach = (4.0 * l.height).clamp(1.0, 4.0);
        let cell = Vec3::new(0.5, 0.15, 0.5);
        let span = Vec3::new(hi.x - lo.x, top - low + reach + 0.2, hi.y - lo.y);
        let dims = (
            (span.x / cell.x).ceil() as usize,
            (span.y / cell.y).ceil().min(48.0) as usize,
            (span.z / cell.z).ceil() as usize,
        );
        let cell = span / Vec3::new(dims.0 as f32, dims.1 as f32, dims.2 as f32);
        let lo3 = Vec3::new(lo.x, low - 0.2, lo.y);
        let (shape, peak) = shape(&floor, &l, lo3, cell, dims);
        let image = images.add(Image::new(
            Extent3d {
                width: dims.0 as u32,
                height: dims.1 as u32,
                depth_or_array_layers: dims.2 as u32,
            },
            TextureDimension::D3,
            vec![0; dims.0 * dims.1 * dims.2],
            TextureFormat::R8Unorm,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        ));
        commands.spawn((
            FogVolume {
                fog_color: l.colour.color(),
                density_factor: w.mist * l.thick * peak,
                density_texture: Some(image.clone()),
                absorption: 0.05,
                scattering: 1.0,
                scattering_asymmetry: l.forward,
                light_intensity: l.glow,
                ..default()
            },
            Transform::from_translation(lo3 + span * 0.5).with_scale(span),
            Mist {
                image,
                dims,
                lo: lo3,
                cell,
                shape,
                layer: l.clone(),
                wind,
            },
        ));
        for cam in &camera {
            commands.entity(cam).insert(VolumetricFog {
                ambient_intensity: 0.0,
                step_count: 64,
                ..default()
            });
        }
        said.push(format!(
            "mist {}x{}x{} voxels over {:.0} m² to {:.1} m up, extinction {:.3}/m at the land, \
             {:.3}/m at its thickest",
            dims.0,
            dims.1,
            dims.2,
            span.x * span.z,
            reach,
            w.mist * l.thick,
            w.mist * l.thick * peak
        ));
    }
    let name = biome.map_or(String::new(), |b| b.refs.clone());
    println!("weather budget ({name}): {}", said.join("; "));
}

/// The streaks' and splashes' quads: each one's four corners share a seed (the position,
/// 0 to 1 each way) and know their corner (uv: x across -1 to 1, y along 0 to 1) and kind
/// (uv_b.x: 0 a streak, 1 a splash).
fn quads(streaks: usize, splashes: usize) -> Mesh {
    let mut rng = crate::forest::Rng::new(0x5ea1);
    let total = streaks + splashes;
    let (mut pos, mut uv, mut uvb, mut idx) = (
        Vec::with_capacity(total * 4),
        Vec::with_capacity(total * 4),
        Vec::with_capacity(total * 4),
        Vec::with_capacity(total * 6),
    );
    for q in 0..total {
        let seed = [rng.f(), rng.f(), rng.f()];
        let kind = if q < streaks { 0.0 } else { 1.0 };
        let base = (q * 4) as u32;
        for (u, v) in [(-1.0, 0.0), (1.0, 0.0), (1.0, 1.0), (-1.0, 1.0)] {
            pos.push(seed);
            uv.push([u, v]);
            uvb.push([kind, rng.f()]);
        }
        idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, uvb)
    .with_inserted_indices(Indices::U32(idx))
}

/// The lamps nearest the rain each frame (the effects' flashes come and go): the brightest
/// `LAMPS` point and spot lights within reach of the box, and the trace of key and sky.
fn light_rain(
    time: Res<Time>,
    light: Option<Res<Light>>,
    falling: Query<&Falling>,
    points: Query<(&PointLight, &GlobalTransform)>,
    spots: Query<(&SpotLight, &GlobalTransform)>,
    mut rains: ResMut<Assets<Rain>>,
) {
    for f in &falling {
        let Some(mut rain) = rains.get_mut(&f.0) else {
            continue;
        };
        let lo = rain.look.lo.truncate();
        let hi = lo + rain.look.size.truncate();
        let near = |p: Vec3, r: f32| {
            p.cmpge(lo - Vec3::splat(r)).all() && p.cmple(hi + Vec3::splat(r)).all()
        };
        let rgb = |c: Color| {
            let l = c.to_linear();
            Vec3::new(l.red, l.green, l.blue)
        };
        let mut lamps: Vec<Lamp> = points
            .iter()
            .filter(|(p, t)| p.intensity > 0.0 && near(t.translation(), p.range))
            .map(|(p, t)| Lamp::point(t.translation(), rgb(p.color), p.intensity, p.range))
            .chain(
                spots
                    .iter()
                    .filter(|(s, t)| s.intensity > 0.0 && near(t.translation(), s.range))
                    .map(|(s, t)| {
                        Lamp::spot(
                            t.translation(),
                            rgb(s.color),
                            s.intensity,
                            s.range,
                            *t.forward(),
                            s.inner_angle,
                            s.outer_angle,
                        )
                    }),
            )
            .collect();
        lamps.sort_by(|a, b| {
            b.colour
                .truncate()
                .length()
                .total_cmp(&a.colour.truncate().length())
        });
        lamps.truncate(LAMPS);
        rain.look.drops.z = lamps.len() as f32;
        rain.look.lamps = [Lamp::default(); LAMPS];
        rain.look.lamps[..lamps.len()].copy_from_slice(&lamps);
        rain.look.ambient = ambient(light.as_deref()).extend(0.0);
        rain.look.lo.w = time.elapsed_secs();
    }
}

/// The mist's density at `p` before the drift (0 up): falling by e every `height` m over the
/// surface, more over open water and in hollows, none under the surface.
pub fn density(floor: &Floor, broad: &[f32], l: &Layer, p: Vec3) -> f32 {
    let xz = Vec2::new(p.x, p.z);
    let s = floor.surface(xz);
    let above = p.y - s;
    if above < -0.05 {
        return 0.0;
    }
    let low = (-above.max(0.0) / l.height.max(0.05)).exp();
    let hollow = smooth(0.0, 0.5, broad[floor.cell(xz)] - s);
    let water = if floor.wet(xz) { l.water } else { 1.0 };
    let clear = l.clear.iter().fold(1.0, |k, &(x, z, r)| {
        k * smooth(0.0, r.max(0.01), xz.distance(Vec2::new(x, z)))
    });
    low * (1.0 + l.hollows * hollow) * water * clear
}

/// The volume's shape, normalised to 0 to 1 for the texture, and the peak it was divided by.
fn shape(
    floor: &Floor,
    l: &Layer,
    lo: Vec3,
    cell: Vec3,
    dims: (usize, usize, usize),
) -> (Vec<f32>, f32) {
    let broad = floor.broad(4.0);
    let mut out = Vec::with_capacity(dims.0 * dims.1 * dims.2);
    for z in 0..dims.2 {
        for y in 0..dims.1 {
            for x in 0..dims.0 {
                let p = lo + (Vec3::new(x as f32, y as f32, z as f32) + 0.5) * cell;
                out.push(density(floor, &broad, l, p));
            }
        }
    }
    let peak = out.iter().copied().fold(1e-6, f32::max);
    out.iter_mut().for_each(|d| *d /= peak);
    (out, peak)
}

/// The banks drifting downwind: the shape times broad noise moving with the wind (a second,
/// slower field higher up, so the layer shears), written into the volume's texture.
fn drift(time: Res<Time>, mists: Query<&Mist>, mut images: ResMut<Assets<Image>>) {
    let t = time.elapsed_secs();
    for m in &mists {
        let Some(mut image) = images.get_mut(&m.image) else {
            continue;
        };
        let (dx, dy, dz) = m.dims;
        let b = m.layer.banks.max(0.5);
        let mut cols = vec![(0.0f32, 0.0f32); dx * dz];
        for z in 0..dz {
            for x in 0..dx {
                let p = Vec2::new(
                    m.lo.x + (x as f32 + 0.5) * m.cell.x,
                    m.lo.z + (z as f32 + 0.5) * m.cell.z,
                );
                let a = fbm((p - m.wind * t) / b, 71);
                let c = fbm((p - m.wind * t * 0.6) / (b * 1.6), 72);
                cols[z * dx + x] = (smooth(0.3, 0.7, a), smooth(0.3, 0.7, c));
            }
        }
        let k = m.layer.drift.clamp(0.0, 1.0);
        let mut data = vec![0u8; dx * dy * dz];
        for z in 0..dz {
            for y in 0..dy {
                let up = y as f32 / dy.max(1) as f32;
                for x in 0..dx {
                    let (a, c) = cols[z * dx + x];
                    let n = 1.0 - k + k * (a + (c - a) * up) * 1.6;
                    let i = (z * dy + y) * dx + x;
                    data[i] = (m.shape[i] * n).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8;
                }
            }
        }
        image.data = Some(data);
    }
}

/// The lights in a misty biome light the mist as its layer says, the lamps and the key each
/// (Bevy 0.20's volumetric_fog.wesl skips a light without `VolumetricLight`'s flag bit, the
/// clustered point and spot lights by `POINT_LIGHT_FLAGS_VOLUMETRIC_BIT`).
#[allow(clippy::type_complexity)]
fn volumetric_lights(
    mut commands: Commands,
    mists: Query<&Mist>,
    lamps: Query<
        Entity,
        (
            Or<(With<PointLight>, With<SpotLight>)>,
            Without<VolumetricLight>,
        ),
    >,
    keys: Query<Entity, (With<DirectionalLight>, Without<VolumetricLight>)>,
) {
    let Some(m) = mists.iter().next() else { return };
    if m.layer.lamps {
        for e in &lamps {
            commands.entity(e).insert(VolumetricLight);
        }
    }
    if m.layer.keylit {
        for e in &keys {
            commands.entity(e).insert(VolumetricLight);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A drop in a street lamp's cone is lit; one beside the cone at the same distance, or
    /// past the lamp's range, is near black: rain shows only in the light.
    #[test]
    fn rain_shows_only_in_light() {
        let lamp = Lamp::spot(
            Vec3::new(0.0, 6.0, 0.0),
            Vec3::ONE,
            600_000.0,
            14.0,
            Vec3::NEG_Y,
            0.3,
            0.52,
        );
        let amb = Vec3::splat(2.0);
        let under = lit(Vec3::new(0.0, 2.0, 0.0), &[lamp], amb).x;
        let beside = lit(Vec3::new(4.0, 6.0, 0.0), &[lamp], amb).x;
        let far = lit(
            Vec3::new(0.0, -9.0, 0.0),
            &[Lamp::point(Vec3::ZERO, Vec3::ONE, 1e5, 8.0)],
            amb,
        )
        .x;
        assert!(
            under > 100.0 * beside,
            "under the lamp {under} lux, beside its cone {beside}"
        );
        assert!(far <= amb.x + 1e-3, "past the range {far}");
        // A point light lights all round it, falling with the square of the distance.
        let p = Lamp::point(Vec3::ZERO, Vec3::ONE, 4000.0, 20.0);
        let (a, b) = (
            lit(Vec3::X, &[p], Vec3::ZERO).x,
            lit(Vec3::X * 2.0, &[p], Vec3::ZERO).x,
        );
        assert!((a / b - 4.0).abs() < 0.2, "1 m {a}, 2 m {b}");
    }

    /// The mist is thickest low: at the ground many times what it is two metres up, more
    /// over open water than over land at the same height above it, more in a hollow.
    #[test]
    fn mist_is_thickest_low() {
        // Land at 0.4 m with a pool (bed -0.5, water 0) on x < 0 and a dip at x 6.
        let floor = Floor::sample(
            Vec2::splat(-10.0),
            Vec2::splat(10.0),
            0.25,
            Some(0.0),
            |p| {
                if p.x < 0.0 {
                    -0.5
                } else if (p.x - 6.0).abs() < 1.0 {
                    0.15
                } else {
                    0.4
                }
            },
        );
        let l = Layer::default();
        let broad = floor.broad(4.0);
        let d = |x: f32, up: f32| {
            let s = floor.surface(Vec2::new(x, 3.0));
            density(&floor, &broad, &l, Vec3::new(x, s + up, 3.0))
        };
        assert!(
            d(3.0, 0.05) > 5.0 * d(3.0, 2.0),
            "ground {} two metres up {}",
            d(3.0, 0.05),
            d(3.0, 2.0)
        );
        assert!(
            d(-4.0, 0.2) > 1.5 * d(3.0, 0.2),
            "over water {} over land {}",
            d(-4.0, 0.2),
            d(3.0, 0.2)
        );
        assert!(
            d(6.0, 0.2) > d(9.0, 0.2) * 1.1,
            "in the dip {} on the flat {}",
            d(6.0, 0.2),
            d(9.0, 0.2)
        );
        assert_eq!(
            density(&floor, &broad, &l, Vec3::new(3.0, -0.5, 3.0)),
            0.0,
            "none under the land"
        );
        // A clearing at x 3 (a camp fire's): none at its centre, the full mist past it.
        let c = Layer {
            clear: vec![(3.0, 3.0, 2.0)],
            ..Layer::default()
        };
        let dc = |x: f32| {
            let s = floor.surface(Vec2::new(x, 3.0));
            density(&floor, &broad, &c, Vec3::new(x, s + 0.2, 3.0))
        };
        assert_eq!(dc(3.0), 0.0, "none at the clearing's centre");
        assert!(dc(4.0) < 0.6 * d(4.0, 0.2), "thin within it");
        assert_eq!(dc(8.0), d(8.0, 0.2), "the full mist past it");
    }

    /// Rain's count follows its amount up to the kit's budget; no rain draws nothing.
    #[test]
    fn rain_keeps_to_its_budget() {
        let d = Drops::default();
        assert_eq!(counts(0.0, &d), (0, 0));
        let (s, p) = counts(0.5, &d);
        assert_eq!((s, p), (STREAKS / 2, SPLASHES / 2));
        let (s, p) = counts(3.0, &Drops { splashes: 9.0, ..d });
        assert!(
            s <= STREAKS && p <= 2 * SPLASHES,
            "{s} streaks, {p} splashes"
        );
        let m = quads(10, 5);
        assert_eq!(m.count_vertices(), 60);
    }
}
