//! How a dungeon's door opens (D512): a body walking into it pushes its leaves open, away
//! from itself, as The Witcher 3's doors open with no button; a locked door only once every
//! key of its lock is held; a one-way door only from its free side. Once open it stays open.
//!
//! In the doorway's frame, metres: x across from the middle of the sill, z out of the room
//! the leaves hang in (into its corridor), so a body at z > 0 stands outside.

use glam::Vec3;

/// How far in front of the leaves a body walking at them pushes them, in metres: a stride
/// and an arm, so they are moving before the body (or a camera at a walk) reaches them.
pub const REACH_M: f32 = 1.5;
/// A body moving at the leaves faster than this pushes; one standing or passing does not.
pub const WALKING_M_S: f32 = 0.3;
/// How fast a pushed leaf swings, in quarter turns a second: open in half a second.
pub const SWING_PER_S: f32 = 2.0;

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
            // Outside, the leaves swing into the room; inside, out of it.
            self.toward = if at.z > 0.0 { 1.0 } else { -1.0 };
        }
        if self.opened() {
            let swing = self.swing + self.toward * SWING_PER_S * dt;
            self.swing = swing.clamp(-1.0, 1.0);
        }
    }

    fn pushed(&self, at: Vec3, velocity: Vec3, width: f32, keys: bool) -> bool {
        let outside = at.z > 0.0;
        let gives = match self.lock {
            Lock::None => true,
            Lock::Keys => keys,
            Lock::Barred { inside } => inside == outside,
        };
        let across = at.x.abs() <= width * 0.5;
        let near = at.z.abs() <= REACH_M;
        // Its speed toward the leaves' plane.
        let at_it = if outside { -velocity.z } else { velocity.z };
        gives && across && near && at_it > WALKING_M_S
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
}
