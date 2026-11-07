//! The kit's street (layout `Street`, D634): a road running east and west through the origin
//! between two kerbs, a pavement each side, and building fronts beyond them, all as the
//! biome's `street` says. The road and the kerbs are laid floors (kit/laid.rs: a scan at its
//! true size, weathered per pixel), the pavements are the biome's `ground.laid`; the fronts
//! are blocks of a scan with storeys of windows, some lit; street lamps hang sodium spots
//! over the road; signs are emissive cards, each with a coloured light so it lights the
//! ground; props stand by the kerb, the wall or out on the road. Nothing here is a genre's:
//! a neon city at night and a dusty frontier town are the same fields.
//!
//! North is -z, so the camera (south of the street, looking north) sees the north fronts'
//! faces and the south fronts' roofs. The paving carries three maps for the laid floors
//! (`SITES`): `gutter` (by the kerb's face, where the road's camber runs the rain),
//! `puddled` (low patches where water stands) and `walls` (where a wall or kerb rises).
//! The biome's `water` stands in the `puddled` patches as the kit's water (kit/water.rs),
//! its `level` over the road and the pavements: the mirror and the lights' streaks.

use super::biome::{Biome, Laid};
use super::contact::Grounded;
use super::laid::{LaidMaterial, Place};
use super::light::Colour;
use super::water::{Glint, KitWater};
use crate::Args;
use crate::forest::Rng;
use crate::scene::{material, model};
use crate::terrain::{fbm, smooth};
use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use serde::Deserialize;

/// The street's maps for the laid floors, in the second UV's x and y and the colour's alpha.
pub const SITES: [&str; 3] = ["gutter", "puddled", "walls"];
/// The paving's mesh spacing, metres.
const CELL: f32 = 0.25;

/// A street: the road's width between the kerbs' faces, each pavement's, the kerb's width
/// and rise, and how long it all runs (centred on the origin, along x).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Street {
    pub road: f32,
    pub walk: f32,
    pub kerb: (f32, f32),
    pub length: f32,
    /// The road's surface (asphalt: a laid scan with no joints).
    pub asphalt: Laid,
    /// The kerbstones' surface.
    pub kerbs: Laid,
    /// Where the camera looks, (x, z): the fight's middle.
    #[serde(default)]
    pub focus: (f32, f32),
    /// After rain, how deep (m) a film of the biome's water lies over all the paving between
    /// the puddles (0: only the puddles), mirroring at the shore's fade of that depth.
    #[serde(default)]
    pub film: f32,
    /// Where water stands: the patches' noise (0 to 1) over which a puddle lies, so lower
    /// is more puddles (0.64 about a tenth of the street; iso-kit j46).
    #[serde(default = "puddle_level")]
    pub puddles: f32,
    /// Wires strung across and along the street and pipes down its fronts (kit/cables.rs).
    #[serde(default)]
    pub cables: Vec<super::cables::Cable>,
    #[serde(default)]
    pub fronts: Vec<Front>,
    #[serde(default)]
    pub lamps: Option<Lamps>,
    #[serde(default)]
    pub signs: Vec<Sign>,
    #[serde(default)]
    pub props: Vec<Prop>,
}

/// Which side of the road: north (-z, its fronts face the camera) or south.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    North,
    South,
}

impl Side {
    /// Its sign along z.
    pub fn z(self) -> f32 {
        match self {
            Side::North => -1.0,
            Side::South => 1.0,
        }
    }
    /// The turn about y that faces a thing on this side toward the road (models face +z).
    pub(crate) fn facing(self) -> f32 {
        match self {
            Side::North => 0.0,
            Side::South => std::f32::consts::PI,
        }
    }
}

/// A block of building along one side, from x `along.0` to `along.1`, `height` tall and
/// `depth` deep, faced in a scan of `assets/materials/` at its true `metres`; its roof a scan
/// too (the front's own when `None`). Its face to the road is either the scan with `windows`
/// or a facade kit's `bays` (kit/facade.rs), the block's face then set back dark behind them.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Front {
    pub side: Side,
    pub along: (f32, f32),
    pub height: f32,
    #[serde(default = "deep")]
    pub depth: f32,
    pub material: String,
    pub metres: f32,
    #[serde(default = "white")]
    pub tint: (f32, f32, f32),
    #[serde(default)]
    pub roof: Option<String>,
    #[serde(default)]
    pub windows: Option<Windows>,
    #[serde(default)]
    pub bays: Option<super::facade::Bays>,
}

/// A front's windows: a storey's height, the first sill's height, one every `every` metres
/// along, each `size` (wide, tall); `lit` of them glowing in one of `colours` at `nits`, the
/// rest dark glass.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Windows {
    pub storey: f32,
    pub from: f32,
    pub every: f32,
    pub size: (f32, f32),
    pub lit: f32,
    pub colours: Vec<Colour>,
    pub nits: f32,
}

