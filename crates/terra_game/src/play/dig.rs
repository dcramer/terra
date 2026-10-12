//! Digging (D39, D96): a pickaxe aimed at bare ground digs it. Hold the button, and after
//! a moment a scoop of ground comes out and the hole shows. What came out is measured by
//! material: stone and ore become items by the volume dug, the rest of a part kept toward
//! the next, so nothing dug is lost. Soil and sand give nothing yet.
//!
//! A tree, a rock or a plant in the way is worked as before (`harvest`); digging is for
//! when the pickaxe meets the ground itself. Digging works anywhere for now; that the
//! world heals outside a claim (D94) comes later.

use super::bag::Notice;
use super::controls::{Action, Controls};
use super::dial::Dial;
use super::harvest::Cutting;
use super::inventory::{Belongings, Cut, InventoryScreen, Raw};
use super::skills::{Did, ONCE, Skill, Skills};
use super::work::Hold;
use super::{IsLocalPlayer, Player, Stance};
use crate::planet::MainCamera;
use crate::planet::Planet;
use bevy::prelude::*;
use terra_core::{Harvested, Operation};
use terra_voxel::material;

/// How far from the player the pickaxe reaches the ground.
const REACH_M: f32 = 4.0;
/// How big a scoop one dig takes.
const SCOOP_M: f32 = 0.7;
/// How long a dig takes with a stone pickaxe at the start, in seconds.
const DIG_SECONDS: f32 = 0.9;
/// How much ground makes one item of what it is made of, in cubic metres.
const PER_ITEM_M3: f32 = 0.4;

/// Where the pickaxe meets the ground, and how far through digging there it is.
/// What a player is digging, if anything.
#[derive(Component, Default)]
pub struct Digging {
    pub target: Option<Vec3>,
    held: Option<(Vec3, Hold)>,
}

impl Digging {
    pub fn progress(&self) -> Option<f32> {
        self.held.map(|(_, hold)| hold.fraction())
    }
}

/// What has been dug but not yet made an item, by what it is made of.
/// The loose stone and ore a player has dug that has not yet made a whole one.
#[derive(Component, Default)]
struct Spoil {
    stone_m3: f32,
    ore_m3: f32,
}

pub fn plugin(app: &mut App) {
    app.register_required_components::<Player, Digging>()
        .register_required_components::<Player, Spoil>()
        .add_systems(Update, dig.after(super::harvest::Harvesting));
}

/// The items a part of what was dug makes, and what is left toward the next.
fn items_from(volume_m3: f32) -> (u32, f32) {
    let items = (volume_m3 / PER_ITEM_M3 + 1e-4).floor();
    (items as u32, (volume_m3 - items * PER_ITEM_M3).max(0.0))
}

