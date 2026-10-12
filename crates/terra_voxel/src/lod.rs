//! Which chunks exist, and at what detail, around the people looking: an octree over the
//! planet, split finer near each viewer, kept to chunks the surface can pass through.
//! Neighbouring chunks differ by one level at most, and each knows which of its faces
//! meet finer chunks, so it can be meshed with a transition toward them (D32).

use crate::field::Field;
use crate::mesh::{Sides, VOXEL_M, chunk_origin, chunk_size_m};
use glam::{IVec3, Vec3};
use std::collections::HashSet;
use terra_core::ChunkId;

/// How the tree is shaped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detail {
    /// The coarsest level, whose chunks together cover the whole planet.
    pub top: u8,
    /// A chunk splits when a viewer is closer to it than this many of its own sizes.
    pub split: f32,
    /// How far from the centre the surface can be, lowest and highest, in metres, with
    /// any digging included. Chunks wholly outside that shell hold no surface.
    pub shell: (f32, f32),
}

impl Detail {
    /// A tree for a planet of a radius whose land rises and falls by so much, with room
    /// for digging below it.
    pub fn for_planet(radius_m: f32, relief_m: f32) -> Self {
        let mut top = 0;
        while chunk_size_m(top) < radius_m + relief_m + 64.0 {
            top += 1;
        }
        Self {
            top,
            split: 2.0,
            shell: (radius_m - relief_m - 64.0, radius_m + relief_m + 8.0),
        }
    }
}

/// Whether the surface of a ground might pass through a chunk, judged from how far the
/// chunk's middle is from it. A distance measured straight up from the centre is longer
/// than the true one on a slope, so the test allows half as much again as the chunk's
/// reach, enough for slopes up to about 48 degrees.
pub fn may_hold_surface<F: Field>(field: &F, id: ChunkId) -> bool {
    let half = chunk_size_m(id.level) * 0.5;
    let middle = chunk_origin(id) + Vec3::splat(half);
    let reach = half * 3f32.sqrt();
    field.distance_in(middle, &mut F::Memo::default()).abs() <= reach * 1.5 + VOXEL_M
}

/// The chunks to draw for some viewers, and which faces of each meet finer chunks.
/// `holds` says whether the surface might pass through a chunk, such as
/// [`may_hold_surface`]; chunks it rules out are not drawn or split.
pub fn chunks_around(
    viewers: &[Vec3],
    detail: &Detail,
    holds: impl Fn(ChunkId) -> bool,
) -> Vec<(ChunkId, Sides)> {
    let holds = |id: ChunkId| in_shell(id, detail) && holds(id);
    let mut leaves = HashSet::new();
    // The top level's chunks round the centre, which between them hold the planet.
    for x in -1..1 {
        for y in -1..1 {
            for z in -1..1 {
                let root = ChunkId {
                    level: detail.top,
                    coord: IVec3::new(x, y, z),
                };
                split_near(root, viewers, detail, &holds, &mut leaves);
            }
        }
    }
    balance(&mut leaves, detail, &holds);
    let mut chunks: Vec<(ChunkId, Sides)> = leaves
        .iter()
        .map(|&leaf| (leaf, finer_sides(leaf, &leaves)))
        .collect();
    chunks.sort_by_key(|(id, _)| (id.level, id.coord.x, id.coord.y, id.coord.z));
    chunks
}

/// Whether a chunk lies within the shell where the surface can be at all.
fn in_shell(id: ChunkId, detail: &Detail) -> bool {
    let low = chunk_origin(id);
    let high = low + Vec3::splat(chunk_size_m(id.level));
    let nearest = Vec3::ZERO.clamp(low, high).length();
    let farthest = Vec3::new(
        low.x.abs().max(high.x.abs()),
        low.y.abs().max(high.y.abs()),
        low.z.abs().max(high.z.abs()),
    )
    .length();
    farthest >= detail.shell.0 && nearest <= detail.shell.1
}

