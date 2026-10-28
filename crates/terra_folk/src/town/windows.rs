//! Lit windows (settlement r24): a home's windows glow with its hearth's firelight within
//! as the light outside falls, so a town at dusk reads lived in from afar, as a town at dusk
//! does in shipped games (Toussaint, KCD2's villages). Every home of the town with windows
//! takes it: each window its builders fitted shutters to (`settlement::fit_all`) gets a pane
//! set back in its hole, drawn emissive only, with no light of its own, so a town of
//! hundreds of windows costs a few batched quads; the few homes nearest the eye also light
//! their doorways with a real light (`light`). Round each lit window its firelight lies on
//! the facade as a soft warm pool (`spill`), as a light standing a little out from the hole
//! would throw it, drawn as an additive card on the wall's face with its falloff baked into
//! the card's vertices as a lightmap holds it, again with no light (settlement r32). A home is lit when its household is in or by
//! it, and then about three of its four windows, each by its room's fire or candle, in a
//! colour and brightness of its own; a lit home's windows flicker a little as firelight does,
//! fade in as the sun goes down, are tonemapped as a flame is, out of the dusk's grade (D415),
//! and wear a halo turned to the eye (`window_halo.wesl`), as shipped games draw a far
//! light's corona where bloom cannot make a few pixels read.

use super::Market;
use crate::rising::{Rises, Rising};
use bevy::{
    light::NotShadowCaster,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use terra_bases::Built;
use terra_build::kit::Kits;
use terra_build::{Base, Prop};
use terra_build_layout::fitting::{self, Hang};
use terra_gen::hash::{of, unit};
use terra_gen::smoothstep;
use terra_town::Life;
use terra_world::daylight::{Clock, SkyPath};
use terra_world::fire::{Glow as FireGlow, GlowMaterial};

/// The share of a lit home's windows lit: the rooms its household is in, the hearth's and
/// the one off it, are lit and a store or a loft over them is not, so a street's houses each
/// show their own pattern of lit and dark windows, as a real street at dusk does. A home's
/// first window is always lit.
const WINDOW_LIT_SHARE: f32 = 0.75;
/// The sun's height, in degrees, at which the first windows begin to glow, and at which they
/// are at their brightest: a room goes dim well before the street does, and people light
/// their fires and rushlights for the evening meal while the sun is still up.
const GLOW_FROM_DEG: f32 = 28.0;
const GLOW_FULL_DEG: f32 = 6.0;
/// Firelight seen through a window, from a room lit by a low hearth and tallow, about
/// 2,000 K, to one by a blazing hearth and wax candles, about 2,900 K (Tanner Helland's fit of
/// the blackbody: sRGB 255, 137, 14 and 255, 177, 101), in linear light: each look takes its
/// own between them, so no two houses' windows are one colour.
const WARMEST: Vec3 = Vec3::new(1.0, 0.25, 0.0044);
const COOLEST: Vec3 = Vec3::new(1.0, 0.44, 0.13);
/// How bright a lit window is at its brightest, before the tonemapper, the same at any
/// exposure (Bevy's default emissive exposure weight, 0), so it reads warm amber under the
/// hue-keeping shoulder whatever the sky round it does (D430).
const GLOW: f32 = 2.4;
/// How many looks a window's glow takes, each its own brightness and flicker; each home
/// takes one, so a street's windows neither match nor pulse together.
const LOOKS: usize = 8;
/// How far a pane stands in from its hole's edges, in metres, behind the frame's boards,
/// and how far into the wall from its middle, as a share of the wall's half thickness: on
/// the room's side, so the jambs and sill frame it.
const INSET_M: f32 = 0.02;
const SET_BACK: f32 = 0.7;
/// The pool of firelight a lit window throws on the wall round it: as from a light standing
/// `SPILL_STAND_M` out from the hole, so the wall a hole's width off takes about a third of
/// the light at the window's edge, eased to nothing `SPILL_REACH_M` out; at its brightest
/// this share of the pane's glow, which over a dusk wall (display level 0.03-0.06) reads as
/// a warm pool, not a lamp. The card stands `SPILL_OUT_M` off the wall's face, clear of a log
/// wall's bulge.
const SPILL_STAND_M: f32 = 0.32;
const SPILL_REACH_M: f32 = 0.7;
const SPILL_SHARE: f32 = 0.12;
const SPILL_OUT_M: f32 = 0.05;
/// How many rows of vertices hold the pool's falloff either side of the window.
const SPILL_STEPS: usize = 4;
/// A lit window's halo (`window_halo.wesl`): at its middle this share of the pane's glow,
/// this many times the window's size across, never under this share of the view's height
/// (about 6 pixels at 1080p), and stood this far out toward the eye so the wall does not cut
/// it. A window a few pixels across blooms by too little to read from over the town.
const HALO_SHARE: f32 = 0.45;
const HALO_WIDE: f32 = 3.0;
const HALO_LEAST: f32 = 0.006;
const HALO_STAND_M: f32 = 0.4;

/// What `shaders/window_halo.wesl` is told. Field order must match the struct there.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct HaloExtension {
    /// Its colour at its middle, linear, before exposure; and how many times the window's
    /// size it spans.
    #[uniform(100)]
    colour: Vec4,
    /// The least share of the view's height it spans, and how far it stands toward the eye.
    #[uniform(100)]
    shape: Vec4,
}

