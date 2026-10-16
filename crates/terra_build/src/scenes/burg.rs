//! A castle that burns (D377): a hill castle of stone with its upper works of timber, as
//! central European castles were built, put up whole as an example base (`build.example
//! {name: "burg"}`) for fire to be seen in. Stone walls, towers and a keep; timber in all the
//! places a castle had it, each of which a fire takes:
//!
//! - **Hoardings** (hourds): covered timber galleries along the top of the curtain wall,
//!   carried out over its outer face on beams, under shake roofs, as sieges burned them.
//! - **Jettied timber top storeys** on the towers, the gatehouse and the keep, under tall
//!   shake roofs.
//! - **Timber floors** in the towers and the keep, their beams running through the walls as
//!   a course of bond timbers (timber lacing): the stone over them stands on wood, so when
//!   the course burns out all round, the stone above comes down.
//! - **The flanking towers stand clear of the curtain**, a hand's breadth of straight joint
//!   between them, joined only by the hoardings' timber bridge, so a tower can be cut off and
//!   held or lost on its own, as towers were.
//! - **A hall** of squared timber on a stone ground storey along the north wall, and log
//!   **sheds** under split shakes along the east and west walls.
//!
//! Laid out in its own cubes with the gate's middle at the origin on the south wall's outer
//! face, the ward to the north (-Z), as a village's buildings and [`super::castle`] are.

use crate::roofer::{self, Style};
use crate::{Base, BuildOp, Cube, Fill, Kind, STOREY_CUBES};
use std::collections::BTreeSet;

/// What the castle is built of: stone, squared timber, round logs and a covering of wood, the
/// settlement's own (its masonry, beams and timber, and its timber's roof).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Materials {
    pub stone: Kind,
    pub beams: Kind,
    pub logs: Kind,
    pub shakes: Kind,
}

/// The ward's outer extent, in cubes: x from `-WIDE / 2`, z from `-DEEP` to -1.
const WIDE: i32 = 88;
const DEEP: i32 = 72;
/// The curtain's thickness and height, and the hoardings' storey over it.
const THICK: i32 = 3;
const HIGH: i32 = 18;
/// A flanking tower's side and walls, how high its stone stands, and the course of bond
/// timbers through it.
const TOWER: i32 = 14;
const TOWER_WALL: i32 = 2;
const TOWER_STONE: i32 = 24;
const LACING: i32 = 11;
/// The keep's side, its stone's height, and its lacing.
const KEEP: i32 = 18;
const KEEP_STONE: i32 = 30;
const KEEP_LACING: i32 = 5;

type Box3 = ([i32; 3], [i32; 3]);
/// A roof to put up once the walls stand: its footprint, the storey under it, its style, which
/// way its ridge runs, and its covering.
type Roof = (BTreeSet<[i32; 2]>, i32, Style, Option<bool>, Kind);

struct Plan {
    ops: Vec<BuildOp>,
    roofs: Vec<Roof>,
}

impl Plan {
    fn fill(&mut self, (min, max): Box3, cube: Option<Cube>) {
        self.ops.push(BuildOp::Fill { min, max, cube });
    }

    fn solid(&mut self, b: Box3, kind: Kind) {
        self.fill(b, Some(Cube::new(kind, Fill::Solid)));
    }

    fn clear(&mut self, b: Box3) {
        self.fill(b, None);
    }

    /// A ring of walls round a box, `thick` cubes in from its sides, from `y0` to `y1`.
    fn ring(&mut self, [x0, z0, x1, z1]: [i32; 4], y0: i32, y1: i32, thick: i32, kind: Kind) {
        self.solid(([x0, y0, z0], [x1, y1, z1]), kind);
        self.clear(([x0 + thick, y0, z0 + thick], [x1 - thick, y1, z1 - thick]));
    }

    fn roof(
        &mut self,
        [x0, z0, x1, z1]: [i32; 4],
        storey: i32,
        style: Style,
        along_x: Option<bool>,
        kind: Kind,
    ) {
        let footprint = (x0..=x1)
            .flat_map(|x| (z0..=z1).map(move |z| [x, z]))
            .collect();
        self.roofs.push((footprint, storey, style, along_x, kind));
    }

