//! The woods' grown trees streamed round the player (D622), as open worlds stream their
//! placed foliage in cells round the camera: Unreal's World Partition with its PCG runtime
//! generation (a grid of cells generated round each generation source, cleaned up past a
//! larger radius), Horizon Zero Dawn's placement (Guerrilla, GDC 2017: tiles round the player,
//! the same from the same seed every time). The woods' grid (`terra_land::woods`) is cut into
//! blocks of `BLOCK_CELLS` cells; a block's trees are the same trees the far cards were laid
//! from, worked out on the streaming's pool when the camera comes within `IN_M` of it, stood
//! a few a frame under the streaming's budget, and taken out again once it is past `OUT_M`,
//! the band between them so a player walking to and fro at an edge does not lay and drop a
//! block every step. A felled tree is kept felled: its block, streamed in again, leaves it out.

use super::felled::WoodsTree;
use super::{COARSE_M, ConiferModels, Drawn, SPACING_M, planted};
use crate::cover::forest::{GrownFloor, Stand};
use crate::flora::{Obstacles, Trunk, Trunks};
use crate::planet::{MainCamera, Planet};
use crate::start::Start;
use crate::streaming::{Class, Place, Streaming, Ticket};
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use std::sync::Arc;
use terra_land::woods::{Tree, Woods};

/// A block is this many of the woods' cells a side: 104 m.
const BLOCK_CELLS: i32 = 16;
/// A block's trees are asked for once its nearest edge is this near the camera on the woods'
/// map, past the farthest any grown tree's last level reaches (280 m for the tallest, the lush
/// forest's `far_from`), so the walker meets none coming in; and go once it is past `OUT_M`.
pub(super) const IN_M: f32 = 360.0;
const OUT_M: f32 = 460.0;

/// A block of the woods' grid, by its first cell east and north over `BLOCK_CELLS`.
pub(super) type Block = (i32, i32);

/// The woods' map: the plane they are laid on about their middle, east and north of it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Map {
    /// The woods' middle as their trees are laid about it, on the ground.
    pub middle: Vec3,
    up: Vec3,
    east: Vec3,
    north: Vec3,
    radius_m: f32,
}

impl Map {
    pub(super) fn new(middle: Vec3, east: Vec3, north: Vec3, radius_m: f32) -> Self {
        Self {
            middle,
            up: middle.normalize(),
            east,
            north,
            radius_m,
        }
    }

    /// Where a point lies on the map, metres east and north of the middle: along the ray from
    /// the planet's centre through it to the plane the woods' cells are laid out on.
    pub(super) fn at(&self, point: Vec3) -> Vec2 {
        let toward = point.normalize_or(self.up);
        let on_plane = toward * (self.radius_m / toward.dot(self.up).max(1e-3));
        let offset = on_plane - self.up * self.radius_m;
        Vec2::new(offset.dot(self.east), offset.dot(self.north))
    }

    /// The direction a place on the map stands toward from the planet's centre.
    fn toward(&self, at: Vec2) -> Vec3 {
        (self.up * self.radius_m + self.east * at.x + self.north * at.y).normalize()
    }
}

/// The block's corners on the map, south-west and north-east.
fn span(block: Block) -> (Vec2, Vec2) {
    let side = BLOCK_CELLS as f32 * SPACING_M;
    let low = Vec2::new(block.0 as f32, block.1 as f32) * side;
    (low, low + Vec2::splat(side))
}

/// How far a place on the map is from a block, 0 inside it.
fn away(block: Block, at: Vec2) -> f32 {
    let (low, high) = span(block);
    at.clamp(low, high).distance(at)
}

/// The blocks within `reach_m` of a place on the map.
pub(super) fn within(at: Vec2, reach_m: f32) -> impl Iterator<Item = Block> {
    let side = BLOCK_CELLS as f32 * SPACING_M;
    let low = ((at - reach_m) / side).floor().as_ivec2();
    let high = ((at + reach_m) / side).floor().as_ivec2();
    (low.y..=high.y)
        .flat_map(move |y| (low.x..=high.x).map(move |x| (x, y)))
        .filter(move |&block| away(block, at) <= reach_m)
}

