//! The run-in: what the game spawns in the laid dungeon. Door leaves in the doorways of the
//! links that have doors, a chest against a wall of each room with loot, the mobs of each room
//! on free floor away from its doors, torches on walls (a room's wall decorations first, a
//! corridor's every third cell, about 30 lights in all), the start on the surface and a tour
//! for a camera, from the stair down through the key rooms to the goal.

use super::modules::{Class, Look};
use super::{
    CELL, CUBE, ChestSpawn, Frame, LEVEL, LIGHTS, Light, LightKind, MobSpawn, Object, ObjectRole,
    Stamp, floor_of, hash, stands,
};
use crate::Dungeon;
use crate::graph::{Graph, Link};
use crate::place::{Layout, Side, Space, opposite};
use crate::theme::Role;
use glam::{IVec3, Vec3};
use std::collections::{HashMap, VecDeque};
use terra_build::Cube;

type Map = HashMap<IVec3, Cube>;

pub(super) fn run_in(dungeon: &Dungeon, looks: &[Look], map: &Map, out: &mut Stamp) {
    doors(dungeon, out);
    chests(dungeon, looks, map, out);
    mobs(dungeon, looks, map, out);
    torches(dungeon, looks, out);
    tour(dungeon, out);
}

/// A leaf in the doorway at the far room of each door, locked door and one-way door; an arch
/// and a secret way have none.
fn doors(dungeon: &Dungeon, out: &mut Stamp) {
    let layout = &dungeon.layout;
    for (e, edge) in dungeon.graph.edges.iter().enumerate() {
        let (name, locked) = match edge.link {
            Link::Door => ("door", false),
            Link::Locked(_) => ("locked_door", true),
            Link::OneWay => ("one_way_door", false),
            Link::Corridor | Link::Secret => continue,
        };
        let Some((at, d)) = [edge.b, edge.a]
            .iter()
            .find_map(|&room| doorway(layout, room, e))
        else {
            continue;
        };
        out.objects.push(Object {
            name: name.to_string(),
            role: ObjectRole::Door,
            at: Frame::new(at, d).point(3.0, 0.0, 0.0),
            facing: d,
            model: "models/openings/StuddedDoor.glb".to_string(),
            locked,
        });
    }
}

/// The room's cell and side with the doorway of an edge.
fn doorway(layout: &Layout, room: usize, e: usize) -> Option<(IVec3, usize)> {
    layout.rooms[room].cells().find_map(|at| {
        let cell = &layout.cells[layout.cell(at)?];
        (0..4)
            .find(|&d| cell.sides[d] == Side::Door(e))
            .map(|d| (at, d))
    })
}

/// Whether mobs and chests may stand in a cell of this class.
fn free(class: Class) -> bool {
    !matches!(
        class,
        Class::Collapse | Class::Goal | Class::Key | Class::Rest | Class::Stair
    )
}

/// Where things already stand: props, objects, fires and chests.
fn taken(out: &Stamp) -> Vec<Vec3> {
    let fires = out.lights.iter().filter(|l| l.kind != LightKind::Torch);
    out.props
        .iter()
        .map(|p| p.at)
        .chain(out.objects.iter().map(|o| o.at))
        .chain(fires.map(|l| l.at))
        .chain(out.chests.iter().map(|c| c.at))
        .collect()
}

