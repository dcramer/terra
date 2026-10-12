//! What a player means to do (D51): the actions they hold and press, the wheel, where they
//! aim, and whether their hands are on a screen rather than the world. Gameplay reads a
//! player's `Intent`, never the keyboard, so it acts the same for every player: this
//! machine writes its own player's from its keys and mouse (`read`), and a server will
//! write everyone else's from what their games send. As leafwing-input-manager's
//! `ActionState` is, it is a component of the player it belongs to.
//!
//! Where a player looks is their `View`, which this machine's mouse turns (`look`).

use super::build_menu::BuildMenu;
use super::controls::{Action, Controls};
use super::dial::Dial;
use super::inventory::InventoryScreen;
use super::options::OptionsMenu;
use super::{IsLocalPlayer, Player};
use crate::planet::MainCamera;
use bevy::{input::mouse::AccumulatedMouseScroll, prelude::*};

/// What a player means to do this moment.
#[derive(Component, Default, Clone, Debug, PartialEq)]
pub struct Intent {
    held: u64,
    pressed: u64,
    /// Notches the wheel turned, away from the player counting up.
    pub wheel: i32,
    /// Where they aim: from the eye they see by, through the aiming mark.
    pub aim: Option<Ray3d>,
    /// Their hands are on a screen or a menu, such as the inventory or the dial, not on
    /// the world.
    pub busy: bool,
}

impl Intent {
    fn bit(action: Action) -> u64 {
        1 << action as u64
    }

    /// Whether they hold an action down.
    pub fn held(&self, action: Action) -> bool {
        self.held & Self::bit(action) != 0
    }

    /// Whether they pressed an action this moment.
    pub fn pressed(&self, action: Action) -> bool {
        self.pressed & Self::bit(action) != 0
    }

    /// Whether they hold an action down with their hands on the world.
    pub fn working(&self, action: Action) -> bool {
        !self.busy && self.held(action)
    }

    /// Whether they pressed an action with their hands on the world.
    pub fn starts(&self, action: Action) -> bool {
        !self.busy && self.pressed(action)
    }

    /// Holds an action down, and presses it if it was not held, as a key does.
    #[cfg(test)]
    pub fn hold(&mut self, action: Action) {
        if !self.held(action) {
            self.pressed |= Self::bit(action);
        }
        self.held |= Self::bit(action);
    }
}

pub fn plugin(app: &mut App) {
    app.register_required_components::<Player, Intent>();
}

/// Writes this machine's player's intent from its keys, mouse and screens.
pub fn read(
    controls: Controls,
    scroll: Res<AccumulatedMouseScroll>,
    screen: Res<InventoryScreen>,
    build_menu: Res<BuildMenu>,
    menu: Res<OptionsMenu>,
    dial: Res<Dial>,
    camera: Single<&GlobalTransform, With<MainCamera>>,
    mut intent: Single<&mut Intent, IsLocalPlayer>,
) {
    let mut now = Intent {
        busy: screen.open || build_menu.open || menu.open || dial.open,
        aim: Some(Ray3d::new(camera.translation(), camera.forward())),
        // One notch at a time, however the wheel reports it.
        wheel: (scroll.delta.y > 0.0) as i32 - (scroll.delta.y < 0.0) as i32,
        ..default()
    };
    for action in Action::ALL {
        if controls.held(action) {
            now.held |= Intent::bit(action);
        }
        if controls.went_down(action) {
            now.pressed |= Intent::bit(action);
        }
    }
    if **intent != now {
        **intent = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holding_presses_once_and_busy_hands_do_nothing_in_the_world() {
        let mut intent = Intent::default();
        intent.hold(Action::Use);
        assert!(intent.held(Action::Use) && intent.pressed(Action::Use));
        assert!(intent.working(Action::Use) && intent.starts(Action::Use));
        assert!(!intent.held(Action::Jump));
        intent.busy = true;
        assert!(!intent.working(Action::Use) && !intent.starts(Action::Use));
        // Every action has a bit of its own.
        assert!(Action::ALL.len() <= 64);
    }
}
