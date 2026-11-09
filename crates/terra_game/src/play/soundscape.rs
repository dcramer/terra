//! The sound of the place (docs/audio.md): air, birds and water all round the listener,
//! louder or quieter for where they stand, and sounds that belong to things: fires
//! burning, the hum where magic pools, a settlement's bell.
//!
//! Layers that surround the listener are always playing and fade to suit the place:
//! wind outdoors, birds where it is safe, water near the shore. Sources that belong to
//! a thing carry `Ambient`, and are heard only within earshot, so a world can hold any
//! number of them.

use super::claim::Village;
use super::crafting::{Fire, Stations, Works};
use super::creatures::Lair;
use super::noise::{Cue, Noise};
use super::props::Prop;
use super::{IsLocalPlayer, Motion};
use crate::audio::{
    self, Bus, Fade, Mix, Voice,
    synth::{Chorus, Sound, Synth},
};
use crate::planet::MainCamera;
use crate::planet::Planet;
use bevy::prelude::*;
use terra_gen::{Rng, Seed};
use terra_world::climate::ClimateFields;
use terra_world::flora::{Obstacles, Trunks};
use terra_world::instance::{Building, InWorld, WorldScoped};

/// How often the place is looked at again.
const LOOK_EVERY_S: f32 = 0.25;
/// How loud a campfire is, a forge or smelter while it works, and the hum where magic
/// pools.
const CAMPFIRE: f32 = 0.55;
const LIT: f32 = 0.55;
const HUM: f32 = 0.4;
/// Rings of the ground looked at for water, and their distance in metres.
const SHORE_RINGS_M: [f32; 4] = [6.0, 15.0, 30.0, 55.0];
/// Birds are gone this near the place where magic pools.
const BIRDS_GONE_M: f32 = 40.0;

pub fn plugin(app: &mut App) {
    app.insert_resource(Bells(Seed(19).derive("bells").rng()))
        // The world's own beds (D469): a realm sounds as its own place, made on entering it.
        .add_systems(
            OnEnter(InWorld),
            surround.run_if(audio::speaking).in_set(Building::Place),
        )
        .add_systems(
            Update,
            (
                (follow_place, mark_sources, stoke, earshot).run_if(audio::speaking),
                ring,
            ),
        );
    terra_world::remote::method(app, "sound.state", state);
}

/// A sound that surrounds the listener.
#[derive(Component)]
struct Layer(Sound);

/// A thing that makes a sound of its own as long as it is there: a fire, the hum where
/// magic pools.
#[derive(Component)]
pub struct Ambient {
    pub sound: Sound,
    /// How loud, from 0; 0 is quiet, such as a forge not lit.
    pub level: f32,
    /// Heard at full loudness within so many metres.
    pub reach_m: f32,
    playing: Option<Entity>,
}

impl Ambient {
    pub fn new(sound: Sound, level: f32, reach_m: f32) -> Self {
        Ambient {
            sound,
            level,
            reach_m,
            playing: None,
        }
    }
}

/// A settlement's bell, and how long until it rings.
#[derive(Component)]
struct Bell {
    left_s: f32,
}

#[derive(Resource)]
struct Bells(Rng);

fn surround(mut commands: Commands, mut synths: ResMut<Assets<Synth>>, mix: Res<Mix>) {
    for (sound, seed) in LAYERS {
        let voice = Voice {
            bus: Bus::Ambience,
            level: 0.0,
        };
        let layer = audio::play_synth(
            &mut commands,
            &mut synths,
            &mix,
            Synth { sound, seed },
            voice,
            None,
            1.0,
        );
        commands.entity(layer).insert((Layer(sound), WorldScoped));
    }
}

/// The surrounding layers, each with the seed its sound is made from.
const LAYERS: [(Sound, u64); 7] = [
    (Sound::Wind, 1),
    (Sound::Birds, 2),
    (Sound::Water, 3),
    (Sound::Calls(Chorus::Woods), 4),
    (Sound::Calls(Chorus::Fells), 5),
    (Sound::Calls(Chorus::Shore), 6),
    (Sound::Chimes, 7),
];

