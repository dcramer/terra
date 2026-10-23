//! Where a settlement's buildings stand (the village's docs, "How it is laid out"): the
//! village's street, one of each sort of its buildings for the checks, and the towns and
//! hamlets of every building the rules make, to measure drawing at scale (D185).

use super::*;

/// One of each sort of the village's buildings (`village`, seed 7), as the checks look at
/// them (`cargo xtask artifacts`, `cargo xtask rules`): each put up whole in its own cubes, on
/// ground falling in steps of 20 cm, its footings laid in the palette's first masonry.
pub fn each_sort(palette: &Palette) -> Vec<(Building, Base)> {
    sorts(palette)
        .into_iter()
        .map(|building| {
            let base = standing(&building, palette);
            (building, base)
        })
        .collect()
}

/// One of each sort of the village's buildings (seed 7), not yet put up.
pub fn sorts(palette: &Palette) -> Vec<Building> {
    let mut seen = BTreeSet::new();
    village(7, palette)
        .into_iter()
        .filter(|building| seen.insert(format!("{:?} {:?}", building.plan, building.scheme)))
        .collect()
}

/// A building put up whole in its own cubes, as [`each_sort`] puts each up.
pub fn standing(building: &Building, palette: &Palette) -> Base {
    let mut base = Base::default();
    for op in &building.ops {
        base.apply(*op);
    }
    base.lay_footings(palette.first().masonry, |column| {
        -((column[0] + column[1]).rem_euclid(3) as i16) * 20
    });
    base
}

/// The ways a door looks: north, south and east.
const NORTH: f32 = std::f32::consts::PI;
const SOUTH: f32 = 0.0;
const EAST: f32 = std::f32::consts::FRAC_PI_2;

/// The village's houses: each one's plan, scheme and roof, how wide its front, where its
/// door is east of the green's middle and how far from the street's middle, and which side
/// it is on.
const HOUSES: [(Plan, Scheme, Roof, i32, [f32; 2], f32); 12] = [
    (
        Plan::Rectangle,
        Scheme::Timber,
        Roof::Gable,
        12,
        [-26.0, -5.5],
        SOUTH,
    ),
    (
        Plan::Rectangle,
        Scheme::Masonry,
        Roof::Hip,
        14,
        [-13.0, -5.5],
        SOUTH,
    ),
    (
        Plan::TwoStoreys,
        Scheme::MasonryUnderBeams,
        Roof::Gable,
        12,
        [-1.0, -9.5],
        SOUTH,
    ),
    (
        Plan::Rectangle,
        Scheme::TimberOnPlinth,
        Roof::Gable,
        14,
        [13.0, -5.5],
        SOUTH,
    ),
    (Plan::L, Scheme::Beams, Roof::Gable, 14, [27.0, -9.0], SOUTH),
    (
        Plan::Rectangle,
        Scheme::MasonryEnds,
        Roof::Gable,
        14,
        [-25.0, 5.5],
        NORTH,
    ),
    (Plan::T, Scheme::Timber, Roof::Hip, 14, [-9.0, 9.5], NORTH),
    (
        Plan::U,
        Scheme::Masonry,
        Roof::Gable,
        18,
        [8.0, 10.0],
        NORTH,
    ),
    (
        Plan::Rectangle,
        Scheme::Beams,
        Roof::Hip,
        12,
        [24.0, 5.5],
        NORTH,
    ),
    // At the street's ends, the cob and the wattle and daub (D236).
    (
        Plan::Rectangle,
        Scheme::Cob,
        Roof::Gable,
        12,
        [-40.0, 5.5],
        NORTH,
    ),
    (
        Plan::Rectangle,
        Scheme::Framed,
        Roof::Gable,
        12,
        [-42.0, -5.5],
        SOUTH,
    ),
    (
        Plan::TwoStoreys,
        Scheme::MasonryUnderFramed,
        Roof::Gable,
        12,
        [40.0, 9.5],
        NORTH,
    ),
];

