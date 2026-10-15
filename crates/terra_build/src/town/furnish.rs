//! What a lived-in town has standing about besides its buildings and the things its day is
//! done at (D346): the trough by the well; barrels by the stalls, by the forge and against
//! yards' backs; carts by the market and in yards; and fences round the yards, wattle round
//! the hens and split rails round the woodpiles, each a run of modules along the yard's
//! edge with a gap for its gate. Every one is a rule here with its numbers in
//! [`FURNISHING`], and each is placed only where it keeps clear of every building, thing,
//! lane and place to stand, so nothing it adds closes a way (`cargo xtask life`).

use super::{
    CLEAR_M, GATHERED_M, Thing, ThingKind, Town, facing_of, from_street, inside, point_segment,
    polygons, segment_polygon, segments,
};
use crate::layout::{hash, unit};
use glam::Vec2;

/// How a fence is made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FenceKind {
    /// Hazel rods woven between stakes, tight enough to keep hens in.
    Wattle,
    /// Split rails between posts, round a woodpile.
    Rail,
}

/// One module of a fence, from its start to its end on the ground, in the town's metres; the
/// game fits its model between them (the model contract's module, D303).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fence {
    pub kind: FenceKind,
    pub from: Vec2,
    pub to: Vec2,
}

/// How much of each a town has, and how its fences are laid: the layout's numbers, as
/// [`super::BUILDINGS`] is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Furnishing {
    /// Of the stalls, the share with barrels beside them, and at most how many each.
    pub stall_barrels: (f32, u32),
    /// Barrels beside the forge, its water for quenching.
    pub forge_barrels: u32,
    /// Of the yards, the share with barrels against their back, and at most how many each.
    pub yard_barrels: (f32, u32),
    /// Carts beside the market, and in yards deep enough to hold one.
    pub market_carts: u32,
    pub yard_carts: u32,
    /// A fence module's length along its line, by kind, and its gate's gap.
    pub wattle_m: f32,
    pub rail_m: f32,
    pub gate_m: f32,
}

pub const FURNISHING: Furnishing = Furnishing {
    stall_barrels: (0.6, 2),
    forge_barrels: 2,
    yard_barrels: (0.5, 2),
    // None on the square yet: one beside the stalls jammed its crowd (`cargo xtask life`).
    market_carts: 0,
    yard_carts: 2,
    wattle_m: 2.0,
    rail_m: 2.5,
    gate_m: 1.8,
};

/// How far a fence stands inside its yard's edge.
const FENCE_IN_M: f32 = 0.15;
/// How far a fence keeps from anything built, and from any way walked.
const FENCE_APART_M: f32 = 0.15;

impl Town {
    /// The town's fences, trough, barrels and carts, by [`FURNISHING`]'s numbers.
    pub(super) fn furnish(&mut self, seed: u32) {
        let numbers = FURNISHING;
        self.fences_round_yards(&numbers);
        self.trough();
        self.barrels(seed, &numbers);
        self.carts(seed, &numbers);
    }

    /// Takes away any barrel, cart or trough the path graph could not reach, so the graph is
    /// one, and walks it again.
    pub(super) fn unfurnish_cut_off(&mut self) {
        let parts = self.paths.parts();
        let Some(&main) = parts.first() else {
            return;
        };
        let before = self.things.len();
        self.things.retain(|thing| {
            !matches!(
                thing.kind,
                ThingKind::Barrel | ThingKind::Cart | ThingKind::Trough
            ) || parts.get(thing.node as usize) == Some(&main)
        });
        if self.things.len() != before {
            self.paths();
        }
    }