/// Standing spots on a room's free floor, the farthest from its doorways first, dry before
/// wet.
fn spots(dungeon: &Dungeon, looks: &[Look], map: &Map, room: usize) -> Vec<(Vec3, f32)> {
    let layout = &dungeon.layout;
    let rect = layout.rooms[room];
    let cells: Vec<(IVec3, usize)> = rect
        .cells()
        .filter_map(|at| layout.cell(at).map(|c| (at, c)))
        .collect();
    let doors: Vec<Vec3> = cells
        .iter()
        .flat_map(|&(at, c)| {
            let sides = layout.cells[c].sides;
            (0..4)
                .filter(move |&d| matches!(sides[d], Side::Door(_) | Side::Way))
                .map(move |d| Frame::new(at, d).point(3.0, 0.0, 0.0))
        })
        .collect();
    let mut out = Vec::new();
    for &(at, _) in cells.iter().filter(|&&(_, c)| free(looks[c].class)) {
        let f = Frame::new(at, 0);
        for x in 1..=4 {
            for z in 1..=4 {
                let Some(y) = [0, -1].into_iter().find(|&y| stands(map, f.cube(x, y, z))) else {
                    continue;
                };
                let p = f.point(x as f32 + 0.5, y as f32, z as f32 + 0.5);
                let reach = doors.iter().map(|d| d.distance(p)).fold(100.0, f32::min);
                let wet = if y == 0 { 0.0 } else { 3.0 };
                let jitter = (hash(at, (x * 6 + z) as u32) % 100) as f32 * 0.003;
                out.push((p, reach - wet + jitter));
            }
        }
    }
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out
}

/// The best spot at least `gap` from everything taken, closer if none is.
fn pick(spots: &[(Vec3, f32)], taken: &mut Vec<Vec3>, gap: f32) -> Option<Vec3> {
    for gap in [gap, gap * 0.5, 0.0] {
        let found = spots
            .iter()
            .find(|(p, _)| taken.iter().all(|t| t.distance(*p) > gap));
        if let Some(&(p, _)) = found {
            taken.push(p);
            return Some(p);
        }
    }
    None
}

/// A chest against a wall of each room with loot, facing in, holding the room's loot.
fn chests(dungeon: &Dungeon, looks: &[Look], map: &Map, out: &mut Stamp) {
    let layout = &dungeon.layout;
    let mut taken = taken(out);
    for (r, room) in dungeon.graph.rooms.iter().enumerate() {
        if room.loot.is_empty() {
            continue;
        }
        let mut walls: Vec<(u32, IVec3, usize)> = Vec::new();
        for at in layout.rooms[r].cells() {
            let Some(c) = layout.cell(at) else { continue };
            let plain = matches!(
                looks[c].class,
                Class::Plain | Class::Runner | Class::Decor | Class::Timber | Class::Pillar
            );
            for d in (0..4).filter(|&d| plain && layout.cells[c].sides[d] == Side::Wall) {
                walls.push((hash(at, 200 + d as u32), at, d));
            }
        }
        walls.sort_by_key(|&(rank, at, d)| (rank, at.to_array(), d));
        let against = walls.iter().find_map(|&(_, at, d)| {
            let f = Frame::new(at, d);
            let p = f.point(3.0, 0.0, 1.8);
            let fits = (2..=3).all(|x| stands(map, f.cube(x, 0, 1)));
            (fits && taken.iter().all(|t| t.distance(p) > 1.0)).then_some((p, opposite(d)))
        });
        let fallback = || spots(dungeon, looks, map, r).first().map(|&(p, _)| (p, 2));
        let Some((at, facing)) = against.or_else(fallback) else {
            continue;
        };
        taken.push(at);
        out.chests.push(ChestSpawn {
            at,
            facing,
            items: room
                .loot
                .iter()
                .map(|l| (l.item.clone(), l.count))
                .collect(),
            room: r,
        });
    }
}

/// Each room's mobs, a spot each, apart and away from the doors.
fn mobs(dungeon: &Dungeon, looks: &[Look], map: &Map, out: &mut Stamp) {
    let mut taken = taken(out);
    for (r, room) in dungeon.graph.rooms.iter().enumerate() {
        if room.mobs.is_empty() {
            continue;
        }
        let spots = spots(dungeon, looks, map, r);
        for group in &room.mobs {
            for _ in 0..group.count {
                if let Some(at) = pick(&spots, &mut taken, 1.5) {
                    out.mobs.push(MobSpawn {
                        species: group.species.clone(),
                        at,
                        room: r,
                    });
                }
            }
        }
    }
}