/// The listener's place, read from the world round the player: nothing where there is no
/// world yet (the title screen, a run's first frames), which a system asking for the
/// world's own resources outright would stop the game at.
#[derive(bevy::ecs::system::SystemParam)]
struct Place<'w, 's> {
    planet: Option<Res<'w, Planet>>,
    lair: Option<Res<'w, Lair>>,
    trunks: Option<Res<'w, Trunks>>,
    obstacles: Option<Res<'w, Obstacles>>,
    climate: Option<Res<'w, ClimateFields>>,
    realms: Option<Res<'w, terra_world::realm::Realms>>,
    spec: Option<Res<'w, terra_world::instance::WorldSpec>>,
    player: Query<'w, 's, (&'static Motion, &'static GlobalTransform), IsLocalPlayer>,
}

impl Place<'_, '_> {
    fn here(&self) -> Option<Here> {
        let (motion, at) = self.player.iter().next()?;
        let (planet, trunks, obstacles) = (
            self.planet.as_ref()?,
            self.trunks.as_ref()?,
            self.obstacles.as_ref()?,
        );
        let here = at.translation();
        let ground = planet.ground_at(here);
        let standing = trunks.near(ground, WOODED.1, obstacles).count();
        let season_c = self.climate.as_ref().map_or(12.0, |climate| {
            climate.fields.season(climate.place(ground)).growing_mean_c
        });
        let realm = (self.realms.as_ref().zip(self.spec.as_ref()))
            .and_then(|(realms, spec)| realms.of(&spec.kind));
        Some(Here {
            birdsong: realm.and_then(|realm| realm.birdsong).unwrap_or(1.0),
            chimes: realm.and_then(|realm| realm.chimes).unwrap_or(0.0),
            outdoors: !motion.indoors,
            lair_m: (self.lair.as_ref()).map_or(f32::MAX, |lair| lair.0.distance(here)),
            shore_m: shore_distance(planet, here),
            wooded: (standing as f32 / WOODED.0).min(1.0),
            fells: ((FELLS_C - season_c) / 3.0).clamp(0.0, 1.0),
        })
    }
}

/// How loud each surrounding layer should be where the player stands.
fn follow_place(
    mut commands: Commands,
    time: Res<Time>,
    place: Place,
    layers: Query<(Entity, &Layer, &Voice)>,
    mut wait: Local<f32>,
) {
    *wait -= time.delta_secs();
    if *wait > 0.0 {
        return;
    }
    *wait = LOOK_EVERY_S;
    let Some(place) = place.here() else {
        return;
    };
    for (entity, layer, voice) in &layers {
        let wanted = place_level(layer.0, &place);
        if (wanted - voice.level).abs() > 0.01 {
            commands.entity(entity).insert(Fade {
                to: wanted,
                per_second: 0.25,
                then_stop: false,
            });
        }
    }
}

/// `sound.state`: how loud each surrounding layer is to be where the player stands, by
/// the place alone, so a muted run can say what a place sounds of (D69).
fn state(In(_): In<Option<serde_json::Value>>, place: Place) -> bevy::remote::BrpResult {
    let here = place.here();
    let layers: serde_json::Map<String, serde_json::Value> = LAYERS
        .iter()
        .map(|(sound, _)| {
            let level = here.as_ref().map_or(0.0, |here| place_level(*sound, here));
            (format!("{sound:?}"), level.into())
        })
        .collect();
    Ok(serde_json::json!({ "layers": layers }))
}

/// A wood is so many trunks within so far; fells are where the growing season is colder
/// than this, °C, trees giving way to heath (D313).
const WOODED: (f32, f32) = (6.0, 15.0);
const FELLS_C: f32 = 8.0;

/// What the listener's place is, as the surrounding layers ask it.
#[derive(Clone, Copy, Debug)]
struct Here {
    outdoors: bool,
    /// How far the place where magic pools is.
    lair_m: f32,
    /// How far the nearest water is, if any is near.
    shore_m: Option<f32>,
    /// How wooded, 0 open to 1 among trees, and how much it is the cold open fells.
    wooded: f32,
    fells: f32,
    /// Its realm's share of the first realm's birdsong and calls, and how loud its own air
    /// rings (`data/realms.ron`).
    birdsong: f32,
    chimes: f32,
}

impl Here {
    /// In the open by default, far from anything.
    #[cfg(test)]
    fn open() -> Self {
        Here {
            outdoors: true,
            lair_m: 500.0,
            shore_m: None,
            wooded: 0.0,
            fells: 0.0,
            birdsong: 1.0,
            chimes: 0.0,
        }
    }
}