fn dig(
    time: Res<Time>,
    controls: Controls,
    dial: Res<Dial>,
    screen: Res<InventoryScreen>,
    cutting: Single<&Cutting, IsLocalPlayer>,
    camera: Single<&GlobalTransform, With<MainCamera>>,
    player: Single<(&Transform, &mut Stance), IsLocalPlayer>,
    skills: Single<&Skills, IsLocalPlayer>,
    mut planet: ResMut<Planet>,
    mut belongings: Belongings,
    mut digging: Single<&mut Digging, IsLocalPlayer>,
    mut spoil: Single<&mut Spoil, IsLocalPlayer>,
    mut notice: ResMut<Notice>,
    mut did: MessageWriter<Did>,
    mut gizmos: Gizmos,
) {
    let (player_at, mut stance) = player.into_inner();
    let feet = player_at.translation;
    digging.target = None;
    let pickaxe = belongings
        .holding()
        .filter(|tool| tool.works(Cut::Rock) && !screen.open && !dial.open);
    // Something to cut in the way is cut, not dug round.
    let (Some(pickaxe), None) = (pickaxe, cutting.target) else {
        digging.held = None;
        return;
    };
    let origin = camera.translation();
    let Some(hit) = planet
        .raycast(origin, *camera.forward(), 30.0)
        .filter(|hit| hit.point.distance(feet) < REACH_M)
    else {
        digging.held = None;
        return;
    };
    digging.target = Some(hit.point);

    // A ring where it would dig, filling as it digs.
    let normal = planet.normal(hit.point);
    let across = normal.any_orthonormal_vector();
    let fraction = digging.progress().unwrap_or(0.0);
    let around = |turn: f32| {
        hit.point
            + normal * 0.05
            + Quat::from_axis_angle(normal, turn * std::f32::consts::TAU) * across * SCOOP_M
    };
    let amber = Color::srgb(1.0, 0.78, 0.25);
    gizmos.linestrip(
        (0..=32).map(|step| around(step as f32 / 32.0)),
        amber.with_alpha(0.45),
    );
    if fraction > 0.0 {
        let steps = (fraction * 32.0).ceil() as usize;
        gizmos.linestrip(
            (0..=steps).map(|step| around(step as f32 / steps as f32 * fraction)),
            Color::WHITE,
        );
    }

    if !controls.held(Action::Use) {
        digging.held = None;
        return;
    }
    // Moving the aim to another place starts again.
    if digging
        .held
        .is_some_and(|(at, _)| at.distance(hit.point) > SCOOP_M)
    {
        digging.held = None;
    }
    let speed = pickaxe.cuts(Cut::Rock).unwrap_or(1.0) * skills.speed(Skill::Mining);
    let (at, mut hold) = *digging
        .held
        .get_or_insert((hit.point, Hold::new(DIG_SECONDS / speed.max(0.01))));
    let finished = hold.advance(time.delta_secs());
    digging.held = Some((at, hold));
    stance.since_acting = 0.0;
    if !finished {
        return;
    }
    digging.held = None;

    // The scoop is centred a little into the ground, so it bites.
    let taken = planet.apply(&Operation::Dig {
        centre: at - normal * (SCOOP_M * 0.3),
        radius: SCOOP_M,
    });
    spoil.stone_m3 += taken.amount(Harvested::Material(material::STONE));
    spoil.ore_m3 += taken.amount(Harvested::Material(material::ORE));
    // What it gives is shown as it comes in (`gains`).
    let spoil: &mut Spoil = &mut spoil;
    for (raw, kept) in [
        (Raw::Stone, &mut spoil.stone_m3),
        (Raw::Ore, &mut spoil.ore_m3),
    ] {
        let (count, rest) = items_from(*kept);
        if count == 0 {
            continue;
        }
        let given = belongings.give(raw, count);
        // What does not fit stays in the spoil, to be taken when there is room.
        *kept = rest + (count - given) as f32 * PER_ITEM_M3;
        if given < count {
            notice.say("Your backpack is full");
        }
    }
    did.write(Did(Skill::Mining, ONCE));
    belongings.wear_in_hand(1);
    info!(
        "Dug at {:.1} m: {:.2} m³ of stone, {:.2} m³ of ore, the rest soil.",
        at.length(),
        taken.amount(Harvested::Material(material::STONE)),
        taken.amount(Harvested::Material(material::ORE))
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::inventory::Tool;

    #[test]
    fn what_is_dug_makes_items_and_keeps_the_rest() {
        assert_eq!(items_from(PER_ITEM_M3 * 0.5).0, 0);
        let (count, rest) = items_from(PER_ITEM_M3 * 2.25);
        assert_eq!(count, 2);
        assert!((rest - PER_ITEM_M3 * 0.25).abs() < 1e-5);
        assert_eq!(items_from(PER_ITEM_M3 * 2.0).0, 2);
    }

    #[test]
    fn a_pickaxe_digs_and_an_axe_does_not() {
        assert!(Tool::StonePickaxe.works(Cut::Rock));
        assert!(!Tool::StoneAxe.works(Cut::Rock));
    }
}
