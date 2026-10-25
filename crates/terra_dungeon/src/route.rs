//! Corridors between placed rooms (D473): A* from every free cell beside one room to a free
//! cell beside the other, through free cells only, a turn costing more than a step so runs come
//! out straight, and a stair as one move of three cells along and a level up or down, which
//! takes two cells on both levels.

use crate::graph::Graph;
use crate::place::{Plan, STEPS, Side, Space, Stair, opposite};
use glam::IVec3;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

const STEP: u32 = 2;
const TURN: u32 = 3;
const STAIR: u32 = 12;
/// Cells around the rooms a corridor may wander through.
const MARGIN: i32 = 6;
/// Nodes looked at before a route is given up on.
const LIMIT: usize = 60_000;

#[derive(Clone, Copy)]
enum Move {
    /// The first cell, beside this cell of room `a`.
    Start(IVec3),
    Walk,
    /// Along this side, up (-1) or down (+1) a level.
    Stair(usize, i32),
}

struct Node {
    at: IVec3,
    facing: usize,
    from: Option<usize>,
    by: Move,
}

/// Routes the corridor for edge `e`, or says there is none.
pub(crate) fn route(plan: &mut Plan, graph: &Graph, e: usize) -> Option<()> {
    let edge = graph.edges[e];
    let (from, to) = (plan.rooms[edge.a], plan.rooms[edge.b]);
    let (low, high) = plan
        .rooms
        .iter()
        .fold((IVec3::MAX, IVec3::MIN), |(lo, hi), r| {
            (
                lo.min(r.min),
                hi.max(r.min + IVec3::new(r.size.x, 0, r.size.y)),
            )
        });
    let inside = |at: IVec3| {
        at.x >= low.x - MARGIN
            && at.x <= high.x + MARGIN
            && at.z >= low.z - MARGIN
            && at.z <= high.z + MARGIN
    };
    let free = |at: IVec3| inside(at) && plan.free(at);
    let guess = |at: IVec3| {
        (to.reach(at) - 1).max(0) as u32 * STEP + (at.y - to.min.y).unsigned_abs() * STAIR / 2
    };
    let beside_goal = |at: IVec3| at.y == to.min.y && to.reach(at) == 1;

    let mut nodes: Vec<Node> = Vec::new();
    let mut best: HashMap<(IVec3, usize), u32> = HashMap::new();
    let mut open = BinaryHeap::new();
    for room_cell in from.cells() {
        for (side, step) in STEPS.iter().enumerate() {
            let at = room_cell + *step;
            if free(at) && best.insert((at, side), 0).is_none() {
                nodes.push(Node {
                    at,
                    facing: side,
                    from: None,
                    by: Move::Start(room_cell),
                });
                open.push(Reverse((guess(at), 0, nodes.len() - 1)));
            }
        }
    }
    let mut end = None;
    while let Some(Reverse((_, cost, id))) = open.pop() {
        let (at, facing) = (nodes[id].at, nodes[id].facing);
        if best.get(&(at, facing)).is_some_and(|&b| b < cost) {
            continue;
        }
        if beside_goal(at) {
            end = Some(id);
            break;
        }
        if nodes.len() > LIMIT {
            return None;
        }
        for (side, step) in STEPS.iter().enumerate() {
            if side == opposite(facing) {
                continue;
            }
            let turn = if side == facing { 0 } else { TURN };
            let mut moves = Vec::with_capacity(3);
            if free(at + *step) {
                moves.push((at + *step, STEP + turn, Move::Walk));
            }
            for dy in [-1, 1] {
                let other = at + IVec3::Y * dy;
                let well = [
                    at + *step,
                    at + *step * 2,
                    other + *step,
                    other + *step * 2,
                    other + *step * 3,
                ];
                if well.iter().all(|&c| free(c)) {
                    moves.push((other + *step * 3, STAIR + turn, Move::Stair(side, dy)));
                }
            }
            for (next, price, by) in moves {
                let cost = cost + price;
                if best.get(&(next, side)).is_none_or(|&b| cost < b) {
                    best.insert((next, side), cost);
                    nodes.push(Node {
                        at: next,
                        facing: side,
                        from: Some(id),
                        by,
                    });
                    open.push(Reverse((cost + guess(next), cost, nodes.len() - 1)));
                }
            }
        }
    }
    let mut path = Vec::new();
    let mut at = end;
    while let Some(id) = at {
        path.push(id);
        at = nodes[id].from;
    }
    path.reverse();
    lay(plan, &nodes, &path, e, to)
}

/// Lays a found path's cells, stairs and joins, unless it crosses itself.
fn lay(
    plan: &mut Plan,
    nodes: &[Node],
    path: &[usize],
    e: usize,
    to: crate::place::Rect,
) -> Option<()> {
    let mut run = Vec::new();
    let mut stairs = Vec::new();
    let mut joins = Vec::new();
    for (k, &id) in path.iter().enumerate() {
        let node = &nodes[id];
        run.push(node.at);
        match node.by {
            Move::Start(room) => joins.push((room, node.at, Side::Door(e))),
            Move::Walk => joins.push((nodes[path[k - 1]].at, node.at, Side::Open)),
            Move::Stair(side, dy) => {
                let prev = nodes[path[k - 1]].at;
                let step = STEPS[side];
                let (at, up) = if dy < 0 {
                    (prev + step, side)
                } else {
                    (prev + step * 2 + IVec3::Y, opposite(side))
                };
                let top = at + STEPS[up];
                let cells = vec![at, top, at - IVec3::Y, top - IVec3::Y];
                joins.push((at - STEPS[up], at, Side::Way));
                joins.push((at, top, Side::Open));
                joins.push((cells[2], cells[3], Side::Open));
                joins.push((cells[3], cells[3] + STEPS[up], Side::Way));
                stairs.push(Stair {
                    edge: Some(e),
                    at,
                    up,
                    cells,
                });
            }
        }
    }
    let last = *run.last()?;
    let door = (0..4).map(|d| last + STEPS[d]).find(|&c| to.contains(c))?;
    joins.push((last, door, Side::Door(e)));

    let mut seen = HashSet::new();
    let all = run
        .iter()
        .chain(stairs.iter().flat_map(|s: &Stair| s.cells.iter()));
    if !all.clone().all(|&c| seen.insert(c)) || !all.clone().all(|&c| plan.free(c)) {
        return None;
    }
    for &at in &run {
        plan.occupied.insert(at, Space::Corridor(e));
    }
    for stair in stairs {
        let index = plan.stairs.len();
        for &at in &stair.cells {
            plan.occupied.insert(at, Space::Stair(index));
        }
        plan.stairs.push(stair);
    }
    for (a, b, side) in joins {
        plan.join(a, b, side);
    }
    plan.corridors[e] = run;
    Some(())
}
