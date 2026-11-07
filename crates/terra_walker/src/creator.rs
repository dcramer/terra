//! What a player chooses of their character (D111): the hairstyle and beard that are their
//! own whatever they wear, and the colour of their hair, skin and eyes. The choices on offer
//! are data (`models/character/creator.json`, its schema in its `about`; hairstyles and
//! beards are the wardrobe's pieces of those slots, D611), so a new one is a row (D136).
//!
//! [`Features`] is a component of the character. Where it is present the wardrobe composes
//! the body with that hair and beard in place of its look's (`Wardrobe::compose_for`), and
//! each piece of the body is painted ([`paint`]): its skin and hair multiplied by a colour,
//! its eyes given a map. A character without it (the town's people) looks as its look says.

pub mod remote;

/// A player stands on the body their features name.
pub fn plugin(app: &mut App) {
    app.add_systems(Update, stand_on);
}

fn stand_on(mut players: Query<(&Features, &mut crate::figure::Sex), Changed<Features>>) {
    for (features, mut sex) in &mut players {
        if sex.0 != features.body && creator().roles.contains_key(&features.body) {
            sex.0 = features.body.clone();
        }
    }
}

use crate::character::{Coat, Paint};
use crate::wardrobe::wardrobe;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// A character's own features, each by the name the data gives it. A name the data no longer
/// has falls back to the first on offer, so an old save still makes a person.
#[derive(Component, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Features {
    /// The body they stand on: a sex the data gives roles (`figure::Sex` follows it).
    #[serde(default = "first_body")]
    pub body: String,
    /// The hairstyle, a wardrobe piece of the slot `hair`; none is a bare head.
    pub hair: Option<String>,
    /// The beard, a wardrobe piece of the slot `beard`; none is a bare chin.
    pub beard: Option<String>,
    pub hair_colour: String,
    pub skin: String,
    pub eyes: String,
}

/// The body a character with none named stands on: the man's, the first the wardrobe had.
fn first_body() -> String {
    "male".into()
}

/// A colour on offer: its name, and what the body's map is multiplied by (linear RGB).
#[derive(Clone, Debug, Deserialize)]
pub struct Tone {
    pub name: String,
    pub colour: [f32; 3],
}

/// An eye colour on offer: its name and its map, under `assets/`.
#[derive(Clone, Debug, Deserialize)]
pub struct Eyes {
    pub name: String,
    pub map: String,
}

/// Which of a body's materials, by glTF name, are its skin, hair and eyes.
#[derive(Clone, Debug, Deserialize)]
pub struct Roles {
    pub skin: String,
    pub hair: String,
    pub eyes: String,
}

/// What the creator offers.
#[derive(Debug, Default, Deserialize)]
pub struct Creator {
    pub roles: BTreeMap<String, Roles>,
    pub skin: Vec<Tone>,
    pub hair: Vec<Tone>,
    pub eyes: Vec<Eyes>,
}

/// The choices the game ships with.
pub fn creator() -> &'static Creator {
    static CREATOR: OnceLock<Creator> = OnceLock::new();
    CREATOR.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../assets/models/character/creator.json"
        ))
        .unwrap_or_else(|error| {
            bevy::log::error!("models/character/creator.json: {error}");
            Creator::default()
        })
    })
}

/// The wardrobe's slots a person's own pieces are of.
pub const HAIR: &str = "hair";
pub const BEARD: &str = "beard";

impl Creator {
    /// The pieces of an optional slot a body can have, in the catalogue's order: its
    /// hairstyles, or its beards.
    pub fn pieces(&self, body: &str, slot: &str) -> Vec<&'static str> {
        (wardrobe().pieces.iter())
            .filter(|(_, piece)| piece.body == body && piece.slot.as_deref() == Some(slot))
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// What a new character of a body has: the first of each choice, a beardless chin.
    pub fn first(&self, body: &str) -> Features {
        let first = |tones: &[Tone]| tones.first().map(|tone| tone.name.clone());
        Features {
            body: body.to_string(),
            hair: self.pieces(body, HAIR).first().map(|name| name.to_string()),
            beard: None,
            hair_colour: first(&self.hair).unwrap_or_default(),
            skin: first(&self.skin).unwrap_or_default(),
            eyes: (self.eyes.first().map(|eyes| eyes.name.clone())).unwrap_or_default(),
        }
    }

    /// The paint a body's pieces take for these features: none for a body the data gives no
    /// roles.
    pub fn paint(&self, body: &str, features: &Features) -> Paint {
        let Some(roles) = self.roles.get(body) else {
            return Paint::default();
        };
        let tone = |tones: &[Tone], name: &str| {
            let tone = tones.iter().find(|tone| tone.name == name);
            tone.or(tones.first()).map(|tone| {
                let [red, green, blue] = tone.colour;
                Coat::Colour(LinearRgba::rgb(red, green, blue))
            })
        };
        let eyes = (self.eyes.iter().find(|eyes| eyes.name == features.eyes))
            .or(self.eyes.first())
            .map(|eyes| Coat::Map(eyes.map.clone()));
        let coats = [
            (&roles.skin, tone(&self.skin, &features.skin)),
            (&roles.hair, tone(&self.hair, &features.hair_colour)),
            (&roles.eyes, eyes),
        ];
        Paint(
            coats
                .into_iter()
                .filter_map(|(role, coat)| Some((role.clone(), coat?)))
                .collect(),
        )
    }
}

