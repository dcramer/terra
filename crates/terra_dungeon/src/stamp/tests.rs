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
        for step in STEPS {
            for dy in [1, 0, -1, -2] {
                let n = p + step + IVec3::Y * dy;
                if seen.contains(&n) || !stands(map, n) {
                    continue;
                }
                let head = if dy > 0 {
                    clear(map, p + IVec3::Y * 4)
                } else {
                    (n.y + 4..=p.y + 3).all(|y| clear(map, IVec3::new(n.x, y, n.z)))
                };
                if head {
                    seen.insert(n);
                    queue.push_back(n);
                }
            }
        }
    }
    seen
}

fn cube_of(p: Vec3) -> IVec3 {
    (p / CUBE).floor().as_ivec3()
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
