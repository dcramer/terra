//! Operations (docs/building-plan.md, D21, D193): everything a player does to a base,
//! each checked by one pure function against the state when it arrives, in the server's
//! order, and each with an inverse for undo, checked the same way. A refusal says why, in
//! the words the ghost shows.
//!
//! What a check asks:
//!
//! - **In the claim**, and a plan **within the claim's budget** of planned cubes (D193).
//! - **Built from the bottom**: nothing goes in over a cube partly in, and a cube comes
//!   out only from under air or nothing in (D194: last in, first out). A cube marked to be
//!   carved out is the exception: an opening is cut into a wall already built, and what is
//!   over it stays, held by the wall either side as a lintel spans it, so long as all of it
//!   stays joined (construction-001, experience 4).
//! - **Nothing floats**: a cube going in is joined, face to face or along a roof's ramp
//!   (`support::ramp`), to the ground or to cubes already in; one coming out leaves nothing
//!   unjoined (rule 11).
//! - **Nothing taken while something rests on it**: a cube, a fitted prop or an item
//!   (D193).
//! - **No body in the way** of what goes in (construction-001, gap 17).
//! - **The materials there** for what goes in; what comes out gives back exactly what
//!   went in (gap 25).
//!
//! Applying an operation gives what it took, what it gave back, and the operations that
//! undo it.

use crate::cost::{Bill, Rules};
use crate::cubes::{Cube, FULL, Fill, Kind, PartId, Pending, Replacement, Shape, section_of};
use crate::ground::Sample;
use crate::support::Holder;
use crate::{Base, Part, Prop};
use glam::IVec3;
use std::collections::{BTreeMap, BTreeSet};

/// A cube of a plan: where, and its shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Planned {
    pub at: [i32; 3],
    pub shape: Shape,
}

/// Something a player does to a base.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Op {
    /// Lay a part's cubes as a plan, each with nothing in it, with the ground's height
    /// under the columns it is the first in. A part laid again takes more cubes.
    LayPlan {
        part: Option<PartId>,
        kind: Kind,
        cubes: Vec<Planned>,
        ground: Vec<Sample>,
    },
    /// Take planned cubes with nothing in them out of the plan.
    Unplan { cubes: Vec<[i32; 3]> },
    /// Put the next step into cubes: a course of logs into each cube it spans, a block
    /// into its cube.
    Fill { cubes: Vec<[i32; 3]> },
    /// Take the last step out of cubes.
    Empty { cubes: Vec<[i32; 3]> },
    /// Change the shape of planned cubes with nothing in them.
    SetShape { cubes: Vec<[i32; 3]>, shape: Shape },
    /// Mark cubes to change to another kind, in a part: the old taken out, then
    /// [`Op::Swap`], then the new put in.
    Replace {
        cubes: Vec<[i32; 3]>,
        kind: Kind,
        part: Option<PartId>,
    },
    /// Turn cubes marked to change, with nothing in them, to what they change to, and
    /// keep what they were, so a swap again turns them back.
    Swap { cubes: Vec<[i32; 3]> },
    /// Mark cubes as changed from another kind, in a part, the new going in: what clearing
    /// that mark, once the new is in, undoes (D232).
    Changed {
        cubes: Vec<[i32; 3]>,
        kind: Kind,
        part: Option<PartId>,
    },
    /// Mark cubes to be cut out, leaving an opening: taken out step by step, then out of
    /// the plan.
    Carve { cubes: Vec<[i32; 3]> },
    /// Clear the change marked on cubes.
    Cancel { cubes: Vec<[i32; 3]> },
    /// Fit a door, window or shutters into a hole in built walls.
    FitProp(Prop),
    /// Take a fitted prop out, by its lowest cube.
    RemoveProp { at: [i32; 3] },
    /// Open or shut a fitted prop.
    SetOpen { at: [i32; 3], open: bool },
    /// Make planned cubes with nothing in them, or cubes marked to be cut out and now
    /// empty, an opening: part of the wall still, walked through, a hole a door or window
    /// can be fitted into (construction-001, rule 7).
    Open { cubes: Vec<[i32; 3]> },
    /// Close openings with nothing fitted in them: planned cubes again, to be built.
    Close { cubes: Vec<[i32; 3]> },
    /// The ground under columns sampled again, after it was dug or filled
    /// (construction-001, gap 3): refused if it would leave anything built unjoined.
    Ground { samples: Vec<Sample> },
    /// Move cubes into a part of their kind laid on the same terms, a new one taking their
    /// terms: how parts that touch and agree are merged, and a part taken apart into pieces
    /// split (`parts`), so refunds stay exact whatever the cubes are grouped in.
    Regroup { cubes: Vec<[i32; 3]>, part: PartId },
    /// Harm cubes, as an attack does (D90, D193; `loss`): so much damage to each with
    /// anything in it; one left with no health is destroyed, and what then hangs from
    /// nothing comes down with it, said in [`Done::fell`]. Never refused for what it brings
    /// down.
    Damage { cubes: Vec<[i32; 3]>, amount: u16 },
    /// Mend damaged cubes, for a share of what they cost (`loss`).
    Repair { cubes: Vec<[i32; 3]> },
    /// The ground under columns gives way, or rises (D193): sampled again as [`Op::Ground`]
    /// is, but never refused; what it leaves hanging comes down.
    Subside { samples: Vec<Sample> },
    /// Puts back what harm, the ground giving way or a repair changed: the
    /// inverse the server made of one of those, saying itself what it takes and gives, so
    /// not a player's to send (`loss::Patch`).
    Restore(Box<crate::loss::Patch>),
}

/// A claim's ground and its budget of planned cubes (D193).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Claim {
    /// The columns it covers, counted in cubes east and south, both inclusive.
    pub min: [i32; 2],
    pub max: [i32; 2],
    /// How many cubes may be planned and not yet built.
    pub budget: usize,
}

