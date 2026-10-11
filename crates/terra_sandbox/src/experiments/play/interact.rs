//! What the player is close enough to use with the interact key: a station, the claim
//! stone, and later people. One system decides what that is each frame, and each feature
//! acts only on its own kind of thing, so two never answer the same key.

use super::Player;
use super::controls::{Action, Settings};
use super::crafting::Station;
use super::options::OptionsMenu;
use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

/// How close you have to stand to use something.
const REACH_M: f32 = 4.0;

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
    player: Single<&Transform, With<Player>>,
    things: Query<(Entity, &CanInteract, &GlobalTransform)>,
    mut focus: ResMut<Focus>,
) {
    let feet = player.translation;
    let nearest = things
        .iter()
        .map(|(entity, can, at)| (entity, can.0, at.translation().distance(feet)))
        .filter(|(.., distance)| *distance < REACH_M)
        .min_by(|a, b| a.2.total_cmp(&b.2));
    focus.entity = nearest.map(|(entity, ..)| entity);
    focus.what = nearest.map(|(_, what, _)| what);
    let captured = cursor.grab_mode != CursorGrabMode::None;
    focus.pressed = !menu.open && settings.went_down(Action::Interact, &keys, &mouse, captured);
}