/// How far a point is from a chunk's box.
fn distance_to(id: ChunkId, point: Vec3) -> f32 {
    let low = chunk_origin(id);
    let high = low + Vec3::splat(chunk_size_m(id.level));
    point.clamp(low, high).distance(point)
}

fn children(id: ChunkId) -> impl Iterator<Item = ChunkId> {
    (0..8).map(move |i| ChunkId {
        level: id.level - 1,
        coord: id.coord * 2 + IVec3::new(i & 1, (i >> 1) & 1, (i >> 2) & 1),
    })
}

fn split_near(
    id: ChunkId,
    viewers: &[Vec3],
    detail: &Detail,
    holds: &impl Fn(ChunkId) -> bool,
    leaves: &mut HashSet<ChunkId>,
) {
    if !holds(id) {
        return;
    }
    let size = chunk_size_m(id.level);
    let near = viewers
        .iter()
        .any(|&viewer| distance_to(id, viewer) < detail.split * size);
    if id.level == 0 || !near {
        leaves.insert(id);
        return;
    }
    for child in children(id) {
        split_near(child, viewers, detail, holds, leaves);
    }
}

/// The chunk at a coarser level that holds this one.
fn ancestor(id: ChunkId, level: u8) -> ChunkId {
    let shift = level - id.level;
    ChunkId {
        level,
        coord: IVec3::new(
            id.coord.x >> shift,
            id.coord.y >> shift,
            id.coord.z >> shift,
        ),
    }
}

/// Whether a place, given as a chunk's region, is drawn by one chunk at that level or a
/// coarser one, rather than split into finer ones. Also true where nothing is drawn.
fn whole(region: ChunkId, leaves: &HashSet<ChunkId>, top: u8) -> bool {
    (region.level..=top).any(|level| leaves.contains(&ancestor(region, level)))
        || !leaves
            .iter()
            .any(|leaf| leaf.level < region.level && ancestor(*leaf, region.level) == region)
}

/// The faces of a chunk across which the region at its level is split finer.
fn finer_sides(id: ChunkId, leaves: &HashSet<ChunkId>) -> Sides {
    let top = leaves
        .iter()
        .map(|leaf| leaf.level)
        .max()
        .unwrap_or(id.level);
    let mut sides = Sides::none();
    for side in Sides::ALL {
        let across = ChunkId {
            level: id.level,
            coord: id.coord + side.outward(),
        };
        if !whole(across, leaves, top) {
            sides = sides.with(side);
        }
    }
    sides
}

/// Splits any chunk that meets chunks more than one level finer, until none does, so
/// every seam is one the mesher can stitch.
fn balance(leaves: &mut HashSet<ChunkId>, detail: &Detail, holds: &impl Fn(ChunkId) -> bool) {
    loop {
        let too_coarse: Vec<ChunkId> = leaves
            .iter()
            .copied()
            .filter(|&leaf| leaf.level >= 2 && meets_much_finer(leaf, leaves, detail.top))
            .collect();
        if too_coarse.is_empty() {
            return;
        }
        for leaf in too_coarse {
            leaves.remove(&leaf);
            for child in children(leaf) {
                if holds(child) {
                    leaves.insert(child);
                }
            }
        }
    }
}

