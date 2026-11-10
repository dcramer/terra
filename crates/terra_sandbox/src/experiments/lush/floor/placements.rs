//! How each kind of the lush floor's undergrowth is placed (D246, D268): the table
//! `lay` walks, kind by kind.

use super::{Area, Here, Under};
use terra_world::realm::{GroundPlant, Undergrowth};

/// How each kind of undergrowth is placed: how many drift centres are tried, how many
/// plants a clump has and how far it spreads, how big they are, how far each is drawn,
/// where its near level hands over to its far one (none at 0), where its shadow has shrunk
/// away (none at 0), and how broad its drifts are; and, for a realm's own kind, its row
/// (`data/realms.ron`'s `undergrowth`), which the rest is read from.
#[derive(Clone)]
pub(super) struct Placement {
    pub(super) kind: Under,
    pub(super) name: &'static str,
    pub(super) tries: u32,
    pub(super) clump: (u32, u32),
    pub(super) spread_m: f32,
    pub(super) scale: (f32, f32),
    pub(super) seen_m: f32,
    pub(super) near_m: f32,
    pub(super) shadow_m: f32,
    pub(super) drift_m: f32,
    pub(super) own: Option<&'static GroundPlant>,
}

impl Placement {
    /// A realm's own kind's, from its row: its tries the forest walk's, as every kind's are,
    /// from how many its row tries a hectare.
    fn of(row: &'static GroundPlant) -> Self {
        Placement {
            kind: Under::Placed,
            name: row.name.as_str(),
            tries: (row.tries_ha * Area::YARD.undergrowth_ha()).round() as u32,
            clump: row.clump,
            spread_m: row.spread_m,
            scale: row.grown,
            seen_m: row.seen_m,
            near_m: row.near_m,
            shadow_m: 0.0,
            drift_m: row.drift_m,
            own: Some(row),
        }
    }

    /// How much it likes a place, 0 to 1, before its drifts: its kind's rule, and for a
    /// realm's own kind how its row likes the ground dry or damp.
    pub(super) fn likes(&self, here: &Here) -> f32 {
        let ground = self
            .own
            .map_or(1.0, |row| row.dry + (row.wet - row.dry) * here.wet);
        self.kind.likes(here) * ground
    }

    /// The share of its tries a world makes: the first realm's kinds by the realm's share
    /// of the first realm's plants (D642), a realm's own all of them.
    pub(super) fn share(&self, plants: f32) -> f32 {
        if self.own.is_some() { 1.0 } else { plants }
    }
}

/// The kinds a floor lays, in the order their dice are rolled: the first realm's, or in a
/// realm with undergrowth of its own those of the first realm's it keeps, then its own.
pub(super) fn placements(own: Option<&'static Undergrowth>) -> Vec<Placement> {
    let Some(own) = own else {
        return PLACEMENTS.to_vec();
    };
    let kept = |placement: &&Placement| own.keeps.iter().any(|name| name == placement.name);
    let mut all: Vec<Placement> = PLACEMENTS.iter().filter(kept).cloned().collect();
    all.extend(own.kinds.iter().map(Placement::of));
    all
}

