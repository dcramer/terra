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
pub fn pointer_on(hovered: &Hovered, pressed: bool) -> bool {
    hovered.get() || pressed
}

/// Whether a button pressed this frame was clicked, with the left mouse button.
pub fn left_click(mouse: &ButtonInput<MouseButton>) -> bool {
    mouse.just_pressed(MouseButton::Left)
}
