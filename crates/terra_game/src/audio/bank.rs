//! Recorded clips, in named sets. A set is one sound with a few takes: each time it plays,
//! one take is picked and its pitch and loudness shifted a little, so a thing done twice
//! does not sound like a recording played twice.
//!
//! What the sets are is the game's to say (a table of `Clips`); this loads and plays
//! them.

use super::{Bus, Mix, Voice, place};
use bevy::{audio::Volume, platform::collections::HashMap, prelude::*};
use terra_gen::{Rng, Seed};

/// One named sound: its takes, and how it plays.
pub struct Clips {
    pub name: &'static str,
    /// Files under `assets/sounds/`, without `.ogg`.
    pub files: &'static [&'static str],
    pub bus: Bus,
    /// How loud, from 0 to 1.
    pub level: f32,
    /// How far the pitch may be shifted, as a range of playback speeds around 1.
    pub pitch: (f32, f32),
    /// Within how many metres it is heard at full loudness. Further off it fades with
    /// the square of the distance.
    pub reach_m: f32,
}

impl Clips {
    pub fn paths(&self) -> impl Iterator<Item = String> + '_ {
        self.files.iter().map(|file| format!("sounds/{file}.ogg"))
    }
}

#[derive(Resource)]
pub struct Bank {
    sets: HashMap<&'static str, (&'static Clips, Vec<Handle<AudioSource>>)>,
    dice: Rng,
}

impl Bank {
    pub fn load(table: &'static [Clips], assets: &AssetServer) -> Self {
        let sets = table
            .iter()
            .map(|clips| {
                (
                    clips.name,
                    (clips, clips.paths().map(|path| assets.load(path)).collect()),
                )
            })
            .collect();
        Bank {
            sets,
            dice: Seed(5).derive("bank").rng(),
        }
    }

    /// Every take of every sound, as asked to be loaded.
    pub fn takes(&self) -> impl Iterator<Item = &Handle<AudioSource>> {
        self.sets.values().flat_map(|(_, takes)| takes)
    }

    /// Plays a take of the named sound, at a place in the world or, with none, in the ear.
    pub fn play(&mut self, commands: &mut Commands, mix: &Mix, name: &str, at: Option<Vec3>) {
        let Some((clips, takes)) = self.sets.get(name) else {
            warn!("No sound called {name}.");
            return;
        };
        let take = takes[self.dice.below(takes.len() as u32) as usize].clone();
        let level = clips.level * self.dice.range(0.85, 1.0);
        let voice = Voice {
            bus: clips.bus,
            level,
        };
        let settings = PlaybackSettings {
            speed: self.dice.range(clips.pitch.0, clips.pitch.1),
            volume: Volume::Linear(level * mix.level(clips.bus)),
            ..PlaybackSettings::DESPAWN
        };
        let mut sound = commands.spawn((AudioPlayer::new(take), voice));
        place(&mut sound, settings, at, clips.reach_m);
    }
}
