//! A body's warmth (D153, climate.md): its temperature, drifting toward where its
//! surroundings push it, as `terra_climate::body` works it out the same on every machine
//! (D312). The surroundings are the climate where it stands (`ClimateSample`, bent by fires,
//! shelter and the canopy); clothing keeps the heat in, as much as each worn thing's
//! `warmth` says (`assets/data/items.ron`); work warms; rain and snow soak it, and soaked
//! clothes keep less.
//!
//! Far out of its band the body is in danger, and health falls (`vitals`), the first of
//! climate.md's effects, the stakes. A body says how it feels as it crosses each edge of the
//! band, as a body would feel it; there is no meter (D93).
//!
//! People feel it as players do (D169), in what their outfit shows they wear, and out of
//! their band they work slower (`crafting`, `terra_climate::body::pace`).

use super::death::effort;
use super::inventory::{Gear, Items, warmth};
use super::people::{Doing, Person};
use super::tick::Sim;
use super::told::Told;
use super::vitals::{Effort, Vitals};
use super::{Motion, Player, Stance};
use crate::character::{self, Outfit};
use crate::planet::Planet;
use bevy::prelude::*;
use terra_climate::body::{self, COMFORT_C, DANGER_C, NORMAL_C, Surroundings};
use terra_world::climate::ClimateSample;

/// What a body wears under any gear, in clo: almost nothing (`appearance`'s bare body).
const BARE_CLO: f32 = 0.1;
/// How fast what falls soaks a body: so much wetness a second for each millimetre an hour,
/// soaked through in a couple of minutes of steady rain; snow, which mostly brushes off, half
/// as fast. It dries over this long, faster by a fire.
const SOAKS: f32 = 1.0 / 120.0;
const SNOW_SOAKS: f32 = 0.5;
const DRIES_S: f32 = 240.0;
/// A fire's heat this strong, in watts a square metre, dries a body twice as fast again.
const DRYING_FIRE: f32 = 50.0;

/// How warm a body is, and how wet.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct BodyHeat {
    /// Its temperature, in degrees Celsius.
    pub temperature_c: f32,
    /// How wet it is, 0 dry to 1 soaked.
    pub wet: f32,
    /// How hard it works to keep its temperature, -1 shivering all it can to 1 sweating all
    /// it can.
    pub regulating: f32,
    #[reflect(ignore)]
    felt: Feeling,
}

impl Default for BodyHeat {
    fn default() -> Self {
        Self {
            temperature_c: NORMAL_C,
            wet: 0.0,
            regulating: 0.0,
            felt: Feeling::Fine,
        }
    }
}

impl BodyHeat {
    /// How far out of danger it is, in degrees: below nothing too cold, above too hot.
    pub fn danger(&self) -> f32 {
        body::danger(self.temperature_c)
    }
}

/// How a body feels its temperature.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
enum Feeling {
    Freezing,
    Cold,
    #[default]
    Fine,
    Hot,
    Overheating,
}

impl Feeling {
    fn at(temperature_c: f32) -> Self {
        if temperature_c < DANGER_C.0 {
            Feeling::Freezing
        } else if temperature_c < COMFORT_C.0 {
            Feeling::Cold
        } else if temperature_c > DANGER_C.1 {
            Feeling::Overheating
        } else if temperature_c > COMFORT_C.1 {
            Feeling::Hot
        } else {
            Feeling::Fine
        }
    }

    /// What a body says on coming to feel so, from feeling another way.
    fn said(self, before: Feeling) -> &'static str {
        match self {
            Feeling::Freezing => "You are freezing",
            Feeling::Cold if before == Feeling::Freezing => "You are still cold",
            Feeling::Cold => "You are getting cold",
            Feeling::Fine if before < Feeling::Fine => "You are warm again",
            Feeling::Fine => "You have cooled down",
            Feeling::Hot if before == Feeling::Overheating => "You are still hot",
            Feeling::Hot => "You are getting hot",
            Feeling::Overheating => "You are overheating",
        }
    }
}

pub fn plugin(app: &mut App) {
    app.register_type::<BodyHeat>()
        .add_systems(FixedUpdate, (feel, feel_people).in_set(Sim::World));
}

/// How warm what an outfit shows keeps its wearer, in clo, by each worn item's warmth.
fn outfit_warmth(items: &Items, outfit: &Outfit) -> f32 {
    outfit
        .0
        .iter()
        .filter_map(|worn| {
            items
                .kinds()
                .find(|&kind| character::item_id(items.name(kind)) == worn.item)
        })
        .filter_map(|kind| items.worn(kind))
        .map(|worn| worn.warmth)
        .sum()
}

