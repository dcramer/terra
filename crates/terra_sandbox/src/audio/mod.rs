//! Sound, for any experiment: how loud each kind of sound is, where it is heard from,
//! and two ways of making it.
//!
//! A sound is an entity. It carries a `Voice`: which part of the mix it belongs to, and
//! how loud it is within that part. The `Mix` sets each part's level, from the options
//! menu, and a system keeps every playing sound at voice times part times the whole, so
//! turning music down turns down what is already playing. A `Fade` moves a voice's level
//! over time, for crossfades and for ambience that follows the place. A sound with a
//! place in the world is heard from the camera, which carries the listener.
//!
//! Sounds come from recorded clips, in named sets (`bank`), or are made in code as they
//! play (`synth`, `music`): air, water, fire, birds and music, which never repeat.
//!
//! Whether anything is heard at all is the shell's to say: scripted runs are silent, and
//! so is WSL unless asked, since it usually has no sound device. With sound off nothing
//! here plays, and the game still decides what it would have played.

pub mod bank;
pub mod music;
pub mod synth;

use bevy::{
    audio::{AddAudioSource, AudioSinkPlayback, SpatialScale, Volume},
    ecs::system::EntityCommands,
    prelude::*,
};
use synth::Synth;

/// The parts of the mix, each with its own level.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bus {
    Music,
    /// The world going on: air, water, fire.
    Ambience,
    /// Things done: steps, blows, tools.
    Effects,
    /// Menus and the like.
    Interface,
}

/// How loud each part of the mix is, from 0 to 1.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Mix {
    pub master: f32,
    pub music: f32,
    pub ambience: f32,
    pub effects: f32,
}

impl Default for Mix {
    fn default() -> Self {
        Mix {
            master: 0.8,
            music: 0.6,
            ambience: 0.8,
            effects: 0.9,
        }
    }
}

impl Mix {
    pub fn level(&self, bus: Bus) -> f32 {
        self.master
            * match bus {
                Bus::Music => self.music,
                Bus::Ambience => self.ambience,
                Bus::Effects | Bus::Interface => self.effects,
            }
    }
}

/// Whether sound is on at all.
#[derive(Resource, Clone, Copy)]
pub struct Speaker(pub bool);

/// A run condition: sound is on.
pub fn speaking(speaker: Res<Speaker>) -> bool {
    speaker.0
}

/// On a sound: its part of the mix, and how loud it is within it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Voice {
    pub bus: Bus,
    pub level: f32,
}

/// Moves a voice's level toward `to`, by `per_second`. With `then_stop`, the sound is
/// removed once it is silent.
#[derive(Component, Clone, Copy, Debug)]
pub struct Fade {
    pub to: f32,
    pub per_second: f32,
    pub then_stop: bool,
}

pub fn plugin(on: bool) -> impl Fn(&mut App) {
    move |app| {
        app.insert_resource(Speaker(on)).init_resource::<Mix>();
        if !on {
            return;
        }
        app.add_audio_source::<Synth>().add_systems(
            Update,
            (
                listen,
                fade,
                (keep_level::<AudioSink>, keep_level::<SpatialAudioSink>),
            )
                .chain(),
        );
    }
}

/// Starts a sound where it belongs: at a place in the world, heard at full loudness
/// within `reach_m`, or with no place, in the ear.
pub fn place(
    sound: &mut EntityCommands,
    settings: PlaybackSettings,
    at: Option<Vec3>,
    reach_m: f32,
) {
    match at {
        Some(at) => sound.insert((
            settings
                .with_spatial(true)
                .with_spatial_scale(SpatialScale::new(1.0 / reach_m.max(0.1))),
            Transform::from_translation(at),
        )),
        None => sound.insert(settings),
    };
}

/// Plays a sound made in code.
pub fn play_synth(
    commands: &mut Commands,
    synths: &mut Assets<Synth>,
    mix: &Mix,
    synth: Synth,
    voice: Voice,
    at: Option<Vec3>,
    reach_m: f32,
) -> Entity {
    let settings = PlaybackSettings {
        volume: Volume::Linear(voice.level * mix.level(voice.bus)),
        ..if synth.sound.endless() {
            PlaybackSettings::LOOP
        } else {
            PlaybackSettings::DESPAWN
        }
    };
    let mut sound = commands.spawn((AudioPlayer(synths.add(synth)), voice));
    place(&mut sound, settings, at, reach_m);
    sound.id()
}

/// The camera is where sounds are heard from.
fn listen(
    mut commands: Commands,
    cameras: Query<Entity, (With<Camera3d>, Without<SpatialListener>)>,
) {
    for camera in &cameras {
        commands.entity(camera).insert(SpatialListener::new(0.3));
    }
}

fn fade(mut commands: Commands, time: Res<Time>, mut fading: Query<(Entity, &mut Voice, &Fade)>) {
    for (entity, mut voice, fade) in &mut fading {
        let step = fade.per_second * time.delta_secs();
        let level = if voice.level < fade.to {
            (voice.level + step).min(fade.to)
        } else {
            (voice.level - step).max(fade.to)
        };
        if level != voice.level {
            voice.level = level;
        }
        if level == fade.to {
            if fade.then_stop && level <= 0.0 {
                commands.entity(entity).despawn();
            } else {
                commands.entity(entity).remove::<Fade>();
            }
        }
    }
}

/// Keeps each playing sound at its voice times its part of the mix.
fn keep_level<Sink: Component<Mutability = bevy::ecs::component::Mutable> + AudioSinkPlayback>(
    mix: Res<Mix>,
    mut sounds: Query<(Ref<Voice>, &mut Sink)>,
) {
    for (voice, mut sink) in &mut sounds {
        if mix.is_changed() || voice.is_changed() || sink.is_added() {
            sink.set_volume(Volume::Linear(voice.level * mix.level(voice.bus)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_of_the_mix_is_its_level_times_the_whole() {
        let mix = Mix {
            master: 0.5,
            music: 0.4,
            ambience: 1.0,
            effects: 0.8,
        };
        assert_eq!(mix.level(Bus::Music), 0.2);
        assert_eq!(mix.level(Bus::Ambience), 0.5);
        assert_eq!(mix.level(Bus::Interface), mix.level(Bus::Effects));
    }
}
