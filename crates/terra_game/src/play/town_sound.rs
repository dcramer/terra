//! A town's sound (docs/demos.md, a living town; docs/audio.md): the murmur of its people,
//! a bed round the listener as The Witcher 3's crowd is, as loud as there are people out of
//! doors near, and nothing when there are none. No words, for now (audio.md's question 4).
//! And what its people do that is heard, as noises (D113): the smith's hammer on the anvil,
//! an axe at a chopping block.

use super::crafting::Stations;
use super::inventory::Cut;
use super::noise::{Cue, Noise};
use super::town::Market;
use crate::planet::MainCamera;
use bevy::prelude::*;
use terra_town::Shows;
use terra_world::audio::{
    self, Bus, Fade, Mix, Voice,
    synth::{Sound, Synth},
};

/// How loud the murmur is among a full crowd, and how many make one.
const MURMUR: f32 = 0.35;
const CROWD: f32 = 24.0;
/// Who is near enough to be heard in it.
const NEAR_M: f32 = 35.0;
/// How often the crowd is counted.
const COUNT_EVERY_S: f32 = 0.25;

/// How far apart the smith's strokes and the woodcutter's are, in seconds, and how far off
/// either is heard at all.
const ANVIL_S: f32 = 0.9;
const CHOP_S: f32 = 1.6;
const HEARD_M: f32 = 60.0;

/// The murmur round the listener.
#[derive(Component)]
struct Murmur;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            start.run_if(resource_added::<Market>),
            follow.run_if(resource_exists::<Market>),
        )
            .chain()
            .run_if(audio::speaking),
    )
    .add_systems(Update, work.run_if(resource_exists::<Market>));
}

fn start(mut commands: Commands, mut synths: ResMut<Assets<Synth>>, mix: Res<Mix>) {
    let layer = audio::play_synth(
        &mut commands,
        &mut synths,
        &mix,
        Synth {
            sound: Sound::Murmur,
            seed: 11,
        },
        Voice {
            bus: Bus::Ambience,
            level: 0.0,
        },
        None,
        1.0,
    );
    commands.entity(layer).insert(Murmur);
}

/// How loud the murmur is with so many people out near the listener.
fn level(near: usize) -> f32 {
    MURMUR * libm::sqrtf((near as f32 / CROWD).min(1.0))
}

/// The murmur as loud as the crowd near the listener.
fn follow(
    mut commands: Commands,
    time: Res<Time>,
    market: Res<Market>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    layers: Query<(Entity, &Voice), With<Murmur>>,
    mut wait: Local<f32>,
) {
    *wait -= time.delta_secs();
    if *wait > 0.0 {
        return;
    }
    *wait = COUNT_EVERY_S;
    let Ok(ear) = camera.single() else {
        return;
    };
    let here = market.local(ear.translation());
    let near = (0..market.life.people.len())
        .filter(|&p| {
            !matches!(market.life.shows(p), Shows::Hidden)
                && market.life.people[p].at.distance(here) < NEAR_M
        })
        .count();
    let wanted = level(near);
    for (entity, voice) in &layers {
        if (wanted - voice.level).abs() > 0.01 {
            commands.entity(entity).insert(Fade {
                to: wanted,
                per_second: 0.3,
                then_stop: false,
            });
        }
    }
}

/// Each person at a forge or a chopping block heard at work, a stroke at a time, each on a
/// beat of their own.
fn work(
    time: Res<Time>,
    planet: Res<crate::planet::Planet>,
    market: Res<Market>,
    stations: Res<Stations>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut noise: MessageWriter<Noise>,
    mut beats: Local<std::collections::BTreeMap<usize, f32>>,
) {
    let Ok(ear) = camera.single() else {
        return;
    };
    let here = market.local(ear.translation());
    let forge = stations.named("Forge");
    let life = &market.life;
    for person in 0..life.people.len() {
        let (every, cue) = match life.activity(person) {
            Some("Smith") => match forge {
                Some(forge) => (ANVIL_S, Cue::Working(forge)),
                None => continue,
            },
            Some("Chop") => (CHOP_S, Cue::Strike(Cut::Tree)),
            _ => {
                beats.remove(&person);
                continue;
            }
        };
        let p = &life.people[person];
        if !matches!(p.state, terra_town::State::Doing { .. }) || p.at.distance(here) > HEARD_M {
            continue;
        }
        let beat = beats
            .entry(person)
            .or_insert((person as f32 * 0.37) % every);
        *beat += time.delta_secs();
        if *beat >= every {
            *beat -= every;
            let at = market.ground(&planet, p.at);
            noise.write(Noise::at(cue, at + at.normalize() * 1.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crowd_murmurs_with_the_worlds_background_and_an_empty_street_is_quiet() {
        use terra_world::audio::synth::tests::{active_db, at_ear, render, within};
        let (samples, _) = render(Sound::Murmur, 5, 20.0);
        let heard = at_ear(active_db(&samples), level(40), Bus::Ambience);
        within("Murmur", heard, (-44.0, -32.0));
        assert_eq!(level(0), 0.0);
        assert!(level(6) < level(20));
    }
}