impl MaterialExtension for HaloExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/window_halo.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/window_halo.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/window_halo.wesl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/window_halo.wesl".into()
    }
}

pub type HaloMaterial = ExtendedMaterial<StandardMaterial, HaloExtension>;

/// A home's windows, as one entity under its build, their panes its children: shown while
/// it is lit.
#[derive(Component)]
pub struct Windows {
    home: usize,
}

/// A building of the town laid whole: which of the town's buildings it is.
#[derive(Component)]
pub struct Home(pub usize);

/// A build whose windows have been glazed, or that has none to glaze.
#[derive(Component)]
struct Glazed;

/// The pane every window is drawn with, a unit quad facing +Z, its halo's quad, each look,
/// and the pool cards made so far, one a window size.
#[derive(Resource)]
struct Glow {
    pane: Handle<Mesh>,
    halo: Handle<Mesh>,
    looks: Vec<Look>,
    spills: std::collections::HashMap<(i32, i32), Handle<Mesh>>,
}

/// A look of a lit window: its pane's material, its pool's and its halo's, its firelight's
/// colour, how bright and its flicker's phase.
struct Look {
    pane: Handle<GlowMaterial>,
    spill: Handle<StandardMaterial>,
    halo: Handle<HaloMaterial>,
    warm: Vec3,
    level: f32,
    phase: f32,
}

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<HaloMaterial>::default())
        .add_systems(Startup, make)
        .add_systems(Update, (glaze, light).run_if(resource_exists::<Market>));
}

/// A halo's quad: corners at plus and minus a half, its second texture coordinates there for
/// the shader to carry the halo's strength in.
fn halo_quad() -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    let corners = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        corners.map(|[x, y]| [x, y, 0.0f32]).to_vec(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; 4])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, corners.to_vec())
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, vec![[0.0f32, 0.0]; 4])
    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
}

/// A look's firelight colour, between the warmest and the coolest, a share `t` of the way.
fn warmth(t: f32) -> Vec3 {
    WARMEST.lerp(COOLEST, t.clamp(0.0, 1.0))
}

