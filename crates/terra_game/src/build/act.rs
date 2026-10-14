//! Acting on the player's base (docs/building-plan.md, step 4; D139, D193): everything the
//! hammer, a script or the remote does to it is a `terra_build` operation, checked against
//! the base as it is, with the claim and its budget of planned cubes, who stands where, what
//! rests on what, and what the builder carries. Applied, it takes from the builder's pack
//! what it took and gives back what it gave, exactly; a refusal is said to them in the words
//! the operation gives, which are the words the ghost shows.
//!
//! What goes in costs fractions of an item, a sixteenth of a Wood for a course of log in a
//! cube. A whole item is taken when a fraction is first needed and what is left of it kept
//! as the claim's offcuts ([`Offcuts`]), which later steps use first; what is given back
//! adds to them, and comes back to the pack as it makes whole items. So what is in the
//! pack, the offcuts and the base together never changes by building and taking apart.
//!
//! Every act is logged as `Build: <what>: done` or `Build: <what>: refused: <why>`, so a
//! script can wait for it.

use super::Site;
use crate::planet::Planet;
use crate::play::Player;
use crate::play::bodies::Body;
use crate::play::claim::Claim;
use crate::play::inventory::Belongings;
use crate::play::props::Prop as Item;
use crate::play::told::Told;
use crate::shell::Options;
use bevy::{ecs::system::SystemParam, prelude::*};
use std::collections::{BTreeMap, BTreeSet};
use terra_build::ops::{self, Context, Done, Op, Refusal};
use terra_build::{Amount, Bill, CELL_M, Kind, PartId, RoofId, Rules, Sample, Walker};

/// How long after doing something to the base it can be undone, in seconds (D144).
pub const UNDO_S: f32 = 60.0;
/// How far from the builder the next part to build is looked for, in metres.
const NEAR_WORK_M: f32 = 14.0;
/// How many of a block kind's cubes, nearest first, are tried for the one to go in next.
const BLOCKS_TRIED: usize = 24;

/// What building costs and how each kind goes in, from the kits and the catalogue.
#[derive(Resource, Default, Clone)]
pub struct BuildRules {
    pub rules: Rules,
    /// The kinds that go in a cube at a time, as blocks do (D194).
    pub blocks: BTreeSet<Kind>,
    /// The block kinds a floor of stands on footings of its own kind: not a framed kind's,
    /// whose floor stands on stone (D236).
    pub footed: BTreeSet<Kind>,
    /// Each kind's name, as its kit's file is called: "logs", "stone".
    pub names: BTreeMap<Kind, String>,
    /// How many cubes a claim may have planned and not built, by how many times it has
    /// grown (D193).
    pub budgets: Vec<usize>,
    /// The kind footings are laid in under what is built on the ground (D194), if any.
    pub footing: Option<Kind>,
}

impl BuildRules {
    pub fn name(&self, kind: Kind) -> &str {
        self.names.get(&kind).map_or("plain", String::as_str)
    }

    /// The kind called a name, or numbered so.
    pub fn kind(&self, name: &str) -> Option<Kind> {
        self.names
            .iter()
            .find(|(_, each)| each.as_str() == name)
            .map(|(&kind, _)| kind)
            .or_else(|| name.parse().ok().map(Kind))
    }
}

/// What is left of items cut into for building: the claim's, used first by the next step
/// and given back as it makes whole items.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct Offcuts(pub Bill);

/// The changes a player made to the base, newest last, each with what undoes it.
#[derive(Component, Default)]
pub struct History(pub Vec<Undo>);

pub struct Undo {
    pub what: String,
    pub ops: Vec<Op>,
    /// When it was done, in seconds of the game's clock.
    pub at_s: f32,
}

/// Something a builder could do next to the base, and whether it would be allowed.
#[derive(Clone, Debug)]
pub struct Work {
    pub what: String,
    pub ops: Vec<Op>,
    pub check: Result<Done, Refusal>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<BuildRules>()
        .init_resource::<Offcuts>()
        .register_required_components::<Player, History>();
}

/// The world round the base, as an operation is checked against it.
struct Around {
    bodies: BTreeSet<[i32; 3]>,
    items: BTreeSet<[i32; 3]>,
    /// What the builder has to hand, by item name; `None` has everything, as on the yard.
    stock: Option<BTreeMap<String, Amount>>,
}

