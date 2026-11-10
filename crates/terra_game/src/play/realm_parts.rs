//! A realm's built parts, stood by rule (D642, docs/realm-two.md): what its row of
//! `data/realms.ron` says stands on its land (the industrial realm's pylons, vent stacks,
//! tanks, generators and signs), each a catalogue item, scattered by the world's seed: a cell
//! of the ground holds one or none by the seed, which one by their shares, where the ground
//! is dry and level enough, clear of the start and of the gate. They are
//! the realm's, not a player's: never saved, the same on every entering.
//!
//! They stand round the player and follow them (D127): a cell is taken up as it comes within
//! reach and let go past it, so only the ring that changed is worked on. The cells are laid on
//! the start's level, as `Start::at` gives places, which holds for the few kilometres a
//! player walks from it and no further than the planet's curve allows.

use super::catalogue::Builds;
use super::portal::CLEAR_M;
use super::portal::gates::Gate;
use super::props::{self, Models, Theirs};
use crate::build::Site;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use terra_gen::hash::{mix3, unit3};
use terra_walker::walker::Player;
use terra_world::flora::{ClearAround, KeptClear};
use terra_world::instance::{InstanceApp, WorldSpec};
use terra_world::planet::Planet;
use terra_world::realm::Realms;
use terra_world::start::Start;

/// The side of a cell, which holds one part at most, in metres, and how many cells round the
/// player's the parts stand in: 520 m, past where the haze takes a part this size. A realm's
/// parts are at most 10,000 a square kilometre in all, one a cell: at 20 m a cell, 2,500, the
/// far realm's forest stood a tenth as close as the first realm's and read as a meadow.
const CELL_M: f32 = 10.0;
const REACH_CELLS: i32 = 52;
/// A cell is let go this many cells past the reach, so a player pacing a cell's edge does
/// not stand and fell the same ring.
const SLACK_CELLS: i32 = 4;
/// A part is stood this far past where it is first drawn from, so it is there before it
/// fades in.
const STOOD_PAST_M: f32 = 40.0;
/// No part within this of the start, where a player arrives.
const CLEAR_OF_START_M: f32 = 25.0;
/// The ground may rise or fall this much across a part's footing, in metres.
const LEVEL_M: f32 = 0.6;

/// How often the parts' showing is looked at, in frames.
const SHOWN_EVERY: u32 = 20;

/// How far off a part is drawn from, metres: its row's `seen_m` (`data/realms.ron`), or as
/// far as the parts stand. Some two hundred parts drawn and shadowed to 510 m, whatever
/// their size, were a cost nobody saw.
#[derive(Component)]
struct SeenFrom(f32);

/// What a part is: a catalogue item's model, or a grown kind and how tall it stands.
enum What<'a> {
    Model(std::sync::Arc<super::catalogue::Item>),
    Grown(&'a str, (f32, f32)),
}

/// A part stood: its entity, and the ground it keeps clear of plants while it stands.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Part {
    entity: Entity,
    ground: Vec3,
    clear_m: f32,
    /// Whether it stands among the walkers' obstacles: a grown part's stalk does (a model's
    /// own solid goes with its entity).
    solid: bool,
}

/// Made afresh in each world: the cells its parts have been stood in, each with its part if
/// it holds one that could stand, round the cell the player was last in.
#[derive(Resource, Default)]
struct Stood {
    middle: Option<IVec2>,
    cells: HashMap<IVec2, Option<Part>>,
}

impl Stood {
    /// The player is in the cell `middle`: the cells come within reach, to be taken up, and
    /// the parts of those gone past it, let go. Nothing if they have not left their cell.
    fn moved(&mut self, middle: IVec2) -> (Vec<IVec2>, Vec<Part>) {
        if self.middle.replace(middle) == Some(middle) {
            return (Vec::new(), Vec::new());
        }
        let within = |cell: IVec2, cells: i32| (cell - middle).length_squared() <= cells * cells;
        let mut gone = Vec::new();
        self.cells.retain(|cell, part| {
            let keep = within(*cell, REACH_CELLS + SLACK_CELLS);
            if !keep {
                gone.extend(part.take());
            }
            keep
        });
        let mut come = Vec::new();
        for row in -REACH_CELLS..=REACH_CELLS {
            for column in -REACH_CELLS..=REACH_CELLS {
                let cell = middle + IVec2::new(column, row);
                if within(cell, REACH_CELLS) && !self.cells.contains_key(&cell) {
                    self.cells.insert(cell, None);
                    come.push(cell);
                }
            }
        }
        (come, gone)
    }
}

