//! Random operations on a base, which the operations' property test runs and checks step
//! by step (`ops::tests::random_operations_keep_every_rule`), and which the layout's tests
//! lay (`terra_build_layout`'s random bases laid twice): here, not in a test module, as a
//! crate's tests see only the crates below it (D407). A seed gives the same operations on
//! every machine.

use crate::cost::{Amount, Bill, PropKind, Rules, Terms};
use crate::cubes::{Cube, Facing, Fill, Kind, PartId, Shape};
use crate::ground::Sample;
use crate::ops::{Claim, Context, Op, Planned, World};
use crate::{Base, Prop};
use glam::IVec3;
use std::collections::{BTreeMap, BTreeSet};
use terra_gen::{Rng, Seed};

pub const LOGS: Kind = Kind(1);
pub const STONE: Kind = Kind(2);
pub const THATCH: Kind = Kind::FIRST_ROOF;
pub const DOOR: PropKind = PropKind(1);

pub fn rules() -> Rules {
    let mut stone = Bill::of("Stone", Amount::whole(1));
    stone.add(&Bill::of("Mortar", Amount::new(3, 8)));
    Rules {
        kinds: BTreeMap::from([
            // A log fills a cube in two courses, and spans eight cubes.
            (LOGS, Terms::new(2, Bill::of("Log", Amount::new(1, 8)))),
            (STONE, Terms::new(1, stone)),
            // A roof's cube in four stages: frame, battens, covering, trims.
            (THATCH, Terms::new(4, Bill::of("Thatch", Amount::new(1, 4)))),
        ]),
        props: BTreeMap::from([(DOOR, Bill::of("Door", Amount::whole(1)))]),
        // Logs weaker than stone; thatch as any kind.
        toughness: BTreeMap::from([
            (
                LOGS,
                crate::Toughness {
                    health: 60,
                    repair: Amount::new(1, 2),
                },
            ),
            (
                STONE,
                crate::Toughness {
                    health: 150,
                    repair: Amount::new(1, 4),
                },
            ),
        ]),
    }
}

/// The world round a base in a test: what is in stock, who stands where, what rests on
/// what.
#[derive(Default)]
pub struct Yard {
    pub stock: Bill,
    pub bodies: BTreeSet<[i32; 3]>,
    pub items: BTreeSet<[i32; 3]>,
}

impl World for Yard {
    fn in_the_way(&self, at: IVec3) -> bool {
        self.bodies.contains(&at.to_array())
    }
    fn holds_item(&self, at: IVec3) -> bool {
        self.items.contains(&at.to_array())
    }
    fn has(&self, bill: &Bill) -> bool {
        self.stock.covers(bill)
    }
}

pub fn plenty() -> Bill {
    let mut stock = Bill::default();
    for (material, count) in [
        ("Log", 40),
        ("Stone", 40),
        ("Mortar", 20),
        ("Thatch", 20),
        ("Door", 3),
    ] {
        stock.add(&Bill::of(material, Amount::whole(count)));
    }
    stock
}

/// The ground's height under a column for a seed, in centimetres: a slope with bumps.
pub fn ground_at(seed: u64, column: [i32; 2]) -> i16 {
    let bump = Seed(seed).derive(&format!("{column:?}")).rng().below(40) as i16;
    (column[0] - 14) as i16 * -12 + bump - 20
}

/// Where random operations happen: across the edges of sections, east and down.
pub const LOW: IVec3 = IVec3::new(12, -2, 0);
pub const HIGH: IVec3 = IVec3::new(19, 5, 4);

pub fn somewhere(rng: &mut Rng) -> IVec3 {
    let span = HIGH - LOW + IVec3::ONE;
    LOW + IVec3::new(
        rng.below(span.x as u32) as i32,
        rng.below(span.y as u32) as i32,
        rng.below(span.z as u32) as i32,
    )
}

fn pick<T: Copy>(rng: &mut Rng, from: &[T]) -> Option<T> {
    (!from.is_empty()).then(|| from[rng.below(from.len() as u32) as usize])
}

/// Up to so many cubes in a row from one, along a random way, that pass a test.
fn run_from(rng: &mut Rng, start: IVec3, most: i32, keep: impl Fn(IVec3) -> bool) -> Vec<[i32; 3]> {
    let step = [IVec3::X, IVec3::Z, IVec3::Y][rng.below(3) as usize];
    (0..1 + rng.below(most as u32) as i32)
        .map(|i| start + step * i)
        .take_while(|&at| keep(at))
        .map(|at| at.to_array())
        .collect()
}

