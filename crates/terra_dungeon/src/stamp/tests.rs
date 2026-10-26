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
        }
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

/// Whether a point is inside a cube not open or within 5 cm of an open leaf.
fn solid_at(map: &Map, leaves: &[(Vec3, Vec3)], p: Vec3) -> bool {
    !clear(map, cube_of(p))
        || leaves.iter().any(|&(hinge, into)| {
            let q = p - hinge;
            let along = q.dot(into);
            let off = (q - into * along) * Vec3::new(1.0, 0.0, 1.0);
            (0.0..=DOORWAY_M[0] * 0.5).contains(&along)
                && off.length() < 0.05
                && (0.0..=DOORWAY_M[1]).contains(&q.y)
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

#[derive(Default)]
struct Looked {
    samples: usize,
    /// Looks with the way 4 m on more than `FRAME_RAD` off them, out of frame, but where the
    /// way turns back on itself (its 4 m behind and 4 m on over 120° apart), where a camera
    /// turning round has it out of frame for a moment whatever it does.
    lost: usize,
    /// The longest stretch of the way, in metres, so.
    lost_m: f32,
    /// Looks facing a wall within `NEAR_M`, within 45° of straight on.
    walled: usize,
    /// The same for a look at the way 4 m on.
    naive_walled: usize,
    /// How far down the look is open, summed, and the same for a look at the way 4 m on.
    open_m: f32,
    naive_open_m: f32,
    turn: f32,
    goal_off: f32,
    /// The fastest the look turns over `PAN_S`, in radians a second, at the stamp's pace and
    /// at the plain dolly's 2.5 m/s.
    pan: f32,
    plain_pan: f32,
    /// Frames with a wall within `WALL_M` of the eye anywhere in them (`frame_rays`), and
    /// within a metre.
    walled_frame: usize,
    metre_frame: usize,
    /// The nearest wall in any frame, in metres (up to 1.5).
    nearest: f32,
    /// Eyes back within `RETRACE_M` of where the eye stood more than `RETRACE_ALONG_M` of the
    /// view before: the tour gone into a dead end and out the way it came, which a film cuts
    /// out, turning on the spot where it cut (round 20's mine and keep, 2-7 s at a wall).
    retraced: usize,
    /// Looks where the way turns back on itself, its 4 m behind and 4 m on over 120° apart.
    turned_back: usize,
    /// The steepest the look pitches, up or down, in radians.
    pitch: f32,
    /// How long the dolly takes from the first look measured to the last, at the stamp's
    /// pace and at 2.5 m/s, in seconds.
    seconds: f32,
    plain_seconds: f32,
}

/// The look's turn is measured over this long, in seconds (round 17's mine whip: 168° in
/// 1.9 s, some 90° a second).
const PAN_S: f32 = 0.5;
/// A wall this near the eye in frame fills it (the smear round 18 saw at 0.3-0.6 m).
const WALL_M: f32 = 0.5;

/// Rays across a 20 mm lens's frame, 84° by 53°: five across and three up, through its
/// corners, edges' middles and middle.
fn frame_rays(d: Vec3) -> impl Iterator<Item = Vec3> {
    let right = d.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = right.cross(d);
    let (across, high) = (
        libm::tanf(42f32.to_radians()),
        libm::tanf(26.5f32.to_radians()),
    );
    (0..15).map(move |k| {
        let (u, w) = ((k % 5) as f32 / 2.0 - 1.0, (k / 5) as f32 - 1.0);
        (d + right * (u * across) + up * (w * high)).normalize()
    })
}

/// Where a look is at most this far off the way 4 m on, the way on is in frame (a 20 mm
/// lens's frame is some 84° across).
const FRAME_RAD: f32 = 0.7;
/// A wall at arm's length or so.
const NEAR_M: f32 = 1.5;
/// An eye this near where the eye stood over `RETRACE_ALONG_M` of the view before has come
/// back the way it went: round 20's tour tool cut such a loop out, as a film would.
const RETRACE_M: f32 = 0.5;
const RETRACE_ALONG_M: f32 = 2.0;

/// The look from each eye of one stamp's view, past the surface leg and short of where a
/// film stops (3.5 m from the goal in plan): how often the way on is out of frame and the
/// longest stretch so, how often it faces a wall near, how far down it is open (a cube met
/// marching the ray every 0.05 m, up to 8 m) against a look at the way 4 m on, the fastest
/// turn a metre, and how far off the goal it looks where the film stops.
fn check_look(theme: &Theme, dungeon: &Dungeon) -> Looked {
    use super::view::STEP_M;
    let stamp = stamp(dungeon, theme, &kind);
    let map = cubes(&stamp.ops);
    let (view, look) = (&stamp.view, &stamp.look);
    assert_eq!(view.len(), look.len(), "a look an eye");
    assert_eq!(view.len(), stamp.pace.len(), "a pace an eye");
    let dir = |i: usize| (look[i] - view[i]).normalize();
    let goal = stamp
        .objects
        .iter()
        .find(|o| o.role == ObjectRole::Goal)
        .expect("a goal");
    let stop = (0..view.len())
        .rev()
        .find(|&i| {
            let off = view[i] - goal.at;
            off.x.hypot(off.z) >= 3.5
        })
        .unwrap_or(view.len() - 1);
    let surface = stamp.route[0].distance(stamp.route[1]);
    let lead = (4.0 / STEP_M) as usize;
    // How far a ray from an eye runs through open cubes, up to `most` metres, and the axis it
    // stepped across into the cube it meets (nought if none).
    let hit = |from: Vec3, d: Vec3, most: f32| {
        let mut was = cube_of(from);
        for k in 1..=(most / 0.05).round() as i32 {
            let c = cube_of(from + d * (k as f32 * 0.05));
            if !clear(&map, c) {
                return (k as f32 * 0.05, c - was);
            }
            was = c;
        }
        (most, IVec3::ZERO)
    };
    // How far it runs, up to 8 m, and whether the cube it meets faces it within 45° of
    // straight on.
    let open = |from: Vec3, d: Vec3| {
        let (free, across) = hit(from, d, 8.0);
        let face = if across.x != 0 {
            d.x
        } else if across.y != 0 {
            d.y
        } else {
            d.z
        };
        (
            free,
            across != IVec3::ZERO && face.abs() > std::f32::consts::FRAC_1_SQRT_2,
        )
    };
    // When the dolly reaches each eye, at the stamp's pace and at the plain 2.5 m/s.
    let mut times = vec![0.0f32];
    let mut plain = vec![0.0f32];
    for i in 1..view.len() {
        times.push(times[i - 1] + stamp.pace[i - 1]);
        plain.push(plain[i - 1] + view[i].distance(view[i - 1]) / 2.5);
    }
    let pan = |t: &[f32], i: usize| {
        (i + 1..view.len())
            .find(|&j| t[j] - t[i] >= PAN_S)
            .map_or(0.0, |j| dir(i).angle_between(dir(j)) / (t[j] - t[i]))
    };
    let mut seen = Looked {
        nearest: 1.5,
        ..Looked::default()
    };
    let mut run = 0.0f32;
    for i in 0..stop {
        if (i as f32) * STEP_M < surface + 1.0 {
            continue;
        }
        let on = view[(i + lead).min(view.len() - 1)] - view[i];
        if on.length() < 0.5 {
            continue;
        }
        seen.samples += 1;
        let d = dir(i);
        let lost = d.angle_between(on) > FRAME_RAD;
        let behind = view[i] - view[i.saturating_sub(lead)];
        let back = behind.length() > 0.5 && behind.angle_between(on) > 2.1;
        seen.turned_back += back as usize;
        let gone_by = i.saturating_sub((RETRACE_ALONG_M / STEP_M) as usize);
        seen.retraced += (0..gone_by).any(|j| view[j].distance(view[i]) < RETRACE_M) as usize;
        seen.lost += (lost && !back) as usize;
        run = if lost { run + STEP_M } else { 0.0 };
        seen.lost_m = seen.lost_m.max(run);
        let (free, flat) = open(view[i], d);
        seen.walled += (free < NEAR_M && flat) as usize;
        seen.open_m += free;
        let (naive_free, naive_flat) = open(view[i], on.normalize());
        seen.naive_open_m += naive_free;
        seen.naive_walled += (naive_free < NEAR_M && naive_flat) as usize;
        if i + 1 < view.len() {
            seen.turn = seen.turn.max(d.angle_between(dir(i + 1)) / STEP_M);
        }
        seen.pan = seen.pan.max(pan(&times, i));
        seen.plain_pan = seen.plain_pan.max(pan(&plain, i));
        let wall = |r: Vec3| match hit(view[i], r, 1.5) {
            (free, across) if across.y == 0 && across != IVec3::ZERO => free,
            _ => 1.5,
        };
        let nearest = frame_rays(d).map(wall).fold(1.5, f32::min);
        seen.walled_frame += (nearest < WALL_M) as usize;
        seen.metre_frame += (nearest < 1.0) as usize;
        seen.nearest = seen.nearest.min(nearest);
        seen.pitch = seen.pitch.max(libm::asinf(d.y.abs().min(1.0)));
        if i + 1 < view.len() {
            seen.seconds += times[i + 1] - times[i];
            seen.plain_seconds += plain[i + 1] - plain[i];
        }
    }
    seen.goal_off = dir(stop).angle_between(goal.at + Vec3::Y * 0.5 - view[stop]);
    seen
}

#[test]
fn the_camera_looks_down_the_way_ahead_not_at_walls() {
    use super::look::{PITCH_MOST, TURN_MOST_PER_M, TURN_MOST_PER_S};
    for theme in Theme::all(&assets()).unwrap() {
        let set = ModuleSet::new(&theme).unwrap();
        let seeds: Vec<u64> = (0..SEEDS).collect();
        let seen: Vec<Looked> = terra_gen::par::map(&seeds, |&s| {
            let dungeon = generate(&theme, &set, Seed(s)).ok()?;
            Some(check_look(&theme, &dungeon))
        })
        .into_iter()
        .flatten()
        .collect();
        let sum = |f: &dyn Fn(&Looked) -> f32| seen.iter().map(f).sum::<f32>();
        let samples = sum(&|s| s.samples as f32);
        let lost = sum(&|s| s.lost as f32) / samples;
        let walled = sum(&|s| s.walled as f32) / samples;
        let naive_walled = sum(&|s| s.naive_walled as f32) / samples;
        let open = sum(&|s| s.open_m) / samples;
        let naive_open = sum(&|s| s.naive_open_m) / samples;
        let most = |f: &dyn Fn(&Looked) -> f32| seen.iter().map(f).fold(0.0, f32::max);
        let (lost_m, turn) = (most(&|s| s.lost_m), most(&|s| s.turn));
        let goal_off = most(&|s| s.goal_off);
        let (pan, plain_pan) = (most(&|s| s.pan), most(&|s| s.plain_pan));
        let walled_frame = sum(&|s| s.walled_frame as f32) / samples;
        let metre_frame = sum(&|s| s.metre_frame as f32) / samples;
        let nearest = seen.iter().map(|s| s.nearest).fold(1.5, f32::min);
        let pitch = most(&|s| s.pitch);
        let retraced = sum(&|s| s.retraced as f32) / samples;
        let turned_back = sum(&|s| s.turned_back as f32) / samples;
        let slower = sum(&|s| s.seconds) / sum(&|s| s.plain_seconds);
        println!(
            "{}: turning {:.0}°/s over {PAN_S} s at most at the tour's pace ({:.0} at 2.5 m/s), the tour {slower:.2} times as long; pitched {:.0}° at most; a wall in frame within {WALL_M} m {:.2}% of frames, within 1 m {:.2}%, {nearest:.2} m at the nearest; back where it was {:.2}% of eyes, the way turning back on itself {:.2}%",
            theme.name,
            pan.to_degrees(),
            plain_pan.to_degrees(),
            pitch.to_degrees(),
            100.0 * walled_frame,
            100.0 * metre_frame,
            100.0 * retraced,
            100.0 * turned_back
        );
        // A showcase walk never goes into a dead end it must turn round in (round 20's films
        // turned on the spot at a key room's back wall for 2-7 s; 14/14/28% of eyes came
        // back where they had been, crypt/mine/keep), and turns sharply round only where a
        // room's way out is beside its way in, short of its middle (4.7/4.3/6.1% of looks
        // with the way turning back on itself then, 0.27/0.39/0.27 round 21).
        assert!(
            retraced == 0.0 && turned_back < 0.01,
            "{}: back where it was in {retraced} of eyes, the way turning back {turned_back}",
            theme.name
        );
        println!(
            "{}: {} tours, {samples} looks: the way on out of frame {:.2}% but turning round ({lost_m:.1} m at most), facing a wall within {NEAR_M} m {:.2}% ({:.2}), open {open:.2} m down the look on average ({naive_open:.2} looking at the way 4 m on), turning {:.0}° a metre at most, the goal {:.0}° off at the stop at most",
            theme.name,
            seen.len(),
            100.0 * lost,
            100.0 * walled,
            100.0 * naive_walled,
            turn.to_degrees(),
            goal_off.to_degrees()
        );
        // Round 13's films looked down a corridor's straight to its end wall, the way on 90°
        // off and out of frame for a second (the crypt's 16 s), then whipped round. Turning
        // round out of a dead end, at most 2.5 m (some 2.3 at `TURN_MOST_PER_M`).
        assert!(
            lost < 0.02 && lost_m <= 2.5,
            "{}: the way on out of frame {lost}, {lost_m} m",
            theme.name
        );
        // Facing a wall near no more often than the plain look at the way 4 m on, give or
        // take a thousandth (round 15's look at the farthest point it saw: 3% against 0.11%).
        // The plain look itself faces one in 1.09% of the mine's looks, along straight ways:
        // the view's, not the look's (round 16).
        assert!(
            walled <= naive_walled + 0.001,
            "{}: {walled} of looks at a wall near, against {naive_walled}",
            theme.name
        );
        // As open in front of it as the plain look at the way 4 m on, within a twentieth: the
        // smoothing, the turn bound and the pull towards the goal cost it little.
        assert!(
            open > 0.95 * naive_open,
            "{}: open {open} m against {naive_open}",
            theme.name
        );
        // At most the bound in yaw and in pitch at once.
        let bound = TURN_MOST_PER_M * std::f32::consts::SQRT_2 * 1.01;
        assert!(turn <= bound, "{}: turns {turn} a metre", theme.name);
        assert!(goal_off < 0.35, "{}: the goal {goal_off} off", theme.name);
        // Round 17's mine film whipped round out of a dead end, some 90° a second; at the
        // tour's pace no look turns faster than a slow pan.
        assert!(
            pan <= TURN_MOST_PER_S * 1.01,
            "{}: turns {}°/s",
            theme.name,
            pan.to_degrees()
        );
        // The view keeps the eye off walls (`view::ROOM_M`), and the look keeps a wall from
        // filling the frame: round 18 saw walls smeared at 0.3-0.6 m.
        // Round 17's mine film pitched 59° down at a well.
        assert!(
            pitch <= PITCH_MOST + 1e-3,
            "{}: pitched {}°",
            theme.name,
            pitch.to_degrees()
        );
        assert!(
            walled_frame <= 0.001 && metre_frame < 0.03 && nearest >= 0.6,
            "{}: a wall within {WALL_M} m in {walled_frame} of frames, {nearest} m at the nearest",
            theme.name
        );
    }
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