/// What the streaming does about the camera at `at` on the map, with `have` laid or asked:
/// the blocks to ask for, nearest first, and those to take out.
fn wanted(at: Vec2, have: impl Iterator<Item = Block>) -> (Vec<Block>, Vec<Block>) {
    let have: Vec<Block> = have.collect();
    let mut ask: Vec<Block> = within(at, IN_M)
        .filter(|block| !have.contains(block))
        .collect();
    ask.sort_by(|a, b| away(*a, at).total_cmp(&away(*b, at)));
    let drop = have
        .into_iter()
        .filter(|&block| away(block, at) > OUT_M)
        .collect();
    (ask, drop)
}

/// The trees a block holds, in the order the woods lay them, none in the start's clearing
/// or a snowbed: the same as the far cards of those cells stand for.
pub(super) fn lay(planet: &Planet, map: &Map, block: Block, seed: terra_gen::Seed) -> Vec<Tree> {
    let Some(round) = planet.round() else {
        return Vec::new();
    };
    let (x, y) = (block.0 * BLOCK_CELLS, block.1 * BLOCK_CELLS);
    let cells = (x..x + BLOCK_CELLS, y..y + BLOCK_CELLS);
    let trees = Woods::of(round).trees_in_cells(map.middle, cells, COARSE_M, SPACING_M, seed);
    let trees = super::cleared(map.middle, trees);
    super::off_snowbeds(planet, trees)
}

/// A grown tree standing: its entity, its foot, and whether it stops a walker.
#[derive(Clone, Copy, Debug)]
pub(super) struct Standing {
    entity: Entity,
    foot: Vec3,
    solid: bool,
}

/// Where a block is in its streaming.
enum State {
    /// Its trees being worked out on the streaming's pool, each with how its floor sees it.
    Laying(Ticket<Vec<(Tree, Stand)>>),
    /// Its trees standing, and those still to stand, the last first; and the feet of those
    /// on the woods' floor, on the start's map (`GrownFloor`), whether standing yet or not.
    In {
        standing: Vec<Standing>,
        coming: Vec<Tree>,
        canopy: Vec<Vec2>,
    },
}

/// The woods' grown trees round the player, block by block, and the felled.
#[derive(Resource)]
pub struct NearWoods {
    map: Map,
    seed: terra_gen::Seed,
    /// The start's map, which the floor under the trees is laid on.
    start: Start,
    conifers: Arc<dyn ConiferModels>,
    blocks: HashMap<Block, State>,
    /// The grown trees felled, by their foot (`foot_key`): left out when their block comes in
    /// again (`felled::unfar_the_felled` files them).
    pub felled: HashSet<[u32; 3]>,
}

impl NearWoods {
    pub(super) fn new(
        map: Map,
        seed: terra_gen::Seed,
        start: Start,
        conifers: Arc<dyn ConiferModels>,
    ) -> Self {
        Self {
            map,
            seed,
            start,
            conifers,
            blocks: HashMap::default(),
            felled: HashSet::default(),
        }
    }

    /// A block laid at the start, its trees standing, and their feet on the floor laid with
    /// them.
    pub(super) fn laid(&mut self, block: Block, standing: Vec<Standing>, canopy: Vec<Vec2>) {
        let coming = Vec::new();
        self.blocks.insert(
            block,
            State::In {
                standing,
                coming,
                canopy,
            },
        );
    }

    /// A block's trees worked out, come in to stand, less the felled: their crowns go on the
    /// woods' floor at once, so the cover under them is the forest's (D619) before the first
    /// of them stands.
    fn arrive(&mut self, block: Block, laid: Vec<(Tree, Stand)>, floor: Option<&mut GrownFloor>) {
        let (mut coming, stands): (Vec<Tree>, Vec<Stand>) = laid
            .into_iter()
            .filter(|(tree, _)| !self.felled.contains(&super::foot_key(tree.at)))
            .unzip();
        coming.reverse();
        let canopy: Vec<Vec2> = stands.iter().map(|stand| stand.at).collect();
        if let Some(floor) = floor {
            floor.change(canopy.iter().copied(), |floor| {
                for stand in stands {
                    floor.canopy.add(stand);
                }
            });
        }
        let standing = Vec::with_capacity(coming.len());
        self.blocks.insert(
            block,
            State::In {
                standing,
                coming,
                canopy,
            },
        );
    }