pub fn random_op(base: &Base, rng: &mut Rng, seed: u64, kind_of: u32) -> Op {
    let cubes: Vec<(IVec3, Cube)> = base.cubes().collect();
    let with = |test: &dyn Fn(&Cube) -> bool| -> Vec<IVec3> {
        cubes
            .iter()
            .filter(|(_, cube)| test(cube))
            .map(|&(at, _)| at)
            .collect()
    };
    let props: Vec<[i32; 3]> = base.props().map(|prop| prop.min).collect();
    let fallback = Op::Unplan {
        cubes: vec![somewhere(rng).to_array()],
    };
    match kind_of {
        0..=2 => {
            let kind = [LOGS, STONE][rng.below(2) as usize];
            let parts: Vec<PartId> = base
                .parts()
                .filter(|(_, part)| part.kind == kind)
                .map(|(id, _)| id)
                .collect();
            let part = match pick(rng, &parts) {
                Some(part) if rng.below(4) == 0 => part,
                _ => base.next_part(),
            };
            let start = somewhere(rng);
            let cubes: Vec<Planned> = run_from(rng, start, 4, |at| base.cube(at).is_none())
                .into_iter()
                .map(|at| Planned {
                    at,
                    shape: if rng.below(6) == 0 {
                        Shape::Half
                    } else {
                        Shape::Whole
                    },
                })
                .collect();
            let ground = cubes
                .iter()
                .map(|cube| {
                    let column = [cube.at[0], cube.at[2]];
                    Sample {
                        column,
                        height_cm: ground_at(seed, column),
                    }
                })
                .collect();
            Op::LayPlan {
                part: Some(part),
                kind,
                cubes,
                ground,
            }
        }
        3..=6 => {
            let planned = with(&|cube| !cube.is_built());
            let Some(start) = pick(rng, &planned) else {
                return fallback;
            };
            let level = base.cube(start).map(|cube| cube.level());
            let cubes = run_from(rng, start, 4, |at| {
                base.cube(at).map(|cube| cube.level()) == level
            });
            Op::Fill { cubes }
        }
        7..=8 => {
            let ins = with(&|cube| cube.is_in());
            let Some(start) = pick(rng, &ins) else {
                return fallback;
            };
            let cubes = run_from(rng, start, 3, |at| base.is_in(at));
            Op::Empty { cubes }
        }
        9 => {
            let bare = with(&|cube| cube.level() == 0);
            let Some(start) = pick(rng, &bare) else {
                return fallback;
            };
            let cubes = run_from(rng, start, 3, |at| {
                base.cube(at).is_some_and(|cube| cube.level() == 0)
            });
            Op::Unplan { cubes }
        }
        10 => {
            let bare = with(&|cube| cube.level() == 0);
            match pick(rng, &bare) {
                Some(at) => Op::SetShape {
                    cubes: vec![at.to_array()],
                    shape: Shape::ALL[rng.below(Shape::ALL.len() as u32) as usize],
                },
                None => fallback,
            }
        }
        11..=14 => {
            let any: Vec<IVec3> = cubes.iter().map(|&(at, _)| at).collect();
            let Some(start) = pick(rng, &any) else {
                return fallback;
            };
            let cubes = run_from(rng, start, 3, |at| base.cube(at).is_some());
            match kind_of {
                11 => Op::Carve { cubes },
                12 => Op::Cancel { cubes },
                13 => Op::Replace {
                    cubes,
                    kind: [LOGS, STONE][rng.below(2) as usize],
                    part: Some(base.next_part()),
                },
                _ => Op::Swap { cubes },
            }
        }
        15 | 16 => {
            // A roof's cubes as the roofer lays them over a small block, on whatever layer.
            let low = somewhere(rng);
            let size = [1 + rng.below(4) as i32, 1 + rng.below(4) as i32];
            let footprint = (0..size[0])
                .flat_map(|x| (0..size[1]).map(move |z| [low.x + x, low.z + z]))
                .collect();
            let style = [
                crate::roofer::Style::Hip,
                crate::roofer::Style::Gable,
                crate::roofer::Style::LeanTo,
            ][rng.below(3) as usize];
            let roof = crate::roofer::over(&footprint, low.y, style, None);
            Op::LayPlan {
                part: Some(base.next_part()),
                kind: THATCH,
                cubes: roof
                    .roof
                    .iter()
                    .filter(|(at, _)| base.cube(*at).is_none())
                    .map(|&(at, shape)| Planned {
                        at: at.to_array(),
                        shape,
                    })
                    .collect(),
                ground: Vec::new(),
            }
        }
        17 => {
            // Slopes climbing a cube a cube from one already there, by hand.
            let Some(&from) = pick(rng, &with(&|cube| cube.kind == THATCH)).as_ref() else {
                return fallback;
            };
            let facing = Facing::ALL[rng.below(4) as usize];
            let cubes = (1..=1 + rng.below(3) as i32)
                .map(|k| from + (facing.step() + IVec3::Y) * k)
                .take_while(|&at| base.cube(at).is_none())
                .map(|at| Planned {
                    at: at.to_array(),
                    shape: Shape::Slope(facing),
                })
                .collect();
            let part = base.cube(from).and_then(|cube| cube.part);
            Op::LayPlan {
                part,
                kind: THATCH,
                cubes,
                ground: Vec::new(),
            }
        }
        18 => {
            // A door into a hole of built cubes, where there is one.
            let mut holes = Vec::new();
            for x in LOW.x..=HIGH.x {
                for y in LOW.y..=HIGH.y {
                    for z in LOW.z..=HIGH.z {
                        for high in [y, y + 1] {
                            let prop = Prop {
                                kind: DOOR,
                                min: [x, y, z],
                                max: [x, high, z],
                                open: false,
                                ..Prop::default()
                            };
                            let free = prop.cubes().all(|at| base.cube(at).is_none());
                            if free && base.hole_for(&prop).is_some() {
                                holes.push(prop);
                            }
                        }
                    }
                }
            }
            match pick(rng, &holes) {
                Some(prop) => Op::FitProp(prop),
                None => Op::FitProp(Prop {
                    kind: DOOR,
                    min: somewhere(rng).to_array(),
                    max: somewhere(rng).to_array(),
                    open: false,
                    ..Prop::default()
                }),
            }
        }
        23 => {
            // A blow, as an attack's (D193): a run of cubes, whatever they are.
            let ins = with(&|cube| cube.is_in());
            let Some(start) = pick(rng, &ins) else {
                return fallback;
            };
            let cubes = run_from(rng, start, 3, |at| base.cube(at).is_some());
            Op::Damage {
                cubes,
                amount: 10 + rng.below(140) as u16,
            }
        }
        24 => {
            let damaged: Vec<IVec3> = base.damaged().map(|(at, _)| at).collect();
            match pick(rng, &damaged) {
                Some(at) => Op::Repair {
                    cubes: vec![at.to_array()],
                },
                None => fallback,
            }
        }
        25 => {
            // A blow that destroys whatever it hits.
            let ins = with(&|cube| cube.is_in());
            match pick(rng, &ins) {
                Some(at) => Op::Damage {
                    cubes: vec![at.to_array()],
                    amount: u16::MAX,
                },
                None => fallback,
            }
        }
        26 => {
            // The ground under a few columns gives way, or rises.
            let columns: Vec<[i32; 2]> = base.occupied_columns().map(|c| [c.x, c.y]).collect();
            let mut samples: BTreeMap<[i32; 2], i16> = BTreeMap::new();
            for _ in 0..1 + rng.below(4) {
                if let Some(column) = pick(rng, &columns) {
                    // From a metre and a half down to a little up.
                    let by = 20 - rng.below(170) as i16;
                    samples.insert(column, base.ground().height_cm(column) + by);
                }
            }
            if samples.is_empty() {
                return fallback;
            }
            Op::Subside {
                samples: samples
                    .into_iter()
                    .map(|(column, height_cm)| Sample { column, height_cm })
                    .collect(),
            }
        }
        19 => match pick(rng, &props) {
            Some(at) => Op::RemoveProp { at },
            None => fallback,
        },
        20 => match pick(rng, &props) {
            Some(at) => Op::SetOpen {
                at,
                open: rng.below(2) == 0,
            },
            None => fallback,
        },
        21 => {
            let bare = with(&|cube| cube.level() == 0 && cube.fill == Fill::Solid);
            let Some(start) = pick(rng, &bare) else {
                return fallback;
            };
            let cubes = run_from(rng, start, 3, |at| {
                base.cube(at)
                    .is_some_and(|cube| cube.level() == 0 && cube.fill == Fill::Solid)
            });
            Op::Open { cubes }
        }
        _ => {
            let open = with(&|cube| cube.fill == Fill::Open);
            let Some(start) = pick(rng, &open) else {
                return fallback;
            };
            let cubes = run_from(rng, start, 3, |at| {
                base.cube(at).is_some_and(|cube| cube.fill == Fill::Open)
            });
            Op::Close { cubes }
        }
    }
}