pub fn plugin(app: &mut App) {
    app.init_world_resource::<Stood>().add_systems(
        Update,
        (stand.run_if(resource_exists::<Site>), show).run_if(terra_world::instance::in_world),
    );
}

/// Hides each part past where it is drawn from and shows it within, a look every few frames.
fn show(
    mut since: Local<u32>,
    player: Query<&GlobalTransform, With<Player>>,
    mut parts: Query<(&SeenFrom, &GlobalTransform, &mut Visibility)>,
) {
    *since += 1;
    if *since < SHOWN_EVERY {
        return;
    }
    *since = 0;
    let Some(eye) = player.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    for (seen, at, mut visibility) in &mut parts {
        let shown = if at.translation().distance(eye) <= seen.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        visibility.set_if_neq(shown);
    }
}

/// What a cell of the ground holds, by the seed: which part (an index into `shares`), where
/// in metres east and south of the start, and how turned; or nothing. `shares` are how many
/// of each part a square kilometre holds.
pub fn cell(seed: u32, shares: &[f32], cell: IVec2) -> Option<(usize, Vec2, f32)> {
    let total: f32 = shares.iter().sum();
    let (c, r) = (cell.x as u32, cell.y as u32);
    // The chance a cell holds one: the parts a square kilometre holds over its cells.
    if unit3(seed, c, r, 0) >= (total * CELL_M * CELL_M / 1.0e6).min(1.0) {
        return None;
    }
    let at = Vec2::new(
        (cell.x as f32 + unit3(seed, c, r, 1)) * CELL_M,
        (cell.y as f32 + unit3(seed, c, r, 2)) * CELL_M,
    );
    if at.length() < CLEAR_OF_START_M {
        return None;
    }
    // Which part: by its share of them all.
    let mut pick = unit3(seed, c, r, 3) * total;
    let part = shares
        .iter()
        .position(|share| {
            pick -= share;
            pick < 0.0
        })
        .unwrap_or(shares.len() - 1);
    Some((part, at, (mix3(seed, c, r, 4) % 360) as f32))
}

/// What each of a realm's parts is, with how many a square kilometre holds and from how far
/// it is drawn: a catalogue item's model, or a grown kind this world has. One the data names
/// that is neither is said and left out.
fn parts_of<'a>(
    realm: &'a terra_world::realm::Realm,
    builds: &Builds,
    stock: Option<&terra_world::flora::Stock>,
) -> Vec<(What<'a>, f32, f32)> {
    realm
        .parts
        .iter()
        .filter_map(|part| {
            let seen_m = part.seen_m.unwrap_or(REACH_CELLS as f32 * CELL_M);
            let what = match &part.grown {
                Some(kind) => {
                    let grows = stock.is_some_and(|stock| stock.variants(kind) > 0);
                    if !grows {
                        error!(
                            "{} has {kind} among its parts, which is no grown kind of data/flora.ron (one that grows: Stood).",
                            realm.name
                        );
                    }
                    grows.then(|| What::Grown(kind.as_str(), part.tall_m.unwrap_or((4.0, 8.0))))
                }
                None => {
                    let item = builds.item_from(&part.file);
                    if item.is_none() {
                        error!(
                            "{} has {} among its parts, which is not in the catalogue (data/builds.ron).",
                            realm.name, part.file
                        );
                    }
                    item.map(What::Model)
                }
            };
            Some((what?, part.a_km2, seen_m))
        })
        .collect()
}

