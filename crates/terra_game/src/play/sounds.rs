//! What each noise sounds like, and playing it (docs/audio.md).
//!
//! Every noise in the world comes through here as a `Noise`: this module picks the
//! sound for it, from the recorded clips in the table below or from a sound made in
//! code, and plays it where it happened. Nothing else in the game plays a sound effect.
//! Volumes come from the options menu.
//!
//! The clips are CC0 recordings by Kenney (THIRD_PARTY.md). A scripted run plays
//! nothing, but counts what it would have played and prints it at the end, so a script
//! can check that walking made steps and cutting made strikes.

use super::controls::Settings;
use super::crafting::Station;
use super::inventory::Cut;
use super::noise::{Cue, Feet, Noise};
use crate::audio::{
    self, Bus, Mix, Voice,
    bank::{Bank, Clips},
    synth::{Sound, Synth},
};
use crate::planet::MainCamera;
use crate::shell::Options;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.init_resource::<Heard>()
        .add_systems(Startup, load.run_if(audio::speaking))
        .add_systems(Update, (mix, hear).chain())
        .add_systems(Last, report);
}

/// How a cue is heard: recorded clips by name, or a sound made in code with its level
/// and reach.
enum Like {
    Clips(&'static str),
    Made(&'static str, Sound, f32, f32),
    Nothing,
}

fn sounds_like(cue: Cue) -> Like {
    use Like::{Clips, Made, Nothing};
    match cue {
        Cue::Step(Feet::Grass) => Clips("step.grass"),
        Cue::Step(Feet::Wood) => Clips("step.wood"),
        Cue::Step(Feet::Water) => Made("step.water", Sound::Splash, 0.35, 3.0),
        Cue::Step(Feet::Hoof) => Clips("step.hoof"),
        Cue::Step(Feet::Skitter) => Clips("step.skitter"),
        Cue::Swing => Made("swing", Sound::Whoosh, 0.38, 3.0),
        Cue::Hit => Clips("hit"),
        Cue::Kill => Clips("kill"),
        Cue::Hurt => Clips("hurt"),
        Cue::Down => Clips("down"),
        Cue::Strike(Cut::Tree | Cut::GreatTree) => Clips("strike.wood"),
        Cue::Strike(Cut::Rock | Cut::Ore) => Clips("strike.rock"),
        Cue::Strike(Cut::Plant | Cut::Flesh) => Clips("strike.plant"),
        Cue::Felled { tall: true } => Clips("felled.tree"),
        Cue::Felled { tall: false } => Clips("felled.small"),
        Cue::Hammer => Clips("hammer"),
        Cue::Built => Clips("built"),
        Cue::Placed => Clips("placed"),
        Cue::TakenApart => Clips("taken_apart"),
        Cue::Working(Station::Forge) => Clips("anvil"),
        Cue::Working(Station::Workbench) => Clips("bench"),
        // Fires are heard as they burn (soundscape.rs), not in strokes.
        Cue::Working(Station::Smelter | Station::Campfire) => Nothing,
        Cue::Made => Clips("made"),
        Cue::Snort => Made("snort", Sound::Snort, 0.38, 5.0),
        Cue::Hiss => Made("hiss", Sound::Hiss, 0.44, 5.0),
        Cue::Bell => Clips("bell"),
        Cue::Click => Clips("click"),
        Cue::Open => Clips("open"),
        Cue::Close => Clips("close"),
    }
}

/// The recorded sounds. Each takes one of its files at random, at a pitch in range.
const CLIPS: &[Clips] = &[
    clips(
        "step.grass",
        &[
            "footstep_grass_000",
            "footstep_grass_001",
            "footstep_grass_002",
            "footstep_grass_003",
            "footstep_grass_004",
        ],
        Bus::Effects,
        0.24,
        (0.9, 1.08),
        2.0,
    ),
    clips(
        "step.wood",
        &[
            "footstep_wood_000",
            "footstep_wood_001",
            "footstep_wood_002",
            "footstep_wood_003",
            "footstep_wood_004",
        ],
        Bus::Effects,
        0.14,
        (0.9, 1.08),
        2.0,
    ),
    clips(
        "step.hoof",
        &[
            "footstep_grass_000",
            "footstep_grass_002",
            "footstep_grass_004",
        ],
        Bus::Effects,
        0.12,
        (1.25, 1.45),
        2.0,
    ),
    clips(
        "step.skitter",
        &[
            "impactGeneric_light_000",
            "impactGeneric_light_001",
            "impactGeneric_light_002",
            "impactGeneric_light_003",
            "impactGeneric_light_004",
        ],
        Bus::Effects,
        0.12,
        (1.7, 2.3),
        1.5,
    ),
    clips(
        "hit",
        &[
            "impactPunch_medium_000",
            "impactPunch_medium_001",
            "impactPunch_medium_002",
            "impactPunch_medium_003",
            "impactPunch_medium_004",
        ],
        Bus::Effects,
        0.4,
        (0.9, 1.1),
        3.0,
    ),
    clips(
        "kill",
        &[
            "impactSoft_heavy_000",
            "impactSoft_heavy_001",
            "impactSoft_heavy_002",
            "impactSoft_heavy_003",
            "impactSoft_heavy_004",
        ],
        Bus::Effects,
        0.46,
        (0.8, 0.95),
        3.0,
    ),
    clips(
        "hurt",
        &[
            "impactPunch_heavy_000",
            "impactPunch_heavy_001",
            "impactPunch_heavy_002",
            "impactPunch_heavy_003",
            "impactPunch_heavy_004",
        ],
        Bus::Effects,
        0.54,
        (0.85, 1.0),
        3.0,
    ),
    clips(
        "down",
        &[
            "impactSoft_heavy_000",
            "impactSoft_heavy_002",
            "impactSoft_heavy_004",
        ],
        Bus::Effects,
        0.58,
        (0.65, 0.75),
        3.0,
    ),
    clips(
        "strike.wood",
        &[
            "chop",
            "impactWood_medium_000",
            "impactWood_medium_001",
            "impactWood_medium_002",
        ],
        Bus::Effects,
        0.45,
        (0.9, 1.1),
        3.0,
    ),
    clips(
        "strike.rock",
        &[
            "impactMining_000",
            "impactMining_001",
            "impactMining_002",
            "impactMining_003",
            "impactMining_004",
        ],
        Bus::Effects,
        0.5,
        (0.9, 1.1),
        3.0,
    ),
    clips(
        "strike.plant",
        &["knifeSlice", "knifeSlice2"],
        Bus::Effects,
        0.36,
        (0.9, 1.15),
        2.5,
    ),
    clips(
        "felled.tree",
        &[
            "impactWood_heavy_000",
            "impactWood_heavy_001",
            "impactWood_heavy_002",
            "impactWood_heavy_003",
            "impactWood_heavy_004",
        ],
        Bus::Effects,
        0.64,
        (0.6, 0.75),
        6.0,
    ),
    clips(
        "felled.small",
        &[
            "impactSoft_medium_000",
            "impactSoft_medium_001",
            "impactSoft_medium_002",
            "impactSoft_medium_003",
            "impactSoft_medium_004",
        ],
        Bus::Effects,
        0.31,
        (0.9, 1.1),
        3.0,
    ),
    clips(
        "hammer",
        &[
            "impactPlank_medium_000",
            "impactPlank_medium_001",
            "impactPlank_medium_002",
            "impactPlank_medium_003",
            "impactPlank_medium_004",
        ],
        Bus::Effects,
        0.27,
        (0.9, 1.1),
        3.0,
    ),
    clips(
        "built",
        &[
            "impactWood_heavy_000",
            "impactWood_heavy_002",
            "impactWood_heavy_004",
        ],
        Bus::Effects,
        0.45,
        (1.0, 1.15),
        4.0,
    ),
    clips(
        "placed",
        &[
            "impactWood_light_000",
            "impactWood_light_001",
            "impactWood_light_002",
            "impactWood_light_003",
            "impactWood_light_004",
        ],
        Bus::Effects,
        0.37,
        (0.9, 1.1),
        3.0,
    ),
    clips(
        "taken_apart",
        &["creak1", "creak2", "creak3"],
        Bus::Effects,
        0.58,
        (0.9, 1.1),
        3.0,
    ),
    clips(
        "anvil",
        &[
            "impactMetal_medium_000",
            "impactMetal_medium_001",
            "impactMetal_medium_002",
            "impactMetal_medium_003",
            "impactMetal_medium_004",
        ],
        Bus::Effects,
        0.36,
        (0.95, 1.05),
        4.0,
    ),
    clips(
        "bench",
        &[
            "impactWood_light_000",
            "impactWood_light_002",
            "impactWood_light_004",
            "impactPlank_medium_001",
        ],
        Bus::Effects,
        0.19,
        (1.0, 1.2),
        3.0,
    ),
    clips(
        "made",
        &["metalClick", "metalLatch"],
        Bus::Effects,
        0.31,
        (0.95, 1.05),
        3.0,
    ),
    clips(
        "bell",
        &[
            "impactBell_heavy_000",
            "impactBell_heavy_001",
            "impactBell_heavy_002",
            "impactBell_heavy_003",
            "impactBell_heavy_004",
        ],
        Bus::Ambience,
        0.3,
        (0.55, 0.6),
        25.0,
    ),
    clips(
        "click",
        &["bookPlace1", "bookPlace2", "bookPlace3"],
        Bus::Interface,
        0.16,
        (1.4, 1.6),
        1.0,
    ),
    clips(
        "open",
        &["handleSmallLeather", "clothBelt"],
        Bus::Interface,
        0.8,
        (0.95, 1.05),
        1.0,
    ),
    clips(
        "close",
        &["handleSmallLeather2", "clothBelt2"],
        Bus::Interface,
        0.8,
        (0.95, 1.05),
        1.0,
    ),
];

const fn clips(
    name: &'static str,
    files: &'static [&'static str],
    bus: Bus,
    level: f32,
    pitch: (f32, f32),
    reach_m: f32,
) -> Clips {
    Clips {
        name,
        files,
        bus,
        level,
        pitch,
        reach_m,
    }
}

/// What has been heard, by name, for scripted runs to report.
#[derive(Resource, Default)]
pub struct Heard(Vec<(&'static str, u32)>);

impl Heard {
    fn count(&mut self, name: &'static str) {
        match self.0.iter_mut().find(|(heard, _)| *heard == name) {
            Some((_, count)) => *count += 1,
            None => self.0.push((name, 1)),
        }
    }
}

fn load(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Bank::load(CLIPS, &assets));
}

/// The options menu's volumes, into the mix.
fn mix(settings: Res<Settings>, mut mix: ResMut<Mix>) {
    if settings.is_changed() {
        *mix = Mix {
            master: settings.master_volume,
            music: settings.music_volume,
            ambience: settings.ambience_volume,
            effects: settings.effects_volume,
        };
    }
}

fn hear(
    mut commands: Commands,
    mut noises: MessageReader<Noise>,
    mix: Res<Mix>,
    ear: Query<&GlobalTransform, With<MainCamera>>,
    bank: Option<ResMut<Bank>>,
    synths: Option<ResMut<Assets<Synth>>>,
    mut heard: ResMut<Heard>,
    mut seed: Local<u64>,
) {
    let ear = ear.single().ok().map(GlobalTransform::translation);
    let (mut bank, mut synths) = (bank, synths);
    for noise in noises.read() {
        let (name, reach_m) = match sounds_like(noise.cue) {
            Like::Clips(name) => (name, reach_of(name)),
            Like::Made(name, _, _, reach_m) => (name, reach_m),
            Like::Nothing => continue,
        };
        // Far enough off to be all but silent: not worth playing.
        if let (Some(at), Some(ear)) = (noise.at, ear)
            && at.distance(ear) > reach_m * 12.0
        {
            continue;
        }
        heard.count(name);
        match sounds_like(noise.cue) {
            Like::Clips(name) => {
                if let Some(bank) = bank.as_mut() {
                    bank.play(&mut commands, &mix, name, noise.at);
                }
            }
            Like::Made(_, sound, level, reach_m) => {
                if let Some(synths) = synths.as_mut() {
                    *seed += 1;
                    audio::play_synth(
                        &mut commands,
                        synths,
                        &mix,
                        Synth { sound, seed: *seed },
                        Voice {
                            bus: Bus::Effects,
                            level,
                        },
                        noise.at,
                        reach_m,
                    );
                }
            }
            Like::Nothing => {}
        }
    }
}

fn reach_of(name: &str) -> f32 {
    CLIPS
        .iter()
        .find(|clips| clips.name == name)
        .map_or(3.0, |clips| clips.reach_m)
}

/// At the end of a scripted run, what was heard, so a script can check it.
fn report(options: Res<Options>, mut exits: MessageReader<AppExit>, heard: Res<Heard>) {
    if !options.scripted() || exits.read().next().is_none() {
        return;
    }
    let mut heard: Vec<_> = heard
        .0
        .iter()
        .map(|(name, count)| format!("{name} {count}"))
        .collect();
    heard.sort();
    println!(
        "SANDBOX heard: {}",
        if heard.is_empty() {
            "nothing".to_string()
        } else {
            heard.join(", ")
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::io::file::FileAssetReader;

    #[test]
    fn every_clip_is_in_the_assets() {
        let assets = FileAssetReader::get_base_path().join("assets");
        for clips in CLIPS {
            for path in clips.paths() {
                assert!(
                    assets.join(&path).exists(),
                    "{} is missing {path}",
                    clips.name
                );
            }
        }
    }

    #[test]
    fn every_cue_that_names_clips_names_ones_that_exist() {
        let cues = [
            Cue::Step(Feet::Grass),
            Cue::Step(Feet::Wood),
            Cue::Step(Feet::Hoof),
            Cue::Step(Feet::Skitter),
            Cue::Hit,
            Cue::Kill,
            Cue::Hurt,
            Cue::Down,
            Cue::Strike(Cut::Tree),
            Cue::Strike(Cut::Ore),
            Cue::Strike(Cut::Plant),
            Cue::Felled { tall: true },
            Cue::Felled { tall: false },
            Cue::Hammer,
            Cue::Built,
            Cue::Placed,
            Cue::TakenApart,
            Cue::Working(Station::Forge),
            Cue::Working(Station::Workbench),
            Cue::Made,
            Cue::Bell,
            Cue::Click,
            Cue::Open,
            Cue::Close,
        ];
        for cue in cues {
            let Like::Clips(name) = sounds_like(cue) else {
                panic!("{cue:?} should be recorded clips");
            };
            assert!(
                CLIPS.iter().any(|clips| clips.name == name),
                "{cue:?}: no clips called {name}"
            );
        }
    }

    #[test]
    fn counts_what_is_heard() {
        let mut heard = Heard::default();
        heard.count("hit");
        heard.count("hit");
        heard.count("bell");
        assert_eq!(heard.0, [("hit", 2), ("bell", 1)]);
    }
}

/// Every sound in its place in the mix (docs/audio.md), measured from the clips and the
/// sounds made in code, at the volumes the options menu starts with.
#[cfg(test)]
mod loudness {
    use super::*;
    use crate::audio::synth::tests::{active_db, at_ear, render, within};
    use bevy::asset::io::file::FileAssetReader;
    use bevy::audio::{AudioSource, Decodable};

    /// The band each kind of sound is heard in, in dBFS while it sounds.
    fn band(name: &str, bus: Bus) -> (f32, f32) {
        match bus {
            _ if name.starts_with("step.") => (-42.0, -28.0),
            Bus::Interface => (-38.0, -28.0),
            // A bell is meant to carry.
            Bus::Ambience => (-36.0, -26.0),
            _ => (-30.0, -18.0),
        }
    }

    fn clip_db(file: &str) -> f32 {
        let path = FileAssetReader::get_base_path().join(format!("assets/sounds/{file}.ogg"));
        let source = AudioSource {
            bytes: std::fs::read(path).unwrap().into(),
        };
        active_db(&source.decoder().collect::<Vec<f32>>())
    }

    #[test]
    fn every_recorded_sound_sits_in_its_place_in_the_mix() {
        for clips in CLIPS {
            // The loudest take, since that is the one that stands out.
            let loudest = clips
                .files
                .iter()
                .map(|file| clip_db(file))
                .fold(f32::MIN, f32::max);
            let heard = at_ear(loudest, clips.level, clips.bus);
            within(clips.name, heard, band(clips.name, clips.bus));
        }
    }

    #[test]
    fn every_sound_made_in_code_sits_in_its_place_in_the_mix() {
        for cue in [Cue::Step(Feet::Water), Cue::Swing, Cue::Snort, Cue::Hiss] {
            let Like::Made(name, sound, level, _) = sounds_like(cue) else {
                panic!("{cue:?} should be made in code");
            };
            let (samples, _) = render(sound, 3, 2.0);
            let heard = at_ear(active_db(&samples), level, Bus::Effects);
            within(name, heard, band(name, Bus::Effects));
        }
    }
}
