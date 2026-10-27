//! How a dungeon's door opens (D512): a body walking into it pushes its leaves open, away
//! from itself, as The Witcher 3's doors open with no button; a locked door only once every
//! key of its lock is held; a one-way door only from its free side. Once open it stays open.
//!
//! In the doorway's frame, metres: x across from the middle of the sill, z out of the room
//! the leaves hang in (into its corridor), so a body at z > 0 stands outside.

use glam::{Vec2, Vec3};
use std::f32::consts::{FRAC_PI_2, PI};

/// How far in front of the leaves a body walking at them pushes them, in metres: a stride
/// and an arm, so they are moving before the body (or a camera at a walk) reaches them.
pub const REACH_M: f32 = 1.5;
/// A body moving at the leaves faster than this pushes; one standing or passing does not.
pub const WALKING_M_S: f32 = 0.3;
/// How fast a pushed leaf swings, in quarter turns a second: open in half a second.
pub const SWING_PER_S: f32 = 2.0;
/// How far ahead a filming camera reads its shot, in seconds: a swing and a tenth, so a door
/// its shot walks into stands open a tenth of a second before the camera is in reach.
pub const FILM_LEAD_S: f32 = 1.0 / SWING_PER_S + 0.1;
/// How near a filming camera pushes, in metres along its shot from the doorway: a 20 mm
/// lens on Super 35 sees 64 degrees across, so the shut 2 m leaves fill it from 1.6 m, and a
/// shot that slows to hold 3 m short while they swing (as a game's door cinematic holds) has
/// them open as it stands there, never shut in the picture.
pub const FILM_REACH_M: f32 = 3.5;
/// A filming camera reads its shot ahead until its eye has gone so far past where it is
/// `FILM_LEAD_S` from now, in metres (`film_path`): by distance, not time, so a shot that
/// crawls for seconds short of a door still reads the doorway (barrow round 18's tour took
/// 10 s over its last 3.5 m, and a 5 s read saw the sill only 2.6 m out). Twice the reach,
/// so a shot climbing as steeply as 60 degrees still reads `FILM_REACH_M` in plan.
pub const FILM_READ_M: f32 = 2.0 * FILM_REACH_M;
/// The longest a filming camera reads its shot ahead, in seconds, for a shot that stands
/// still: longer than a game's door cinematic holds.
pub const FILM_HORIZON_S: f32 = 20.0;

/// The shot a filming camera reads ahead for the doors it walks into: its eye `eye(t)`, `t`
/// seconds from now, every `every` seconds, until it has gone `FILM_READ_M` from where it is
/// `FILM_LEAD_S` from now, or for `FILM_HORIZON_S`. What `Door::film` takes, in the world's
/// frame (each doorway's is the world's turned and moved, so distances are the same).
pub fn film_path(mut eye: impl FnMut(f32) -> Vec3, every: f32) -> Vec<Vec3> {
    let lead = (FILM_LEAD_S / every).round() as usize;
    let reads = (FILM_HORIZON_S / every).round() as usize;
    let mut path: Vec<Vec3> = Vec::with_capacity(lead + 1);
    let mut gone = 0.0;
    for n in 0..=reads {
        let at = eye(n as f32 * every);
        if n > lead
            && let Some(&before) = path.last()
        {
            gone += at.distance(before);
        }
        path.push(at);
        if gone > FILM_READ_M {
            break;
        }
    }
    path
}

/// Where one of a door's two leaves hangs with the door `swing` open, in the doorway's
/// frame: its hinge, and its turn about up for a leaf reaching +X from its hinge. Hung as a
/// gate of its `width` is, each at one jamb reaching to the middle when shut, the second
/// turned half round so it reaches back; and `depth` behind the sill, at the room's face of
/// the wall, as a door in a thick wall is hung so it swings back to the wall. Into the room
/// it turns `opens` quarter turns: two folds it flat on the wall it is hung in, one lays it
/// on a side wall flush with its jamb; minus one turns it out of the room however it is
/// pushed. Out of the room a quarter turn, along its jamb.
pub fn leaf(second: bool, swing: f32, opens: f32, width: f32, depth: f32) -> (Vec3, f32) {
    let turn = if swing > 0.0 { swing * opens } else { swing } * FRAC_PI_2;
    let side = if second { 1.0 } else { -1.0 };
    let hinge = Vec3::new(side * width * 0.5, 0.0, -depth);
    (hinge, if second { PI - turn } else { turn })
}