impl Claim {
    pub fn covers(&self, column: [i32; 2]) -> bool {
        (self.min[0]..=self.max[0]).contains(&column[0])
            && (self.min[1]..=self.max[1]).contains(&column[1])
    }
}

/// What a check needs to know of the world beyond the base.
pub trait World {
    /// Whether a body, a creature or a dropped item is in a cube (gap 17).
    fn in_the_way(&self, _at: IVec3) -> bool {
        false
    }
    /// Whether an item stands on a cube or hangs from it (D193).
    fn holds_item(&self, _at: IVec3) -> bool {
        false
    }
    /// Whether the builder has what a bill asks.
    fn has(&self, _bill: &Bill) -> bool {
        true
    }
}

/// A world with nothing in the way and everything to hand, as the yard.
pub struct Free;

impl World for Free {}

/// Everything an operation is checked against beyond the base.
#[derive(Clone, Copy)]
pub struct Context<'a> {
    pub rules: &'a Rules,
    /// The claim the base is in; none, as in the yard, allows anything anywhere.
    pub claim: Option<&'a Claim>,
    pub world: &'a dyn World,
}

impl<'a> Context<'a> {
    /// Building with these rules, unclaimed, with nothing in the way and everything to
    /// hand.
    pub fn free(rules: &'a Rules) -> Context<'a> {
        Context {
            rules,
            claim: None,
            world: &Free,
        }
    }
}

/// Why an operation is refused, to show the player.
#[derive(Clone, Debug, PartialEq)]
pub enum Refusal {
    /// It names no cubes, or a cube twice.
    Malformed,
    /// A kind past the last a cube can hold.
    UnknownKind(Kind),
    /// The part is of another kind.
    WrongPart(PartId),
    OutsideClaim(IVec3),
    OverBudget {
        budget: usize,
    },
    /// Something is there already.
    Taken(IVec3),
    /// There is nothing there.
    Nothing(IVec3),
    /// It is an opening, not something built.
    Opening(IVec3),
    AlreadyBuilt(IVec3),
    /// Something is in it: take it out first.
    InProgress(IVec3),
    /// Nothing is in it to take out.
    NothingIn(IVec3),
    /// What it stands on is not built yet.
    BuildUnder(IVec3),
    /// Something is built over it, so it cannot be built a step at a time.
    Covered(IVec3),
    /// It would not be joined to the ground.
    Floats(IVec3),
    /// Something rests on it or is attached to it.
    Holds(Vec<Holder>),
    InTheWay(IVec3),
    /// Not enough materials: what it takes.
    Materials(Bill),
    /// Too many parts meet in one section.
    TooManyParts(IVec3),
    /// A change is already marked on it.
    Pending(IVec3),
    /// No change is marked on it.
    NotPending(IVec3),
    /// It is like that already.
    Unchanged,
    NoProp(IVec3),
    /// A prop is in the way.
    PropThere(IVec3),
    /// No hole of built walls fits it.
    NoHole,
    /// It is not an opening.
    NotOpen(IVec3),
    /// The ground would go from under what is built.
    Undermines(IVec3),
    /// It has taken no damage.
    NotDamaged(IVec3),
    /// It is damaged: mended before it is taken apart (`loss`).
    Damaged(IVec3),
    /// A roof's cube cut to a shape no roof has (`Shape::roofs`): a stair, an upper half, a
    /// post or one upside down (docs/building-voxels.md, gap 4).
    RoofShape(IVec3),
}

impl Refusal {
    /// The words shown to the player.
    pub fn message(&self) -> String {
        match self {
            Refusal::Malformed => "That can't be built.".into(),
            Refusal::UnknownKind(_) => "That isn't a kind of building.".into(),
            Refusal::WrongPart(_) => "That part is of another kind.".into(),
            Refusal::OutsideClaim(_) => "That's outside your claim.".into(),
            Refusal::OverBudget { budget } => {
                format!("Your claim can plan only {budget} cubes at once.")
            }
            Refusal::Taken(_) => "Something is built there already.".into(),
            Refusal::Nothing(_) => "There's nothing there.".into(),
            Refusal::Opening(_) => "That's an opening.".into(),
            Refusal::AlreadyBuilt(_) => "That's built already.".into(),
            Refusal::InProgress(_) => "Take it apart first.".into(),
            Refusal::NothingIn(_) => "There's nothing in it to take out.".into(),
            Refusal::BuildUnder(_) => "Build what it stands on first.".into(),
            Refusal::Covered(_) => "Something is built over it.".into(),
            Refusal::Floats(_) => "It would float: it must be joined to the ground.".into(),
            Refusal::Holds(holders) => {
                let what = match holders.first() {
                    Some(Holder::Cube(_)) => "the building above",
                    Some(Holder::Prop(_)) => "a door or window",
                    Some(Holder::Item(_)) => "something on it",
                    None => "something",
                };
                format!("It holds up {what}: take that away first.")
            }
            Refusal::InTheWay(_) => "Something is in the way.".into(),
            Refusal::Materials(bill) => format!("You need {bill}."),
            Refusal::TooManyParts(_) => "Too many parts meet here.".into(),
            Refusal::Pending(_) => "A change is already marked there.".into(),
            Refusal::NotPending(_) => "No change is marked there.".into(),
            Refusal::Unchanged => "It's like that already.".into(),
            Refusal::NoProp(_) => "There's nothing fitted there.".into(),
            Refusal::PropThere(_) => "A door or window is there.".into(),
            Refusal::NoHole => "It needs a hole in a built wall that fits it.".into(),
            Refusal::NotOpen(_) => "That's not an opening.".into(),
            Refusal::Undermines(_) => "That would dig out from under what is built.".into(),
            Refusal::NotDamaged(_) => "It isn't damaged.".into(),
            Refusal::Damaged(_) => "It's damaged: repair it first.".into(),
            Refusal::RoofShape(_) => "A roof is cut only to slopes, corners and halves.".into(),
        }
    }
}

/// What an operation did: what it took, what it gave back, and what undoes it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Done {
    pub took: Bill,
    pub gave: Bill,
    /// The operations that undo it, in order.
    pub undo: Vec<Op>,
    /// What it destroyed or used up for good (`loss`): what went into what came down, what
    /// a repair took.
    pub lost: Bill,
    /// What an undo brought back of what was lost.
    pub recovered: Bill,
    /// The cubes that came down, destroyed or left hanging, as they were: what the game
    /// bursts into fragments (D305).
    pub fell: Vec<(IVec3, Cube)>,
}