    /// Windows a cube wide and two high every `every` cubes along each side of a ring, at
    /// `y`, through `thick` cubes of its wall.
    fn windows(&mut self, [x0, z0, x1, z1]: [i32; 4], y: i32, thick: i32, every: usize) {
        for x in (x0 + 3..x1 - 2).step_by(every) {
            self.clear(([x, y, z0], [x, y + 1, z0 + thick - 1]));
            self.clear(([x, y, z1 - thick + 1], [x, y + 1, z1]));
        }
        for z in (z0 + 3..z1 - 2).step_by(every) {
            self.clear(([x0, y, z], [x0 + thick - 1, y + 1, z]));
            self.clear(([x1 - thick + 1, y, z], [x1, y + 1, z]));
        }
    }

    /// A tower of stone to `stone`, hollow, with timber floors a storey apart whose course
    /// at `lacing` runs through its walls, and a jettied timber storey over it a cube out
    /// all round under a hip roof of `roof`.
    fn tower(
        &mut self,
        at: [i32; 2],
        side: i32,
        wall: i32,
        stone: i32,
        lacing: i32,
        m: &Materials,
    ) {
        let [x0, z0] = at;
        let (x1, z1) = (x0 + side - 1, z0 + side - 1);
        self.ring([x0, z0, x1, z1], 0, stone - 1, wall, m.stone);
        for y in (5..stone - 1).step_by(6) {
            if y == lacing {
                // The bond timbers, through the walls all round. Over the ground storey they
                // carry its floor, so the storey under them, its door at the foot, is a room a
                // fire fills; higher, the storey over them is open down to the floor under, a
                // hall two storeys high, so they face its air.
                self.ring([x0, z0, x1, z1], y, y, wall, m.beams);
                if y == STOREY_CUBES - 1 {
                    self.solid(
                        ([x0 + wall, y, z0 + wall], [x1 - wall, y, z1 - wall]),
                        m.beams,
                    );
                }
            } else {
                self.solid(
                    ([x0 + wall, y, z0 + wall], [x1 - wall, y, z1 - wall]),
                    m.beams,
                );
            }
        }
        for y in (2..stone - 3).step_by(6) {
            self.windows([x0, z0, x1, z1], y + 1, wall, 5);
        }
        // A door at its foot, on its south side.
        let mid = x0 + side / 2;
        self.clear(([mid - 1, 0, z1 - wall + 1], [mid, 3, z1]));
        // The jetty: its floor a cube out all round over the stone, its walls of squared
        // timber a storey high, windows in them.
        let jetty = [x0 - 1, z0 - 1, x1 + 1, z1 + 1];
        self.solid(
            ([x0 - 1, stone - 1, z0 - 1], [x1 + 1, stone - 1, z1 + 1]),
            m.beams,
        );
        self.ring(jetty, stone, stone + 5, 1, m.beams);
        self.windows(jetty, stone + 2, 1, 4);
        self.roof(jetty, (stone + 6) / 6 - 1, Style::Hip, None, m.shakes);
    }
}