/// Stands the realm's parts in the cells come within the player's reach, and takes down
/// those gone past it.
#[allow(clippy::too_many_arguments)]
fn stand(
    mut commands: Commands,
    mut stood: ResMut<Stood>,
    (realms, spec): (Res<Realms>, Res<WorldSpec>),
    (planet, start, site): (Res<Planet>, Res<Start>, Res<Site>),
    (builds, models): (Res<Builds>, Res<Models>),
    (stock, mut obstacles): (
        Option<Res<terra_world::flora::Stock>>,
        ResMut<terra_world::flora::Obstacles>,
    ),
    player: Query<&GlobalTransform, With<Player>>,
    gate: Option<Res<Gate>>,
    mut kept: Option<ResMut<KeptClear>>,
    mut clear: MessageWriter<ClearAround>,
) {
    let Some(realm) = realms
        .of(&spec.kind)
        .filter(|realm| !realm.parts.is_empty())
    else {
        return;
    };
    let Some(player) = player.iter().next() else {
        return;
    };
    let here = start.local(player.translation());
    let first = stood.middle.is_none();
    let (come, gone) = stood.moved((here.xz() / CELL_M).floor().as_ivec2());
    for part in gone {
        commands.entity(part.entity).try_despawn();
        if part.solid {
            obstacles.remove(part.ground);
        }
        if let Some(kept) = kept.as_mut() {
            kept.release(part.ground, part.clear_m);
        }
    }
    if come.is_empty() {
        return;
    }
    let items = parts_of(realm, &builds, stock.as_deref());
    let shares: Vec<f32> = items.iter().map(|(_, share, _)| *share).collect();
    let seed = planet.seed().derive("realm parts").0 as u32;
    let to_site = site.frame.compute_affine().inverse();
    let on_ground = |at: Vec2| planet.ground_at(start.at(at.x, at.y));
    let mut count = 0;
    for at_cell in come {
        let Some((part, at, turn)) = cell(seed, &shares, at_cell) else {
            continue;
        };
        let (what, _, seen_m) = &items[part];
        // Not stood until within its own sight and a little: a part drawn from 300 m, stood
        // at 520 and hidden, was half the realm's entities for nothing (its cell is looked at
        // again as the player's cell changes).
        if at.distance(here.xz()) > *seen_m + STOOD_PAST_M {
            stood.cells.remove(&at_cell);
            continue;
        }
        let ground = on_ground(at);
        // How tall a grown one stands, by the seed; and half the footing either keeps.
        let (c, r) = (at_cell.x as u32, at_cell.y as u32);
        let tall = match what {
            What::Grown(_, (least, most)) => least + (most - least) * unit3(seed, c, r, 5),
            What::Model(item) => item.dims.y,
        };
        let footing = match what {
            What::Grown(..) => tall * 0.12,
            What::Model(item) => item.dims.x.max(item.dims.z) / 2.0,
        };
        // Dry, level under its footing, and clear of the gate: by the land alone, never by
        // what else stands, so a cell come back to holds what it held.
        let rise = [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y]
            .iter()
            .map(|way| (on_ground(at + *way * footing).length() - ground.length()).abs())
            .fold(0.0, f32::max);
        let wet = ground.length() < planet.water_near_m(ground) + 0.05;
        let in_gate = gate
            .as_ref()
            .is_some_and(|gate| gate.at.distance(ground) < footing + CLEAR_M);
        if wet || rise > LEVEL_M || in_gate {
            continue;
        }
        let (entity, solid) = match what {
            What::Model(item) => {
                let Some(entity) = props::place(
                    &mut commands,
                    &models,
                    &site,
                    &planet,
                    item,
                    to_site.transform_point3(ground).with_y(0.0),
                    0.0,
                    turn,
                    false,
                ) else {
                    continue;
                };
                commands.entity(entity).insert(Theirs);
                (entity, false)
            }
            What::Grown(kind, _) => {
                let variant = mix3(seed, c, r, 6) as usize;
                let stood = stock.as_ref().and_then(|stock| {
                    stock.stand(
                        &mut commands,
                        kind,
                        (variant, ground, tall, turn.to_radians()),
                    )
                });
                let Some((entity, _)) = stood else {
                    continue;
                };
                // Its stalk stops a walker, as a trunk does.
                obstacles.add(ground, tall * 0.035);
                (entity, true)
            }
        };
        commands.entity(entity).insert(SeenFrom(*seen_m));
        let clear_m = footing + 0.8;
        clear.write(ClearAround(ground, clear_m));
        let part = Part {
            entity,
            ground,
            clear_m,
            solid,
        };
        stood.cells.insert(at_cell, Some(part));
        count += 1;
    }
    if first {
        info!(
            "{}: {count} of its parts stand round the player.",
            realm.name
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the cells within the reach of the start's hold.
    fn round_the_start(seed: u32, shares: &[f32]) -> Vec<(usize, Vec2, f32)> {
        let (come, _) = Stood::default().moved(IVec2::ZERO);
        come.into_iter()
            .filter_map(|at| cell(seed, shares, at))
            .collect()
    }

    #[test]
    fn the_seed_scatters_the_parts_by_their_shares_and_clear_of_the_start() {
        let shares = [20.0, 25.0, 25.0, 30.0, 20.0];
        assert_eq!(REACH_CELLS as f32 * CELL_M, 520.0);
        let held = round_the_start(7, &shares);
        // About as many as the land in reach holds at those shares: 120 a square kilometre
        // over a disc of 520 m, 0.85 km2: some 100.
        assert!((60..135).contains(&held.len()), "{} parts", held.len());
        // Each kind stands, none within the start's clearing.
        for part in 0..shares.len() {
            assert!(
                held.iter().any(|(which, ..)| *which == part),
                "no part {part}"
            );
        }
        assert!(
            held.iter()
                .all(|(_, at, _)| at.length() >= CLEAR_OF_START_M)
        );
        // The same seed, the same land; another seed, another.
        assert_eq!(round_the_start(7, &shares), held);
        assert_ne!(round_the_start(8, &shares), held);
    }

    /// The parts follow the player (D127): a step within a cell changes nothing, a step into
    /// the next takes up only a ring, a long walk holds no more cells than the reach and its
    /// slack, and the parts of the cells left behind are let go, each once.
    #[test]
    fn the_parts_follow_the_player_a_ring_at_a_time_and_let_go_what_is_left_behind() {
        let mut stood = Stood::default();
        let (first, _) = stood.moved(IVec2::ZERO);
        // Every cell held is given a part, numbered, so what is let go can be counted.
        let mut made = 0;
        let mut fill = |stood: &mut Stood, come: &[IVec2]| {
            for cell in come {
                made += 1;
                let part = Part {
                    entity: Entity::from_raw_u32(made).unwrap(),
                    ground: Vec3::ZERO,
                    clear_m: 0.0,
                    solid: false,
                };
                stood.cells.insert(*cell, Some(part));
            }
        };
        fill(&mut stood, &first);
        assert_eq!(stood.moved(IVec2::ZERO), (Vec::new(), Vec::new()));
        let (ring, gone) = stood.moved(IVec2::X);
        assert!(
            !ring.is_empty() && ring.len() < first.len() / 8,
            "{}",
            ring.len()
        );
        assert!(gone.is_empty(), "a step of one cell is within the slack");
        fill(&mut stood, &ring);
        let most = (2 * (REACH_CELLS + SLACK_CELLS) + 1).pow(2) as usize;
        let mut let_go = Vec::new();
        for step in 2..200 {
            let (come, gone) = stood.moved(IVec2::new(step, step / 3));
            fill(&mut stood, &come);
            let_go.extend(gone);
            assert!(
                stood.cells.len() <= most,
                "{} cells held",
                stood.cells.len()
            );
        }
        assert!(
            !stood.cells.contains_key(&IVec2::ZERO),
            "the start's cell is still held"
        );
        let before = let_go.len();
        let_go.sort_by_key(|part| part.entity);
        let_go.dedup();
        assert_eq!(let_go.len(), before, "a part was let go twice");
        assert_eq!(
            let_go.len() + stood.cells.len(),
            made as usize,
            "a part was lost"
        );
    }
}
