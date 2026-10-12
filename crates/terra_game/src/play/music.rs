//! Music (docs/audio.md): sparse, and following the place. A phrase now and then, then
//! a long silence; warmer near the village; low and uneasy where magic pools or when
//! something is hunting you, and then with hardly a pause. The phrases are made in code
//! (`audio::music`), so none repeats.

use super::Player;
use super::creatures::{Creature, Lair, State};
use super::settlement::Village;
use crate::audio::{
    self, Bus, Fade, Mix, Voice,
    music::Phrase,
    synth::{Sound, Synth},
};
use bevy::prelude::*;
use terra_gen::{Rng, Seed};

const PENTATONIC: &[i32] = &[0, 2, 4, 7, 9];
/// Darker: a half step above the root.
const UNEASY: &[i32] = &[0, 1, 3, 7, 8];

/// What the music is about, from where the player is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mood {
    Calm,
    Village,
    Danger,
}

impl Mood {
    fn phrase(self) -> Phrase {
        match self {
            Mood::Calm => Phrase {
                root_hz: 146.83,
                scale: PENTATONIC,
                seconds: 22.0,
                notes: (7, 12),
                register: (1, 3),
                pad: 0.8,
                drone: 0.0,
            },
            Mood::Village => Phrase {
                root_hz: 196.0,
                scale: PENTATONIC,
                seconds: 18.0,
                notes: (10, 16),
                register: (1, 3),
                pad: 1.0,
                drone: 0.0,
            },
            Mood::Danger => Phrase {
                root_hz: 110.0,
                scale: UNEASY,
                seconds: 16.0,
                notes: (3, 6),
                register: (0, 2),
                pad: 0.0,
                drone: 1.0,
            },
        }
    }

    /// Seconds of silence after a phrase.
    fn silence_s(self, dice: &mut Rng) -> f32 {
        match self {
            Mood::Calm => dice.range(50.0, 120.0),
            Mood::Village => dice.range(30.0, 80.0),
            Mood::Danger => dice.range(1.0, 4.0),
        }
    }

    /// How long the place has to stay changed before the music follows: danger at once.
    fn settle_s(self) -> f32 {
        if self == Mood::Danger { 0.5 } else { 4.0 }
    }
}

/// The mood for where the player is: danger when hunted or near where magic pools,
/// the village's when near it, and calm otherwise.
pub fn mood_at(here: Vec3, village: Option<Vec3>, lair: Option<Vec3>, hunted: bool) -> Mood {
    if hunted || lair.is_some_and(|lair| lair.distance(here) < 45.0) {
        Mood::Danger
    } else if village.is_some_and(|village| village.distance(here) < 70.0) {
        Mood::Village
    } else {
        Mood::Calm
    }
}

/// A phrase that is playing.
#[derive(Component)]
struct Playing;

#[derive(Resource)]
struct Director {
    mood: Mood,
    /// How long the place has wanted another mood.
    wanting_s: f32,
    /// Seconds until the next phrase may start.
    wait_s: f32,
    dice: Rng,
    played: u64,
}

pub fn plugin(app: &mut App) {
    app.insert_resource(Director {
        mood: Mood::Calm,
        wanting_s: 0.0,
        wait_s: 8.0,
        dice: Seed(23).derive("music").rng(),
        played: 0,
    })
    .add_systems(Update, (follow, play.run_if(audio::speaking)).chain());
}

/// Changes mood when the place has changed for long enough, cutting short whatever
/// was playing.
fn follow(
    mut commands: Commands,
    time: Res<Time>,
    village: Option<Res<Village>>,
    lair: Option<Res<Lair>>,
    player: Single<&GlobalTransform, With<Player>>,
    creatures: Query<(&Creature, &GlobalTransform)>,
    playing: Query<Entity, With<Playing>>,
    mut director: ResMut<Director>,
) {
    let here = player.translation();
    let hunted = creatures.iter().any(|(creature, at)| {
        matches!(creature.state, State::Chase | State::Attack(_))
            && at.translation().distance(here) < 30.0
    });
    let wanted = mood_at(here, village.map(|v| v.0), lair.map(|l| l.0), hunted);
    if wanted == director.mood {
        director.wanting_s = 0.0;
        return;
    }
    director.wanting_s += time.delta_secs();
    if director.wanting_s < wanted.settle_s() {
        return;
    }
    info!("The music turns {wanted:?}.");
    director.mood = wanted;
    director.wanting_s = 0.0;
    director.wait_s = if wanted == Mood::Danger {
        0.5
    } else {
        director.dice.range(6.0, 14.0)
    };
    for phrase in &playing {
        commands.entity(phrase).try_insert(Fade {
            to: 0.0,
            per_second: 0.5,
            then_stop: true,
        });
    }
}

/// Starts the next phrase once the silence after the last is over.
fn play(
    mut commands: Commands,
    time: Res<Time>,
    mix: Res<Mix>,
    mut synths: ResMut<Assets<Synth>>,
    playing: Query<(), (With<Playing>, Without<Fade>)>,
    mut director: ResMut<Director>,
) {
    if !playing.is_empty() {
        return;
    }
    director.wait_s -= time.delta_secs();
    if director.wait_s > 0.0 {
        return;
    }
    let mood = director.mood;
    let phrase = mood.phrase();
    director.played += 1;
    let synth = Synth {
        sound: Sound::Music(phrase),
        seed: director.played,
    };
    let voice = Voice {
        bus: Bus::Music,
        level: 1.0,
    };
    let sound = audio::play_synth(&mut commands, &mut synths, &mix, synth, voice, None, 1.0);
    commands.entity(sound).insert(Playing);
    // The wait counts down only once the phrase has finished.
    director.wait_s = mood.silence_s(&mut director.dice);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn danger_wins_then_the_village_then_calm() {
        let here = Vec3::new(0.0, 2000.0, 0.0);
        let near = here + Vec3::X * 20.0;
        let far = here + Vec3::X * 500.0;
        assert_eq!(mood_at(here, Some(near), Some(near), false), Mood::Danger);
        assert_eq!(mood_at(here, Some(near), Some(far), true), Mood::Danger);
        assert_eq!(mood_at(here, Some(near), Some(far), false), Mood::Village);
        assert_eq!(mood_at(here, Some(far), Some(far), false), Mood::Calm);
        assert_eq!(mood_at(here, None, None, false), Mood::Calm);
    }

    #[test]
    fn danger_follows_at_once_and_its_silences_are_short() {
        let mut dice = Seed(1).rng();
        assert!(Mood::Danger.settle_s() < Mood::Calm.settle_s());
        assert!(Mood::Danger.silence_s(&mut dice) < 5.0);
        assert!(Mood::Calm.silence_s(&mut dice) > 30.0);
    }

    #[test]
    fn music_sits_under_the_world_s_sounds() {
        use crate::audio::synth::tests::{active_db, at_ear, render, within};
        for mood in [Mood::Calm, Mood::Village, Mood::Danger] {
            let phrase = mood.phrase();
            let (samples, _) = render(Sound::Music(phrase), 2, phrase.length_s());
            let heard = at_ear(active_db(&samples), 1.0, Bus::Music);
            within(&format!("{mood:?} music"), heard, (-40.0, -28.0));
        }
    }
}