/// How many seeds of random operations a test run tries, unless `TERRA_SEEDS` says: a few
/// by default, enough that every operation is accepted; more for a sweep, as
/// `TERRA_SEEDS=64 cargo test -p terra_build random_operations`.
pub fn seeds() -> u64 {
    const SEEDS: u64 = 4;
    std::env::var("TERRA_SEEDS")
        .ok()
        .and_then(|seeds| seeds.parse().ok())
        .unwrap_or(SEEDS)
}

/// The claim random operations work in.
pub fn claim() -> Claim {
    Claim {
        min: [LOW.x, LOW.z],
        max: [HIGH.x, HIGH.z],
        budget: 120,
    }
}

/// A seed's stream of random operations.
pub fn rng(seed: u64) -> Rng {
    Seed(seed).derive("building").rng()
}

/// What random operations begin from: a stone wall a storey high with a doorway in it and a
/// roof's cubes planned along its top and climbing from it, so doors are fitted and roofs
/// go up and are held.
pub fn start(seed: u64) -> Vec<Op> {
    let wall: Vec<[i32; 3]> = (13..18)
        .flat_map(|x| (-1..6).map(move |y| [x, y, 2]))
        .filter(|&[x, y, _]| !(x == 15 && (0..2).contains(&y)))
        .collect();
    let mut start = vec![Op::LayPlan {
        part: Some(PartId(0)),
        kind: STONE,
        cubes: wall
            .iter()
            .map(|&at| Planned {
                at,
                shape: Shape::Whole,
            })
            .collect(),
        ground: (13..18)
            .map(|x| Sample {
                column: [x, 2],
                height_cm: ground_at(seed, [x, 2]),
            })
            .collect(),
    }];
    for y in -1..6 {
        let cubes = wall.iter().copied().filter(|at| at[1] == y).collect();
        start.push(Op::Fill { cubes });
    }
    start.push(Op::LayPlan {
        part: Some(PartId(1)),
        kind: THATCH,
        cubes: (13..18)
            .flat_map(|x| [[x, 6, 2], [x, 7, 3]])
            .map(|at| Planned {
                at,
                shape: Shape::Slope(Facing::South),
            })
            .collect(),
        ground: Vec::new(),
    });
    start
}

