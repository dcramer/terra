//! Loss and repair (D193, step 6 of docs/building-plan.md): cubes take damage, are
//! destroyed, and what then hangs from nothing collapses into debris; debris is gathered
//! for part of what went in; a damaged cube is repaired for a share of what it cost.
//!
//! - **Health is the kind's** ([`crate::cost::Toughness`], its kit's `toughness`, by its
//!   pattern unless its material says otherwise). A cube's damage is kept beside the cubes,
//!   as what a cube changes to is, since a cube's 32 bits are full (D231).
//! - **Damage is an operation** ([`Op::Damage`]), as every change to a base is, with an
//!   inverse. An attack (D90) is the one thing that will send it; until then the remote and
//!   a debug key do. It is never refused for what it brings down: a player is refused when
//!   taking apart would leave anything hanging (D193), harm is not asked.
//! - **What collapses is found by a flood from the change** (`support::unjoined`), on the
//!   same rule of joined as placing (D199), lowest first, so a hit costs what it brings
//!   down and the cubes round it, not the base.
//! - **Debris falls straight down** its column to what stops it, a cube that stays or the
//!   ground, and lies there as a pile (a [`Pile`]) of what fell into it, giving back its
//!   kinds' salvage share of what went into each cube, exactly. A pile is in the way of
//!   building where it lies, and rests on its cube as an item does.
//! - **A damaged cube is mended before it comes out**: taking apart gives back all that
//!   went in (D193), so it is no way round a repair's cost.
//! - **The ground giving way** ([`Op::Subside`]) is sampled again as [`Op::Ground`] is but
//!   never refused, and what it leaves hanging collapses as harm's does.

use crate::cost::{Amount, Bill, Toughness};
use crate::cubes::{Cube, Kind, PartId, Replacement};
use crate::ground::{Ground, Sample, cube_cm};
use crate::ops::{Change, Context, Done, Op, Refusal};
use crate::{Base, Part};
use glam::IVec3;
use std::collections::{BTreeMap, BTreeSet};

/// Debris lying in a cube: what fell into it and what gathering it gives back.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Pile {
    /// How many cubes of each kind fell into it, by kind.
    pub kinds: Vec<(Kind, u32)>,
    /// How many fitted doors and windows fell into it.
    #[serde(default)]
    pub props: u32,
    /// What gathering it gives back.
    pub gives: Bill,
    /// The height it lies on, in centimetres in the base's own space: the ground's, or the
    /// top of the cube under it.
    pub floor_cm: i32,
    /// The highest layer anything in it fell from, to draw it falling.
    pub from: i32,
}

impl Pile {
    /// How many cubes fell into it.
    pub fn cubes(&self) -> u32 {
        self.kinds.iter().map(|(_, count)| count).sum()
    }

    /// Another pile's debris added to this one.
    fn add(&mut self, other: &Pile) {
        for &(kind, count) in &other.kinds {
            match self.kinds.iter_mut().find(|(each, _)| *each == kind) {
                Some((_, mine)) => *mine += count,
                None => self.kinds.push((kind, count)),
            }
        }
        self.kinds.sort();
        self.props += other.props;
        self.gives.add(&other.gives);
        self.from = self.from.max(other.from);
    }
}

/// What puts a base back as it was before harm, the ground giving way, a repair or
/// gathering: [`Op::Restore`]'s, made by the server as that act's inverse; and a patch's own
/// inverse, as it is applied, so an undo is undone too.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Patch {
    /// Cubes put back where there is none, or taken away.
    pub cubes: Vec<([i32; 3], Option<Cube>)>,
    /// The parts those put back were in, in case none of theirs is left.
    pub parts: Vec<(PartId, Part)>,
    /// What those of them marked to change were changing to.
    pub replacing: Vec<([i32; 3], Replacement)>,
    /// Props fitted again, with what was paid for them, or taken away, by their lowest cube.
    pub props: Vec<([i32; 3], Option<crate::ops::Fitted>)>,
    /// Cubes' damage set again, none as 0.
    pub damage: Vec<([i32; 3], u16)>,
    /// Debris set again, or taken away.
    pub debris: Vec<([i32; 3], Option<Pile>)>,
    /// The debris as the act left it, which must be so still: what was gathered since is
    /// not put back as cubes too.
    pub expect: Vec<([i32; 3], Option<Pile>)>,
    /// The ground under columns set again, or forgotten.
    pub ground: Vec<([i32; 2], Option<i16>)>,
    pub took: Bill,
    pub gave: Bill,
    /// What it brings back that was lost, and what it loses.
    pub recovered: Bill,
    pub lost: Bill,
}

