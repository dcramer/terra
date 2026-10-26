//! How a dungeon's door opens (D512): a body walking into it pushes its leaves open, away
//! from itself, as The Witcher 3's doors open with no button; a locked door only once every
//! key of its lock is held; a one-way door only from its free side. Once open it stays open.
//!
//! In the doorway's frame, metres: x across from the middle of the sill, z out of the room
//! the leaves hang in (into its corridor), so a body at z > 0 stands outside.

use glam::{Vec2, Vec3};

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
/// How near a filming camera pushes, in metres from the doorway: a 20 mm lens on Super 35
/// sees 64 degrees across, so the shut 2 m leaves fill it from 1.6 m; open from 2 m out, the
/// leaves are out of the picture before then, from in front or the side.
pub const FILM_REACH_M: f32 = 2.0;
/// How far ahead a filming camera reads its shot for whether it goes through a door: long
/// enough for a shot that slows and turns in front of one.
pub const FILM_HORIZON_S: f32 = 5.0;

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
    /// `every` seconds over `FILM_HORIZON_S`. The camera pushes as that body would a swing
    /// ahead: once its eye within `FILM_LEAD_S` from now is within `FILM_REACH_M` of the
    /// doorway, from in front or the side, if the shot goes through it. A shot read ahead,
    /// not the camera's velocity, so one that slows to turn in front of a door has it open
    /// as it comes.
    pub fn film(&mut self, path: &[Vec3], every: f32, width: f32, keys: bool, dt: f32) {
        if !self.opened()
            && let Some(&now) = path.first()
        {
            let half = width * 0.5;
            let lead = ((FILM_LEAD_S / every).round() as usize).min(path.len() - 1);
            let near = path[..=lead]
                .iter()
                .any(|p| Vec2::new((p.x.abs() - half).max(0.0), p.z).length() <= FILM_REACH_M);
            let through = path.windows(2).any(|pair| {
                let (a, b) = (pair[0], pair[1]);
                a.z * b.z <= 0.0 && a.z != b.z && {
                    let x = a.x + (b.x - a.x) * a.z / (a.z - b.z);
                    x.abs() <= half
                }
            });
            if near && through && self.gives(now.z > 0.0, keys) {
                self.push(now.z > 0.0);
            }
        }
        self.turn(dt);
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
            let path: Vec<Vec3> = (0..=(FILM_HORIZON_S / EVERY) as usize)
                .map(|n| eye(t + n as f32 * EVERY))
                .collect();
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