/// The castle's shortcuts.
pub fn burg(m: Materials) -> Vec<BuildOp> {
    let mut plan = Plan {
        ops: Vec::new(),
        roofs: Vec::new(),
    };
    let (x0, x1) = (-WIDE / 2, WIDE / 2 - 1);
    let (z0, z1) = (-DEEP, -1);
    // The flanking towers at the corners, standing four cubes out past the curtain's faces.
    let corners = [
        [x0 - 4, z0 - 4],
        [x1 - TOWER + 5, z0 - 4],
        [x0 - 4, z1 - TOWER + 5],
        [x1 - TOWER + 5, z1 - TOWER + 5],
    ];
    let clear_of_towers = |x: i32, z: i32| {
        !corners.iter().any(|&[tx, tz]| {
            (tx - 1..=tx + TOWER).contains(&x) && (tz - 1..=tz + TOWER).contains(&z)
        })
    };
    // The curtain, each side a box, cut back a cube short of each tower, merlons on its
    // outer edge where no hoarding runs (by the gatehouse).
    let sides: [Box3; 4] = [
        ([x0, 0, z1 - THICK + 1], [x1, HIGH - 1, z1]),
        ([x0, 0, z0], [x1, HIGH - 1, z0 + THICK - 1]),
        ([x0, 0, z0], [x0 + THICK - 1, HIGH - 1, z1]),
        ([x1 - THICK + 1, 0, z0], [x1, HIGH - 1, z1]),
    ];
    for (min, max) in sides {
        // Each run of the side clear of the towers.
        let runs = runs(min, max, &clear_of_towers);
        for (a, b) in runs {
            plan.solid(([a[0], min[1], a[1]], [b[0], max[1], b[1]]), m.stone);
        }
    }
    // The gate through the south side, three metres wide and five high, under a gatehouse
    // of stone over the curtain, its top storey of timber.
    let (gx0, gx1, gz0) = (-7, 6, -12);
    plan.ring([gx0, gz0, gx1, z1], 0, TOWER_STONE - 1, 3, m.stone);
    plan.solid(([gx0 + 3, 11, gz0 + 3], [gx1 - 3, 11, z1 - 3]), m.beams);
    plan.solid(([gx0 + 3, 17, gz0 + 3], [gx1 - 3, 17, z1 - 3]), m.beams);
    plan.clear(([-3, 0, gz0], [2, 9, z1]));
    plan.windows([gx0, gz0, gx1, z1], 13, 3, 5);
    let gate_jetty = [gx0 - 1, gz0 - 1, gx1 + 1, z1 + 1];
    plan.solid(
        (
            [gx0 - 1, TOWER_STONE - 1, gz0 - 1],
            [gx1 + 1, TOWER_STONE - 1, z1 + 1],
        ),
        m.beams,
    );
    plan.ring(gate_jetty, TOWER_STONE, TOWER_STONE + 5, 1, m.beams);
    plan.windows(gate_jetty, TOWER_STONE + 2, 1, 4);
    plan.roof(gate_jetty, 4, Style::Hip, None, m.shakes);
    // Hoardings along every run of the curtain but the gatehouse's: a floor on beams carried
    // two cubes out over the outer face, timber walls a storey high along its outer edge and
    // the wall's inner face, loopholes in the outer, under a gable along the run. They run on
    // over the joint to each tower's stone, the timber bridge that joins it to the walls.
    let gate_x = gx0 - 1..=gx1 + 1;
    let hoardings: [(Box3, [i32; 3]); 4] = [
        (sides[0], [0, 0, 1]),
        (sides[1], [0, 0, -1]),
        (sides[2], [-1, 0, 0]),
        (sides[3], [1, 0, 0]),
    ];
    for ((min, max), out) in hoardings {
        let along_x = out[2] != 0;
        // Its strip of columns: the wall's thickness and two cubes out.
        let strip = |from: i32, to: i32| -> [i32; 4] {
            if along_x {
                let (za, zb) = if out[2] > 0 {
                    (min[2], max[2] + 2)
                } else {
                    (min[2] - 2, max[2])
                };
                [from, za, to, zb]
            } else {
                let (xa, xb) = if out[0] > 0 {
                    (min[0], max[0] + 2)
                } else {
                    (min[0] - 2, max[0])
                };
                [xa, from, xb, to]
            }
        };
        // Each run of the wall between towers, a cube longer at each end over the joint, and
        // on the south side either side of the gatehouse.
        let mut pieces: Vec<(i32, i32)> = Vec::new();
        for (a, b) in runs(min, max, &clear_of_towers) {
            let (from, to) = if along_x {
                (a[0] - 1, b[0] + 1)
            } else {
                (a[1] - 1, b[1] + 1)
            };
            if along_x && out[2] > 0 {
                pieces.push((from, *gate_x.start() - 1));
                pieces.push((*gate_x.end() + 1, to));
            } else {
                pieces.push((from, to));
            }
        }
        for (from, to) in pieces {
            let [sx0, sz0, sx1, sz1] = strip(from, to);
            // The floor over the wall's top and out.
            plan.solid(([sx0, HIGH - 1, sz0], [sx1, HIGH - 1, sz1]), m.beams);
            // Its two long walls, outer and inner, a storey high.
            let (outer, inner): (Box3, Box3) = if along_x {
                let (zo, zi) = if out[2] > 0 { (sz1, sz0) } else { (sz0, sz1) };
                (
                    ([sx0, HIGH, zo], [sx1, HIGH + 5, zo]),
                    ([sx0, HIGH, zi], [sx1, HIGH + 5, zi]),
                )
            } else {
                let (xo, xi) = if out[0] > 0 { (sx1, sx0) } else { (sx0, sx1) };
                (
                    ([xo, HIGH, sz0], [xo, HIGH + 5, sz1]),
                    ([xi, HIGH, sz0], [xi, HIGH + 5, sz1]),
                )
            };
            plan.solid(outer, m.beams);
            plan.solid(inner, m.beams);
            // Loopholes in the outer wall every three cubes.
            let (lo, hi) = if along_x { (sx0, sx1) } else { (sz0, sz1) };
            for v in (lo + 2..hi - 1).step_by(3) {
                let at = if along_x {
                    [v, HIGH + 2, outer.0[2]]
                } else {
                    [outer.0[0], HIGH + 2, v]
                };
                plan.clear((at, [at[0], at[1] + 1, at[2]]));
            }
            plan.roof(
                [sx0, sz0, sx1, sz1],
                HIGH / 6,
                Style::Gable,
                Some(along_x),
                m.shakes,
            );
        }
    }
    // The flanking towers.
    for at in corners {
        plan.tower(at, TOWER, TOWER_WALL, TOWER_STONE, LACING, &m);
    }
    // The keep, standing free in the ward north of the gate.
    plan.tower([-KEEP / 2, -48], KEEP, 2, KEEP_STONE, KEEP_LACING, &m);
    // The hall along the north wall, a cube clear of it: a ground storey of stone, two of
    // squared timber over it, under a gable.
    let hall = [-32, z0 + THICK + 1, 23, z0 + THICK + 12];
    plan.ring(hall, 0, 5, 2, m.stone);
    plan.solid(
        ([hall[0] + 2, 5, hall[1] + 2], [hall[2] - 2, 5, hall[3] - 2]),
        m.beams,
    );
    plan.ring(hall, 6, 17, 1, m.beams);
    plan.solid(
        (
            [hall[0] + 1, 11, hall[1] + 1],
            [hall[2] - 1, 11, hall[3] - 1],
        ),
        m.beams,
    );
    plan.windows(hall, 2, 2, 6);
    plan.windows(hall, 8, 1, 4);
    plan.windows(hall, 14, 1, 4);
    plan.clear(([-3, 0, hall[3] - 1], [-2, 3, hall[3]]));
    plan.roof(hall, 2, Style::Gable, Some(true), m.shakes);
    // Sheds of logs along the east and west walls, under shakes.
    for (sx0, sx1) in [
        (x0 + THICK + 1, x0 + THICK + 7),
        (x1 - THICK - 7, x1 - THICK - 1),
    ] {
        for (sz0, sz1) in [(-52, -41), (-36, -25)] {
            let shed = [sx0, sz0, sx1, sz1];
            plan.ring(shed, 0, 5, 1, m.logs);
            let door = (sz0 + sz1) / 2;
            let face = if sx0 < 0 { sx1 } else { sx0 };
            plan.clear(([face, 0, door], [face, 3, door + 1]));
            plan.roof(shed, 0, Style::Gable, Some(false), m.shakes);
        }
    }
    // The roofs, each over what stands by then.
    let mut walls = Base::from_shortcuts(&plan.ops);
    let mut ops = plan.ops;
    for (footprint, storey, style, along_x, kind) in plan.roofs {
        let height = roofer::field(&footprint, style, along_x);
        let roofed = roofer::put_up(&walls, &footprint, storey, kind, &height);
        walls.apply_shortcuts(&roofed);
        ops.extend(roofed);
    }
    ops
}

