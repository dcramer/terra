//! The hero in play (`--play`) is the game's own body: `models/character/male.glb`'s skeleton
//! and clips, dressed in the wardrobe's Knight Cloth (a surcoat over the knight's plate), the
//! arming sword and heater shield in his hands, playing the sword set's moves (D628 as
//! amended, D635: one game, the shared code called, never copied). The developer, 2026-11-08:
//! "better character, better animations on the character (no auto magic)", so no Tripo
//! preset clip plays on him.
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
/// The wardrobe's look he wears (wardrobe.json), and his hands' things. The Knight Cloth as
/// made, its white and red surcoat over the knight's plate: the plain Knight's bare plate read
/// a dull grey-blue in the forest's shade at the iso camera, the surcoat reads as the staged
/// Tripo knight's red did; colourways 2 (gilt plate, blue surcoat) and 3 (blackened) read
/// gaudy and duller (iso-hero j4's forest trial). Its horned helmet stays: bareheaded, his
/// buzzed head read as a white bald blob from above, and the plate's armet is refused over
/// the surcoat (the wardrobe's pairings).
const LOOK: &str = "knight_cloth";
/// His metal's share taken to grey (rim.rs `Fighter::with_steel`): T_Knight_BaseColor's
/// plate is painted green-grey (#7A8B84, #909A91; its metal texels' mean chroma 6.7 at hue
/// 150, where iron's is about 1), and as a metal's base colour it tinted every reflection:
/// his plate's brightest tenth read chroma 10.9 at hue 133, mint, beside the staged Tripo
/// knight's 2.2 (forest --play, iso-hero j5).
const STEEL: f32 = 1.0;
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
    let mut looks = Vec::new();
    for piece in pieces {
        let path = format!("models/character/male/{piece}.gltf");
        looks.push(character::wear(
            &mut commands,
            asset.load(GltfAssetLabel::Scene(0).from_asset(path)),
            body,
        ));
    }
    for (socket, item) in [(character::HAND_R, SWORD), (character::HAND_L, SHIELD)] {
        match character::find(body, socket, &children, &names) {
            Some(hand) => {
                looks.push(character::hold(
                    &mut commands,
                    asset.load(GltfAssetLabel::Scene(0).from_asset(item)),
                    hand,
                ));
            }
            None => warn!("the hero's body has no {socket}: no {item} in it"),
        }
    }
    lit_once_in(&mut commands, body, &looks);
}

/// How many of the hero's looks (worn pieces, held things) are still loading.
#[derive(Component)]
struct Dressing(usize);

/// Once every one of `looks` is in, the hero becomes a fighter (rim.rs `Fighter`), lit and
/// shaded as the staged cast he replaces: his materials swapped for the rim's, so the oaks'
/// canopy dims his sky fill and its reflection as it dims the ground's, with the fighters'
/// band on his outline. As a bare `StandardMaterial` his knight's plate (metal 1 on T_Knight's
/// ORM) mirrored the open sky's blue under the crowns and read pale cyan plastic (iso-hero
/// j4's judge). Not before: the rim swaps a fighter's materials once, `FIGHTER_WAIT` frames
/// after the first is in, and a piece loading later would stay unlit by it.
fn lit_once_in(commands: &mut Commands, body: Entity, looks: &[Entity]) {
    if looks.is_empty() {
        return;
    }
    commands.entity(body).insert(Dressing(looks.len()));
    for &look in looks {
        commands.entity(look).observe(
            move |_: On<WorldInstanceReady>,
                  mut left: Query<&mut Dressing>,
                  mut commands: Commands| {
                let Ok(mut left) = left.get_mut(body) else {
                    return;
                };
                left.0 = left.0.saturating_sub(1);
                if left.0 == 0 {
                    commands
                        .entity(body)
                        .remove::<Dressing>()
                        .insert(crate::rim::Fighter::default().with_steel(STEEL));
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// His look composes whole on the game's body: a worn piece in every slot, none bare
    /// (a refused pairing would leave one bare), the surcoat on his body and a helmet on.
    #[test]
    fn he_wears_the_whole_look() {
        let w = terra_walker::wardrobe::wardrobe();
        let pieces = w.compose("male", LOOK, None);
        let items: Vec<&str> = (pieces.iter())
            .filter_map(|p| w.pieces.get(*p)?.item.as_deref())
            .collect();
        let bare = (w.bodies["male"].bare.values()).find(|b| pieces.contains(&b.as_str()));
        assert!(bare.is_none(), "{bare:?} bare in {pieces:?}");
        for want in ["surcoat", "plate_legs", "sabatons"] {
            assert!(items.contains(&want), "no {want} in {items:?}");
        }
        let helmet = items.iter().any(|i| i.ends_with("helmet"));
        assert!(helmet, "no helmet: {items:?}");
    }

    /// He becomes a fighter (lit as the staged cast) once the last of his looks is in, not
    /// while one still loads.
    #[test]
    fn he_is_lit_as_a_fighter_once_dressed() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::world_serialization::{WorldAsset, WorldSerializationPlugin};
        let mut app = App::new();
        app.add_plugins((AssetPlugin::default(), WorldSerializationPlugin));
        let body = app.world_mut().spawn_empty().id();
        let looks: Vec<Entity> = (0..2).map(|_| app.world_mut().spawn_empty().id()).collect();
        let ready = looks.clone();
        app.world_mut()
            .run_system_once(move |mut commands: Commands| lit_once_in(&mut commands, body, &ready))
            .unwrap();
        let world = |app: &mut App| {
            let asset = app.world().resource::<AssetServer>();
            WorldAssetRoot(asset.add(WorldAsset::new(World::new())))
        };
        let first = world(&mut app);
        app.world_mut().entity_mut(looks[0]).insert(first);
        app.update();
        assert!(app.world().get::<crate::rim::Fighter>(body).is_none());
        let second = world(&mut app);
        app.world_mut().entity_mut(looks[1]).insert(second);
        app.update();
        assert!(app.world().get::<crate::rim::Fighter>(body).is_some());
        assert!(app.world().get::<Dressing>(body).is_none());
    }
}
