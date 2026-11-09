//! What makes a node on the game's screens clickable. Bevy's headless button marks a node
//! `Pressed` while the pointer holds it down, and `Hovered` says whether the pointer is on it
//! or anything inside it. (Bevy 0.20 has these in place of `Interaction`.)
//!
//! A click is a button `Added<Pressed>` on a frame the left mouse button went down
//! ([`left_click`]): Bevy's button is pressed by any mouse button, and the game's buttons
//! answer the left one, a right-click being free to mean something else.

use bevy::{picking::hover::Hovered, prelude::*, ui_widgets};

/// A button, to spawn with a node.
pub fn button() -> impl Bundle {
    (ui_widgets::Button, Hovered::default())
}

/// Whether the pointer is on a button or holding it down.
/// Sets a part of the interface (a label's text, whether it shows, a border's colour) only
/// where it differs, and without marking it changed otherwise: a helper taking it by plain
/// `&mut` marked it changed by being handed it, compare as it might, and every label so set
/// was laid out again every frame (1.6 ms a frame of the interface's, by a trace).
pub fn put<T: bevy::ecs::change_detection::DetectChangesMut>(part: &mut T, to: T::Inner)
where
    T::Inner: PartialEq + Sized,
{
    part.set_if_neq(to);
}

/// A label says so, written only where it differs ([`put`]).
pub fn say<T: bevy::ecs::change_detection::DetectChangesMut<Inner = Text>>(
    text: &mut T,
    to: String,
) {
    put(text, Text(to));
}

/// A part shows or is hidden, written only where it differs ([`put`]).
pub fn show<T: bevy::ecs::change_detection::DetectChangesMut<Inner = Visibility>>(
    part: &mut T,
    shown: bool,
) {
    put(
        part,
        if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        },
    );
}

pub fn pointer_on(hovered: &Hovered, pressed: bool) -> bool {
    hovered.get() || pressed
}

/// Whether a button pressed this frame was clicked, with the left mouse button.
pub fn left_click(mouse: &ButtonInput<MouseButton>) -> bool {
    mouse.just_pressed(MouseButton::Left)
}

#[cfg(test)]
mod put_tests {
    use super::*;

    #[derive(Component, PartialEq, Debug)]
    struct Label(String);

    /// What `put` leaves as it was is not marked changed, and what it changes is: handed to
    /// a helper by plain `&mut`, a label was marked changed whatever the helper did.
    #[test]
    fn a_part_put_as_it_was_is_not_marked_changed() {
        let mut world = World::new();
        let label = world.spawn(Label("empty".into())).id();
        let mut same = Schedule::default();
        same.add_systems(|mut labels: Query<&mut Label>| {
            for mut label in &mut labels {
                put(&mut label, Label("empty".into()));
            }
        });
        let mut other = Schedule::default();
        other.add_systems(|mut labels: Query<&mut Label>| {
            for mut label in &mut labels {
                put(&mut label, Label("Boots".into()));
            }
        });
        let mut seen = Schedule::default();
        seen.add_systems(|labels: Query<(), Changed<Label>>, mut count: Local<u32>| {
            *count += 1;
            // The first run sees it as new; from the second, only what was changed since.
            if *count == 2 {
                assert_eq!(
                    labels.iter().count(),
                    0,
                    "put as it was, it reads as changed"
                );
            }
            if *count == 3 {
                assert_eq!(
                    labels.iter().count(),
                    1,
                    "put to another, it does not read as changed"
                );
            }
        });
        seen.run(&mut world);
        same.run(&mut world);
        seen.run(&mut world);
        other.run(&mut world);
        seen.run(&mut world);
        assert_eq!(world.get::<Label>(label).unwrap().0, "Boots");
    }
}