/// How loud a surrounding layer is: outdoors or in, how near the place where magic
/// pools (birds go quiet there), how near water, and among trees or out on the fells: the
/// wind louder in the open and in the leaves above a wood, songbirds most in woods and
/// warmth and few on the fells, each place's own calls where it is.
fn place_level(sound: Sound, here: &Here) -> f32 {
    let inside = if here.outdoors { 1.0 } else { 0.2 };
    let near_water = here
        .shore_m
        .map_or(0.0, |metres| (1.0 - (metres - 6.0) / 55.0).clamp(0.0, 1.0));
    let unspoilt = ((here.lair_m - BIRDS_GONE_M) / 80.0).clamp(0.0, 1.0);
    match sound {
        Sound::Wind => (0.25 + 0.15 * here.fells) * inside,
        Sound::Birds => {
            0.3 * inside
                * unspoilt
                * (0.55 + 0.45 * here.wooded)
                * (1.0 - 0.8 * here.fells)
                * here.birdsong
        }
        // Its realm's own air: most among its caps, a little everywhere out of doors.
        Sound::Chimes => here.chimes * inside * (0.6 + 0.4 * here.wooded),
        Sound::Water => near_water * 0.5 * inside,
        Sound::Calls(Chorus::Woods) => 0.3 * inside * unspoilt * here.wooded * here.birdsong,
        Sound::Calls(Chorus::Fells) => 0.35 * inside * unspoilt * here.fells * here.birdsong,
        Sound::Calls(Chorus::Shore) => 0.3 * inside * unspoilt * near_water * here.birdsong,
        _ => 0.0,
    }
}

/// How far the nearest water is, looking round in rings, if there is any near.
fn shore_distance(planet: &Planet, here: Vec3) -> Option<f32> {
    let up = here.normalize_or(Vec3::Y);
    let east = up.any_orthonormal_vector();
    if planet.under_sea(here) {
        return Some(0.0);
    }
    SHORE_RINGS_M.into_iter().find(|&metres| {
        (0..12).any(|i| {
            let round = Quat::from_axis_angle(up, i as f32 * std::f32::consts::TAU / 12.0);
            let there = here + round * east * metres;
            planet.under_sea(there)
        })
    })
}

/// Gives the things that make sounds of their own their sound: fires at stations, the
/// place where magic pools, and the settlement's bell.
fn mark_sources(
    mut commands: Commands,
    stations: Query<(Entity, &Prop), Added<Works>>,
    kinds: Res<Stations>,
    lair: Option<Res<Lair>>,
    village: Option<Res<Village>>,
    mut marked: Local<(bool, bool)>,
) {
    for (entity, prop) in &stations {
        let level = match prop
            .item
            .station
            .and_then(|station| kinds.get(station).fire)
        {
            Some(Fire::Always) => CAMPFIRE,
            // Lit only while they work (`stoke`).
            Some(Fire::WhileWorking) => 0.0,
            None => continue,
        };
        commands
            .entity(entity)
            .try_insert(Ambient::new(Sound::Fire, level, 2.5));
    }
    if let Some(lair) = lair.filter(|_| !marked.0) {
        marked.0 = true;
        commands.spawn((
            Name::new("Where magic pools, heard"),
            Transform::from_translation(lair.0),
            Ambient::new(Sound::Hum, HUM, 8.0),
        ));
    }
    if let Some(village) = village.filter(|_| !marked.1) {
        marked.1 = true;
        let up = village.0.normalize_or(Vec3::Y);
        commands.spawn((
            Name::new("The village bell"),
            Transform::from_translation(village.0 + up * 6.0),
            Bell { left_s: 25.0 },
        ));
    }
}

/// A station whose fire burns while it works, such as a forge or a smelter, burns while it
/// works.
fn stoke(kinds: Res<Stations>, mut fires: Query<(&Prop, &Works, &mut Ambient)>) {
    for (prop, works, mut ambient) in &mut fires {
        let fire = prop
            .item
            .station
            .and_then(|station| kinds.get(station).fire);
        if fire == Some(Fire::WhileWorking) {
            let level = if works.queue.is_empty() { 0.0 } else { LIT };
            if ambient.level != level {
                ambient.level = level;
            }
        }
    }
}