    /// Fences round each yard that is not a pen, which has its own walls: three sides whole,
    /// and the side toward the house either side of a gap where the lane comes in, whose
    /// last step is then on into the yard.
    fn fences_round_yards(&mut self, numbers: &Furnishing) {
        for (home, quad) in self.yards.clone() {
            let kind = match self
                .things
                .iter()
                .find(|t| t.building == Some(home) && t.kind != ThingKind::Door)
                .map(|t| t.kind)
            {
                Some(ThingKind::Yard) => FenceKind::Wattle,
                Some(ThingKind::Block) => FenceKind::Rail,
                _ => continue,
            };
            let middle = quad.iter().copied().sum::<Vec2>() / 4.0;
            let c: Vec<Vec2> = quad
                .iter()
                .map(|&p| p + (middle - p).normalize_or_zero() * FENCE_IN_M)
                .collect();
            // The side toward the house is from corner 2 to corner 3 (`turned_box`).
            let gate = (c[2] + c[3]) * 0.5;
            let along = (c[3] - c[2]).normalize_or_zero();
            let half = numbers.gate_m * 0.5;
            let runs = [
                (c[0], c[1]),
                (c[1], c[2]),
                (c[2], gate - along * half),
                (gate + along * half, c[3]),
                (c[3], c[0]),
            ];
            let module = match kind {
                FenceKind::Wattle => numbers.wattle_m,
                FenceKind::Rail => numbers.rail_m,
            };
            let mut laid = false;
            for (from, to) in runs {
                let length = from.distance(to);
                if length < module * 0.4 {
                    continue;
                }
                let count = libm::roundf(length / module).max(1.0) as usize;
                for i in 0..count {
                    let a = from.lerp(to, i as f32 / count as f32);
                    let b = from.lerp(to, (i + 1) as f32 / count as f32);
                    if self.fence_fits(home, a, b) {
                        self.fences.push(Fence {
                            kind,
                            from: a,
                            to: b,
                        });
                        laid = true;
                    }
                }
            }
            // In through the gap: the lane's last step, before the yard's gate, goes on
            // inside.
            if laid
                && let Some((_, lane)) = self.lanes.iter_mut().find(|(h, _)| *h == home)
                && let Some(&outside) = lane.last()
            {
                lane.push(gate + (gate - outside).normalize_or_zero() * 1.0);
            }
        }
    }

    /// Whether a fence module from `a` to `b` round a home's yard keeps clear of everything
    /// built, every thing and where it is stood at, and every lane.
    fn fence_fits(&self, home: usize, a: Vec2, b: Vec2) -> bool {
        self.footprints
            .iter()
            .flatten()
            .all(|quad| segment_polygon(a, b, quad) >= FENCE_APART_M)
            && self.things.iter().all(|thing| {
                thing
                    .outline()
                    .is_none_or(|quad| segment_polygon(a, b, &quad) >= FENCE_APART_M)
                    && point_segment(self.stand(thing), a, b) >= CLEAR_M + 0.1
            })
            && self.lanes.iter().all(|(owner, lane)| {
                // Its own lane's last step comes in through the gap.
                let steps = if *owner == home {
                    lane.len() - 1
                } else {
                    lane.len()
                };
                lane[..steps]
                    .windows(2)
                    .all(|pair| segments(a, b, pair[0], pair[1]) >= CLEAR_M + 0.05)
            })
            && self
                .streets
                .iter()
                .all(|street| from_street(&[a, b], street) >= street.wide * 0.5 + 0.5)
    }

