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
use bevy::asset::RenderAssetUsages;
use bevy::light::{EnvironmentMapLight, LightProbe};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
use std::collections::BTreeMap;
use terra_world::climate::{ClimateSample, Sheltered};
use terra_world::planet::{MainCamera, Planet};
use terra_world::skylight::OpenSky;

/// How much of the sky's light reaches inside a room with a door or window onto the open
/// air; a room's own share of it is what its openings let in (`terra_build::sky`, D486).
const INDOORS: f32 = 0.6;
/// How far a ray from the air beyond an opening looks for the ground hiding the sky, in
/// metres: past a dungeon's well or a cellar's steps, short of the far hills.
const SKY_RAY_M: f32 = 40.0;
/// How far under the ground as generated the air beyond an opening must be before rays are
/// cast to find how much sky it sees, in metres: above it, a house's door, all of it.
const UNDER_GROUND_M: f32 = 0.5;
/// How much of the light come into a room its walls, floor and ceiling give back, for what
/// is in it to reflect: weathered wood and stone, about a fifth.
const ROOM_GIVES_BACK: f32 = 0.2;
/// How far a room's probe reaches past its tiles, into its walls' inner faces and up
/// under its ceiling, in metres; and how far in from its edges it fades, in metres, all of
/// it inside the walls, so their faces take the room's light whole. Bevy's falloff is a
/// share of the box: at 0.03 of it, an 11 m room's walls took a tenth of the sky's
/// reflection, white at grazing angles (the mine's jambs and wall bands, dungeon round 12).
const INTO_WALLS_M: f32 = 0.45;
const FALLOFF_M: f32 = 0.1;

/// How far from the camera a room's probe lights what is in it, in metres from its middle, as
/// authored: past it a room's insides are a few pixels through its door and windows, and
/// Bevy lights a view by its nearest few probes in any case.
const PROBE_M: f32 = 60.0;

/// How much a probe's brightness may lag the sky's, as a share of it: a two-hundredth,
/// under a step of an 8-bit colour.
const BRIGHTNESS_STEP: f32 = 0.005;

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
    /// How much of the sky's light its room's openings let in, from 0 to 1 (D486).
    sky: f32,
    going: bool,
    /// Its reflection: a cube of one colour, what the room gives back, faded in with it;
    /// every room's own while it fades, and the one all rooms closed share once it has
    /// ([`RoomLight`]).
    reflects: Handle<Image>,
    /// The light that falls on what is in it from every way: a cube of one colour, the sky's
    /// light on average, not the open sky over it, which a room's ceiling hides; the same
    /// in every room ([`RoomLight`]).
    falls: Handle<Image>,
    /// The colour its own reflection was last filled with, while it fades.
    filled: Vec3,
}

/// The pictures every room's probe shares (`falls`, and `reflects` once faded in), and the
/// colours they were last filled with. A town's few hundred rooms each filled two of their
/// own as the day turned, every one at once when the sky's colour moved a step: a few
/// hundred textures made again on the GPU in one frame, about once a second, the town walk's
/// worst frames. Shared, as a probe per room of the same light is in shipped games, the day
/// turning fills two.
#[derive(Resource, Default)]
struct RoomLight {
    images: Option<[Handle<Image>; 2]>,
    filled: [Vec3; 2],
}

impl RoomLight {
    /// The shared pictures, what falls and what a room closed reflects, made on first use
    /// and filled again when the sky's light has changed enough to see.
    fn images(&mut self, open: &OpenSky, images: &mut Assets<Image>) -> [Handle<Image>; 2] {
        let colours = [reflected(open, 0.0), reflected(open, 1.0)];
        let Some(shared) = self.images.clone() else {
            self.filled = colours;
            let made = colours.map(|colour| images.add(cube(colour)));
            self.images = Some(made.clone());
            return made;
        };
        for (k, handle) in shared.iter().enumerate() {
            if seen(colours[k], self.filled[k]) {
                self.filled[k] = colours[k];
                if let Some(mut image) = images.get_mut(handle) {
                    *image = cube(colours[k]);
                }
            }
        }
        shared
    }
}

/// Whether a colour has moved from another far enough to see in a byte.
fn seen(colour: Vec3, was: Vec3) -> bool {
    (colour - was).abs().max_element() > 1.0 / 255.0
}

/// Whether a room's probe is faded all the way in and staying, to share the reflection of
/// every room so.
fn steady(share: f32, going: bool) -> bool {
    share >= 1.0 && !going
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
        .init_resource::<RoomLight>()
        .add_systems(Update, (place, follow, inside, shelter).chain());
}