fn make(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut glows: ResMut<Assets<GlowMaterial>>,
    mut halos: ResMut<Assets<HaloMaterial>>,
) {
    let looks = (0..LOOKS)
        .map(|n| {
            // Tonemapped as a flame is, out of the dusk's grade, which greyed the panes
            // with the land round them (D415).
            let pane = glows.add(GlowMaterial {
                base: StandardMaterial {
                    base_color: Color::BLACK,
                    emissive: LinearRgba::BLACK,
                    perceptual_roughness: 1.0,
                    reflectance: 0.0,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                },
                extension: FireGlow::default(),
            });
            let halo = halos.add(HaloMaterial {
                base: StandardMaterial {
                    base_color: Color::BLACK,
                    unlit: true,
                    alpha_mode: AlphaMode::Add,
                    ..default()
                },
                extension: HaloExtension {
                    colour: Vec4::new(0.0, 0.0, 0.0, HALO_WIDE),
                    shape: Vec4::new(HALO_LEAST, HALO_STAND_M, 0.0, 0.0),
                },
            });
            // Added to the wall behind it, the same at any exposure as the pane's glow.
            let spill = materials.add(StandardMaterial {
                base_color: Color::BLACK,
                unlit: true,
                alpha_mode: AlphaMode::Add,
                ..default()
            });
            Look {
                pane,
                spill,
                halo,
                warm: warmth((n as f32 * 0.381_966 + 0.1) % 1.0),
                // From a room lit by embers to one by a blazing hearth.
                level: 0.55 + 0.75 * ((n as f32 * 0.618_034) % 1.0),
                phase: n as f32 * 2.399,
            }
        })
        .collect();
    commands.insert_resource(Glow {
        pane: meshes.add(Rectangle::new(1.0, 1.0)),
        halo: meshes.add(halo_quad()),
        looks,
        spills: default(),
    });
}

/// How far the windows have faded in with the sun this high, in degrees: none by day, all
/// once it is nearly down, and through the night until it is up again.
pub fn dusk(sun_deg: f32) -> f32 {
    smoothstep(GLOW_FROM_DEG, GLOW_FULL_DEG, sun_deg)
}

/// Whether anyone of a home's household is in it or by its door.
pub fn at_home(life: &Life, home: usize) -> bool {
    let town = &life.town;
    let door = town
        .things
        .iter()
        .position(|t| t.kind == terra_town::town::ThingKind::Door && t.building == Some(home));
    door.is_some_and(|door| {
        life.people.iter().any(|p| {
            p.home == door
                && (p.state == terra_town::State::Inside
                    || p.at.distance(town.things[door].at) < 12.0)
        })
    })
}

/// Whether a home is lit, its hearth burning with someone home: once it is up and lived in
/// if the town is rising (D381). Every lived-in home lights its fire for the evening meal.
pub fn lit_home(life: &Life, rising: Option<&Rising>, home: usize) -> bool {
    rising.is_none_or(|r| r.settled(home)) && at_home(life, home)
}

/// Whether a lit home's `n`th window is lit: its first always, about `WINDOW_LIT_SHARE` of
/// the rest.
fn window_lit(home: usize, n: usize) -> bool {
    n == 0 || unit(of(&[home as u32, n as u32, 0x7f4a_7c15])) < WINDOW_LIT_SHARE
}

/// Which look a lit home's `n`th window takes: the house's own for most, so its rooms share
/// a fire, and now and then a neighbour look, a room lit by a candle rather than the hearth.
fn window_look(home: usize, n: usize) -> usize {
    let own = of(&[home as u32, 24]) as usize;
    let other = unit(of(&[home as u32, n as u32, 25])) < 0.3;
    (own + usize::from(other) * (1 + n % (LOOKS - 1))) % LOOKS
}

/// Each window's pane in a base, in its own space ([`pane`]), with its pool card
/// ([`spill_at`]).
fn panes(base: &Base, kits: Option<&Kits>) -> Vec<(Transform, (Transform, Vec2))> {
    base.props()
        .filter(|prop| prop.hang == Hang::Shutters)
        .map(|prop| {
            let face = fitting::wall_face(base, prop, kits);
            (pane(prop, face), spill_at(prop, face))
        })
        .collect()
}