/// A prop fitted, and what was paid for it.
pub type Fitted = (Prop, Bill);

/// An operation's checked effect, before it is applied.
#[derive(Default)]
pub(crate) struct Change {
    pub(crate) parts: Vec<(PartId, Part)>,
    pub(crate) cubes: Vec<(IVec3, Option<Cube>)>,
    pub(crate) replacing: Vec<(IVec3, Option<Replacement>)>,
    pub(crate) ground: Vec<Sample>,
    /// Columns whose ground is forgotten, as it was before it was sampled.
    pub(crate) forget: Vec<[i32; 2]>,
    /// Props fitted, with what was paid for them, or taken out.
    pub(crate) props: Vec<([i32; 3], Option<Fitted>)>,
    /// Cubes' damage set, none or nothing taking it away (`loss`).
    pub(crate) damage: Vec<(IVec3, Option<u16>)>,
    pub(crate) done: Done,
}

impl Base {
    /// Checks an operation against the base as it is, changing nothing: what it would
    /// take, give back and be undone by, or why it is refused.
    pub fn check(&self, op: &Op, cx: &Context) -> Result<Done, Refusal> {
        self.plan(op, cx).map(|change| change.done)
    }

    /// Checks an operation and, if it is allowed, applies it.
    pub fn act(&mut self, op: &Op, cx: &Context) -> Result<Done, Refusal> {
        let change = self.plan_breaking(op, cx)?;
        for (id, part) in change.parts {
            self.parts.entry(id).or_insert(part);
        }
        for (at, cube) in change.cubes {
            self.put(at, cube);
        }
        for (at, to) in change.replacing {
            self.set_replacing(at, to);
        }
        for sample in change.ground {
            self.ground.set(sample.column, Some(sample.height_cm));
        }
        for column in change.forget {
            self.ground.set(column, None);
        }
        for (at, damage) in change.damage {
            match damage.filter(|&damage| damage > 0) {
                Some(damage) => self.damage.insert(at.to_array(), damage),
                None => self.damage.remove(&at.to_array()),
            };
            // Drawn as worn as it is (`sections`).
            self.cubes.mark(at);
        }
        for (at, prop) in change.props {
            let marks: Vec<IVec3> =
                match prop.as_ref().map(|(prop, _)| prop).or(self.props.get(&at)) {
                    Some(prop) => prop.cubes().collect(),
                    None => Vec::new(),
                };
            match prop {
                Some((prop, paid)) => {
                    self.props.insert(at, prop);
                    self.prop_paid.insert(at, paid);
                }
                None => {
                    self.props.remove(&at);
                    self.prop_paid.remove(&at);
                }
            };
            for cube in marks {
                self.cubes.mark(cube);
            }
        }
        self.settle();
        Ok(change.done)
    }

    /// Applies operations in order as one act, such as an undo: all of them, or, if one
    /// is refused, none.
    pub fn act_all(&mut self, ops: &[Op], cx: &Context) -> Result<Done, Refusal> {
        let mut done = Done::default();
        let mut undo: Vec<Vec<Op>> = Vec::new();
        for op in ops {
            match self.act(op, cx) {
                Ok(step) => {
                    done.took.add(&step.took);
                    done.gave.add(&step.gave);
                    done.lost.add(&step.lost);
                    done.recovered.add(&step.recovered);
                    done.fell.extend(step.fell);
                    undo.push(step.undo);
                }
                Err(refusal) => {
                    for ops in undo.iter().rev() {
                        for op in ops {
                            self.act(op, &Context::free(cx.rules))
                                .expect("an inverse just taken applies");
                        }
                    }
                    return Err(refusal);
                }
            }
        }
        done.undo = undo.into_iter().rev().flatten().collect();
        Ok(done)
    }

    pub(crate) fn plan(&self, op: &Op, cx: &Context) -> Result<Change, Refusal> {
        match op {
            Op::LayPlan {
                part,
                kind,
                cubes,
                ground,
            } => self.lay_plan(*part, *kind, cubes, ground, cx),
            Op::Unplan { cubes } => self.unplan(cubes, cx),
            Op::Fill { cubes } => self.fill(cubes, cx),
            Op::Empty { cubes } => self.empty(cubes, cx),
            Op::SetShape { cubes, shape } => self.set_shape(cubes, *shape, cx),
            Op::Replace { cubes, kind, part } => self.replace(cubes, *kind, *part, cx),
            Op::Swap { cubes } => self.swap(cubes, cx),
            Op::Changed { cubes, kind, part } => self.changed(cubes, *kind, *part, cx),
            Op::Carve { cubes } => self.carve(cubes, cx),
            Op::Cancel { cubes } => self.cancel(cubes, cx),
            Op::FitProp(prop) => self.fit_prop(prop, cx),
            Op::RemoveProp { at } => self.remove_prop(IVec3::from_array(*at), cx),
            Op::SetOpen { at, open } => self.set_open(IVec3::from_array(*at), *open, cx),
            Op::Open { cubes } => self.open(cubes, cx),
            Op::Close { cubes } => self.close(cubes, cx),
            Op::Ground { samples } => self.resample(samples, cx),
            Op::Regroup { cubes, part } => self.regroup(cubes, *part, cx),
            Op::Damage { cubes, amount } => self.harm(cubes, *amount, cx),
            Op::Repair { cubes } => self.repair(cubes, cx),
            Op::Subside { samples } => self.subside(samples),
            Op::Restore(patch) => self.restore(patch, cx),
        }
    }

