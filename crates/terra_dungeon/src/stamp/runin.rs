//! The run-in: what the game spawns in the laid dungeon. Door leaves in the doorways of the
//! links that have doors, a chest against a wall of each room with loot, the mobs of each room
//! on free floor away from its doors, torches on walls (a room's wall decorations first, a
//! corridor's every third cell, about 30 lights in all), the start on the surface and a tour
//! for a camera, from the stair down by the longest way that enters no room twice to the goal.

use super::modules::{Class, Look};
use super::view::Room;
use super::{
    CELL, CUBE, ChestSpawn, DOORWAY_M, Frame, INNER, LEVEL, LIGHTS, Light, LightKind, MID,
    MobSpawn, Object, ObjectRole, Stamp, clear, cube_of, floor_of, hash, stands, steps,
};
use crate::Dungeon;
use crate::door::Lock;
use crate::graph::{Graph, Link};
use crate::place::{Layout, Side, Space, opposite};
use crate::theme::Role;
use glam::{IVec3, Vec3};
use std::collections::{HashMap, VecDeque};
use terra_build::Cube;

type Map = HashMap<IVec3, Cube>;

pub(super) fn run_in(dungeon: &Dungeon, looks: &[Look], map: &Map, out: &mut Stamp) {
    doors(dungeon, map, out);
    chests(dungeon, looks, map, out);
    mobs(dungeon, looks, map, out);
    torches(dungeon, looks, out);
    tour(dungeon, out);
    let room = Room::new(map, &out.objects);
    out.route = walked(map, &room, &out.route);
    out.view = room.view(&out.route);
    out.look = super::look::looks(map, &out.view, &out.objects);
    out.pace = super::look::pace(&out.view, &out.look);
}

/// A leaf in the doorway at the far room of each door, locked door and one-way door; an arch
/// and a secret way have none.
fn doors(dungeon: &Dungeon, map: &Map, out: &mut Stamp) {
    let layout = &dungeon.layout;
    for (e, edge) in dungeon.graph.edges.iter().enumerate() {
        let (name, lock) = match edge.link {
            Link::Door => ("door", Lock::None),
            Link::Locked(_) => ("locked_door", Lock::Keys),
            // Barred on `b`'s side: the room side when the leaf hangs in `b`.
            Link::OneWay => ("one_way_door", Lock::Barred { inside: true }),
            Link::Corridor | Link::Secret => continue,
        };
        let Some((room, (at, d))) = [edge.b, edge.a]
            .iter()
            .find_map(|&room| Some((room, doorway(layout, room, e)?)))
        else {
            continue;
        };
        let lock = match lock {
            Lock::Barred { .. } => Lock::Barred {
                inside: room == edge.b,
            },
            lock => lock,
        };
        let sill = Frame::new(at, d).point(MID, 0.0, 0.0);
        out.objects.push(Object {
            name: name.to_string(),
            role: ObjectRole::Door,
            at: sill,
            facing: d,
            model: "models/openings/StuddedDoor.glb".to_string(),
            lock,
            opens: opens(map, sill, d),
        });
    }
}

/// A doorway's frame in the world: across (+X, the first leaf's jamb at -X) and into its
/// room (the doorway's -Z), from the side it is in.
pub(super) fn doorway_axes(facing: usize) -> (Vec3, Vec3) {
    let f = Frame::new(IVec3::ZERO, facing);
    let into = (f.point(0.0, 0.0, 1.0) - f.point(0.0, 0.0, 0.0)).normalize();
    (Vec3::Y.cross(-into), into)
}