/// Street lamps over the road: one every `every` metres from x `start`, sides taking turns
/// from the north, a pole `height` tall by the kerb with an arm `reach` out over the road,
/// a spot of `colour` and `lumens` down through `angle` degrees, `range` metres.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Lamps {
    pub every: f32,
    pub start: f32,
    pub height: f32,
    pub reach: f32,
    pub colour: Colour,
    pub lumens: f32,
    pub angle: f32,
    pub range: f32,
    #[serde(default)]
    pub shadows: bool,
}

/// A sign: a texture (under `assets/`) on a card `size` metres (wide, tall), its middle at x
/// `at` and `height` up a front of `side`, flat on the wall or a `blade` standing out from
/// it; it glows at `nits` and lights the street around it in `colour` at `lumens`, out to
/// `range` metres (a short one makes a pool that falls off to dark), its light `shadows`
/// cast or not. With a `pool` (cone degrees, metres out from the wall, lumens) its light on
/// the street is a spot from the sign aimed down at the pavement that far out, and `lumens`
/// and `range` only its glow on the wall about it, so the face falls dark above and beside
/// it instead of taking the sign's colour whole (iso-kit j42: "neon floods the face").
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Sign {
    pub texture: String,
    pub side: Side,
    pub at: f32,
    pub height: f32,
    pub size: (f32, f32),
    #[serde(default)]
    pub blade: bool,
    pub colour: Colour,
    pub nits: f32,
    pub lumens: f32,
    #[serde(default = "nine")]
    pub range: f32,
    #[serde(default)]
    pub shadows: bool,
    #[serde(default)]
    pub pool: Option<(f32, f32, f32)>,
}

fn nine() -> f32 {
    9.0
}

fn puddle_level() -> f32 {
    0.64
}

/// Where a prop stands across the street: by the kerb on the pavement, against the wall, or
/// on the road `out` metres from its middle toward its side.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    Kerb,
    Wall,
    Road,
}

/// A model (under `assets/`) at each x of `at` on `side`'s `line`, moved `out` metres away
/// from the road (into it when negative; from the middle on `Road`), turned `turn` degrees
/// from facing the road, scaled `scale`; a rigged one (a person on the pavement) held in its
/// `pose`, a clip of its GLB by name and the time in it (the ruin's `Pose`), and edged by
/// `rim` times the fighters' cold rim band (rim.rs; none at 0), so a dark figure reads off
/// the dark wet street as the ARPGs lift theirs. A `stroll`ing one walks on down the
/// pavement the way it faces through a film (ruin/walk.rs), its pose its walk; a still holds it.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Prop {
    pub model: String,
    pub side: Side,
    pub line: Line,
    pub at: Vec<f32>,
    #[serde(default)]
    pub out: f32,
    #[serde(default)]
    pub turn: f32,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default)]
    pub pose: Option<(String, f32)>,
    #[serde(default)]
    pub rim: f32,
    #[serde(default)]
    pub stroll: bool,
    /// Metres up from its line's floor: a wall-hung thing (an air conditioner under a
    /// window, a payphone, a lamp over a door), with no contact shadow under it.
    #[serde(default)]
    pub up: f32,
}

fn deep() -> f32 {
    12.0
}

fn white() -> (f32, f32, f32) {
    (1.0, 1.0, 1.0)
}

fn one() -> f32 {
    1.0
}

impl Street {
    /// Half the road, the kerb's outer edge, and the wall line (the pavements' outer edge).
    pub fn edges(&self) -> (f32, f32, f32) {
        let half = self.road / 2.0;
        (half, half + self.kerb.0, half + self.kerb.0 + self.walk)
    }

    /// Where a prop's foot stands on `side`'s `line`, `out` metres out.
    pub fn spot(&self, side: Side, line: Line, x: f32, out: f32) -> Vec3 {
        let (_, kerb, wall) = self.edges();
        let (across, y) = match line {
            Line::Kerb => (kerb + 0.4 + out, self.kerb.1),
            Line::Wall => (wall - 0.35 - out, self.kerb.1),
            Line::Road => (out, 0.0),
        };
        Vec3::new(x, y, side.z() * across)
    }