    /// The cubes an operation names, sorted, each once and inside the claim.
    pub(crate) fn named(&self, cubes: &[[i32; 3]], cx: &Context) -> Result<Vec<IVec3>, Refusal> {
        let set: BTreeSet<[i32; 3]> = cubes.iter().copied().collect();
        if set.is_empty() || set.len() != cubes.len() {
            return Err(Refusal::Malformed);
        }
        let named: Vec<IVec3> = set.into_iter().map(IVec3::from_array).collect();
        for &at in &named {
            claimed(at, cx)?;
        }
        Ok(named)
    }

    /// The cube at a place, or a refusal that nothing is there.
    pub(crate) fn there(&self, at: IVec3) -> Result<Cube, Refusal> {
        self.cube(at).ok_or(Refusal::Nothing(at))
    }

    /// Cubes moved into a part: every one in a part of one kind and the same terms, the
    /// part they go into of that kind and those terms, or new, taking them.
    fn regroup(&self, cubes: &[[i32; 3]], part: PartId, cx: &Context) -> Result<Change, Refusal> {
        let named = self.named(cubes, cx)?;
        let mut change = Change::default();
        let mut agreed: Option<&Part> = None;
        let mut was: BTreeMap<PartId, Vec<[i32; 3]>> = BTreeMap::new();
        for &at in &named {
            let cube = self.there(at)?;
            let old = cube.part.ok_or(Refusal::WrongPart(part))?;
            let found = self.parts.get(&old).ok_or(Refusal::WrongPart(old))?;
            match agreed {
                Some(first) if first.kind != found.kind || first.terms != found.terms => {
                    return Err(Refusal::WrongPart(old));
                }
                Some(_) => {}
                None => agreed = Some(found),
            }
            if old != part {
                was.entry(old).or_default().push(at.to_array());
                change.cubes.push((
                    at,
                    Some(Cube {
                        part: Some(part),
                        ..cube
                    }),
                ));
            }
        }
        let Some(agreed) = agreed.filter(|_| !was.is_empty()) else {
            return Err(Refusal::Unchanged);
        };
        match self.parts.get(&part) {
            Some(into) if into.kind != agreed.kind || into.terms != agreed.terms => {
                return Err(Refusal::WrongPart(part));
            }
            Some(_) => {}
            None => change
                .parts
                .push((part, Part::new(agreed.kind, agreed.terms.clone()))),
        }
        let placed: Vec<(IVec3, Option<PartId>)> =
            named.iter().map(|&at| (at, Some(part))).collect();
        self.room_for_parts(&placed)?;
        change.done.undo = was
            .into_iter()
            .map(|(part, cubes)| Op::Regroup { cubes, part })
            .collect();
        Ok(change)
    }

    /// The part a plan or change is laid in, if it is new: refused if it is of another
    /// kind.
    fn new_part(
        &self,
        part: Option<PartId>,
        kind: Kind,
        cx: &Context,
    ) -> Result<Option<(PartId, Part)>, Refusal> {
        if kind > Kind::LAST {
            return Err(Refusal::UnknownKind(kind));
        }
        let Some(id) = part else { return Ok(None) };
        match self.parts.get(&id) {
            Some(found) if found.kind != kind => Err(Refusal::WrongPart(id)),
            Some(_) => Ok(None),
            None => Ok(Some((id, Part::new(kind, cx.rules.of(kind))))),
        }
    }

    /// Whether every section these cubes are in has room for their parts.
    fn room_for_parts(&self, cubes: &[(IVec3, Option<PartId>)]) -> Result<(), Refusal> {
        let mut by_section: BTreeMap<[i32; 3], BTreeSet<PartId>> = BTreeMap::new();
        for &(at, part) in cubes {
            let parts = by_section.entry(section_of(at).to_array()).or_default();
            parts.extend(part);
        }
        for (section, parts) in by_section {
            let section = IVec3::from_array(section);
            if !self.cubes.has_room(section, &parts) {
                return Err(Refusal::TooManyParts(section));
            }
        }
        Ok(())
    }

    fn lay_plan(
        &self,
        part: Option<PartId>,
        kind: Kind,
        cubes: &[Planned],
        ground: &[Sample],
        cx: &Context,
    ) -> Result<Change, Refusal> {
        let new_part = self.new_part(part, kind, cx)?;
        let places: Vec<[i32; 3]> = cubes.iter().map(|cube| cube.at).collect();
        let named = self.named(&places, cx)?;
        for &at in &named {
            if self.cube(at).is_some() {
                return Err(Refusal::Taken(at));
            }
            if let Some(prop) = self.prop_in(at) {
                return Err(Refusal::PropThere(IVec3::from_array(prop.min)));
            }
        }
        if kind.is_roof()
            && let Some(cube) = cubes.iter().find(|cube| !cube.shape.roofs())
        {
            return Err(Refusal::RoofShape(IVec3::from_array(cube.at)));
        }
        if let Some(claim) = cx.claim
            && self.cubes.planned() + named.len() > claim.budget
        {
            return Err(Refusal::OverBudget {
                budget: claim.budget,
            });
        }
        let placed: Vec<(IVec3, Option<PartId>)> = named.iter().map(|&at| (at, part)).collect();
        self.room_for_parts(&placed)?;
        let mut change = Change::default();
        change.parts.extend(new_part);
        for planned in cubes {
            let cube = Cube {
                shape: planned.shape,
                ..Cube::planned(kind, part)
            };
            change
                .cubes
                .push((IVec3::from_array(planned.at), Some(cube)));
        }
        // The ground under the columns it is the first in.
        let columns: BTreeSet<[i32; 2]> = named.iter().map(|at| [at.x, at.z]).collect();
        let mut sampled = BTreeSet::new();
        for sample in ground {
            if columns.contains(&sample.column)
                && !self.occupied(sample.column[0], sample.column[1])
                && sampled.insert(sample.column)
            {
                change.ground.push(*sample);
            }
        }
        change.done.undo = vec![Op::Unplan { cubes: places }];
        Ok(change)
    }

