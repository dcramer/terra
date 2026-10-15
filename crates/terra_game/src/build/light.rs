//! Indoor light (D155; docs/building-plan.md, drawing): a room closed all round, roof and
//! floor too (`terra_build::Base::indoors`, D261), is not lit by the open sky. The sky lights the
//! world through the camera's environment map (`terra_world::skylight`); inside each such
//! room stands a light probe holding the same sky, dimmed, which Bevy uses in place of the
//! camera's for what is inside it, as shipped games place a probe per room. The sun still
//! comes in through doors and windows, by its shadows. What a room's surfaces reflect is not
//! the sky, which they cannot see, but the room: each probe's specular map is the room's own,
//! a cube of one colour, the sky's light come in and given back by its walls; and the light
//! that falls on what is in it comes from every way alike, the sky's on average, not from
//! the open sky over it, which its ceiling hides. Lit from the sky above, the tops of logs
//! indoors, worn pale, read blue-white (research/kit-review-2026-10-14.md; specular occlusion
//! and a room's own ambient, as shipped engines give a room's probe). A probe per room lit
//! from what is really round it is later work.

use super::store::each;
use super::{Built, Site};
use crate::planet::MainCamera;
use bevy::asset::RenderAssetUsages;
use bevy::light::{EnvironmentMapLight, LightProbe};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
use std::collections::BTreeMap;
use terra_world::climate::{ClimateSample, Sheltered};
use terra_world::skylight::OpenSky;

/// How much of the sky's light reaches inside a room.
const INDOORS: f32 = 0.6;
/// How much of the light come into a room its walls, floor and ceiling give back, for what
/// is in it to reflect: weathered wood and stone, about a fifth.
const ROOM_GIVES_BACK: f32 = 0.2;
/// How far a room's probe reaches past its tiles, into its walls' inner faces and up
/// under its ceiling, in metres; and how much of it fades out at its edges.
const INTO_WALLS_M: f32 = 0.3;
const FALLOFF: f32 = 0.03;

/// How long a room's light takes to change when it is closed or opened, in seconds: a probe
/// is blended in and out, as engines blend a probe's weight, not switched, which dimmed a
/// room by two fifths in one frame as its last cube went in (`cargo xtask artifacts
/// --frames`, going-up).
const FADE_S: f32 = 0.8;

/// A light probe dimming the sky inside a room of a base: the room's box, how far it is
/// faded in, from 0 (lit as outside) to 1, and whether it is going, its room no longer
/// closed.
#[derive(Component)]
struct Indoors {
    room: [i32; 6],
    share: f32,
    going: bool,
    /// Its own reflection: a cube of one colour, what the room gives back, faded in with it.
    reflects: Handle<Image>,
    /// The light that falls on what is in it from every way: a cube of one colour, the sky's
    /// light on average, not the open sky over it, which a room's ceiling hides.
    falls: Handle<Image>,
    /// The colours they were last filled with, what falls and what is reflected.
    filled: [Vec3; 2],
}

/// The light a room's surfaces reflect, as a cube of one colour, linear: from what the open
/// sky gives on average, each side's light from above and below, toward what the room gives
/// back as the room is faded in.
fn reflected(open: &OpenSky, share: f32) -> Vec3 {
    let average = (open.sky.iter().sum::<Vec3>() + open.ground.iter().sum::<Vec3>()) / 6.0;
    let eased = share * share * (3.0 - 2.0 * share);
    average * (1.0 + (ROOM_GIVES_BACK - 1.0) * eased)
}

/// A cube of one colour, a texel a side, as a probe's specular map.
fn cube(colour: Vec3) -> Image {
    let texel =
        [colour.x, colour.y, colour.z, 1.0].map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
    let mut image = Image::new_fill(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        &texel,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image
}

/// A room's box as a key, to the centimetre.
fn key(min: Vec3, max: Vec3) -> [i32; 6] {
    let cm = |m: f32| (m * 100.0).round() as i32;
    [
        cm(min.x),
        cm(min.y),
        cm(min.z),
        cm(max.x),
        cm(max.y),
        cm(max.z),
    ]
}

/// The revision of each base its rooms were last looked at for, and the rooms' boxes
/// its probes were placed for.
#[derive(Resource, Default)]
struct Placed(BTreeMap<Entity, (u64, Vec<(IVec3, IVec3)>)>);

pub fn plugin(app: &mut App) {
    app.init_resource::<Placed>()
        .add_systems(Update, (place, follow, inside, shelter).chain());
}

/// Whether a point is in a room closed and covered, by its probes, each a unit box
/// stretched over its room.
fn in_a_room<'a>(
    rooms: impl IntoIterator<Item = (&'a GlobalTransform, &'a Indoors)>,
    point: Vec3,
) -> bool {
    rooms.into_iter().any(|(room, probe)| {
        if probe.going {
            return false;
        }
        let local = room.affine().inverse().transform_point3(point);
        local.abs().cmple(Vec3::splat(0.5)).all()
    })
}

