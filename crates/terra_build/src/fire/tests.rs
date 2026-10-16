//! Fire, case by case, each answer known from how fire behaves rather than from the code's
//! numbers: what does not burn stays, what burns burns through and what it held comes down,
//! flame climbs faster than it creeps, the wind carries it, a smothered cube smoulders, and
//! the same start burns the same way.

use super::*;
use crate::cost::{Rules, Toughness};
use crate::ops::{Context, Op};
use crate::{BuildOp, Cube, Fill};

const STONE: Kind = Kind(2);
const LOGS: Kind = Kind(1);
const HEALTH: u16 = 400;

fn burns(kind: Kind) -> Option<Burns> {
    (kind == LOGS).then_some(Burns {
        catches: 2.0,
        fuel_s: 20.0,
        heat: 1.0,
    })
}

fn health(_: Kind) -> u16 {
    HEALTH
}

fn rules() -> Rules {
    let mut rules = Rules::default();
    for kind in [STONE, LOGS] {
        rules.toughness.insert(
            kind,
            Toughness {
                health: HEALTH,
                repair: crate::cost::Amount::new(1, 2),
            },
        );
    }
    rules
}

fn solid(kind: Kind, min: [i32; 3], max: [i32; 3]) -> BuildOp {
    BuildOp::Fill {
        min,
        max,
        cube: Some(Cube::new(kind, Fill::Solid)),
    }
}

/// Steps the fire `ticks` times, sending its harm to the base as the game does; every cube
/// that came down, and the tick each first caught on.
fn burn(
    base: &mut Base,
    fire: &mut Fire,
    ticks: usize,
    wind: Vec3,
) -> (Vec<IVec3>, BTreeMap<[i32; 3], usize>) {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut fell = Vec::new();
    let mut caught = BTreeMap::new();
    for tick in 0..ticks {
        let stepped = fire.step(base, &burns, &health, wind);
        for at in stepped.caught {
            caught.entry(at).or_insert(tick);
        }
        for (amount, cubes) in stepped.harm {
            let cubes: Vec<[i32; 3]> = cubes
                .into_iter()
                .filter(|&at| base.cube(IVec3::from_array(at)).is_some())
                .collect();
            if cubes.is_empty() {
                continue;
            }
            let done = base.act(&Op::Damage { cubes, amount }, &cx).unwrap();
            fire.fell(base, &burns, &done.fell);
            fell.extend(done.fell.iter().map(|(at, _)| *at));
        }
    }
    (fell, caught)
}

#[test]
fn stone_does_not_burn_but_is_sooted_where_flame_licks_it() {
    let mut base = Base::from_shortcuts(&[solid(STONE, [0, 0, 0], [3, 3, 0])]);
    let mut fire = Fire::default();
    let at = IVec3::new(1, 1, 0);
    fire.light(&base, &burns, at);
    for _ in 0..20 {
        fire.heat(&base, &burns, at, 5.0);
    }
    let (fell, _) = burn(&mut base, &mut fire, 400, Vec3::ZERO);
    assert!(fell.is_empty(), "stone came down: {fell:?}");
    assert_eq!(base.cubes().count(), 16);
    assert!(fire.is_out());
    assert!(
        fire.scorch()[&at.to_array()] > 0,
        "no soot where the flame was"
    );
}

#[test]
fn a_lit_post_burns_through_and_the_stone_it_held_comes_down() {
    // A stone lintel on two log posts, the ground at their feet.
    let mut base = Base::from_shortcuts(&[
        solid(LOGS, [0, 0, 0], [0, 3, 0]),
        solid(LOGS, [4, 0, 0], [4, 3, 0]),
        solid(STONE, [0, 4, 0], [4, 4, 0]),
    ]);
    let mut fire = Fire::default();
    fire.light(&base, &burns, IVec3::new(0, 0, 0));
    fire.light(&base, &burns, IVec3::new(4, 0, 0));
    let (fell, _) = burn(&mut base, &mut fire, 2000, Vec3::ZERO);
    for x in 0..=4 {
        assert!(
            base.cube(IVec3::new(x, 4, 0)).is_none(),
            "the lintel's stone at {x} still stands"
        );
        assert!(
            fell.contains(&IVec3::new(x, 4, 0)),
            "the lintel's stone at {x} did not come down"
        );
    }
    assert!(
        fire.is_out(),
        "a fire burns out once nothing is left to burn"
    );
}