/// Starts a thing's sound when the listener comes within earshot, follows its level,
/// and lets it go when they leave.
fn earshot(
    mut commands: Commands,
    mut synths: ResMut<Assets<Synth>>,
    mix: Res<Mix>,
    ear: Single<&GlobalTransform, With<MainCamera>>,
    mut sources: Query<(Entity, &GlobalTransform, &mut Ambient)>,
    voices: Query<&Voice>,
) {
    let ear = ear.translation();
    for (entity, at, mut ambient) in &mut sources {
        let near = at.translation().distance(ear) < ambient.reach_m * 14.0;
        let heard = near && ambient.level > 0.0;
        match ambient.playing {
            None if heard => {
                let voice = Voice {
                    bus: Bus::Ambience,
                    level: 0.0,
                };
                let synth = Synth {
                    sound: ambient.sound,
                    seed: entity.to_bits(),
                };
                let sound = audio::play_synth(
                    &mut commands,
                    &mut synths,
                    &mix,
                    synth,
                    voice,
                    Some(at.translation()),
                    ambient.reach_m,
                );
                commands.entity(sound).insert(Fade {
                    to: ambient.level,
                    per_second: 0.5,
                    then_stop: false,
                });
                ambient.playing = Some(sound);
            }
            Some(sound) if !near => {
                if let Ok(mut gone) = commands.get_entity(sound) {
                    gone.try_insert(Fade {
                        to: 0.0,
                        per_second: 1.0,
                        then_stop: true,
                    });
                }
                ambient.playing = None;
            }
            Some(sound) => {
                if voices
                    .get(sound)
                    .is_ok_and(|voice| (voice.level - ambient.level).abs() > 0.01)
                    && let Ok(mut playing) = commands.get_entity(sound)
                {
                    playing.try_insert(Fade {
                        to: ambient.level,
                        per_second: 0.5,
                        then_stop: false,
                    });
                }
            }
            None => {}
        }
    }
}

