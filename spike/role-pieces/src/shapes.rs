//! The shapes every candidate is drawn on, in cubes: Keen's gallery test piece in stone and
//! in logs, and our own cases. Each is a base laid by the game's own operations, the ground
//! under it, and the Enshrouded gallery picture it is judged beside.

use terra_build::{Base, BuildOp, Cube, Fill, Kind, Panel, RoofSection, RoofStyle};

pub const LOGS: Kind = Kind(1);
pub const STONE: Kind = Kind(2);

/// The ground's height in metres at a point east and south, in the base's own space.
pub type Ground = fn(f32, f32) -> f32;

pub struct Case {
    pub name: &'static str,
    pub base: Base,
    pub ground: Ground,
    /// How far down the whole is moved to stand on the yard: the ground's height at the
    /// base's middle.
    pub sink: f32,
    /// Whether doors and windows are fitted in its holes.
    pub fitted: bool,
}

fn flat(_: f32, _: f32) -> f32 {
    0.0
}

/// Keen's piece stands with its platform's foot on layer 3, so its deck is a floor layer.
fn keen_ground(_: f32, _: f32) -> f32 {
    1.5
}

/// Rising a metre for every four east, to the base's floor at its east end.
fn slope(x: f32, _: f32) -> f32 {
    0.25 * x - 1.5
}

fn fill(base: &mut Base, min: [i32; 3], max: [i32; 3], kind: Kind, fill: Fill) {
    base.apply(BuildOp::Fill {
        min,
        max,
        cube: Some(Cube::new(kind, fill)),
    });
}

fn wall(x: i32, z: i32, panel: Panel, kind: Kind) -> BuildOp {
    BuildOp::SetWall {
        tile: [x, 0, z],
        panel: Some(panel),
        kind,
    }
}

fn east_west(from: i32, to: i32, z: i32, kind: Kind) -> Vec<BuildOp> {
    (from..=to)
        .map(|x| wall(x, z, Panel::Solid, kind))
        .collect()
}

fn north_south(from: i32, to: i32, x: i32, kind: Kind) -> Vec<BuildOp> {
    (from..=to)
        .map(|z| wall(x, z, Panel::Solid, kind))
        .collect()
}

fn of(ops: Vec<BuildOp>) -> Base {
    let mut base = Base::default();
    for op in ops {
        base.apply(op);
    }
    base
}

/// Keen's test piece, as every wiki gallery shows it: a platform reached by a flight of
/// steps, a wall with a window hole on it and a post beside, and a slab over them
/// overhanging the front. A timber platform is walls round a deck, as one is built.
pub fn keen(kind: Kind) -> Base {
    let mut base = Base::default();
    // The platform: two cubes and its deck, eight by six.
    if kind == STONE {
        fill(&mut base, [0, 3, 0], [7, 5, 5], kind, Fill::Solid);
    } else {
        fill(&mut base, [0, 3, 0], [7, 4, 0], kind, Fill::Solid);
        fill(&mut base, [0, 3, 5], [7, 4, 5], kind, Fill::Solid);
        fill(&mut base, [0, 3, 0], [0, 4, 5], kind, Fill::Solid);
        fill(&mut base, [7, 3, 0], [7, 4, 5], kind, Fill::Solid);
        fill(&mut base, [0, 5, 0], [7, 5, 5], kind, Fill::Solid);
    }
    // The steps up the west side, from the front to the back, each a cube higher. Timber
    // lays a solid mass as logs crossing both ways, a pile of ends, so its flight is a
    // stepped wall a cube thick, as a log stair's stringer is.
    let wide = if kind == STONE { -3 } else { -1 };
    for step in 0..7 {
        let z = 6 - step;
        fill(&mut base, [wide, 3, z], [-1, 3 + step, z], kind, Fill::Solid);
    }
    // The wall along the back, with a window a cube wide and two high, a cube from its end.
    fill(&mut base, [0, 6, 0], [4, 10, 0], kind, Fill::Solid);
    fill(&mut base, [1, 8, 0], [2, 9, 0], kind, Fill::Open);
    // A post at the back corner.
    fill(&mut base, [7, 6, 0], [7, 10, 0], kind, Fill::Solid);
    // The slab over them, overhanging the steps and the front.
    fill(&mut base, [-2, 11, -1], [8, 11, 3], kind, Fill::Solid);
    base
}