    fn unplan(&self, cubes: &[[i32; 3]], cx: &Context) -> Result<Change, Refusal> {
        let named = self.named(cubes, cx)?;
        let mut change = Change::default();
        // What lays them again: a plan for each part and kind, and the carving marked.
        let mut plans: BTreeMap<(Option<PartId>, Kind), Vec<Planned>> = BTreeMap::new();
        let mut carved = Vec::new();
        let mut opened = Vec::new();
        for &at in &named {
            let cube = self.there(at)?;
            if cube.fill == Fill::Open {
                // An opening goes with its plan, unless something is fitted in it.
                if let Some(prop) = self.prop_in(at) {
                    return Err(Refusal::PropThere(IVec3::from_array(prop.min)));
                }
                opened.push(at.to_array());
            } else if cube.level() > 0 {
                return Err(Refusal::InProgress(at));
            }
            match cube.pending {
                Pending::Replace | Pending::Swapped => return Err(Refusal::Pending(at)),
                Pending::Carve => carved.push(at.to_array()),
                Pending::None => {}
            }
            plans
                .entry((cube.part, cube.kind))
                .or_default()
                .push(Planned {
                    at: at.to_array(),
                    shape: cube.shape,
                });
            change.cubes.push((at, None));
        }
        // The ground under columns it leaves empty, as it was sampled.
        let leaving: BTreeSet<[i32; 3]> = named.iter().map(|at| at.to_array()).collect();
        let emptied = |column: [i32; 2]| {
            self.column(column[0], column[1])
                .all(|y| leaving.contains(&[column[0], y, column[1]]))
        };
        for ((part, kind), cubes) in plans {
            let columns: BTreeSet<[i32; 2]> = cubes.iter().map(|c| [c.at[0], c.at[2]]).collect();
            let ground = columns
                .into_iter()
                .filter(|&column| emptied(column))
                .filter_map(|column| {
                    let height_cm = self.ground.sample(column)?;
                    Some(Sample { column, height_cm })
                })
                .collect();
            change.done.undo.push(Op::LayPlan {
                part,
                kind,
                cubes,
                ground,
            });
        }
        if !opened.is_empty() {
            change.done.undo.push(Op::Open { cubes: opened });
        }
        if !carved.is_empty() {
            change.done.undo.push(Op::Carve { cubes: carved });
        }
        Ok(change)
    }

    fn fill(&self, cubes: &[[i32; 3]], cx: &Context) -> Result<Change, Refusal> {
        let named = self.named(cubes, cx)?;
        let mut after: BTreeMap<[i32; 3], Cube> = BTreeMap::new();
        // Cubes going in whole, by how many steps that is (D199 as amended by D305).
        let mut whole: BTreeMap<[i32; 3], u8> = BTreeMap::new();
        let mut change = Change::default();
        for &at in &named {
            let cube = self.there(at)?;
            if cube.fill == Fill::Open {
                return Err(Refusal::Opening(at));
            }
            if cube.is_built() {
                return Err(Refusal::AlreadyBuilt(at));
            }
            if cx.world.in_the_way(at) {
                return Err(Refusal::InTheWay(at));
            }
            // A damaged cube is mended before more goes in, as before anything comes out.
            if self.damage.contains_key(&at.to_array()) {
                return Err(Refusal::Damaged(at));
            }
            let terms = self.terms(&cube);
            // Under something already in, as a breach in a wall rebuilt under the wall that
            // stands over it, a cube goes in whole in one act, worked in from the side as a
            // repair is, since it may not be left partly in under what is in.
            if self.is_in(at + IVec3::Y) && !cube.pending.in_place() {
                let steps = (FULL - cube.level()).div_ceil(terms.step().max(1));
                change
                    .done
                    .took
                    .add(&terms.per_step.times(u64::from(steps)));
                after.insert(at.to_array(), cube.at_level(FULL));
                whole.insert(at.to_array(), steps);
                continue;
            }
            change.done.took.add(&terms.per_step);
            after.insert(at.to_array(), cube.at_level(cube.level() + terms.step()));
        }
        let now = |at: IVec3| after.get(&at.to_array()).copied().or(self.cube(at));
        let partial =
            |cube: Option<Cube>| cube.is_some_and(|cube| cube.is_in() && !cube.is_built());
        let carving = |cube: Option<Cube>| cube.is_some_and(|cube| cube.pending.in_place());
        for &at in &named {
            // Built from the bottom: nothing goes in over a cube partly in, and no cube is
            // left partly in under something already in, which nothing could then finish
            // or take apart; but a cube being carved out or changed to another kind may be
            // part in, as it comes out from under what stays, or goes in again (D232).
            let (under, over) = (at - IVec3::Y, at + IVec3::Y);
            if partial(now(under)) && !carving(now(under)) {
                return Err(Refusal::BuildUnder(under));
            }
            if partial(now(at)) && !carving(now(at)) && now(over).is_some_and(|cube| cube.is_in()) {
                return Err(Refusal::Covered(over));
            }
        }
        // Each cube going in joined, through the others going in, to the ground or to
        // something already in.
        let new: BTreeSet<[i32; 3]> = named
            .iter()
            .filter(|&&at| !self.is_in(at))
            .map(|at| at.to_array())
            .collect();
        let anchored = |at: IVec3| {
            self.grounded(at)
                || self
                    .joins(at)
                    .into_iter()
                    .any(|beside| !new.contains(&beside.to_array()) && self.is_in(beside))
        };
        let mut reached: BTreeSet<[i32; 3]> = new
            .iter()
            .copied()
            .filter(|&at| anchored(IVec3::from_array(at)))
            .collect();
        let mut frontier: Vec<[i32; 3]> = reached.iter().copied().collect();
        while let Some(at) = frontier.pop() {
            for next in self.joins(IVec3::from_array(at)) {
                let next = next.to_array();
                if new.contains(&next) && reached.insert(next) {
                    frontier.push(next);
                }
            }
        }
        if let Some(&at) = new.difference(&reached).next() {
            return Err(Refusal::Floats(IVec3::from_array(at)));
        }
        if !cx.world.has(&change.done.took) {
            return Err(Refusal::Materials(change.done.took));
        }
        change.cubes = after
            .into_iter()
            .map(|(at, cube)| (IVec3::from_array(at), Some(cube)))
            .collect();
        // Undone, a step out of what went in a step, and what went in whole out from under
        // what stays, as a carving comes out, step by step, its mark then cleared.
        let stepped: Vec<[i32; 3]> = cubes
            .iter()
            .copied()
            .filter(|at| !whole.contains_key(at))
            .collect();
        if !stepped.is_empty() {
            change.done.undo.push(Op::Empty { cubes: stepped });
        }
        if !whole.is_empty() {
            let under: Vec<[i32; 3]> = whole.keys().copied().collect();
            change.done.undo.push(Op::Carve {
                cubes: under.clone(),
            });
            let most = whole.values().copied().max().unwrap_or(0);
            for step in 0..most {
                let out: Vec<[i32; 3]> = whole
                    .iter()
                    .filter(|&(_, &steps)| steps > step)
                    .map(|(&at, _)| at)
                    .collect();
                change.done.undo.push(Op::Empty { cubes: out });
            }
            change.done.undo.push(Op::Cancel { cubes: under });
        }
        Ok(change)
    }

