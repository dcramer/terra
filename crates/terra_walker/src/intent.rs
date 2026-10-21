//! What a player means to do (D51): the actions they hold and press, the wheel, where they
//! aim, and whether their hands are on a screen rather than the world. Gameplay reads a
//! player's `Intent`, never the keyboard, so it acts the same for every player: this
//! machine writes its own player's from its keys and mouse (`read`), and a server will
//! write everyone else's from what their games send. As leafwing-input-manager's
//! `ActionState` is, it is a component of the player it belongs to.
//!
//! Where a player looks is their `View`, which this machine's mouse turns (`look`).

use crate::controls::{Action, Controls};
use crate::options::OptionsMenu;
use crate::walker::Hands;
use crate::{IsLocalPlayer, Player};
use bevy::{input::mouse::AccumulatedMouseScroll, prelude::*};
use terra_world::planet::MainCamera;
use terra_world::tick::Sim;

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

// Every action has a bit of its own.
const _: () = assert!(Action::ALL.len() <= 64);

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

    /// Holds an action down for a script, as a key held all along is: held, not pressed.
    pub fn keep_held(&mut self, action: Action) {
        self.held |= Self::bit(action);
    }

    /// Holds an action down, and presses it if it was not held, as a key does: for tests,
    /// here and above (the `testing` feature).
    #[cfg(any(test, feature = "testing"))]
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
    menu: Res<OptionsMenu>,
    hands: Res<Hands>,
    camera: Single<&GlobalTransform, With<MainCamera>>,
    mut intent: Single<&mut Intent, IsLocalPlayer>,
) {
    let mut now = Intent {
        busy: hands.busy || menu.open,
        choosing: hands.choosing,
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
    use crate::LocalPlayer;
    use crate::controls::{Binding, Settings};
    use bevy::ecs::system::RunSystemOnce;
    use bevy::window::CursorOptions;

    /// A world with this machine's player, its keys, mouse and camera, and the screens
    /// `read` looks at, all closed.
    fn world() -> (World, Entity) {
        let mut world = World::new();
        world.init_resource::<Settings>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<ButtonInput<MouseButton>>();
        world.init_resource::<AccumulatedMouseScroll>();
        world.init_resource::<OptionsMenu>();
        world.init_resource::<Hands>();
        world.spawn(CursorOptions::default());
        world.spawn((MainCamera, GlobalTransform::default()));
        let player = world.spawn((Player, LocalPlayer, Intent::default())).id();
        (world, player)
    }

    /// An action bound to a key, and the key.
    fn keyed() -> (Action, KeyCode) {
        let settings = Settings::default();
        Action::ALL
            .into_iter()
            .find_map(|action| {
                settings
                    .bindings(action)
                    .into_iter()
                    .flatten()
                    .find_map(|binding| match binding {
                        Binding::Key(key) => Some((action, key)),
                        Binding::Mouse(_) => None,
                    })
            })
            .unwrap()
    }

    /// One frame: the keys as they are, read into the intent, and then whether a tick ran.
    fn frame(world: &mut World, player: Entity, tick: bool) -> Intent {
        world.run_system_once(read).unwrap();
        let seen = world.get::<Intent>(player).unwrap().clone();
        if tick {
            world.run_system_once(spend).unwrap();
        }
        // The keys' presses last the frame they happened in.
        world.resource_mut::<ButtonInput<KeyCode>>().clear();
        world.resource_mut::<AccumulatedMouseScroll>().delta = Vec2::ZERO;
        seen
    }

    #[test]
    fn a_press_lasts_until_a_tick_has_seen_it() {
        let (mut world, player) = world();
        let (action, key) = keyed();
        world.resource_mut::<ButtonInput<KeyCode>>().press(key);
        // Pressed on a frame with no tick, it is still pressed on the next frame's tick.
        let first = frame(&mut world, player, false);
        assert!(first.pressed(action) && first.held(action));
        let ticked = frame(&mut world, player, true);
        assert!(ticked.pressed(action) && ticked.held(action));
        // Once a tick has seen it, it is spent, though the key is still held.
        let after = frame(&mut world, player, true);
        assert!(!after.pressed(action) && after.held(action));
        world.resource_mut::<ButtonInput<KeyCode>>().release(key);
        assert!(!frame(&mut world, player, true).held(action));
    }

    #[test]
    fn the_wheel_counts_notches_until_a_tick_spends_them() {
        let (mut world, player) = world();
        let turn = |world: &mut World, by: f32, tick: bool| {
            world.resource_mut::<AccumulatedMouseScroll>().delta.y = by;
            frame(world, player, tick).wheel
        };
        // However far it reports turning, a frame's turn is one notch.
        assert_eq!(turn(&mut world, 3.0, false), 1);
        assert_eq!(turn(&mut world, 0.5, true), 2);
        assert_eq!(turn(&mut world, 0.0, true), 0);
        assert_eq!(turn(&mut world, -2.0, true), -1);
    }

    #[test]
    fn hands_on_a_screen_start_nothing_in_the_world() {
        let (mut world, player) = world();
        let (action, key) = keyed();
        world.resource_mut::<Hands>().busy = true;
        world.resource_mut::<ButtonInput<KeyCode>>().press(key);
        let intent = frame(&mut world, player, true);
        assert!(intent.pressed(action), "the press is still seen");
        assert!(!intent.starts(action) && !intent.working(action));
    }
}