/// A window's pool card: on the wall's face a little out from it and facing out, and the
/// window's width and height, in metres.
fn spill_at(prop: &Prop, face: f32) -> (Transform, Vec2) {
    let (opens, along, low, high, _) = prop.hole();
    let at = (low + high) * 0.5 + opens * (face + SPILL_OUT_M);
    let size = Vec2::new((high - low).dot(along), high.y - low.y);
    (
        Transform::from_translation(at).looking_to(-opens, Vec3::Y),
        size,
    )
}

/// How much of a window's pool lies on the wall `off` metres out from the window's edge: all
/// of it over the window, then as a light standing out from the hole lights a wall, the
/// inverse square and the slant together, eased to nothing at the pool's reach.
pub fn spill(off: f32) -> f32 {
    let off = off.max(0.0);
    let h2 = SPILL_STAND_M * SPILL_STAND_M;
    libm::powf(h2 / (h2 + off * off), 1.5)
        * (1.0 - smoothstep(0.6 * SPILL_REACH_M, SPILL_REACH_M, off))
}

/// A window's pool card, facing +Z: a grid over the window and `SPILL_REACH_M` round it, its
/// rows closer near the window, where the light falls off fastest, each vertex's colour the
/// share of the pool there ([`spill`]).
fn spill_card(size: Vec2) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    // Along one axis from the pool's edge in to the window's and out again: each row's offset
    // from the middle and how far it is out from the window.
    let line = |half: f32| -> Vec<(f32, f32)> {
        let out: Vec<f32> = (0..=SPILL_STEPS)
            .map(|i| SPILL_REACH_M * (i as f32 / SPILL_STEPS as f32).powi(2))
            .collect();
        let mut at: Vec<(f32, f32)> = out.iter().rev().map(|&o| (-half - o, o)).collect();
        at.extend(out.iter().map(|&o| (half + o, o)));
        at
    };
    let (xs, ys) = (line(size.x * 0.5), line(size.y * 0.5));
    let mut positions = Vec::new();
    let mut colours = Vec::new();
    for &(y, oy) in &ys {
        for &(x, ox) in &xs {
            positions.push([x, y, 0.0]);
            let share = spill(Vec2::new(ox, oy).length());
            colours.push([share, share, share, 1.0]);
        }
    }
    let n = positions.len();
    let width = xs.len() as u32;
    let mut indices = Vec::new();
    for row in 0..ys.len() as u32 - 1 {
        for col in 0..width - 1 {
            let a = row * width + col;
            indices.extend([a, a + 1, a + width + 1, a, a + width + 1, a + width]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; n])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n])
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_indices(Indices::U32(indices))
}

/// A window's pane on a wall whose face stands `face` out from its middle: a quad across the
/// hole the shutters hang in, facing out of the wall, set back toward the room behind the
/// frame.
fn pane(prop: &Prop, face: f32) -> Transform {
    let (opens, along, low, high, _) = prop.hole();
    let at = (low + high) * 0.5 - opens * face * SET_BACK;
    let size = Vec3::new(
        (high - low).dot(along) - 2.0 * INSET_M,
        high.y - low.y - 2.0 * INSET_M,
        1.0,
    );
    Transform::from_translation(at)
        .looking_to(-opens, Vec3::Y)
        .with_scale(size)
}