/// The village's buildings from its seed: the same seed, the same village.
pub fn village(seed: u32, picks: impl Picks + Copy) -> Vec<Building> {
    let kinds = picks.pick(seed);
    let mut buildings = Vec::new();
    for (index, &(plan, scheme, roofed, wide, at, facing)) in HOUSES.iter().enumerate() {
        let seed = hash::of(&[seed, index as u32]);
        let kinds = picks.pick(seed);
        // A little out of line, as a village is: a lean of a few degrees and a step either
        // way along the street.
        let lean = (hash::unit(hash::of(&[seed, 1])) - 0.5) * 0.1;
        let shift = (hash::unit(hash::of(&[seed, 2])) - 0.5) * 1.2;
        let ops = match plan {
            Plan::TwoStoreys => two_storeys(seed, scheme, roofed, kinds),
            _ => one_storey(seed, plan, scheme, roofed, wide, kinds),
        };
        buildings.push(Building {
            name: name(plan, scheme, roofed),
            plan,
            scheme,
            at: [at[0] + shift, at[1]],
            facing: facing + lean,
            ops,
        });
    }
    // A woodshed beside the log cabin, its open front to the street.
    buildings.push(Building {
        name: name(Plan::Shed, Scheme::Timber, Roof::LeanTo),
        plan: Plan::Shed,
        scheme: Scheme::Timber,
        at: [-34.5, -6.5],
        facing: SOUTH + 0.15,
        ops: shed(hash::of(&[seed, 20]), kinds),
    });
    // The lookout at the east end, its stair down toward the street.
    buildings.push(Building {
        name: name(Plan::Deck, Scheme::Beams, Roof::Gable),
        plan: Plan::Deck,
        scheme: Scheme::Beams,
        at: [38.0, 2.0],
        facing: -std::f32::consts::FRAC_PI_2,
        ops: deck(kinds),
    });
    // At the street's ends, two built by hand a cube at a time (D226): a log cabin with a
    // dormer, and a stone house with a storey of timber over it.
    buildings.push(Building {
        name: "log cabin with a dormer, built by hand".to_string(),
        plan: Plan::Rectangle,
        scheme: Scheme::Timber,
        at: [-56.0, -5.5],
        facing: SOUTH,
        ops: dormer_house(kinds),
    });
    buildings.push(Building {
        name: "masonry house with a timber storey, built by hand".to_string(),
        plan: Plan::Rectangle,
        scheme: Scheme::MasonryUnderBeams,
        at: [54.0, -5.5],
        facing: SOUTH,
        ops: storey_over_roof(kinds),
    });
    // Gardens walled in stone behind two houses of the southern row, a gap in each back
    // wall for a gate; each length its own, standing on the ground where it is.
    let long = |metres: f32| (metres / terra_build::CELL_M).round() as i32;
    let mut n = 0;
    for (west, east, front, back) in [(-30.5, -19.5, 11.5, 17.5), (19.5, 29.5, 11.5, 16.5)] {
        let gate = (west + east) * 0.5;
        let lengths = [
            // The back wall, either side of the gate.
            (
                (west + gate - 0.5) * 0.5,
                back,
                NORTH,
                long(gate - 0.5 - west),
            ),
            (
                (gate + 0.5 + east) * 0.5,
                back,
                NORTH,
                long(east - gate - 0.5),
            ),
            // The sides, from the house's back to a cube short of the back wall.
            (
                west,
                (front + back - 0.5) * 0.5,
                -EAST,
                long(back - 0.5 - front),
            ),
            (
                east,
                (front + back - 0.5) * 0.5,
                EAST,
                long(back - 0.5 - front),
            ),
        ];
        for (at_east, at_south, facing, long) in lengths {
            n += 1;
            buildings.push(Building {
                name: format!("masonry garden wall {n}"),
                plan: Plan::Wall,
                scheme: Scheme::Masonry,
                at: [at_east, at_south],
                facing,
                ops: low_wall(long, kinds),
            });
        }
    }
    buildings
}

/// Every building the rules make, as its plan, scheme, roof and front's width (D182): a
/// rectangle under each roof, an L, a T or a U under a gable or a hip (a U only wide enough
/// for a yard between its wings), two storeys in each scheme that has them, a shed and a
/// deck. What a town to measure picks from ([`town`]).
pub fn every_building() -> Vec<(Plan, Scheme, Roof, i32)> {
    use Scheme::*;
    const SCHEMES: [Scheme; 9] = [
        Timber,
        Masonry,
        Beams,
        TimberOnPlinth,
        MasonryUnderBeams,
        MasonryEnds,
        Cob,
        Framed,
        MasonryUnderFramed,
    ];
    let mut all = Vec::new();
    for scheme in SCHEMES {
        for wide in WIDTHS {
            for roofed in [Roof::Gable, Roof::Hip, Roof::LeanTo] {
                all.push((Plan::Rectangle, scheme, roofed, wide));
            }
            for plan in [Plan::L, Plan::T, Plan::U] {
                if plan == Plan::U && wide < 14 {
                    continue;
                }
                for roofed in [Roof::Gable, Roof::Hip] {
                    all.push((plan, scheme, roofed, wide));
                }
            }
        }
        if !matches!(scheme, TimberOnPlinth | MasonryEnds) {
            for roofed in [Roof::Gable, Roof::Hip] {
                all.push((Plan::TwoStoreys, scheme, roofed, 12));
            }
        }
    }
    all.push((Plan::Shed, Timber, Roof::LeanTo, 6));
    all.push((Plan::Deck, Beams, Roof::Gable, 6));
    all
}