/// Whether a point is in a room closed and covered, by its probes, each a unit box
/// stretched over its room.
fn in_a_room<'a>(
    rooms: impl IntoIterator<Item = (&'a GlobalTransform, &'a Indoors)>,
    point: Vec3,
) -> bool {
    rooms
        .into_iter()
        .any(|(room, probe)| holds(room, probe, point))
}

/// Whether a room's probe, not going, holds a point.
fn holds(room: &GlobalTransform, probe: &Indoors, point: Vec3) -> bool {
    !probe.going && {
        let local = room.affine().inverse().transform_point3(point);
        local.abs().cmple(Vec3::splat(0.5)).all()
    }
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
    mut shared: ResMut<RoomLight>,
    planet: Res<Planet>,
) {
    // Nothing to dim until the sky lights the camera.
    let Some(sky) = camera.iter().next().cloned() else {
        return;
    };
    for build in each(&mut site, &mut builds) {
        if placed.0.get(&build.root).map(|(revision, _)| *revision) == Some(build.revision) {
            continue;
        }
        // A doorway between two rooms is part of the wall, in neither room's box: its own
        // probe gives its jambs, lintel and leaves the rooms' light, not the sky's.
        let doorways = build.base.passages();
        let rooms: Vec<(IVec3, IVec3)> = build
            .base
            .indoors()
            .into_iter()
            .chain(doorways.iter().map(|d| (d.low, d.high)))
            .collect();
        let unchanged = placed
            .0
            .get(&build.root)
            .is_some_and(|(_, before)| *before == rooms);
        placed.0.insert(build.root, (build.revision, rooms.clone()));
        if unchanged {
            continue;
        }
        let cube_m = terra_build::CELL_M;
        let boxes: Vec<(Vec3, Vec3)> = rooms
            .iter()
            .map(|&(low, high)| {
                (
                    low.as_vec3() * cube_m - Vec3::splat(INTO_WALLS_M),
                    (high + IVec3::ONE).as_vec3() * cube_m + Vec3::splat(INTO_WALLS_M),
                )
            })
            .collect();
        let wanted: Vec<[i32; 6]> = boxes.iter().map(|&(min, max)| key(min, max)).collect();
        let skies = room_skies(&build.base, &build.frame, &planet, &rooms, &doorways);
        let mut kept = Vec::new();
        for (_, parent, mut probe) in &mut probes {
            if parent.parent() != build.root {
                continue;
            }
            let at = wanted.iter().position(|room| *room == probe.room);
            probe.going = at.is_none();
            if let Some(at) = at {
                kept.push(probe.room);
                if probe.sky != skies[at] {
                    probe.sky = skies[at];
                }
            }
        }
        said(&skies, sky.intensity);
        for ((&(min, max), room), let_in) in boxes.iter().zip(wanted).zip(skies) {
            if kept.contains(&room) {
                continue;
            }
            let filled = reflected(&open, 0.0);
            let falls = shared.images(&open, &mut images)[0].clone();
            let reflects = images.add(cube(filled));
            commands.spawn((
                Name::new("Indoors"),
                lit(&sky, (0.0, let_in), &falls, &reflects),
                Indoors {
                    room,
                    share: 0.0,
                    sky: let_in,
                    going: false,
                    reflects,
                    falls,
                    filled,
                },
                LightProbe {
                    falloff: falloff(max - min),
                },
                // Bevy lights a view by its nearest few probes and gathers every one in
                // view each frame to find them: a town's hundreds were most of that.
                bevy::camera::visibility::VisibilityRange {
                    start_margin: 0.0..0.0,
                    end_margin: PROBE_M..PROBE_M * 1.1,
                    use_aabb: false,
                },
                Transform::from_translation((min + max) * 0.5).with_scale(max - min),
                Visibility::default(),
                ChildOf(build.root),
            ));
        }
    }
}

/// A probe's falloff as Bevy takes it, a share of its box on each axis, for `FALLOFF_M` on
/// every one: its weight is the distance in from a face over the box's size times this.
fn falloff(size: Vec3) -> Vec3 {
    (Vec3::splat(FALLOFF_M) / size.max(Vec3::splat(FALLOFF_M))).min(Vec3::splat(0.5))
}