/// Whether, across any face, a chunk meets chunks two or more levels finer.
fn meets_much_finer(id: ChunkId, leaves: &HashSet<ChunkId>, top: u8) -> bool {
    Sides::ALL.into_iter().any(|side| {
        let across = ChunkId {
            level: id.level,
            coord: id.coord + side.outward(),
        };
        if whole(across, leaves, top) {
            return false;
        }
        // Split one level finer; each of those next to this face must be drawn whole.
        children(across)
            .filter(|child| {
                let toward = child.coord - across.coord * 2;
                let out = side.outward();
                // The children of `across` on the face that touches this chunk.
                (out.x == 0 || toward.x == if out.x > 0 { 0 } else { 1 })
                    && (out.y == 0 || toward.y == if out.y > 0 { 0 } else { 1 })
                    && (out.z == 0 || toward.z == if out.z > 0 { 0 } else { 1 })
            })
            .any(|child| !whole(child, leaves, top))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{GeneratedPlanet, VoxelPlanet};
    use terra_core::PlanetSpec;
    use terra_gen::Seed;

    fn detail() -> Detail {
        Detail::for_planet(2000.0, 150.0)
    }

    fn planet() -> VoxelPlanet<GeneratedPlanet> {
        VoxelPlanet::new(GeneratedPlanet::new(
            PlanetSpec {
                seed: Seed(7),
                radius_m: 2000.0,
            },
            150.0,
            Some(-20.0),
        ))
    }

    /// The chunks around viewers on the test planet.
    fn around(viewers: &[Vec3]) -> Vec<(ChunkId, Sides)> {
        let planet = planet();
        chunks_around(viewers, &detail(), |id| may_hold_surface(&planet, id))
    }

    /// A point on the test planet's ground in a direction.
    fn ground(direction: Vec3) -> Vec3 {
        let direction = direction.normalize();
        direction * (2000.0 + planet().field().height_m(direction))
    }

    fn level_at(point: Vec3, chunks: &[(ChunkId, Sides)]) -> Option<u8> {
        chunks
            .iter()
            .find(|(id, _)| distance_to(*id, point) == 0.0)
            .map(|(id, _)| id.level)
    }

    #[test]
    fn detail_is_finest_by_the_viewer_and_coarser_away() {
        let viewer = ground(Vec3::Y) + Vec3::Y * 1.7;
        let chunks = around(&[viewer]);
        assert_eq!(level_at(viewer, &chunks), Some(0));
        // A chunk splits within two of its sizes, so 300 m off is 64 m chunks or coarser,
        // and a kilometre off coarser still.
        let away = |metres: f32| ground(Vec3::new(0.0, 2000.0, metres));
        assert!(level_at(away(300.0), &chunks).unwrap() >= 2);
        assert!(level_at(away(1000.0), &chunks).unwrap() >= 4);
        let mut by_level = [0; 12];
        for (id, _) in &chunks {
            by_level[id.level as usize] += 1;
        }
        assert!(
            chunks.len() < 1500,
            "{} chunks, by level {by_level:?}",
            chunks.len()
        );
        // Nothing far above or deep below the surface.
        assert!(chunks.iter().all(|(id, _)| in_shell(*id, &detail())));
    }

    #[test]
    fn chunks_do_not_overlap() {
        let chunks = around(&[ground(Vec3::Y)]);
        let set: HashSet<ChunkId> = chunks.iter().map(|(id, _)| *id).collect();
        for (id, _) in &chunks {
            for level in id.level + 1..=detail().top {
                assert!(!set.contains(&ancestor(*id, level)));
            }
        }
    }

    #[test]
    fn neighbours_differ_by_one_level_and_the_coarser_knows() {
        let detail = detail();
        let viewers = [ground(Vec3::Y), ground(Vec3::new(500.0, 1940.0, -80.0))];
        let chunks = around(&viewers);
        let set: HashSet<ChunkId> = chunks.iter().map(|(id, _)| *id).collect();
        for (id, sides) in &chunks {
            assert!(
                !meets_much_finer(*id, &set, detail.top),
                "{id:?} meets much finer"
            );
            for side in Sides::ALL {
                let across = ChunkId {
                    level: id.level,
                    coord: id.coord + side.outward(),
                };
                assert_eq!(sides.contains(side), !whole(across, &set, detail.top));
            }
        }
    }

    #[test]
    fn two_viewers_far_apart_both_see_fine_detail() {
        let here = ground(Vec3::Y);
        let there = ground(Vec3::X);
        let chunks = around(&[here, there]);
        assert_eq!(level_at(here, &chunks), Some(0));
        assert_eq!(level_at(there, &chunks), Some(0));
    }
}
