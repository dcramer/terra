//! The creator by the remote protocol (D69, tools/sandbox), so a scripted run can change
//! this machine's player's features and read, with no picture, what its body then shows.
//!
//! - `creator.set {"body": "female", "hair": "male_hair_dreads", "beard": null, "hair_colour": "Black",
//!   "skin": "Deep", "eyes": "Blue"}`: any of them; one left out stays. A name not on offer
//!   is refused with what is.
//! - `creator.state`: the player's features, what is on offer, the files of the pieces its
//!   body shows, and for each painted role (skin, hair, eyes) what its parts are drawn with:
//!   the colour their map is multiplied by, the map, and how many parts.

use super::{BEARD, Features, HAIR, creator};
use crate::IsLocalPlayer;
use crate::character::{Body, Worn};
use crate::wardrobe::wardrobe;
use bevy::gltf::GltfMaterialName;
use bevy::prelude::*;
use bevy::remote::{BrpResult, builtin_methods::parse_some};
use bevy::world_serialization::WorldAssetRoot;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use terra_world::remote::{method, refused};

pub fn plugin(app: &mut App) {
    method(app, "creator.set", set);
    method(app, "creator.state", state);
}

/// A field left out is not changed; `null` for the hair or beard is none.
#[derive(Deserialize, Default)]
struct Ask {
    body: Option<String>,
    #[serde(default, deserialize_with = "given")]
    hair: Option<Option<String>>,
    #[serde(default, deserialize_with = "given")]
    beard: Option<Option<String>>,
    hair_colour: Option<String>,
    skin: Option<String>,
    eyes: Option<String>,
}

fn given<'de, D: serde::Deserializer<'de>>(from: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(from).map(Some)
}

/// The body this machine's player stands on: its entity, its name in the wardrobe.
fn body(world: &mut World) -> Option<(Entity, Entity, &'static str)> {
    let mut players = world.query_filtered::<Entity, IsLocalPlayer>();
    let player = players.iter(world).next()?;
    let mut bodies = world.query_filtered::<(Entity, &ChildOf, &WorldAssetRoot), With<Body>>();
    let (root, _, model) = bodies
        .iter(world)
        .find(|(_, parent, _)| parent.parent() == player)?;
    let path = model.0.path()?.to_string();
    let (name, _) = wardrobe().body_for_model(&path)?;
    let name = wardrobe().bodies.get_key_value(name)?.0.as_str();
    Some((player, root, name))
}

const NO_BODY: &str = "This machine's player has no wardrobe body yet: wait for the world to load (the man stands on one; the woman does not yet).";

fn set(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let ask: Ask = parse_some(params)?;
    let Some((player, _, name)) = body(world) else {
        return Err(refused(NO_BODY));
    };
    let creator = creator();
    let mut features = world
        .get::<Features>(player)
        .cloned()
        .unwrap_or_else(|| creator.first(name));
    let bodies: Vec<&str> = creator.roles.keys().map(String::as_str).collect();
    if let Some(asked) = ask.body.filter(|asked| *asked != features.body) {
        if !bodies.contains(&asked.as_str()) {
            return Err(refused(format!(
                "No body is named {asked:?}: choose one of {}.",
                bodies.join(", ")
            )));
        }
        // Another body: its own first hair, the colours kept (`Features::turn`).
        while features.body != asked {
            features.turn(super::Feature::Body, true);
        }
    }
    let name = wardrobe()
        .bodies
        .get_key_value(features.body.as_str())
        .map_or(name, |(name, _)| name.as_str());
    let offered = |what: &str, asked: &str, names: Vec<&str>| {
        names.contains(&asked).then_some(()).ok_or_else(|| {
            refused(format!(
                "No {what} is named {asked:?}: choose one of {}.",
                names.join(", ")
            ))
        })
    };
    for (slot, asked, to) in [
        (HAIR, ask.hair, &mut features.hair),
        (BEARD, ask.beard, &mut features.beard),
    ] {
        let Some(asked) = asked else { continue };
        if let Some(piece) = &asked {
            offered(slot, piece, creator.pieces(name, slot))?;
        }
        *to = asked;
    }
    let tones = |tones: &'static [super::Tone]| tones.iter().map(|t| t.name.as_str()).collect();
    if let Some(asked) = ask.hair_colour {
        offered("hair colour", &asked, tones(&creator.hair))?;
        features.hair_colour = asked;
    }
    if let Some(asked) = ask.skin {
        offered("skin tone", &asked, tones(&creator.skin))?;
        features.skin = asked;
    }
    if let Some(asked) = ask.eyes {
        let names = creator.eyes.iter().map(|e| e.name.as_str()).collect();
        offered("eye colour", &asked, names)?;
        features.eyes = asked;
    }
    let told = json!(features);
    world.entity_mut(player).insert(features);
    Ok(told)
}