    /// The street as laid out, or what is wrong and the fix (D556): every sign on a front of
    /// its side and inside it, every prop inside the street's length and off the road unless
    /// it stands on it.
    pub fn check(&self) -> Result<(), String> {
        let ends = self.length / 2.0;
        for f in &self.fronts {
            let Some(b) = &f.bays else { continue };
            if f.windows.is_some() {
                return Err(format!(
                    "the front from x {} to {} has both `windows` and `bays`: its face is one \
                     or the other. Fix: drop its `windows` (the bays' `lit` and `colours` light \
                     its rooms; kit/street.rs, D634)",
                    f.along.0, f.along.1
                ));
            }
            b.check(f.along, f.height)?;
        }
        for s in &self.signs {
            let (w, h) = (s.size.0 / 2.0, s.size.1 / 2.0);
            let on = self.fronts.iter().any(|f| {
                f.side == s.side
                    && f.along.0 <= s.at - w.min(0.3)
                    && s.at + w.min(0.3) <= f.along.1
                    && s.height + h <= f.height
                    && s.height - h >= 0.0
            });
            if !on {
                return Err(format!(
                    "sign {:?} at x {} and {} m up on the {:?} side hangs on no front: each sign \
                     goes on a front of its side, inside its span and under its height. Fix: \
                     move its `at` or `height`, or add a front there (kit/street.rs, D634)",
                    s.texture, s.at, s.height, s.side
                ));
            }
        }
        for p in &self.props {
            for x in &p.at {
                let far = p.line == Line::Road && p.out.abs() > self.road / 2.0;
                if x.abs() > ends || far {
                    return Err(format!(
                        "prop {:?} at x {x} (out {}) lies off the street ({} m long, the road \
                         {} m wide). Fix: an x within ±{ends} and a road prop's out within \
                         ±{} (kit/street.rs, D634)",
                        p.model,
                        p.out,
                        self.length,
                        self.road,
                        self.road / 2.0
                    ));
                }
            }
        }
        for c in &self.cables {
            c.check()?;
        }
        Ok(())
    }

    /// How near the kerb's face a point of the road is: 1 at the face, gone 0.9 m out, its
    /// edge ragged; 0 off the road.
    pub fn gutter(&self, p: Vec2) -> f32 {
        let (half, _, _) = self.edges();
        if p.y.abs() > half {
            return 0.0;
        }
        let d = half - p.y.abs() + (fbm(p * 0.7, 401) - 0.5) * 0.5;
        smooth(0.9, 0.05, d)
    }

    /// Where water stands: low patches over the street, more of them in the gutters.
    pub fn puddled(&self, p: Vec2) -> f32 {
        let patch = fbm(p * 0.32 + Vec2::splat(17.0), 402);
        let lower = 0.08 * self.gutter(p);
        smooth(self.puddles, self.puddles + 0.06, patch + lower)
    }

    /// How deep water stands at a point (m), as the water's face reads it: up to 6 cm in a
    /// puddle's middle, 0 at its edge (`puddled` a half), the `film` between them; with no
    /// film under the face's cut (5 cm) on the dry, so no face is drawn there.
    pub fn standing(&self, p: Vec2) -> f32 {
        let puddle = (self.puddled(p) - 0.5) * 0.12;
        if self.film > 0.0 {
            puddle.max(self.film)
        } else {
            puddle
        }
    }

    /// How near a rising wall a point is: a front's foot on the pavements, the kerb's face on
    /// the road; 1 at the foot, gone a metre out.
    pub fn walls(&self, p: Vec2) -> f32 {
        let (half, kerb, wall) = self.edges();
        let a = p.y.abs();
        let d = if a <= half {
            half - a
        } else if a >= kerb {
            wall - a
        } else {
            9.0
        };
        smooth(1.0, 0.0, d + (fbm(p * 0.9, 403) - 0.5) * 0.4)
    }
}

