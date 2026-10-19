//! A castle to measure (D185's `castle`): one base of about a hundred and fifty thousand
//! cubes, as the biggest a tribe will build, so building's drawing is measured on a base
//! that size and not only on many small ones. A curtain wall of stone three metres thick
//! and twelve high round a ward eighty metres square, crenellated, with a gate through its
//! south side between two towers; a tower of six storeys at each corner under a hip roof;
//! a keep of five storeys in the middle, its floors of timber, under a gable; and a hall of
//! two storeys along the north wall. Laid out in its own cubes with the gate's middle at
//! the origin on the wall's outer face, looking south (+Z), as a village's buildings are.

use crate::roofer::{self, Style};
use crate::{Base, BuildOp, Cube, Fill, Kind, STOREY_CUBES};
use std::collections::BTreeSet;

/// The ward's side and the curtain's thickness and height, in cubes.
const SIDE: i32 = 160;
const THICK: i32 = 6;
const HIGH: i32 = 24;
/// A corner tower's side, its walls' thickness, and its storeys.
const TOWER: i32 = 16;
const TOWER_WALL: i32 = 4;
const TOWER_STOREYS: i32 = 6;

/// The castle's shortcuts: stone walls, timber floors, and roofs of the roof kind given.
pub fn castle(stone: Kind, timber: Kind, roof: Kind) -> Vec<BuildOp> {
    let mut ops = Vec::new();
    let solid = |kind: Kind| Some(Cube::new(kind, Fill::Solid));
    let fill = |ops: &mut Vec<BuildOp>, min: [i32; 3], max: [i32; 3], cube: Option<Cube>| {
        ops.push(BuildOp::Fill { min, max, cube });
    };
    let (x0, x1) = (-SIDE / 2, SIDE / 2 - 1);
    let (z0, z1) = (-SIDE, -1);
    // The curtain, each side a box, then merlons on its outer edge, two cubes in every four.
    for (min, max) in [
        ([x0, 0, z1 - THICK + 1], [x1, HIGH - 1, z1]),
        ([x0, 0, z0], [x1, HIGH - 1, z0 + THICK - 1]),
        ([x0, 0, z0], [x0 + THICK - 1, HIGH - 1, z1]),
        ([x1 - THICK + 1, 0, z0], [x1, HIGH - 1, z1]),
    ] {
        fill(&mut ops, min, max, solid(stone));
    }
    for x in (x0..=x1).step_by(4) {
        for z in [z0, z1] {
            fill(&mut ops, [x, HIGH, z], [x + 1, HIGH + 1, z], solid(stone));
        }
    }
    for z in (z0..=z1).step_by(4) {
        for x in [x0, x1] {
            fill(&mut ops, [x, HIGH, z], [x, HIGH + 1, z + 1], solid(stone));
        }
    }
    // The gate through the south side, three metres wide and five high.
    fill(&mut ops, [-3, 0, z1 - THICK + 1], [2, 9, z1], None);
    // A tower at each corner and either side of the gate, hollow, a timber floor each
    // storey, under a hip roof.
    let corners = [
        [x0 - 4, z0 - 4],
        [x1 - TOWER + 5, z0 - 4],
        [x0 - 4, z1 - TOWER + 5],
        [x1 - TOWER + 5, z1 - TOWER + 5],
        [-3 - TOWER, z1 - TOWER + 3],
        [3, z1 - TOWER + 3],
    ];
    let top = TOWER_STOREYS * STOREY_CUBES;
    let mut roofs: Vec<(BTreeSet<[i32; 2]>, i32, Style)> = Vec::new();
    for [tx, tz] in corners {
        let (ax, az, bx, bz) = (tx, tz, tx + TOWER - 1, tz + TOWER - 1);
        fill(&mut ops, [ax, 0, az], [bx, top - 1, bz], solid(stone));
        fill(
            &mut ops,
            [ax + TOWER_WALL, 0, az + TOWER_WALL],
            [bx - TOWER_WALL, top - 1, bz - TOWER_WALL],
            None,
        );
        for storey in 1..TOWER_STOREYS {
            let y = storey * STOREY_CUBES - 1;
            fill(
                &mut ops,
                [ax + TOWER_WALL, y, az + TOWER_WALL],
                [bx - TOWER_WALL, y, bz - TOWER_WALL],
                solid(timber),
            );
        }
        // Arrow slits up each face, a cube wide and two high, every other storey.
        for storey in (1..TOWER_STOREYS).step_by(2) {
            let y = storey * STOREY_CUBES + 2;
            let mid = TOWER / 2;
            for (min, max) in [
                ([ax + mid, y, az], [ax + mid, y + 1, az + TOWER_WALL - 1]),
                ([ax + mid, y, bz - TOWER_WALL + 1], [ax + mid, y + 1, bz]),
                ([ax, y, az + mid], [ax + TOWER_WALL - 1, y + 1, az + mid]),
                ([bx - TOWER_WALL + 1, y, az + mid], [bx, y + 1, az + mid]),
            ] {
                fill(&mut ops, min, max, None);
            }
        }
        let footprint = (ax..=bx)
            .flat_map(|x| (az..=bz).map(move |z| [x, z]))
            .collect();
        roofs.push((footprint, TOWER_STOREYS - 1, Style::Hip));
    }
    // The keep: five storeys, walls a metre and a half thick, timber floors, windows a cube
    // wide and two high in each storey's walls every four metres.
    let (kx0, kx1, kz0, kz1) = (-20, 19, -100, -71);
    let keep_top = 5 * STOREY_CUBES;
    fill(
        &mut ops,
        [kx0, 0, kz0],
        [kx1, keep_top - 1, kz1],
        solid(stone),
    );
    fill(
        &mut ops,
        [kx0 + 3, 0, kz0 + 3],
        [kx1 - 3, keep_top - 1, kz1 - 3],
        None,
    );
    for storey in 1..5 {
        let y = storey * STOREY_CUBES - 1;
        fill(
            &mut ops,
            [kx0 + 3, y, kz0 + 3],
            [kx1 - 3, y, kz1 - 3],
            solid(timber),
        );
    }
    for storey in 0..5 {
        let y = storey * STOREY_CUBES + 2;
        for x in (kx0 + 4..kx1 - 3).step_by(8) {
            for (za, zb) in [(kz0, kz0 + 2), (kz1 - 2, kz1)] {
                fill(&mut ops, [x, y, za], [x, y + 1, zb], None);
            }
        }
        for z in (kz0 + 4..kz1 - 3).step_by(8) {
            for (xa, xb) in [(kx0, kx0 + 2), (kx1 - 2, kx1)] {
                fill(&mut ops, [xa, y, z], [xb, y + 1, z], None);
            }
        }
    }
    // Its door, on the ground facing the gate.
    fill(&mut ops, [-1, 0, kz1 - 2], [0, 3, kz1], None);
    let keep: BTreeSet<[i32; 2]> = (kx0..=kx1)
        .flat_map(|x| (kz0..=kz1).map(move |z| [x, z]))
        .collect();
    roofs.push((keep, 4, Style::Gable));
    // The hall along the north wall: two storeys of stone a metre thick, under a gable.
    let (hx0, hx1, hz0, hz1) = (-40, 39, z0 + THICK, z0 + THICK + 19);
    let hall_top = 2 * STOREY_CUBES;
    fill(
        &mut ops,
        [hx0, 0, hz0],
        [hx1, hall_top - 1, hz1],
        solid(stone),
    );
    fill(
        &mut ops,
        [hx0 + 2, 0, hz0 + 2],
        [hx1 - 2, hall_top - 1, hz1 - 2],
        None,
    );
    fill(
        &mut ops,
        [hx0 + 2, STOREY_CUBES - 1, hz0 + 2],
        [hx1 - 2, STOREY_CUBES - 1, hz1 - 2],
        solid(timber),
    );
    for x in (hx0 + 4..hx1 - 3).step_by(6) {
        for y in [2, STOREY_CUBES + 2] {
            fill(&mut ops, [x, y, hz1 - 1], [x + 1, y + 2, hz1], None);
        }
    }
    fill(&mut ops, [-1, 0, hz1 - 1], [0, 3, hz1], None);
    let hall: BTreeSet<[i32; 2]> = (hx0..=hx1)
        .flat_map(|x| (hz0..=hz1).map(move |z| [x, z]))
        .collect();
    roofs.push((hall, 1, Style::Gable));
    // The roofs, each over what stands by then.
    let mut walls = Base::from_shortcuts(&ops);
    for (footprint, storey, style) in roofs {
        let height = roofer::field(&footprint, style, None);
        let roofed = roofer::put_up(&walls, &footprint, storey, roof, &height);
        walls.apply_shortcuts(&roofed);
        ops.extend(roofed);
    }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_castle_is_a_base_of_over_a_hundred_thousand_cubes_that_holds() {
        let ops = castle(Kind(2), Kind(3), Kind(0x801));
        let base = Base::from_shortcuts(&ops);
        let cubes = base.cubes().count();
        assert!((100_000..=200_000).contains(&cubes), "{cubes} cubes");
        let roofed = base.cubes().filter(|(_, c)| c.kind.is_roof()).count();
        assert!(roofed > 1000, "{roofed} roof cubes");
        assert!(base.floating().is_empty(), "{:?}", &base.floating()[..5]);
    }
}