/// Each kind's placement, in the order its dice are rolled: a new kind goes last, so every
/// other kind stays where it was.
pub(super) const PLACEMENTS: [Placement; 17] = [
    Placement {
        kind: Under::Bracken,
        name: "Bracken",
        tries: 11000,
        clump: (5, 13),
        spread_m: 1.1,
        scale: (0.7, 1.5),
        seen_m: 55.0,
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 16.0,
        own: None,
    },
    Placement {
        kind: Under::Fern,
        name: "Fern",
        tries: 6000,
        clump: (2, 5),
        spread_m: 0.8,
        scale: (0.6, 1.7),
        seen_m: 45.0,
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 9.0,
        own: None,
    },
    Placement {
        kind: Under::Frond,
        name: "Fern",
        tries: 14000,
        clump: (5, 12),
        spread_m: 0.9,
        scale: (0.55, 1.6),
        seen_m: 45.0,
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 9.0,
        own: None,
    },
    Placement {
        kind: Under::Herb,
        name: "Herb",
        tries: 12000,
        clump: (6, 14),
        spread_m: 0.7,
        scale: (0.6, 1.1),
        seen_m: 35.0,
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 7.0,
        own: None,
    },
    Placement {
        kind: Under::Hazel,
        name: "Hazel",
        tries: 6000,
        clump: (1, 3),
        spread_m: 1.4,
        scale: (0.85, 1.2),
        seen_m: 85.0,
        near_m: 25.0,
        shadow_m: 24.0,
        drift_m: 20.0,
        own: None,
    },
    Placement {
        kind: Under::Bilberry,
        name: "Bilberry",
        tries: 7000,
        clump: (5, 11),
        spread_m: 0.8,
        scale: (0.45, 0.8),
        seen_m: 40.0,
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 11.0,
        own: None,
    },
    Placement {
        kind: Under::Scrub,
        name: "Bush",
        tries: 3500,
        clump: (1, 3),
        spread_m: 1.0,
        scale: (0.85, 1.2),
        seen_m: 70.0,
        near_m: 22.0,
        shadow_m: 24.0,
        drift_m: 14.0,
        own: None,
    },
    Placement {
        kind: Under::Sapling,
        name: "Sapling",
        tries: 10000,
        clump: (1, 4),
        spread_m: 2.2,
        scale: (0.8, 1.2),
        seen_m: 85.0,
        near_m: 28.0,
        shadow_m: 32.0,
        drift_m: 24.0,
        own: None,
    },
    Placement {
        kind: Under::Branches,
        name: "Branches",
        tries: 6000,
        clump: (1, 2),
        spread_m: 1.2,
        scale: (0.5, 1.2),
        seen_m: 45.0,
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 10.0,
        own: None,
    },
    Placement {
        kind: Under::Flowers,
        name: "Flowers",
        tries: 5000,
        clump: (3, 9),
        spread_m: 0.6,
        scale: (0.8, 1.3),
        seen_m: 35.0,
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 7.0,
        own: None,
    },
    Placement {
        kind: Under::Coppice,
        name: "Hazel",
        tries: 18000,
        clump: (1, 4),
        spread_m: 2.6,
        scale: (0.8, 1.15),
        seen_m: 85.0,
        near_m: 28.0,
        shadow_m: 32.0,
        drift_m: 22.0,
        own: None,
    },
    Placement {
        kind: Under::Holly,
        name: "Holly",
        tries: 18000,
        clump: (1, 4),
        spread_m: 2.2,
        scale: (0.8, 1.2),
        seen_m: 85.0,
        near_m: 28.0,
        shadow_m: 32.0,
        drift_m: 26.0,
        own: None,
    },
    Placement {
        kind: Under::Spruce,
        name: "Young spruce",
        tries: 16000,
        clump: (2, 6),
        spread_m: 2.4,
        scale: (0.75, 1.2),
        seen_m: 90.0,
        near_m: 28.0,
        shadow_m: 32.0,
        drift_m: 20.0,
        own: None,
    },
    Placement {
        kind: Under::Bramble,
        name: "Bramble",
        tries: 8000,
        clump: (1, 4),
        spread_m: 1.8,
        scale: (0.8, 1.3),
        seen_m: 70.0,
        near_m: 25.0,
        shadow_m: 24.0,
        drift_m: 14.0,
        own: None,
    },
    Placement {
        kind: Under::Snag,
        name: "Snag",
        tries: 5000,
        clump: (1, 1),
        spread_m: 0.0,
        scale: (0.9, 1.1),
        seen_m: 110.0,
        near_m: 35.0,
        shadow_m: 45.0,
        drift_m: 30.0,
        own: None,
    },
    Placement {
        kind: Under::Twigs,
        name: "Twigs",
        // Fewer than the litter's twigs (D275), longer and lying loose.
        tries: 3000,
        clump: (1, 3),
        spread_m: 1.6,
        scale: (0.8, 1.2),
        seen_m: 30.0,
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 12.0,
        own: None,
    },
    Placement {
        kind: Under::Verge,
        name: "Verge",
        tries: 4500,
        clump: (3, 6),
        spread_m: 0.9,
        scale: (0.7, 1.3),
        seen_m: 55.0,
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 10.0,
        own: None,
    },
];
