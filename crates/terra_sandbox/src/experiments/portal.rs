//! The portal (D467's way, [docs/instances.md](../../../../docs/instances.md) "The first
//! slice"): the forest walk with a portal a few steps ahead of the player, leading to a
//! placeholder realm, a playground for world loading, and a portal back there beside its
//! arrival. N steps through the nearest portal, for scripted runs; F, aimed at it, as in play.
//!
//! The realm is made only of what the generator already has: another seed, from the forest's
//! and the portal's place; the sun and the sky tinted (`RealmLight`) and the sun lower; the
//! ground's, the grass's and the plants' colours turned round the hue circle (the plants'
//! in their shader, `Tweaks::foliage_hue`, not on the camera's grade, so the player keeps
//! theirs). It gets no town. What an
//! alien realm looks like is Open, the developers' to answer: this answers none of it.
//!
//!   cargo xtask sandbox --experiment portal

use bevy::color::Hue;
use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;
use std::collections::HashMap;
use terra_game::play::portal::{self, Portal, Returns, StepThrough};
use terra_walker::LocalPlayer;
use terra_world::daylight::{Clock, RealmLight};
use terra_world::instance::{Building, InWorld, InstanceKey, WorldSpec};
use terra_world::planet::{Planet, Tweaks};

/// The forest walk's, and the walk itself.
pub const WITH: &[&str] = &[
    "--experiment",
    "play",
    "--experiment",
    "lush",
    "--experiment",
    "forest-walk",
    "--level",
    "yard",
    "--no-lair",
    "--at",
    "-154,-104.4",
    "--face",
    "-128.6,-88.4",
    "--date",
    super::forest_walk::SUMMER,
    "--time",
    "12",
];

/// Steps through the nearest portal, wherever the player stands.
pub const CROSS: KeyCode = KeyCode::KeyN;

/// The placeholder realm's kind.
pub const PLAYGROUND: &str = "playground";

/// How far ahead of the player's arrival a world's portal stands, the first time it is
/// entered.
const AHEAD_M: f32 = 6.0;

/// Where each world's portal stands, and the way its opening faces: found on first entering
/// it, so a world entered again has it where it was (the instance's record, D471).
#[derive(Resource, Default)]
struct Placed(HashMap<InstanceKey, (Vec3, Vec3)>);

pub fn plugin(app: &mut App) {
    app.init_resource::<Placed>()
        .add_systems(OnEnter(InWorld), realm_light.in_set(Building::Stage))
        .add_systems(OnEnter(InWorld), stand_portal.in_set(Building::View))
        .add_systems(
            Update,
            (
                realm_colours.run_if(resource_added::<Tweaks>),
                nearest.run_if(input_just_pressed(CROSS)),
            ),
        );
}

/// The realm a portal at `at` in `from` leads to: its seed from the world's and the place
/// (`terra_gen::hash`), so another portal leads to another realm.
pub fn playground(from: &WorldSpec, at: Vec3) -> WorldSpec {
    let seed = terra_gen::hash::of(&[
        from.seed as u32,
        (from.seed >> 32) as u32,
        at.x as i32 as u32,
        at.y as i32 as u32,
        at.z as i32 as u32,
    ]);
    WorldSpec {
        key: InstanceKey((1 << 32) | u64::from(seed)),
        kind: PLAYGROUND.into(),
        seed: u64::from(seed),
    }
}

fn in_playground(spec: &WorldSpec) -> bool {
    spec.kind == PLAYGROUND
}

/// How far round the hue circle the realm's plants are turned, in radians
/// (`Tweaks::foliage_hue`): the leaves and the floor's cards the lush forest draws from its
/// own textures, which the land's colours do not reach, to violet. On the plants, not the
/// camera's grade, which turned the player green with them.
const PLANTS_HUE_TURN: f32 = 2.6;

