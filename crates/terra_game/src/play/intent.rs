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
use super::tick::Sim;
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
    /// Their hands are on a screen, such as the inventory, a station's or the options,
    /// not on the world.
    pub busy: bool,
    /// They are choosing what to build, on the dial or in the build menu.
    pub choosing: bool,
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

    /// Whether their hands are on the world, not on a screen or choosing.
    pub fn free(&self) -> bool {
        !self.busy && !self.choosing
    }

    /// Whether they hold an action down with their hands on the world.
    pub fn working(&self, action: Action) -> bool {
        self.free() && self.held(action)
    }

    /// Whether they pressed an action with their hands on the world.
    pub fn starts(&self, action: Action) -> bool {
        self.free() && self.pressed(action)
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
    app.register_required_components::<Player, Intent>()
        .add_systems(FixedUpdate, spend.in_set(Sim::End));
}

/// After a tick, the presses and wheel it has seen are spent; what is held stays held.
fn spend(mut intents: Query<&mut Intent>) {
    for mut intent in &mut intents {
        if intent.pressed != 0 || intent.wheel != 0 {
            intent.pressed = 0;
            intent.wheel = 0;
        }
    }
}

/// Writes this machine's player's intent from its keys, mouse and screens, before the
/// frame's ticks. A press is kept until a tick has seen it, so none is lost on a frame
/// with no tick or seen twice on a frame with two.
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
        busy: screen.open || menu.open,
        choosing: dial.open || build_menu.open,
        aim: Some(Ray3d::new(camera.translation(), camera.forward())),
        // One notch at a time, however the wheel reports it, kept with what is unspent.
        wheel: intent.wheel + (scroll.delta.y > 0.0) as i32 - (scroll.delta.y < 0.0) as i32,
        pressed: intent.pressed,
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
        intent.busy = false;
        intent.choosing = true;
        assert!(!intent.working(Action::Use) && intent.held(Action::Use));
        // Every action has a bit of its own.
        assert!(Action::ALL.len() <= 64);
    }
}
