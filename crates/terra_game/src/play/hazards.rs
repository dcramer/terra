//! Hazards: what a fire or a dragon is to what meets it. The shared types between what
//! makes them (building play's fire, the creatures' fliers) and what meets them (a shy
//! creature's mind, a base's fire), held here, the lowest layer both stand on (D407).

use bevy::prelude::*;

/// Something every shy creature runs from when it comes within so many metres, as from a
/// player it is shy of: a dragon on the wing, a fire (D382, D377). A shy creature startled by
/// it flees as its kind does, a bird taking wing away from it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Frightful {
    pub within_m: f32,
}

/// Something breathing fire from where it is, along its own forward (-Z, as Bevy's
/// transforms look): how far its flame reaches, how wide it spreads (the tangent of its half
/// angle), how hot it is (heat a second given what it meets, as a burning cube's full flame
/// gives the cube over it), and whether it is breathing now. A base's fire heats what its
/// cone meets (`build::fire`).
#[derive(Component, Clone, Copy, Debug)]
pub struct BreathesFire {
    pub reach_m: f32,
    pub spread: f32,
    pub heat: f32,
    pub on: bool,
    /// What it is breathing at, in the world, if anything: its flame goes there, not along
    /// its forward.
    pub aim: Option<Vec3>,
}
