//! The sound of the place (docs/audio.md): air, birds and water all round the listener,
//! louder or quieter for where they stand, and sounds that belong to things: fires
//! burning, the hum where magic pools, a settlement's bell.
//!
//! Layers that surround the listener are always playing and fade to suit the place:
//! wind outdoors, birds where it is safe, water near the shore. Sources that belong to
//! a thing carry `Ambient`, and are heard only within earshot, so a world can hold any
//! number of them.

use super::Player;
use super::crafting::{Station, Works};
use super::creatures::Lair;
use super::noise::{Cue, Noise};
use super::props::Prop;
use super::settlement::Village;
use crate::audio::{
    self, Bus, Fade, Mix, Voice,
    synth::{Sound, Synth},
};
use crate::planet::Planet;
use bevy::prelude::*;
use terra_gen::{Rng, Seed};

/// How often the place is looked at again.
const LOOK_EVERY_S: f32 = 0.25;
/// Rings of the ground looked at for water, and their distance in metres.
const SHORE_RINGS_M: [f32; 4] = [6.0, 15.0, 30.0, 55.0];

pub fn plugin(app: &mut App) {
    app.insert_resource(Bells(Seed(19).derive("bells").rng()))
        .add_systems(Startup, surround.run_if(audio::speaking))
        .add_systems(
            Update,
            (
                (follow_place, mark_sources, stoke, earshot).run_if(audio::speaking),
                ring,
            ),
        );
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
    for (sound, seed) in [(Sound::Wind, 1), (Sound::Birds, 2), (Sound::Water, 3)] {
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
        commands.entity(layer).insert(Layer(sound));
    }
}

/// How loud each surrounding layer should be where the player stands.
fn follow_place(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<Planet>,
    lair: Option<Res<Lair>>,
    player: Single<(&Player, &GlobalTransform)>,
    layers: Query<(Entity, &Layer, &Voice)>,
    mut wait: Local<f32>,
) {
    *wait -= time.delta_secs();
    if *wait > 0.0 {
        return;
    }
    *wait = LOOK_EVERY_S;
    let (player, at) = *player;
    let here = at.translation();
    let outdoors = !player.indoors;
    let lair_m = lair.map_or(f32::MAX, |lair| lair.0.distance(here));
    let shore_m = shore_distance(&planet, here);
    for (entity, layer, voice) in &layers {
        let wanted = place_level(layer.0, outdoors, lair_m, shore_m);
        if (wanted - voice.level).abs() > 0.01 {
            commands.entity(entity).insert(Fade {
                to: wanted,
                per_second: 0.25,
                then_stop: false,
            });
        }
    }
}

/// How loud a surrounding layer is: outdoors or in, how near the place where magic
/// pools (birds go quiet there), and how near water.
fn place_level(sound: Sound, outdoors: bool, lair_m: f32, shore_m: Option<f32>) -> f32 {
    let inside = if outdoors { 1.0 } else { 0.2 };
    match sound {
        Sound::Wind => 0.5 * inside,
        Sound::Birds => 0.55 * inside * ((lair_m - 40.0) / 80.0).clamp(0.0, 1.0),
        Sound::Water => {
            shore_m.map_or(0.0, |metres| (1.0 - (metres - 6.0) / 55.0).clamp(0.0, 1.0))
                * 0.7
                * inside
        }
        _ => 0.0,
    }
}

/// How far the nearest water is, looking round in rings, if there is any near.
fn shore_distance(planet: &Planet, here: Vec3) -> Option<f32> {
    let up = here.normalize_or(Vec3::Y);
    let east = up.any_orthonormal_vector();
    let sea = planet.sea_radius_m();
    if planet.drawn(up).length() < sea {
        return Some(0.0);
    }
    SHORE_RINGS_M.into_iter().find(|&metres| {
        (0..12).any(|i| {
            let round = Quat::from_axis_angle(up, i as f32 * std::f32::consts::TAU / 12.0);
            let there = here + round * east * metres;
            planet.drawn(there.normalize()).length() < sea
        })
    })
}

/// Gives the things that make sounds of their own their sound: fires at stations, the
/// place where magic pools, and the settlement's bell.
fn mark_sources(
    mut commands: Commands,
    stations: Query<(Entity, &Prop), (With<Works>, Without<Ambient>)>,
    lair: Option<Res<Lair>>,
    village: Option<Res<Village>>,
    mut marked: Local<(bool, bool)>,
) {
    for (entity, prop) in &stations {
        let level = match prop.item.station {
            Some(Station::Campfire) => 0.7,
            // Lit only while they work (`stoke`).
            Some(Station::Forge | Station::Smelter) => 0.0,
            _ => continue,
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
            Ambient::new(Sound::Hum, 0.8, 8.0),
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

/// A forge or smelter burns while it works.
fn stoke(mut fires: Query<(&Prop, &Works, &mut Ambient)>) {
    for (prop, works, mut ambient) in &mut fires {
        if matches!(prop.item.station, Some(Station::Forge | Station::Smelter)) {
            let level = if works.queue.is_empty() { 0.0 } else { 0.8 };
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
    ear: Single<&GlobalTransform, With<Camera3d>>,
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
        let open = place_level(Sound::Birds, true, 500.0, None);
        assert!(open > 0.5);
        assert_eq!(place_level(Sound::Birds, true, 30.0, None), 0.0);
        assert!(place_level(Sound::Birds, false, 500.0, None) < open / 4.0);
    }

    #[test]
    fn water_is_loudest_at_the_shore_and_gone_far_from_it() {
        assert!(place_level(Sound::Water, true, 500.0, Some(0.0)) > 0.6);
        assert!(place_level(Sound::Water, true, 500.0, Some(30.0)) > 0.0);
        assert_eq!(place_level(Sound::Water, true, 500.0, None), 0.0);
    }
}