/// Moves every person's warmth on by a tick, as a player's: working warms them, standing
/// about does not.
fn feel_people(
    time: Res<Time>,
    items: Res<Items>,
    mut people: Query<(&Person, &ClimateSample, &mut BodyHeat, Option<&Outfit>), Without<Player>>,
) {
    let dt = time.delta_secs().min(0.1);
    for (person, sample, mut heat, outfit) in &mut people {
        let clo = BARE_CLO + outfit.map_or(0.0, |outfit| outfit_warmth(&items, outfit));
        let mets = match person.doing {
            Doing::Working(_) | Doing::Following(_) => 3.0,
            _ => 1.0,
        };
        let next = warmed(*heat, sample, clo, mets, false, dt);
        heat.set_if_neq(BodyHeat {
            felt: Feeling::at(next.temperature_c),
            ..next
        });
    }
}

/// How hard a body works, in mets: what warms it.
fn mets(effort: Effort) -> f32 {
    match effort {
        Effort::Resting => 1.0,
        Effort::Moving | Effort::Working => 3.0,
        Effort::Sprinting => 6.0,
    }
}

/// Moves every body's warmth on by a tick, from where it stands and what it wears.
fn feel(
    time: Res<Time>,
    items: Res<Items>,
    planet: Option<Res<Planet>>,
    mut bodies: Query<
        (
            Entity,
            &ClimateSample,
            &mut BodyHeat,
            &Transform,
            Option<&Gear>,
            &Motion,
            &Stance,
            Option<&mut Vitals>,
        ),
        With<Player>,
    >,
    mut told: MessageWriter<Told>,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, sample, mut heat, transform, gear, motion, stance, vitals) in &mut bodies {
        let clo = BARE_CLO + gear.map_or(0.0, |gear| warmth(&items, &gear.0));
        let wading = planet
            .as_ref()
            .is_some_and(|planet| planet.under_sea(transform.translation));
        let next = warmed(*heat, sample, clo, mets(effort(motion, stance)), wading, dt);
        let feeling = Feeling::at(next.temperature_c);
        if feeling != heat.felt {
            told.write(Told::new(entity, feeling.said(heat.felt)));
        }
        *heat = BodyHeat {
            felt: feeling,
            ..next
        };
        if let Some(mut vitals) = vitals {
            vitals.expose(dt, heat.danger());
        }
    }
}