impl ops::World for Around {
    fn in_the_way(&self, at: IVec3) -> bool {
        self.bodies.contains(&at.to_array())
    }

    fn holds_item(&self, at: IVec3) -> bool {
        self.items.contains(&at.to_array())
    }

    fn has(&self, bill: &Bill) -> bool {
        let Some(stock) = &self.stock else {
            return true;
        };
        bill.0
            .iter()
            .all(|(name, &amount)| stock.get(name).copied().unwrap_or(Amount::ZERO) >= amount)
    }
}

/// Everything acting on the player's base needs.
#[derive(SystemParam)]
pub struct Building<'w, 's> {
    pub site: ResMut<'w, Site>,
    pub rules: Res<'w, BuildRules>,
    pub offcuts: ResMut<'w, Offcuts>,
    pub planet: ResMut<'w, Planet>,
    pub belongings: Belongings<'w, 's>,
    options: Res<'w, Options>,
    time: Res<'w, Time>,
    claims: Query<'w, 's, &'static Claim>,
    bodies: Query<'w, 's, (&'static Transform, &'static Body), Without<Player>>,
    walkers: Query<'w, 's, &'static Transform, With<Player>>,
    items: Query<'w, 's, &'static Item>,
    histories: Query<'w, 's, &'static mut History>,
    told: MessageWriter<'w, Told>,
}

impl Building<'_, '_> {
    /// Whether building costs nothing and every place is claimed, as on the yard.
    pub fn free(&self) -> bool {
        self.options.yard()
    }

    /// Where a player stands, in the world.
    pub fn player_at(&self, player: Entity) -> Option<Vec3> {
        self.walkers.get(player).ok().map(|at| at.translation)
    }

    /// The base's own space from a place in the world.
    pub fn local(&self, point: Vec3) -> Vec3 {
        self.site
            .frame
            .compute_affine()
            .inverse()
            .transform_point3(point)
    }

    /// The claim a player builds in, as the operations see it: the columns of the base it
    /// covers, a square round its stone, and its budget of planned cubes. On the yard,
    /// none; with no claim stone, one that covers nothing.
    fn claim_of(&self, by: Option<Entity>) -> Option<ops::Claim> {
        if self.free() {
            return None;
        }
        let nothing = ops::Claim {
            min: [1, 1],
            max: [0, 0],
            budget: 0,
        };
        let Some(claim) = by.and_then(|by| self.claims.get(by).ok()) else {
            return Some(nothing);
        };
        let Some((_, centre)) = claim.stone else {
            return Some(nothing);
        };
        let middle = self.local(centre);
        let reach = claim.across_m() * 0.5 / CELL_M;
        let budget = self
            .rules
            .budgets
            .get(claim.step())
            .or(self.rules.budgets.last())
            .copied()
            .unwrap_or(usize::MAX);
        Some(ops::Claim {
            min: [
                (middle.x / CELL_M - reach).floor() as i32,
                (middle.z / CELL_M - reach).floor() as i32,
            ],
            max: [
                (middle.x / CELL_M + reach).floor() as i32,
                (middle.z / CELL_M + reach).floor() as i32,
            ],
            budget,
        })
    }

