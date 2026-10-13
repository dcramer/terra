//! What the interact key would act on: the thing under the aiming mark, if it is within
//! reach. You use what you look at, not whatever happens to be nearest. One system
//! decides what that is each frame for every player (`Focus`) and sends their use of it
//! (`Interacted`); each feature acts only on its own kind of thing, so two never answer
//! the same key.

use super::Player;
use super::controls::Action;
use super::crafting::{Station, Stations};
use super::intent::Intent;
use super::tick::Sim;
use bevy::prelude::*;

/// How far from you something can be and still be used.
pub const REACH_M: f32 = 3.5;
/// How far the aiming mark may pass outside a thing's target and still be on it.
const AIM_SLACK_M: f32 = 0.6;

/// Something the interact key does something to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Interactable {
    Station(Station),
    ClaimStone,
    /// What you carried, where you went down.
    Bag,
    /// A dead animal, to butcher.
    Carcass,
    /// Someone held captive, to free.
    Prisoner,
    /// Someone to talk to.
    Talk,
    /// A door fitted in a wall, to open or shut.
    Door,
}

impl Interactable {
    /// Where to aim at it, as a height above its foot, and how near the aiming mark has
    /// to pass to count as on it, both in metres.
    fn target(self, stations: &Stations) -> (f32, f32) {
        match self {
            Interactable::Station(station) => stations.get(station).aim,
            Interactable::ClaimStone => (0.6, 0.8),
            Interactable::Bag => (0.2, 0.6),
            Interactable::Carcass => (0.4, 0.9),
            Interactable::Prisoner => (0.6, 0.7),
            Interactable::Talk => (1.0, 0.6),
            Interactable::Door => (1.1, 0.7),
        }
    }

    /// What pressing the key does, for the prompt.
    pub fn verb(self, stations: &Stations) -> String {
        match self {
            Interactable::Station(station) => {
                format!("use the {}", stations.name(station).to_lowercase())
            }
            Interactable::ClaimStone => "the claim stone".to_string(),
            Interactable::Bag => "pick up what lies here".to_string(),
            Interactable::Carcass => "butcher it".to_string(),
            Interactable::Prisoner => "free them".to_string(),
            Interactable::Talk => "talk".to_string(),
            Interactable::Door => "open or shut the door".to_string(),
        }
    }
}

/// Marks something the interact key can be used on.
#[derive(Component, Clone, Copy)]
pub struct CanInteract(pub Interactable);

/// What a player's aiming mark is on, within their reach, if anything they can use.
#[derive(Component, Default)]
pub struct Focus {
    pub entity: Option<Entity>,
    pub what: Option<Interactable>,
}

/// A player used the interact key on something: each feature answers its own kind of
/// thing, for whoever used it.
#[derive(Message, Clone, Copy, Debug)]
pub struct Interacted {
    pub player: Entity,
    pub target: Entity,
    pub what: Interactable,
}

impl Interacted {
    /// The thing, if this was a use of that kind of thing.
    pub fn on(&self, what: Interactable) -> Option<Entity> {
        (self.what == what).then_some(self.target)
    }
}

/// Runs before anything that reads the focus.
#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Focusing;

pub fn plugin(app: &mut App) {
    app.register_required_components::<Player, Focus>()
        .add_message::<Interacted>()
        .configure_sets(FixedUpdate, Focusing.in_set(Sim::Act))
        .add_systems(FixedUpdate, focus.in_set(Focusing));
}

/// Finds what each player aims at within reach, and sends their use of it.
fn focus(
    mut players: Query<(Entity, &Transform, &Intent, &mut Focus), With<Player>>,
    things: Query<(Entity, &CanInteract, &GlobalTransform)>,
    stations: Res<Stations>,
    mut interacted: MessageWriter<Interacted>,
) {
    for (player, at, intent, mut focus) in &mut players {
        let feet = at.translation;
        let Some(aim) = intent.aim else {
            continue;
        };
        let (eye, looking) = (aim.origin, *aim.direction);
        // The thing within reach that the aiming mark passes closest to. It may pass a
        // little wide, since the camera looks over a shoulder and so sees things close by
        // from the side; it is still aiming, not whatever happens to be nearest.
        let aimed = things
            .iter()
            .filter_map(|(entity, can, at)| {
                let (height, radius) = can.0.target(&stations);
                let middle = at.translation() + at.translation().normalize() * height;
                let along = (middle - eye).dot(looking);
                let off = (eye + looking * along).distance(middle);
                let near = middle.distance(feet) - radius < REACH_M;
                (along > 0.0 && off < radius + AIM_SLACK_M && near).then_some((
                    entity,
                    can.0,
                    off - radius,
                ))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2));
        let (entity, what) = (
            aimed.map(|(entity, ..)| entity),
            aimed.map(|(_, what, _)| what),
        );
        if focus.entity != entity || focus.what != what {
            focus.entity = entity;
            focus.what = what;
        }
        if let (true, Some((target, what, _))) = (intent.pressed(Action::Interact), aimed) {
            interacted.write(Interacted {
                player,
                target,
                what,
            });
        }
    }
}