/// A building of [`every_building`]'s, from its seed, its door at a place looking a way.
pub fn put_up(
    seed: u32,
    (plan, scheme, roofed, wide): (Plan, Scheme, Roof, i32),
    at: [f32; 2],
    facing: f32,
    kinds: Kinds,
) -> Building {
    let ops = match plan {
        Plan::TwoStoreys => two_storeys(seed, scheme, roofed, kinds),
        Plan::Shed => shed(seed, kinds),
        Plan::Deck => deck(kinds),
        Plan::Wall => low_wall(wide, kinds),
        _ => one_storey(seed, plan, scheme, roofed, wide, kinds),
    };
    Building {
        name: name(plan, scheme, roofed),
        plan,
        scheme,
        at,
        facing,
        ops,
    }
}

/// How far a building's door stands back from its street's middle: further where its
/// wings come forward, or its stair beside it.
fn set_back(plan: Plan) -> f32 {
    match plan {
        Plan::L | Plan::U | Plan::TwoStoreys => 9.5,
        _ => 5.5,
    }
}

/// [`every_building`] in an order its seed shuffles, so a town of any size mixes them.
fn shuffled(seed: u32) -> Vec<(Plan, Scheme, Roof, i32)> {
    let mut all = every_building();
    for i in (1..all.len()).rev() {
        let j = hash::of(&[seed, 100, i as u32]) as usize % (i + 1);
        all.swap(i, j);
    }
    all
}

/// A town's plots: this far apart along a street, so many a side of each, and its streets
/// this far apart, so two rows of houses stand back to back between them with a gap.
const PLOT_M: f32 = 14.0;
const PLOTS_A_SIDE: usize = 20;
const STREETS_APART_M: f32 = 36.0;

/// A town of `count` buildings to measure building's drawing at scale (D185's `town300`):
/// streets running east and west, houses along both sides of each with their doors to it,
/// every plan, scheme, roof and width the rules make in an order the seed shuffles, so
/// three hundred are nearly every one of them once. Places in metres east and south of
/// the town's middle, as [`village`] gives them.
pub fn town(seed: u32, count: usize, picks: impl Picks + Copy) -> Vec<Building> {
    let choices = shuffled(seed);
    let per_street = PLOTS_A_SIDE * 2;
    let streets = count.div_ceil(per_street).max(1);
    (0..count)
        .map(|index| {
            let choice = choices[index % choices.len()];
            let (street, along) = (index / per_street, index % per_street);
            let (plot, north) = (along / 2, along % 2 == 0);
            let seed = hash::of(&[seed, index as u32]);
            // A little out of line, as a village's are.
            let lean = (hash::unit(hash::of(&[seed, 1])) - 0.5) * 0.1;
            let shift = (hash::unit(hash::of(&[seed, 2])) - 0.5) * 1.2;
            let east = (plot as f32 - (PLOTS_A_SIDE as f32 - 1.0) * 0.5) * PLOT_M + shift;
            let middle = (street as f32 - (streets as f32 - 1.0) * 0.5) * STREETS_APART_M;
            let back = set_back(choice.0);
            let (south, facing) = if north {
                (middle - back, 0.0)
            } else {
                (middle + back, std::f32::consts::PI)
            };
            put_up(seed, choice, [east, south], facing + lean, picks.pick(seed))
        })
        .collect()
}

/// Hamlets of houses spread over the land round a middle, out to `radius_m` and no nearer
/// than a tenth of it, to measure bases seen far off (D185's `far`): `count` buildings, five
/// to a hamlet along a lane of its own, each hamlet turned its own way.
pub fn hamlets(seed: u32, count: usize, radius_m: f32, picks: impl Picks + Copy) -> Vec<Building> {
    const PER_HAMLET: usize = 5;
    let choices = shuffled(seed ^ 0x5eed);
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        let hamlet = (index / PER_HAMLET) as u32;
        let (plot, north) = ((index % PER_HAMLET) / 2, index % 2 == 0);
        // Spread evenly over the area between a tenth of the radius and all of it.
        let near = radius_m * 0.1;
        let share = hash::unit(hash::of(&[seed, hamlet, 1]));
        let distance = libm::sqrtf(near * near + share * (radius_m * radius_m - near * near));
        let angle = hash::unit(hash::of(&[seed, hamlet, 2])) * std::f32::consts::TAU;
        let turn = hash::unit(hash::of(&[seed, hamlet, 3])) * std::f32::consts::TAU;
        let middle = [distance * libm::cosf(angle), distance * libm::sinf(angle)];
        let choice = choices[index % choices.len()];
        let back = set_back(choice.0);
        // Along its lane, then turned with the hamlet about its middle.
        let east = (plot as f32 - 1.0) * PLOT_M;
        let (south, facing) = if north {
            (-back, 0.0)
        } else {
            (back, std::f32::consts::PI)
        };
        let (s, c) = (libm::sinf(turn), libm::cosf(turn));
        let at = [
            middle[0] + east * c + south * s,
            middle[1] - east * s + south * c,
        ];
        let seed_of = hash::of(&[seed, index as u32]);
        out.push(put_up(
            seed_of,
            choice,
            at,
            facing + turn,
            picks.pick(seed_of),
        ));
    }
    out
}