fn state(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Some((player, root, name)) = body(world) else {
        return Err(refused(NO_BODY));
    };
    let creator = creator();
    // The looks on the body, and under each the parts and what they are drawn with.
    let mut looks = world.query::<(Entity, &Worn, Option<&WorldAssetRoot>)>();
    let looks: Vec<(Entity, Option<String>)> = looks
        .iter(world)
        .filter(|(_, worn, _)| worn.body == root)
        .map(|(look, _, file)| {
            let file = file.and_then(|file| file.0.path()).map(|p| p.to_string());
            (look, file)
        })
        .collect();
    let roles = creator.roles.get(name);
    let role_of = |material: &str| {
        let roles = roles?;
        let named = material.split('.').next().unwrap_or_default();
        [
            ("skin", &roles.skin),
            ("hair", &roles.hair),
            ("eyes", &roles.eyes),
        ]
        .into_iter()
        .find(|(_, role)| *role == named)
        .map(|(what, _)| what)
    };
    let mut drawn: BTreeMap<&str, BTreeMap<String, usize>> = BTreeMap::new();
    let mut parts = world.query::<(&MeshMaterial3d<StandardMaterial>, &GltfMaterialName)>();
    let mut below = world.query::<&Children>();
    for (look, _) in &looks {
        let under: Vec<Entity> = below.query(world).iter_descendants(*look).collect();
        for part in under {
            let Ok((material, named)) = parts.get(world, part) else {
                continue;
            };
            let (Some(role), Some(material)) = (
                role_of(&named.0),
                world
                    .resource::<Assets<StandardMaterial>>()
                    .get(&material.0),
            ) else {
                continue;
            };
            let colour = material.base_color.to_linear();
            let map = (material.base_color_texture.as_ref())
                .and_then(|map| map.path())
                .map(|path| path.to_string());
            let with = json!({
                "colour": [colour.red, colour.green, colour.blue],
                "map": map,
            });
            *drawn
                .entry(role)
                .or_default()
                .entry(with.to_string())
                .or_default() += 1;
        }
    }
    let drawn: BTreeMap<&str, Vec<Value>> = drawn
        .into_iter()
        .map(|(role, with)| {
            let with = with.into_iter().map(|(with, parts)| {
                let mut with: Value = serde_json::from_str(&with).unwrap_or_default();
                with["parts"] = json!(parts);
                with
            });
            (role, with.collect())
        })
        .collect();
    let mut pieces: Vec<String> = looks.into_iter().filter_map(|(_, file)| file).collect();
    pieces.sort();
    Ok(json!({
        "body": name,
        "features": world.get::<Features>(player),
        "offered": {
            "hair": creator.pieces(name, HAIR),
            "beard": creator.pieces(name, BEARD),
            "hair_colour": creator.hair.iter().map(|t| &t.name).collect::<Vec<_>>(),
            "skin": creator.skin.iter().map(|t| &t.name).collect::<Vec<_>>(),
            "eyes": creator.eyes.iter().map(|e| &e.name).collect::<Vec<_>>(),
        },
        "pieces": pieces,
        "drawn": drawn,
    }))
}
