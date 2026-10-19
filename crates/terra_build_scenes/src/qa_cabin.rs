//! The QA build film's cabin as it stands finished (tools/sandbox/qa/build.ts), laid by the
//! same plans the hammer and the remote lay (`plans`) and built by the checked operations,
//! so the drawing checker (`inspect`) sees what a player builds: walls of several kinds,
//! a door, windows cut and carved, a wall changed to another kind in place, a garden wall
//! of blocks, a storey over the first with a roof and a dormer, and a deck on posts with
//! stairs up to it. What the film knocks down and mends, and the roof it takes off before
//! the storey goes on, leave it as this.

use glam::{IVec2, IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use terra_build::kit::Kits;
use terra_build::ops::{Context, Op};
use terra_build::plans::{self, Footings};
use terra_build::roofer::Style;
use terra_build::{Base, Facing, Kind, Panel, Sample, Shape};

/// The cabin's corner tiles, as the film's: five metres east to west, four north to south.
const X0: i32 = 4;
const X1: i32 = 13;
const Z0: i32 = 4;
const Z1: i32 = 11;

/// The finished cabin in kits' kinds: its walls, each thing laid in the next of `walls` as a
/// mix lays them, the wall changed to `other`, on ground given in centimetres a column; with
/// `roof_left_on`, its storey laid by the room tool round a roof over the first storey left
/// on, standing on its eaves and gables (D343), as the film's storey may be built.
pub fn cabin(
    kits: &Kits,
    walls: &[Kind],
    other: Kind,
    ground_cm: &dyn Fn([i32; 2]) -> i16,
    roof_left_on: bool,
) -> Base {
    let wall = |n: usize| walls[n % walls.len()];
    let rules = kits.rules();
    let cx = Context::free(&rules);
    let footing = kits.footing().map(|kit| kit.kind());
    let stone = footing.unwrap_or(wall(0));
    let footed: BTreeSet<Kind> = kits
        .0
        .iter()
        .filter(|kit| kit.pattern == terra_build::kit::Pattern::Masonry && kit.framing.is_none())
        .map(|kit| kit.kind())
        .collect();
    let families: BTreeMap<Kind, u8> = kits
        .0
        .iter()
        .map(|kit| (kit.kind(), plans::family(kit)))
        .collect();
    let blocks = |kind: Kind| kits.one_at_a_time(kind);
    let footings = || Footings::of(footing, ground_cm, &footed);
    let mut base = Base::default();
    // Each plan with the ground under its columns, as whoever lays it fills it in.
    let lay = |base: &mut Base, ops: Vec<Op>| {
        let sample = |column| Sample {
            column,
            height_cm: ground_cm(column),
        };
        for op in plans::with_ground(ops, sample) {
            let _ = base.act(&op, &cx);
        }
    };
    let tiles = |cubes: &[[i32; 3]]| -> Vec<IVec3> {
        cubes.iter().map(|&c| IVec3::from_array(c)).collect()
    };
    // The plan: four walls, a stone floor, a door and a window.
    let sides = [
        ([X0, Z1], [X1, Z1]),
        ([X1, Z0], [X1, Z1]),
        ([X0, Z0], [X1, Z0]),
        ([X0, Z0], [X0, Z1]),
    ];
    for (i, (from, to)) in sides.into_iter().enumerate() {
        let run = plans::wall_tiles(IVec2::from_array(from), IVec2::from_array(to), 0);
        let ops = plans::wall(&base, &run, wall(i), footings());
        lay(&mut base, ops);
    }
    let ops = plans::floor(
        &base,
        IVec3::new(X0 + 1, 0, Z0 + 1),
        IVec3::new(X1 - 1, 0, Z1 - 1),
        stone,
        footings(),
    );
    lay(&mut base, ops);
    for (cubes, panel) in [
        (tiles(&[[8, 0, Z1], [9, 0, Z1]]), Panel::Door),
        (tiles(&[[X1, 0, 7], [X1, 0, 8]]), Panel::WINDOW),
    ] {
        let ops = plans::opening(&base, &plans::opening_cubes(&cubes, panel), footings());
        lay(&mut base, ops);
    }
    // A low garden wall of blocks west of the door, a post at each end.
    let garden = wall(4);
    for y in [-4, -2] {
        for x in [0, 2, 4, 6] {
            let cubes = plans::brush(IVec3::new(x, y, 14), 2);
            let ops = plans::blocks(&base, &cubes, Shape::Whole, garden, &rules, footings());
            lay(&mut base, ops);
        }
    }
    for x in [0, 7] {
        for z in [14, 15] {
            let cubes = plans::brush(IVec3::new(x, 0, z), 1);
            let ops = plans::blocks(&base, &cubes, Shape::Whole, garden, &rules, footings());
            lay(&mut base, ops);
        }
    }
    build(&mut base, &cx, &blocks);
    // A window cut into the built north wall.
    let cut = plans::opening_cubes(&tiles(&[[8, 0, Z0], [9, 0, Z0]]), Panel::WINDOW);
    let ops = plans::opening(&base, &cut, footings());
    lay(&mut base, ops);
    build(&mut base, &cx, &blocks);
    // The west wall changed to another kind in place, and a window carved in it.
    for z in [4, 6, 8, 10] {
        for y in [0, 2, 4] {
            let cubes = plans::brush(IVec3::new(X0, y, z), 2);
            let ops = plans::change(&base, &cubes, other, &rules, &families, blocks);
            lay(&mut base, ops);
        }
    }
    build(&mut base, &cx, &blocks);
    let ops = plans::carve(&base, &plans::brush(IVec3::new(X0, 2, 6), 2), footings());
    lay(&mut base, ops);
    build(&mut base, &cx, &blocks);
    if roof_left_on {
        let roof = kits.roof_for(wall(0)).unwrap_or(Kind::FIRST_ROOF);
        let footprint = plans::room_footprint(&base, IVec3::new(8, 0, 7));
        let ops = plans::roof(&base, &footprint, 0, Style::Gable, None, roof);
        lay(&mut base, ops);
        build(&mut base, &cx, &blocks);
    }
    // A storey on it, a roof over the storey and a dormer on its south slope.
    let upper = wall(1);
    let ops = plans::room(
        &base,
        IVec3::new(X0, 1, Z0),
        IVec3::new(X1, 1, Z1),
        upper,
        upper,
        footings(),
    );
    lay(&mut base, ops);
    build(&mut base, &cx, &blocks);
    let roof = kits.roof_over(upper);
    let footprint = plans::room_footprint(&base, IVec3::new(8, 1, 7));
    let ops = plans::roof(&base, &footprint, 1, Style::Gable, None, roof);
    lay(&mut base, ops);
    let dormer: BTreeSet<[i32; 2]> = (7..=10)
        .flat_map(|x| (8..=Z1).map(move |z| [x, z]))
        .collect();
    let ops = plans::roof(&base, &dormer, 1, Style::Gable, Some(false), roof);
    lay(&mut base, ops);
    build(&mut base, &cx, &blocks);
    // A deck on four posts, and stairs up to it.
    let deck = wall(2);
    for at in [[17, 0, 5], [22, 0, 5], [17, 0, 10], [22, 0, 10]] {
        let ops = plans::post(&base, IVec3::from_array(at), deck, footings());
        lay(&mut base, ops);
    }
    let ops = plans::floor(
        &base,
        IVec3::new(17, 1, 5),
        IVec3::new(22, 1, 10),
        deck,
        footings(),
    );
    lay(&mut base, ops);
    let ops = plans::flight(
        &base,
        IVec3::new(18, -1, 17),
        Facing::North,
        terra_build::cells(1.0),
        Shape::Stair(Facing::North),
        deck,
        footings(),
    );
    lay(&mut base, ops);
    build(&mut base, &cx, &blocks);
    base
}

/// Builds everything laid, as a builder standing by keeps filling: what is being carved
/// out, what is changing kind, then every part a layer at a time from the bottom, a block
/// kind a cube at a time where its layer at once is refused.
pub(crate) fn build(base: &mut Base, cx: &Context, blocks: &dyn Fn(Kind) -> bool) {
    let near = Vec3::ZERO;
    for _ in 0..100_000 {
        if let Some(op) = base.next_carve(near, blocks)
            && base.act(&op, cx).is_ok()
        {
            continue;
        }
        if let Some(op) = base.next_change(near, blocks)
            && base.act(&op, cx).is_ok()
        {
            continue;
        }
        let mut parts: Vec<(i32, terra_build::PartId, Vec<[i32; 3]>)> = base
            .parts()
            .filter_map(|(id, _)| {
                let cubes = base.next_in(id, near, false)?;
                Some((cubes[0][1], id, cubes))
            })
            .collect();
        parts.sort_by_key(|(y, id, _)| (*y, *id));
        let mut done = false;
        'parts: for (_, _, cubes) in parts {
            if base
                .act(
                    &Op::Fill {
                        cubes: cubes.clone(),
                    },
                    cx,
                )
                .is_ok()
            {
                done = true;
                break;
            }
            for cube in cubes {
                if base.act(&Op::Fill { cubes: vec![cube] }, cx).is_ok() {
                    done = true;
                    break 'parts;
                }
            }
        }
        if !done {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_build::Fill;

    #[test]
    fn the_cabin_is_built_whole_with_its_storey_dormer_and_deck() {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let read = |path: &str| {
            std::fs::read_to_string(assets.join(path)).map_err(|why| format!("{path}: {why}"))
        };
        let kits = Kits(
            ["logs", "hewn", "stone", "shakes", "slate"]
                .iter()
                .map(|name| {
                    terra_build::kit::Kit::read(&format!("data/kits/{name}.ron"), &read).unwrap()
                })
                .collect(),
        );
        let logs = kits.0[0].kind();
        let hewn = kits.0[1].kind();
        let base = cabin(
            &kits,
            &[logs],
            hewn,
            &|[x, z]| (-(x as f32) * 2.0 - z as f32) as i16,
            false,
        );
        // Everything laid is built: the film's fills leave nothing waiting.
        let waiting: Vec<_> = base
            .cubes()
            .filter(|(_, cube)| cube.fill != Fill::Open && !cube.is_built())
            .map(|(at, _)| at)
            .collect();
        assert!(
            waiting.is_empty(),
            "{} not built: {:?}",
            waiting.len(),
            &waiting[..waiting.len().min(8)]
        );
        // A storey over the first, a roof over it with a dormer standing out of its south
        // slope, a deck on posts, the west wall changed and carved.
        assert!(base.cube(IVec3::new(X1, 8, 5)).is_some(), "the storey");
        let roof = |at: [i32; 3]| {
            base.cube(IVec3::from_array(at))
                .is_some_and(|c| c.kind.is_roof())
        };
        assert!(
            (12..20).any(|y| roof([8, y, Z1])),
            "the dormer at the eaves"
        );
        assert!(base.cube(IVec3::new(20, 5, 7)).is_some(), "the deck");
        assert_eq!(base.cube(IVec3::new(X0, 1, 6)).map(|c| c.kind), Some(hewn));
        assert!(
            base.cube(IVec3::new(X0, 2, 6))
                .is_none_or(|c| c.fill == Fill::Open),
            "the carve"
        );
    }
}