    fn empty(&self, cubes: &[[i32; 3]], cx: &Context) -> Result<Change, Refusal> {
        let named = self.named(cubes, cx)?;
        let mut after: BTreeMap<[i32; 3], Cube> = BTreeMap::new();
        let mut change = Change::default();
        for &at in &named {
            let cube = self.there(at)?;
            if !cube.is_in() {
                return Err(Refusal::NothingIn(at));
            }
            if cx.world.holds_item(at) {
                return Err(Refusal::Holds(vec![Holder::Item(at)]));
            }
            // A damaged cube is mended before it comes out, so taking apart, which gives
            // back all that went in, is no way round a repair's cost (`loss`).
            if self.damage.contains_key(&at.to_array()) {
                return Err(Refusal::Damaged(at));
            }
            let terms = self.terms(&cube);
            change.done.gave.add(&terms.per_step);
            after.insert(at.to_array(), cube.at_level(cube.level() - terms.step()));
        }
        let now = |at: IVec3| after.get(&at.to_array()).copied().or(self.cube(at));
        for &at in &named {
            // Last in, first out: nothing in the cube over it; but a cube marked to be
            // carved out or changed comes out from under what stays, which must stay
            // joined.
            let over = at + IVec3::Y;
            let carving = self.cube(at).is_some_and(|cube| cube.pending.in_place());
            if !carving && now(over).is_some_and(|cube| cube.is_in()) {
                return Err(Refusal::Holds(vec![Holder::Cube(over)]));
            }
            // Nor a prop fitted against it (D193).
            let held: Vec<Holder> = self
                .resting_on(at)
                .into_iter()
                .filter(|holder| !matches!(holder, Holder::Cube(_)))
                .collect();
            if !held.is_empty() {
                return Err(Refusal::Holds(held));
            }
        }
        let gone: BTreeSet<[i32; 3]> = after
            .iter()
            .filter(|(_, cube)| !cube.is_in())
            .map(|(&at, _)| at)
            .collect();
        let lost = self.unjoined_without(&gone);
        if !lost.is_empty() {
            return Err(Refusal::Holds(lost.into_iter().map(Holder::Cube).collect()));
        }
        change.cubes = after
            .into_iter()
            .map(|(at, cube)| (IVec3::from_array(at), Some(cube)))
            .collect();
        change.done.undo = vec![Op::Fill {
            cubes: cubes.to_vec(),
        }];
        Ok(change)
    }

    /// The planned cubes an operation names, with nothing in them.
    fn bare(&self, named: &[IVec3]) -> Result<Vec<(IVec3, Cube)>, Refusal> {
        named
            .iter()
            .map(|&at| {
                let cube = self.there(at)?;
                if cube.fill != Fill::Solid {
                    return Err(Refusal::Opening(at));
                }
                if cube.level() > 0 {
                    return Err(Refusal::InProgress(at));
                }
                Ok((at, cube))
            })
            .collect()
    }

    fn set_shape(&self, cubes: &[[i32; 3]], shape: Shape, cx: &Context) -> Result<Change, Refusal> {
        let named = self.named(cubes, cx)?;
        let mut change = Change::default();
        let mut was: BTreeMap<ShapeKey, Vec<[i32; 3]>> = BTreeMap::new();
        for (at, cube) in self.bare(&named)? {
            if cube.kind.is_roof() && !shape.roofs() {
                return Err(Refusal::RoofShape(at));
            }
            if cube.shape != shape {
                was.entry(ShapeKey(cube.shape))
                    .or_default()
                    .push(at.to_array());
                change.cubes.push((at, Some(Cube { shape, ..cube })));
            }
        }
        if was.is_empty() {
            return Err(Refusal::Unchanged);
        }
        change.done.undo = was
            .into_iter()
            .map(|(shape, cubes)| Op::SetShape {
                cubes,
                shape: shape.0,
            })
            .collect();
        Ok(change)
    }

    /// The cubes an operation marks with a change, with none marked on them yet.
    fn unmarked(&self, named: &[IVec3]) -> Result<Vec<(IVec3, Cube)>, Refusal> {
        named
            .iter()
            .map(|&at| {
                let cube = self.there(at)?;
                if cube.fill == Fill::Open {
                    return Err(Refusal::Opening(at));
                }
                if cube.pending != Pending::None {
                    return Err(Refusal::Pending(at));
                }
                Ok((at, cube))
            })
            .collect()
    }

