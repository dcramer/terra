//! Your claim (D94): you build only inside ground you have claimed, so the world is not
//! built over or taken apart everywhere. Place a claim stone with the hammer to make one;
//! it starts 40 metres across and grows to 80 and then 160 when you pay at the stone with
//! the interact key (D104). One claim each, for now.
//!
//! While the hammer is in hand, its edge is drawn on the ground.

use super::bag::Notice;
use super::interact::{Focus, Focusing, Interactable};
use super::inventory::{Belongings, Cost, Raw, Tool, describe};
use super::props::Prop;
use crate::planet::Planet;
use bevy::prelude::*;

/// How far across a claim is at each step (D104).
const ACROSS_M: [f32; 3] = [40.0, 80.0, 160.0];
/// What growing a claim to the next step costs.
const GROW: [Cost; 2] = [
    &[(Raw::Wood, 10), (Raw::Stone, 10)],
    &[(Raw::Wood, 20), (Raw::Stone, 20), (Raw::Iron, 4)],
];

/// The player's claim, if they have placed one.
#[derive(Resource, Default)]
pub struct Claim {
    /// The claim stone, and where it stands.
    pub stone: Option<(Entity, Vec3)>,
    step: usize,
}

impl Claim {
    pub fn across_m(&self) -> f32 {
        ACROSS_M[self.step]
    }

    /// Whether a point on the planet is inside the claim.
    pub fn covers(&self, point: Vec3) -> bool {
        self.stone.is_some_and(|(_, centre)| {
            (point - centre).reject_from(centre.normalize()).length() <= self.across_m() * 0.5
        })
    }

    /// What growing it costs, if it can grow.
    pub fn next(&self) -> Option<Cost> {
        GROW.get(self.step).copied()
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Claim>()
        .add_systems(Update, (track, grow.after(Focusing), show));
}

/// Notices a claim stone being placed or taken away.
fn track(
    placed: Query<(Entity, &Prop, &Transform), Added<Prop>>,
    mut removed: RemovedComponents<Prop>,
    mut claim: ResMut<Claim>,
) {
    for (entity, prop, at) in &placed {
        if prop.item.claims {
            claim.stone = Some((entity, at.translation));
            info!("Claimed the land around {}.", at.translation);
        }
    }
    for entity in removed.read() {
        if claim.stone.is_some_and(|(stone, _)| stone == entity) {
            *claim = Claim::default();
            info!("The claim is given up.");
        }
    }
}

/// Grows the claim when the player pays at its stone.
fn grow(
    focus: Res<Focus>,
    mut claim: ResMut<Claim>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
) {
    if !focus.used(Interactable::ClaimStone) {
        return;
    }
    let Some(cost) = claim.next() else {
        notice.say(format!(
            "Your claim is as large as it grows, {} metres across",
            claim.across_m()
        ));
        return;
    };
    if let Some((raw, more)) = belongings.missing(cost, 1) {
        notice.say(format!(
            "Growing your claim takes {}: you need {more} more {}",
            describe(cost, 1),
            raw.name()
        ));
        return;
    }
    belongings.pay(cost, 1);
    claim.step += 1;
    notice.say(format!(
        "Your claim now reaches {} metres across",
        claim.across_m()
    ));
}

/// Draws the claim's edge on the ground while the hammer is in hand.
fn show(claim: Res<Claim>, planet: Res<Planet>, belongings: Belongings, mut gizmos: Gizmos) {
    let Some((_, centre)) = claim.stone else {
        return;
    };
    if belongings.holding() != Some(Tool::Hammer) {
        return;
    }
    let up = centre.normalize();
    let east = up.any_orthonormal_vector();
    let radius = claim.across_m() * 0.5;
    let steps = 96;
    let points = (0..=steps).map(|step| {
        let turn = step as f32 / steps as f32 * std::f32::consts::TAU;
        let out = Quat::from_axis_angle(up, turn) * east * radius;
        let ground = planet.drawn((centre + out).normalize());
        ground + ground.normalize() * 0.15
    });
    gizmos.linestrip(points, Color::srgba(1.0, 0.78, 0.3, 0.8));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claim_covers_what_is_within_half_its_width_and_grows() {
        let centre = Vec3::new(0.0, 2000.0, 0.0);
        let mut claim = Claim::default();
        assert!(!claim.covers(centre), "no stone, no claim");
        claim.stone = Some((Entity::PLACEHOLDER, centre));
        assert!(claim.covers(centre + Vec3::X * 19.0));
        assert!(!claim.covers(centre + Vec3::X * 21.0));
        // Height does not matter, only distance across the ground.
        assert!(claim.covers(centre + Vec3::Y * 50.0));
        assert!(claim.next().is_some());
        claim.step = 1;
        assert!(claim.covers(centre + Vec3::Z * 39.0));
        claim.step = 2;
        assert_eq!(claim.next(), None);
    }
}