/// A mesh being built of flat sheets, each a grid over a rectangle, carrying the laid
/// floor's maps.
#[derive(Default)]
struct Sheets {
    pos: Vec<[f32; 3]>,
    normal: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    sites: Vec<[f32; 2]>,
    colour: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl Sheets {
    /// A sheet from `o` along `u` and `v` (its sides), in cells about `CELL` across, its
    /// texture at `metres` along the sheet's own axes, `maps` giving each vertex's maps.
    fn add(&mut self, o: Vec3, u: Vec3, v: Vec3, metres: f32, maps: &dyn Fn(Vec3) -> [f32; 4]) {
        let n = u.cross(v).normalize();
        let (nu, nv) = (
            (u.length() / CELL).ceil().max(1.0) as u32,
            (v.length() / CELL).ceil().max(1.0) as u32,
        );
        let (du, dv) = (u.normalize(), v.normalize());
        let base = self.pos.len() as u32;
        for j in 0..=nv {
            for i in 0..=nu {
                let p = o + u * (i as f32 / nu as f32) + v * (j as f32 / nv as f32);
                self.pos.push(p.to_array());
                self.normal.push(n.to_array());
                self.uv.push([p.dot(du) / metres, -p.dot(dv) / metres]);
                let m = maps(p);
                self.sites.push([m[0], m[1]]);
                let g = 0.82 * (0.9 + 0.2 * fbm(Vec2::new(p.x, p.z) * 0.6, 404));
                self.colour.push(
                    Color::srgba(g, g * 0.98, g * 0.96, m[2])
                        .to_linear()
                        .to_f32_array(),
                );
            }
        }
        let row = nu + 1;
        for j in 0..nv {
            for i in 0..nu {
                let a = base + j * row + i;
                // Counter-clockwise seen from the normal's side: u then v.
                self.idx
                    .extend([a, a + 1, a + row + 1, a, a + row + 1, a + row]);
            }
        }
    }

    fn mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normal)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.sites)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colour)
        .with_inserted_indices(Indices::U32(self.idx))
        .with_generated_tangents()
        .expect("a street sheet has normals and UVs")
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins((
        super::laid::plugin,
        super::water::plugin,
        super::scatter::plugin,
        super::weather::plugin,
        super::facade::plugin,
        super::contact::plugin,
        crate::ruin::walk::plugin,
    ))
    .add_systems(Startup, (spawn, strew))
    // Rigged props stand in their clips (the ruin's poser).
    .add_systems(Update, crate::ruin::fight::pose);
}

/// How far down the street the scatter is strewn each side of the focus (m): the frame and
/// a margin.
const STREWN: f32 = 18.0;