/// How much of the sky's light each room box lets in (D486), in `rooms`' order: its room's
/// share from its openings (`Base::room_sky`), the air beyond each seeing what of the sky the
/// ground round it leaves open (`sky_seen`); the boxes past the rooms' are `doorways`', each
/// the mean of the two rooms it joins.
fn room_skies(
    base: &terra_build::Base,
    frame: &Transform,
    planet: &Planet,
    rooms: &[(IVec3, IVec3)],
    doorways: &[terra_build::sky::Passage],
) -> Vec<f32> {
    let cube = terra_build::CELL_M;
    let shares =
        base.room_sky(|air| sky_seen(planet, frame.transform_point((air.as_vec3() + 0.5) * cube)));
    let share = |room: u32| shares.get(&room).copied().unwrap_or(1.0);
    let boxes = rooms.len() - doorways.len();
    rooms[..boxes]
        .iter()
        .map(|(low, _)| base.room(*low).map_or(1.0, share))
        .chain(
            doorways
                .iter()
                .map(|d| (share(d.rooms[0]) + share(d.rooms[1])) * 0.5),
        )
        .collect()
}

/// How much of the open sky a point in the air sees, from 0 to 1: all of it above the ground
/// as generated (every house's door, at once); under it, as a dug well or a dungeon is, the
/// share of rays over its hemisphere, weighed by how steeply they fall, that reach the open
/// air without meeting the ground; none inside the ground.
fn sky_seen(planet: &Planet, point: Vec3) -> f32 {
    let up = point.normalize_or(Vec3::Y);
    if planet.surface(up).length() - point.length() < UNDER_GROUND_M {
        return 1.0;
    }
    if planet.distance(point) < 0.0 {
        return 0.0;
    }
    let east = up.any_orthonormal_vector();
    let south = up.cross(east);
    let (mut open, mut all) = (0.0, 0.0);
    for (rise, around) in [(90.0_f32, 1), (60.0, 6), (30.0, 6)] {
        for k in 0..around {
            let turn = k as f32 * std::f32::consts::TAU / around as f32;
            let rise = rise.to_radians();
            let flat = east * libm::cosf(turn) + south * libm::sinf(turn);
            let way = (up * libm::sinf(rise) + flat * libm::cosf(rise)).normalize();
            let weight = libm::sinf(rise);
            all += weight;
            if planet.raycast(point, way, SKY_RAY_M).is_none() {
                open += weight;
            }
        }
    }
    open / all
}