fn torch(at: IVec3, d: usize) -> Light {
    Light {
        at: Frame::new(at, d).point(3.0, 4.0, 1.3),
        kind: LightKind::Torch,
        facing: opposite(d),
    }
}

/// A walled side of a cell, from a repeatable pick.
fn wall_of(layout: &Layout, c: usize, salt: u32) -> Option<usize> {
    let cell = &layout.cells[c];
    let start = hash(cell.at, salt) as usize;
    (0..4)
        .map(|k| (start + k) % 4)
        .find(|&d| cell.sides[d] == Side::Wall)
}

/// Torches: one in each room (none in the rest room, which has its fire), on a wall
/// decoration first; then one every third corridor cell; then a second in rooms of six cells
/// or more; until the dungeon has `LIGHTS`.
fn torches(dungeon: &Dungeon, looks: &[Look], out: &mut Stamp) {
    let layout = &dungeon.layout;
    let mut rooms: Vec<Vec<(IVec3, usize)>> = Vec::new();
    for (r, rect) in layout.rooms.iter().enumerate() {
        let mut found: Vec<(u32, IVec3, usize)> = Vec::new();
        for at in rect.cells() {
            let Some(c) = layout.cell(at) else { continue };
            let look = looks[c];
            if !free(look.class) || matches!(look.class, Class::Alcove | Class::Shore) {
                continue;
            }
            let decorated = look.class == Class::Decor;
            let decor = Some(look.turn).filter(|&d| layout.cells[c].sides[d] == Side::Wall);
            let decor = decor.filter(|_| decorated);
            if let Some(d) = decor.or_else(|| wall_of(layout, c, 300)) {
                let rank = if decor.is_some() {
                    0
                } else {
                    1 + hash(at, 301) % 1000
                };
                found.push((rank, at, d));
            }
        }
        found.sort_by_key(|&(rank, at, d)| (rank, at.to_array(), d));
        if dungeon.graph.rooms[r].role == Role::Rest {
            found.clear();
        }
        rooms.push(found.into_iter().map(|(_, at, d)| (at, d)).collect());
    }
    let mut lit = Vec::new();
    lit.extend(rooms.iter().filter_map(|found| found.first().copied()));
    for run in &layout.corridors {
        for (k, &at) in run.iter().enumerate().filter(|(k, _)| k % 3 == 1) {
            if let Some(d) = layout
                .cell(at)
                .and_then(|c| wall_of(layout, c, 310 + k as u32))
            {
                lit.push((at, d));
            }
        }
    }
    for (r, found) in rooms.iter().enumerate() {
        let big = layout.rooms[r].size.element_product() >= 6;
        let Some(&(first, _)) = found.first() else {
            continue;
        };
        let far = found
            .iter()
            .filter(|(at, _)| *at != first)
            .max_by_key(|(at, _)| (*at - first).abs().element_sum());
        if let (true, Some(&second)) = (big, far) {
            lit.push(second);
        }
    }
    let room = LIGHTS.saturating_sub(out.lights.len());
    out.lights
        .extend(lit.into_iter().take(room).map(|(at, d)| torch(at, d)));
}

/// A room's middle on its floor.
fn centre(layout: &Layout, room: usize) -> Vec3 {
    let rect = layout.rooms[room];
    let c = rect.centre() * (CELL as f32 * CUBE);
    Vec3::new(c.x, floor_of(rect.min.y) as f32 * CUBE, c.y)
}

/// A corridor or stair cell's middle where a walker's feet are.
fn cell_point(layout: &Layout, c: usize) -> Vec3 {
    let cell = &layout.cells[c];
    let y = match cell.space {
        Space::Stair(s) => match layout.stairs[s].cells.iter().position(|&a| a == cell.at) {
            Some(0) => 1.5,
            Some(1) => LEVEL as f32 - 0.5,
            _ => 0.0,
        },
        _ => 0.0,
    };
    Frame::new(cell.at, 0).point(3.0, y, 3.0)
}