    fn replace(
        &self,
        cubes: &[[i32; 3]],
        kind: Kind,
        part: Option<PartId>,
        cx: &Context,
    ) -> Result<Change, Refusal> {
        let new_part = self.new_part(part, kind, cx)?;
        let named = self.named(cubes, cx)?;
        let mut change = Change::default();
        change.parts.extend(new_part);
        for (at, cube) in self.unmarked(&named)? {
            if cube.fill != Fill::Solid {
                return Err(Refusal::Opening(at));
            }
            if (cube.kind, cube.part) == (kind, part) {
                return Err(Refusal::Unchanged);
            }
            if kind.is_roof() && !cube.shape.roofs() {
                return Err(Refusal::RoofShape(at));
            }
            let marked = Cube {
                pending: Pending::Replace,
                ..cube
            };
            change.cubes.push((at, Some(marked)));
            change.replacing.push((at, Some((kind, part))));
        }
        change.done.undo = vec![Op::Cancel {
            cubes: cubes.to_vec(),
        }];
        Ok(change)
    }

    fn swap(&self, cubes: &[[i32; 3]], cx: &Context) -> Result<Change, Refusal> {
        let named = self.named(cubes, cx)?;
        let mut change = Change::default();
        let mut placed = Vec::new();
        for (at, cube) in self.bare(&named)? {
            let Some((kind, part)) = self.replacing(at) else {
                return Err(Refusal::NotPending(at));
            };
            // Marked to change, it has changed, the new to go in; changed, it is marked to
            // change back.
            let pending = match cube.pending {
                Pending::Swapped => Pending::Replace,
                _ => Pending::Swapped,
            };
            let swapped = Cube {
                kind,
                part,
                pending,
                ..cube
            };
            placed.push((at, part));
            change.cubes.push((at, Some(swapped)));
            change.replacing.push((at, Some((cube.kind, cube.part))));
        }
        self.room_for_parts(&placed)?;
        change.done.undo = vec![Op::Swap {
            cubes: cubes.to_vec(),
        }];
        Ok(change)
    }

    fn carve(&self, cubes: &[[i32; 3]], cx: &Context) -> Result<Change, Refusal> {
        let named = self.named(cubes, cx)?;
        let mut change = Change::default();
        for (at, cube) in self.unmarked(&named)? {
            let marked = Cube {
                pending: Pending::Carve,
                ..cube
            };
            change.cubes.push((at, Some(marked)));
        }
        change.done.undo = vec![Op::Cancel {
            cubes: cubes.to_vec(),
        }];
        Ok(change)
    }

    fn cancel(&self, cubes: &[[i32; 3]], cx: &Context) -> Result<Change, Refusal> {
        let named = self.named(cubes, cx)?;
        let mut change = Change::default();
        let mut carved = Vec::new();
        let mut replaced: BTreeMap<Replacement, Vec<[i32; 3]>> = BTreeMap::new();
        let mut changed: BTreeMap<Replacement, Vec<[i32; 3]>> = BTreeMap::new();
        for &at in &named {
            let cube = self.there(at)?;
            match cube.pending {
                Pending::None => return Err(Refusal::NotPending(at)),
                // A cube part carved out or changed under what stays is finished first, as
                // nothing may be left part in under it.
                _ if cube.is_in() && !cube.is_built() && self.is_in(at + IVec3::Y) => {
                    return Err(Refusal::Covered(at + IVec3::Y));
                }
                Pending::Carve => carved.push(at.to_array()),
                Pending::Replace => {
                    let to = self.replacing(at).ok_or(Refusal::NotPending(at))?;
                    replaced.entry(to).or_default().push(at.to_array());
                }
                Pending::Swapped => {
                    let was = self.replacing(at).ok_or(Refusal::NotPending(at))?;
                    changed.entry(was).or_default().push(at.to_array());
                }
            }
            let cleared = Cube {
                pending: Pending::None,
                ..cube
            };
            change.cubes.push((at, Some(cleared)));
        }
        if !carved.is_empty() {
            change.done.undo.push(Op::Carve { cubes: carved });
        }
        for ((kind, part), cubes) in replaced {
            change.done.undo.push(Op::Replace { cubes, kind, part });
        }
        for ((kind, part), cubes) in changed {
            change.done.undo.push(Op::Changed { cubes, kind, part });
        }
        Ok(change)
    }

    /// Marks cubes as changed from another kind, the new going in: what clearing that mark
    /// undoes.
    fn changed(
        &self,
        cubes: &[[i32; 3]],
        kind: Kind,
        part: Option<PartId>,
        cx: &Context,
    ) -> Result<Change, Refusal> {
        let new_part = self.new_part(part, kind, cx)?;
        let named = self.named(cubes, cx)?;
        let mut change = Change::default();
        change.parts.extend(new_part);
        for (at, cube) in self.unmarked(&named)? {
            let marked = Cube {
                pending: Pending::Swapped,
                ..cube
            };
            change.cubes.push((at, Some(marked)));
            change.replacing.push((at, Some((kind, part))));
        }
        change.done.undo = vec![Op::Cancel {
            cubes: cubes.to_vec(),
        }];
        Ok(change)
    }

    fn fit_prop(&self, prop: &Prop, cx: &Context) -> Result<Change, Refusal> {
        let (low, high) = (IVec3::from_array(prop.min), IVec3::from_array(prop.max));
        if low.cmpgt(high).any() {
            return Err(Refusal::Malformed);
        }
        for at in prop.cubes() {
            claimed(at, cx)?;
            if self.cube(at).is_some_and(|cube| cube.fill != Fill::Open) {
                return Err(Refusal::Taken(at));
            }
            if let Some(other) = self.prop_in(at) {
                return Err(Refusal::PropThere(IVec3::from_array(other.min)));
            }
            if cx.world.in_the_way(at) {
                return Err(Refusal::InTheWay(at));
            }
        }
        self.hole_for(prop).ok_or(Refusal::NoHole)?;
        // A pair of doors or shutters is two of one.
        let took = cx.rules.prop(prop.kind).times(prop.leaf_count() as u64);
        if !cx.world.has(&took) {
            return Err(Refusal::Materials(took));
        }
        let mut change = Change::default();
        change.props.push((prop.min, Some((*prop, took.clone()))));
        change.done.took = took;
        change.done.undo = vec![Op::RemoveProp { at: prop.min }];
        Ok(change)
    }