    /// Whether a thing would stand clear where it is offered: of everything built, every
    /// other thing by `apart` (barrels a little closer to one another), every place
    /// someone stands, the streets, lanes and fences; and, if given, inside the square.
    fn furnishing_fits(&self, thing: &Thing, apart: f32, on_square: bool) -> bool {
        let Some(ground) = thing.outline() else {
            return false;
        };
        let stand = self.stand(thing);
        let square = &self.square.outline;
        (!on_square
            || ground.iter().all(|&corner| {
                inside(corner, square)
                    && (0..square.len()).all(|i| {
                        point_segment(corner, square[i], square[(i + 1) % square.len()]) >= 1.0
                    })
            }))
            && self
                .footprints
                .iter()
                .flatten()
                .all(|quad| polygons(&ground, quad) >= 0.3)
            && self.things.iter().all(|other| {
                let gap = if other.kind == ThingKind::Barrel && thing.kind == ThingKind::Barrel {
                    0.04
                } else {
                    apart
                };
                other.outline().is_none_or(|theirs| polygons(&ground, &theirs) >= gap)
                    && segment_polygon(self.stand(other), self.stand(other), &ground)
                        >= CLEAR_M + 0.2
                    && (other.kind != ThingKind::Gathering
                        || segment_polygon(other.at, other.at, &ground) >= GATHERED_M + 1.5)
            })
            && self.streets.iter().all(|street| {
                from_street(&ground, street) >= street.wide * 0.5 + 0.3
            })
            && self.lanes.iter().all(|(_, lane)| {
                lane.windows(2)
                    .all(|pair| segment_polygon(pair[0], pair[1], &ground) >= CLEAR_M + 0.15)
            })
            && self
                .fences
                .iter()
                .all(|fence| segment_polygon(fence.from, fence.to, &ground) >= 0.1)
            && self.standable(stand, CLEAR_M)
            // One in a yard is come to from the yard's gate.
            && thing.building.is_none_or(|home| {
                self.lanes
                    .iter()
                    .find(|(owner, _)| *owner == home)
                    .and_then(|(_, lane)| lane.last())
                    .is_some_and(|&gate| self.walkable(gate, stand))
            })
    }

    /// The trough along the square's edge nearest the well, a little in from it and out of
    /// the way of anyone coming onto the square from a street, its front to the square: the
    /// square's middle is where its crowd walks.
    fn trough(&mut self) {
        let Some(well) = self
            .things
            .iter()
            .find(|t| t.kind == ThingKind::Well)
            .cloned()
        else {
            return;
        };
        let outline = self.square.outline.clone();
        let middle = outline.iter().copied().sum::<Vec2>() / outline.len() as f32;
        let mut offers: Vec<(f32, Thing)> = Vec::new();
        for index in 0..outline.len() {
            let (a, b) = (outline[index], outline[(index + 1) % outline.len()]);
            let edge = b - a;
            let inward = (middle - a).dot(edge.perp()).signum() * edge.perp().normalize_or_zero();
            for step in 1..8 {
                let at = a + edge * (step as f32 / 8.0) + inward * 1.75;
                let mouths = self
                    .streets
                    .iter()
                    .map(|street| street.points[0].distance(at))
                    .fold(f32::MAX, f32::min);
                if mouths < 8.0 {
                    continue;
                }
                let trough = Thing::new(ThingKind::Trough, at, facing_of(inward), None);
                offers.push((at.distance(well.at), trough));
            }
        }
        offers.sort_by(|x, y| x.0.total_cmp(&y.0));
        for (_, trough) in offers {
            if self.furnishing_fits(&trough, 2.0, true) {
                self.things.push(trough);
                return;
            }
        }
    }

