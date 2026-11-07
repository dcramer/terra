//! The kit's ground (D634): a biome's `ground` layers, each a scan of the `materials/ground`
//! stack at its true size laid by a mask, blended by height where they overlap (ground.wesl).
//! A mask is the layout's own map by name (`Site("grass")`, its authored weights, carried per
//! vertex) or one of the kit's rules, baked here on the CPU from the land into two mask
//! textures a quarter metre a texel: mud low by the water, wet nearer and lower, moss in
//! shade and creases, snow on what faces up, grime in hollows, steep where the land is; each
//! broken at two scales of noise so no edge is a contour line. A rule is the same for a
//! marsh's bank and a street's gutter: nothing here knows a genre.
//!
//! The height blend is Andrey Mishkinis', "Advanced Terrain Texture Splatting" (Gamasutra,
//! 2013): of the layers lying at a point, the highest of weight plus the scan's own height
//! wins, over a short depth, so grass tufts stand through moss and leaves lie in clumps.

use super::biome::{Ground, Mask, Weather};
use crate::terrain::{EXTENT, Land, fbm, smooth};
use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::image::{
    ImageAddressMode, ImageArrayLayout, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor,
};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat,
};
use bevy::shader::ShaderRef;

/// The `materials/ground` stack's layers in order and the metres each scan covers
/// (assets/materials/README.md; moss_004's size is not given there: 1.1 m, as the glade
/// drew it).
pub const STACK: [(&str, f32); 12] = [
    ("brown_mud_02", 1.3),
    ("leafy_grass", 2.0),
    ("rock_face_03", 2.7),
    ("sand_01", 1.5),
    ("dry_decay_leaves", 2.0),
    ("forest_leaves_04", 1.5),
    ("roots", 1.67),
    ("moss_004", 1.1),
    ("gray_rocks", 1.8),
    ("rocks_ground_01", 2.0),
    ("snow_02", 2.0),
    ("cobblestone_floor_05", 2.0),
];

/// Most layers the shader blends.
pub const LAYERS: usize = 8;
/// The mask textures' texel, metres.
const TEXEL: f32 = 0.25;

pub type GroundMaterial = ExtendedMaterial<StandardMaterial, GroundExt>;

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "ground.wesl");
    app.add_plugins(MaterialPlugin::<GroundMaterial>::default());
}