/// How far each leaf of the door at `sill` swings into its room (`Object::opens`): folded
/// flat on its wall where the wall runs on a leaf's width past the jamb with the room clear in
/// front of it, as a door in a thick wall swings back to the wall; otherwise a quarter turn,
/// onto a side wall flush with the jamb where one runs a leaf's width into the room, else
/// standing free. Only where the room is solid just inside the jamb (a wall or pier narrower
/// than the doorway) or where the leaf would stand does it turn out of the room however it is
/// pushed, along the jamb onto the corridor's wall. On 4 m cells a 1.5 m leaf is longer than the metre of wall between
/// doorways in neighbouring cells, and a doorway or an alcove beside it leaves it nothing to
/// fold onto: turned out of the room there instead, 7% of leaves swung towards a body pushing
/// from the corridor, against D512's "away from whoever pushes".
fn opens(map: &Map, sill: Vec3, facing: usize) -> [f32; 2] {
    let (across, into) = doorway_axes(facing);
    // Up the leaf: its foot, middle and head, in cubes.
    let high = DOORWAY_M[1] / CUBE;
    let ups = [1.5, high * 0.5 + 0.5, high - 0.5];
    let wide = (DOORWAY_M[0] * 0.5 / CUBE).round() as i32;
    // A leaf lies along a face at `at(k)` for each of `ks` cubes and each height: the cube
    // `behind` it solid, its own clear.
    let lies = |at: &dyn Fn(i32) -> Vec3, behind: Vec3, ks: std::ops::Range<i32>| {
        ks.into_iter().all(|k| {
            ups.iter().all(|&up| {
                let p = at(k) + Vec3::Y * CUBE * up;
                !clear(map, cube_of(p + behind * CUBE * 0.5))
                    && clear(map, cube_of(p - behind * CUBE * 0.5))
            })
        })
    };
    [-1.0, 1.0].map(|side| {
        let jamb = sill + across * side * DOORWAY_M[0] * 0.5;
        let out = across * side;
        // Along the room's face of its wall, past the jamb.
        let wall = |k: i32| jamb + out * CUBE * (k as f32 + 0.5) + into * CUBE;
        // Along the jamb's line into the room, past the wall's thickness.
        let side_wall = |k: i32| jamb + into * CUBE * (k as f32 + 1.5);
        let inside = jamb - out * CUBE * 0.5 + into * CUBE * 1.5;
        let blocked = ups
            .iter()
            .any(|&up| !clear(map, cube_of(inside + Vec3::Y * CUBE * up)));
        if blocked {
            -1.0
        } else if lies(&wall, -into, 0..wide) {
            2.0
        } else if (0..wide).all(|k| {
            let p = side_wall(k) - out * CUBE * 0.5;
            ups.iter()
                .all(|&up| clear(map, cube_of(p + Vec3::Y * CUBE * up)))
        }) {
            1.0
        } else {
            -1.0
        }
    })
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
                .map(move |d| Frame::new(at, d).point(MID, 0.0, 0.0))
        })
        .collect();
    let mut out = Vec::new();
    for &(at, _) in cells.iter().filter(|&&(_, c)| free(looks[c].class)) {
        let f = Frame::new(at, 0);
        for x in 1..=INNER {
            for z in 1..=INNER {
                let Some(y) = [0, -1].into_iter().find(|&y| stands(map, f.cube(x, y, z))) else {
                    continue;
                };
                let p = f.point(x as f32 + 0.5, y as f32, z as f32 + 0.5);
                let reach = doors.iter().map(|d| d.distance(p)).fold(100.0, f32::min);
                let wet = if y == 0 { 0.0 } else { 3.0 };
                let jitter = (hash(at, (x * CELL + z) as u32) % 100) as f32 * 0.003;
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
            let p = f.point(MID, 0.0, 1.8);
            let fits = (CELL / 2 - 1..=CELL / 2).all(|x| stands(map, f.cube(x, 0, 1)));
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

/// How far either side of a doorway's middle the tour passes square through it, in metres.
const DOOR_STEP_M: f32 = 0.9;

fn torch(at: IVec3, d: usize) -> Light {
    Light {
        at: Frame::new(at, d).point(MID, 4.0, 1.3),
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
            // Half a cube over the steps at the cell's middle (`shell::steps`).
            Some(i @ (0 | 1)) => (i as i32 * CELL) as f32 + MID - 1.5,
            _ => 0.0,
        },
        _ => 0.0,
    };
    Frame::new(cell.at, 0).point(MID, y, MID)
}

/// The start at the stair's top on the surface, facing down it; then down the stair and room
/// to room by the graph's longest way from the entrance to the goal that enters no room twice
/// (`showcase`), so it walks the loop the key rooms hang from and never turns round in a dead
/// end: round 20's films turned on the spot for 2-7 s at a key room's back wall, the tour
/// walking in to the key and out the way it came. A key room is seen from its doorway as the
/// tour passes the room it opens off, its key drawing the eye (`look::INTEREST_M`).
fn tour(dungeon: &Dungeon, out: &mut Stamp) {
    let (layout, graph) = (&dungeon.layout, &dungeon.graph);
    let Some(stair) = layout.stairs.iter().find(|s| s.edge.is_none()) else {
        return;
    };
    let top = stair.cells[stair.cells.len() - 1];
    out.start = Frame::new(top, stair.up).point(MID, LEVEL as f32, 0.5);
    out.start_facing = opposite(stair.up);
    out.route.push(out.start);
    for &at in stair.cells.iter().rev() {
        out.route
            .extend(layout.cell(at).map(|c| cell_point(layout, c)));
    }
    let foot = stair.cells.first().and_then(|&at| layout.cell(at));
    let into = foot.and_then(|c| {
        layout
            .neighbours(c)
            .find(|&n| layout.cells[n].space == Space::Room(graph.entrance()))
            .map(|n| (c, n))
    });
    if let Some((c, n)) = into {
        through(layout, c, n, &mut out.route);
    }
    let mut room = centre(layout, graph.entrance());
    for (e, a, b) in showcase(graph, graph.entrance(), graph.goal()) {
        let mut leg = Vec::new();
        let cells = cells_between(layout, e, a, b);
        for pair in cells.windows(2) {
            through(layout, pair[0], pair[1], &mut leg);
            if !matches!(layout.cells[pair[1]].space, Space::Room(_)) {
                leg.push(cell_point(layout, pair[1]));
            }
        }
        if let (Some(&from), Some(&to)) = (out.route.last(), leg.first()) {
            out.route.push(across(from, room, to));
        }
        out.route.extend(leg);
        room = centre(layout, b);
    }
    // It ends at the goal (its plinth's foot, `walked`), not the goal room's middle: in a
    // room of 4 m cells the goal's cell middle stands 2 m or more off the room's, and the look
    // at the way on toward the middle held the goal 0.42 rad off where the film stops 3.5 m
    // short of it (D527's first build; the look's pull keeps the way on in frame).
    let goal = out.objects.iter().find(|o| o.role == ObjectRole::Goal);
    out.route
        .push(goal.map_or(room, |g| Vec3::new(g.at.x, room.y, g.at.z)));
}

/// Where the tour crosses a room, from where it came in to where it goes out: the room's
/// middle, or short of it where the way out is beside the way in, so the way turns no
/// sharper than a right angle there (on a circle through both, Thales) and never runs back
/// along itself: round 21's tours went in to a room's middle and back out beside the door
/// they came in by, 17° apart.
fn across(from: Vec3, middle: Vec3, to: Vec3) -> Vec3 {
    let between = (from + to) * 0.5;
    let reach = (from.distance(to) * 0.5).min(between.distance(middle));
    let toward = (middle - between).normalize_or_zero();
    let at = between + toward * reach;
    Vec3::new(at.x, middle.y, at.z)
}

/// The longest way by the graph's links from one room to another that enters no room twice,
/// secrets left out, locks open and one-way doors only their way (a showcase walk, not a
/// player's): each step's edge, from and to. Of ways as long, the first found. A graph is
/// some 16 rooms, most of them dead ends, so every such way is tried.
pub(crate) fn showcase(graph: &Graph, from: usize, to: usize) -> Vec<(usize, usize, usize)> {
    fn walk(
        graph: &Graph,
        at: usize,
        to: usize,
        seen: &mut [bool],
        way: &mut Vec<(usize, usize, usize)>,
        best: &mut Option<Vec<(usize, usize, usize)>>,
    ) {
        if at == to {
            if best.as_ref().is_none_or(|b| way.len() > b.len()) {
                *best = Some(way.clone());
            }
            return;
        }
        for (e, edge) in graph.edges.iter().enumerate() {
            if edge.link == Link::Secret {
                continue;
            }
            for (a, b) in edge.ways() {
                if a == at && !seen[b] {
                    seen[b] = true;
                    way.push((e, a, b));
                    walk(graph, b, to, seen, way, best);
                    way.pop();
                    seen[b] = false;
                }
            }
        }
    }
    let mut seen = vec![false; graph.rooms.len()];
    seen[from] = true;
    let mut best = None;
    walk(graph, from, to, &mut seen, &mut Vec::new(), &mut best);
    best.unwrap_or_default()
}

/// From one cell into the next of another space, through the doorway between them: a step
/// short of it, its middle and a step past, so a path smoothed through the tour (a camera's)
/// passes the doorway square, not across its jamb.
fn through(layout: &Layout, from: usize, to: usize, route: &mut Vec<Vec3>) {
    if layout.cells[from].space == layout.cells[to].space {
        return;
    }
    let (a, b) = (cell_point(layout, from), cell_point(layout, to));
    let door = (a + b) * 0.5;
    let step = (b - a).normalize_or_zero() * DOOR_STEP_M;
    route.extend([door - step, door, door + step]);
}

/// The corridor and stair cells of an edge in walking order from room `a` to room `b`, after
/// the cell of `a` it leaves from and before the cell of `b` it comes into.
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
        path.push(c);
        at = back[c];
    }
    path.reverse();
    path
}