    /// What surrounds the base for a builder: the cubes bodies stand in, those items stand
    /// on, and what the builder has, with the offcuts.
    fn around(&mut self, by: Option<Entity>) -> Around {
        let to_base = self.site.frame.compute_affine().inverse();
        let mut bodies = BTreeSet::new();
        let walker = Walker::default();
        let shapes = self
            .bodies
            .iter()
            .map(|(at, body)| (at.translation, body.radius))
            .chain(
                self.walkers
                    .iter()
                    .map(|at| (at.translation, walker.radius)),
            );
        for (at, radius) in shapes {
            let local = to_base.transform_point3(at);
            let low = ((local - Vec3::new(radius, 0.0, radius)) / CELL_M)
                .floor()
                .as_ivec3();
            let high = ((local + Vec3::new(radius, walker.height, radius)) / CELL_M)
                .floor()
                .as_ivec3();
            // Far from the base, nobody is in its way.
            if low.length_squared() > 1_000_000 {
                continue;
            }
            for x in low.x..=high.x {
                for y in low.y..=high.y {
                    for z in low.z..=high.z {
                        bodies.insert([x, y, z]);
                    }
                }
            }
        }
        // An item stands on the cube under its foot.
        let items = self
            .items
            .iter()
            .map(|item| {
                let foot = item.at + Vec3::Y * (item.lift - 0.05);
                (foot / CELL_M).floor().as_ivec3().to_array()
            })
            .collect();
        let stock = match by {
            _ if self.free() => None,
            None => None,
            Some(by) => {
                let names: BTreeSet<String> = self
                    .rules
                    .rules
                    .kinds
                    .values()
                    .flat_map(|terms| terms.per_step.0.keys())
                    .chain(
                        self.rules
                            .rules
                            .roofs
                            .values()
                            .flat_map(|bill| bill.0.keys()),
                    )
                    .chain(
                        self.rules
                            .rules
                            .props
                            .values()
                            .flat_map(|bill| bill.0.keys()),
                    )
                    .cloned()
                    .collect();
                let offcuts = self.offcuts.0.clone();
                let inventory = self.belongings.of(by);
                Some(
                    names
                        .into_iter()
                        .map(|name| {
                            let carried = inventory.as_ref().map_or(0, |inventory| {
                                inventory
                                    .items()
                                    .find(&name)
                                    .map_or(0, |kind| inventory.count(kind))
                            });
                            let cut = offcuts.0.get(&name).copied().unwrap_or(Amount::ZERO);
                            (name, Amount::whole(u64::from(carried)) + cut)
                        })
                        .collect(),
                )
            }
        };
        Around {
            bodies,
            items,
            stock,
        }
    }

    /// Whether operations would be allowed, as one act, and what they would take and give
    /// back; nothing changes.
    pub fn check(&mut self, by: Option<Entity>, ops: &[Op]) -> Result<Done, Refusal> {
        let around = self.around(by);
        let claim = self.claim_of(by);
        let cx = Context {
            rules: &self.rules.rules,
            claim: claim.as_ref(),
            world: &around,
        };
        match ops {
            [op] => self.site.base.check(op, &cx),
            ops => self.site.base.clone().act_all(ops, &cx),
        }
    }

    /// Does operations to the base as one act, taking and giving back what they say, and
    /// keeping what undoes them. A refusal is told to the builder. Logged either way.
    pub fn act(&mut self, by: Option<Entity>, what: &str, ops: &[Op]) -> Result<Done, Refusal> {
        let done = self.apply(by, ops);
        match &done {
            Ok(done) => {
                info!(
                    "Build: {what}: done{}{}",
                    describe(" taking ", &done.took),
                    describe(" giving back ", &done.gave)
                );
                if let Some(by) = by
                    && let Ok(mut history) = self.histories.get_mut(by)
                {
                    history.0.push(Undo {
                        what: what.to_string(),
                        ops: done.undo.clone(),
                        at_s: self.time.elapsed_secs(),
                    });
                    let keep = history.0.len().saturating_sub(64);
                    history.0.drain(..keep);
                }
            }
            Err(why) => {
                info!("Build: {what}: refused: {}", why.message());
                if let Some(by) = by {
                    self.told.write(Told::new(by, why.message()));
                }
            }
        }
        done
    }

    /// Undoes the last change a player made, within a while: its inverse, checked like
    /// anything else, so it is refused if the base has moved on under it.
    pub fn undo(&mut self, by: Entity) -> Result<String, String> {
        let now = self.time.elapsed_secs();
        let last = self
            .histories
            .get_mut(by)
            .ok()
            .and_then(|mut history| history.0.pop());
        let Some(last) = last else {
            return Err("Nothing to undo".to_string());
        };
        if now - last.at_s > UNDO_S {
            return Err("Too long ago to undo".to_string());
        }
        let what = format!("undo {}", last.what);
        match self.apply(Some(by), &last.ops) {
            Ok(done) => {
                info!(
                    "Build: {what}: done{}{}",
                    describe(" taking ", &done.took),
                    describe(" giving back ", &done.gave)
                );
                Ok(what)
            }
            Err(why) => {
                info!("Build: {what}: refused: {}", why.message());
                Err(why.message())
            }
        }
    }

    /// When the last change a player made to the base was, if they made one.
    pub fn last_change_s(&self, by: Entity) -> Option<f32> {
        self.histories
            .get(by)
            .ok()
            .and_then(|history| history.0.last())
            .map(|undo| undo.at_s)
    }