/// The bases a seed's random operations build, one every so many steps: the bases the
/// property test checks at those steps, as this draws the same numbers in the same order
/// (an undo at once and its redo, which the test checks, leave the base as it was, so here
/// only the draw).
pub fn bases(seed: u64, steps: usize, every: usize) -> Vec<Base> {
    let rules = rules();
    let claim = claim();
    let mut rng = rng(seed);
    let mut yard = Yard {
        stock: plenty(),
        ..Yard::default()
    };
    let mut base = Base::default();
    let begun = base
        .act_all(&start(seed), &Context::free(&rules))
        .expect("the start is built");
    yard.stock.take(&begun.took).expect("the start is in stock");
    let mut undos: Vec<Vec<Op>> = Vec::new();
    let mut bases = Vec::new();
    for step in 0..steps {
        if step % 50 == 0 {
            yard.bodies = (0..3).map(|_| somewhere(&mut rng).to_array()).collect();
            yard.items = (0..3).map(|_| somewhere(&mut rng).to_array()).collect();
        }
        let roll = rng.below(31);
        let undo = roll >= 27;
        let ops = if undo {
            match undos.pop() {
                Some(ops) => ops,
                None => continue,
            }
        } else {
            vec![random_op(&base, &mut rng, seed, roll)]
        };
        let cx = Context {
            rules: &rules,
            claim: Some(&claim),
            world: &yard,
        };
        if let Ok(done) = base.act_all(&ops, &cx) {
            yard.stock.take(&done.took);
            yard.stock.add(&done.gave);
            rng.below(3);
            if !undo {
                undos.push(done.undo);
            }
        }
        if step % every == every - 1 {
            bases.push(base.clone());
        }
    }
    bases
}