/// The start at the stair's top on the surface, facing down it; then down the stair and room
/// to room by the graph to each key room, the lock shut, and on to the goal.
fn tour(dungeon: &Dungeon, out: &mut Stamp) {
    let (layout, graph) = (&dungeon.layout, &dungeon.graph);
    let Some(stair) = layout.stairs.iter().find(|s| s.edge.is_none()) else {
        return;
    };
    let top = stair.cells[stair.cells.len() - 1];
    out.start = Frame::new(top, stair.up).point(3.0, LEVEL as f32, 0.5);
    out.start_facing = opposite(stair.up);
    out.route.push(out.start);
    for &at in stair.cells.iter().rev() {
        out.route
            .extend(layout.cell(at).map(|c| cell_point(layout, c)));
    }
    out.route.push(centre(layout, graph.entrance()));
    let mut keys: Vec<usize> = (0..graph.rooms.len())
        .filter(|&r| graph.rooms[r].role == Role::Puzzle)
        .collect();
    keys.sort_by_key(|&r| graph.rooms[r].depth);
    let legs = keys
        .iter()
        .map(|&r| (r, true))
        .chain([(graph.goal(), false)]);
    let mut here = graph.entrance();
    for (to, shut) in legs {
        let Some(steps) = rooms_between(graph, here, to, shut) else {
            continue;
        };
        for (e, a, b) in steps {
            let cells = cells_between(layout, e, a, b);
            out.route
                .extend(cells.into_iter().map(|c| cell_point(layout, c)));
            out.route.push(centre(layout, b));
        }
        here = to;
    }
}

/// The rooms from one to another by the graph's ways, secrets left out and the lock shut if
/// asked: each step's edge, from and to.
fn rooms_between(
    graph: &Graph,
    from: usize,
    to: usize,
    shut: bool,
) -> Option<Vec<(usize, usize, usize)>> {
    let mut back: Vec<Option<(usize, usize)>> = vec![None; graph.rooms.len()];
    let mut seen = vec![false; graph.rooms.len()];
    seen[from] = true;
    let mut queue = VecDeque::from([from]);
    while let Some(r) = queue.pop_front() {
        for (e, edge) in graph.edges.iter().enumerate() {
            let locked = matches!(edge.link, Link::Locked(_));
            if edge.link == Link::Secret || (shut && locked) {
                continue;
            }
            for (a, b) in edge.ways() {
                if a == r && !seen[b] {
                    seen[b] = true;
                    back[b] = Some((e, a));
                    queue.push_back(b);
                }
            }
        }
    }
    let mut steps = Vec::new();
    let mut r = to;
    while r != from {
        let (e, a) = back[r]?;
        steps.push((e, a, r));
        r = a;
    }
    steps.reverse();
    Some(steps)
}

/// The corridor and stair cells of an edge in walking order from room `a` to room `b`.
fn cells_between(layout: &Layout, e: usize, a: usize, b: usize) -> Vec<usize> {
    let own = |c: usize| match layout.cells[c].space {
        Space::Room(r) => r == a || r == b,
        Space::Corridor(x) => x == e,
        Space::Stair(s) => layout.stairs[s].edge == Some(e),
    };
    let mut back: Vec<Option<usize>> = vec![None; layout.cells.len()];
    let mut seen = vec![false; layout.cells.len()];
    let mut queue = VecDeque::new();
    for at in layout.rooms[a].cells() {
        if let Some(c) = layout.cell(at) {
            seen[c] = true;
            queue.push_back(c);
        }
    }
    let mut end = None;
    while let Some(c) = queue.pop_front() {
        if layout.cells[c].space == Space::Room(b) {
            end = Some(c);
            break;
        }
        for next in layout.neighbours(c) {
            if !seen[next] && own(next) {
                seen[next] = true;
                back[next] = Some(c);
                queue.push_back(next);
            }
        }
    }
    let mut path = Vec::new();
    let mut at = end;
    while let Some(c) = at {
        if !matches!(layout.cells[c].space, Space::Room(_)) {
            path.push(c);
        }
        at = back[c];
    }
    path.reverse();
    path
}