/// Shelters from the weather what feels the climate and stands in a room (D261): out of the
/// wind, the rain and the sun, and a fire's warmth held in (`terra_world::climate`).
fn shelter(
    mut commands: Commands,
    rooms: Query<(&GlobalTransform, &Indoors)>,
    bodies: Query<(Entity, &GlobalTransform, Option<&Sheltered>), With<ClimateSample>>,
) {
    for (entity, at, sheltered) in &bodies {
        // Chest high, so a body standing on a floor is in the room over it.
        let chest = at.translation() + at.translation().normalize_or(Vec3::Y) * 1.2;
        let now = Sheltered(if in_a_room(rooms.iter(), chest) {
            1.0
        } else {
            0.0
        });
        if sheltered != Some(&now) {
            commands.entity(entity).insert(now);
        }
    }
}

/// Places a probe in every room indoors, again whenever a base changes: a room that is new
/// fades its probe in, one no longer closed fades its out; rooms that stay keep theirs.
fn place(
    mut commands: Commands,
    mut placed: ResMut<Placed>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
    mut probes: Query<(Entity, &ChildOf, &mut Indoors)>,
    camera: Query<&EnvironmentMapLight, With<MainCamera>>,
    open: Res<OpenSky>,
    mut images: ResMut<Assets<Image>>,
) {
    // Nothing to dim until the sky lights the camera.
    let Some(sky) = camera.iter().next().cloned() else {
        return;
    };
    for build in each(&mut site, &mut builds) {
        if placed.0.get(&build.root).map(|(revision, _)| *revision) == Some(build.revision) {
            continue;
        }
        let rooms = build.base.indoors();
        let unchanged = placed
            .0
            .get(&build.root)
            .is_some_and(|(_, before)| *before == rooms);
        placed.0.insert(build.root, (build.revision, rooms.clone()));
        if unchanged {
            continue;
        }
        let boxes: Vec<(Vec3, Vec3)> = build
            .base
            .indoors_m()
            .into_iter()
            .map(|(low, high)| {
                (
                    low - Vec3::splat(INTO_WALLS_M),
                    high + Vec3::splat(INTO_WALLS_M),
                )
            })
            .collect();
        let wanted: Vec<[i32; 6]> = boxes.iter().map(|&(min, max)| key(min, max)).collect();
        let mut kept = Vec::new();
        for (_, parent, mut probe) in &mut probes {
            if parent.parent() != build.root {
                continue;
            }
            probe.going = !wanted.contains(&probe.room);
            if !probe.going {
                kept.push(probe.room);
            }
        }
        for (&(min, max), room) in boxes.iter().zip(wanted) {
            if kept.contains(&room) {
                continue;
            }
            let filled = [reflected(&open, 0.0), reflected(&open, 0.0)];
            let (falls, reflects) = (images.add(cube(filled[0])), images.add(cube(filled[1])));
            commands.spawn((
                Name::new("Indoors"),
                lit(&sky, 0.0, &falls, &reflects),
                Indoors {
                    room,
                    share: 0.0,
                    going: false,
                    reflects,
                    falls,
                    filled,
                },
                LightProbe {
                    falloff: Vec3::splat(FALLOFF),
                },
                Transform::from_translation((min + max) * 0.5).with_scale(max - min),
                Visibility::default(),
                ChildOf(build.root),
            ));
        }
    }
}

/// How far a probe has faded after a frame so long, toward in or out.
fn faded(share: f32, going: bool, seconds: f32) -> f32 {
    let step = seconds / FADE_S;
    if going {
        (share - step).max(0.0)
    } else {
        (share + step).min(1.0)
    }
}

/// The sky's light inside a room faded in so far, eased in and out: as outside at 0, dimmed
/// at 1.
fn dimming(share: f32) -> f32 {
    let eased = share * share * (3.0 - 2.0 * share);
    1.0 + (INDOORS - 1.0) * eased
}

/// The sky's light as it reaches inside a room faded in so far: the camera's, at the
/// brightness the sky gives it now, dimmed, falling on it from every way alike; what it
/// reflects, the room's own.
fn lit(
    light: &EnvironmentMapLight,
    share: f32,
    falls: &Handle<Image>,
    reflects: &Handle<Image>,
) -> EnvironmentMapLight {
    EnvironmentMapLight {
        intensity: light.intensity * dimming(share),
        diffuse_map: falls.clone(),
        specular_map: reflects.clone(),
        ..light.clone()
    }
}

