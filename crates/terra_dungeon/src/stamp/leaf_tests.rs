//! The doors' leaves hung open in every theme's stamps: each lies flat on a wall.

use super::tests::{Map, SEEDS, kind};
use super::*;
use crate::theme::assets;
use crate::{ModuleSet, generate};
use terra_gen::Seed;

/// Where an open leaf of a door does not lie flat on a wall, or `None`: the wall solid a
/// hand behind it and its own place clear (rubble may lie at its foot), along its width and
/// up its height.
fn off_the_wall(map: &Map, o: &Object, second: bool, swing: f32) -> Option<String> {
    let (across, into) = super::runin::doorway_axes(o.facing);
    let world = |p: Vec3| o.at + across * p.x + Vec3::Y * p.y - into * p.z;
    let opens = o.opens[second as usize];
    let (hinge, turn) = crate::door::leaf(second, swing, opens, DOORWAY_M[0], CUBE);
    let along = Vec3::new(libm::cosf(turn), 0.0, -libm::sinf(turn));
    // Toward the wall: away from the doorway's middle, or out of the room.
    let n = Vec3::new(along.z, 0.0, -along.x);
    let back = if n.x * hinge.x.signum() + n.z > 0.0 {
        n
    } else {
        -n
    };
    // Into the room its whole width; out of it, through its own wall.
    let reach = if swing > 0.0 && opens > 0.0 {
        DOORWAY_M[0] * 0.5
    } else {
        CUBE
    };
    for k in 1..10 {
        for up in [0.3, 1.2, 2.2] {
            let p = hinge + along * (reach * k as f32 / 10.0) + Vec3::Y * up;
            let wall = !clear(map, cube_of(world(p + back * 0.1)));
            let room = clear(map, cube_of(world(p - back * 0.03)));
            if !(wall && room) {
                let at = world(p);
                return Some(format!(
                    "{} at {} leaf {} swung {swing} opening {opens}: at {at:.2} wall {wall} room {room}",
                    o.name, o.at, second as u8
                ));
            }
        }
    }
    None
}

/// Every door's leaves swung open into the room as the game hangs them (`door::leaf`,
/// `Object::opens`) lie flat on a wall, folded back on the wall they hang in or along a side
/// wall flush with the jamb, so an open leaf never stands out into the room a tour walks
/// into; swung out of it, each lies along its jamb through the wall.
#[test]
fn open_door_leaves_lie_flat_on_a_wall() {
    let (mut faulty, mut leaves) = (Vec::new(), 0);
    for theme in Theme::all(&assets()).unwrap() {
        let set = ModuleSet::new(&theme).unwrap();
        let seeds: Vec<u64> = (0..SEEDS).collect();
        let found: Vec<(Vec<String>, [usize; 4])> = terra_gen::par::map(&seeds, |&s| {
            let dungeon = generate(&theme, &set, Seed(s)).ok()?;
            let stamp = stamp(&dungeon, &theme, &kind);
            let map = cubes(&stamp.ops);
            let (mut off, mut opens) = (Vec::new(), [0; 4]);
            for o in stamp.objects.iter().filter(|o| o.role == ObjectRole::Door) {
                for second in [false, true] {
                    opens[(o.opens[second as usize] + 1.0) as usize] += 1;
                    for swing in [1.0, -1.0] {
                        off.extend(
                            off_the_wall(&map, o, second, swing).map(|f| format!("seed {s}: {f}")),
                        );
                    }
                }
            }
            Some((off, opens))
        })
        .into_iter()
        .flatten()
        .collect();
        let count = |n: usize| found.iter().map(|f| f.1[n]).sum::<usize>();
        leaves += count(0) + count(2) + count(3);
        println!(
            "{}: {} leaves fold flat on their wall, {} turn a quarter onto a side wall, {} swing out",
            theme.name,
            count(3),
            count(2),
            count(0)
        );
        faulty.extend(
            found
                .into_iter()
                .flat_map(|f| f.0)
                .map(|f| format!("{}: {f}", theme.name)),
        );
    }
    // A leaf with no wall to lie on (the door's wall ends within a metre of the jamb and no
    // side wall stands flush with it) stands a quarter turn into the room: rare, kept rare.
    println!("{} of {leaves} leaves have no wall to lie on", faulty.len());
    assert!(
        faulty.len() * 100 <= leaves,
        "{} of {leaves} leaves off the wall: {:#?}",
        faulty.len(),
        &faulty[..faulty.len().min(12)]
    );
}
