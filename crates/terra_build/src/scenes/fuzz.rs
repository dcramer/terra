//! The fuzz pass (D287): small bases built at random from a seed, as a player builds, out of
//! the hammer's own plans (`plans`) put through the checked operations and built, so the
//! mesh checker sees junctions nobody thought to set up. Each base is a few elements, each a
//! tool's use: a room, a wall, a door or window in one, a roof over a room, a box of blocks
//! of any shape and kind, a deck on posts, a flight of stairs, a box carved out or changed
//! to another kind. The same seed builds the same base.
//!
//! `cargo xtask fuzz --seeds N` inspects seeds 0 to N, shrinks each base with a fault to the
//! fewest elements that still show it, and prints it as a test to paste; the full check runs
//! seeds 0 to 200 against the ratchet's baseline (`examples/artifacts-baseline.tsv`), so a
//! seed's faults may fall but never rise. A junction it finds that the kits do not yet close
//! is fixed, or listed in the voxel contract's Gaps (docs/building-voxels.md) with the rule
//! it needs and its seeds' faults recorded in the baseline until it is closed.

use crate::kit::{Kits, Pattern};
use crate::ops::{Context, Op};
use crate::plans::{self, Footings};
use crate::roofer::Style;
use crate::{Base, Facing, Kind, Panel, STOREY_CUBES, Sample, Shape};
use glam::{IVec2, IVec3};
use std::collections::{BTreeMap, BTreeSet};