    /// Barrels: beside the forge, beside some stalls, and against the backs of some yards.
    fn barrels(&mut self, seed: u32, numbers: &Furnishing) {
        let roll = |salt: u32| unit(hash(&[seed, 41, salt]));
        let mut offers: Vec<(Vec2, f32, Option<usize>)> = Vec::new();
        for (index, thing) in self.things.iter().enumerate() {
            let salt = index as u32 * 8;
            match thing.kind {
                ThingKind::Forge => {
                    for k in 0..numbers.forge_barrels {
                        offers.push((
                            thing.place(Vec2::new(-1.75, -0.35 + k as f32 * 0.64)),
                            thing.facing,
                            None,
                        ));
                    }
                }
                ThingKind::Stall if roll(salt) < numbers.stall_barrels.0 => {
                    let side = if roll(salt + 1) < 0.5 { -1.0 } else { 1.0 };
                    let count = 1 + (roll(salt + 2) * numbers.stall_barrels.1 as f32) as u32;
                    for k in 0..count.min(numbers.stall_barrels.1) {
                        offers.push((
                            thing.place(Vec2::new(side * 1.72, -0.35 - k as f32 * 0.64)),
                            thing.facing,
                            None,
                        ));
                    }
                }
                _ => {}
            }
        }
        for (index, (home, quad)) in self.yards.iter().enumerate() {
            let salt = 1000 + index as u32 * 8;
            let kind = self
                .things
                .iter()
                .find(|t| t.building == Some(*home) && t.kind != ThingKind::Door)
                .map(|t| t.kind);
            if !matches!(kind, Some(ThingKind::Yard | ThingKind::Block))
                || roll(salt) >= numbers.yard_barrels.0
            {
                continue;
            }
            // Against the back, in the corner on the side the seed picks.
            let (corner, next, along) = if roll(salt + 1) < 0.5 {
                (quad[0], quad[1], quad[3])
            } else {
                (quad[1], quad[0], quad[2])
            };
            let across = (next - corner).normalize_or_zero();
            let inward = (along - corner).normalize_or_zero();
            let facing = facing_of(inward);
            let count = 1 + (roll(salt + 2) * numbers.yard_barrels.1 as f32) as u32;
            for k in 0..count.min(numbers.yard_barrels.1) {
                offers.push((
                    corner + inward * 0.5 + across * (0.55 + k as f32 * 0.64),
                    facing,
                    Some(*home),
                ));
            }
        }
        for (at, facing, home) in offers {
            let barrel = Thing::new(ThingKind::Barrel, at, facing, home);
            if self.furnishing_fits(&barrel, 0.5, false) {
                self.things.push(barrel);
            }
        }
    }

    /// Carts: one or so beside the market's stalls, along their row, and others in deep
    /// yards along a side, their shafts toward the house.
    fn carts(&mut self, seed: u32, numbers: &Furnishing) {
        let roll = |salt: u32| unit(hash(&[seed, 42, salt]));
        let stalls: Vec<Thing> = self
            .things
            .iter()
            .filter(|t| t.kind == ThingKind::Stall)
            .cloned()
            .collect();
        let mut placed = 0;
        let first = (roll(0) * stalls.len() as f32) as usize;
        'market: for turn in 0..stalls.len() {
            let stall = &stalls[(first + turn) % stalls.len()];
            for side in [1.0, -1.0] {
                for off in [3.4, 3.9] {
                    if placed >= numbers.market_carts {
                        break 'market;
                    }
                    let cart = Thing::new(
                        ThingKind::Cart,
                        stall.place(Vec2::new(side * off, -0.2)),
                        stall.facing + core::f32::consts::FRAC_PI_2 * side,
                        None,
                    );
                    if self.furnishing_fits(&cart, 1.2, true) {
                        self.things.push(cart);
                        placed += 1;
                    }
                }
            }
        }
        let mut placed = 0;
        for (index, (home, quad)) in self.yards.clone().into_iter().enumerate() {
            if placed >= numbers.yard_carts {
                break;
            }
            let kind = self
                .things
                .iter()
                .find(|t| t.building == Some(home) && t.kind != ThingKind::Door)
                .map(|t| t.kind);
            // A woodpile's yard: a hen yard's middle is where its hens are tended and washing
            // done.
            if kind != Some(ThingKind::Block) {
                continue;
            }
            // Along the yard's side, from its back toward the house.
            let toward = (quad[3] - quad[0]).normalize_or_zero();
            let deep = quad[0].distance(quad[3]);
            if deep < 5.5 {
                continue;
            }
            for (side, inward) in [(quad[0], quad[1]), (quad[1], quad[0])] {
                let across = (inward - side).normalize_or_zero();
                let at = side + across * 1.0 + toward * (deep * 0.5);
                let cart = Thing::new(ThingKind::Cart, at, facing_of(toward), Some(home));
                if roll(100 + index as u32) < 0.8 && self.furnishing_fits(&cart, 0.6, false) {
                    self.things.push(cart);
                    placed += 1;
                    break;
                }
            }
        }
    }
}