    fn apply(&mut self, by: Option<Entity>, ops: &[Op]) -> Result<Done, Refusal> {
        let around = self.around(by);
        let claim = self.claim_of(by);
        let cx = Context {
            rules: &self.rules.rules,
            claim: claim.as_ref(),
            world: &around,
        };
        let done = self.site.act(ops, &cx)?;
        if let Some(by) = by
            && around.stock.is_some()
        {
            self.settle(by, &done);
        }
        Ok(done)
    }

    /// Takes what an act took from the offcuts and the pack, cutting into whole items only
    /// as far as it must, and gives back what it gave, whole items to the pack.
    fn settle(&mut self, by: Entity, done: &Done) {
        let Some(mut inventory) = self.belongings.of(by) else {
            return;
        };
        let offcuts = &mut self.offcuts.0;
        for (name, &amount) in &done.took.0 {
            let Some(kind) = inventory.items().find(name) else {
                continue;
            };
            let have = offcuts.0.get(name).copied().unwrap_or(Amount::ZERO);
            let left = match have.minus(amount) {
                Some(left) => left,
                None => {
                    let short = amount.minus(have).unwrap_or(Amount::ZERO);
                    let (num, den) = short.parts();
                    let whole = num.div_ceil(den);
                    inventory.spend(kind, whole as u32);
                    (Amount::whole(whole) + have)
                        .minus(amount)
                        .unwrap_or(Amount::ZERO)
                }
            };
            set(offcuts, name, left);
        }
        for (name, &amount) in &done.gave.0 {
            let Some(kind) = inventory.items().find(name) else {
                continue;
            };
            let have = offcuts.0.get(name).copied().unwrap_or(Amount::ZERO) + amount;
            let (num, den) = have.parts();
            let whole = num / den;
            if whole > 0 {
                inventory.give(kind, whole as u32);
            }
            set(
                offcuts,
                name,
                have.minus(Amount::whole(whole)).unwrap_or(Amount::ZERO),
            );
        }
    }

    /// The ground's height under columns of the base, sampled from the planet.
    pub fn ground_under(&self, columns: impl IntoIterator<Item = [i32; 2]>) -> Vec<Sample> {
        let mut seen = BTreeSet::new();
        columns
            .into_iter()
            .filter(|column| seen.insert(*column))
            .map(|column| {
                let middle = Vec3::new(
                    (column[0] as f32 + 0.5) * CELL_M,
                    0.0,
                    (column[1] as f32 + 0.5) * CELL_M,
                );
                let height = -self
                    .planet
                    .distance(self.site.frame.transform_point(middle));
                Sample::metres(column, height)
            })
            .collect()
    }

    /// Says something to a player, as a notice.
    pub fn tell(&mut self, player: Entity, text: impl Into<String>) {
        self.told.write(Told::new(player, text));
    }

    /// The ground's height under a column of the base, in centimetres in its own space, as
    /// a footing down to it is laid: the lowest under the column, a little lower.
    pub fn ground_cm(&self, column: [i32; 2]) -> i16 {
        super::ground_cm(&self.site.frame, &self.planet, column)
    }

    /// Plans with the ground under their cubes sampled into them (construction-001, gap 3).
    pub fn with_ground(&self, ops: Vec<Op>) -> Vec<Op> {
        ops.into_iter()
            .map(|op| match op {
                Op::LayPlan {
                    part, kind, cubes, ..
                } => {
                    let ground =
                        self.ground_under(cubes.iter().map(|cube| [cube.at[0], cube.at[2]]));
                    Op::LayPlan {
                        part,
                        kind,
                        cubes,
                        ground,
                    }
                }
                op => op,
            })
            .collect()
    }

    /// Lays plans at once, for nothing: for a room set into the ground (a block of tiles),
    /// the ground is cut away where it stands first (D131, D193), the plans take the ground
    /// as it is then, and what stood there already has its ground sampled again.
    pub fn lay(
        &mut self,
        by: Option<Entity>,
        what: &str,
        ops: Vec<Op>,
        room: Option<(IVec3, IVec3)>,
    ) -> Result<Done, Refusal> {
        let Some((min, max)) = room else {
            let ops = self.with_ground(ops);
            return self.act(by, what, &ops);
        };
        // Refused as it is, the ground is left alone.
        let first = self.with_ground(ops.clone());
        if self.check(by, &first).is_err() {
            return self.act(by, what, &first);
        }
        let cut = self.site.cut_for_room(min, max);
        self.planet.apply(&cut);
        let ops = self.with_ground(ops);
        let done = self.act(by, what, &ops)?;
        self.resample(
            by,
            IVec2::new(min.x - 1, min.z - 1),
            IVec2::new(max.x + 1, max.z + 1),
        );
        Ok(done)
    }

