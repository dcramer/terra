//! The doors' leaves hung open in every theme's stamps: each swings away from whoever pushes
//! (D512) and lies flat on a wall or stands clear.

use super::tests::{Map, SEEDS, kind};
use super::*;
use crate::theme::assets;
use crate::{ModuleSet, generate};
use terra_gen::Seed;

/// Where an open leaf of a door does not lie flat on a wall, or `None`: the wall solid a
/// hand behind it and its own place clear (rubble may lie at its foot), along its width and
/// up its height. `Some(true, ..)` where its place is not clear, `Some(false, ..)` where it
/// stands clear with no wall behind it.
fn off_the_wall(map: &Map, o: &Object, second: bool, swing: f32) -> Option<(bool, String)> {
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
    let mut free = None;
    for k in 1..10 {
        for up in [0.3, 1.2, 2.2] {
            let p = hinge + along * (reach * k as f32 / 10.0) + Vec3::Y * up;
            let wall = !clear(map, cube_of(world(p + back * 0.1)));
            let room = clear(map, cube_of(world(p - back * 0.03)));
            let fault = || {
                let at = world(p);
                format!(
                    "{} at {} leaf {} swung {swing} opening {opens}: at {at:.2} wall {wall} room {room}",
                    o.name, o.at, second as u8
                )
            };
            if !room {
                return Some((true, fault()));
            } else if !wall && free.is_none() {
                free = Some((false, fault()));
            }
        }
    }
    free
}

/// Every door's leaves swung open as the game hangs them (`door::leaf`, `Object::opens`):
/// into the room when pushed from the corridor, away from the pusher as D512 has it, all but
/// where the room is solid just inside the jamb; there each lies flat on a wall, folded back
/// on the wall it hangs in or along a side wall flush with the jamb, or stands clear a quarter
/// turn in; swung out of the room, each lies along its jamb through the wall.
#[test]
fn open_door_leaves_swing_away_from_the_pusher_and_lie_on_a_wall_or_clear() {
    let (mut clips, mut leaves, mut free, mut out) = (Vec::new(), 0, 0, 0);
    for theme in Theme::all(&assets()).unwrap() {
        let set = ModuleSet::new(&theme).unwrap();
        let seeds: Vec<u64> = (0..SEEDS).collect();
        type Found = (Vec<String>, usize, [usize; 4]);
        let found: Vec<Found> = terra_gen::par::map(&seeds, |&s| {
            let dungeon = generate(&theme, &set, Seed(s)).ok()?;
            let stamp = stamp(&dungeon, &theme, &kind);
            let map = cubes(&stamp.ops);
            let (mut clips, mut free, mut opens) = (Vec::new(), 0, [0; 4]);
            for o in stamp.objects.iter().filter(|o| o.role == ObjectRole::Door) {
                for second in [false, true] {
                    opens[(o.opens[second as usize] + 1.0) as usize] += 1;
                    for swing in [1.0, -1.0] {
                        match off_the_wall(&map, o, second, swing) {
                            Some((true, f)) => clips.push(format!("seed {s}: {f}")),
                            Some((false, _)) => free += 1,
                            None => {}
                        }
                    }
                }
            }
            Some((clips, free, opens))
        })
        .into_iter()
        .flatten()
        .collect();
        let count = |n: usize| found.iter().map(|f| f.2[n]).sum::<usize>();
        let stood = found.iter().map(|f| f.1).sum::<usize>();
        leaves += count(0) + count(2) + count(3);
        (free, out) = (free + stood, out + count(0));
        println!(
            "{}: {} leaves fold flat on their wall, {} turn a quarter in ({stood} with no wall behind), {} swing out",
            theme.name,
            count(3),
            count(2),
            count(0)
        );
        clips.extend(
            found
                .into_iter()
                .flat_map(|f| f.0)
                .map(|f| format!("{}: {f}", theme.name)),
        );
    }
    println!(
        "{out} of {leaves} leaves swing out of the room however pushed, {free} stand clear with no wall behind, {} where something stands",
        clips.len()
    );
    // Turned out of the room wherever a quarter turn in had no wall, D527's first build swung
    // 7% of leaves towards a body pushing from the corridor.
    assert!(
        out * 100 <= leaves,
        "{out} of {leaves} leaves swing towards whoever pushes from the corridor"
    );
    assert!(
        clips.len() * 100 <= leaves,
        "{} of {leaves} leaves where something stands: {:#?}",
        clips.len(),
        &clips[..clips.len().min(12)]
    );
}