/// A body's warmth after `dt` seconds standing in a sample's climate, wearing so many clo,
/// working so many mets, and in the water or not.
fn warmed(
    heat: BodyHeat,
    sample: &ClimateSample,
    clo: f32,
    mets: f32,
    wading: bool,
    dt: f32,
) -> BodyHeat {
    let soaking = (sample.rain_mm_h + SNOW_SOAKS * sample.snow_mm_h) * SOAKS;
    let drying = (1.0 + sample.fire_w_m2 / DRYING_FIRE) / DRIES_S;
    let wet = if wading {
        1.0
    } else {
        (heat.wet + (soaking - drying) * dt).clamp(0.0, 1.0)
    };
    let around = Surroundings {
        air_c: sample.air_c,
        wind_m_s: sample.wind_m_s,
        sun_w_m2: sample.sun_w_m2,
        wet,
        fire_w_m2: sample.fire_w_m2,
    };
    let balance = body::balance(&around, clo, mets);
    BodyHeat {
        temperature_c: body::drift(heat.temperature_c, body::pushed_to(balance), dt),
        wet,
        regulating: body::effort(balance),
        felt: heat.felt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::inventory::{Backpack, Bar, Items, give_for_testing};
    use crate::play::{test_player, test_world};
    use bevy::ecs::system::RunSystemOnce;

    /// A sample of so cold a wind, with something else set on it.
    fn air(air_c: f32, wind_m_s: f32) -> ClimateSample {
        with(air_c, wind_m_s, |_| {})
    }

    fn with(air_c: f32, wind_m_s: f32, set: impl FnOnce(&mut ClimateSample)) -> ClimateSample {
        let mut sample = ClimateSample::default();
        sample.air_c = air_c;
        sample.wind_m_s = wind_m_s;
        set(&mut sample);
        sample
    }

    /// A body after so many seconds in a climate, a tick at a time.
    fn after(seconds: f32, sample: &ClimateSample, clo: f32, mets: f32) -> BodyHeat {
        let mut heat = BodyHeat::default();
        for _ in 0..(seconds * 10.0) as usize {
            heat = warmed(heat, sample, clo, mets, false, 0.1);
        }
        heat
    }

    #[test]
    fn a_winter_night_chills_a_bare_body_into_danger_and_a_coat_and_fire_keep_it() {
        let night = air(-6.0, 3.0);
        let bare = after(600.0, &night, BARE_CLO, 1.0);
        assert!(bare.danger() < 0.0, "{bare:?}");
        let coat = after(600.0, &night, BARE_CLO + 2.0, 1.0);
        assert!(coat.temperature_c > bare.temperature_c + 3.0);
        let by_fire = with(-6.0, 3.0, |s| s.fire_w_m2 = 250.0);
        assert_eq!(after(600.0, &by_fire, BARE_CLO + 1.0, 1.0).danger(), 0.0);
    }

    #[test]
    fn rain_soaks_and_a_soaked_body_is_colder() {
        let wet = with(4.0, 4.0, |s| s.rain_mm_h = 2.0);
        let dry = air(4.0, 4.0);
        let soaked = after(300.0, &wet, 1.0, 1.0);
        assert!(soaked.wet > 0.9);
        assert!(soaked.temperature_c < after(300.0, &dry, 1.0, 1.0).temperature_c);
        // And it dries out of the rain.
        let mut drying = soaked;
        for _ in 0..3000 {
            drying = warmed(drying, &dry, 1.0, 1.0, false, 0.1);
        }
        assert!(drying.wet < 0.1, "{}", drying.wet);
        assert_eq!(
            warmed(BodyHeat::default(), &dry, 1.0, 1.0, true, 0.1).wet,
            1.0
        );
    }

    #[test]
    fn a_persons_outfit_keeps_them_as_warm_as_its_items_say() {
        let items = Items::default();
        let coat = Outfit(vec![character::Clothing::new("leather_coat")]);
        let warm = items.worn(items.kind("Leather coat")).unwrap().warmth;
        assert_eq!(outfit_warmth(&items, &coat), warm);
        assert_eq!(outfit_warmth(&items, &Outfit(Vec::new())), 0.0);
    }

    #[test]
    fn a_mild_day_leaves_a_body_as_it_is() {
        let day = with(18.0, 2.0, |s| s.sun_w_m2 = 300.0);
        let heat = after(600.0, &day, BARE_CLO + 0.6, 3.0);
        assert_eq!(heat.temperature_c, NORMAL_C);
    }

    #[test]
    fn a_player_feels_the_cold_through_what_they_wear_and_says_so() {
        let mut world = test_world();
        world.insert_resource(Time::<()>::default());
        let ground = Vec3::Y * 2000.0;
        let night = air(-8.0, 4.0);
        let bare = test_player(&mut world, ground);
        let dressed = test_player(&mut world, ground + Vec3::X * 3.0);
        // Both from nothing on but the pack, the dressed one given a coat.
        crate::play::inventory::undress(&mut world, bare);
        crate::play::inventory::undress(&mut world, dressed);
        world.resource_scope(|world, mut items: Mut<Items>| {
            let mut player = world.entity_mut(dressed);
            let mut backpack = player.get::<Backpack>().unwrap().0.clone();
            let mut gear = player.get::<Gear>().unwrap().0.clone();
            let mut bar = player.get::<Bar>().unwrap().0.clone();
            let coat = [("leather_coat".to_string(), 1)];
            give_for_testing(&mut items, &mut backpack, &mut gear, &mut bar, &coat);
            player.insert(Gear(gear));
        });
        for player in [bare, dressed] {
            world
                .entity_mut(player)
                .insert((night, BodyHeat::default()));
        }
        for _ in 0..3000 {
            world
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(100));
            world.run_system_once(feel).unwrap();
        }
        let temperature =
            |world: &World, player| world.get::<BodyHeat>(player).unwrap().temperature_c;
        assert!(temperature(&world, dressed) > temperature(&world, bare));
        let told: Vec<String> = world
            .resource_mut::<Messages<Told>>()
            .drain()
            .filter(|said| said.player == bare)
            .map(|said| said.text)
            .collect();
        assert_eq!(told, ["You are getting cold", "You are freezing"]);
    }
}
