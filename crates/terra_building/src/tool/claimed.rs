//! Whether what a builder aims at may be built where it is, by the claims (D94, D193, D645):
//! nothing is built or taken apart outside a claim, but the claim stone that makes one;
//! there is one claim each; a new claim keeps clear of others' and of the village; and
//! anyone builds in any claim, the one they stand in.

use bevy::prelude::*;
use terra_survival::claim::{Claim, Keeps};

/// Why the place aimed at cannot be built on, if it cannot: `on_planet` the place; whether
/// the world is free of claims (the yard) and whether a claim stone is what is being placed;
/// the builder's own claim and the one they build in; who they are, every claim there is by
/// whose it is, and the villages; and how far across a new claim is.
pub(super) fn refusal(
    on_planet: Option<Vec3>,
    (free, placing_claim): (bool, bool),
    (claim, builds_in): (&Claim, &Claim),
    (player, claims, villages): (Entity, &[(Entity, Keeps)], &[Keeps]),
    first_across: f32,
) -> Option<&'static str> {
    match on_planet {
        // The yard, the flat test level, is all one claim.
        _ if free => None,
        Some(_) if placing_claim && claim.stone.is_some() => {
            Some("You already have a claim; take its stone away to move it")
        }
        Some(point) if placing_claim => terra_survival::claim::too_near(
            point,
            first_across,
            claims
                .iter()
                .filter(|(owner, _)| *owner != player)
                .map(|(_, keeps)| *keeps),
            villages.iter().copied(),
        ),
        Some(point) if !builds_in.covers(point) => Some(if builds_in.stone.is_none() {
            "Place a claim stone to build here: it is under Claim in the build menu"
        } else {
            "That is outside the claim"
        }),
        _ => None,
    }
}
