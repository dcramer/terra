use super::*;
use crate::graph::Link;
use crate::place::{STEPS, Side, Space};
use crate::theme::{Role, assets};
use crate::{ModuleSet, generate};
use std::collections::{HashSet, VecDeque};
use terra_gen::Seed;

pub(super) type Map = HashMap<IVec3, Cube>;

pub(super) const SEEDS: u64 = 200;

pub(super) fn kind(name: &str) -> Kind {
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
        let inside = (1..=INNER)
            .flat_map(|x| (0..=LEVEL - 2).flat_map(move |y| (1..=INNER).map(move |z| (x, y, z))));
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
    // A hero's lamp is dressing, laid where a prop may stand, whether walked to or not.
    let fires =
        (stamp.lights.iter()).filter(|l| !matches!(l.kind, LightKind::Torch | LightKind::Lamp));
    for l in fires {
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
                .any(|o| o.role == ObjectRole::Door && o.lock == crate::door::Lock::Keys),
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
        // The cap, the fires and braziers placed before it, and a torch a stair and two a
        // room past it.
        stamp.lights.len()
            <= LIGHTS + 10 + dungeon.layout.stairs.len() + 2 * dungeon.layout.rooms.len(),
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
        let seeds: Vec<u64> = (0..20).collect();
        terra_gen::par::map(&seeds, |&s| {
            let Ok(dungeon) = generate(&theme, &set, Seed(s)) else {
                return;
            };
            let stamp = stamp(&dungeon, &theme, &kind);
            let models = stamp.props.iter().map(|p| &p.model);
            let pieces = stamp.pieces.iter().map(|p| &p.model);
            let objects = stamp.objects.iter().map(|o| &o.model);
            for model in models.chain(pieces).chain(objects) {
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
        });
    }
}

/// A module lays a weapon rack only where its name says rack or weapon, never by a word inside
/// another: by "rack" in "cracked_flags" each cracked floor of the crypt laid a set of axes
/// (barrow dress6: 21 over the crypt, lying parallel in its vault and goal room).
#[test]
fn a_module_lays_a_rack_only_where_its_name_says_one() {
    for theme in Theme::all(&assets()).unwrap() {
        let bare = Theme {
            dressing: Vec::new(),
            ..theme.clone()
        };
        let named = (theme.modules.iter())
            .any(|m| (m.name.split('_')).any(|w| w == "rack" || w == "weapon"));
        let set = ModuleSet::new(&theme).unwrap();
        let mut racks = 0;
        for s in 0..20 {
            let Ok(dungeon) = generate(&theme, &set, Seed(s)) else {
                continue;
            };
            let laid = stamp(&dungeon, &bare, &kind).props;
            racks += (laid.iter())
                .filter(|p| p.model.ends_with("AxesLaid.glb"))
                .count();
        }
        assert_eq!(racks > 0, named, "{}: {racks} racks laid", theme.name);
    }
}

#[test]
fn frames_turn_as_modules_turn() {
    // A cell's north-west cube seen from the east is its north-east cube.
    let f = Frame::new(IVec3::new(1, 0, 2), 1);
    assert_eq!(f.cube(0, 0, 0), IVec3::new(CELL + EDGE, -LEVEL, 2 * CELL));
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

/// A door's two leaves standing open, as the game swung them before they were hung at the
/// room's face of the wall to lie flat on it (leaf_tests.rs): from each jamb a leaf's width
/// into the room, as a leaf with no wall to lie on still stands. The camera still keeps clear of where they stood.
fn open_leaves(stamp: &Stamp) -> Vec<(Vec3, Vec3)> {
    let doors = stamp.objects.iter().filter(|o| o.role == ObjectRole::Door);
    doors
        .flat_map(|o| {
            let f = Frame::new(IVec3::ZERO, o.facing);
            let across = (f.point(1.0, 0.0, 0.0) - f.point(0.0, 0.0, 0.0)).normalize();
            let into = (f.point(0.0, 0.0, 1.0) - f.point(0.0, 0.0, 0.0)).normalize();
            [-1.0, 1.0].map(|s| (o.at + across * s * DOORWAY_M[0] * 0.5, into))
        })
        .collect()
}

/// What `solid_at` reads round one eye, read once for its ~800 points: whether each cube of
/// the 4 cubes a side that hold everything within `r` of it is open, and the leaves that
/// reach within `r` of it in plan. The same answers as the whole map and every leaf.
struct Near<'a> {
    map: &'a Map,
    min: IVec3,
    clear: [bool; 64],
    leaves: Vec<(Vec3, Vec3)>,
}

impl<'a> Near<'a> {
    fn new(map: &'a Map, leaves: &[(Vec3, Vec3)], p: Vec3, r: f32) -> Near<'a> {
        let min = cube_of(p - Vec3::splat(r));
        let clear = std::array::from_fn(|i| {
            let i = i as i32;
            clear(map, min + IVec3::new(i % 4, i / 4 % 4, i / 16))
        });
        // A point within `r` of the eye is near a leaf only if the leaf's hinge is within
        // its width and 5 cm more of that point.
        let reach = r + DOORWAY_M[0] * 0.5 + 0.06;
        let leaves = leaves
            .iter()
            .copied()
            .filter(|&(hinge, _)| ((hinge - p) * Vec3::new(1.0, 0.0, 1.0)).length() <= reach)
            .collect();
        Near {
            map,
            min,
            clear,
            leaves,
        }
    }

    fn solid_at(&self, p: Vec3) -> bool {
        let c = cube_of(p);
        let i = c - self.min;
        let open = if i.cmpge(IVec3::ZERO).all() && i.cmplt(IVec3::splat(4)).all() {
            self.clear[(i.x + 4 * i.y + 16 * i.z) as usize]
        } else {
            clear(self.map, c)
        };
        !open || on_leaf(&self.leaves, p)
    }
}

/// Whether a point is within 5 cm of an open leaf.
fn on_leaf(leaves: &[(Vec3, Vec3)], p: Vec3) -> bool {
    leaves.iter().any(|&(hinge, into)| {
        let q = p - hinge;
        let along = q.dot(into);
        let off = (q - into * along) * Vec3::new(1.0, 0.0, 1.0);
        (0.0..=DOORWAY_M[0] * 0.5).contains(&along)
            && off.length() < 0.05
            && (0.0..=DOORWAY_M[1]).contains(&q.y)
    })
}

/// The ways `clear_across` and `clear_around` look: every 10° in plan, and a Fibonacci
/// lattice of 96 on the sphere.
static DIRECTIONS: std::sync::LazyLock<([Vec3; 36], [Vec3; 96])> = std::sync::LazyLock::new(|| {
    let plan = std::array::from_fn(|a| {
        let turn = a as f32 * std::f32::consts::TAU / 36.0;
        Vec3::new(libm::cosf(turn), 0.0, libm::sinf(turn))
    });
    let sphere = std::array::from_fn(|i| {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / 96.0;
        let ring = libm::sqrtf(1.0 - y * y);
        let a = i as f32 * 2.399_963;
        Vec3::new(libm::cosf(a) * ring, y, libm::sinf(a) * ring)
    });
    (plan, sphere)
});

/// Whether every point on rings round `p` in plan out to `r`, every 0.1 m and 10°, is clear.
fn clear_across(near: &Near, p: Vec3, r: f32) -> bool {
    let rings = (r / 0.1).ceil() as i32;
    (1..=rings).all(|k| {
        let radius = r * k as f32 / rings as f32;
        DIRECTIONS.0.iter().all(|&d| !near.solid_at(p + d * radius))
    })
}

/// Whether every point on a sphere round `p` of radius `r` (a Fibonacci lattice of 96) and
/// half way to it is clear.
fn clear_around(near: &Near, p: Vec3, r: f32) -> bool {
    DIRECTIONS
        .1
        .iter()
        .all(|&d| !near.solid_at(p + d * r) && !near.solid_at(p + d * r * 0.5))
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
    let room = super::view::Room::of(Grid::new(&map), &stamp.objects);
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
            // Every point read below is within `ROOM_M` of the eye.
            let near = Near::new(&map, &leaves, p, ROOM_M);
            // Where the way is narrower than twice `ROOM_M` (a corridor by a heap), the most
            // room on a line across it through the eye, which the eye must have.
            let wide = if clear_across(&near, p, ROOM_M - 0.02) {
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
            let fault = if near.solid_at(p) {
                Some("is in a cube or a leaf".to_string())
            } else if !clear_across(&near, p, wide - 0.07) {
                Some(format!(
                    "passes within {wide} m of a wall or a leaf in plan"
                ))
            } else if !clear_around(&near, p, TIGHT_M.min(wide) - 0.07) {
                Some(format!("passes within {} m of a cube", TIGHT_M.min(wide)))
            } else {
                None
            };
            if let Some(fault) = fault {
                seen.faults
                    .push(format!("{at}: the eye at {p} (view {i}) {fault}"));
            }
            seen.samples += 1;
            if !clear_around(&near, p, ROOM_M - 0.02) {
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

/// A mine's timber sets do not repeat as tiles down a gallery (round 13's film: five
/// identical frames nested down a straight one, 28.5 s): over many tours, no one way of
/// standing (its place along the bay, which posts, the beam) is most sets'. And a post
/// never stands in a doorway.
#[test]
fn a_mines_timber_sets_stand_each_its_own_way() {
    use super::modules::{Class, looks};
    let theme = Theme::all(&assets())
        .unwrap()
        .into_iter()
        .find(|t| t.name == "flooded_mine")
        .expect("the mine");
    let set = ModuleSet::new(&theme).unwrap();
    let trim = kind(&theme.trim);
    assert_ne!(trim, kind(&theme.walls), "trim told from wall by its kind");
    let mut ways: HashMap<(i32, bool, bool, bool), usize> = HashMap::new();
    for s in 0..60 {
        let Ok(dungeon) = generate(&theme, &set, Seed(s)) else {
            continue;
        };
        let stamp = stamp(&dungeon, &theme, &kind);
        let map = cubes(&stamp.ops);
        let classes = looks(&dungeon, &theme, &set);
        for (cell, look) in dungeon.layout.cells.iter().zip(&classes) {
            if look.class != Class::Timber {
                continue;
            }
            let f = Frame::new(cell.at, look.turn);
            // The cube under the cell's ceiling, where a set's beam lies.
            let head = match cell.space {
                Space::Corridor(_) => theme.headroom.0,
                _ => theme.headroom.1,
            };
            let top = head_of(head) - 1;
            let is_trim = |x, y, z| map.get(&f.cube(x, y, z)).is_some_and(|c| c.kind == trim);
            let post = |x, z| (1..top).all(|y| is_trim(x, y, z));
            for z in 1..=INNER {
                let (east, west) = (post(EDGE, z), post(0, z));
                let beam = (1..=INNER).all(|x| is_trim(x, top, z));
                if east || west || beam {
                    *ways.entry((z, east, west, beam)).or_default() += 1;
                }
            }
            for d in [1, 3] {
                if matches!(cell.sides[f.side(d)], Side::Door(_)) {
                    let x = if d == 1 { EDGE } else { 0 };
                    assert!((1..=INNER).all(|z| !post(x, z)), "a post in a doorway");
                }
            }
        }
    }
    let sets: usize = ways.values().sum();
    let most = ways.values().copied().max().unwrap_or(0);
    println!(
        "{sets} timber sets, {} ways, the commonest {most}",
        ways.len()
    );
    assert!(sets > 50, "{sets} sets");
    assert!(
        2 * most < sets,
        "{most} of {sets} sets stand alike: {ways:?}"
    );
}

#[test]
fn fallen_stone_is_rocks_never_a_lone_half_cube() {
    use super::modules::{Class, looks};
    for theme in Theme::all(&assets()).unwrap() {
        let set = ModuleSet::new(&theme).unwrap();
        let (mut worn, mut rocks) = (0, 0);
        for s in 0..30 {
            let Ok(dungeon) = generate(&theme, &set, Seed(s)) else {
                continue;
            };
            let stamp = stamp(&dungeon, &theme, &kind);
            for op in &stamp.ops {
                if let BuildOp::Fill { cube: Some(c), .. } = op {
                    assert!(
                        !matches!(c.shape, Shape::Half),
                        "{}: a half cube laid, a blockout box on the floor",
                        theme.name
                    );
                }
            }
            let classes = looks(&dungeon, &theme, &set);
            worn += classes.iter().filter(|l| l.class == Class::Rubble).count();
            rocks += stamp
                .props
                .iter()
                .filter(|p| p.model.contains("Rock_"))
                .count();
        }
        assert!(
            rocks >= 4 * worn,
            "{}: {rocks} rocks for {worn} rubble cells",
            theme.name
        );
    }
}

#[test]
fn a_ceiling_is_the_themes_ceilings_but_under_a_room_its_floor() {
    // Each name its own kind. The mine's ceilings are the rock it is cut in, not its
    // trodden floor: a room's slab over it is in the ceilings' kind where no room stands
    // on it, and where one does, that room's floor, in the floors' kind.
    let named = |name: &str| {
        Kind(
            name.bytes()
                .fold(7u16, |h, b| h.wrapping_mul(31) ^ b as u16),
        )
    };
    for theme in Theme::all(&assets()).unwrap() {
        let set = ModuleSet::new(&theme).unwrap();
        let (floor, ceiling) = (
            named(&theme.floors),
            named(theme.ceilings.as_ref().unwrap_or(&theme.floors)),
        );
        let (mut open, mut under, mut rock) = (0, 0, 0);
        for s in 0..20 {
            let Ok(dungeon) = generate(&theme, &set, Seed(s)) else {
                continue;
            };
            let map = cubes(&stamp(&dungeon, &theme, &named).ops);
            let layout = &dungeon.layout;
            for cell in &layout.cells {
                let above = layout.cell(cell.at - IVec3::Y);
                let room = |i: usize| !matches!(layout.cells[i].space, Space::Stair(_));
                if matches!(cell.space, Space::Stair(_)) || above.is_some_and(|i| !room(i)) {
                    continue;
                }
                let middle = map
                    .get(&Frame::new(cell.at, 0).cube(2, LEVEL - 1, 2))
                    .map(|c| c.kind);
                // Open there: a shaft a module left.
                if middle.is_none() {
                    continue;
                }
                if above.is_some() {
                    under += 1;
                    // Trim or a module's piece may stand in the row; never the other slab.
                    assert!(
                        floor == ceiling || middle != Some(ceiling),
                        "{} seed {s} {}",
                        theme.name,
                        cell.at
                    );
                } else {
                    open += 1;
                    assert!(
                        floor == ceiling || middle != Some(floor),
                        "{} seed {s} {}",
                        theme.name,
                        cell.at
                    );
                    rock += usize::from(middle == Some(ceiling));
                }
            }
        }
        assert!(open > 0, "{}: no ceiling without a room on it", theme.name);
        assert!(
            rock * 10 >= open * 9,
            "{}: {rock} of {open} open ceilings in the ceilings' kind",
            theme.name
        );
        if theme.name == "flooded_mine" {
            assert_ne!(ceiling, floor, "the mine's ceilings are its rock");
            assert!(under > 0, "{}: no room over another", theme.name);
        }
    }
}