    /// Samples the ground again under the base's columns in a block, as after the ground
    /// there was dug: refused, and the old samples kept, if that would undermine what is
    /// built, which is said in the log.
    pub fn resample(&mut self, by: Option<Entity>, low: IVec2, high: IVec2) {
        let columns: Vec<[i32; 2]> = (low.x..=high.x)
            .flat_map(|x| (low.y..=high.y).map(move |z| [x, z]))
            .filter(|&[x, z]| self.site.base.occupied(x, z))
            .collect();
        let samples: Vec<Sample> = self
            .ground_under(columns)
            .into_iter()
            .filter(|sample| {
                self.site.base.ground().sample(sample.column) != Some(sample.height_cm)
            })
            .collect();
        if !samples.is_empty() {
            let _ = self.act(by, "sample the ground again", &[Op::Ground { samples }]);
        }
    }

    /// Whether a kind goes in a cube at a time.
    fn blocks(&self, kind: Kind) -> bool {
        self.rules.blocks.contains(&kind)
    }

    /// What building would put in next, for a builder at `near` (the base's own space):
    /// whatever of the base near them can go in, lowest first and the part aimed at, then
    /// the part laid first, first on its layer, and roofs once what holds them is up. The first allowed, or, if none
    /// is, the first with why it is refused.
    pub fn next_work(
        &mut self,
        by: Option<Entity>,
        aimed: Option<PartId>,
        near: Vec3,
    ) -> Option<Work> {
        let base = &self.site.base;
        let mut parts: Vec<(i32, f32, PartId)> = base
            .parts()
            .filter_map(|(id, part)| {
                let cubes = base.next_in(id, near, self.blocks(part.kind))?;
                let at = IVec3::from_array(cubes[0]);
                let distance = ((at.as_vec3() + 0.5) * CELL_M).distance(near);
                (distance <= NEAR_WORK_M || Some(id) == aimed).then_some((at.y, distance, id))
            })
            .collect();
        // Lowest first, as what a part rests on goes in before it; on a layer, the part
        // aimed at, then the part laid first, as a plan lays what holds the rest up first,
        // its footing before its floor (D194), then the nearest.
        parts.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then((Some(b.2) == aimed).cmp(&(Some(a.2) == aimed)))
                .then(a.2.cmp(&b.2))
                .then(a.1.total_cmp(&b.1))
        });
        let roofs: Vec<RoofId> = base
            .roofs()
            .filter(|(id, _)| base.next_stage(*id).is_some())
            .map(|(id, _)| id)
            .collect();
        let mut candidates: Vec<(String, Vec<Op>)> = Vec::new();
        // An opening being cut into what is built first, as it was asked for last.
        if let Some(op) = base.next_carve(near, |kind| self.blocks(kind)) {
            let what = match &op {
                Op::Open { cubes } => format!("open {} carved cube{}", cubes.len(), plural(cubes)),
                Op::Empty { cubes } => format!(
                    "carve {} cube{} out ({} up)",
                    cubes.len(),
                    plural(cubes),
                    cubes[0][1]
                ),
                _ => "carve".to_string(),
            };
            candidates.push((what, vec![op]));
        }
        // Cubes changing to another kind (D232): the old out, the swap, the new in.
        if let Some(op) = base.next_change(near, |kind| self.blocks(kind)) {
            let name = |cubes: &[[i32; 3]]| {
                let kind = base.cube(IVec3::from_array(cubes[0])).map(|cube| cube.kind);
                kind.map_or("plain", |kind| self.rules.name(kind))
                    .to_string()
            };
            let what = match &op {
                Op::Empty { cubes } => format!("take {} out to change it", name(cubes)),
                Op::Swap { cubes } => format!("change {} cube{}", cubes.len(), plural(cubes)),
                Op::Fill { cubes } => format!("put {} in where it changed", name(cubes)),
                Op::Cancel { cubes } => {
                    format!("finish changing {} cube{}", cubes.len(), plural(cubes))
                }
                _ => "change".to_string(),
            };
            candidates.push((what, vec![op]));
        }
        for (_, _, id) in parts {
            let Some(part) = self.site.base.part(id) else {
                continue;
            };
            let kind = part.kind;
            let layer = self.site.base.next_in(id, near, false).unwrap_or_default();
            // A block kind goes in a cube at a time, the nearest that can (D194); a long
            // member across the whole layer.
            let goes_in: Vec<Vec<[i32; 3]>> = if self.blocks(kind) {
                let middle = |at: &[i32; 3]| {
                    ((IVec3::from_array(*at).as_vec3() + 0.5) * CELL_M).distance_squared(near)
                };
                let mut nearest = layer;
                nearest.sort_by(|a, b| middle(a).total_cmp(&middle(b)));
                nearest
                    .into_iter()
                    .take(BLOCKS_TRIED)
                    .map(|at| vec![at])
                    .collect()
            } else {
                vec![layer]
            };
            for cubes in goes_in {
                let what = format!(
                    "put {} into part {} ({} cube{}, {} up)",
                    self.rules.name(kind),
                    id.0,
                    cubes.len(),
                    if cubes.len() == 1 { "" } else { "s" },
                    cubes[0][1]
                );
                candidates.push((what, vec![Op::Fill { cubes }]));
            }
        }
        for roof in roofs {
            let cells = self.site.base.next_stage(roof).unwrap_or_default();
            let stage = self
                .site
                .base
                .roof(roof)
                .and_then(|each| each.stage(cells[0]))
                .unwrap_or(0);
            let what = format!(
                "put roof {} up to its {}",
                roof.0,
                ["frame", "battens", "covering"][usize::from(stage.min(2))]
            );
            candidates.push((
                what,
                vec![Op::RoofStage {
                    roof,
                    cells,
                    up: true,
                }],
            ));
        }
        let mut first: Option<Work> = None;
        for (what, ops) in candidates {
            let check = self.check(by, &ops);
            let work = Work { what, ops, check };
            if work.check.is_ok() {
                return Some(work);
            }
            first.get_or_insert(work);
        }
        first
    }

    /// What taking apart the part a builder aims at would take out first: the last of it
    /// put in, or, with nothing of it in, its whole plan, for nothing; a roof standing on
    /// it comes down first.
    pub fn last_work(&mut self, by: Option<Entity>, cube: IVec3, near: Vec3) -> Option<Work> {
        let base = &self.site.base;
        let found = base.cube(cube)?;
        let column = [cube.x, cube.z];
        let mut ops = Vec::new();
        let what;
        if let Some(roof) = base.roof_over(column)
            && let Some(cells) = base.last_stage(roof)
        {
            what = format!("take roof {} down a stage", roof.0);
            ops.push(Op::RoofStage {
                roof,
                cells,
                up: false,
            });
        } else if let Some(part) = found.part {
            let kind = base.part(part).map_or(found.kind, |part| part.kind);
            match base.last_in(part, near, self.blocks(kind)) {
                Some(cubes) => {
                    what = format!(
                        "take {} out of part {} ({} cube{})",
                        self.rules.name(kind),
                        part.0,
                        cubes.len(),
                        if cubes.len() == 1 { "" } else { "s" }
                    );
                    ops.push(Op::Empty { cubes });
                }
                None => {
                    what = format!("take away the plan of part {}", part.0);
                    ops.push(Op::Unplan {
                        cubes: base.plan_of(part),
                    });
                    // A roof planned over it goes with it.
                    if let Some(roof) = base.roof_over(column) {
                        ops.push(Op::RemoveRoof { roof });
                    }
                }
            }
        } else {
            // Built by a shortcut, before plans: a cube at a time, from the top.
            let top = base
                .column(cube.x, cube.z)
                .filter(|&y| base.is_in(IVec3::new(cube.x, y, cube.z)))
                .max()?;
            what = "take out a cube".to_string();
            ops.push(Op::Empty {
                cubes: vec![[cube.x, top, cube.z]],
            });
        }
        let check = self.check(by, &ops);
        Some(Work { what, ops, check })
    }
}

/// An "s" for more than one.
fn plural<T>(items: &[T]) -> &'static str {
    if items.len() == 1 { "" } else { "s" }
}

/// Sets what is left of an item in the offcuts, none at all when nothing is.
fn set(offcuts: &mut Bill, name: &str, amount: Amount) {
    if amount.is_zero() {
        offcuts.0.remove(name);
    } else {
        offcuts.0.insert(name.to_string(), amount);
    }
}

fn describe(lead: &str, bill: &Bill) -> String {
    if bill.is_empty() {
        String::new()
    } else {
        format!("{lead}{bill}")
    }
}