    /// Files the trees whose feet are `keys` as felled, as a saved game gives them back: those
    /// still to stand are left out, and those standing are handed back to be taken out.
    pub(super) fn fell_again(&mut self, keys: &HashSet<[u32; 3]>) -> Vec<Standing> {
        self.felled.extend(keys.iter().copied());
        let mut taken = Vec::new();
        for state in self.blocks.values_mut() {
            let State::In {
                standing, coming, ..
            } = state
            else {
                continue;
            };
            coming.retain(|tree| !keys.contains(&super::foot_key(tree.at)));
            let (gone, kept): (Vec<Standing>, Vec<Standing>) = std::mem::take(standing)
                .into_iter()
                .partition(|tree| keys.contains(&super::foot_key(tree.foot)));
            *standing = kept;
            taken.extend(gone);
        }
        taken
    }

    /// Whether the trees of every block within `reach_m` of a point have arrived, their
    /// crowns on the woods' floor (`arrive`): what is laid among them there finds them.
    pub fn trees_in(&self, point: Vec3, reach_m: f32) -> bool {
        within(self.map.at(point), reach_m)
            .all(|block| matches!(self.blocks.get(&block), Some(State::In { .. })))
    }

    /// How many grown trees stand.
    fn standing(&self) -> usize {
        self.blocks
            .values()
            .map(|state| match state {
                State::In { standing, .. } => standing.len(),
                State::Laying(_) => 0,
            })
            .sum()
    }
}

/// Stands one grown tree, drawn by `conifers`, and files its trunk.
pub(super) fn grow(
    commands: &mut Commands,
    conifers: &dyn ConiferModels,
    trunks: &mut Trunks,
    tree: &Tree,
) -> (Standing, Drawn) {
    let planted = planted(tree);
    let mut entity = commands.spawn((
        Name::new("Woods tree"),
        Visibility::default(),
        WoodsTree { foot: tree.at },
    ));
    let drawn = conifers.draw(&mut entity, &planted);
    // A solid one felled is no longer found among the trunks (`Trunks::near`).
    trunks.add(Trunk {
        foot: tree.at,
        radius_m: (planted.height_m * 0.012).max(0.08),
        height_m: planted.height_m,
        solid: drawn.solid,
    });
    let standing = Standing {
        entity: entity.id(),
        foot: tree.at,
        solid: drawn.solid,
    };
    (standing, drawn)
}

