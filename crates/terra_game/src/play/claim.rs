//! Your claim (D94): you build only inside ground you have claimed, so the world is not
//! built over or taken apart everywhere. Place a claim stone with the hammer to make one;
//! it starts 40 metres across and grows to 80 and then 160 when you pay at the stone with
//! the interact key (D104). One claim each, for now: it is a component of each player.
//!
//! While the hammer is in hand, its edge is drawn on the ground.

use super::bag::Notice;
use super::interact::{Focus, Focusing, Interactable};
use super::inventory::{Belongings, Cost, describe};
use super::props::Prop;
use super::{IsLocalPlayer, LocalPlayer, Player};
use crate::build::{Site, frame_at};
use crate::planet::Planet;
use bevy::prelude::*;

/// How far across a claim is at each step (D104).
const ACROSS_M: [f32; 3] = [40.0, 80.0, 160.0];
/// What growing a claim to the next step costs.
const GROW: [Cost; 2] = [
    &[("Wood", 10), ("Stone", 10)],
    &[("Wood", 20), ("Stone", 20), ("Iron", 4)],
];

/// A player's claim, if they have placed one.
#[derive(Component, Default)]
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

    /// How many times it has grown, for saving.
    pub fn step(&self) -> usize {
        self.step
    }

    pub fn set_step(&mut self, step: usize) {
        self.step = step.min(ACROSS_M.len() - 1);
    }

    /// What growing it costs, if it can grow.
    pub fn next(&self) -> Option<Cost> {
        GROW.get(self.step).copied()
    }
}

pub fn plugin(app: &mut App) {
    app.register_required_components::<Player, Claim>()
        .add_systems(
            Update,
            ((track, settle).chain(), grow.after(Focusing), show),
        );
}

/// A base is begun where its claim is: when the player's first claim stone goes down
/// before anything is built, the base's grid moves to stand there, its floor level with
/// the ground at the stone (D57). Otherwise a base begun on lower ground than the start
/// would stand on tall foundations. Things already set down keep their places.
fn settle(
    mut commands: Commands,
    mut props: Query<(&mut Prop, &Transform), Without<Player>>,
    mut site: ResMut<Site>,
    planet: Res<Planet>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(stone) = props
        .iter_mut()
        .find(|(prop, _)| prop.is_added() && prop.item.claims)
        .map(|(_, at)| *at)
    else {
        return;
    };
    if !site.operations.is_empty() {
        return;
    }
    let frame = frame_at(stone.translation, &site.frame);
    if frame.translation.distance(site.frame.translation) < 0.01 {
        return;
    }
    site.move_to(frame, &mut commands, &planet, &mut meshes);
    let to_base = frame.compute_affine().inverse();
    for (mut prop, at) in &mut props {
        if !prop.indoors {
            prop.at = to_base.transform_point3(at.translation - at.up() * prop.lift);
        }
    }
    info!(
        "Began the base at the claim stone, {:.1} m from the centre.",
        frame.translation.length()
    );
}

/// Notices a claim stone being placed or taken away. Stones are placed by this machine's
/// player, with the hammer or as a saved game is put back, so a new one is theirs.
fn track(
    placed: Query<(Entity, &Prop, &Transform), Added<Prop>>,
    mut removed: RemovedComponents<Prop>,
    mut ours: Single<&mut Claim, IsLocalPlayer>,
    mut claims: Query<&mut Claim, Without<LocalPlayer>>,
) {
    for (entity, prop, at) in &placed {
        if prop.item.claims {
            ours.stone = Some((entity, at.translation));
            info!("Claimed the land around {}.", at.translation);
        }
    }
    for entity in removed.read() {
        for mut claim in std::iter::once(ours.reborrow()).chain(claims.iter_mut()) {
            if claim.stone.is_some_and(|(stone, _)| stone == entity) {
                *claim = Claim::default();
                info!("A claim is given up.");
            }
        }
    }
}

/// Grows the claim when the player pays at its stone.
fn grow(
    focus: Res<Focus>,
    mut claim: Single<&mut Claim, IsLocalPlayer>,
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
    if let Some((kind, more)) = belongings.missing(cost, 1) {
        notice.say(format!(
            "Growing your claim takes {}: you need {more} more {}",
            describe(cost, 1),
            belongings.items().name(kind)
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
fn show(
    claim: Single<&Claim, IsLocalPlayer>,
    planet: Res<Planet>,
    belongings: Belongings,
    mut gizmos: Gizmos,
) {
    let Some((_, centre)) = claim.stone else {
        return;
    };
    if !belongings.tool_in_hand().is_some_and(|tool| tool.builds) {
        return;
    }
    let up = centre.normalize();
    let east = up.any_orthonormal_vector();
    let radius = claim.across_m() * 0.5;
    let steps = 96;
    let points = (0..=steps).map(|step| {
        let turn = step as f32 / steps as f32 * std::f32::consts::TAU;
        let out = Quat::from_axis_angle(up, turn) * east * radius;
        let ground = planet.ground_at(centre + out);
        ground + ground.normalize() * 0.15
    });
    gizmos.linestrip(points, Color::srgba(1.0, 0.78, 0.3, 0.8));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growing_costs_kinds_of_item_there_are() {
        let items = super::super::inventory::Items::default();
        for (name, _) in GROW.iter().copied().flatten() {
            assert!(items.find(name).is_some(), "{name} is not a kind of item");
        }
    }

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
