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
use crate::flora::{Obstacles, Trunk, Trunks};
use crate::planet::{MainCamera, Planet};
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
    /// Its trees being worked out on the streaming's pool.
    Laying(Ticket<Vec<Tree>>),
    /// Its trees standing, and those still to stand, the last first.
    In {
        standing: Vec<Standing>,
        coming: Vec<Tree>,
    },
}

/// The woods' grown trees round the player, block by block, and the felled.
#[derive(Resource)]
pub(super) struct NearWoods {
    map: Map,
    seed: terra_gen::Seed,
    conifers: Arc<dyn ConiferModels>,
    blocks: HashMap<Block, State>,
    /// The grown trees felled, by their foot (`foot_key`): left out when their block comes in
    /// again (`felled::unfar_the_felled` files them).
    pub felled: HashSet<[u32; 3]>,
}

impl NearWoods {
    pub(super) fn new(map: Map, seed: terra_gen::Seed, conifers: Arc<dyn ConiferModels>) -> Self {
        Self {
            map,
            seed,
            conifers,
            blocks: HashMap::default(),
            felled: HashSet::default(),
        }
    }

    /// A block laid at the start, its trees standing.
    pub(super) fn laid(&mut self, block: Block, standing: Vec<Standing>) {
        let coming = Vec::new();
        self.blocks.insert(block, State::In { standing, coming });
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
        if let Some(State::In { standing, .. }) = near.blocks.remove(&block) {
            for tree in standing {
                trunks.remove(tree.foot);
                if let Ok(placed) = placed.get(tree.entity) {
                    if let (true, Some(obstacles)) = (tree.solid, obstacles.as_mut()) {
                        obstacles.remove(placed.translation);
                    }
                    super::felled::take_out(&mut commands, tree.entity);
                }
            }
        }
    }
    for block in ask {
        let (map, seed, planet) = (near.map, near.seed, planet.clone());
        let (low, high) = span(block);
        let middle = planet.surface(map.toward((low + high) / 2.0));
        let half = Vec3::splat(low.distance(high) / 2.0);
        let place = Place::Bounds(middle - half, middle + half);
        let ticket = streaming.ask(Class::NearWoods, place, move || {
            lay(&planet, &map, block, seed)
        });
        near.blocks.insert(block, State::Laying(ticket));
    }
    // Laid blocks come in to stand.
    for state in near.blocks.values_mut() {
        if let State::Laying(ticket) = state
            && let Some(mut trees) = ticket.take()
        {
            trees.retain(|tree| !near.felled.contains(&super::foot_key(tree.at)));
            trees.reverse();
            *state = State::In {
                standing: Vec::with_capacity(trees.len()),
                coming: trees,
            };
        }
    }
    let mut entering: Vec<(&Block, &mut State)> = near
        .blocks
        .iter_mut()
        .filter(|(_, state)| matches!(state, State::In { coming, .. } if !coming.is_empty()))
        .collect();
    entering.sort_by(|a, b| away(*a.0, at).total_cmp(&away(*b.0, at)));
    let mut stood = 0;
    for (_, state) in entering {
        let State::In { standing, coming } = state else {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