/// Asks for the blocks coming within reach of the camera, stands their trees a few a frame
/// under the streaming's budget, the nearest block's first, and takes out those gone past
/// reach, their trunks and what stops a walker with them.
#[allow(clippy::too_many_arguments)]
pub(super) fn stream(
    mut commands: Commands,
    planet: Res<Planet>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    near: Option<ResMut<NearWoods>>,
    mut streaming: ResMut<Streaming>,
    mut trunks: ResMut<Trunks>,
    mut obstacles: Option<ResMut<Obstacles>>,
    placed: Query<&Transform, With<WoodsTree>>,
    mut floor: Option<ResMut<GrownFloor>>,
) {
    terra_gen::stat_scope!("flora");
    let (Some(mut near), Some(camera)) = (near, camera) else {
        return;
    };
    let near = &mut *near;
    let at = near.map.at(camera.translation());
    let (ask, drop) = wanted(at, near.blocks.keys().copied());
    for block in drop {
        // A block still being laid is cancelled as its ticket goes.
        if let Some(State::In {
            standing, canopy, ..
        }) = near.blocks.remove(&block)
        {
            take_off_the_floor(floor.as_deref_mut(), canopy);
            for tree in standing {
                take_out(
                    &mut commands,
                    (&mut *trunks, obstacles.as_deref_mut()),
                    &placed,
                    tree,
                );
            }
        }
    }
    for block in ask {
        let (map, seed, planet, start) = (near.map, near.seed, planet.clone(), near.start);
        let conifers = near.conifers.clone();
        let (low, high) = span(block);
        let middle = planet.surface(map.toward((low + high) / 2.0));
        let half = Vec3::splat(low.distance(high) / 2.0);
        let place = Place::Bounds(middle - half, middle + half);
        let ticket = streaming.ask(Class::NearWoods, place, move || {
            let trees = lay(&planet, &map, block, seed);
            trees
                .into_iter()
                .map(|tree| {
                    let drawn = conifers.drawn(&planted(&tree));
                    (tree, super::stand(&start, tree.at, &drawn))
                })
                .collect()
        });
        near.blocks.insert(block, State::Laying(ticket));
    }
    // Laid blocks come in to stand.
    let laid: Vec<(Block, Vec<(Tree, Stand)>)> = near
        .blocks
        .iter_mut()
        .filter_map(|(&block, state)| match state {
            State::Laying(ticket) => ticket.take().map(|laid| (block, laid)),
            State::In { .. } => None,
        })
        .collect();
    for (block, laid) in laid {
        near.arrive(block, laid, floor.as_deref_mut());
    }
    let mut entering: Vec<(&Block, &mut State)> = near
        .blocks
        .iter_mut()
        .filter(|(_, state)| matches!(state, State::In { coming, .. } if !coming.is_empty()))
        .collect();
    entering.sort_by(|a, b| away(*a.0, at).total_cmp(&away(*b.0, at)));
    let mut stood = 0;
    for (_, state) in entering {
        let State::In {
            standing, coming, ..
        } = state
        else {
            continue;
        };
        while streaming.may_enter(Class::NearWoods)
            && let Some(tree) = coming.pop()
        {
            let began = std::time::Instant::now();
            let mut spawn = streaming.commands(Class::NearWoods, &commands);
            let (tree, _) = grow(&mut spawn, &*near.conifers, &mut trunks, &tree);
            standing.push(tree);
            streaming.entered(Class::NearWoods, began, 0);
            stood += 1;
        }
        if !coming.is_empty() {
            break;
        }
    }
    let all_in = near
        .blocks
        .values()
        .all(|state| matches!(state, State::In { coming, .. } if coming.is_empty()));
    if stood > 0 && all_in {
        info!(
            "Woods: {} grown trees stand in {} blocks round the player",
            near.standing(),
            near.blocks.len()
        );
    }
}

/// Takes a standing tree out of the world, its trunk and what stops a walker with it, unfiled
/// as felled (`felled::take_out`).
pub(super) fn take_out(
    commands: &mut Commands,
    (trunks, obstacles): (&mut Trunks, Option<&mut Obstacles>),
    placed: &Query<&Transform, With<WoodsTree>>,
    tree: Standing,
) {
    trunks.remove(tree.foot);
    if let Ok(placed) = placed.get(tree.entity) {
        if let (true, Some(obstacles)) = (tree.solid, obstacles) {
            obstacles.remove(placed.translation);
        }
        super::felled::take_out(commands, tree.entity);
    }
}