/// Glazes each home's windows once it stands with its shutters fitted: a town's at once, a
/// rising town's once it is up.
fn glaze(
    mut commands: Commands,
    mut glow: ResMut<Glow>,
    mut meshes: ResMut<Assets<Mesh>>,
    market: Res<Market>,
    rising: Option<Res<Rising>>,
    builds: Query<(Entity, &Built, Option<&Home>, Option<&Rises>), Without<Glazed>>,
) {
    for (entity, built, home, rises) in &builds {
        let index = match (home, rises) {
            (Some(home), _) => Some(home.0),
            (None, Some(rises)) => {
                // Its time is set a few frames after the town is laid (`rising::prepare`).
                if !rising
                    .as_ref()
                    .is_some_and(|r| r.up.get(rises.index) == Some(&true))
                {
                    continue;
                }
                Some(rises.index)
            }
            (None, None) => None,
        };
        commands.entity(entity).insert(Glazed);
        let Some(index) = index.filter(|i| market.life.town.homes.contains(i)) else {
            continue;
        };
        let panes = panes(&built.base, built.kits.as_deref());
        if panes.is_empty() {
            continue;
        }
        let windows = commands
            .spawn((
                Name::new("Windows"),
                Windows { home: index },
                Transform::IDENTITY,
                Visibility::Hidden,
                ChildOf(built.root),
            ))
            .id();
        for (n, (pane, (card, size))) in panes.into_iter().enumerate() {
            if !window_lit(index, n) {
                continue;
            }
            let look = &glow.looks[window_look(index, n)];
            let (material, pool, halo) = (look.pane.clone(), look.spill.clone(), look.halo.clone());
            commands.spawn((
                Mesh3d(glow.pane.clone()),
                MeshMaterial3d(material),
                pane,
                NotShadowCaster,
                ChildOf(windows),
            ));
            // The halo at the window's face, its size the window's.
            commands.spawn((
                Mesh3d(glow.halo.clone()),
                MeshMaterial3d(halo),
                Transform {
                    scale: size.extend(1.0),
                    ..card
                },
                NotShadowCaster,
                bevy::light::NotShadowReceiver,
                ChildOf(windows),
            ));
            // One card mesh a window size, to the centimetre, so a town's cards batch.
            let size_cm = (
                (size.x * 100.0).round() as i32,
                (size.y * 100.0).round() as i32,
            );
            let mesh = glow
                .spills
                .entry(size_cm)
                .or_insert_with(|| meshes.add(spill_card(size)))
                .clone();
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(pool),
                card,
                NotShadowCaster,
                bevy::light::NotShadowReceiver,
                ChildOf(windows),
            ));
        }
    }
}

/// How a hearth's light through a window flickers, about its own brightness, at a time in
/// seconds: a few slow waves and a quicker one, so it wavers rather than pulses.
fn flicker(t: f32, phase: f32) -> f32 {
    0.88 + 0.06 * libm::sinf(t * 5.3 + phase)
        + 0.04 * libm::sinf(t * 11.7 + phase * 2.3)
        + 0.02 * libm::sinf(t * 23.1 + phase * 4.1)
}