/// One of the choices a creator's screen offers, in the order it shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    Body,
    Hair,
    Beard,
    HairColour,
    Skin,
    Eyes,
}

impl Feature {
    pub const ALL: [Feature; 6] = [
        Feature::Body,
        Feature::Hair,
        Feature::Beard,
        Feature::HairColour,
        Feature::Skin,
        Feature::Eyes,
    ];

    /// What a screen calls it.
    pub fn label(self) -> &'static str {
        match self {
            Feature::Body => "Body",
            Feature::Hair => "Hair",
            Feature::Beard => "Beard",
            Feature::HairColour => "Hair colour",
            Feature::Skin => "Skin",
            Feature::Eyes => "Eyes",
        }
    }
}

impl Creator {
    /// What is on offer for a feature of a body, in order, as the names features keep: for
    /// hair and a beard none comes first of the beards and last of the hairstyles.
    pub fn offered(&self, body: &str, feature: Feature) -> Vec<Option<&str>> {
        let tones = |tones: &'static [Tone]| tones.iter().map(|t| Some(t.name.as_str())).collect();
        match feature {
            Feature::Body => self.roles.keys().map(|body| Some(body.as_str())).collect(),
            Feature::Hair => (self.pieces(body, HAIR).into_iter().map(Some))
                .chain([None])
                .collect(),
            Feature::Beard => [None]
                .into_iter()
                .chain(self.pieces(body, BEARD).into_iter().map(Some))
                .collect(),
            Feature::HairColour => tones(&creator().hair),
            Feature::Skin => tones(&creator().skin),
            Feature::Eyes => (creator().eyes.iter())
                .map(|e| Some(e.name.as_str()))
                .collect(),
        }
    }
}

impl Features {
    fn of(&self, feature: Feature) -> Option<&str> {
        match feature {
            Feature::Body => Some(&self.body),
            Feature::Hair => self.hair.as_deref(),
            Feature::Beard => self.beard.as_deref(),
            Feature::HairColour => Some(&self.hair_colour),
            Feature::Skin => Some(&self.skin),
            Feature::Eyes => Some(&self.eyes),
        }
    }

    /// Turns a feature to the next on offer for their body, or the one before, round and
    /// round. One the data no longer has turns from the first. Another body has its own hair
    /// and beards, so turning the body takes that body's first; the colours stay.
    pub fn turn(&mut self, feature: Feature, forward: bool) {
        let body = self.body.clone();
        let offered = creator().offered(&body, feature);
        if offered.is_empty() {
            return;
        }
        let at = (offered.iter().position(|each| *each == self.of(feature))).unwrap_or(0);
        let step = if forward { 1 } else { offered.len() - 1 };
        let next = offered[(at + step) % offered.len()].map(str::to_string);
        match feature {
            Feature::Body => {
                let colours = self.clone();
                *self = Features {
                    hair_colour: colours.hair_colour,
                    skin: colours.skin,
                    eyes: colours.eyes,
                    ..creator().first(&next.unwrap_or_default())
                };
            }
            Feature::Hair => self.hair = next,
            Feature::Beard => self.beard = next,
            Feature::HairColour => self.hair_colour = next.unwrap_or_default(),
            Feature::Skin => self.skin = next.unwrap_or_default(),
            Feature::Eyes => self.eyes = next.unwrap_or_default(),
        }
    }

    /// A feature as a screen says it: a colour by its name, a piece by its own words
    /// (`male_hair_slick_back` is "Slick back"), none as "None".
    pub fn words(&self, feature: Feature) -> String {
        let Some(name) = self.of(feature) else {
            return "None".into();
        };
        let slot = match feature {
            Feature::Hair => HAIR,
            Feature::Beard => BEARD,
            Feature::Body => return if name == "female" { "Woman" } else { "Man" }.to_string(),
            _ => return name.to_string(),
        };
        let own = (name.split_once(&format!("_{slot}_"))).map_or(name, |(_, own)| own);
        let mut words = own.replace('_', " ");
        if let Some(first) = words.get_mut(..1) {
            first.make_ascii_uppercase();
        }
        words
    }