/// A stream of numbers from a seed (SplitMix64): enough for picking, and the same on every
/// platform.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        terra_gen::hash::splitmix64(self.0)
    }

    /// A number from `low` to `high`, both included.
    fn between(&mut self, low: i32, high: i32) -> i32 {
        low + (self.next() % (high - low + 1).max(1) as u64) as i32
    }

    fn pick<T: Copy>(&mut self, from: &[T]) -> T {
        from[(self.next() % from.len() as u64) as usize]
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

/// One use of a tool, in tiles (a column of cubes a storey high) or in cubes as the tool
/// takes them.
#[derive(Clone, Debug, PartialEq)]
pub enum Element {
    /// A room on a storey, its corner tiles both included: a floor, four walls and a flat
    /// roof, in a kind.
    Room {
        min: [i32; 2],
        max: [i32; 2],
        storey: i32,
        kind: Kind,
    },
    /// A wall a storey high along a line of tiles.
    Wall {
        from: [i32; 2],
        to: [i32; 2],
        storey: i32,
        kind: Kind,
    },
    /// A door, or a window, along a line of tiles.
    Opening {
        from: [i32; 2],
        to: [i32; 2],
        storey: i32,
        door: bool,
    },
    /// A roof over the room round a tile.
    Roof {
        tile: [i32; 2],
        storey: i32,
        style: Style,
        along_x: Option<bool>,
        kind: Kind,
    },
    /// A box of blocks of a shape, a brush's, the cube given in it.
    Blocks {
        at: [i32; 3],
        size: i32,
        shape: Shape,
        kind: Kind,
    },
    /// A deck a storey up on a post at each corner of its tiles.
    Deck {
        min: [i32; 2],
        max: [i32; 2],
        kind: Kind,
    },
    /// A flight of stairs, or a ramp, from its foot.
    Flight {
        foot: [i32; 3],
        facing: Facing,
        ramp: bool,
        kind: Kind,
    },
    /// A box of cubes carved out.
    Carve { at: [i32; 3], size: i32 },
    /// A box of cubes changed to another kind.
    Change { at: [i32; 3], size: i32, kind: Kind },
}

/// The elements a seed builds, in the kits given: a room or a wall first, then two to five
/// more, each near what is there.
pub fn elements(seed: u64, kits: &Kits) -> Vec<Element> {
    let mut rng = Rng(seed.wrapping_mul(0xA24B_AED4_963E_E407) ^ 0x5851_F42D_4C95_7F2D);
    let walls: Vec<Kind> = kits
        .0
        .iter()
        .filter(|kit| kit.pattern != Pattern::Roof)
        .map(|kit| kit.kind())
        .collect();
    let roofs: Vec<Kind> = kits
        .0
        .iter()
        .filter(|kit| kit.pattern == Pattern::Roof)
        .map(|kit| kit.kind())
        .collect();
    let timber: Vec<Kind> = kits
        .0
        .iter()
        .filter(|kit| kit.pattern == Pattern::Logs)
        .map(|kit| kit.kind())
        .collect();
    if walls.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    // The rooms and walls laid, as [min, max] tiles and storey, to put the rest by.
    let mut rooms: Vec<([i32; 2], [i32; 2], i32)> = Vec::new();
    let mut runs: Vec<([i32; 2], [i32; 2], i32)> = Vec::new();
    let room = |rng: &mut Rng, storey: i32, over: Option<([i32; 2], [i32; 2])>| {
        let (min, max) = match over {
            Some(below) => below,
            None => {
                let min = [rng.between(0, 4), rng.between(0, 4)];
                (
                    min,
                    [min[0] + rng.between(3, 7), min[1] + rng.between(3, 6)],
                )
            }
        };
        (min, max, storey)
    };
    let first = if rng.chance(65) {
        let (min, max, storey) = room(&mut rng, 0, None);
        rooms.push((min, max, storey));
        Element::Room {
            min,
            max,
            storey,
            kind: rng.pick(&walls),
        }
    } else {
        let from = [rng.between(0, 4), rng.between(0, 4)];
        let to = if rng.chance(50) {
            [from[0] + rng.between(2, 7), from[1]]
        } else {
            [from[0], from[1] + rng.between(2, 7)]
        };
        runs.push((from, to, 0));
        Element::Wall {
            from,
            to,
            storey: 0,
            kind: rng.pick(&walls),
        }
    };
    out.push(first);
    for _ in 0..rng.between(2, 5) {
        let element = match rng.between(0, 9) {
            0 => {
                // Another room: beside, or a storey over one there is.
                if let Some(&(min, max, storey)) = rooms.last().filter(|_| rng.chance(50)) {
                    let (min, max, storey) = room(&mut rng, storey + 1, Some((min, max)));
                    rooms.push((min, max, storey));
                    Element::Room {
                        min,
                        max,
                        storey,
                        kind: rng.pick(&walls),
                    }
                } else {
                    let (min, max, storey) = room(&mut rng, 0, None);
                    rooms.push((min, max, storey));
                    Element::Room {
                        min,
                        max,
                        storey,
                        kind: rng.pick(&walls),
                    }
                }
            }
            1 => {
                let from = [rng.between(-2, 10), rng.between(-2, 10)];
                let to = if rng.chance(50) {
                    [from[0] + rng.between(1, 6), from[1]]
                } else {
                    [from[0], from[1] + rng.between(1, 6)]
                };
                runs.push((from, to, 0));
                Element::Wall {
                    from,
                    to,
                    storey: 0,
                    kind: rng.pick(&walls),
                }
            }
            2 | 3 => {
                // A door or a window in a side of a room or along a wall, a tile or two
                // wide, clear of its ends.
                let sides: Vec<([i32; 2], [i32; 2], i32)> = rooms
                    .iter()
                    .flat_map(|&(min, max, storey)| {
                        [
                            ([min[0], min[1]], [max[0], min[1]], storey),
                            ([min[0], max[1]], [max[0], max[1]], storey),
                            ([min[0], min[1]], [min[0], max[1]], storey),
                            ([max[0], min[1]], [max[0], max[1]], storey),
                        ]
                    })
                    .chain(runs.iter().copied())
                    .collect();
                if sides.is_empty() {
                    continue;
                }
                let (from, to, storey) = rng.pick(&sides);
                let along_x = from[1] == to[1];
                let length = if along_x {
                    to[0] - from[0]
                } else {
                    to[1] - from[1]
                };
                if length < 2 {
                    continue;
                }
                let start = rng.between(1, length - 1);
                let wide = rng.between(0, 1).min(length - 1 - start);
                let at = |i: i32| {
                    if along_x {
                        [from[0] + i, from[1]]
                    } else {
                        [from[0], from[1] + i]
                    }
                };
                Element::Opening {
                    from: at(start),
                    to: at(start + wide),
                    storey,
                    door: storey == 0 && rng.chance(50),
                }
            }
            4 => {
                let Some(&(min, max, storey)) =
                    rooms.get(rng.between(0, rooms.len() as i32 - 1).max(0) as usize)
                else {
                    continue;
                };
                if roofs.is_empty() {
                    continue;
                }
                Element::Roof {
                    tile: [(min[0] + max[0]) / 2, (min[1] + max[1]) / 2],
                    storey,
                    style: rng.pick(&[Style::Gable, Style::Hip, Style::LeanTo]),
                    along_x: rng.pick(&[None, Some(true), Some(false)]),
                    kind: rng.pick(&roofs),
                }
            }
            5 | 6 => {
                // A box of blocks by or on what stands: a roof kind takes a roof's shapes.
                let kinds: Vec<Kind> = walls.iter().chain(&roofs).copied().collect();
                let kind = rng.pick(&kinds);
                let shapes: Vec<Shape> = Shape::ALL
                    .into_iter()
                    .filter(|shape| !kind.is_roof() || shape.roofs())
                    .collect();
                Element::Blocks {
                    at: [
                        rng.between(-2, 11),
                        rng.between(-1, STOREY_CUBES + 1),
                        rng.between(-2, 11),
                    ],
                    size: rng.pick(&[1, 1, 2]),
                    shape: rng.pick(&shapes),
                    kind,
                }
            }
            7 => {
                if timber.is_empty() {
                    continue;
                }
                let min = [rng.between(-4, 10), rng.between(-4, 10)];
                Element::Deck {
                    min,
                    max: [min[0] + rng.between(2, 5), min[1] + rng.between(2, 5)],
                    kind: rng.pick(&timber),
                }
            }
            8 => Element::Flight {
                foot: [rng.between(-3, 12), 0, rng.between(-3, 12)],
                facing: rng.pick(&Facing::ALL),
                ramp: rng.chance(30),
                kind: rng.pick(&walls),
            },
            _ => {
                let at = [
                    rng.between(0, 10),
                    rng.between(0, STOREY_CUBES),
                    rng.between(0, 10),
                ];
                let size = rng.pick(&[1, 2]);
                if rng.chance(50) {
                    Element::Carve { at, size }
                } else {
                    Element::Change {
                        at,
                        size,
                        kind: rng.pick(&walls),
                    }
                }
            }
        };
        out.push(element);
    }
    out
}

/// The ground under the fuzz's bases, in centimetres a column: falling a metre across
/// eight metres east and half that south, as the QA cabin's does.
fn ground_cm([x, z]: [i32; 2]) -> i16 {
    (-(x as f32) * 6.0 - z as f32 * 3.0) as i16
}

/// The base the elements build: each laid as the hammer lays it, with the ground under it
/// and footings in the first block kind, put through the checked operations (what the rules
/// refuse is left out, as a player's refused click is), then built.
pub fn build(kits: &Kits, elements: &[Element]) -> Base {
    let rules = kits.rules();
    let cx = Context::free(&rules);
    let footing = kits.footing().map(|kit| kit.kind());
    let footed: BTreeSet<Kind> = kits
        .0
        .iter()
        .filter(|kit| kit.pattern == Pattern::Masonry && kit.framing.is_none())
        .map(|kit| kit.kind())
        .collect();
    let families: BTreeMap<Kind, u8> = kits
        .0
        .iter()
        .map(|kit| (kit.kind(), plans::family(kit)))
        .collect();
    let blocks = |kind: Kind| kits.one_at_a_time(kind);
    let footings = || Footings::of(footing, &ground_cm, &footed);
    let mut base = Base::default();
    let tiles = |from: [i32; 2], to: [i32; 2], storey: i32| {
        plans::wall_tiles(IVec2::from_array(from), IVec2::from_array(to), storey)
    };
    for element in elements {
        let ops: Vec<Op> = match *element {
            Element::Room {
                min,
                max,
                storey,
                kind,
            } => plans::room(
                &base,
                IVec3::new(min[0], storey, min[1]),
                IVec3::new(max[0], storey, max[1]),
                kind,
                footing.unwrap_or(kind),
                footings(),
            ),
            Element::Wall {
                from,
                to,
                storey,
                kind,
            } => plans::wall(&base, &tiles(from, to, storey), kind, footings()),
            Element::Opening {
                from,
                to,
                storey,
                door,
            } => {
                let panel = if door { Panel::Door } else { Panel::WINDOW };
                let cubes = plans::opening_cubes(&tiles(from, to, storey), panel);
                plans::opening(&base, &cubes, footings())
            }
            Element::Roof {
                tile,
                storey,
                style,
                along_x,
                kind,
            } => {
                let inside = IVec3::new(tile[0], crate::wall_layer(storey), tile[1]);
                let footprint = plans::room_footprint(&base, inside);
                plans::roof(&base, &footprint, storey, style, along_x, kind)
            }
            Element::Blocks {
                at,
                size,
                shape,
                kind,
            } => {
                let cubes = plans::brush(IVec3::from_array(at), size);
                plans::blocks(&base, &cubes, shape, kind, &rules, footings())
            }
            Element::Deck { min, max, kind } => {
                let mut ops = Vec::new();
                for [x, z] in [min, [max[0], min[1]], [min[0], max[1]], max] {
                    ops.extend(plans::post(&base, IVec3::new(x, 0, z), kind, footings()));
                }
                // Posts first, so the deck has them to stand on.
                lay(&mut base, &cx, ops);
                plans::floor(
                    &base,
                    IVec3::new(min[0], 1, min[1]),
                    IVec3::new(max[0], 1, max[1]),
                    kind,
                    footings(),
                )
            }
            Element::Flight {
                foot,
                facing,
                ramp,
                kind,
            } => plans::flight(
                &base,
                IVec3::from_array(foot),
                facing,
                1,
                plans::flight_top(facing, ramp),
                kind,
                footings(),
            ),
            Element::Carve { at, size } => plans::carve(
                &base,
                &plans::brush(IVec3::from_array(at), size),
                footings(),
            ),
            Element::Change { at, size, kind } => plans::change(
                &base,
                &plans::brush(IVec3::from_array(at), size),
                kind,
                &rules,
                &families,
                blocks,
            ),
        };
        lay(&mut base, &cx, ops);
        super::qa_cabin::build(&mut base, &cx, &blocks);
    }
    base
}

/// Operations put to the base with the ground under their columns; what is refused, left.
fn lay(base: &mut Base, cx: &Context, ops: Vec<Op>) {
    let sample = |column| Sample {
        column,
        height_cm: ground_cm(column),
    };
    for op in plans::with_ground(ops, sample) {
        let _ = base.act(&op, cx);
    }
}

/// The fewest of `elements` that still build a base `fails`: each taken out in turn, from
/// the last, while the rest still fail, and a box of two cubes made one; and how many bases
/// it built to find them.
pub fn shrink(
    elements: &[Element],
    kits: &Kits,
    fails: &dyn Fn(&Base) -> bool,
) -> (Vec<Element>, usize) {
    let mut kept = elements.to_vec();
    let mut tries = 0;
    loop {
        let mut smaller = false;
        for i in (0..kept.len()).rev() {
            if kept.len() == 1 {
                break;
            }
            let mut fewer = kept.clone();
            fewer.remove(i);
            tries += 1;
            if fails(&build(kits, &fewer)) {
                kept = fewer;
                smaller = true;
            }
        }
        for i in 0..kept.len() {
            let mut less = kept.clone();
            match &mut less[i] {
                Element::Blocks { size, .. }
                | Element::Carve { size, .. }
                | Element::Change { size, .. }
                    if *size > 1 =>
                {
                    *size = 1;
                }
                _ => continue,
            }
            tries += 1;
            if fails(&build(kits, &less)) {
                kept = less;
                smaller = true;
            }
        }
        if !smaller {
            return (kept, tries);
        }
    }
}

/// Elements as Rust, to paste into a test: `kinds` names each kind by its file.
pub fn as_rust(elements: &[Element], kinds: &dyn Fn(Kind) -> String) -> String {
    let kind = |k: Kind| format!("kind(\"{}\")", kinds(k));
    let facing = |f: Facing| format!("Facing::{f:?}");
    let shape = |s: Shape| match s {
        Shape::Stair(f)
        | Shape::Slope(f)
        | Shape::InvertedStair(f)
        | Shape::InvertedSlope(f)
        | Shape::OuterCorner(f)
        | Shape::InnerCorner(f) => {
            let name = format!("{s:?}");
            let name = name.split('(').next().unwrap_or_default().to_string();
            format!("Shape::{name}({})", facing(f))
        }
        other => format!("Shape::{other:?}"),
    };
    let lines: Vec<String> = elements
        .iter()
        .map(|element| match *element {
            Element::Room {
                min,
                max,
                storey,
                kind: k,
            } => format!(
                "Room {{ min: {min:?}, max: {max:?}, storey: {storey}, kind: {} }}",
                kind(k)
            ),
            Element::Wall {
                from,
                to,
                storey,
                kind: k,
            } => format!(
                "Wall {{ from: {from:?}, to: {to:?}, storey: {storey}, kind: {} }}",
                kind(k)
            ),
            Element::Opening {
                from,
                to,
                storey,
                door,
            } => format!(
                "Opening {{ from: {from:?}, to: {to:?}, storey: {storey}, door: {door} }}"
            ),
            Element::Roof {
                tile,
                storey,
                style,
                along_x,
                kind: k,
            } => format!(
                "Roof {{ tile: {tile:?}, storey: {storey}, style: Style::{style:?}, along_x: {along_x:?}, kind: {} }}",
                kind(k)
            ),
            Element::Blocks {
                at,
                size,
                shape: s,
                kind: k,
            } => format!(
                "Blocks {{ at: {at:?}, size: {size}, shape: {}, kind: {} }}",
                shape(s),
                kind(k)
            ),
            Element::Deck { min, max, kind: k } => {
                format!("Deck {{ min: {min:?}, max: {max:?}, kind: {} }}", kind(k))
            }
            Element::Flight {
                foot,
                facing: f,
                ramp,
                kind: k,
            } => format!(
                "Flight {{ foot: {foot:?}, facing: {}, ramp: {ramp}, kind: {} }}",
                facing(f),
                kind(k)
            ),
            Element::Carve { at, size } => format!("Carve {{ at: {at:?}, size: {size} }}"),
            Element::Change { at, size, kind: k } => format!(
                "Change {{ at: {at:?}, size: {size}, kind: {} }}",
                kind(k)
            ),
        })
        .collect();
    format!("[\n    {},\n]", lines.join(",\n    "))
}

#[cfg(test)]
mod tests {
    use super::Element::*;
    use super::*;
    use crate::scenes::registry::Materials;

    fn baseline() -> Materials {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        Materials::read(&assets, true).expect("the baseline's materials")
    }

    #[test]
    fn a_seed_builds_the_same_base_and_seeds_differ() {
        let materials = baseline();
        let kits = &materials.kits;
        assert_eq!(elements(7, kits), elements(7, kits));
        let a = build(kits, &elements(7, kits));
        let b = build(kits, &elements(7, kits));
        assert_eq!(a.cubes().collect::<Vec<_>>(), b.cubes().collect::<Vec<_>>());
        let kinds: BTreeSet<usize> = (0..16).map(|seed| elements(seed, kits).len()).collect();
        assert!(kinds.len() > 1, "every seed the same length");
        assert_ne!(elements(1, kits), elements(2, kits));
    }

    #[test]
    fn what_the_elements_lay_is_built_through_the_rules() {
        // A room of logs with a door, a gable over it: laid by the hammer's plans and built
        // whole, its doorway open.
        let materials = baseline();
        let kits = &materials.kits;
        let named = |name: &str| {
            materials
                .names
                .iter()
                .find(|(_, file)| file.as_str() == name)
                .map(|(&kind, _)| kind)
                .unwrap()
        };
        let base = build(
            kits,
            &[
                Room {
                    min: [0, 0],
                    max: [5, 4],
                    storey: 0,
                    kind: named("logs"),
                },
                Opening {
                    from: [2, 0],
                    to: [3, 0],
                    storey: 0,
                    door: true,
                },
                Roof {
                    tile: [2, 2],
                    storey: 0,
                    style: Style::Gable,
                    along_x: None,
                    kind: named("shakes"),
                },
            ],
        );
        let waiting = base
            .cubes()
            .filter(|(_, cube)| cube.fill != crate::Fill::Open && !cube.is_built())
            .count();
        assert_eq!(waiting, 0, "everything laid is built");
        assert!(base.kinds().any(|kind| kind == named("shakes")), "the roof");
        let door = base.cube(IVec3::new(2, 1, 0)).map(|cube| cube.fill);
        assert_eq!(door, Some(crate::Fill::Open), "the doorway");
    }

    #[test]
    fn fuzz_seeds_0_3_and_37_every_boarded_timber_cube_alone_is_drawn_without_coplanar() {
        // Found by `cargo xtask fuzz` at seeds 0, 3, 4, 37, 38, 98, 99, 140 and 148, each
        // shrunk to one cube: a timber cube cut upside down, to an upper half or a corner,
        // boarded as a box whose boards' sawn ends lay in the plane of its other boards'
        // faces (the voxel contract's gap 10). Every such shape, each way, in logs and hewn
        // timber, alone on the sloping ground, each its own box.
        let materials = baseline();
        let kind = |name: &str| {
            materials
                .names
                .iter()
                .find(|(_, n)| n.as_str() == name)
                .map(|(&k, _)| k)
                .unwrap()
        };
        let boarded: Vec<Shape> = Shape::ALL
            .into_iter()
            .filter(|shape| {
                matches!(
                    shape,
                    Shape::UpperHalf
                        | Shape::InvertedStair(_)
                        | Shape::InvertedSlope(_)
                        | Shape::OuterCorner(_)
                        | Shape::InnerCorner(_)
                )
            })
            .collect();
        let mut elements = Vec::new();
        for (row, timber) in ["logs", "hewn"].into_iter().enumerate() {
            for (i, &shape) in boarded.iter().enumerate() {
                elements.push(Blocks {
                    at: [i as i32 * 3, 0, row as i32 * 3],
                    size: 1,
                    shape,
                    kind: kind(timber),
                });
            }
        }
        let base = build(&materials.kits, &elements);
        let report = crate::inspect::inspect(
            &base,
            &materials.kits,
            &crate::relief::Maps::worst(&materials.kits, false),
        );
        let fought: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.fault == crate::inspect::Fault::Coplanar)
            .collect();
        assert!(fought.is_empty(), "{fought:#?}");
    }

    #[test]
    fn a_failing_base_shrinks_to_the_element_that_fails_it() {
        // Fails while a box of hewn timber is in it: the room and the wall go, the box
        // stays, made one cube.
        let materials = baseline();
        let kits = &materials.kits;
        let named = |name: &str| {
            materials
                .names
                .iter()
                .find(|(_, file)| file.as_str() == name)
                .map(|(&kind, _)| kind)
                .unwrap()
        };
        let (hewn, logs) = (named("hewn"), named("logs"));
        let all = vec![
            Room {
                min: [0, 0],
                max: [4, 4],
                storey: 0,
                kind: logs,
            },
            Blocks {
                at: [8, 0, 8],
                size: 2,
                shape: Shape::Whole,
                kind: hewn,
            },
            Wall {
                from: [0, 7],
                to: [4, 7],
                storey: 0,
                kind: logs,
            },
        ];
        let fails = |base: &Base| base.kinds().any(|kind| kind == hewn);
        let (kept, tries) = shrink(&all, kits, &fails);
        assert_eq!(
            kept,
            vec![Blocks {
                at: [8, 0, 8],
                size: 1,
                shape: Shape::Whole,
                kind: hewn,
            }]
        );
        assert!(tries >= 3);
        let rust = as_rust(&kept, &|_| "hewn".into());
        assert!(
            rust.contains(
                "Blocks { at: [8, 0, 8], size: 1, shape: Shape::Whole, kind: kind(\"hewn\") }"
            ),
            "{rust}"
        );
    }
}