/// The realm's light: a mint sun under an amber sky, late in the afternoon.
fn realm_light(mut commands: Commands, spec: Res<WorldSpec>, mut clock: ResMut<Clock>) {
    if in_playground(&spec) {
        commands.insert_resource(RealmLight {
            sun: Vec3::new(0.72, 1.0, 0.86),
            sky: Vec3::new(1.3, 0.82, 0.62),
        });
        clock.hour = 16.5;
    }
}

/// How far round the hue circle the realm turns the land's colours: green grass to violet,
/// brown soil to slate teal.
const HUE_TURN_DEG: f32 = 165.0;

/// The ground's, the grass's and the plants' colours turned, once the world's are set (the
/// lush forest's soil included): every other world's as it was, its tweaks made afresh.
fn realm_colours(spec: Res<WorldSpec>, mut tweaks: ResMut<Tweaks>) {
    if !in_playground(&spec) {
        return;
    }
    let tweaks = &mut *tweaks;
    tweaks.foliage_hue = PLANTS_HUE_TURN;
    for colour in [
        &mut tweaks.ground_low,
        &mut tweaks.ground_high,
        &mut tweaks.ground_steep,
        &mut tweaks.grass_dry,
        &mut tweaks.grass_lush,
        &mut tweaks.sand,
    ] {
        *colour = colour.rotate_hue(HUE_TURN_DEG);
    }
}

/// Each world's portal: in the forest, to the playground; in the playground, back where the
/// player came from. Stood a few steps ahead of the first arrival, facing it.
#[allow(clippy::too_many_arguments)]
fn stand_portal(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    planet: Res<Planet>,
    spec: Res<WorldSpec>,
    returns: Res<Returns>,
    mut placed: ResMut<Placed>,
    body: Option<Single<&Transform, With<LocalPlayer>>>,
) {
    let Some(body) = body else { return };
    let (at, facing) = *placed.0.entry(spec.key).or_insert_with(|| {
        let ahead = *body.forward();
        (body.translation + ahead * AHEAD_M, -ahead)
    });
    let to = if in_playground(&spec) {
        match returns.0.last() {
            Some(back) => back.world.clone(),
            None => {
                warn!("portal: in the playground with nowhere to go back to");
                return;
            }
        }
    } else {
        playground(&spec, at)
    };
    info!(
        "portal: in {} {}, a portal to {} {} at {:.1} m",
        spec.kind,
        spec.key.0,
        to.kind,
        to.key.0,
        body.translation.distance(at)
    );
    portal::stand(
        &mut commands,
        &mut meshes,
        &mut materials,
        &planet,
        (at, facing),
        to,
    );
}

/// The N key: through the nearest portal.
fn nearest(
    portals: Query<(Entity, &GlobalTransform), With<Portal>>,
    body: Option<Single<&Transform, With<LocalPlayer>>>,
    mut through: MessageWriter<StepThrough>,
) {
    let Some(body) = body else { return };
    let near = portals.iter().min_by(|a, b| {
        let off = |at: &GlobalTransform| at.translation().distance_squared(body.translation);
        off(a.1).total_cmp(&off(b.1))
    });
    if let Some((portal, _)) = near {
        through.write(StepThrough(portal));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The realm's seed is the forest's and the place's: the same portal leads to the same
    /// realm, another place or another forest to another, and never back to the forest.
    #[test]
    fn a_portal_leads_to_a_realm_of_its_place() {
        let forest = WorldSpec {
            key: InstanceKey(0),
            kind: "yard".into(),
            seed: 1,
        };
        let at = Vec3::new(-150.0, 2010.0, -100.0);
        let realm = playground(&forest, at);
        assert_eq!(realm, playground(&forest, at));
        assert_ne!(realm.seed, playground(&forest, at + Vec3::X * 5.0).seed);
        let other = WorldSpec {
            seed: 2,
            ..forest.clone()
        };
        assert_ne!(realm.seed, playground(&other, at).seed);
        assert_ne!(realm.key, forest.key);
        assert_ne!(realm.seed, forest.seed);
    }
}