/// A wall of logs stands on a course of stone, as the game's cabins stand on footings.
fn footed(kind: Kind, ops: Vec<BuildOp>) -> Base {
    let mut base = of(ops);
    if kind == LOGS {
        let cubes: Vec<_> = base
            .cubes()
            .filter(|(at, cube)| at.y == 0 && cube.kind == LOGS)
            .map(|(at, cube)| (at, cube.fill))
            .collect();
        for (at, fill_was) in cubes {
            if fill_was == Fill::Solid {
                fill(&mut base, at.to_array(), at.to_array(), STONE, Fill::Solid);
            }
        }
    }
    base
}

fn named(kind: Kind) -> &'static str {
    if kind == STONE { "stone" } else { "logs" }
}

pub fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    let mut add = |name: &'static str, base: Base, ground: Ground, sink: f32, fitted: bool| {
        out.push(Case {
            name,
            base,
            ground,
            sink,
            fitted,
        })
    };
    for kind in [STONE, LOGS] {
        let leak = |text: String| -> &'static str { Box::leak(text.into_boxed_str()) };
        let n = |what: &str| leak(format!("{what}-{}", named(kind)));
        add(n("keen"), keen(kind), keen_ground, 1.5, false);
        add(
            n("straight"),
            footed(kind, east_west(0, 9, 0, kind)),
            flat,
            0.0,
            true,
        );
        add(
            n("L"),
            footed(
                kind,
                [east_west(0, 7, 0, kind), north_south(1, 6, 0, kind)].concat(),
            ),
            flat,
            0.0,
            true,
        );
        add(
            n("T"),
            footed(
                kind,
                [east_west(0, 9, 0, kind), north_south(1, 5, 4, kind)].concat(),
            ),
            flat,
            0.0,
            true,
        );
        let mut openings = east_west(0, 11, 0, kind);
        openings.extend((2..=3).map(|x| wall(x, 0, Panel::Door, kind)));
        openings.extend((7..=8).map(|x| wall(x, 0, Panel::WINDOW, kind)));
        add(n("openings"), footed(kind, openings), flat, 0.0, true);
        // Part-built: going up from one end, as a crew lays it, the rest not yet there.
        let mut part = footed(kind, [east_west(0, 9, 0, kind), north_south(1, 5, 0, kind)].concat());
        let high = |x: i32, z: i32| -> i32 {
            if z > 0 {
                3
            } else if x < 4 {
                3
            } else if x < 7 {
                2
            } else {
                1
            }
        };
        let taken: Vec<_> = part
            .cubes()
            .filter(|(at, _)| at.y > high(at.x, at.z))
            .map(|(at, _)| at)
            .collect();
        for at in taken {
            part.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: None,
            });
        }
        add(n("part-built"), part, flat, 0.0, false);
        // A wall along a slope: a level top, its foot stepping down into the ground.
        let mut sloped = Base::default();
        for x in 0..12 {
            let ground = slope(x as f32 * 0.5 + 0.25, 0.0);
            let foot = (ground / 0.5).floor() as i32 - 1;
            if kind == LOGS {
                fill(&mut sloped, [x, foot, 0], [x, -1, 0], STONE, Fill::Solid);
                fill(&mut sloped, [x, 0, 0], [x, 4, 0], LOGS, Fill::Solid);
            } else {
                fill(&mut sloped, [x, foot, 0], [x, 4, 0], STONE, Fill::Solid);
            }
        }
        add(n("slope"), sloped, slope, slope(3.0, 0.0), false);
        // A cabin under a gable roof, a door in its south wall and a window east of it.
        let mut cabin = vec![BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind,
        }];
        cabin.extend((3..=4).map(|x| wall(x, 7, Panel::Door, kind)));
        cabin.extend((6..=7).map(|x| wall(x, 7, Panel::WINDOW, kind)));
        cabin.extend((2..=3).map(|z| wall(0, z + 2, Panel::WINDOW, kind)));
        cabin.push(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [9, 0, 7],
            style: RoofStyle::Gable,
            along_x: true,
            pitch: 0.7,
        }));
        add(n("cabin"), footed(kind, cabin), flat, 0.0, true);
    }
    // Stone meeting logs: an L, stone one way and logs the other, and a T, stone through.
    add(
        "mixed-L",
        footed(
            LOGS,
            [east_west(0, 7, 0, STONE), north_south(1, 6, 0, LOGS)].concat(),
        ),
        flat,
        0.0,
        true,
    );
    add(
        "mixed-T",
        footed(
            LOGS,
            [east_west(0, 9, 0, STONE), north_south(1, 5, 4, LOGS)].concat(),
        ),
        flat,
        0.0,
        true,
    );
    out
}
