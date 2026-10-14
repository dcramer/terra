//! Your claim (D94): you build only inside ground you have claimed, so the world is not
//! built over or taken apart everywhere. Place a claim stone with the hammer to make one;
//! it starts 40 metres across and grows to 80 and then 160 when you pay at the stone with
//! the interact key (D104). One claim each, for now: it is a component of each player.
//!
//! Claims do not overlap and keep a margin (D193): a stone is refused, and a claim does not
//! grow, where its edge would come within [`MARGIN_M`] of another claim's or of a village's
//! (D265).
//!
//! While the hammer is in hand, its edge is drawn on the ground.

use super::catalogue::{Builds, ClaimRule};
use super::interact::{Interactable, Interacted};
use super::inventory::{Belongings, Cost, describe};
use super::props::{PlacedBy, Prop};
use super::settlement::Village;
use super::tick::Sim;
use super::told::Told;
use super::{IsLocalPlayer, Player};
use crate::build::{Site, frame_at};
use crate::planet::Planet;
use bevy::prelude::*;

/// How far apart claims keep, edge to edge, and from a village's reach, in metres (D193,
/// D265): room to walk round a base, and for what is built at a claim's edge, its eaves
/// and footings, not to touch a neighbour's.
pub const MARGIN_M: f32 = 10.0;

/// Something a claim keeps clear of: where its middle is, and how far round it it reaches
/// across the ground, in metres.
pub type Keeps = (Vec3, f32);

/// Why a claim so far across round a centre would be too near something else: another
/// claim, or a village, within the margin of its edge; `None` if it keeps clear of all of
/// them. Distance is across the ground, as [`Claim::covers`] measures it.
pub fn too_near(
    centre: Vec3,
    across_m: f32,
    claims: impl IntoIterator<Item = Keeps>,
    villages: impl IntoIterator<Item = Keeps>,
) -> Option<&'static str> {
    let apart = |other: Vec3| (other - centre).reject_from(centre.normalize()).length();
    let near = |(other, reach): Keeps| apart(other) < across_m * 0.5 + reach + MARGIN_M;
    if claims.into_iter().any(near) {
        Some("Too near another claim: claims keep 10 m apart")
    } else if villages.into_iter().any(near) {
        Some("Too near the village: a claim keeps 10 m from it")
    } else {
        None
    }
}

/// A player's claim, if they have placed one: its stone, how many times it has grown, and
/// how far across that makes it (`ClaimRule`, in `assets/data/builds.ron`).
#[derive(Component, Default, Clone)]
pub struct Claim {
    /// The claim stone, and where it stands.
    pub stone: Option<(Entity, Vec3)>,
    step: usize,
    across_m: f32,
}

impl Claim {
    pub fn across_m(&self) -> f32 {
        self.across_m
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

    /// Grown so many times, as far as the rule lets it.
    pub fn set_step(&mut self, step: usize, rule: &ClaimRule) {
        self.step = step.min(rule.across_m.len() - 1);
        self.across_m = rule.across_m[self.step];
    }

    /// What it keeps clear of others: its stone and half its width, if it has a stone.
    pub fn keeps(&self) -> Option<Keeps> {
        self.stone.map(|(_, centre)| (centre, self.across_m * 0.5))
    }

    /// What growing it costs, if it can grow.
    pub fn next<'a>(&self, rule: &'a ClaimRule) -> Option<&'a Cost> {
        rule.grow.get(self.step).map(Vec::as_slice)
    }
}

pub fn plugin(app: &mut App) {
    app.register_required_components::<Player, Claim>()
        .add_systems(Update, ((track, settle).chain(), show))
        .add_systems(FixedUpdate, grow.in_set(Sim::World));
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
    placed: Query<(Entity, &Prop, &Transform, Option<&PlacedBy>), Added<Prop>>,
    mut removed: RemovedComponents<Prop>,
    ours: Option<Single<Entity, IsLocalPlayer>>,
    builds: Res<Builds>,
    mut claims: Query<&mut Claim>,
) {
    for (entity, prop, at, by) in &placed {
        if !prop.item.claims {
            continue;
        }
        // Whoever placed it; a saved game's stones are this machine's player's, whose
        // game it is.
        let owner = by.map(|by| by.0).or(ours.as_deref().copied());
        if let Some(mut claim) = owner.and_then(|owner| claims.get_mut(owner).ok()) {
            claim.stone = Some((entity, at.translation));
            claim.set_step(0, &builds.claim);
            info!("Claimed the land around {}.", at.translation);
        }
    }
    for entity in removed.read() {
        for mut claim in &mut claims {
            if claim.stone.is_some_and(|(stone, _)| stone == entity) {
                *claim = Claim::default();
                info!("A claim is given up.");
            }
        }
    }
}

