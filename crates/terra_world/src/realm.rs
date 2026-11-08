//! A realm, as data (D17, D642, docs/realm-two.md): what a world of a kind is beyond its
//! seed: its light and sky against the first realm's, and its land's colours, read from
//! `data/realms.ron`, so another realm is rows of data and its own parts, never other rules
//! (docs/art-style.md "What every realm shares"). A world whose kind has no row, or a row
//! that sets nothing, is the first realm's: the land's own light and colours.
//!
//! On entering a world its realm's light is put in (`daylight::RealmLight`) and, once the
//! land's colours are set, its own laid over them. `realm.state` says which realm a run is in
//! and what it set, so a scripted crossing checks a realm without a picture (D69).

use crate::daylight::RealmLight;
use crate::instance::{Building, InWorld, WorldSpec};
use crate::planet::Tweaks;
use bevy::prelude::*;
use serde::Deserialize;

type Rgb = (f32, f32, f32);

/// A realm's row.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Realm {
    pub kind: String,
    pub name: String,
    /// The kind of the realm its gate leads to.
    pub beyond: String,
    #[serde(default)]
    pub sun: Option<Rgb>,
    #[serde(default)]
    pub sky: Option<Rgb>,
    #[serde(default)]
    pub ground_low: Option<Rgb>,
    #[serde(default)]
    pub ground_high: Option<Rgb>,
    #[serde(default)]
    pub ground_steep: Option<Rgb>,
    #[serde(default)]
    pub grass_dry: Option<Rgb>,
    #[serde(default)]
    pub grass_lush: Option<Rgb>,
    #[serde(default)]
    pub sand: Option<Rgb>,
    #[serde(default)]
    pub ground_cover: Option<f32>,
    /// How far round the hue circle its plants' own colours are turned, in radians
    /// (`Tweaks::foliage_hue`): the leaves and the floor's plants, which the ground's colours
    /// do not reach.
    #[serde(default)]
    pub plants_hue: Option<f32>,
    /// The share of the first realm's trees and undergrowth that grow here, 0 to 1.
    #[serde(default)]
    pub plants: Option<f32>,
    /// What stands on its land by rule (the game's to stand, `terra_game::play::realm_parts`).
    #[serde(default)]
    pub parts: Vec<Part>,
}

/// A part of a realm: a catalogue item's model file and how many a square kilometre holds.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Part {
    pub file: String,
    pub a_km2: f32,
}

/// Every realm there is.
#[derive(Resource, Debug)]
pub struct Realms(Vec<Realm>);

impl Realms {
    pub const FILE: &str = "realms.ron";

    pub fn read() -> Self {
        Self(crate::data::read(Self::FILE))
    }

    /// The realms, read once, for what is made off the main thread (a world's land).
    pub fn shared() -> &'static Realms {
        static REALMS: std::sync::OnceLock<Realms> = std::sync::OnceLock::new();
        REALMS.get_or_init(Realms::read)
    }

    /// The realm a world of a kind is, if the data has it.
    pub fn of(&self, kind: &str) -> Option<&Realm> {
        self.0.iter().find(|realm| realm.kind == kind)
    }

    /// The kind of the realm beyond a world's gate: its own row's, or its own kind where the
    /// data says nothing of it (a test level's gate leads to another of its kind).
    pub fn beyond(&self, kind: &str) -> String {
        self.of(kind)
            .map_or_else(|| kind.to_string(), |realm| realm.beyond.clone())
    }
}

impl Realm {
    /// Its light, if it sets any.
    fn light(&self) -> Option<RealmLight> {
        (self.sun.is_some() || self.sky.is_some()).then(|| RealmLight {
            sun: self.sun.map_or(Vec3::ONE, Vec3::from),
            sky: self.sky.map_or(Vec3::ONE, Vec3::from),
        })
    }

    /// Lays its colours over the land's own.
    fn colour(&self, tweaks: &mut Tweaks) {
        let set = |to: &mut Color, from: Option<Rgb>| {
            if let Some((r, g, b)) = from {
                *to = Color::srgb(r, g, b);
            }
        };
        set(&mut tweaks.ground_low, self.ground_low);
        set(&mut tweaks.ground_high, self.ground_high);
        set(&mut tweaks.ground_steep, self.ground_steep);
        set(&mut tweaks.grass_dry, self.grass_dry);
        set(&mut tweaks.grass_lush, self.grass_lush);
        set(&mut tweaks.sand, self.sand);
        if let Some(cover) = self.ground_cover {
            tweaks.ground_cover = cover;
        }
        if let Some(turn) = self.plants_hue {
            tweaks.foliage_hue = turn;
        }
    }
}

pub fn plugin(app: &mut App) {
    app.insert_resource(Realms::read())
        .add_systems(OnEnter(InWorld), light.in_set(Building::Stage))
        .add_systems(Update, colours.run_if(resource_added::<Tweaks>));
    crate::remote::method(app, "realm.state", state);
}

/// The realm's light, on entering a world of its kind.
fn light(mut commands: Commands, realms: Res<Realms>, spec: Res<WorldSpec>) {
    if let Some(light) = realms.of(&spec.kind).and_then(Realm::light) {
        commands.insert_resource(light);
    }
}

/// The realm's colours, once the land's own are set.
fn colours(realms: Res<Realms>, spec: Option<Res<WorldSpec>>, mut tweaks: ResMut<Tweaks>) {
    if let Some(realm) = spec.and_then(|spec| realms.of(&spec.kind)) {
        realm.colour(&mut tweaks);
    }
}

/// `realm.state`: the realm this world is, by its kind, and the light and ground it shows.
fn state(
    In(_): In<Option<serde_json::Value>>,
    realms: Res<Realms>,
    spec: Option<Res<WorldSpec>>,
    light: Option<Res<RealmLight>>,
    tweaks: Option<Res<Tweaks>>,
) -> bevy::remote::BrpResult {
    let spec = spec.ok_or_else(|| crate::remote::refused("No world is made yet."))?;
    let rgb = |colour: Color| {
        let colour = colour.to_srgba();
        [colour.red, colour.green, colour.blue]
    };
    Ok(serde_json::json!({
        "kind": spec.kind,
        "name": realms.of(&spec.kind).map(|realm| realm.name.clone()),
        "beyond": realms.beyond(&spec.kind),
        "sun": light.as_ref().map(|light| light.sun.to_array()),
        "sky": light.as_ref().map(|light| light.sky.to_array()),
        "ground_low": tweaks.as_ref().map(|tweaks| rgb(tweaks.ground_low)),
        "ground_cover": tweaks.as_ref().map(|tweaks| tweaks.ground_cover),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_has_the_first_realm_and_the_industrial_one_each_the_others_beyond() {
        let realms = Realms::read();
        assert_eq!(realms.beyond("world"), "industrial");
        assert_eq!(realms.beyond("industrial"), "world");
        // The first realm's plants keep their own colours; the industrial one's are turned.
        assert_eq!(realms.of("world").unwrap().plants_hue, None);
        assert!(realms.of("industrial").unwrap().plants_hue.is_some());
        // A kind the data does not have leads to its own kind, and sets nothing.
        assert_eq!(realms.beyond("yard"), "yard");
        assert!(realms.of("yard").is_none());
        // The first realm sets no light or colour: the land's own.
        assert_eq!(realms.of("world").and_then(Realm::light), None);
        let industrial = realms
            .of("industrial")
            .expect("the industrial realm is in the data");
        let light = industrial.light().expect("it has its own light");
        // Warmer and duller than the first realm's: less blue than red in its sun.
        assert!(light.sun.z < light.sun.x, "{light:?}");
    }
}
