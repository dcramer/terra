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
use super::noise::{Cry, Cue, Feet, Noise};
use crate::audio::{
    self, Bus, Mix, Voice,
    bank::Bank,
    synth::{Sound, Synth},
};
use crate::loading::Loading;
use crate::planet::MainCamera;
use crate::shell::Options;
use bevy::prelude::*;
use serde::Deserialize;
use terra_motion::moves::Taken;
use terra_motion::struck::{Struck, Surface};

mod budget;
mod clips;
use budget::Budget;
use clips::CLIPS;

pub fn plugin(app: &mut App) {
    app.init_resource::<Heard>()
        .insert_resource(Contacts::read())
        .add_systems(Startup, load.run_if(audio::speaking))
        .add_systems(Update, (mix, hear).chain())
        .add_systems(Last, report);
    crate::remote::method(app, "sounds.heard", heard_so_far);
    crate::remote::method(app, "sounds.unheard", unheard_so_far);
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
        Cue::Step(Feet::Heavy) => Clips("step.heavy"),
        Cue::Cry(Cry::Roar) => Clips("cry.roar"),
        Cue::Cry(Cry::Growl) => Clips("cry.growl"),
        Cue::Cry(Cry::Grunt) => Clips("cry.grunt"),
        Cue::Cry(Cry::Groan) => Clips("cry.groan"),
        Cue::Cry(Cry::Whoosh) => Clips("swing.great"),
        Cue::Cry(Cry::Slam) => Clips("slam"),
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
        Cue::Wings => Clips("wings"),
        Cue::Voiced(name) => Clips(name),
        Cue::Bell => Clips("bell"),
        Cue::Click => Clips("click"),
        Cue::Open => Clips("open"),
        Cue::Close => Clips("close"),
    }
}

/// What has been heard, by name, for scripted runs to report; and the animal voices in
/// earshot that went unheard, over their budget (`budget`).
#[derive(Resource, Default)]
pub struct Heard {
    heard: Counts,
    unheard: Counts,
}

#[derive(Default)]
struct Counts(Vec<(&'static str, u32)>);

impl Heard {
    fn count(&mut self, name: &'static str) {
        self.heard.count(name);
    }
}

impl Counts {
    fn count(&mut self, name: &'static str) {
        match self.0.iter_mut().find(|(heard, _)| *heard == name) {
            Some((_, count)) => *count += 1,
            None => self.0.push((name, 1)),
        }
    }

    fn json(&self) -> serde_json::Value {
        let counts: serde_json::Map<_, _> = self
            .0
            .iter()
            .map(|&(name, count)| (name.to_string(), count.into()))
            .collect();
        counts.into()
    }
}

/// An animal's voice, a cry or a call, which its budget holds (`budget`); not its steps,
/// its wings or a moment of its clips, each heard as it happens.
fn animal_voice(cue: Cue) -> bool {
    use Cue::{Bark, Chatter, Chirp, Croak, Hiss, Quack, Snort, Voiced};
    matches!(
        cue,
        Voiced(_) | Snort | Hiss | Chatter | Chirp | Quack | Croak | Bark
    )
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
    mut budget: Local<Budget>,
    time: Res<Time>,
) {
    let ear = ear.single().ok().map(GlobalTransform::translation);
    let (mut bank, mut synths) = (bank, synths);
    // The nearest first, so of a voice over its budget the far ones go unheard.
    let off = |noise: &Noise| match (noise.at, ear) {
        (Some(at), Some(ear)) => at.distance(ear),
        _ => 0.0,
    };
    let mut noises: Vec<&Noise> = noises.read().collect();
    noises.sort_by(|a, b| off(a).total_cmp(&off(b)));
    for noise in noises {
        let (name, reach_m) = match sounds_like(noise.cue, &stations, &contacts) {
            Like::Clips(name) => (name, reach_of(name)),
            Like::Made(name, _, _, reach_m) => (name, reach_m),
            Like::Nothing => continue,
        };
        // Far enough off to be all but silent: not worth playing.
        if off(noise) > reach_m * 12.0 {
            continue;
        }
        if animal_voice(noise.cue) && !budget.lets(name, time.elapsed_secs(), off(noise) <= reach_m)
        {
            heard.unheard.count(name);
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

/// `sounds.heard`: what has been heard so far, each sound's name and how many times, so a
/// run driven over the remote checks what an action sounded of (D69).
fn heard_so_far(
    In(_): In<Option<serde_json::Value>>,
    heard: Res<Heard>,
) -> bevy::remote::BrpResult {
    Ok(heard.heard.json())
}

/// `sounds.unheard`: the animal voices in earshot not played, over their budget, by name:
/// with `sounds.heard`, how often each was asked for.
fn unheard_so_far(
    In(_): In<Option<serde_json::Value>>,
    heard: Res<Heard>,
) -> bevy::remote::BrpResult {
    Ok(heard.unheard.json())
}

/// The set of recorded clips a kind's data names as its voice, if there is one: with none
/// the kind is heard as the sound made in code.
pub(super) fn voiced(name: &str) -> Option<&'static str> {
    CLIPS
        .iter()
        .find(|clips| clips.name == name)
        .map(|clips| clips.name)
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
        .heard
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
            Cue::Step(Feet::Heavy),
            Cue::Cry(Cry::Roar),
            Cue::Cry(Cry::Growl),
            Cue::Cry(Cry::Grunt),
            Cue::Cry(Cry::Groan),
            Cue::Cry(Cry::Whoosh),
            Cue::Cry(Cry::Slam),
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
            Cue::Step(Feet::Heavy) => 37,
            Cue::Cry(Cry::Roar) => 38,
            Cue::Cry(Cry::Growl) => 39,
            Cue::Cry(Cry::Grunt) => 40,
            Cue::Cry(Cry::Groan) => 41,
            Cue::Cry(Cry::Whoosh) => 42,
            Cue::Cry(Cry::Slam) => 43,
            Cue::Voiced(_) => 44,
        };
        // Every recorded voice a kind names (`cry`, `call`), so a name with no clips fails
        // `every_cue_is_heard_and_its_clips_exist`.
        let bestiary = super::super::species::Bestiary::read(&Default::default());
        for kind in bestiary.all() {
            let about = bestiary.get(kind);
            for name in [&about.cry, &about.call].into_iter().flatten() {
                cues.push(Cue::Voiced(name.clone().leak()));
            }
        }
        let mut listed = [false; 45];
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