/// Grows the claim when the player pays at its stone.
fn grow(
    mut interacted: MessageReader<Interacted>,
    mut claims: Query<&mut Claim>,
    builds: Res<Builds>,
    village: Option<Res<Village>>,
    mut belongings: Belongings,
    mut told: MessageWriter<Told>,
) {
    let rule = &builds.claim;
    let villages: Vec<Keeps> = village
        .iter()
        .flat_map(|village| village.1.iter().map(|&point| (point, 0.0)))
        .collect();
    for used in interacted.read() {
        let others: Vec<Keeps> = claims
            .iter()
            .filter(|other| other.stone.map(|(own, _)| own) != used.on(Interactable::ClaimStone))
            .filter_map(Claim::keeps)
            .collect();
        let Some(stone) = used.on(Interactable::ClaimStone) else {
            continue;
        };
        let (Ok(mut claim), Some(mut inventory)) =
            (claims.get_mut(used.player), belongings.of(used.player))
        else {
            continue;
        };
        let said = if claim.stone.map(|(own, _)| own) != Some(stone) {
            "That is not your claim stone".to_string()
        } else if let Some(cost) = claim.next(rule) {
            let grown = rule.across_m[(claim.step + 1).min(rule.across_m.len() - 1)];
            let centre = claim.stone.map_or(Vec3::ZERO, |(_, at)| at);
            if let Some(why) = too_near(
                centre,
                grown,
                others.iter().copied(),
                villages.iter().copied(),
            ) {
                format!("Your claim can't grow to {grown} metres here. {why}")
            } else if let Some((kind, more)) = inventory.missing(cost, 1) {
                let items = inventory.items();
                format!(
                    "Growing your claim takes {}: you need {more} more {}",
                    describe(items, cost, 1),
                    items.name(kind)
                )
            } else {
                inventory.pay(cost, 1);
                let step = claim.step + 1;
                claim.set_step(step, rule);
                format!("Your claim now reaches {} metres across", claim.across_m())
            }
        } else {
            format!(
                "Your claim is as large as it grows, {} metres across",
                claim.across_m()
            )
        };
        told.write(Told::new(used.player, said));
    }
}

/// The claim's edge as last laid on the ground, and for which claim and ground.
type Edge = Option<((Vec3, f32, usize), Vec<Vec3>)>;

/// Draws the claim's edge on the ground while the hammer is in hand. The edge is laid on
/// the ground again only when the claim or the ground changes.
fn show(
    claim: Single<&Claim, IsLocalPlayer>,
    planet: Res<Planet>,
    mut belongings: Belongings,
    mut gizmos: Gizmos,
    mut edge: Local<Edge>,
) {
    let Some(belongings) = belongings.mine() else {
        return;
    };
    let Some((_, centre)) = claim.stone else {
        return;
    };
    if !belongings.tool_in_hand().is_some_and(|tool| tool.builds) {
        return;
    }
    let laid_for = (centre, claim.across_m(), planet.voxels().edit_count());
    if edge.as_ref().is_none_or(|(was, _)| *was != laid_for) {
        let up = centre.normalize();
        let east = up.any_orthonormal_vector();
        let radius = claim.across_m() * 0.5;
        let steps = 96;
        let points = (0..=steps)
            .map(|step| {
                let turn = step as f32 / steps as f32 * std::f32::consts::TAU;
                let out = Quat::from_axis_angle(up, turn) * east * radius;
                let ground = planet.ground_at(centre + out);
                ground + ground.normalize() * 0.15
            })
            .collect();
        *edge = Some((laid_for, points));
    }
    if let Some((_, points)) = edge.as_ref() {
        gizmos.linestrip(points.iter().copied(), Color::srgba(1.0, 0.78, 0.3, 0.8));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::crafting::Stations;
    use crate::play::inventory::Items;

    #[test]
    fn claims_keep_apart_and_clear_of_the_village() {
        let up = Vec3::new(0.0, 2000.0, 0.0);
        let east = |metres: f32| up + Vec3::X * metres;
        // Another claim 40 m across: a new one 40 m across keeps 10 m from its edge, so its
        // stone 50 m from the other's or more.
        let other = [(east(0.0), 20.0)];
        assert!(
            too_near(east(49.0), 40.0, other, []).is_some(),
            "within the margin"
        );
        assert!(
            too_near(east(30.0), 40.0, other, []).is_some(),
            "overlapping"
        );
        assert_eq!(too_near(east(51.0), 40.0, other, []), None);
        // Only distance across the ground counts, not height.
        assert_eq!(too_near(east(51.0) + Vec3::Y * 30.0, 40.0, other, []), None);
        // A village keeps claims off its street and houses, and the margin past them: the
        // nearest point it reaches 200 m west.
        let village = [(east(-260.0), 0.0), (east(-200.0), 0.0)];
        let edge = -200.0 + MARGIN_M + 20.0;
        assert!(too_near(east(edge - 1.0), 40.0, [], village).is_some());
        assert_eq!(too_near(east(edge + 1.0), 40.0, [], village), None);
        // Grown, a claim reaches further, and may come too near what it kept clear of.
        assert!(too_near(east(51.0), 80.0, other, []).is_some());
    }

    #[test]
    fn a_claim_covers_what_is_within_half_its_width_and_grows() {
        let builds = Builds::read(&Items::default(), &Stations::read());
        let rule = &builds.claim;
        let centre = Vec3::new(0.0, 2000.0, 0.0);
        let mut claim = Claim::default();
        assert!(!claim.covers(centre), "no stone, no claim");
        claim.stone = Some((Entity::PLACEHOLDER, centre));
        claim.set_step(0, rule);
        let half = rule.across_m[0] * 0.5;
        assert!(claim.covers(centre + Vec3::X * (half - 1.0)));
        assert!(!claim.covers(centre + Vec3::X * (half + 1.0)));
        // Height does not matter, only distance across the ground.
        assert!(claim.covers(centre + Vec3::Y * 50.0));
        assert!(claim.next(rule).is_some());
        claim.set_step(1, rule);
        assert!(claim.covers(centre + Vec3::Z * (half + 1.0)));
        claim.set_step(rule.across_m.len() - 1, rule);
        assert_eq!(claim.next(rule), None);
    }
}
