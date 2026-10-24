//! What each noise sounds like, and playing it (docs/audio.md).
//!
//! Every noise in the world comes through here as a `Noise`: this module picks the
//! sound for it, from the recorded clips in the table below or from a sound made in
//! code, and plays it where it happened. Nothing else in the game plays a sound effect.
//! Volumes come from the options menu.
//!
//! The clips are CC0 recordings (THIRD_PARTY.md). What a blow in a fight sounds like is a
//! table in data (`data/contacts.ron`): the first row fitting what struck what names its
//! clips. A scripted run plays
//! nothing, but counts what it would have played and prints it at the end, so a script
//! can check that walking made steps and cutting made strikes.

use super::controls::Settings;
use super::crafting::Stations;
use super::inventory::Cut;
use super::noise::{Cue, Feet, Noise};
use crate::audio::{
    self, Bus, Mix, Voice,
    bank::{Bank, Clips},
    synth::{Sound, Synth},
};
use crate::loading::Loading;
use crate::planet::MainCamera;
use crate::shell::Options;
use bevy::prelude::*;
use serde::Deserialize;
use terra_motion::moves::Taken;
use terra_motion::struck::{Struck, Surface};

pub fn plugin(app: &mut App) {
    app.init_resource::<Heard>()
        .insert_resource(Contacts::read())
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

/// What a blow in a fight sounds like: `data/contacts.ron`'s rows, each with the clips it
/// names, in order.
#[derive(Resource)]
pub struct Contacts(Vec<(Contact, &'static str)>);

/// A row of the table: what it fits (what it leaves out fits anything) and its sound.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Contact {
    taken: Option<Taken>,
    by: Option<Surface>,
    on: Option<Surface>,
    heavy: Option<bool>,
    sound: String,
}

impl Contact {
    fn fits(&self, struck: Struck) -> bool {
        self.taken.is_none_or(|taken| taken == struck.taken)
            && self.by.is_none_or(|by| by == struck.by)
            && self.on.is_none_or(|on| on == struck.on)
            && self.heavy.is_none_or(|heavy| heavy == struck.heavy)
    }
}

impl Contacts {
    const FILE: &str = "contacts.ron";

    pub fn read() -> Self {
        let rows: Vec<Contact> = terra_world::data::read(Self::FILE);
        Self(
            rows.into_iter()
                .map(|row| {
                    let name = CLIPS
                        .iter()
                        .find(|clips| clips.name == row.sound)
                        .unwrap_or_else(|| {
                            panic!("{}: there are no clips called {}", Self::FILE, row.sound)
                        })
                        .name;
                    (row, name)
                })
                .collect(),
        )
    }

    /// The clips of the first row that fits.
    fn sound(&self, struck: Struck) -> Option<&'static str> {
        self.0
            .iter()
            .find(|(row, _)| row.fits(struck))
            .map(|(_, name)| *name)
    }
}

fn sounds_like(cue: Cue, stations: &Stations, contacts: &Contacts) -> Like {
    use Like::{Clips, Made, Nothing};
    match cue {
        Cue::Step(Feet::Grass) => Clips("step.grass"),
        Cue::Step(Feet::Wood) => Clips("step.wood"),
        Cue::Step(Feet::Water) => Made("step.water", Sound::Splash, 0.35, 3.0),
        Cue::Step(Feet::Hoof) => Clips("step.hoof"),
        Cue::Step(Feet::Skitter) => Clips("step.skitter"),
        Cue::Swing { heavy: false } => Clips("swing.light"),
        Cue::Swing { heavy: true } => Clips("swing.heavy"),
        Cue::Hit => Clips("hit"),
        Cue::Kill => Clips("kill"),
        Cue::Struck(struck) => contacts.sound(struck).map_or(Nothing, Clips),
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
        // Work at a station is heard in strokes, as its kind says; fires are heard as they
        // burn (soundscape.rs), not in strokes.
        Cue::Working(station) => {
            let named = stations.strokes(station).map(|(sound, _)| sound);
            match CLIPS.iter().find(|clips| Some(clips.name) == named) {
                Some(clips) => Clips(clips.name),
                None => Nothing,
            }
        }
        Cue::Made => Clips("made"),
        Cue::Snort => Made("snort", Sound::Snort, 0.38, 5.0),
        Cue::Hiss => Made("hiss", Sound::Hiss, 0.44, 5.0),
        Cue::Chatter => Made("chatter", Sound::Chatter, 0.55, 4.0),
        Cue::Chirp => Made("chirp", Sound::Chirp, 0.22, 3.0),
        Cue::Quack => Made("quack", Sound::Quack, 0.34, 6.0),
        Cue::Croak => Made("croak", Sound::Croak, 0.5, 8.0),
        Cue::Bark => Made("bark", Sound::Bark, 0.4, 8.0),
        Cue::Wings => Made("wings", Sound::Wings, 0.42, 4.0),
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
    // The fight's (THIRD_PARTY.md): by what is struck and how hard, `data/contacts.ron`.
    clips(
        "swing.light",
        &[
            "swing_light_000",
            "swing_light_001",
            "swing_light_002",
            "swing_light_003",
            "swing_light_004",
            "swing_light_005",
        ],
        Bus::Effects,
        0.35,
        (0.92, 1.1),
        3.0,
    ),
    clips(
        "swing.heavy",
        &[
            "swing_heavy_000",
            "swing_heavy_001",
            "swing_heavy_002",
            "swing_heavy_003",
            "swing_heavy_004",
            "swing_heavy_005",
        ],
        Bus::Effects,
        1.0,
        (0.85, 1.0),
        3.5,
    ),
    clips(
        "parry.steel",
        &[
            "parry_steel_000",
            "parry_steel_001",
            "parry_steel_002",
            "parry_steel_003",
            "parry_steel_004",
            "parry_steel_005",
        ],
        Bus::Effects,
        1.0,
        (0.94, 1.06),
        8.0,
    ),
    clips(
        "block.wood",
        &[
            "block_wood_000",
            "block_wood_001",
            "block_wood_002",
            "block_wood_003",
            "block_wood_004",
            "block_wood_005",
        ],
        Bus::Effects,
        0.6,
        (0.9, 1.08),
        5.0,
    ),
    clips(
        "guard_break.wood",
        &[
            "guard_break_wood_000",
            "guard_break_wood_001",
            "guard_break_wood_002",
            "guard_break_wood_003",
        ],
        Bus::Effects,
        0.7,
        (0.9, 1.02),
        6.0,
    ),
    clips(
        "hit.flesh",
        &[
            "hit_flesh_000",
            "hit_flesh_001",
            "hit_flesh_002",
            "hit_flesh_003",
            "hit_flesh_004",
            "hit_flesh_005",
        ],
        Bus::Effects,
        0.55,
        (0.92, 1.08),
        4.0,
    ),
    clips(
        "hit.flesh.heavy",
        &[
            "hit_flesh_heavy_000",
            "hit_flesh_heavy_001",
            "hit_flesh_heavy_002",
            "hit_flesh_heavy_003",
            "hit_flesh_heavy_004",
        ],
        Bus::Effects,
        0.65,
        (0.88, 1.0),
        5.0,
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

fn load(mut commands: Commands, assets: Res<AssetServer>, mut loading: ResMut<Loading>) {
    let bank = Bank::load(CLIPS, &assets);
    // The first sounds are heard as the game starts, so the loading screen waits for them.
    for take in bank.takes() {
        loading.wait_for(take);
    }
    commands.insert_resource(bank);
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
    stations: Res<Stations>,
    contacts: Res<Contacts>,
    ear: Query<&GlobalTransform, With<MainCamera>>,
    bank: Option<ResMut<Bank>>,
    synths: Option<ResMut<Assets<Synth>>>,
    mut heard: ResMut<Heard>,
    mut seed: Local<u64>,
) {
    let ear = ear.single().ok().map(GlobalTransform::translation);
    let (mut bank, mut synths) = (bank, synths);
    for noise in noises.read() {
        let (name, reach_m) = match sounds_like(noise.cue, &stations, &contacts) {
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
        match sounds_like(noise.cue, &stations, &contacts) {
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

    #[test]
    fn every_clip_is_in_the_assets() {
        let assets = terra_world::data::assets();
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

    /// Every cue there is, with work at every station. The match gives each its place,
    /// and does not compile without a new kind of cue, feet or cut; once it has a place,
    /// this fails until it is listed.
    pub(super) fn every_cue(stations: &Stations) -> Vec<Cue> {
        let mut cues = vec![
            Cue::Step(Feet::Grass),
            Cue::Step(Feet::Wood),
            Cue::Step(Feet::Water),
            Cue::Step(Feet::Hoof),
            Cue::Step(Feet::Skitter),
            Cue::Swing { heavy: false },
            Cue::Swing { heavy: true },
            Cue::Hit,
            Cue::Kill,
            Cue::Hurt,
            Cue::Down,
            Cue::Strike(Cut::Tree),
            Cue::Strike(Cut::GreatTree),
            Cue::Strike(Cut::Rock),
            Cue::Strike(Cut::Ore),
            Cue::Strike(Cut::Plant),
            Cue::Strike(Cut::Flesh),
            Cue::Felled { tall: true },
            Cue::Felled { tall: false },
            Cue::Hammer,
            Cue::Built,
            Cue::Placed,
            Cue::TakenApart,
            Cue::Made,
            Cue::Snort,
            Cue::Hiss,
            Cue::Chatter,
            Cue::Chirp,
            Cue::Quack,
            Cue::Croak,
            Cue::Bark,
            Cue::Wings,
            Cue::Bell,
            Cue::Click,
            Cue::Open,
            Cue::Close,
        ];
        cues.extend(stations.all().map(Cue::Working));
        cues.extend(every_struck().into_iter().map(Cue::Struck));
        let place = |cue: Cue| match cue {
            Cue::Step(Feet::Grass) => 0,
            Cue::Step(Feet::Wood) => 1,
            Cue::Step(Feet::Water) => 2,
            Cue::Step(Feet::Hoof) => 3,
            Cue::Step(Feet::Skitter) => 4,
            Cue::Swing { .. } => 5,
            Cue::Hit => 6,
            Cue::Kill => 7,
            Cue::Hurt => 8,
            Cue::Down => 9,
            Cue::Strike(Cut::Tree) => 10,
            Cue::Strike(Cut::GreatTree) => 11,
            Cue::Strike(Cut::Rock) => 12,
            Cue::Strike(Cut::Ore) => 13,
            Cue::Strike(Cut::Plant) => 14,
            Cue::Strike(Cut::Flesh) => 15,
            Cue::Felled { tall: true } => 16,
            Cue::Felled { tall: false } => 17,
            Cue::Hammer => 18,
            Cue::Built => 19,
            Cue::Placed => 20,
            Cue::TakenApart => 21,
            Cue::Working(_) => 22,
            Cue::Made => 23,
            Cue::Snort => 24,
            Cue::Hiss => 25,
            Cue::Bell => 26,
            Cue::Click => 27,
            Cue::Open => 28,
            Cue::Close => 29,
            Cue::Chatter => 30,
            Cue::Chirp => 31,
            Cue::Quack => 32,
            Cue::Croak => 33,
            Cue::Bark => 34,
            Cue::Wings => 35,
            Cue::Struck(_) => 36,
        };
        let mut listed = [false; 37];
        for &cue in &cues {
            listed[place(cue)] = true;
        }
        assert!(
            listed.iter().all(|&listed| listed),
            "a kind of cue is missing from the list"
        );
        cues
    }

    /// Every blow a fight can land: each way of taking it but a dodge, between every pair of
    /// surfaces, light and heavy.
    fn every_struck() -> Vec<Struck> {
        use Surface::{Flesh, Iron, Steel, Wood};
        let taken = [
            Taken::Hit,
            Taken::Blocked,
            Taken::Parried,
            Taken::GuardBroken,
        ];
        let mut all = Vec::new();
        for taken in taken {
            for by in [Steel, Iron, Wood, Flesh] {
                for on in [Steel, Iron, Wood, Flesh] {
                    for heavy in [false, true] {
                        all.push(Struck {
                            taken,
                            by,
                            on,
                            heavy,
                        });
                    }
                }
            }
        }
        all
    }

    /// The sword and shield's blows, as the move set's kit meets: a block knocks on the
    /// shield's wooden face, a parry rings on its iron boss, a guard broken cracks, a blade on
    /// flesh is heard heavier for a heavy blow, and a swing by its weight. Each is its own
    /// sound, so the ear tells them apart.
    #[test]
    fn a_fight_is_heard_by_what_is_struck_and_how_heavy_the_blow() {
        let contacts = Contacts::read();
        let heard = |taken, on, heavy| {
            let struck = Struck {
                taken,
                by: Surface::Steel,
                on,
                heavy,
            };
            contacts.sound(struck).unwrap()
        };
        let block = heard(Taken::Blocked, Surface::Wood, false);
        let parry = heard(Taken::Parried, Surface::Iron, false);
        let broken = heard(Taken::GuardBroken, Surface::Wood, true);
        let hit = heard(Taken::Hit, Surface::Flesh, false);
        let heavy_hit = heard(Taken::Hit, Surface::Flesh, true);
        assert_eq!(
            [block, parry, broken, hit, heavy_hit],
            [
                "block.wood",
                "parry.steel",
                "guard_break.wood",
                "hit.flesh",
                "hit.flesh.heavy"
            ]
        );
        // A heavy parry rings as a light one; a guard of metal clanks rather than rings, so
        // only a parry sounds like one.
        assert_eq!(heard(Taken::Parried, Surface::Iron, true), parry);
        assert_ne!(heard(Taken::Blocked, Surface::Steel, false), parry);
        let swing = |heavy| match sounds_like(Cue::Swing { heavy }, &Stations::read(), &contacts) {
            Like::Clips(name) => name,
            _ => panic!("a swing is not heard from clips"),
        };
        assert_ne!(swing(false), swing(true));
        // Every blow that is not dodged is heard, by the table's last row if no other.
        for struck in every_struck() {
            assert!(
                contacts.sound(struck).is_some(),
                "{struck:?} is heard as nothing"
            );
        }
    }

    #[test]
    fn every_cue_is_heard_and_its_clips_exist() {
        let stations = Stations::read();
        let contacts = Contacts::read();
        for cue in every_cue(&stations) {
            match sounds_like(cue, &stations, &contacts) {
                Like::Clips(name) => assert!(
                    CLIPS.iter().any(|clips| clips.name == name),
                    "{cue:?}: no clips called {name}"
                ),
                Like::Made(..) => {}
                // Only work at a station that is heard in no strokes, such as a fire, is
                // silent here.
                Like::Nothing => assert!(
                    matches!(cue, Cue::Working(station) if stations.strokes(station).is_none()),
                    "{cue:?} is heard as nothing"
                ),
            }
        }
    }
}

/// Every sound in its place in the mix (docs/audio.md), measured from the clips and the
/// sounds made in code, at the volumes the options menu starts with.
#[cfg(test)]
mod loudness {
    use super::*;
    use crate::audio::synth::tests::{active_db, at_ear, render, within};
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
        let path = terra_world::data::assets().join(format!("sounds/{file}.ogg"));
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
        let stations = Stations::read();
        let made: Vec<_> = super::tests::every_cue(&stations)
            .into_iter()
            .filter_map(|cue| match sounds_like(cue, &stations, &Contacts::read()) {
                Like::Made(name, sound, level, _) => Some((name, sound, level)),
                _ => None,
            })
            .collect();
        assert!(!made.is_empty());
        for (name, sound, level) in made {
            let (samples, _) = render(sound, 3, 2.0);
            let heard = at_ear(active_db(&samples), level, Bus::Effects);
            within(name, heard, band(name, Bus::Effects));
        }
    }
}
