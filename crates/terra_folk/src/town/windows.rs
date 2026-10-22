//! Lit windows (settlement r24): a home's windows glow with its hearth's firelight within
//! as the light outside falls, so a town at dusk reads lived in from afar, as a town at dusk
//! does in shipped games (Toussaint, KCD2's villages). Every home of the town with windows
//! takes it: each window its builders fitted shutters to (`settlement::fit_all`) gets a pane
//! set back in its hole, drawn emissive only, with no light of its own, so a town of
//! hundreds of windows costs a few batched quads; the few homes nearest the eye also light
//! their doorways with a real light (`light`). Not every home is lit: only one its household
//! is in or by, and only the share that keep a fire in the evening; a lit home's windows
//! flicker a little as firelight does, and fade in as the sun goes down.

use super::Market;
use crate::rising::{Rises, Rising};
use bevy::{light::NotShadowCaster, prelude::*};
use terra_bases::Built;
use terra_build::kit::Kits;
use terra_build::{Base, Prop};
use terra_build_layout::fitting::{self, Hang};
use terra_gen::hash::{of, unit};
use terra_gen::smoothstep;
use terra_town::Life;
use terra_world::daylight::{Clock, SkyPath};

/// The share of homes whose household keeps a fire burning into the evening, its windows lit
/// and its chimney smoking; the rest sit by embers or have gone to bed.
const LIT_SHARE: f32 = 0.7;
/// The sun's height, in degrees, at which the first windows begin to glow, and at which they
/// are at their brightest: a room goes dim well before the street does, and people light
/// their fires and rushlights for the evening meal while the sun is still up.
const GLOW_FROM_DEG: f32 = 28.0;
const GLOW_FULL_DEG: f32 = 6.0;
/// Firelight seen through a window: a room lit by a hearth and rushlights, about 2,400 K
/// (Tanner Helland's fit of the blackbody: sRGB 255, 155, 61), in linear light.
const WARM: Vec3 = Vec3::new(1.0, 0.328, 0.046);
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

/// The pane every window is drawn with, a unit quad facing +Z, and each look's material,
/// its brightness and its flicker's phase.
#[derive(Resource)]
struct Glow {
    pane: Handle<Mesh>,
    looks: Vec<(Handle<StandardMaterial>, f32, f32)>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, make)
        .add_systems(Update, (glaze, light).run_if(resource_exists::<Market>));
}

fn make(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let looks = (0..LOOKS)
        .map(|n| {
            let material = materials.add(StandardMaterial {
                base_color: Color::BLACK,
                emissive: LinearRgba::BLACK,
                perceptual_roughness: 1.0,
                reflectance: 0.0,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            // From a room lit by embers to one by a blazing hearth.
            let level = 0.55 + 0.75 * ((n as f32 * 0.618_034) % 1.0);
            (material, level, n as f32 * 2.399)
        })
        .collect();
    commands.insert_resource(Glow {
        pane: meshes.add(Rectangle::new(1.0, 1.0)),
        looks,
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
/// if the town is rising (D381), and only for the share of homes that keep a fire.
pub fn lit_home(life: &Life, rising: Option<&Rising>, home: usize) -> bool {
    keeps_fire(home) && rising.is_none_or(|r| r.settled(home)) && at_home(life, home)
}

/// Whether a home is among the share that keep a fire into the evening.
fn keeps_fire(home: usize) -> bool {
    unit(of(&[home as u32, 0x7f4a_7c15])) < LIT_SHARE
}

/// Each window's pane in a base, in its own space ([`pane`]).
fn panes(base: &Base, kits: Option<&Kits>) -> Vec<Transform> {
    base.props()
        .filter(|prop| prop.hang == Hang::Shutters)
        .map(|prop| pane(prop, fitting::wall_face(base, prop, kits)))
        .collect()
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
    glow: Res<Glow>,
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
        let look = of(&[index as u32, 24]) as usize % glow.looks.len();
        let material = glow.looks[look].0.clone();
        let windows = commands
            .spawn((
                Name::new("Windows"),
                Windows { home: index },
                Transform::IDENTITY,
                Visibility::Hidden,
                ChildOf(built.root),
            ))
            .id();
        for pane in panes {
            commands.spawn((
                Mesh3d(glow.pane.clone()),
                MeshMaterial3d(material.clone()),
                pane,
                NotShadowCaster,
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
    for (n, (material, level, phase)) in glow.looks.iter().enumerate() {
        let value = GLOW * fade * level * flicker(t, *phase);
        if (told.0[n] - value).abs() < 0.004 {
            continue;
        }
        told.0[n] = value;
        if let Some(mut material) = materials.get_mut(material) {
            let warm = WARM * value;
            material.emissive = LinearRgba::rgb(warm.x, warm.y, warm.z);
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

    #[test]
    fn about_the_share_of_homes_keep_a_fire() {
        let lit = (0..2000).filter(|&home| keeps_fire(home)).count() as f32 / 2000.0;
        assert!((lit - LIT_SHARE).abs() < 0.04, "{lit}");
    }
}