/// The settlement's bell rings now and then.
fn ring(
    time: Res<Time>,
    mut bells: Query<(&mut Bell, &GlobalTransform)>,
    mut dice: ResMut<Bells>,
    mut noise: MessageWriter<Noise>,
) {
    for (mut bell, at) in &mut bells {
        bell.left_s -= time.delta_secs();
        if bell.left_s <= 0.0 {
            bell.left_s = dice.0.range(80.0, 180.0);
            noise.write(Noise::at(Cue::Bell, at.translation()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn birds_fall_quiet_near_pooled_magic_and_indoors() {
        let far = BIRDS_GONE_M * 10.0;
        let at = |lair_m, outdoors| {
            place_level(
                Sound::Birds,
                &Here {
                    lair_m,
                    outdoors,
                    ..Here::open()
                },
            )
        };
        let open = at(far, true);
        assert!(open > 0.0);
        assert_eq!(at(BIRDS_GONE_M, true), 0.0);
        let nearer = at(BIRDS_GONE_M + 1.0, true);
        assert!(nearer > 0.0 && nearer < open);
        assert!(at(far, false) < open / 2.0);
    }

    #[test]
    fn each_place_sounds_of_its_own_life() {
        let woods = Here {
            wooded: 1.0,
            ..Here::open()
        };
        let fells = Here {
            fells: 1.0,
            ..Here::open()
        };
        let shore = Here {
            shore_m: Some(0.0),
            ..Here::open()
        };
        let level = place_level;
        // Songbirds most in a wood, few on the fells, whose wind is the loudest.
        assert!(level(Sound::Birds, &woods) > level(Sound::Birds, &Here::open()));
        assert!(level(Sound::Birds, &fells) < level(Sound::Birds, &woods) / 3.0);
        assert!(level(Sound::Wind, &fells) > level(Sound::Wind, &woods));
        // Each place's calls only there.
        for (chorus, place) in [
            (Chorus::Woods, woods),
            (Chorus::Fells, fells),
            (Chorus::Shore, shore),
        ] {
            assert!(level(Sound::Calls(chorus), &place) > 0.2, "{chorus:?}");
            assert_eq!(
                level(Sound::Calls(chorus), &Here::open()),
                0.0,
                "{chorus:?}"
            );
        }
    }

    #[test]
    fn water_is_loudest_at_the_shore_and_heard_as_far_as_it_is_looked_for() {
        let level = |shore_m| {
            place_level(
                Sound::Water,
                &Here {
                    shore_m,
                    ..Here::open()
                },
            )
        };
        let at_shore = level(Some(0.0));
        let mut was = at_shore;
        for ring in SHORE_RINGS_M {
            let there = level(Some(ring));
            assert!(there > 0.0 && there <= was, "{there} at {ring} m");
            was = there;
        }
        assert!(level(SHORE_RINGS_M.last().copied()) < at_shore);
        assert_eq!(level(None), 0.0);
    }

    #[test]
    fn the_world_sits_under_everything_else() {
        use crate::audio::synth::tests::{active_db, at_ear, render, within};
        // Around the listener, at the loudest the place makes them.
        for (sound, level) in [
            (
                Sound::Wind,
                place_level(
                    Sound::Wind,
                    &Here {
                        fells: 1.0,
                        ..Here::open()
                    },
                ),
            ),
            (
                Sound::Birds,
                place_level(
                    Sound::Birds,
                    &Here {
                        wooded: 1.0,
                        ..Here::open()
                    },
                ),
            ),
            (
                Sound::Water,
                place_level(
                    Sound::Water,
                    &Here {
                        shore_m: Some(0.0),
                        ..Here::open()
                    },
                ),
            ),
            (
                Sound::Calls(Chorus::Woods),
                place_level(
                    Sound::Calls(Chorus::Woods),
                    &Here {
                        wooded: 1.0,
                        ..Here::open()
                    },
                ),
            ),
            (
                Sound::Calls(Chorus::Fells),
                place_level(
                    Sound::Calls(Chorus::Fells),
                    &Here {
                        fells: 1.0,
                        ..Here::open()
                    },
                ),
            ),
        ] {
            let (samples, _) = render(sound, 5, 20.0);
            let heard = at_ear(active_db(&samples), level, Bus::Ambience);
            within(&format!("{sound:?}"), heard, (-46.0, -34.0));
        }
        // Things that sound of their own, close up.
        for (sound, level) in [
            (Sound::Fire, CAMPFIRE),
            (Sound::Fire, LIT),
            (Sound::Hum, HUM),
        ] {
            let (samples, _) = render(sound, 5, 20.0);
            let heard = at_ear(active_db(&samples), level, Bus::Ambience);
            within(&format!("{sound:?}"), heard, (-42.0, -30.0));
        }
    }

    /// A realm sounds as its own place: where its row silences the first realm's birds and
    /// rings instead, no birdsong or call is heard and its chimes are, most among its trees
    /// and little indoors; the first realm has no chimes.
    #[test]
    fn a_realm_with_its_own_air_rings_and_has_no_birdsong() {
        let first = Here {
            wooded: 1.0,
            ..Here::open()
        };
        let far = Here {
            birdsong: 0.0,
            chimes: 0.45,
            ..first
        };
        assert!(place_level(Sound::Birds, &first) > 0.2);
        assert_eq!(place_level(Sound::Chimes, &first), 0.0);
        assert_eq!(place_level(Sound::Birds, &far), 0.0);
        assert_eq!(place_level(Sound::Calls(Chorus::Woods), &far), 0.0);
        let among = place_level(Sound::Chimes, &far);
        let open = place_level(Sound::Chimes, &Here { wooded: 0.0, ..far });
        let indoors = place_level(
            Sound::Chimes,
            &Here {
                outdoors: false,
                ..far
            },
        );
        assert!(
            among > open && open > indoors && indoors > 0.0,
            "{among} {open} {indoors}"
        );
        assert!(place_level(Sound::Wind, &far) > 0.0);
    }

    /// With no world yet (the title screen, a run's first frames) the place is nothing and
    /// asking it stops nothing: read outright, the world's resources stopped the game at its
    /// start whenever sound was on, which every muted check missed.
    #[test]
    fn the_place_is_asked_safely_where_there_is_no_world() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let said = world.run_system_once_with(state, None).unwrap().unwrap();
        let layers = said["layers"].as_object().unwrap();
        assert_eq!(layers.len(), LAYERS.len());
        assert!(layers.values().all(|level| level.as_f64() == Some(0.0)));
    }
}