/// What a door asks before it gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lock {
    /// Pushed open from either side.
    None,
    /// Opens when every key of its lock is held.
    Keys,
    /// Barred on one side (`inside`: the room's): pushed only from the other.
    Barred { inside: bool },
}

/// A door's two leaves, how far they stand open: 0 shut, 1 swung a quarter turn into the
/// room, -1 a quarter turn out of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Door {
    pub lock: Lock,
    pub swing: f32,
    /// Which way it is swinging once pushed, until fully open.
    toward: f32,
}

impl Door {
    pub fn shut(lock: Lock) -> Door {
        Door {
            lock,
            swing: 0.0,
            toward: 0.0,
        }
    }

    /// Whether it stands open (pushed, swinging or swung).
    pub fn opened(&self) -> bool {
        self.toward != 0.0
    }

    /// One step of `dt` seconds with the bodies near it, each where it is and how it moves
    /// (the doorway's frame), the doorway `width` across; `keys` whether the bodies hold
    /// every key of its lock. A body inside the reach in front of the leaves, across the
    /// doorway, walking at them, pushes them away from itself; the first to push decides
    /// the way, and the leaves swing at their own pace however many bodies there are.
    pub fn step(&mut self, bodies: &[(Vec3, Vec3)], width: f32, keys: bool, dt: f32) {
        if !self.opened()
            && let Some(&(at, _)) = bodies
                .iter()
                .find(|&&(at, velocity)| self.pushed(at, velocity, width, keys))
        {
            self.push(at.z > 0.0);
        }
        self.turn(dt);
    }

    /// One step of `dt` seconds with a filming camera near it, which stands in for the body
    /// walking its shot: `path` is the shot's eye in the doorway's frame from now, every
    /// `every` seconds, as far as `film_path` reads it. The camera pushes as that body would a swing
    /// ahead: once the shot, from where its eye is `FILM_LEAD_S` from now, goes through the
    /// doorway within `FILM_REACH_M` along its path (in plan). By distance along the shot,
    /// not time, so one that slows or holds short of a door has it open as it comes, and
    /// one that turns in from the side measures the way it will walk.
    pub fn film(&mut self, path: &[Vec3], every: f32, width: f32, keys: bool, dt: f32) {
        if !self.opened()
            && let Some(&now) = path.first()
            && let Some(along) = Door::along_to_doorway(path, every, width)
            && along <= FILM_REACH_M
            && self.gives(now.z > 0.0, keys)
        {
            self.push(now.z > 0.0);
        }
        self.turn(dt);
    }

    /// How far in plan the shot `path` walks from its eye `FILM_LEAD_S` ahead to where it
    /// first goes through the doorway `width` across, if it does.
    fn along_to_doorway(path: &[Vec3], every: f32, width: f32) -> Option<f32> {
        let lead = ((FILM_LEAD_S / every).round() as usize).min(path.len() - 1);
        let mut along = 0.0;
        for (i, pair) in path.windows(2).enumerate() {
            let (a, b) = (pair[0], pair[1]);
            // Before the lead the body is already there: what it walks counts from the lead.
            let step = if i < lead {
                0.0
            } else {
                Vec2::new(b.x - a.x, b.z - a.z).length()
            };
            if a.z * b.z <= 0.0 && a.z != b.z {
                let f = a.z / (a.z - b.z);
                if (a.x + (b.x - a.x) * f).abs() <= width * 0.5 {
                    return Some(along + step * f);
                }
            }
            along += step;
        }
        None
    }

    /// Outside, the leaves swing into the room; inside, out of it.
    fn push(&mut self, outside: bool) {
        self.toward = if outside { 1.0 } else { -1.0 };
    }