/// The layers as the shader reads them, one entry a layer.
#[derive(Clone, Copy, Debug, Default, Reflect, ShaderType)]
pub struct Params {
    /// x the stack's layer, y metres, z the mask's source (`source`), w how wet it reads.
    pub what: [Vec4; LAYERS],
    /// rgb the tint, a the amount.
    pub tint: [Vec4; LAYERS],
    /// The drifts: broad scale, fine scale, the band (x 0: none).
    pub breakup: [Vec4; LAYERS],
    /// x 1 projected from three sides, y 1 has a material.
    pub how: [Vec4; LAYERS],
    /// The scorched points, xz, z 1 where one is.
    pub scorch: [Vec4; 4],
    /// x the layers, y the scorches, z and w the broad variation's darkest and brightest.
    pub info: Vec4,
    /// The mask textures' corner (x, z) and size (metres).
    pub masks: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct GroundExt {
    #[texture(100, dimension = "2d_array")]
    #[sampler(101)]
    pub color: Handle<Image>,
    #[texture(102, dimension = "2d_array")]
    pub normal: Handle<Image>,
    #[uniform(103)]
    pub params: Params,
    /// Mud, wet, moss, snow.
    #[texture(104)]
    #[sampler(105)]
    pub masks_a: Handle<Image>,
    /// Grime, steep.
    #[texture(106)]
    pub masks_b: Handle<Image>,
}

impl MaterialExtension for GroundExt {
    fn fragment_shader() -> ShaderRef {
        "embedded://iso/kit/ground.wesl".into()
    }
}

/// Where a layout's ground lies: its land, the names of its six per-vertex maps (in the
/// mesh's colour and second texture coordinate), its water's height, and how shaded each
/// point is (0 open sky, 1 under a crown).
pub struct Place<'a> {
    pub land: &'a Land,
    pub sites: [&'a str; 6],
    pub water: Option<f32>,
    pub shade: &'a dyn Fn(Vec2) -> f32,
}

/// The shader's source for a mask: the six sites, the shade and wear the vertex carries,
/// everywhere, then the rules' textures.
fn source(mask: &Mask, sites: &[&str; 6]) -> Result<f32, String> {
    Ok(match mask {
        Mask::Site(name) => sites.iter().position(|s| s == name).ok_or_else(|| {
            format!(
                "the layout has no map {name:?}; its maps are {}. Fix: name one of those in \
                 the layer's Site(...), or use a rule mask (Mud, Wet, Moss, Snow, Grime, \
                 Steep, Wear, Base; kit/ground.rs, D634)",
                sites.join(", ")
            )
        })? as f32,
        Mask::Wear => 7.0,
        Mask::Base => 8.0,
        Mask::Mud => 9.0,
        Mask::Wet => 10.0,
        Mask::Moss => 11.0,
        Mask::Snow => 12.0,
        Mask::Grime => 13.0,
        Mask::Steep => 14.0,
    })
}

/// A biome's ground as the shader's uniform, or what is wrong with it and the fix (D556).
pub fn params(ground: &Ground, sites: &[&str; 6]) -> Result<Params, String> {
    let mut p = Params::default();
    if ground.layers.len() > LAYERS || ground.scorch.len() > 4 {
        return Err(format!(
            "the ground has {} layers and {} scorches; the kit blends at most {LAYERS} and \
             4. Fix: merge layers that share a mask (kit/ground.rs, D634)",
            ground.layers.len(),
            ground.scorch.len()
        ));
    }
    for (i, l) in ground.layers.iter().enumerate() {
        let say = |e: String| format!("ground layer {} ({:?}): {e}", i + 1, l.mask);
        let (index, size) = match &l.material {
            Some(name) => STACK
                .iter()
                .position(|(n, _)| n == name)
                .map(|k| (k as f32, STACK[k].1))
                .ok_or_else(|| {
                    say(format!(
                        "no material {name:?} in the ground stack. Fix: one of {}, or stack \
                         the scan first (assets/materials/README.md)",
                        STACK.map(|s| s.0).join(", ")
                    ))
                })?,
            None => (0.0, 1.0),
        };
        // Snow's amount moves its slope limit (`snow`), so it is laid whole.
        let amount = if l.mask == Mask::Snow { 1.0 } else { l.amount };
        let src = source(&l.mask, sites).map_err(say)?;
        p.what[i] = Vec4::new(index, l.metres.unwrap_or(size), src, l.wets);
        p.tint[i] = Vec4::new(l.tint.0, l.tint.1, l.tint.2, amount);
        p.breakup[i] = l
            .breakup
            .map_or(Vec4::ZERO, |(a, b, c, d)| Vec4::new(a, b, c, d));
        p.how[i] = Vec4::new(
            f32::from(l.triplanar),
            f32::from(l.material.is_some()),
            0.0,
            0.0,
        );
    }
    for (i, (x, z)) in ground.scorch.iter().enumerate() {
        p.scorch[i] = Vec4::new(*x, *z, 1.0, 0.0);
    }
    p.info = Vec4::new(
        ground.layers.len() as f32,
        ground.scorch.len() as f32,
        ground.broad.0,
        ground.broad.1,
    );
    p.masks = Vec4::new(-EXTENT, -EXTENT, 2.0 * EXTENT, 2.0 * EXTENT);
    Ok(p)
}

/// What the rules know of a point.
#[derive(Clone, Copy, Debug)]
pub struct Probe {
    pub at: Vec2,
    /// Metres over the water's face (negative under it; very high where there is none).
    pub over: f32,
    pub normal: Vec3,
    /// Metres to the nearest open water.
    pub to_water: f32,
    /// How far the land lies below its neighbours a metre round, metres: above 0 in a
    /// hollow or crease, below on a crown.
    pub hollow: f32,
    /// 0 under open sky, 1 in deep shade.
    pub shade: f32,
}

/// An edge's wander, metres: noise at two scales, about -0.7 to 0.7.
fn ragged(p: Vec2, seed: u32) -> f32 {
    (fbm(p * 0.3, seed) - 0.5) * 1.0 + (fbm(p * 1.4, seed + 7) - 0.5) * 0.4
}

/// Mud: low by the water, out to about 1.5 m from it and half a metre over it, its edge
/// ragged; under water too.
pub fn mud(q: &Probe) -> f32 {
    let d = q.to_water + ragged(q.at, 61) * 1.4;
    let reach = if q.to_water == 0.0 {
        1.0
    } else {
        smooth(1.9, 0.6, d)
    };
    reach * smooth(0.5, 0.1, q.over)
}

/// Wet: tighter than the mud, within about a metre of the water and a few centimetres over
/// it; rain wets the rest, hollows most.
pub fn wet(q: &Probe, rain: f32) -> f32 {
    let d = q.to_water + ragged(q.at, 67) * 0.6;
    let bank = smooth(0.9, 0.2, d) * smooth(0.22, 0.03, q.over);
    bank.max(rain * (0.4 + 0.6 * smooth(0.005, 0.04, q.hollow)))
}

/// Moss: where it is shaded or creased and not steep or under water, in cushions.
pub fn moss(q: &Probe) -> f32 {
    let cover = q.shade.max(smooth(0.01, 0.06, q.hollow));
    let cushions = 0.7 + 0.6 * fbm(q.at * 0.45, 71);
    smooth(0.25, 0.6, cover * cushions) * smooth(0.55, 0.8, q.normal.y) * smooth(-0.05, 0.1, q.over)
}

/// Snow: on what faces up; `amount` (0 to 1) brings it down steeper slopes.
pub fn snow(q: &Probe, amount: f32) -> f32 {
    let limit = 0.97 - 0.3 * amount.clamp(0.0, 1.0);
    smooth(limit - 0.08, limit, q.normal.y) * (0.75 + 0.25 * fbm(q.at * 0.6, 73))
}

/// Grime: in hollows and creases, where dirt washes and settles.
pub fn grime(q: &Probe) -> f32 {
    smooth(0.01, 0.06, q.hollow) * (0.6 + 0.4 * fbm(q.at * 0.8, 79))
}

/// Steep: where the land leans past about 30 degrees.
pub fn steep(q: &Probe) -> f32 {
    smooth(0.86, 0.7, q.normal.y)
}

/// The rules baked over the land: `n` texels a side, four channels each (mud, wet, moss,
/// snow; grime, steep).
pub struct Masks {
    pub n: usize,
    pub a: Vec<[u8; 4]>,
    pub b: Vec<[u8; 4]>,
}

/// A texel's middle.
fn texel(i: usize, j: usize) -> Vec2 {
    Vec2::new(
        -EXTENT + (i as f32 + 0.5) * TEXEL,
        -EXTENT + (j as f32 + 0.5) * TEXEL,
    )
}

/// Metres from each texel to the nearest under `level` (a chamfer distance, two passes).
fn to_water(land: &Land, n: usize, level: Option<f32>) -> Vec<f32> {
    let far = 1e4;
    let mut d: Vec<f32> = (0..n * n)
        .map(|k| match level {
            Some(l) if land.height(texel(k % n, k / n)) < l => 0.0,
            _ => far,
        })
        .collect();
    let (s, r) = (TEXEL, TEXEL * std::f32::consts::SQRT_2);
    let fwd: [(i32, i32, f32); 4] = [(-1, 0, s), (0, -1, s), (-1, -1, r), (1, -1, r)];
    let back: [(i32, i32, f32); 4] = [(1, 0, s), (0, 1, s), (1, 1, r), (-1, 1, r)];
    let pass = |d: &mut Vec<f32>, k: usize, steps: &[(i32, i32, f32); 4]| {
        let (i, j) = ((k % n) as i32, (k / n) as i32);
        for (di, dj, w) in steps {
            let (x, y) = (i + di, j + dj);
            if x >= 0 && y >= 0 && (x as usize) < n && (y as usize) < n {
                let v = d[y as usize * n + x as usize] + w;
                if v < d[k] {
                    d[k] = v;
                }
            }
        }
    };
    for k in 0..n * n {
        pass(&mut d, k, &fwd);
    }
    for k in (0..n * n).rev() {
        pass(&mut d, k, &back);
    }
    d
}

/// What the rules see at a point.
pub fn probe(place: &Place, at: Vec2, to_water: f32, shade: f32) -> Probe {
    let h = place.land.height(at);
    let e = 1.0;
    let round = [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y]
        .map(|o| place.land.height(at + o * e))
        .iter()
        .sum::<f32>()
        / 4.0;
    Probe {
        at,
        over: place.water.map_or(1e3, |l| h - l),
        normal: place.land.normal(at),
        to_water,
        hollow: round - h,
        shade,
    }
}

/// Bake the rules over the place: shade is asked only when a layer lies by moss.
pub fn bake(place: &Place, ground: &Ground, weather: Option<&Weather>) -> Masks {
    let n = (2.0 * EXTENT / TEXEL) as usize;
    let water = to_water(place.land, n, place.water);
    let wants_shade = ground.layers.iter().any(|l| l.mask == Mask::Moss);
    // The shade on a metre grid, looked up nearest: a crown's shade is metres across.
    let m = (2.0 * EXTENT) as usize + 1;
    let shade: Vec<f32> = if wants_shade {
        (0..m * m)
            .map(|k| (place.shade)(Vec2::new((k % m) as f32, (k / m) as f32) - EXTENT))
            .collect()
    } else {
        Vec::new()
    };
    let snow_amount = ground
        .layers
        .iter()
        .filter(|l| l.mask == Mask::Snow)
        .map(|l| l.amount)
        .fold(0.0, f32::max);
    let rain = weather.map_or(0.0, |w| w.rain);
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let (mut a, mut b) = (Vec::with_capacity(n * n), Vec::with_capacity(n * n));
    for k in 0..n * n {
        let at = texel(k % n, k / n);
        let s = if wants_shade {
            let g = (at + EXTENT).round();
            shade[(g.y as usize).min(m - 1) * m + (g.x as usize).min(m - 1)]
        } else {
            0.0
        };
        let q = probe(place, at, water[k], s);
        a.push([
            byte(mud(&q)),
            byte(wet(&q, rain)),
            byte(moss(&q)),
            byte(snow(&q, snow_amount)),
        ]);
        b.push([byte(grime(&q)), byte(steep(&q)), 0, 0]);
    }
    Masks { n, a, b }
}

fn image(n: usize, data: &[[u8; 4]]) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: n as u32,
            height: n as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data.iter().flatten().copied().collect(),
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    let mut sampler = ImageSamplerDescriptor::linear();
    sampler.set_address_mode(ImageAddressMode::ClampToEdge);
    image.sampler = ImageSampler::Descriptor(sampler);
    image
}