/// The biome's scatter over the street (kit/scatter.rs): on its maps (`SITES`), the road at
/// 0 and the pavements the kerb's rise up, kept off each prop's foot.
fn strew(
    mut commands: Commands,
    asset: Res<AssetServer>,
    biome: Res<Biome>,
    mut budget: ResMut<super::scatter::Budget>,
) {
    let Some(st) = biome.street.as_ref() else {
        return;
    };
    let (half, _, wall) = st.edges();
    let site = |k: usize, p: Vec2| [st.gutter(p), st.puddled(p), st.walls(p)][k];
    let height = |p: Vec2| if p.y.abs() <= half { 0.0 } else { st.kerb.1 };
    let feet: Vec<Vec2> = st
        .props
        .iter()
        .flat_map(|p| p.at.iter().map(|x| st.spot(p.side, p.line, *x, p.out).xz()))
        .collect();
    let keep = |p: Vec2| feet.iter().any(|f| f.distance(p) < 0.7);
    // The weather's floor (kit/weather.rs): the road and the pavements, no standing water.
    commands.insert_resource(super::weather::Floor::sample(
        Vec2::new(st.focus.0 - STREWN, -wall),
        Vec2::new(st.focus.0 + STREWN, wall),
        0.25,
        None,
        height,
    ));
    let g = super::scatter::Ground {
        sites: &SITES,
        site: &site,
        height: &height,
        water: None,
        rules: None,
        keep: &keep,
        span: (
            Vec2::new(st.focus.0 - STREWN, -wall),
            Vec2::new(st.focus.0 + STREWN, wall),
        ),
    };
    let strewn = super::scatter::spawn(
        &mut commands,
        &asset,
        &mut budget,
        &biome.refs,
        &biome.scatter,
        &g,
    );
    // Each piece of litter on its own contact shadow and stain (kit/contact.rs).
    for (e, at) in strewn {
        let floor = height(at.xz());
        commands.entity(e).insert(Grounded { floor });
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    args: Res<Args>,
    biome: Res<Biome>,
    light: Res<super::light::Light>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut laid: ResMut<Assets<LaidMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut waters: ResMut<Assets<KitWater>>,
) {
    fn stop(refs: &str, e: String) -> ! {
        panic!("biome {refs:?}: {e}")
    }
    let refs = biome.refs.clone();
    let st = biome.street.clone().unwrap_or_else(|| {
        stop(
            &refs,
            "its layout is Street but it has no `street: (...)`. Fix: add one as \
              biomes/neon-street.ron does (kit/street.rs, D634)"
                .into(),
        )
    });
    st.check().unwrap_or_else(|e| stop(&refs, e));
    let walk = biome.ground.laid.clone().unwrap_or_else(|| {
        stop(
            &refs,
            "a street's pavements are its `ground.laid`. Fix: give its ground a \
              `laid: (material: ..., metres: ...)` (kit/laid.rs, D634)"
                .into(),
        )
    });

    // The camera and the light: the ARPG's view on the focus, the preset's key and fill.
    let focus = Vec3::new(st.focus.0, 1.2, st.focus.1);
    let cam = crate::ruin::camera::arpg(&mut commands, &args, &mut images, focus, light.ev100);
    light.camera(&mut commands, cam, crate::ruin::camera::DISTANCE);
    light.spawn(&mut commands);

    // The paving: road, kerbs and pavements, each a laid floor over the same maps.
    let place = Place { sites: SITES };
    let maps = |p: Vec3| {
        let q = Vec2::new(p.x, p.z);
        [st.gutter(q), st.puddled(q), st.walls(q), 0.0]
    };
    let (half, kerb, wall) = st.edges();
    let (l, rise) = (st.length, st.kerb.1);
    let x0 = -l / 2.0;
    let mut road = Sheets::default();
    road.add(
        Vec3::new(x0, 0.0, half),
        Vec3::X * l,
        Vec3::Z * -st.road,
        st.asphalt.metres,
        &maps,
    );
    let (mut kerbs, mut walks) = (Sheets::default(), Sheets::default());
    for side in [Side::North, Side::South] {
        let s = side.z();
        // Each sheet's u then v runs so its normal faces up, or toward the road.
        // A band from |z| p to q, its origin the larger z and v toward -z, so it faces up.
        let band = |p: f32, q: f32| {
            (
                Vec3::new(x0, rise, if s < 0.0 { -p } else { q }),
                Vec3::Z * -(q - p),
            )
        };
        let (o, v) = band(half, kerb);
        kerbs.add(o, Vec3::X * l, v, st.kerbs.metres, &maps);
        let face = Vec3::new(x0, 0.0, s * half);
        if s < 0.0 {
            kerbs.add(face, Vec3::X * l, Vec3::Y * rise, st.kerbs.metres, &maps);
        } else {
            kerbs.add(
                face + Vec3::X * l,
                Vec3::X * -l,
                Vec3::Y * rise,
                st.kerbs.metres,
                &maps,
            );
        }
        let (o, v) = band(kerb, wall);
        walks.add(o, Vec3::X * l, v, walk.metres, &maps);
    }
    for (sheets, floor) in [(road, &st.asphalt), (kerbs, &st.kerbs), (walks, &walk)] {
        commands.spawn((
            Mesh3d(meshes.add(sheets.mesh())),
            MeshMaterial3d(laid.add(super::laid::material(&asset, floor, &place))),
        ));
    }

    // The puddles: the biome's water over the road and each pavement where `puddled` says.
    // One mirror serves every face, at the first's level (the road's): the pavements' sit
    // the kerb's rise over it, a few centimetres' error in what they mirror.
    if let Some(water) = &biome.water {
        let wind = biome
            .weather
            .as_ref()
            .map_or(Vec2::ZERO, |w| Vec2::new(w.wind.0, w.wind.1));
        for (z0, z1, floor) in [(-half, half, 0.0), (-wall, -kerb, rise), (kerb, wall, rise)] {
            let level = floor + water.level;
            let face =
                super::water::surface(Vec2::new(x0, z0), Vec2::new(-x0, z1), CELL, level, |p| {
                    level - st.standing(p)
                });
            super::water::spawn_face(
                &mut commands,
                &mut meshes,
                &mut waters,
                water,
                wind,
                Some(&*light),
                face,
            );
        }
    }

    fronts(&mut commands, &asset, &mut meshes, &mut materials, &st);
    super::cables::spawn(&mut commands, &mut meshes, &mut materials, &st.cables);
    lamps(&mut commands, &mut meshes, &mut materials, &st);
    signs(&mut commands, &asset, &mut meshes, &mut materials, &st);
    for p in &st.props {
        for x in &p.at {
            let at = st.spot(p.side, p.line, *x, p.out) + Vec3::Y * p.up;
            let yaw = p.side.facing() + p.turn.to_radians();
            let body = model(&mut commands, &asset, &p.model, at, yaw, p.scale);
            // A rigged body's bounds are its bind pose's, a square far wider than its feet
            // (j40's walker stood in a 1.5 m black square): a still prop is grounded by its
            // bounds, a posed body by its feet (kit/contact.rs); a hung one not at all.
            if p.up > 0.0 {
                // Hung on the wall: nothing on the floor under it.
            } else if p.pose.is_none() {
                commands.entity(body).insert(Grounded { floor: at.y });
            } else {
                commands
                    .entity(body)
                    .insert(super::contact::Feet { floor: at.y });
            }
            if p.rim > 0.0 {
                commands
                    .entity(body)
                    .insert(crate::rim::Fighter::valued(1.0, p.rim));
            }
            if let Some((clip, time)) = &p.pose {
                // The pose names its clip for the life of the app: a few names, once, at
                // startup.
                commands.entity(body).insert(crate::ruin::fight::Pose {
                    gltf: asset.load(p.model.clone()),
                    clip: Box::leak(clip.clone().into_boxed_str()),
                    at: *time,
                    once: false,
                });
            }
            if p.stroll {
                // Along the street (x) the way it faces, past the film's end.
                let way = Vec2::X * yaw.sin().signum();
                commands.entity(body).insert(crate::ruin::walk::WalkIn {
                    from: at.xz(),
                    to: at.xz() + way * 30.0,
                    floor: crate::ruin::walk::Floor::Flat(at.y),
                    fight: false,
                });
            }
        }
    }
}

/// The fronts: each a block of its scan with a roof, and its storeys of windows.
fn fronts(
    commands: &mut Commands,
    asset: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    st: &Street,
) {
    let (_, _, wall) = st.edges();
    let flat = |_: Vec3| [0.0, 0.0, 0.0, 0.0];
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgb(0.02, 0.025, 0.03),
        perceptual_roughness: 0.08,
        reflectance: 0.6,
        ..default()
    });
    let pane = meshes.add(Rectangle::new(1.0, 1.0));
    let mut rng = Rng::new(0x5_7ee7);
    for f in &st.fronts {
        let s = f.side.z();
        let (x0, x1, h) = (f.along.0, f.along.1, f.height);
        let (near, back) = (s * wall, s * (wall + f.depth));
        let mut faces = Sheets::default();
        // The face to the road (set back dark behind a facade kit's bays), then its ends
        // from the wall line; each wound so its normal looks out.
        let (z_a, z_b) = if s < 0.0 { (near, back) } else { (back, near) };
        let mut inner = Sheets::default();
        let (road, set) = match &f.bays {
            Some(b) => (&mut inner, s * (wall + b.back)),
            None => (&mut faces, near),
        };
        if s < 0.0 {
            road.add(
                Vec3::new(x0, 0.0, set),
                Vec3::X * (x1 - x0),
                Vec3::Y * h,
                f.metres,
                &flat,
            );
        } else {
            road.add(
                Vec3::new(x1, 0.0, set),
                Vec3::X * (x0 - x1),
                Vec3::Y * h,
                f.metres,
                &flat,
            );
        }
        faces.add(
            Vec3::new(x0, 0.0, z_b),
            Vec3::Z * (z_a - z_b),
            Vec3::Y * h,
            f.metres,
            &flat,
        );
        faces.add(
            Vec3::new(x1, 0.0, z_a),
            Vec3::Z * (z_b - z_a),
            Vec3::Y * h,
            f.metres,
            &flat,
        );
        let (r, g, b) = f.tint;
        commands.spawn((
            Mesh3d(meshes.add(faces.mesh())),
            MeshMaterial3d(materials.add(material(asset, &f.material, Color::srgb(r, g, b)))),
        ));
        let mut roof = Sheets::default();
        roof.add(
            Vec3::new(x0, h, z_a),
            Vec3::X * (x1 - x0),
            Vec3::Z * (z_b - z_a),
            3.0,
            &flat,
        );
        let top = f.roof.as_deref().unwrap_or(&f.material);
        commands.spawn((
            Mesh3d(meshes.add(roof.mesh())),
            MeshMaterial3d(materials.add(material(asset, top, Color::srgb(0.45, 0.45, 0.45)))),
        ));
        if let Some(b) = &f.bays {
            // The rooms behind the openings: near black, rough.
            commands.spawn((
                Mesh3d(meshes.add(inner.mesh())),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.015, 0.014, 0.013),
                    perceptual_roughness: 0.95,
                    ..default()
                })),
            ));
            super::facade::spawn(
                commands, asset, meshes, materials, f.side, f.along, h, wall, b, &mut rng,
            );
            continue;
        }
        let Some(w) = &f.windows else { continue };
        let lit: Vec<Handle<StandardMaterial>> = w
            .colours
            .iter()
            .map(|c| {
                materials.add(StandardMaterial {
                    base_color: Color::BLACK,
                    emissive: c.color().to_linear() * w.nits,
                    perceptual_roughness: 0.2,
                    ..default()
                })
            })
            .collect();
        let mut y = w.from + w.size.1 / 2.0;
        while y + w.size.1 / 2.0 < h - 0.5 {
            let mut x = x0 + w.every / 2.0;
            while x < x1 - w.every / 2.0 + 0.01 {
                let on = rng.f() < w.lit && !lit.is_empty();
                let m = if on {
                    lit[(rng.f() * lit.len() as f32) as usize % lit.len()].clone()
                } else {
                    glass.clone()
                };
                commands.spawn((
                    Mesh3d(pane.clone()),
                    MeshMaterial3d(m),
                    Transform::from_xyz(x, y, near - s * 0.02)
                        .with_rotation(Quat::from_rotation_y(f.side.facing()))
                        .with_scale(Vec3::new(w.size.0, w.size.1, 1.0)),
                ));
                x += w.every;
            }
            y += w.storey;
        }
    }
}