/// Each home's windows shown while it is lit, and every look's glow faded in with the dusk
/// and flickering; the count lit written to the log every few seconds.
#[allow(clippy::too_many_arguments)]
fn light(
    time: Res<Time>,
    clock: Res<Clock>,
    sky: Option<Res<SkyPath>>,
    market: Res<Market>,
    rising: Option<Res<Rising>>,
    glow: Res<Glow>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut glows: ResMut<Assets<GlowMaterial>>,
    mut halos: ResMut<Assets<HaloMaterial>>,
    mut windows: Query<(&Windows, &mut Visibility)>,
    mut told: Local<(Vec<f32>, f32)>,
) {
    let fade = sky.map_or(0.0, |sky| dusk(sky.height(clock.hour)));
    let mut lit = 0;
    for (home, mut shown) in &mut windows {
        let on = fade > 0.0 && lit_home(&market.life, rising.as_deref(), home.home);
        lit += usize::from(on);
        let want = if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *shown != want {
            *shown = want;
        }
    }
    let t = time.elapsed_secs();
    told.0.resize(glow.looks.len(), -1.0);
    for (n, look) in glow.looks.iter().enumerate() {
        let value = GLOW * fade * look.level * flicker(t, look.phase);
        if (told.0[n] - value).abs() < 0.004 {
            continue;
        }
        told.0[n] = value;
        let warm = look.warm * value;
        if let Some(mut material) = glows.get_mut(&look.pane) {
            material.base.emissive = LinearRgba::rgb(warm.x, warm.y, warm.z);
        }
        if let Some(mut material) = materials.get_mut(&look.spill) {
            let pool = warm * SPILL_SHARE;
            material.base_color = Color::linear_rgb(pool.x, pool.y, pool.z);
        }
        if let Some(mut material) = halos.get_mut(&look.halo) {
            let halo = warm * HALO_SHARE;
            material.extension.colour = halo.extend(HALO_WIDE);
        }
    }
    if t >= told.1 {
        told.1 = t + 5.0;
        info!(
            "WINDOWS: {lit} of {} homes lit, glow {fade:.2}",
            windows.iter().len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_are_dark_by_day_and_fade_in_as_the_sun_goes_down() {
        assert_eq!(dusk(45.0), 0.0);
        assert_eq!(dusk(GLOW_FROM_DEG), 0.0);
        assert_eq!(dusk(2.0), 1.0);
        assert_eq!(dusk(-20.0), 1.0);
        let fades: Vec<f32> = (0..=30).rev().map(|deg| dusk(deg as f32)).collect();
        assert!(fades.windows(2).all(|pair| pair[1] >= pair[0]), "{fades:?}");
        // Partly in at the settlement film's end, the sun 15 degrees up.
        assert!(dusk(15.0) > 0.3 && dusk(15.0) < 0.9, "{}", dusk(15.0));
    }

    #[test]
    fn a_pane_fills_its_window_on_the_rooms_side_facing_out() {
        let face = 0.16;
        for (min, max, opens) in [
            ([2, 2, 3], [3, 3, 3], [0, 0, 1]),
            ([3, 2, 2], [3, 4, 2], [-1, 0, 0]),
            ([1, 2, 0], [1, 3, 0], [0, 0, -1]),
        ] {
            let prop = Prop {
                min,
                max,
                opens,
                hang: Hang::Shutters,
                ..Prop::default()
            };
            let (out, along, low, high, _) = prop.hole();
            let pane = pane(&prop, face);
            // Facing out of the wall, upright.
            assert!((pane.rotation * Vec3::Z).distance(out) < 1e-4, "{prop:?}");
            assert!(
                (pane.rotation * Vec3::Y).distance(Vec3::Y) < 1e-4,
                "{prop:?}"
            );
            // Behind the wall's middle on the room's side, within the wall.
            let into = ((low + high) * 0.5 - pane.translation).dot(out);
            assert!(into > 0.0 && into < face, "{prop:?}: {into}");
            // Across the hole, all of it but a sliver round its edges.
            let corners = [Vec3::new(-0.5, -0.5, 0.0), Vec3::new(0.5, 0.5, 0.0)]
                .map(|corner| pane.transform_point(corner));
            let (a, b) = (corners[0].min(corners[1]), corners[0].max(corners[1]));
            for axis in [along, Vec3::Y] {
                let (from, to) = (a.dot(axis) - low.dot(axis), high.dot(axis) - b.dot(axis));
                assert!(from > 0.0 && to > 0.0, "{prop:?}: {from} {to}");
                assert!(from < 0.05 && to < 0.05, "{prop:?}: {from} {to}");
            }
        }
    }

    /// The pool on the wall is full over the window, falls off outward without a step, and
    /// is gone at the card's edge, so no card's outline shows on the wall or in the air past
    /// a corner; it stands on the wall's outer face, facing out.
    #[test]
    fn a_windows_pool_fades_out_from_it_to_nothing_at_the_cards_edge() {
        assert_eq!(spill(0.0), 1.0);
        let falls: Vec<f32> = (0..=70).map(|cm| spill(cm as f32 / 100.0)).collect();
        assert!(falls.windows(2).all(|p| p[1] <= p[0]), "{falls:?}");
        assert!(
            falls.windows(2).all(|p| p[0] - p[1] < 0.06),
            "a step: {falls:?}"
        );
        // A light standing out from the hole: about a third a stand's distance off.
        assert!((spill(SPILL_STAND_M) - 0.35).abs() < 0.02);
        assert_eq!(spill(SPILL_REACH_M), 0.0);
        let size = Vec2::new(0.6, 0.9);
        let card = spill_card(size);
        let Some(bevy::mesh::VertexAttributeValues::Float32x4(colours)) =
            card.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("no colours");
        };
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(at)) =
            card.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };
        for (p, c) in at.iter().zip(colours) {
            let on_window = p[0].abs() <= size.x * 0.5 + 1e-4 && p[1].abs() <= size.y * 0.5 + 1e-4;
            let edge = p[0].abs() >= size.x * 0.5 + SPILL_REACH_M - 1e-4
                || p[1].abs() >= size.y * 0.5 + SPILL_REACH_M - 1e-4;
            if on_window {
                assert_eq!(c[0], 1.0, "{p:?}");
            }
            if edge {
                assert_eq!(c[0], 0.0, "{p:?}");
            }
        }
        let face = 0.16;
        let prop = Prop {
            min: [2, 2, 3],
            max: [3, 3, 3],
            opens: [0, 0, 1],
            hang: Hang::Shutters,
            ..Prop::default()
        };
        let (out, _, low, high, _) = prop.hole();
        let (card, _) = spill_at(&prop, face);
        assert!((card.rotation * Vec3::Z).distance(out) < 1e-4);
        let off = (card.translation - (low + high) * 0.5).dot(out);
        assert!(off > face && off < face + 0.1, "{off}");
    }

    /// A lit home always shows a lit window, about the share of its others are lit, and its
    /// windows are not all one look, nor every house's the same.
    #[test]
    fn a_lit_homes_windows_vary_and_one_is_always_lit() {
        assert!((0..500).all(|home| window_lit(home, 0)));
        let lit = (0..500)
            .flat_map(|home| (1..6).map(move |n| window_lit(home, n)))
            .filter(|&l| l)
            .count() as f32
            / 2500.0;
        assert!((lit - WINDOW_LIT_SHARE).abs() < 0.04, "{lit}");
        let mixed = (0..500)
            .filter(|&home| (1..6).any(|n| window_look(home, n) != window_look(home, 0)))
            .count();
        assert!(mixed > 250 && mixed < 500, "{mixed}");
        let firsts: std::collections::HashSet<usize> =
            (0..50).map(|home| window_look(home, 0)).collect();
        assert!(firsts.len() > LOOKS / 2, "{firsts:?}");
        assert!((0..500).all(|home| (0..6).all(|n| window_look(home, n) < LOOKS)));
    }

    /// The looks' firelight runs from a hearth's deep amber to a candle-lit room's paler
    /// gold, each warm (red over green over blue), and no two looks one colour.
    #[test]
    fn the_looks_firelight_is_warm_and_varied() {
        let warms: Vec<Vec3> = (0..LOOKS)
            .map(|n| warmth((n as f32 * 0.381_966 + 0.1) % 1.0))
            .collect();
        for w in &warms {
            assert!(w.x > w.y && w.y > w.z, "{w}");
        }
        for (i, a) in warms.iter().enumerate() {
            for b in &warms[i + 1..] {
                assert!(a.distance(*b) > 0.005, "{a} {b}");
            }
        }
        let greens: Vec<f32> = warms.iter().map(|w| w.y).collect();
        let (lo, hi) = greens
            .iter()
            .fold((1.0f32, 0.0f32), |(lo, hi), &g| (lo.min(g), hi.max(g)));
        assert!(hi - lo > 0.12, "{lo} {hi}");
    }
}
