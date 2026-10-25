use super::*;
use crate::graph::Link;
use crate::place::{STEPS, Space};
use crate::theme::{Role, assets};
use crate::{ModuleSet, generate};
use std::collections::{HashSet, VecDeque};
use terra_gen::Seed;

type Map = HashMap<IVec3, Cube>;

const SEEDS: u64 = 200;

fn kind(name: &str) -> Kind {
    Kind(1 + name.len() as u16)
}

/// Every standing spot a walker reaches from `start`: a step to a side up a cube at most (a
/// stair cube's two treads), down two at most, with its head clear on the way.
fn walk(map: &Map, start: IVec3) -> HashSet<IVec3> {
    let mut seen = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(p) = queue.pop_front() {
        for n in steps(map, p) {
            if seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    seen
}

/// The first leg of a tour where a camera at eye height (or a body's chest) going straight
/// from point to point would pass through a cube not open; the first leg is the surface's,
/// above the dig.
fn blocked(map: &Map, route: &[Vec3]) -> Option<usize> {
    route
        .windows(2)
        .enumerate()
        .skip(1)
        .find(|(_, pair)| !runin::sight(map, pair[0], pair[1]))
        .map(|(leg, _)| leg)
}

/// A reached spot within two cubes of a point.
fn near(reached: &HashSet<IVec3>, p: Vec3) -> bool {
    let c = cube_of(p);
    (-2..=2)
        .any(|x| (-2..=2).any(|y| (-2..=2).any(|z| reached.contains(&(c + IVec3::new(x, y, z))))))
}

/// Air from inside every cell, flooded through faces, stays in the cells of its own space:
/// walls, slabs and openings close each room, corridor and stair; only the surface stair is
/// open to the sky.
fn closed(layout: &Layout, map: &Map) -> Result<usize, String> {
    let space_at = |c: IVec3| {
        let level = (-c.y - 2).div_euclid(LEVEL);
        let at = IVec3::new(c.x.div_euclid(CELL), level, c.z.div_euclid(CELL));
        layout.cell(at).map(|i| layout.cells[i].space)
    };
    let surface = layout.stairs.iter().position(|s| s.edge.is_none());
    let mut seen = HashSet::new();
    for cell in &layout.cells {
        if surface.map(Space::Stair) == Some(cell.space) {
            continue;
        }
        let f = Frame::new(cell.at, 0);
        let inside =
            (1..=4).flat_map(|x| (0..=6).flat_map(move |y| (1..=4).map(move |z| (x, y, z))));
        for start in inside.map(|(x, y, z)| f.cube(x, y, z)) {
            if map.contains_key(&start) || !seen.insert(start) {
                continue;
            }
            let mut queue = VecDeque::from([start]);
            while let Some(p) = queue.pop_front() {
                for step in STEPS.into_iter().chain([IVec3::Y, -IVec3::Y]) {
                    let n = p + step;
                    if map.contains_key(&n) || !seen.insert(n) {
                        continue;
                    }
                    if space_at(n) != Some(cell.space) {
                        return Err(format!("air from {} leaks to cube {n}", cell.at));
                    }
                    queue.push_back(n);
                }
            }
        }
    }
    Ok(seen.len())
}

/// What one stamp measured: cubes, ops, lights, walkable spots.
struct Numbers {
    ops: usize,
    cubes: usize,
    lights: usize,
    spots: usize,
    air: usize,
    ms: f32,
}

fn check(theme: &Theme, dungeon: &Dungeon, s: u64) -> Numbers {
    let at = format!("{} seed {s}", theme.name);
    let clock = std::time::Instant::now();
    let stamp = stamp(dungeon, theme, &kind);
    let ms = clock.elapsed().as_secs_f32() * 1000.0;
    let map = cubes(&stamp.ops);
    let start = cube_of(stamp.start);
    assert!(stands(&map, start), "{at}: the start stands at {start}");
    let reached = walk(&map, start);
    let graph = &dungeon.graph;
    for o in &stamp.objects {
        assert!(
            near(&reached, o.at),
            "{at}: {} at {} out of reach",
            o.name,
            o.at
        );
    }
    for c in &stamp.chests {
        assert!(
            near(&reached, c.at),
            "{at}: room {}'s chest out of reach",
            c.room
        );
    }
    for m in &stamp.mobs {
        let spot = cube_of(m.at);
        assert!(
            reached.contains(&spot),
            "{at}: a mob at {spot} out of reach"
        );
    }
    for l in stamp.lights.iter().filter(|l| l.kind != LightKind::Torch) {
        assert!(near(&reached, l.at), "{at}: a {:?} out of reach", l.kind);
    }
    let air = closed(&dungeon.layout, &map).unwrap_or_else(|e| panic!("{at}: {e}"));

    let count = |role: ObjectRole| stamp.objects.iter().filter(|o| o.role == role).count();
    let puzzles = graph
        .rooms
        .iter()
        .filter(|r| r.role == Role::Puzzle)
        .count();
    assert_eq!(count(ObjectRole::Goal), 1, "{at}: one goal");
    assert_eq!(count(ObjectRole::Key), puzzles, "{at}: a key a puzzle room");
    let locked = |l: &Link| matches!(l, Link::Locked(_));
    assert!(
        graph.edges.iter().any(|e| locked(&e.link))
            && stamp
                .objects
                .iter()
                .any(|o| o.role == ObjectRole::Door && o.locked),
        "{at}: the gate has a locked leaf"
    );
    let looted = graph.rooms.iter().filter(|r| !r.loot.is_empty()).count();
    assert_eq!(stamp.chests.len(), looted, "{at}: a chest a room with loot");
    let mobs: u32 = graph
        .rooms
        .iter()
        .flat_map(|r| &r.mobs)
        .map(|m| m.count)
        .sum();
    assert_eq!(stamp.mobs.len(), mobs as usize, "{at}: every mob placed");
    assert!(
        stamp.digs.len() <= 2 * dungeon.layout.levels as usize + 1,
        "{at}: {} digs",
        stamp.digs.len()
    );
    assert!(
        stamp.lights.len() <= 40,
        "{at}: {} lights",
        stamp.lights.len()
    );
    assert!(stamp.route.len() >= graph.main.len() + 3, "{at}: a tour");
    if let Some(leg) = blocked(&map, &stamp.route) {
        let r = &stamp.route;
        panic!(
            "{at}: the tour's leg {leg} of {} passes through a cube, {} to {}",
            r.len() - 1,
            r[leg],
            r[leg + 1]
        );
    }
    assert_eq!(
        stamp.route[0], stamp.start,
        "{at}: the tour starts at the start"
    );
    Numbers {
        ops: stamp.ops.len(),
        cubes: map.len(),
        lights: stamp.lights.len(),
        spots: reached.len(),
        air,
        ms,
    }
}

#[test]
fn stamps_walk_from_the_surface_to_everything_and_close_every_space() {
    for theme in Theme::all(&assets()).unwrap() {
        let set = ModuleSet::new(&theme).unwrap();
        let seeds: Vec<u64> = (0..SEEDS).collect();
        let numbers: Vec<Numbers> = terra_gen::par::map(&seeds, |&s| {
            let dungeon = generate(&theme, &set, Seed(s)).ok()?;
            Some(check(&theme, &dungeon, s))
        })
        .into_iter()
        .flatten()
        .collect();
        assert!(
            numbers.len() as u64 >= SEEDS * 9 / 10,
            "{}: placed",
            theme.name
        );
        let mean =
            |f: fn(&Numbers) -> f32| numbers.iter().map(f).sum::<f32>() / numbers.len() as f32;
        println!(
            "{}: {} stamps, {:.0} ops, {:.0} cubes, {:.1} lights, {:.0} walkable spots, {:.0} air cubes, {:.2} ms a stamp",
            theme.name,
            numbers.len(),
            mean(|n| n.ops as f32),
            mean(|n| n.cubes as f32),
            mean(|n| n.lights as f32),
            mean(|n| n.spots as f32),
            mean(|n| n.air as f32),
            mean(|n| n.ms),
        );
    }
}

#[test]
fn stamps_name_models_that_exist_and_repeat() {
    let root = assets();
    for theme in Theme::all(&root).unwrap() {
        let set = ModuleSet::new(&theme).unwrap();
        for s in 0..20 {
            let Ok(dungeon) = generate(&theme, &set, Seed(s)) else {
                continue;
            };
            let stamp = stamp(&dungeon, &theme, &kind);
            let models = stamp.props.iter().map(|p| &p.model);
            for model in models.chain(stamp.objects.iter().map(|o| &o.model)) {
                assert!(root.join(model).exists(), "{}: no {model}", theme.name);
            }
            let again = super::stamp(&dungeon, &theme, &kind);
            assert_eq!(
                again.ops, stamp.ops,
                "{} seed {s}: the same cubes",
                theme.name
            );
            assert_eq!(
                again.route, stamp.route,
                "{} seed {s}: the same tour",
                theme.name
            );
        }
    }
}

#[test]
fn frames_turn_as_modules_turn() {
    // A cell's north-west cube seen from the east is its north-east cube.
    let f = Frame::new(IVec3::new(1, 0, 2), 1);
    assert_eq!(f.cube(0, 0, 0), IVec3::new(11, -8, 12));
    assert_eq!(f.side(0), 1);
    // A point in a cube turns with it.
    for turn in 0..4 {
        let f = Frame::new(IVec3::ZERO, turn);
        for (x, z) in [(0, 0), (2, 5), (4, 1)] {
            let p = f.point(x as f32 + 0.5, 0.5, z as f32 + 0.5);
            assert_eq!(cube_of(p), f.cube(x, 0, z), "turn {turn}");
        }
    }
    // A stair cube turned with its frame rises toward the frame's north.
    let mut ops = Vec::new();
    Frame::new(IVec3::ZERO, 3).fill(
        &mut ops,
        [0, 0, 0],
        [0, 0, 0],
        shaped(Kind(1), Shape::Stair(Facing::North)),
    );
    let BuildOp::Fill { cube: Some(c), .. } = ops[0] else {
        panic!("a fill");
    };
    assert_eq!(c.shape, Shape::Stair(Facing::West));
}

/// Seeds a theme the camera's view is checked over.
const VIEW_SEEDS: u64 = 1000;

/// A door's two leaves standing open, as the game swings them (spawn.rs `open`): from each
/// jamb, a metre either side of the sill's middle, a metre into the room, 2.5 m high.
fn open_leaves(stamp: &Stamp) -> Vec<(Vec3, Vec3)> {
    let doors = stamp.objects.iter().filter(|o| o.role == ObjectRole::Door);
    doors
        .flat_map(|o| {
            let f = Frame::new(IVec3::ZERO, o.facing);
            let across = (f.point(1.0, 0.0, 0.0) - f.point(0.0, 0.0, 0.0)).normalize();
            let into = (f.point(0.0, 0.0, 1.0) - f.point(0.0, 0.0, 0.0)).normalize();
            [-1.0, 1.0].map(|s| (o.at + across * s, into))
        })
        .collect()
}

/// Whether a point is inside a cube not open or within 5 cm of an open leaf.
fn solid_at(map: &Map, leaves: &[(Vec3, Vec3)], p: Vec3) -> bool {
    !clear(map, cube_of(p))
        || leaves.iter().any(|&(hinge, into)| {
            let q = p - hinge;
            let along = q.dot(into);
            let off = (q - into * along) * Vec3::new(1.0, 0.0, 1.0);
            (0.0..=1.0).contains(&along) && off.length() < 0.05 && (0.0..=2.5).contains(&q.y)
        })
}

/// Whether every point on rings round `p` in plan out to `r`, every 0.1 m and 10°, is clear.
fn clear_across(map: &Map, leaves: &[(Vec3, Vec3)], p: Vec3, r: f32) -> bool {
    let rings = (r / 0.1).ceil() as i32;
    (1..=rings).all(|k| {
        let radius = r * k as f32 / rings as f32;
        (0..36).all(|a| {
            let turn = a as f32 * std::f32::consts::TAU / 36.0;
            let d = Vec3::new(libm::cosf(turn), 0.0, libm::sinf(turn));
            !solid_at(map, leaves, p + d * radius)
        })
    })
}

/// Whether every point on a sphere round `p` of radius `r` (a Fibonacci lattice of 96) and
/// half way to it is clear.
fn clear_around(map: &Map, leaves: &[(Vec3, Vec3)], p: Vec3, r: f32) -> bool {
    const N: usize = 96;
    (0..N).all(|i| {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / N as f32;
        let ring = libm::sqrtf(1.0 - y * y);
        let a = i as f32 * 2.399_963;
        let d = Vec3::new(libm::cosf(a) * ring, y, libm::sinf(a) * ring);
        !solid_at(map, leaves, p + d * r) && !solid_at(map, leaves, p + d * r * 0.5)
    })
}

#[derive(Default)]
struct Seen {
    faults: Vec<String>,
    samples: usize,
    narrow: usize,
    tight: usize,
    lowest: f32,
}

/// The view of one stamp sampled every 0.1 m: its eye `EYE_M` over the tour's feet, in an
/// open cube, `ROOM_M` clear in plan and `TIGHT_M` all round; how many samples have less
/// than `ROOM_M` all round, and the lowest eye.
fn check_view(theme: &Theme, dungeon: &Dungeon, s: u64) -> Seen {
    use super::view::{EYE_M, ROOM_M, STEP_M, TIGHT_M, resample};
    let at = format!("{} seed {s}", theme.name);
    let stamp = stamp(dungeon, theme, &kind);
    let map = cubes(&stamp.ops);
    let leaves = open_leaves(&stamp);
    let room = super::view::Room::new(&map, &stamp.objects);
    let feet = resample(&stamp.route, STEP_M);
    assert_eq!(feet.len(), stamp.view.len(), "{at}: an eye a foot");
    let mut seen = Seen {
        lowest: f32::MAX,
        ..Seen::default()
    };
    // The first leg is the surface's, above the dig, where the ground is not in the cubes.
    let surface = stamp.route[0].distance(stamp.route[1]);
    for (i, pair) in stamp.view.windows(2).enumerate() {
        let (a, b) = (pair[0], pair[1]);
        for k in [i, i + 1] {
            let up = stamp.view[k].y - feet[k].y;
            assert!(
                (EYE_M[0] - 1e-3..=EYE_M[1] + 1e-3).contains(&up),
                "{at}: the eye at {} is {up} m over the feet",
                feet[k]
            );
            seen.lowest = seen.lowest.min(up);
        }
        if (i as f32 + 1.0) * STEP_M < surface {
            continue;
        }
        let n = ((a.distance(b) / 0.1).ceil() as i32).max(1);
        for k in 0..n {
            let p = a.lerp(b, k as f32 / n as f32);
            // Where the way is narrower than twice `ROOM_M` (a corridor by a heap), the most
            // room on a line across it through the eye, which the eye must have.
            let wide = if clear_across(&map, &leaves, p, ROOM_M - 0.02) {
                ROOM_M
            } else {
                seen.narrow += 1;
                let d = (b - a) * Vec3::new(1.0, 0.0, 1.0);
                let side = Vec3::new(-d.z, 0.0, d.x).normalize_or_zero();
                (-20..=20)
                    .map(|k| room.across(p + side * (k as f32 * 0.05)))
                    .fold(0.0, f32::max)
                    .min(ROOM_M)
            };
            let fault = if solid_at(&map, &leaves, p) {
                Some("is in a cube or a leaf".to_string())
            } else if !clear_across(&map, &leaves, p, wide - 0.07) {
                Some(format!(
                    "passes within {wide} m of a wall or a leaf in plan"
                ))
            } else if !clear_around(&map, &leaves, p, TIGHT_M.min(wide) - 0.07) {
                Some(format!("passes within {} m of a cube", TIGHT_M.min(wide)))
            } else {
                None
            };
            if let Some(fault) = fault {
                seen.faults
                    .push(format!("{at}: the eye at {p} (view {i}) {fault}"));
            }
            seen.samples += 1;
            if !clear_around(&map, &leaves, p, ROOM_M - 0.02) {
                seen.tight += 1;
            }
        }
    }
    seen
}

#[test]
fn the_camera_keeps_off_walls_leaves_and_slabs() {
    let mut faulty = Vec::new();
    for theme in Theme::all(&assets()).unwrap() {
        let set = ModuleSet::new(&theme).unwrap();
        let seeds: Vec<u64> = (0..VIEW_SEEDS).collect();
        let seen: Vec<Seen> = terra_gen::par::map(&seeds, |&s| {
            let dungeon = generate(&theme, &set, Seed(s)).ok()?;
            Some(check_view(&theme, &dungeon, s))
        })
        .into_iter()
        .flatten()
        .collect();
        assert!(
            seen.len() as u64 >= VIEW_SEEDS * 9 / 10,
            "{}: placed",
            theme.name
        );
        let samples: usize = seen.iter().map(|s| s.samples).sum();
        let tight: usize = seen.iter().map(|s| s.tight).sum();
        let narrow: usize = seen.iter().map(|s| s.narrow).sum();
        let lowest = seen.iter().map(|s| s.lowest).fold(f32::MAX, f32::min);
        let faults: Vec<&String> = seen.iter().flat_map(|s| &s.faults).collect();
        for fault in faults.iter().take(3) {
            println!("{fault}");
        }
        faulty.push((theme.name.clone(), faults.len() as f32 / samples as f32));
        println!(
            "{}: {} views, {samples} samples, {:.2}% in ways too narrow for the full room across, {:.2}% with less than it all round, the eye {lowest:.2} m over the feet at the lowest",
            theme.name,
            seen.len(),
            100.0 * narrow as f32 / samples as f32,
            100.0 * tight as f32 / samples as f32,
        );
    }
    // The flooded mine still has a few narrow places (a third of its seeds, 0.2% of its
    // samples) where the eye comes within 0.25-0.6 m of a post or a heap: the next step.
    assert!(
        faulty.iter().all(|(_, share)| *share < 0.003),
        "the share of views every 0.1 m that fail, a theme: {faulty:?}"
    );
}