/// Keeps every room's light the sky's, dimmed as far as it has faded in, as the day turns;
/// a probe whose room has gone is taken away once faded out.
fn follow(
    mut commands: Commands,
    time: Res<Time>,
    camera: Query<&EnvironmentMapLight, With<MainCamera>>,
    mut probes: Query<(Entity, &mut Indoors, &mut EnvironmentMapLight), Without<MainCamera>>,
    open: Res<OpenSky>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(sky) = camera.iter().next() else {
        return;
    };
    for (entity, mut probe, mut light) in &mut probes {
        let share = faded(probe.share, probe.going, time.delta_secs());
        if share != probe.share {
            probe.share = share;
        }
        if probe.going && share == 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        // Its reflection's colour again where it has changed enough to see: as it fades, and
        // as the day turns.
        let colour = reflected(&open, share);
        let colours = [reflected(&open, 0.0), colour];
        for (k, handle) in [probe.falls.clone(), probe.reflects.clone()]
            .iter()
            .enumerate()
        {
            if (colours[k] - probe.filled[k]).abs().max_element() > 1.0 / 255.0 {
                probe.filled[k] = colours[k];
                if let Some(mut image) = images.get_mut(handle) {
                    *image = cube(colours[k]);
                }
            }
        }
        let now = lit(sky, share, &probe.falls, &probe.reflects);
        if light.intensity != now.intensity
            || light.diffuse_map != now.diffuse_map
            || light.specular_map != now.specular_map
        {
            *light = now;
        }
    }
}

/// Says whether the camera is in a room closed and covered, so the eye can open further
/// there (`terra_world::scene::Indoors`).
fn inside(
    camera: Query<&GlobalTransform, With<MainCamera>>,
    rooms: Query<(&GlobalTransform, &Indoors), Without<MainCamera>>,
    mut indoors: ResMut<terra_world::scene::Indoors>,
) {
    let Some(eye) = camera.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    let within = in_a_room(rooms.iter(), eye);
    if indoors.0 != within {
        indoors.0 = within;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_in_a_room_is_in_it_and_one_outside_or_in_a_room_going_is_not() {
        let probe = |going| Indoors {
            room: [0; 6],
            share: 1.0,
            going,
            reflects: Handle::default(),
            falls: Handle::default(),
            filled: [Vec3::ZERO; 2],
        };
        let room = GlobalTransform::from(
            Transform::from_xyz(10.0, 2.0, 0.0).with_scale(Vec3::new(4.0, 3.0, 6.0)),
        );
        let closed = [(&room, &probe(false))];
        assert!(in_a_room(closed, Vec3::new(11.5, 3.0, 2.5)));
        assert!(
            !in_a_room(closed, Vec3::new(12.5, 3.0, 0.0)),
            "past its wall"
        );
        assert!(!in_a_room(
            [(&room, &probe(true))],
            Vec3::new(10.0, 2.0, 0.0)
        ));
    }

    #[test]
    fn a_room_reflects_the_light_its_walls_give_back_not_the_sky() {
        let open = OpenSky {
            sky: [Vec3::new(0.3, 0.4, 0.6); 6],
            ground: [Vec3::new(0.1, 0.1, 0.05); 6],
            brightness: 1.0,
        };
        // Faded in, a fifth of what comes in on average; at first, all of it, so nothing
        // changes in a frame as the room closes.
        let (start, closed) = (reflected(&open, 0.0), reflected(&open, 1.0));
        assert!(
            (start - Vec3::new(0.4, 0.5, 0.65)).length() < 1e-5,
            "{start}"
        );
        assert!(
            (closed - start * ROOM_GIVES_BACK).length() < 1e-5,
            "{closed}"
        );
        assert!(reflected(&open, 0.5).x < start.x && reflected(&open, 0.5).x > closed.x);
    }

    #[test]
    fn a_room_closing_dims_its_light_a_little_each_frame_not_at_once() {
        // At sixty frames a second, from outside's light to the room's and back.
        let frame = 1.0 / 60.0;
        let mut share = 0.0;
        let mut biggest: f32 = 0.0;
        let mut frames = 0;
        while share < 1.0 {
            let next = faded(share, false, frame);
            biggest = biggest.max((dimming(next) - dimming(share)).abs());
            share = next;
            frames += 1;
        }
        assert!((dimming(1.0) - INDOORS).abs() < 1e-6 && dimming(0.0) == 1.0);
        // Two fifths of the light over most of a second, never more than a fiftieth of
        // it in a frame.
        assert!(frames > 30, "{frames} frames");
        assert!(biggest < 0.02, "{biggest} in a frame");
        while share > 0.0 {
            share = faded(share, true, frame);
        }
        assert_eq!(dimming(share), 1.0);
    }
}
