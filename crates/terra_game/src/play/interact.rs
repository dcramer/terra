//! What the interact key would act on: the thing under the aiming mark, if it is within
//! reach. You use what you look at, not whatever happens to be nearest. One system
//! decides what that is each frame, and each feature acts only on its own kind of thing,
//! so two never answer the same key.

use super::Player;
use super::controls::{Action, Settings};
use super::crafting::Station;
use super::options::OptionsMenu;
use crate::planet::MainCamera;
use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

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
}

impl Interactable {
    /// Where to aim at it, as a height above its foot, and how near the aiming mark has
    /// to pass to count as on it, both in metres.
    fn target(self) -> (f32, f32) {
        match self {
            Interactable::Station(Station::Forge) => (1.0, 1.3),
            Interactable::Station(Station::Smelter) => (1.0, 1.1),
            Interactable::Station(_) => (0.8, 0.9),
            Interactable::ClaimStone => (0.6, 0.8),
            Interactable::Bag => (0.2, 0.6),
            Interactable::Carcass => (0.4, 0.9),
            Interactable::Prisoner => (0.6, 0.7),
            Interactable::Talk => (1.0, 0.6),
        }
    }

    /// What pressing the key does, for the prompt.
    pub fn verb(self) -> String {
        match self {
            Interactable::Station(station) => format!("use the {}", station.name().to_lowercase()),
            Interactable::ClaimStone => "the claim stone".to_string(),
            Interactable::Bag => "take back your things".to_string(),
            Interactable::Carcass => "butcher it".to_string(),
            Interactable::Prisoner => "free them".to_string(),
            Interactable::Talk => "talk".to_string(),
        }
    }
}

/// Marks something the interact key can be used on.
#[derive(Component, Clone, Copy)]
pub struct CanInteract(pub Interactable);

/// The nearest thing within reach, and whether the key went down for it this frame.
#[derive(Resource, Default)]
pub struct Focus {
    pub entity: Option<Entity>,
    pub what: Option<Interactable>,
    pub pressed: bool,
}

impl Focus {
    /// Whether the key went down on this kind of thing.
    pub fn used(&self, what: Interactable) -> bool {
        self.pressed && self.what == Some(what)
    }
}

/// Runs before anything that reads the focus.
#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Focusing;

pub fn plugin(app: &mut App) {
    app.init_resource::<Focus>()
        .add_systems(Update, focus.in_set(Focusing));
}

#[allow(clippy::too_many_arguments)]
fn focus(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    settings: Res<Settings>,
    menu: Res<OptionsMenu>,
    cursor: Single<&CursorOptions>,
    camera: Single<&GlobalTransform, With<MainCamera>>,
    player: Single<&Transform, With<Player>>,
    things: Query<(Entity, &CanInteract, &GlobalTransform)>,
    mut focus: ResMut<Focus>,
) {
    let feet = player.translation;
    let eye = camera.translation();
    let looking = *camera.forward();
    // The thing within reach that the aiming mark passes closest to. It may pass a little
    // wide, since the camera looks over a shoulder and so sees things close by from the
    // side; it is still aiming, not whatever happens to be nearest.
    let aimed = things
        .iter()
        .filter_map(|(entity, can, at)| {
            let (height, radius) = can.0.target();
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
    focus.entity = aimed.map(|(entity, ..)| entity);
    focus.what = aimed.map(|(_, what, _)| what);
    let captured = cursor.grab_mode != CursorGrabMode::None;
    focus.pressed = !menu.open && settings.went_down(Action::Interact, &keys, &mouse, captured);
}