/// Says in the log how much sky a base's rooms let in, the least, the mean and the most, and
/// the sky's light in the darkest of them once faded in: what a room's light is measured by.
fn said(skies: &[f32], sky: f32) {
    if skies.is_empty() {
        return;
    }
    let least = skies.iter().copied().fold(1.0_f32, f32::min);
    let most = skies.iter().copied().fold(0.0_f32, f32::max);
    let mean = skies.iter().sum::<f32>() / skies.len() as f32;
    info!(
        "Indoors: {} room boxes let in {least:.2} to {most:.2} of the sky, {mean:.2} on average; the darkest lit {:.0} of the sky's {sky:.0}",
        skies.len(),
        sky * dimming(1.0, least)
    );
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
fn dimming(share: f32, sky: f32) -> f32 {
    let eased = share * share * (3.0 - 2.0 * share);
    1.0 + (INDOORS * sky.clamp(0.0, 1.0) - 1.0) * eased
}

/// The sky's light as it reaches inside a room faded in so far: the camera's, at the
/// brightness the sky gives it now, dimmed, falling on it from every way alike; what it
/// reflects, the room's own.
fn lit(
    light: &EnvironmentMapLight,
    (share, sky): (f32, f32),
    falls: &Handle<Image>,
    reflects: &Handle<Image>,
) -> EnvironmentMapLight {
    EnvironmentMapLight {
        intensity: light.intensity * dimming(share, sky),
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
    mut shared: ResMut<RoomLight>,
) {
    let Some(sky) = camera.iter().next() else {
        return;
    };
    if probes.is_empty() {
        return;
    }
    let [falls, closed] = shared.images(&open, &mut images);
    for (entity, mut probe, mut light) in &mut probes {
        let share = faded(probe.share, probe.going, time.delta_secs());
        if share != probe.share {
            probe.share = share;
        }
        if probe.going && share == 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        if probe.falls != falls {
            probe.falls = falls.clone();
        }
        // Faded all the way in, the reflection every room closed shares; fading, its own,
        // filled again where it has changed enough to see.
        if steady(share, probe.going) {
            if probe.reflects != closed {
                probe.reflects = closed.clone();
            }
        } else {
            let colour = reflected(&open, share);
            if probe.reflects == closed || !images.contains(&probe.reflects) {
                probe.filled = colour;
                probe.reflects = images.add(cube(colour));
            } else if seen(colour, probe.filled) {
                probe.filled = colour;
                if let Some(mut image) = images.get_mut(&probe.reflects) {
                    *image = cube(colour);
                }
            }
        }
        let now = lit(sky, (share, probe.sky), &probe.falls, &probe.reflects);
        // Its brightness follows the sky's as the day turns, a step too small to see left
        // until it is not, so a town's probes are not every one changed every frame.
        if (light.intensity - now.intensity).abs() > light.intensity.abs() * BRIGHTNESS_STEP
            || light.diffuse_map != now.diffuse_map
            || light.specular_map != now.specular_map
        {
            *light = now;
        }
    }
}

/// Says whether the camera is in a room closed and covered, so the eye can open further
/// there (`terra_world::scene::Indoors`), and how much of the sky's light reaches the air
/// round it, for the haze and the fog (`IndoorSky`, D485). The world's scene owns both;
/// without the world (the sandbox's `build` alone), there is no eye to tell.
fn inside(
    camera: Query<&GlobalTransform, With<MainCamera>>,
    rooms: Query<(&GlobalTransform, &Indoors), Without<MainCamera>>,
    indoors: Option<ResMut<terra_world::scene::Indoors>>,
    air: Option<ResMut<terra_world::scene::IndoorSky>>,
    time: Res<Time>,
) {
    let Some(mut indoors) = indoors else {
        return;
    };
    let Some(eye) = camera.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    let within = in_a_room(rooms.iter(), eye);
    if indoors.0 != within {
        indoors.0 = within;
    }
    if let Some(mut air) = air {
        let wanted = air_sky(rooms.iter(), eye);
        let now = toward(air.0, wanted, time.delta_secs());
        air.set_if_neq(terra_world::scene::IndoorSky(now));
    }
}

/// How much of the sky's light reaches the air at a point (D485): in a room, as much as
/// reaches what is in it (`dimming`), the darkest room's where boxes overlap; outside, all.
fn air_sky<'a>(
    rooms: impl IntoIterator<Item = (&'a GlobalTransform, &'a Indoors)>,
    point: Vec3,
) -> f32 {
    rooms
        .into_iter()
        .filter(|(room, probe)| holds(room, probe, point))
        .map(|(_, probe)| dimming(probe.share, probe.sky))
        .fold(1.0, f32::min)
}

/// The air's light a step toward what it is to be, over [`FADE_S`] for the whole way, as a
/// room's probe fades: stepping through a door does not switch the haze.
fn toward(now: f32, wanted: f32, seconds: f32) -> f32 {
    let step = seconds / FADE_S;
    now + (wanted - now).clamp(-step, step)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_in_a_room_is_in_it_and_one_outside_or_in_a_room_going_is_not() {
        let probe = |going| Indoors {
            room: [0; 6],
            share: 1.0,
            sky: 1.0,
            going,
            reflects: Handle::default(),
            falls: Handle::default(),
            filled: Vec3::ZERO,
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
    fn the_air_in_a_room_shut_from_the_sky_is_unlit_by_it_and_outside_wholly_lit() {
        let probe = |sky| Indoors {
            room: [0; 6],
            share: 1.0,
            sky,
            going: false,
            reflects: Handle::default(),
            falls: Handle::default(),
            filled: Vec3::ZERO,
        };
        let room = GlobalTransform::from(
            Transform::from_xyz(10.0, 2.0, 0.0).with_scale(Vec3::new(4.0, 3.0, 6.0)),
        );
        let (shut, open) = (probe(0.0), probe(1.0));
        let inside = Vec3::new(10.0, 2.0, 0.0);
        assert_eq!(air_sky([(&room, &shut)], inside), 0.0);
        assert_eq!(air_sky([(&room, &shut)], Vec3::new(20.0, 2.0, 0.0)), 1.0);
        // Where a shut room's box meets an open one's, the darker holds.
        assert_eq!(air_sky([(&room, &open), (&room, &shut)], inside), 0.0);
        // A room with a door onto the air lets in what reaches what is in it.
        assert!((air_sky([(&room, &open)], inside) - INDOORS).abs() < 1e-6);
        // The air takes FADE_S to go all the way, not one frame.
        let mut air = 1.0;
        for _ in 0..30 {
            air = toward(air, 0.0, 1.0 / 60.0);
        }
        assert!((air - (1.0 - 0.5 / FADE_S)).abs() < 1e-4, "{air}");
        for _ in 0..60 {
            air = toward(air, 0.0, 1.0 / 60.0);
        }
        assert_eq!(air, 0.0);
    }

    #[test]
    fn a_rooms_walls_take_its_probe_whole_however_big_the_room() {
        // Bevy's weight (light_probe.wesl): on each axis, saturate((1 - 2|p|) / 2f) for a
        // point at p in the box's space, its faces at ±0.5; the least of the three.
        let weight = |size: f32, in_m: f32| {
            let f = falloff(Vec3::splat(size)).x;
            let p = 0.5 - in_m / size;
            ((1.0 - 2.0 * p) / (2.0 * f)).clamp(0.0, 1.0)
        };
        for size in [1.6, 3.1, 11.0, 40.0] {
            assert_eq!(
                weight(size, INTO_WALLS_M),
                1.0,
                "a wall's face, a {size} m box"
            );
            assert_eq!(weight(size, 0.0), 0.0, "the box's edge, {size} m");
            assert!(
                weight(size, FALLOFF_M * 0.5) < 1.0,
                "fading inside the wall, {size} m"
            );
        }
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
    fn rooms_closed_share_their_light_so_the_day_turning_fills_two_pictures_not_two_a_room() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.init_resource::<Assets<Image>>();
        world.init_resource::<RoomLight>();
        world.insert_resource(Time::<()>::default());
        let sky = |c: f32| OpenSky {
            sky: [Vec3::splat(c); 6],
            ground: [Vec3::splat(c * 0.4); 6],
            brightness: 1.0,
        };
        world.insert_resource(sky(0.5));
        world.spawn((MainCamera, EnvironmentMapLight::default()));
        // Four rooms closed, one still fading in.
        for (i, share) in [1.0, 1.0, 1.0, 1.0, 0.5].into_iter().enumerate() {
            world.spawn((
                Indoors {
                    room: [i as i32; 6],
                    share,
                    sky: 1.0,
                    going: false,
                    reflects: Handle::default(),
                    falls: Handle::default(),
                    filled: Vec3::ZERO,
                },
                EnvironmentMapLight::default(),
            ));
        }
        let lights = |world: &mut World| {
            let mut probes = world.query::<(&Indoors, &EnvironmentMapLight)>();
            probes
                .iter(world)
                .map(|(probe, light)| {
                    assert_eq!(light.specular_map, probe.reflects);
                    assert_eq!(light.diffuse_map, probe.falls);
                    (probe.share, probe.falls.id(), probe.reflects.id())
                })
                .collect::<Vec<_>>()
        };
        world.run_system_once(follow).unwrap();
        let first = lights(&mut world);
        let closed: std::collections::BTreeSet<_> = first
            .iter()
            .filter(|(share, ..)| *share >= 1.0)
            .map(|(_, _, reflects)| *reflects)
            .collect();
        let falls: std::collections::BTreeSet<_> = first.iter().map(|(_, f, _)| *f).collect();
        assert_eq!(closed.len(), 1, "the rooms closed share one reflection");
        assert_eq!(falls.len(), 1, "every room shares what falls in it");
        let fading = first.iter().find(|(share, ..)| *share < 1.0).unwrap();
        assert!(!closed.contains(&fading.2), "the room fading has its own");
        // Shared, fading: three pictures in all, and as many after the day has turned,
        // filled again rather than made.
        assert_eq!(world.resource::<Assets<Image>>().len(), 3);
        world.insert_resource(sky(0.7));
        world.run_system_once(follow).unwrap();
        assert_eq!(lights(&mut world), first);
        assert_eq!(world.resource::<Assets<Image>>().len(), 3);
        let [_, reflects] = world.resource::<RoomLight>().images.clone().unwrap();
        let image = world.resource::<Assets<Image>>().get(&reflects).unwrap();
        let want = cube(reflected(&sky(0.7), 1.0));
        assert_eq!(
            image.data, want.data,
            "the shared reflection follows the sky"
        );
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
            biggest = biggest.max((dimming(next, 1.0) - dimming(share, 1.0)).abs());
            share = next;
            frames += 1;
        }
        assert!((dimming(1.0, 1.0) - INDOORS).abs() < 1e-6 && dimming(0.0, 1.0) == 1.0);
        // A room no sky reaches goes dark once faded in, and is lit as outside before.
        assert!(dimming(1.0, 0.0) == 0.0 && dimming(0.0, 0.0) == 1.0);
        // Two fifths of the light over most of a second, never more than a fiftieth of
        // it in a frame.
        assert!(frames > 30, "{frames} frames");
        assert!(biggest < 0.02, "{biggest} in a frame");
        while share > 0.0 {
            share = faded(share, true, frame);
        }
        assert_eq!(dimming(share, 1.0), 1.0);
    }
}