    fn turn(&mut self, dt: f32) {
        if self.opened() {
            let swing = self.swing + self.toward * SWING_PER_S * dt;
            self.swing = swing.clamp(-1.0, 1.0);
        }
    }

    fn gives(&self, outside: bool, keys: bool) -> bool {
        match self.lock {
            Lock::None => true,
            Lock::Keys => keys,
            Lock::Barred { inside } => inside == outside,
        }
    }

    fn pushed(&self, at: Vec3, velocity: Vec3, width: f32, keys: bool) -> bool {
        let outside = at.z > 0.0;
        let across = at.x.abs() <= width * 0.5;
        let near = at.z.abs() <= REACH_M;
        // Its speed toward the leaves' plane.
        let at_it = if outside { -velocity.z } else { velocity.z };
        self.gives(outside, keys) && across && near && at_it > WALKING_M_S
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: f32 = 2.0;
    const DT: f32 = 1.0 / 60.0;

    /// Walks a body from `from` at `velocity` for `seconds`, keys held or not, stopping it
    /// at the sill; the door as it is then.
    fn walk(mut door: Door, from: Vec3, velocity: Vec3, keys: bool, seconds: f32) -> Door {
        let mut at = from;
        for _ in 0..(seconds / DT) as usize {
            door.step(&[(at, velocity)], WIDTH, keys, DT);
            let next = at + velocity * DT;
            at = if next.z * at.z <= 0.0 { at } else { next };
        }
        door
    }

    fn walking_in(from_z: f32) -> (Vec3, Vec3) {
        let v = Vec3::new(0.0, 0.0, -from_z.signum() * 1.5);
        (Vec3::new(0.3, 0.0, from_z), v)
    }

    #[test]
    fn a_door_walked_into_from_outside_swings_into_the_room_and_stays_open() {
        let (at, v) = walking_in(2.5);
        let door = walk(Door::shut(Lock::None), at, v, false, 0.5);
        assert_eq!(door.swing, 0.0, "shut while out of reach");
        let door = walk(door, at, v, false, 3.0);
        assert_eq!(door.swing, 1.0, "open into the room");
        // Walked away from, it stays open.
        let door = walk(door, Vec3::new(0.0, 0.0, 1.0), Vec3::Z * 1.5, false, 3.0);
        assert_eq!(door.swing, 1.0);
    }

    #[test]
    fn a_door_walked_into_from_inside_swings_out_of_the_room() {
        let (at, v) = walking_in(-2.0);
        let door = walk(Door::shut(Lock::None), at, v, false, 3.0);
        assert_eq!(door.swing, -1.0);
    }

    #[test]
    fn it_opens_in_about_half_a_second_from_the_push() {
        let (at, v) = walking_in(REACH_M - 0.01);
        let door = walk(Door::shut(Lock::None), at, v, false, 0.25);
        assert!(door.swing > 0.4 && door.swing < 0.6, "{}", door.swing);
        let door = walk(door, at, v, false, 0.3);
        assert_eq!(door.swing, 1.0);
    }

    #[test]
    fn it_swings_at_its_own_pace_however_many_bodies_are_near() {
        // A camera walking at it and a player standing by: open in half a second still.
        let (at, v) = walking_in(REACH_M - 0.01);
        let mut door = Door::shut(Lock::None);
        let stood = (Vec3::new(0.0, 0.0, 3.0), Vec3::ZERO);
        for _ in 0..15 {
            door.step(&[(at, v), stood, stood], WIDTH, false, DT);
        }
        assert!(door.swing > 0.4 && door.swing < 0.6, "{}", door.swing);
    }

    #[test]
    fn a_body_standing_passing_or_walking_away_does_not_open_it() {
        let shut = Door::shut(Lock::None);
        let stood = walk(shut, Vec3::new(0.0, 0.0, 0.8), Vec3::ZERO, false, 3.0);
        assert!(!stood.opened(), "standing in front");
        let past = walk(shut, Vec3::new(-3.0, 0.0, 0.8), Vec3::X * 1.5, false, 4.0);
        assert!(!past.opened(), "walking along the wall past it");
        let away = walk(shut, Vec3::new(0.0, 0.0, 0.5), Vec3::Z * 1.5, false, 3.0);
        assert!(!away.opened(), "walking away");
        let beside = walk(shut, Vec3::new(1.6, 0.0, 1.4), -Vec3::Z * 1.5, false, 3.0);
        assert!(!beside.opened(), "walking at the wall beside it");
    }

    #[test]
    fn a_locked_door_opens_only_with_its_keys() {
        let (at, v) = walking_in(2.0);
        let door = walk(Door::shut(Lock::Keys), at, v, false, 5.0);
        assert!(!door.opened(), "without its keys");
        let door = walk(door, at, v, true, 3.0);
        assert_eq!(door.swing, 1.0, "with them");
    }

    #[test]
    fn a_one_way_door_opens_only_from_its_free_side_then_stays_open_for_both() {
        let barred = Door::shut(Lock::Barred { inside: true });
        let (inside, v_in) = walking_in(-2.0);
        let door = walk(barred, inside, v_in, true, 5.0);
        assert!(!door.opened(), "from the barred side, keys or not");
        let (outside, v_out) = walking_in(2.0);
        let door = walk(door, outside, v_out, false, 3.0);
        assert_eq!(door.swing, 1.0, "from the free side");
        let door = walk(door, inside, v_in, false, 3.0);
        assert_eq!(door.swing, 1.0, "open for both");
    }

    /// How often a filming camera's shot is read ahead.
    const EVERY: f32 = 0.05;

    /// A shot's name and its eye at each time.
    type Shot = (&'static str, Box<dyn Fn(f32) -> Vec3>);

    /// The plan distance from a point to the doorway's opening.
    fn to_doorway(p: Vec3) -> f32 {
        Vec2::new((p.x.abs() - WIDTH * 0.5).max(0.0), p.z).length()
    }

    /// Films a shot whose eye is `eye(t)` for `seconds`: the door when it is done, and how
    /// far from the doorway the eye was when it first stood fully open.
    fn film(eye: impl Fn(f32) -> Vec3, seconds: f32) -> (Door, Option<f32>) {
        let mut door = Door::shut(Lock::None);
        let mut open_at = None;
        for k in 0..(seconds / DT) as usize {
            let t = k as f32 * DT;
            let path = film_path(|ahead| eye(t + ahead), EVERY);
            door.film(&path, EVERY, WIDTH, false, DT);
            if door.swing.abs() >= 1.0 && open_at.is_none() {
                open_at = Some(to_doorway(eye(t)));
            }
        }
        (door, open_at)
    }

    /// Eased from `a` to `b` between `t0` and `t1` seconds, standing at either end outside.
    fn dolly(a: Vec3, b: Vec3, t0: f32, t1: f32) -> impl Fn(f32) -> Vec3 {
        move |t| {
            let s = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
            a.lerp(b, s * s * (3.0 - 2.0 * s))
        }
    }

    #[test]
    fn a_shot_that_slows_or_turns_in_front_of_a_door_has_it_open_before_the_lens_fills() {
        // A 20 mm lens is filled by the shut leaves from 1.6 m: open before the eye is in.
        let eye = Vec3::new(0.2, 1.6, 0.0);
        let shots: [Shot; 3] = [
            (
                "a walk at 2.5 m/s",
                Box::new(move |t| eye + Vec3::Z * (8.0 - 2.5 * t)),
            ),
            (
                "slowing to stand 0.5 m out for a second, then on",
                Box::new(move |t| {
                    if t < 3.0 {
                        dolly(eye + Vec3::Z * 6.0, eye + Vec3::Z * 0.5, 0.0, 3.0)(t)
                    } else {
                        dolly(eye + Vec3::Z * 0.5, eye - Vec3::Z * 2.0, 4.0, 6.0)(t)
                    }
                }),
            ),
            (
                // As the mine's tour at 17 s: along the wall 1.5 m out, slowing to turn in.
                "sliding past 1.5 m out, then turning in at a crawl",
                Box::new(move |t| {
                    if t < 3.5 {
                        Vec3::new(-4.0 + 1.2 * t, 1.6, 1.5)
                    } else {
                        Vec3::new(0.2, 1.6, 1.5 - 0.8 * (t - 3.5))
                    }
                }),
            ),
        ];
        for (shot, eye) in shots {
            let (door, open_at) = film(eye, 8.0);
            assert_eq!(door.swing, 1.0, "{shot}: into the room");
            let open_at = open_at.unwrap_or(0.0);
            assert!(open_at > 1.6, "{shot}: open only {open_at:.2} m out");
        }
    }

    #[test]
    fn a_shot_holding_3_m_short_of_a_door_sees_it_open_while_it_holds() {
        // As a tour slows to hold 3 m out while the leaves swing, then walks through.
        let eye = Vec3::new(0.2, 1.6, 0.0);
        let hold = eye + Vec3::Z * 3.0;
        for (shot, hold_s) in [("held a second", 1.0), ("held two seconds", 2.0)] {
            let path = move |t: f32| {
                if t < 3.0 {
                    dolly(eye + Vec3::Z * 10.0, hold, 0.0, 3.0)(t)
                } else {
                    dolly(hold, eye - Vec3::Z * 2.0, 3.0 + hold_s, 6.0 + hold_s)(t)
                }
            };
            let mut door = Door::shut(Lock::None);
            let mut open_by = None;
            for k in 0..(9.0 / DT) as usize {
                let t = k as f32 * DT;
                let ahead = film_path(|s| path(t + s), EVERY);
                door.film(&ahead, EVERY, WIDTH, false, DT);
                if door.swing >= 1.0 && open_by.is_none() {
                    open_by = Some(t);
                }
            }
            let open_by = open_by.unwrap_or(f32::INFINITY);
            assert!(
                open_by <= 3.0 + hold_s * 0.5,
                "{shot}: fully open only at {open_by:.2} s, the hold ends at {:.1} s",
                3.0 + hold_s
            );
        }
    }

    #[test]
    fn the_crypt_tours_crawl_to_its_first_door_sees_it_open_3_m_short() {
        // The eye's distance to the sill each second as barrow round 19's probe logged it in
        // the crypt's tour film (frames 1120 to 1480, then through the sill at 1741): it
        // crawls from 3.6 to 2.5 m for four seconds and takes ten over its last 3.5 m.
        let keys = [
            (0.0, 7.0),
            (1.0, 4.52),
            (2.0, 3.57),
            (3.0, 3.25),
            (4.0, 2.99),
            (5.0, 2.74),
            (6.0, 2.50),
            (10.35, 0.0),
            (11.5, -2.0),
        ];
        let out = move |t: f32| {
            let t = t.clamp(0.0, 11.5);
            let i = keys.windows(2).position(|k| t <= k[1].0).unwrap_or(0);
            let ((t0, z0), (t1, z1)) = (keys[i], keys[i + 1]);
            z0 + (z1 - z0) * (t - t0) / (t1 - t0)
        };
        let (_, open_at) = film(move |t| Vec3::new(0.0, 1.6, out(t)), 11.5);
        let open_at = open_at.unwrap_or(0.0);
        assert!(open_at >= 3.0, "fully open only {open_at:.2} m out");
    }

    #[test]
    fn a_shot_passing_a_door_or_leaving_it_does_not_open_it() {
        let shots: [Shot; 3] = [
            (
                "along the wall 1 m out",
                Box::new(|t| Vec3::new(-4.0 + 1.5 * t, 1.6, 1.0)),
            ),
            (
                "walking away from it",
                Box::new(|t| Vec3::new(0.2, 1.6, 0.5 + 1.5 * t)),
            ),
            (
                "up to it, stopping short",
                Box::new(dolly(
                    Vec3::new(0.2, 1.6, 4.0),
                    Vec3::new(0.2, 1.6, 0.6),
                    0.0,
                    2.0,
                )),
            ),
        ];
        for (shot, eye) in shots {
            let (door, _) = film(eye, 6.0);
            assert!(!door.opened(), "{shot}");
        }
    }
}