impl Base {
    /// How much damage a cube has taken.
    pub fn damage(&self, at: IVec3) -> u16 {
        self.damage.get(&at.to_array()).copied().unwrap_or(0)
    }

    /// Every damaged cube and its damage, sorted.
    pub fn damaged(&self) -> impl Iterator<Item = (IVec3, u16)> + '_ {
        self.damage
            .iter()
            .map(|(&at, &damage)| (IVec3::from_array(at), damage))
    }

    /// Every pile of debris, by the cube it lies in, sorted.
    pub fn debris(&self) -> impl Iterator<Item = (IVec3, &Pile)> + '_ {
        self.debris
            .iter()
            .map(|(&at, pile)| (IVec3::from_array(at), pile))
    }

    /// The debris lying in a cube.
    pub fn pile(&self, at: IVec3) -> Option<&Pile> {
        self.debris.get(&at.to_array())
    }

    /// What went into a cube: so many of its steps, on its part's terms.
    pub fn put_in(&self, cube: &Cube) -> Bill {
        if !cube.is_in() {
            return Bill::default();
        }
        let terms = self.terms(cube);
        terms
            .per_step
            .times(u64::from(cube.level() / terms.step().max(1)))
    }

    /// What repairing a cube costs: its kind's repair share of what went into it, a quarter
    /// of that for each quarter of its health it has lost. `None` if it is not damaged.
    pub fn repair_cost(&self, at: IVec3, toughness: &Toughness) -> Option<Bill> {
        let damage = self.damage(at);
        let cube = self.cube(at).filter(|_| damage > 0)?;
        let quarters = u64::from(toughness.quarters(damage));
        Some(
            self.put_in(&cube)
                .share(toughness.repair)
                .share(Amount::new(quarters, 4)),
        )
    }

    /// Harms cubes ([`Op::Damage`]).
    pub(crate) fn harm(
        &self,
        cubes: &[[i32; 3]],
        amount: u16,
        cx: &Context,
    ) -> Result<Change, Refusal> {
        // Harm is not the claim's to bound: anything built can be hit.
        let named: BTreeSet<[i32; 3]> = cubes.iter().copied().collect();
        if named.is_empty() || named.len() != cubes.len() || amount == 0 {
            return Err(Refusal::Malformed);
        }
        let mut change = Change::default();
        let mut patch = Patch::default();
        let mut destroyed = BTreeSet::new();
        for &at in &named {
            let place = IVec3::from_array(at);
            let Some(cube) = self.cube(place).filter(Cube::is_in) else {
                continue;
            };
            let was = self.damage(place);
            let now = was.saturating_add(amount);
            patch.damage.push((at, was));
            if now >= cx.rules.toughness(cube.kind).health {
                destroyed.insert(at);
            } else {
                change.damage.push((place, Some(now)));
            }
        }
        if patch.damage.is_empty() {
            return Err(Refusal::Nothing(IVec3::from_array(cubes[0])));
        }
        let lost = self.unjoined_without(&destroyed);
        let mut falling = destroyed;
        falling.extend(lost.iter().map(|at| at.to_array()));
        let columns = falling.iter().map(|at| [at[0], at[2]]).collect();
        self.collapse(
            &falling,
            &columns,
            &self.ground,
            cx,
            &mut change,
            &mut patch,
        );
        change.done.undo = vec![Op::Restore(Box::new(patch))];
        Ok(change)
    }

    /// The ground under columns gives way, or rises ([`Op::Subside`]).
    pub(crate) fn subside(&self, samples: &[Sample], cx: &Context) -> Result<Change, Refusal> {
        let columns: BTreeSet<[i32; 2]> = samples.iter().map(|sample| sample.column).collect();
        if columns.is_empty() || columns.len() != samples.len() {
            return Err(Refusal::Malformed);
        }
        let mut ground = self.ground.clone();
        let mut change = Change::default();
        let mut patch = Patch::default();
        let mut moved = BTreeSet::new();
        for sample in samples {
            // Only where something stands is the ground kept, as `Op::Ground` keeps it.
            let [x, z] = sample.column;
            if !self.occupied(x, z) {
                continue;
            }
            patch
                .ground
                .push((sample.column, self.ground.sample(sample.column)));
            ground.set(sample.column, Some(sample.height_cm));
            change.ground.push(*sample);
            moved.insert(sample.column);
        }
        if change.ground.is_empty() {
            return Err(Refusal::Unchanged);
        }
        // What the ground no longer reaches, and what hangs from that alone.
        let starts: Vec<IVec3> = moved
            .iter()
            .flat_map(|&[x, z]| {
                self.column(x, z)
                    .map(move |y| IVec3::new(x, y, z))
                    .collect::<Vec<_>>()
            })
            .filter(|&at| self.is_in(at) && self.ground.reaches(at) && !ground.reaches(at))
            .collect();
        let falling: BTreeSet<[i32; 3]> = self
            .unjoined(&BTreeSet::new(), starts, &ground)
            .into_iter()
            .map(|at| at.to_array())
            .collect();
        let mut columns: BTreeSet<[i32; 2]> = falling.iter().map(|at| [at[0], at[2]]).collect();
        columns.extend(moved);
        self.collapse(&falling, &columns, &ground, cx, &mut change, &mut patch);
        change.done.undo = vec![Op::Restore(Box::new(patch))];
        Ok(change)
    }

    /// Takes cubes falling out of the base into debris, with props whose holes they were
    /// part of, and lets debris lying on them or in `columns` fall too: the change, and
    /// what puts it back.
    fn collapse(
        &self,
        falling: &BTreeSet<[i32; 3]>,
        columns: &BTreeSet<[i32; 2]>,
        ground: &Ground,
        cx: &Context,
        change: &mut Change,
        patch: &mut Patch,
    ) {
        let half = Toughness::default().salvage;
        let mut piles: BTreeMap<[i32; 3], Pile> = BTreeMap::new();
        let mut parts: BTreeMap<PartId, Part> = BTreeMap::new();
        let mut lost = Bill::default();
        let keep_part = |id: Option<PartId>, parts: &mut BTreeMap<PartId, Part>| {
            if let Some(id) = id
                && let Some(part) = self.part(id)
            {
                parts.insert(id, Part::new(part.kind, part.terms.clone()));
            }
        };
        let recorded: BTreeSet<[i32; 3]> = patch.damage.iter().map(|(at, _)| *at).collect();
        for &at in falling {
            let place = IVec3::from_array(at);
            let Some(cube) = self.cube(place) else {
                continue;
            };
            patch.cubes.push((at, Some(cube)));
            change.cubes.push((place, None));
            keep_part(cube.part, &mut parts);
            if let Some(to) = self.replacing(place) {
                patch.replacing.push((at, to));
                keep_part(to.1, &mut parts);
            }
            if self.damage.contains_key(&at) && !recorded.contains(&at) {
                patch.damage.push((at, self.damage(place)));
            }
            // Its damage goes with it, whatever this act did to it first.
            change.damage.push((place, None));
            let toughness = cx.rules.toughness(cube.kind);
            let went_in = self.put_in(&cube);
            let back = went_in.share(toughness.salvage);
            lost.add(&went_in.share(rest_of(toughness.salvage)));
            let (rest, floor_cm) = self.rest(place, falling, ground);
            let pile = piles.entry(rest.to_array()).or_default();
            pile.add(&Pile {
                kinds: vec![(cube.kind, 1)],
                gives: back,
                from: at[1],
                ..Pile::default()
            });
            pile.floor_cm = floor_cm;
        }
        patch.parts = parts.into_iter().collect();
        // A door or window whose hole is broken falls with it.
        for prop in self.props() {
            let broken = self
                .hole_for(prop)
                .is_some_and(|holders| holders.iter().any(|at| falling.contains(&at.to_array())));
            if !broken {
                continue;
            }
            let paid = self.prop_paid.get(&prop.min).cloned().unwrap_or_default();
            change.props.push((prop.min, None));
            patch.props.push((prop.min, Some((*prop, paid.clone()))));
            lost.add(&paid.share(rest_of(half)));
            let (rest, floor_cm) = self.rest(IVec3::from_array(prop.min), falling, ground);
            let pile = piles.entry(rest.to_array()).or_default();
            pile.add(&Pile {
                props: 1,
                gives: paid.share(half),
                from: prop.max[1],
                ..Pile::default()
            });
            pile.floor_cm = floor_cm;
        }
        // The ground under columns the fall leaves empty, which is forgotten with them.
        let mut ground_was: BTreeMap<[i32; 2], Option<i16>> =
            patch.ground.iter().copied().collect();
        let emptied: BTreeSet<[i32; 2]> = falling.iter().map(|at| [at[0], at[2]]).collect();
        for [x, z] in emptied {
            if self.column(x, z).all(|y| falling.contains(&[x, y, z])) {
                ground_was
                    .entry([x, z])
                    .or_insert_with(|| self.ground.sample([x, z]));
            }
        }
        patch.ground = ground_was.into_iter().collect();
        // Debris whose footing went falls on, to lie with what fell after it.
        let mut moved: BTreeMap<[i32; 3], Option<Pile>> = BTreeMap::new();
        for (&at, pile) in &self.debris {
            let place = IVec3::from_array(at);
            if !columns.contains(&[at[0], at[2]]) {
                continue;
            }
            let under = place - IVec3::Y;
            let held = (self.is_in(under) && !falling.contains(&under.to_array()))
                || ground.reaches(place);
            if held {
                continue;
            }
            let (rest, floor_cm) = self.rest(place, falling, ground);
            if rest == place {
                continue;
            }
            moved.insert(at, None);
            let into = piles.entry(rest.to_array()).or_default();
            into.add(pile);
            into.floor_cm = floor_cm;
        }
        // Each pile laid onto whatever already lies there.
        for (at, pile) in piles {
            let mut now = match moved.get(&at) {
                Some(_) => Pile::default(),
                None => self.debris.get(&at).cloned().unwrap_or_default(),
            };
            let floor_cm = pile.floor_cm;
            now.add(&pile);
            now.floor_cm = floor_cm;
            moved.insert(at, Some(now));
        }
        for (at, pile) in moved {
            patch.debris.push((at, self.debris.get(&at).cloned()));
            patch.expect.push((at, pile.clone()));
            change.debris.push((at, pile));
        }
        patch.recovered = lost.clone();
        change.done.lost = lost;
    }

    /// Where something falling from a cube comes to rest, straight down its column: the
    /// first cube with the ground in it or under it, or one standing on a cube that stays
    /// in; and the height it lies on, in centimetres.
    fn rest(&self, from: IVec3, falling: &BTreeSet<[i32; 3]>, ground: &Ground) -> (IVec3, i32) {
        let cube = cube_cm();
        let column = [from.x, from.z];
        let height = i32::from(ground.height_cm(column));
        let mut at = from;
        // Never further than the base's deepest footing could be.
        for _ in 0..4096 {
            if ground.reaches(at) {
                // On the ground: in the cube its surface is in.
                let surface = IVec3::new(at.x, height.div_euclid(cube), at.z);
                return (surface.max(at), height);
            }
            let under = at - IVec3::Y;
            if self.is_in(under) && !falling.contains(&under.to_array()) {
                return (at, at.y * cube);
            }
            at = under;
        }
        (at, at.y * cube)
    }

    /// Mends damaged cubes ([`Op::Repair`]).
    pub(crate) fn repair(&self, cubes: &[[i32; 3]], cx: &Context) -> Result<Change, Refusal> {
        let named = self.named(cubes, cx)?;
        let mut change = Change::default();
        let mut patch = Patch::default();
        let mut took = Bill::default();
        for &at in &named {
            let cube = self.there(at)?;
            let toughness = cx.rules.toughness(cube.kind);
            let cost = self
                .repair_cost(at, &toughness)
                .ok_or(Refusal::NotDamaged(at))?;
            took.add(&cost);
            patch.damage.push((at.to_array(), self.damage(at)));
            change.damage.push((at, None));
        }
        if !cx.world.has(&took) {
            return Err(Refusal::Materials(took));
        }
        patch.gave = took.clone();
        patch.recovered = took.clone();
        change.done.took = took.clone();
        change.done.lost = took;
        change.done.undo = vec![Op::Restore(Box::new(patch))];
        Ok(change)
    }

    /// Gathers the debris lying in a cube ([`Op::Gather`]).
    pub(crate) fn gather(&self, at: IVec3) -> Result<Change, Refusal> {
        let pile = self.pile(at).ok_or(Refusal::NoDebris(at))?.clone();
        let mut change = Change::default();
        change.debris.push((at.to_array(), None));
        change.done.gave = pile.gives.clone();
        change.done.undo = vec![Op::Restore(Box::new(Patch {
            debris: vec![(at.to_array(), Some(pile.clone()))],
            expect: vec![(at.to_array(), None)],
            took: pile.gives,
            ..Patch::default()
        }))];
        Ok(change)
    }

    /// Puts back what a [`Patch`] holds ([`Op::Restore`]): refused if a cube it puts back is
    /// taken, one it takes away is gone, the debris is not as the act left it, or what it
    /// takes is not to hand.
    pub(crate) fn restore(&self, patch: &Patch, cx: &Context) -> Result<Change, Refusal> {
        let mut change = Change::default();
        for (at, pile) in &patch.expect {
            if self.debris.get(at) != pile.as_ref() {
                return Err(Refusal::NoDebris(IVec3::from_array(*at)));
            }
        }
        for &(at, cube) in &patch.cubes {
            let place = IVec3::from_array(at);
            match (cube, self.cube(place)) {
                (Some(_), Some(_)) => return Err(Refusal::Taken(place)),
                (None, None) => return Err(Refusal::Nothing(place)),
                (None, Some(_)) => change.damage.push((place, None)),
                (Some(_), None) => {}
            }
            change.cubes.push((place, cube));
        }
        if !cx.world.has(&patch.took) {
            return Err(Refusal::Materials(patch.took.clone()));
        }
        change.parts = patch.parts.clone();
        change.replacing = patch
            .replacing
            .iter()
            .map(|&(at, to)| (IVec3::from_array(at), Some(to)))
            .collect();
        for (min, prop) in &patch.props {
            let there = self.prop(IVec3::from_array(*min)).is_some();
            if there == prop.is_some() {
                return Err(Refusal::PropThere(IVec3::from_array(*min)));
            }
            change.props.push((*min, prop.clone()));
        }
        change.damage.extend(
            patch
                .damage
                .iter()
                .map(|&(at, damage)| (IVec3::from_array(at), Some(damage))),
        );
        change.debris = patch.debris.clone();
        for &(column, height_cm) in &patch.ground {
            match height_cm {
                Some(height_cm) => change.ground.push(Sample { column, height_cm }),
                None => change.forget.push(column),
            }
        }
        change.done = Done {
            took: patch.took.clone(),
            gave: patch.gave.clone(),
            recovered: patch.recovered.clone(),
            lost: patch.lost.clone(),
            undo: vec![Op::Restore(Box::new(self.inverse(patch)))],
        };
        Ok(change)
    }

    /// The patch that undoes one, made before it is applied: what it puts back taken away
    /// and what it takes away put back, each thing as it is now.
    fn inverse(&self, patch: &Patch) -> Patch {
        let mut back = Patch::default();
        let mut parts: BTreeMap<PartId, Part> = BTreeMap::new();
        let mut keep_part = |id: Option<PartId>| {
            if let Some(id) = id
                && let Some(part) = self.part(id)
            {
                parts.insert(id, Part::new(part.kind, part.terms.clone()));
            }
        };
        let mut damage: BTreeMap<[i32; 3], u16> = BTreeMap::new();
        let mut taken: BTreeSet<[i32; 3]> = BTreeSet::new();
        for &(at, cube) in &patch.cubes {
            let place = IVec3::from_array(at);
            match cube {
                Some(_) => back.cubes.push((at, None)),
                None => {
                    let now = self.cube(place);
                    back.cubes.push((at, now));
                    keep_part(now.and_then(|cube| cube.part));
                    if let Some(to) = self.replacing(place) {
                        back.replacing.push((at, to));
                        keep_part(to.1);
                    }
                    if self.damage(place) > 0 {
                        damage.insert(at, self.damage(place));
                    }
                    taken.insert(at);
                }
            }
        }
        back.parts = parts.into_iter().collect();
        for (min, prop) in &patch.props {
            back.props.push(match prop {
                Some(_) => (*min, None),
                None => (
                    *min,
                    self.prop(IVec3::from_array(*min)).map(|prop| {
                        let paid = self.prop_paid.get(min).cloned().unwrap_or_default();
                        (*prop, paid)
                    }),
                ),
            });
        }
        for &(at, _) in &patch.damage {
            damage.insert(at, self.damage(IVec3::from_array(at)));
        }
        back.damage = damage.into_iter().collect();
        back.debris = patch
            .debris
            .iter()
            .map(|(at, _)| (*at, self.debris.get(at).cloned()))
            .collect();
        back.expect = patch.debris.clone();
        // The ground as it is under every column it sets, and under those it leaves empty.
        let mut ground: BTreeMap<[i32; 2], Option<i16>> = patch
            .ground
            .iter()
            .map(|&(column, _)| (column, self.ground.sample(column)))
            .collect();
        for &[x, y, z] in &taken {
            let _ = y;
            if self.column(x, z).all(|y| taken.contains(&[x, y, z])) {
                ground.insert([x, z], self.ground.sample([x, z]));
            }
        }
        back.ground = ground.into_iter().collect();
        back.took = patch.gave.clone();
        back.gave = patch.took.clone();
        back.lost = patch.recovered.clone();
        back.recovered = patch.lost.clone();
        back
    }
}