/// The ground's material for a place: the stack's scans, the biome's layers, the rules
/// baked over its land. A bad layer panics with what to fix.
pub fn material(
    asset: &AssetServer,
    images: &mut Assets<Image>,
    ground: &Ground,
    weather: Option<&Weather>,
    place: &Place,
) -> GroundMaterial {
    let params = params(ground, &place.sites).unwrap_or_else(|e| panic!("{e}"));
    let masks = bake(place, ground, weather);
    let load = |file: &'static str, srgb: bool| -> Handle<Image> {
        asset
            .load_builder()
            .with_settings(move |s: &mut ImageLoaderSettings| {
                s.array_layout = Some(ImageArrayLayout::RowCount {
                    rows: STACK.len() as u32,
                });
                s.is_srgb = srgb;
            })
            .load(file)
    };
    ExtendedMaterial {
        base: StandardMaterial {
            perceptual_roughness: 0.92,
            reflectance: 0.3,
            ..default()
        },
        extension: GroundExt {
            color: load("materials/ground/color.jpg", true),
            normal: load("materials/ground/normal.jpg", false),
            params,
            masks_a: images.add(image(masks.n, &masks.a)),
            masks_b: images.add(image(masks.n, &masks.b)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(over: f32, to_water: f32, n_y: f32, hollow: f32, shade: f32) -> Probe {
        let s = (1.0 - n_y * n_y).max(0.0).sqrt();
        Probe {
            at: Vec2::new(to_water * 3.1, over * 7.3 + hollow * 11.0),
            over,
            normal: Vec3::new(s, n_y, 0.0),
            to_water,
            hollow,
            shade,
        }
    }

    /// Probes over a spread of heights, distances, slopes, hollows and shade.
    fn spread() -> Vec<Probe> {
        let mut out = Vec::new();
        for over in [-0.3, 0.0, 0.05, 0.15, 0.3, 0.6, 1.5] {
            for d in [0.0, 0.3, 0.8, 1.4, 2.5, 6.0] {
                for ny in [0.2, 0.5, 0.75, 0.9, 1.0] {
                    for hollow in [-0.1, 0.0, 0.02, 0.1] {
                        for shade in [0.0, 0.5, 1.0] {
                            out.push(at(over, d, ny, hollow, shade));
                        }
                    }
                }
            }
        }
        out
    }

    /// Wet lies only near the water and low over it, and is wet at the water's line.
    #[test]
    fn wet_lies_only_near_water_and_low() {
        for q in spread() {
            if wet(&q, 0.0) > 0.02 {
                assert!(q.to_water < 1.4 && q.over < 0.22, "wet at {q:?}");
            }
        }
        assert!(wet(&at(0.02, 0.0, 1.0, 0.0, 0.0), 0.0) > 0.6);
        // Rain wets the open too, a hollow most.
        let open = at(1.5, 6.0, 1.0, 0.0, 0.0);
        let hollow = at(1.5, 6.0, 1.0, 0.1, 0.0);
        assert!(wet(&open, 1.0) > 0.3 && wet(&hollow, 1.0) > wet(&open, 1.0));
    }

    /// Mud reaches farther than wet but stays low by the water.
    #[test]
    fn mud_lies_low_by_the_water() {
        for q in spread() {
            if mud(&q) > 0.02 {
                assert!(q.to_water < 2.6 && q.over < 0.5, "mud at {q:?}");
            }
        }
        assert!(mud(&at(0.05, 0.3, 1.0, 0.0, 0.0)) > 0.5);
        assert!(mud(&at(-0.3, 0.0, 1.0, 0.0, 0.0)) > 0.9);
    }

    /// Snow lies only on what faces up, and more of it reaches steeper ground.
    #[test]
    fn snow_lies_only_on_what_faces_up() {
        for q in spread() {
            for amount in [0.0, 0.5, 1.0] {
                if q.normal.y < 0.6 {
                    assert_eq!(snow(&q, amount), 0.0, "snow on a wall at {q:?}");
                }
            }
        }
        assert!(snow(&at(1.0, 6.0, 1.0, 0.0, 0.0), 0.0) > 0.7);
        let slope = at(1.0, 6.0, 0.8, 0.0, 0.0);
        assert!(snow(&slope, 0.0) == 0.0 && snow(&slope, 1.0) > 0.7);
    }

    /// Grime settles only in hollows and creases, never on the flat or a crown.
    #[test]
    fn grime_lies_only_in_hollows() {
        for q in spread() {
            if grime(&q) > 0.0 {
                assert!(q.hollow > 0.01, "grime at {q:?}");
            }
        }
        assert!(grime(&at(1.0, 6.0, 1.0, 0.1, 0.0)) > 0.5);
    }

    /// Moss grows only in shade or a crease, not on the steep or under water.
    #[test]
    fn moss_lies_only_in_shade_or_creases() {
        for q in spread() {
            if moss(&q) > 0.0 {
                assert!(
                    q.shade > 0.0 || q.hollow > 0.01,
                    "moss in the open at {q:?}"
                );
                assert!(q.normal.y > 0.55 && q.over > -0.05, "moss at {q:?}");
            }
        }
        assert!(moss(&at(1.0, 6.0, 1.0, 0.0, 1.0)) > 0.8);
    }

    /// Baked over a land with a pond, the bank is wet at the water's line and nowhere far
    /// from it; a land without water has none.
    #[test]
    fn the_bake_wets_a_ponds_bank_only() {
        let bowl = |p: Vec2| 0.02 * p.length_squared() - 0.5;
        let land = Land::from_shape(bowl);
        let shade = |_: Vec2| 0.0;
        let mut place = Place {
            land: &land,
            sites: ["a", "b", "c", "d", "e", "f"],
            water: Some(0.0),
            shade: &shade,
        };
        let ground = Ground::default();
        let m = bake(&place, &ground, None);
        let shore = 0.5f32 / 0.02;
        let mut wet_at_line = 0;
        for k in 0..m.n * m.n {
            let r = texel(k % m.n, k / m.n).length();
            if m.a[k][1] > 5 {
                assert!(r < shore.sqrt() + 1.4, "wet {r:.2} m out, the shore at 5 m");
            }
            if (r - shore.sqrt()).abs() < 0.2 && m.a[k][1] > 150 {
                wet_at_line += 1;
            }
        }
        assert!(
            wet_at_line > 50,
            "the water's line is wet at {wet_at_line} texels"
        );
        place.water = None;
        let dry = bake(&place, &ground, None);
        assert!(dry.a.iter().all(|t| t[0] == 0 && t[1] == 0));
    }

    /// The forest's and the marsh's grounds resolve against the glade's maps; a misspelt map
    /// or material says what to use.
    #[test]
    fn the_biomes_grounds_resolve() {
        for name in ["forest", "marsh"] {
            let b = super::super::biome::Biome::load(name).unwrap();
            params(&b.ground, &crate::terrain::SITES).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        let mut g = Ground::default();
        g.layers.push(super::super::biome::Layer {
            material: Some("leafy_gras".into()),
            mask: Mask::Site("gras".into()),
            amount: 1.0,
            metres: None,
            tint: (1.0, 1.0, 1.0),
            breakup: None,
            wets: 0.0,
            triplanar: false,
        });
        let e = params(&g, &crate::terrain::SITES).unwrap_err();
        assert!(e.contains("leafy_grass") && e.contains("Fix"), "{e}");
        g.layers[0].material = None;
        let e = params(&g, &crate::terrain::SITES).unwrap_err();
        assert!(e.contains("grass, leaves") && e.contains("Fix"), "{e}");
    }
}