/// The street lamps: a dark pole by the kerb, an arm over the road, a glowing head and its
/// spot straight down.
fn lamps(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    st: &Street,
) {
    let Some(lp) = &st.lamps else { return };
    let (_, kerb, _) = st.edges();
    let steel = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.085, 0.09),
        metallic: 0.7,
        perceptual_roughness: 0.45,
        ..default()
    });
    let glow = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: lp.colour.color().to_linear() * 400.0,
        ..default()
    });
    let pole = meshes.add(Cylinder::new(0.07, lp.height));
    let arm = meshes.add(Cuboid::new(0.06, 0.06, lp.reach));
    let head = meshes.add(Cuboid::new(0.32, 0.08, 0.6));
    let mut x = lp.start;
    let mut k = 0;
    while x <= st.length / 2.0 {
        let side = if k % 2 == 0 { Side::North } else { Side::South };
        let s = side.z();
        let foot = Vec3::new(x, st.kerb.1, s * (kerb + 0.35));
        commands.spawn((
            Mesh3d(pole.clone()),
            MeshMaterial3d(steel.clone()),
            Transform::from_translation(foot + Vec3::Y * lp.height / 2.0),
        ));
        let top = foot + Vec3::Y * lp.height;
        commands.spawn((
            Mesh3d(arm.clone()),
            MeshMaterial3d(steel.clone()),
            Transform::from_translation(top - Vec3::Z * s * lp.reach / 2.0),
        ));
        let lamp = top - Vec3::Z * s * lp.reach - Vec3::Y * 0.06;
        commands.spawn((
            Mesh3d(head.clone()),
            MeshMaterial3d(glow.clone()),
            Transform::from_translation(lamp),
        ));
        commands.spawn((
            SpotLight {
                color: lp.colour.color(),
                intensity: lp.lumens,
                range: lp.range,
                radius: 0.15,
                inner_angle: lp.angle.to_radians() * 0.55,
                outer_angle: lp.angle.to_radians(),
                shadow_maps_enabled: lp.shadows,
                ..default()
            },
            Transform::from_translation(lamp - Vec3::Y * 0.1)
                .looking_at(lamp - Vec3::Y * 5.0, Vec3::Z),
            Glint,
        ));
        x += lp.every;
        k += 1;
    }
}