#[test]
fn flame_climbs_a_post_faster_than_it_creeps_along_a_beam() {
    // A post and a beam the same length, apart, each lit at one end.
    let mut base = Base::from_shortcuts(&[
        solid(LOGS, [0, 0, 0], [0, 7, 0]),
        solid(LOGS, [4, 0, 0], [11, 0, 0]),
    ]);
    let mut fire = Fire::default();
    fire.light(&base, &burns, IVec3::new(0, 0, 0));
    fire.light(&base, &burns, IVec3::new(4, 0, 0));
    let (_, caught) = burn(&mut base, &mut fire, 600, Vec3::ZERO);
    let up = caught.get(&[0, 6, 0]).copied().unwrap_or(usize::MAX);
    let along = caught.get(&[10, 0, 0]).copied().unwrap_or(usize::MAX);
    assert!(along < usize::MAX, "the fire never crept along the beam");
    assert!(
        up < along,
        "six cubes up caught on tick {up}, six along on {along}"
    );
}

#[test]
fn the_wind_carries_a_fire_downwind_faster_than_up_it() {
    let mut base = Base::from_shortcuts(&[solid(LOGS, [-8, 0, 0], [8, 0, 0])]);
    let mut fire = Fire::default();
    fire.light(&base, &burns, IVec3::new(0, 0, 0));
    let (_, caught) = burn(&mut base, &mut fire, 600, Vec3::new(6.0, 0.0, 0.0));
    let downwind = caught.get(&[5, 0, 0]).copied().unwrap_or(usize::MAX);
    let upwind = caught.get(&[-5, 0, 0]).copied().unwrap_or(usize::MAX);
    assert!(
        downwind < upwind,
        "downwind caught on tick {downwind}, upwind on {upwind}"
    );
}

#[test]
fn a_cube_with_no_air_smoulders_longer_than_one_in_the_open() {
    // A block of logs, its middle cube shut in all round, and a lone log apart.
    let mut base = Base::from_shortcuts(&[
        solid(LOGS, [0, 0, 0], [2, 2, 2]),
        solid(LOGS, [8, 0, 0], [8, 0, 0]),
    ]);
    let mut fire = Fire::default();
    fire.light(&base, &burns, IVec3::new(1, 1, 1));
    fire.light(&base, &burns, IVec3::new(8, 0, 0));
    let mut gone = BTreeMap::new();
    for tick in 0..2000 {
        burn(&mut base, &mut fire, 1, Vec3::ZERO);
        for at in [[1, 1, 1], [8, 0, 0]] {
            if base.cube(IVec3::from_array(at)).is_none() {
                gone.entry(at).or_insert(tick);
            }
        }
    }
    // The block round it burns too, and so opens it to the air; it still outlasts the lone log.
    assert!(gone[&[1, 1, 1]] > gone[&[8, 0, 0]], "{gone:?}");
}

#[test]
fn a_burning_cube_shows_its_harm_as_it_goes_and_chars() {
    let mut base = Base::from_shortcuts(&[solid(LOGS, [0, 0, 0], [0, 0, 0])]);
    let mut fire = Fire::default();
    let at = IVec3::ZERO;
    fire.light(&base, &burns, at);
    let mut last = 0;
    let mut chars = Vec::new();
    for _ in 0..40 {
        burn(&mut base, &mut fire, 1, Vec3::ZERO);
        let now = base.damage(at);
        assert!(now >= last, "its damage went back from {last} to {now}");
        last = now;
        chars.push(fire.scorch().get(&at.to_array()).copied().unwrap_or(0));
    }
    assert!(
        last > 0 && last < HEALTH,
        "half burnt, its damage is {last}"
    );
    assert!(
        chars.windows(2).all(|w| w[1] >= w[0]) && chars.last() > chars.first(),
        "{chars:?}"
    );
}