/// How high over the feet a tour's way must be clear: a body's chest and a camera's eye.
const CLEAR_AT_M: [f32; 2] = [1.0, 1.6];
/// How far round a leg's ends the way round what stands in it is looked for, in cubes.
const ROUND_CUBES: i32 = 16;

/// Whether the way from one point to another, at chest and eye height over it, passes no
/// cube that is not open.
pub(super) fn sight(map: &Map, a: Vec3, b: Vec3) -> bool {
    let steps = ((b - a).length() / 0.1).ceil().max(1.0) as i32;
    (0..=steps).all(|k| {
        let p = a.lerp(b, k as f32 / steps as f32);
        CLEAR_AT_M
            .iter()
            .all(|&up| clear(map, cube_of(p + Vec3::Y * up)))
    })
}

/// The tour walked: each point past the surface moved to the nearest place with `ROOM_M`
/// round it if it has not; each leg that a pillar, a plinth or a heap stands
/// in, or that passes nearer a wall, a pillar's corner or an open leaf than `ROOM_M`, goes
/// round it by the shortest walk over the cubes from end to end that keeps that room
/// (else by any walk), pulled taut to the fewest straight legs that keep it (else that are
/// clear): a navigation mesh's path, its agent radius and its string pulling. A leg with no
/// walk round is kept as it was.
fn walked(map: &Map, room: &Room, route: &[Vec3]) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = route.iter().take(2).copied().collect();
    for pair in route.windows(2).skip(1) {
        let a = *out.last().unwrap_or(&pair[0]);
        let mut b = room.settle(pair[1]);
        if !room.roomy(a, b) {
            let feet = |c: IVec3| (c.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CUBE;
            let (from_cube, to_cube) = (cube_of(a), cube_of(b));
            let roomy = |c: IVec3| room.roomy_at(feet(c));
            let mut cubes = round(map, from_cube, to_cube, &roomy);
            if cubes.last() != Some(&to_cube) {
                cubes = round(map, from_cube, to_cube, &|_| true);
            }
            let path: Vec<Vec3> = cubes.into_iter().map(feet).collect();
            // An end on what stands in the room (the goal's plinth) moves to the nearest
            // place a walker stands.
            if let Some(&last) = path.last().filter(|_| !stands(map, cube_of(b))) {
                b = last;
            }
            let mut from = a;
            let mut i = 0;
            while i + 1 < path.len() {
                if room.roomy(from, b) {
                    break;
                }
                let far = |ok: &dyn Fn(Vec3, Vec3) -> bool| {
                    (i + 1..path.len()).rev().find(|&j| ok(from, path[j]))
                };
                let far = far(&|p, q| room.roomy(p, q))
                    .or_else(|| far(&|p, q| sight(map, p, q)))
                    .unwrap_or(i + 1);
                from = path[far];
                out.push(from);
                i = far;
            }
        }
        out.push(b);
    }
    unfolded(map, room, out)
}