    fn remove_prop(&self, at: IVec3, cx: &Context) -> Result<Change, Refusal> {
        let prop = self.prop(at).ok_or(Refusal::NoProp(at))?;
        claimed(at, cx)?;
        let mut change = Change::default();
        change.props.push((prop.min, None));
        // What went in, not what fitting one costs now (gap 25).
        change.done.gave = self.prop_paid.get(&prop.min).cloned().unwrap_or_default();
        change.done.undo = vec![Op::FitProp(*prop)];
        Ok(change)
    }

    fn set_open(&self, at: IVec3, open: bool, cx: &Context) -> Result<Change, Refusal> {
        let prop = self.prop(at).ok_or(Refusal::NoProp(at))?;
        claimed(at, cx)?;
        if prop.open == open {
            return Err(Refusal::Unchanged);
        }
        // Shut on someone standing in it, it would close round them, as building where they
        // stand would (construction-001, gap 17).
        if !open && let Some(at) = prop.cubes().find(|&at| cx.world.in_the_way(at)) {
            return Err(Refusal::InTheWay(at));
        }
        let mut change = Change::default();
        let paid = self.prop_paid.get(&prop.min).cloned().unwrap_or_default();
        change
            .props
            .push((prop.min, Some((Prop { open, ..*prop }, paid))));
        change.done.undo = vec![Op::SetOpen {
            at: prop.min,
            open: !open,
        }];
        Ok(change)
    }

    fn open(&self, cubes: &[[i32; 3]], cx: &Context) -> Result<Change, Refusal> {
        let named = self.named(cubes, cx)?;
        let mut change = Change::default();
        let mut carved = Vec::new();
        for &at in &named {
            let cube = self.there(at)?;
            if cube.fill != Fill::Solid {
                return Err(Refusal::Opening(at));
            }
            if cube.level() > 0 {
                return Err(Refusal::InProgress(at));
            }
            match cube.pending {
                Pending::Replace | Pending::Swapped => return Err(Refusal::Pending(at)),
                Pending::Carve => carved.push(at.to_array()),
                Pending::None => {}
            }
            let opening = Cube {
                fill: Fill::Open,
                shape: Shape::Whole,
                state: crate::State::Built,
                pending: Pending::None,
                ..cube
            };
            change.cubes.push((at, Some(opening)));
        }
        // Closed again as it was: its shape, and the carving marked.
        let mut shapes: BTreeMap<ShapeKey, Vec<[i32; 3]>> = BTreeMap::new();
        for &at in &named {
            let shape = self.cube(at).map_or(Shape::Whole, |cube| cube.shape);
            shapes
                .entry(ShapeKey(shape))
                .or_default()
                .push(at.to_array());
        }
        change.done.undo.push(Op::Close {
            cubes: cubes.to_vec(),
        });
        for (shape, cubes) in shapes {
            if shape.0 != Shape::Whole {
                change.done.undo.push(Op::SetShape {
                    cubes,
                    shape: shape.0,
                });
            }
        }
        if !carved.is_empty() {
            change.done.undo.push(Op::Carve { cubes: carved });
        }
        Ok(change)
    }

    fn close(&self, cubes: &[[i32; 3]], cx: &Context) -> Result<Change, Refusal> {
        let named = self.named(cubes, cx)?;
        if let Some(claim) = cx.claim
            && self.cubes.planned() + named.len() > claim.budget
        {
            return Err(Refusal::OverBudget {
                budget: claim.budget,
            });
        }
        let mut change = Change::default();
        for &at in &named {
            let cube = self.there(at)?;
            if cube.fill != Fill::Open {
                return Err(Refusal::NotOpen(at));
            }
            if let Some(prop) = self.prop_in(at) {
                return Err(Refusal::PropThere(IVec3::from_array(prop.min)));
            }
            let planned = Cube {
                fill: Fill::Solid,
                ..Cube::planned(cube.kind, cube.part)
            };
            change.cubes.push((at, Some(planned)));
        }
        change.done.undo = vec![Op::Open {
            cubes: cubes.to_vec(),
        }];
        Ok(change)
    }

    fn resample(&self, samples: &[Sample], cx: &Context) -> Result<Change, Refusal> {
        let columns: BTreeSet<[i32; 2]> = samples.iter().map(|sample| sample.column).collect();
        if columns.is_empty() || columns.len() != samples.len() {
            return Err(Refusal::Malformed);
        }
        let mut ground = self.ground.clone();
        let mut change = Change::default();
        let mut was = Vec::new();
        for sample in samples {
            let [x, z] = sample.column;
            claimed(IVec3::new(x, 0, z), cx)?;
            // Only where something stands is the ground kept.
            if !self.occupied(x, z) {
                continue;
            }
            was.push(Sample {
                column: sample.column,
                height_cm: self.ground.height_cm(sample.column),
            });
            ground.set(sample.column, Some(sample.height_cm));
            change.ground.push(*sample);
        }
        if change.ground.is_empty() {
            return Err(Refusal::Unchanged);
        }
        if let Some(&at) = self.floating_on(&ground).first() {
            return Err(Refusal::Undermines(at));
        }
        change.done.undo = vec![Op::Ground { samples: was }];
        Ok(change)
    }
}

/// A cube's place inside the claim, if there is one.
fn claimed(at: IVec3, cx: &Context) -> Result<(), Refusal> {
    match cx.claim {
        Some(claim) if !claim.covers([at.x, at.z]) => Err(Refusal::OutsideClaim(at)),
        _ => Ok(()),
    }
}

/// A shape, ordered, to group cubes by.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ShapeKey(Shape);

impl ShapeKey {
    fn rank(self) -> u8 {
        self.0.code()
    }
}

impl PartialOrd for ShapeKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ShapeKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

#[cfg(test)]
mod tests;