/// Takes a block's trees off the woods' floor as it streams out, by their feet.
fn take_off_the_floor(floor: Option<&mut GrownFloor>, canopy: Vec<Vec2>) {
    if let Some(floor) = floor {
        floor.change(canopy.iter().copied(), |floor| {
            for at in &canopy {
                floor.canopy.remove(*at);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cover::forest::{Crowns, Floor, Wild};
    use crate::woods::TreeKind;
    use crate::woods::tests::{Bare, tree};

    /// A block's trees go onto the woods' floor as the block comes in, before any of them
    /// stands, so the cover under them grows by the forest's rules (D619) and is grown again
    /// there; a felled one stays off; and they go off it with their block.
    #[test]
    fn a_blocks_trees_come_onto_the_floor_with_it_and_go_with_it() {
        let ground = Vec3::new(0.0, 30_000.0, 0.0);
        let map = Map::new(ground, Vec3::X, Vec3::NEG_Z, ground.length());
        let start = Start(Transform::from_translation(ground));
        let mut near = NearWoods::new(map, terra_gen::Seed(1), start, Arc::new(Bare));
        let mut floor = GrownFloor::new(Floor::new(Wild, Crowns::default(), terra_gen::Seed(2)));
        let (under, felled_under) = (Vec2::new(10.0, 10.0), Vec2::new(60.0, -30.0));
        let on_ground = |at: Vec2| ground + Vec3::new(at.x, 0.0, at.y);
        let trees = [under, under + Vec2::new(4.0, 0.0), felled_under]
            .map(|at| tree(on_ground(at), TreeKind::Oak, 1.0));
        near.felled.insert(super::super::foot_key(trees[2].at));
        let laid: Vec<(Tree, Stand)> = trees
            .iter()
            .map(|tree| {
                let drawn = near.conifers.drawn(&planted(tree));
                (*tree, super::super::stand(&start, tree.at, &drawn))
            })
            .collect();
        let open = floor.read().here(under).light;
        assert!(!floor.read().canopy.among(under));

        near.arrive((0, 0), laid, Some(&mut floor));
        assert_eq!(floor.read().canopy.len(), 2, "the felled one stays off");
        assert!(floor.read().canopy.among(under));
        assert!(!floor.read().canopy.among(felled_under));
        let shaded = floor.read().here(under).light;
        assert!(
            shaded < open * 0.8,
            "under its crown: {shaded} against {open}"
        );
        let changes = floor.take_changes();
        assert!(
            changes.iter().any(
                |(low, high)| under.cmpge(*low + 20.0).all() && under.cmple(*high - 20.0).all()
            ),
            "the cover round them is grown again: {changes:?}"
        );
        let Some(State::In { coming, canopy, .. }) = near.blocks.remove(&(0, 0)) else {
            panic!("the block came in");
        };
        assert_eq!(coming.len(), 2);

        take_off_the_floor(Some(&mut floor), canopy);
        assert!(floor.read().canopy.is_empty());
        assert_eq!(floor.read().here(under).light, open);
        assert_eq!(floor.take_changes().len(), 1);
    }

    /// The trees round a place have arrived only when every block within reach of it is in:
    /// a block still laying, or not asked yet, has not put its crowns on the floor.
    #[test]
    fn trees_are_in_round_a_place_only_once_every_block_near_it_is() {
        let ground = Vec3::new(0.0, 30_000.0, 0.0);
        let map = Map::new(ground, Vec3::X, Vec3::NEG_Z, ground.length());
        let start = Start(Transform::from_translation(ground));
        let mut near = NearWoods::new(map, terra_gen::Seed(1), start, Arc::new(Bare));
        let side = BLOCK_CELLS as f32 * SPACING_M;
        // Map east and north: north is the world's -z.
        let point = |at: Vec2| ground + Vec3::new(at.x, 0.0, -at.y);
        let middle = point(Vec2::splat(side / 2.0));
        assert!(!near.trees_in(middle, 5.0), "no block is in yet");
        near.arrive((0, 0), Vec::new(), None);
        assert!(near.trees_in(middle, 5.0));
        assert!(
            !near.trees_in(middle, side),
            "the blocks round it are not in"
        );
        let edge = point(Vec2::new(side - 1.0, side / 2.0));
        assert!(!near.trees_in(edge, 5.0), "the block east of it is not in");
    }

    /// The blocks within reach are asked for, the nearest first; one is not taken out until
    /// the camera is past the farther reach, so walking to and fro at an edge keeps it.
    #[test]
    fn blocks_come_within_reach_and_go_only_past_the_farther_one() {
        let side = BLOCK_CELLS as f32 * SPACING_M;
        let (ask, drop) = wanted(Vec2::ZERO, std::iter::empty());
        assert!(drop.is_empty());
        assert!(ask.iter().all(|&block| away(block, Vec2::ZERO) <= IN_M));
        assert!(
            ask.windows(2)
                .all(|w| away(w[0], Vec2::ZERO) <= away(w[1], Vec2::ZERO))
        );
        // Every place within reach is in a block asked for, none twice.
        for step in 0..40 {
            let angle = step as f32 * 0.37;
            let toward = Vec2::new(libm::cosf(angle), libm::sinf(angle));
            let at = toward * (IN_M - 1.0) * (step as f32 / 40.0);
            let cell = (at / side).floor().as_ivec2();
            assert!(ask.contains(&(cell.x, cell.y)), "{at} in {cell}");
        }
        let mut unique = ask.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), ask.len());

        // Walking east: the block at the start's west edge is kept while it is within the
        // farther reach, and goes past it; walking back, it is asked for only within the
        // nearer one.
        let west = (-1, 0);
        let have = ask.clone();
        let kept = Vec2::new(OUT_M - 1.0, 0.0);
        let (_, drop) = wanted(kept, have.iter().copied());
        assert!(!drop.contains(&west));
        let past = Vec2::new(OUT_M + 1.0, 0.0);
        let (_, drop) = wanted(past, have.iter().copied());
        assert!(drop.contains(&west));
        let left: Vec<Block> = have
            .into_iter()
            .filter(|block| !drop.contains(block))
            .collect();
        let (ask, _) = wanted(Vec2::new(IN_M + 1.0, 0.0), left.iter().copied());
        assert!(!ask.contains(&west), "between the reaches it stays out");
        let (ask, _) = wanted(Vec2::new(IN_M - 1.0, 0.0), left.iter().copied());
        assert!(ask.contains(&west));
    }

    /// A point on the woods' map is where it was laid from: the map gives back the place the
    /// trees' grid put it, out to the woods' far edge.
    #[test]
    fn the_map_gives_back_where_a_place_was_laid() {
        let middle = Vec3::new(0.3, 0.8, 0.52).normalize() * 30_000.0;
        let up = middle.normalize();
        let east = up.any_orthonormal_vector();
        let north = up.cross(east);
        let map = Map::new(middle, east, north, 30_000.0);
        for at in [
            Vec2::ZERO,
            Vec2::new(250.0, -40.0),
            Vec2::new(-1_500.0, 900.0),
        ] {
            // On the ground, hundreds of metres above or below the sphere.
            let point = map.toward(at) * 30_350.0;
            assert!(
                map.at(point).distance(at) < 0.01,
                "{at} came back {}",
                map.at(point)
            );
        }
    }

    /// A saved game's felled trees are felled again as it loads (D622): one standing goes
    /// with its trunk, one still to stand in its block is left out, the rest stand; and the
    /// felled are read back as they were given, to be saved again, one whose block is still
    /// to come with them.
    #[test]
    fn trees_felled_in_a_saved_game_are_felled_again_standing_or_still_to_stand() {
        use super::super::felled::{FellAgain, FelledWoods, fell_again};
        use bevy::ecs::system::RunSystemOnce;
        let ground = Vec3::new(0.0, 30_000.0, 0.0);
        let map = Map::new(ground, Vec3::X, Vec3::NEG_Z, ground.length());
        let start = Start(Transform::from_translation(ground));
        let [stood, kept, coming, later] =
            [0.0, 4.0, 8.0, 300.0].map(|east| tree(ground + Vec3::X * east, TreeKind::Oak, 1.0));
        let mut world = World::new();
        world.init_resource::<Messages<FellAgain>>();
        let mut trunks = Trunks::default();
        let standing = {
            let mut commands = world.commands();
            [stood, kept].map(|tree| {
                let (standing, _) = grow(&mut commands, &Bare, &mut trunks, &tree);
                commands
                    .entity(standing.entity)
                    .insert(Transform::from_translation(tree.at));
                standing
            })
        };
        world.flush();
        let mut near = NearWoods::new(map, terra_gen::Seed(1), start, Arc::new(Bare));
        near.blocks.insert(
            (0, 0),
            State::In {
                standing: standing.to_vec(),
                coming: vec![coming],
                canopy: Vec::new(),
            },
        );
        world.insert_resource(near);
        world.insert_resource(trunks);
        let felled = vec![stood.at, coming.at, later.at];
        world.write_message(FellAgain(felled.clone()));
        world.run_system_once(fell_again).unwrap();

        assert!(world.get_entity(standing[0].entity).is_err());
        assert!(world.get_entity(standing[1].entity).is_ok());
        assert_eq!(
            world.resource::<Trunks>().len(),
            1,
            "the felled one's trunk goes"
        );
        let Some(State::In {
            standing, coming, ..
        }) = world.resource::<NearWoods>().blocks.get(&(0, 0))
        else {
            panic!("the block is in");
        };
        assert_eq!(standing.len(), 1);
        assert!(
            coming.is_empty(),
            "the felled one still to stand is left out"
        );
        let mut read = world
            .run_system_once(|felled: FelledWoods| felled.feet())
            .unwrap();
        let mut given = felled;
        for feet in [&mut read, &mut given] {
            feet.sort_by_key(|foot| super::super::foot_key(*foot));
        }
        assert_eq!(read, given);
    }
}