/// The signs: a dark box on the wall, its face the texture glowing, and a light of its
/// colour half a metre out so it lights the wet ground.
fn signs(
    commands: &mut Commands,
    asset: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    st: &Street,
) {
    let (_, _, wall) = st.edges();
    let backing = materials.add(StandardMaterial {
        base_color: Color::srgb(0.03, 0.03, 0.035),
        metallic: 0.5,
        perceptual_roughness: 0.5,
        ..default()
    });
    for sg in &st.signs {
        let s = sg.side.z();
        let (w, h) = sg.size;
        let face = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::rgb(sg.nits, sg.nits, sg.nits),
            emissive_texture: Some(asset.load(sg.texture.clone())),
            perceptual_roughness: 0.3,
            ..default()
        });
        // Out from the wall toward the road, and the card's turn: facing the road, or
        // edge-on to it (a blade, seen from up and down the street). A flat sign's box runs
        // from the wall to its card, out past a facade's frames (0.2 m) and under its crown.
        let (out, turn, depth) = if sg.blade {
            (
                0.15 + w / 2.0,
                std::f32::consts::FRAC_PI_2,
                Vec3::new(0.08, h, w),
            )
        } else {
            (0.25, sg.side.facing(), Vec3::new(w, h, 0.24))
        };
        let mid = Vec3::new(sg.at, sg.height, s * (wall - out));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(depth.x, depth.y, depth.z))),
            MeshMaterial3d(backing.clone()),
            Transform::from_translation(mid + Vec3::Z * s * 0.125 * (!sg.blade as u8 as f32)),
            // Its own light sits in it (a blade's) or just before it: a shadowed one would
            // black out its own pool, as a brazier's bowl did (traps.md).
            NotShadowCaster,
        ));
        let to_road = Vec3::Z * -s;
        // A blade shows a card on each face of its box, a flat sign one toward the road.
        let card = meshes.add(Rectangle::new(w, h));
        let cards: Vec<Transform> = if sg.blade {
            [1.0f32, -1.0]
                .map(|k| {
                    Transform::from_translation(mid + Vec3::X * k * 0.045)
                        .with_rotation(Quat::from_rotation_y(turn * k))
                })
                .to_vec()
        } else {
            vec![
                Transform::from_translation(mid + to_road * 0.005)
                    .with_rotation(Quat::from_rotation_y(turn)),
            ]
        };
        for t in cards {
            commands.spawn((
                Mesh3d(card.clone()),
                MeshMaterial3d(face.clone()),
                t,
                NotShadowCaster,
            ));
        }
        let lamp = mid + to_road * if sg.blade { 0.4 } else { 0.6 };
        let mut glow = commands.spawn((
            PointLight {
                color: sg.colour.color(),
                intensity: sg.lumens,
                range: sg.range,
                radius: (w.max(h) * 0.3).min(0.6),
                shadow_maps_enabled: sg.shadows && sg.pool.is_none(),
                ..default()
            },
            Transform::from_translation(lamp),
        ));
        let Some((cone, reach, lumens)) = sg.pool else {
            glow.insert(Glint);
            continue;
        };
        // The pool: down from the sign onto the pavement `reach` out from the wall, its
        // cone's edge grazing the wall a little under the sign, so the face lights only
        // below it and falls off with the distance (the ARPGs' pool under a light).
        let foot = Vec3::new(sg.at, st.kerb.1, s * (wall - reach));
        commands.spawn((
            SpotLight {
                color: sg.colour.color(),
                intensity: lumens,
                range: (lamp - foot).length() + 3.0,
                radius: (w.max(h) * 0.3).min(0.6),
                inner_angle: cone.to_radians() * 0.5,
                outer_angle: cone.to_radians(),
                shadow_maps_enabled: sg.shadows,
                ..default()
            },
            Transform::from_translation(lamp).looking_at(foot, Vec3::X),
            Glint,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn neon() -> Street {
        Biome::load("neon-street").unwrap().street.unwrap()
    }

    /// The neon street lays out: its signs hang on fronts, its props lie on the street.
    #[test]
    fn the_neon_street_lays_out() {
        neon().check().unwrap_or_else(|e| panic!("{e}"));
    }

    /// A sign off every front fails, naming the sign and the fix.
    #[test]
    fn a_sign_off_the_fronts_says_the_fix() {
        let mut st = neon();
        st.signs[0].at = st.length * 4.0;
        let e = st.check().unwrap_err();
        assert!(
            e.contains(&st.signs[0].texture) && e.contains("Fix:"),
            "{e}"
        );
    }

    /// The gutter lies along the kerb's face and nowhere near the road's middle or on the
    /// pavement; the walls' map at the fronts' feet and the kerb's, not mid-pavement.
    #[test]
    fn gutters_and_wall_feet_lie_where_they_rise() {
        let st = neon();
        let (half, kerb, wall) = st.edges();
        for x in [-9.0, -2.5, 0.0, 4.0, 11.0] {
            for s in [-1.0, 1.0] {
                assert!(st.gutter(Vec2::new(x, s * (half - 0.05))) > 0.6);
                assert_eq!(st.gutter(Vec2::new(x, 0.0)), 0.0);
                assert_eq!(st.gutter(Vec2::new(x, s * (kerb + 0.5))), 0.0);
                assert!(st.walls(Vec2::new(x, s * (wall - 0.05))) > 0.6);
                assert!(st.walls(Vec2::new(x, s * (half - 0.05))) > 0.6);
                assert_eq!(st.walls(Vec2::new(x, s * (kerb + st.walk / 2.0))), 0.0);
            }
        }
    }

    /// Water stands in some patches and not most of the street.
    #[test]
    fn puddles_are_patches() {
        let st = neon();
        let mut wet = 0;
        let n = 2000;
        for k in 0..n {
            let p = Vec2::new((k % 50) as f32 * 0.7 - 17.0, (k / 50) as f32 * 0.4 - 8.0);
            if st.puddled(p) > 0.5 {
                wet += 1;
            }
        }
        let share = wet as f32 / n as f32;
        assert!((0.03..0.4).contains(&share), "puddles cover {share}");
    }

    /// A film wets the whole paving at its depth and the puddles stand deeper; with none,
    /// the dry lies under the water face's cut.
    #[test]
    fn a_film_lies_between_the_puddles() {
        let mut st = neon();
        st.film = 0.0;
        let ps: Vec<Vec2> = (0..2000)
            .map(|k| Vec2::new((k % 50) as f32 * 0.7 - 17.0, (k / 50) as f32 * 0.4 - 8.0))
            .collect();
        let dry = ps
            .iter()
            .find(|p| st.puddled(**p) < 0.01)
            .expect("a dry point");
        let wet = ps
            .iter()
            .find(|p| st.puddled(**p) > 0.99)
            .expect("a puddle");
        assert!(st.standing(*dry) < -0.05);
        st.film = 0.008;
        assert_eq!(st.standing(*dry), 0.008);
        assert!(st.standing(*wet) > 0.05);
    }
}