/// How much darker a cube that has lost all its health is drawn, and the most of its
/// surface broken into dark flecks then.
const WORN_DARK: f32 = 0.34;
const WORN_FLECKS: f32 = 0.5;
/// How dark a fleck is, and how far across, in metres.
const FLECK_DARK: f32 = 0.42;
const FLECK_M: f32 = 0.07;

/// Wears a drawn surface where its cubes are damaged, by how many quarters of their health
/// they have lost: darker, and broken up in dark flecks that spread as more is lost, as a
/// battered wall is scorched, split and chipped, in whatever material it is (D14): each
/// corner by the cube it is part of, a little in from its face.
/// How much of its health the cube a point of a surface belongs to has lost, from 0 to 1:
/// the cube it lies in a little in from its face (`normal`).
pub fn lost(worn: &BTreeMap<[i32; 3], u8>, at: glam::Vec3, normal: glam::Vec3) -> f32 {
    let inside = at - normal * 0.02;
    let cube = (inside / crate::CELL_M).floor().as_ivec3().to_array();
    worn.get(&cube)
        .map_or(0.0, |&quarters| f32::from(quarters.min(4)) / 4.0)
}

pub fn wear(surface: &mut crate::Surface, worn: &[([i32; 3], u8)]) {
    let worn: BTreeMap<[i32; 3], u8> = worn.iter().copied().collect();
    let shade = |at: glam::Vec3, normal: glam::Vec3| -> f32 {
        let inside = at - normal * 0.02;
        let cube = (inside / crate::CELL_M).floor().as_ivec3().to_array();
        let Some(&quarters) = worn.get(&cube) else {
            return 1.0;
        };
        let lost = f32::from(quarters.min(4)) / 4.0;
        let fleck = (at / FLECK_M).floor().as_ivec3();
        let fleck = if unit(fleck) < lost * WORN_FLECKS {
            FLECK_DARK
        } else {
            1.0
        };
        (1.0 - WORN_DARK * lost) * fleck
    };
    let darken = |colour: &mut [f32; 3], by: f32| {
        for channel in colour.iter_mut() {
            *channel *= by;
        }
    };
    for index in 0..surface.positions.len() {
        let normal = surface.normals.get(index).copied().unwrap_or(glam::Vec3::Y);
        let by = shade(surface.positions[index], normal);
        if let Some(colour) = surface.colours.get_mut(index) {
            darken(colour, by);
        }
    }
    let shared = &mut surface.shared;
    for index in 0..shared.positions.len() {
        let normal = shared.normals.get(index).copied().unwrap_or(glam::Vec3::Y);
        let by = shade(shared.positions[index], normal);
        if let Some(colour) = shared.colours.get_mut(index) {
            darken(colour, by);
        }
    }
}

/// A number from 0 to 1 for a place, the same on every machine.
fn unit(at: IVec3) -> f32 {
    let mut hash = (at.x as u32).wrapping_mul(0x9E37_79B1)
        ^ (at.y as u32).wrapping_mul(0x85EB_CA77)
        ^ (at.z as u32).wrapping_mul(0xC2B2_AE3D);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x2C1B_3C6D);
    hash ^= hash >> 12;
    (hash & 0xFFFF) as f32 / 65535.0
}

/// What is left of the whole once a share is taken.
fn rest_of(share: Amount) -> Amount {
    Amount::whole(1).minus(share).unwrap_or(Amount::ZERO)
}

#[cfg(test)]
mod tests;