#[test]
fn the_same_fire_burns_the_same_way_every_time() {
    let start = || {
        let base = Base::from_shortcuts(&[
            solid(LOGS, [0, 0, 0], [6, 4, 3]),
            solid(STONE, [0, 5, 0], [6, 5, 3]),
        ]);
        let mut fire = Fire::default();
        fire.light(&base, &burns, IVec3::new(3, 0, 1));
        (base, fire)
    };
    let (mut a, mut fa) = start();
    let (mut b, mut fb) = start();
    let ra = burn(&mut a, &mut fa, 300, Vec3::new(1.0, 0.0, 2.0));
    let rb = burn(&mut b, &mut fb, 300, Vec3::new(1.0, 0.0, 2.0));
    assert_eq!(ra, rb);
    assert_eq!(fa, fb);
    assert!(!ra.1.is_empty(), "nothing caught");
}

#[test]
fn a_burning_beam_that_falls_sets_fire_to_the_floor_it_lands_on() {
    // A log floor on the ground, and ten cubes over it a log beam carried out from a stone
    // post, all of it lit; the floor is far beyond any flame's reach.
    let mut base = Base::from_shortcuts(&[
        solid(LOGS, [0, 0, 0], [2, 0, 0]),
        solid(STONE, [5, 0, 0], [5, 9, 0]),
        solid(LOGS, [1, 10, 0], [5, 10, 0]),
    ]);
    let mut fire = Fire::default();
    for x in 1..=5 {
        fire.light(&base, &burns, IVec3::new(x, 10, 0));
    }
    let (fell, caught) = burn(&mut base, &mut fire, 400, Vec3::ZERO);
    assert!(
        fell.contains(&IVec3::new(1, 10, 0)),
        "the beam's end never came down"
    );
    assert!(
        caught.contains_key(&[1, 0, 0]),
        "the floor it fell on never caught: {caught:?}"
    );
}

#[test]
fn a_fire_in_a_shut_room_reaches_its_far_wall_sooner_than_one_under_the_open_sky() {
    // Two log rooms alike, a fire lit on each floor by one wall; one has a ceiling, the
    // other is open to the sky, so its hot gas rises away.
    let room = |roofed: bool| {
        let mut ops = vec![
            solid(LOGS, [0, 0, 0], [8, 7, 8]),
            BuildOp::Fill {
                min: [1, 1, 1],
                max: [7, 7, 7],
                cube: None,
            },
        ];
        if roofed {
            ops.push(solid(LOGS, [0, 8, 0], [8, 8, 8]));
        }
        let mut base = Base::from_shortcuts(&ops);
        let mut fire = Fire::default();
        for z in 3..=5 {
            fire.light(&base, &burns, IVec3::new(1, 0, z));
        }
        let (_, caught) = burn(&mut base, &mut fire, 400, Vec3::ZERO);
        caught.get(&[8, 4, 4]).copied().unwrap_or(usize::MAX)
    };
    let (shut, open) = (room(true), room(false));
    assert!(
        shut < open,
        "the far wall caught on tick {shut} shut in, {open} in the open"
    );
}

#[test]
fn timber_under_stone_gives_way_before_it_has_burnt_through() {
    // Two log posts lit alike, apart: one carrying a stone on it, one bare.
    let mut base = Base::from_shortcuts(&[
        solid(LOGS, [0, 0, 0], [0, 0, 0]),
        solid(STONE, [0, 1, 0], [0, 1, 0]),
        solid(LOGS, [6, 0, 0], [6, 0, 0]),
    ]);
    let mut fire = Fire::default();
    fire.light(&base, &burns, IVec3::new(0, 0, 0));
    fire.light(&base, &burns, IVec3::new(6, 0, 0));
    let mut gone = BTreeMap::new();
    for tick in 0..2000 {
        burn(&mut base, &mut fire, 1, Vec3::ZERO);
        for at in [[0, 0, 0], [6, 0, 0]] {
            if base.cube(IVec3::from_array(at)).is_none() {
                gone.entry(at).or_insert(tick);
            }
        }
    }
    assert!(gone[&[0, 0, 0]] < gone[&[6, 0, 0]], "{gone:?}");
}