/// The walked tour with no point where it folds back on itself, turning more than 120°: a
/// point settled past the next, or a walk round one leg starting back along the last
/// (round 21, a crypt's doorway: a metre on and back), left out where the leg past it keeps
/// its room, or is clear at least.
fn unfolded(map: &Map, room: &Room, route: Vec<Vec3>) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = Vec::with_capacity(route.len());
    for b in route {
        while let [.., p, m] = out[..] {
            let back = (m - p).normalize_or_zero().dot((b - m).normalize_or_zero()) < -0.5;
            if !(back && out.len() > 2 && (room.roomy(p, b) || sight(map, p, b))) {
                break;
            }
            out.pop();
        }
        out.push(b);
    }
    out
}

/// The shortest walk over the cubes from one standing place to another, near them, through
/// places `ok` lets it (the first always); or to the place nearest it, if no walker reaches
/// it.
fn round(map: &Map, from: IVec3, to: IVec3, ok: &dyn Fn(IVec3) -> bool) -> Vec<IVec3> {
    let (low, high) = (
        from.min(to) - IVec3::splat(ROUND_CUBES),
        from.max(to) + IVec3::splat(ROUND_CUBES),
    );
    let mut back: HashMap<IVec3, IVec3> = HashMap::from([(from, from)]);
    let mut queue = VecDeque::from([from]);
    let mut nearest = from;
    let away = |p: IVec3| (p - to).length_squared();
    while let Some(p) = queue.pop_front() {
        if away(p) < away(nearest) {
            nearest = p;
        }
        if p == to {
            break;
        }
        for n in steps(map, p) {
            if n.cmpge(low).all() && n.cmple(high).all() && !back.contains_key(&n) && ok(n) {
                back.insert(n, p);
                queue.push_back(n);
            }
        }
    }
    let mut path = vec![nearest];
    let mut at = nearest;
    while at != from {
        at = back[&at];
        path.push(at);
    }
    path.reverse();
    path
}