/// The runs of a box's columns, in x and z, along its longer side, where `keep` holds of
/// every column across it: each as its first and last column.
fn runs(
    min: [i32; 3],
    max: [i32; 3],
    keep: &dyn Fn(i32, i32) -> bool,
) -> Vec<([i32; 2], [i32; 2])> {
    let along_x = max[0] - min[0] >= max[2] - min[2];
    let (lo, hi) = if along_x {
        (min[0], max[0])
    } else {
        (min[2], max[2])
    };
    let held = |v: i32| {
        if along_x {
            (min[2]..=max[2]).all(|z| keep(v, z))
        } else {
            (min[0]..=max[0]).all(|x| keep(x, v))
        }
    };
    let mut out = Vec::new();
    let mut start = None;
    for v in lo..=hi + 1 {
        match (v <= hi && held(v), start) {
            (true, None) => start = Some(v),
            (false, Some(s)) => {
                out.push(if along_x {
                    ([s, min[2]], [v - 1, max[2]])
                } else {
                    ([min[0], s], [max[0], v - 1])
                });
                start = None;
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost::Rules;
    use crate::ops::{Context, Op};
    use glam::IVec3;

    const M: Materials = Materials {
        stone: Kind(2),
        beams: Kind(3),
        logs: Kind(1),
        shakes: Kind(0x800),
    };

    #[test]
    fn the_burg_stands_and_its_timber_burns_where_a_castles_did() {
        let base = Base::from_shortcuts(&burg(M));
        assert!(base.floating().is_empty(), "{:?}", &base.floating()[..5]);
        let count = |kind: Kind| base.cubes().filter(|(_, c)| c.kind == kind).count();
        assert!(count(M.stone) > 20_000, "{} stone", count(M.stone));
        for kind in [M.beams, M.logs, M.shakes] {
            assert!(count(kind) > 200, "{} of {kind:?}", count(kind));
        }
    }

    #[test]
    fn with_its_timber_gone_the_towers_tops_fall_and_the_curtain_stands() {
        let mut base = Base::from_shortcuts(&burg(M));
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let stone_in = |base: &Base,
                        x: std::ops::RangeInclusive<i32>,
                        y: std::ops::RangeInclusive<i32>,
                        z: std::ops::RangeInclusive<i32>| {
            base.cubes()
                .filter(|(at, c)| {
                    c.kind == M.stone && x.contains(&at.x) && y.contains(&at.y) && z.contains(&at.z)
                })
                .count()
        };
        // The north curtain between its towers, and the north-west tower over its lacing.
        let curtain =
            |base: &Base| stone_in(base, -30..=30, 0..=HIGH - 1, -DEEP..=-DEEP + THICK - 1);
        let tower = |base: &Base| {
            stone_in(
                base,
                -WIDE / 2 - 4..=-WIDE / 2 - 4 + TOWER - 1,
                LACING + 1..=TOWER_STONE,
                -DEEP - 4..=-DEEP - 4 + TOWER - 1,
            )
        };
        let (walls, top) = (curtain(&base), tower(&base));
        assert!(walls > 500 && top > 300, "{walls} {top}");
        let timber: Vec<[i32; 3]> = base
            .cubes()
            .filter(|(_, c)| c.kind == M.beams)
            .map(|(at, _)| at.to_array())
            .collect();
        base.act(
            &Op::Damage {
                cubes: timber,
                amount: u16::MAX,
            },
            &cx,
        )
        .unwrap();
        assert_eq!(
            curtain(&base),
            walls,
            "the curtain came down with the timber"
        );
        assert_eq!(
            tower(&base),
            0,
            "the tower's top stands with its timber gone"
        );
    }

    #[test]
    fn the_keep_stands_on_its_bond_timbers_and_falls_when_they_are_gone() {
        let mut base = Base::from_shortcuts(&burg(M));
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let (k0, k1) = (-KEEP / 2, -KEEP / 2 + KEEP - 1);
        let (z0, z1) = (-48, -48 + KEEP - 1);
        let stone_over = |base: &Base| {
            (k0..=k1)
                .flat_map(|x| (z0..=z1).map(move |z| (x, z)))
                .flat_map(|(x, z)| {
                    (KEEP_LACING + 1..KEEP_STONE - 1).map(move |y| IVec3::new(x, y, z))
                })
                .filter(|&at| base.cube(at).is_some_and(|c| c.kind == M.stone))
                .count()
        };
        let before = stone_over(&base);
        assert!(before > 1000, "{before}");
        let lacing: Vec<[i32; 3]> = (k0..=k1)
            .flat_map(|x| (z0..=z1).map(move |z| [x, KEEP_LACING, z]))
            .filter(|&at| {
                base.cube(IVec3::from_array(at))
                    .is_some_and(|c| c.kind == M.beams)
            })
            .collect();
        // Burnt through on three sides, it still stands.
        let three: Vec<[i32; 3]> = lacing.iter().copied().filter(|at| at[0] > k0 + 2).collect();
        base.act(
            &Op::Damage {
                cubes: three,
                amount: u16::MAX,
            },
            &cx,
        )
        .unwrap();
        assert_eq!(
            stone_over(&base),
            before,
            "the keep fell with a side of its lacing left"
        );
        base.act(
            &Op::Damage {
                cubes: lacing,
                amount: u16::MAX,
            },
            &cx,
        )
        .unwrap();
        assert_eq!(
            stone_over(&base),
            0,
            "the keep's stone stands with its lacing gone"
        );
    }
}