    /// These features as the wardrobe is asked for them: the person's own piece, or none,
    /// for each slot that is theirs.
    pub fn own(&self) -> [(&'static str, Option<&str>); 2] {
        [(HAIR, self.hair.as_deref()), (BEARD, self.beard.as_deref())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped data offers each of D111's choices, names nothing twice, and every eye
    /// map and every role it names is there.
    #[test]
    fn the_shipped_choices_are_whole() {
        let creator = creator();
        assert!(creator.skin.len() >= 4 && creator.hair.len() >= 4 && creator.eyes.len() >= 3);
        for names in [
            creator.skin.iter().map(|t| &t.name).collect::<Vec<_>>(),
            creator.hair.iter().map(|t| &t.name).collect(),
            creator.eyes.iter().map(|e| &e.name).collect(),
        ] {
            let mut unique = names.clone();
            unique.sort();
            unique.dedup();
            assert_eq!(unique.len(), names.len(), "a name twice: {names:?}");
        }
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        for eyes in &creator.eyes {
            assert!(assets.join(&eyes.map).is_file(), "{} is missing", eyes.map);
        }
        for (body, roles) in &creator.roles {
            assert!(wardrobe().bodies.contains_key(body), "no body {body}");
            // A body with hairstyles has a choice of them; the woman has none yet (D599).
            let styles = creator.pieces(body, HAIR).len();
            assert!(styles == 0 || styles >= 4, "{body}'s hairstyles: {styles}");
            assert!(body != "male" || styles >= 4);
            // The head carries each of the three materials.
            let head = &wardrobe().bodies[body].head;
            let file = assets.join(wardrobe().file(head).unwrap());
            let text = std::fs::read_to_string(file).unwrap();
            for role in [&roles.skin, &roles.hair, &roles.eyes] {
                assert!(
                    text.contains(&format!("\"{role}\"")),
                    "{head} has no {role}"
                );
            }
        }
    }

    #[test]
    fn a_persons_hair_and_beard_are_their_own_whatever_the_look() {
        let (creator, wardrobe) = (creator(), wardrobe());
        let mut features = creator.first("male");
        let styles = creator.pieces("male", HAIR);
        let beards = creator.pieces("male", BEARD);
        for (look, _) in wardrobe
            .looks
            .iter()
            .filter(|(_, look)| look.body == "male")
        {
            // Undressed, so nothing covers the head.
            for style in &styles {
                features.hair = Some(style.to_string());
                features.beard = Some(beards[0].to_string());
                let shown = wardrobe.compose_for("male", look, Some(&[]), &features.own());
                assert!(shown.contains(style), "{look}: {style} not in {shown:?}");
                assert!(shown.contains(&beards[0]), "{look}: no beard in {shown:?}");
                let other = styles.iter().filter(|s| *s != style);
                assert!(
                    !other.clone().any(|s| shown.contains(s)),
                    "{look}: two heads of hair"
                );
            }
            // None chosen is none shown, though the look has its own.
            (features.hair, features.beard) = (None, None);
            let shown = wardrobe.compose_for("male", look, Some(&[]), &features.own());
            let any = styles.iter().chain(&beards).any(|s| shown.contains(s));
            assert!(!any, "{look}: hair shown on a bare head: {shown:?}");
        }
    }

    #[test]
    fn a_feature_turns_through_all_on_offer_and_back_and_reads_in_words() {
        let creator = creator();
        for feature in Feature::ALL {
            let mut features = creator.first("male");
            let began = features.clone();
            let offered = creator.offered("male", feature);
            let mut seen = vec![features.words(feature)];
            for _ in 1..offered.len() {
                features.turn(feature, true);
                seen.push(features.words(feature));
            }
            let mut unique = seen.clone();
            unique.sort();
            unique.dedup();
            assert_eq!(unique.len(), offered.len(), "{feature:?}: {seen:?}");
            features.turn(feature, true);
            if feature == Feature::Body {
                // Round to the same body, whose hair is then its first again.
                assert_eq!(features.body, began.body, "the body comes round");
                continue;
            }
            assert_eq!(features, began, "{feature:?} comes round");
            features.turn(feature, false);
            features.turn(feature, true);
            assert_eq!(features, began, "{feature:?} back and forth");
        }
        let mut features = creator.first("male");
        features.hair = Some("male_hair_slick_back".into());
        assert_eq!(features.words(Feature::Hair), "Slick back");
        assert_eq!(features.words(Feature::Beard), "None");
        assert_eq!(features.words(Feature::Skin), creator.skin[0].name);
    }

    /// A name the data does not have paints as the first on offer, never as nothing.
    #[test]
    fn features_paint_skin_hair_and_eyes_and_an_unknown_name_is_the_first() {
        let creator = creator();
        let mut features = creator.first("male");
        features.eyes = "Blue".into();
        features.skin = "no such tone".into();
        let Paint(coats) = creator.paint("male", &features);
        let roles = &creator.roles["male"];
        let coat = |role: &str| &coats.iter().find(|(r, _)| r == role).unwrap().1;
        assert_eq!(
            coat(&roles.eyes),
            &Coat::Map("models/character/eyes/blue.png".into())
        );
        let [red, green, blue] = creator.skin[0].colour;
        assert_eq!(
            coat(&roles.skin),
            &Coat::Colour(LinearRgba::rgb(red, green, blue))
        );
        assert!(matches!(coat(&roles.hair), Coat::Colour(_)));
        assert_eq!(creator.paint("nobody", &features), Paint::default());
    }
}
