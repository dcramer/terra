//! The hero in play (`--play`) is the game's own body: `models/character/male.glb`'s skeleton
//! and clips, dressed in the wardrobe's knight armour, the arming sword and heater shield in
//! his hands, playing the sword set's moves (D628 as amended, D635: one game, the shared code
//! called, never copied). The developer, 2026-11-08: "better character, better animations on
//! the character (no auto magic)", so no Tripo preset clip plays on him.
//!
//! Three parts, each its own module, talking through two components on the hero:
//! - `abilities`: what he can do (`Ability`, named in the play help), read from the keys and
//!   mouse or from a script (`--do`, `--press`, `--click`) into his `HeroIntent`, the seam a
//!   scripted film or another system writes too (`HeroSystems::Intent`).
//! - `moves`: the sword set's rules (terra_motion::moves' `Runner`) and his motion, eased
//!   toward the speed wished or driven by the move, kept to the layout's ground; `Doing` says
//!   what he does, for the others to read (a mob's AI: `Doing::invulnerable`, `blocking`).
//! - `anim`: which clip shows it, through terra_walker's graph, stance hold, inertialization,
//!   gait rates and foot markers, as the game's figure plays them.
//!
//! Everything here is play's alone: the loop's stills and films (no `--play`) never add it,
//! and the dual-quaternion skinning terra_walker's character plugin brings with it (for every
//! skinned mesh) moves no still (`node crates/terra_iso/loop.ts shoot`: every biome "same").

use crate::play::Hero;
use crate::ruin::fight::Pose;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use terra_walker::character;

pub mod abilities;
mod anim;
mod moves;

pub use abilities::{Ability, HeroIntent, HeroSystems};
pub use moves::Doing;

/// The game's body: its skeleton, sockets and clips, no mesh of its own.
const BODY: &str = "models/character/male.glb";
/// The wardrobe's look he wears (wardrobe.json), and his hands' things.
const LOOK: &str = "knight";
const SWORD: &str = "models/held/arming_sword.glb";
const SHIELD: &str = "models/held/heater_shield.glb";

/// The game's body as the hero, before his clips are on it (`anim::rig`).
#[derive(Component)]
pub struct GameBody;

/// His clips' file and his move set.
#[derive(Resource)]
struct Kit {
    gltf: Handle<Gltf>,
    set: terra_motion::moves::MoveSet,
}

pub fn plugin(app: &mut App) {
    let set = terra_motion::moves::MoveSet::read(include_str!(
        "../../../../assets/data/moves/sword.shielded.ron"
    ))
    .unwrap_or_else(|why| panic!("assets/data/moves/sword.shielded.ron: {why}"));
    let gltf = app.world().resource::<AssetServer>().load(BODY);
    app.insert_resource(Kit { gltf, set })
        .add_plugins((
            // The game's dressing (pieces rebound onto the body's joints) and its skinning.
            character::plugin,
            terra_walker::inertia::plugin,
            abilities::plugin,
        ))
        .configure_sets(Update, (HeroSystems::Intent, HeroSystems::Act).chain())
        .add_systems(
            Update,
            (
                anim::rig,
                moves::act.in_set(HeroSystems::Act),
                anim::show.after(HeroSystems::Act),
            ),
        );
}

/// The staged Tripo knight, once play adopts him, gives way to the game's body where he
/// stood, facing his way: play's own rig for a Tripo body then has nothing to take.
pub fn swap(
    mut commands: Commands,
    knights: Query<(Entity, &Transform), (With<Hero>, With<Pose>)>,
    asset: Res<AssetServer>,
) {
    for (knight, tr) in &knights {
        commands.entity(knight).despawn();
        commands
            .spawn((
                Name::new("Hero"),
                Hero,
                character::Body,
                GameBody,
                HeroIntent::default(),
                Doing::default(),
                // The oaks before the fight open about him as about the staged cast.
                crate::foliage::Seen,
                WorldAssetRoot(asset.load(GltfAssetLabel::Scene(0).from_asset(BODY))),
                // The game's model faces +Z, as the experiment's do (play.rs `facing`).
                Transform::from_translation(tr.translation).with_rotation(tr.rotation),
            ))
            .observe(dress);
    }
}

/// Once his bones exist: the knight's pieces put on (rebound to his joints by the character
/// plugin's `put_on`), and the sword and shield in his hands' sockets.
fn dress(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    asset: Res<AssetServer>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    let body = ready.entity;
    let pieces = terra_walker::wardrobe::wardrobe().compose("male", LOOK, None);
    if pieces.is_empty() {
        warn!("the wardrobe has no {LOOK:?} look on the male body: the hero stands bare");
    }
    for piece in pieces {
        let path = format!("models/character/male/{piece}.gltf");
        character::wear(
            &mut commands,
            asset.load(GltfAssetLabel::Scene(0).from_asset(path)),
            body,
        );
    }
    for (socket, item) in [(character::HAND_R, SWORD), (character::HAND_L, SHIELD)] {
        match character::find(body, socket, &children, &names) {
            Some(hand) => {
                character::hold(
                    &mut commands,
                    asset.load(GltfAssetLabel::Scene(0).from_asset(item)),
                    hand,
                );
            }
            None => warn!("the hero's body has no {socket}: no {item} in it"),
        }
    }
}
