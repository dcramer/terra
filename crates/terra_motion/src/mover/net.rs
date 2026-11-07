//! A body on the wire (docs/netcode.md; D651, D652, D636): what the mover's state and intent
//! look like to `terra_net`, defined here by the crate that owns them (D198).
//!
//! - [`Shown`]: a body as everyone near is told it, quantised (a place to 2 mm, 19 bytes).
//! - [`Own`]: a body as its own player is told it, exact, to check its prediction against.
//! - [`Intent`]'s wire form. It is quantised too, so both ends must run the intent as it
//!   reads back ([`Intent::wired`]), never as it was before it was written.
//!
//! A client predicts its body with this same mover (D30 makes both ends compute alike) and,
//! when the server's [`Own`] for a tick differs ([`Body::agrees`]), sets the body to it
//! ([`Body::correct`]) and runs its later intents again (`terra_net::predict`).

use super::{Bodies, Body, Ground, Intent, World, settle};
use glam::{DVec3, Vec3};
use terra_net::wire::{Malformed, Reader, Wire, Writer};

/// A place's step on the wire, a metre: 2 mm.
const PLACE_STEPS: f64 = 512.0;
/// The bits of a place's axis, signed: 65 km either way, more than a realm's radius (D13).
const PLACE_BITS: u32 = 26;
/// The fastest a body is told to be moving, m/s, and its bits: a step of 16 mm/s.
const MOST_SPEED: f32 = 64.0;
const SPEED_BITS: u32 = 13;
/// The bits of a direction's axis.
const WAY_BITS: u32 = 10;

const GROUNDS: [Ground; 5] = [
    Ground::Grounded,
    Ground::Steep,
    Ground::Airborne,
    Ground::Swimming,
    Ground::Climbing,
];

impl Wire for Ground {
    fn write(&self, to: &mut Writer) {
        let index = GROUNDS
            .iter()
            .position(|ground| ground == self)
            .unwrap_or(0);
        to.ranged(index as u32, GROUNDS.len() as u32 - 1);
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        Ok(GROUNDS[from.ranged(GROUNDS.len() as u32 - 1)? as usize])
    }
}

fn write_way(way: Vec3, to: &mut Writer) {
    for along in way.to_array() {
        to.quantised(along, -1.0, 1.0, WAY_BITS);
    }
}

/// A direction read back to unit length, or `otherwise` where it is none.
fn read_way(from: &mut Reader, otherwise: Vec3) -> Result<Vec3, Malformed> {
    let mut way = Vec3::ZERO;
    for axis in 0..3 {
        way[axis] = from.quantised(-1.0, 1.0, WAY_BITS)?;
    }
    Ok(way.try_normalize().unwrap_or(otherwise))
}

fn write_exact(vector: Vec3, to: &mut Writer) {
    vector.to_array().write(to);
}

fn read_exact(from: &mut Reader) -> Result<Vec3, Malformed> {
    Ok(Vec3::from_array(<[f32; 3]>::read(from)?))
}

/// A body as everyone near it is told: enough to draw it between two of these.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shown {
    pub feet: DVec3,
    pub velocity: Vec3,
    pub facing: Vec3,
    pub ground: Ground,
}

impl Wire for Shown {
    fn write(&self, to: &mut Writer) {
        let most = (1i64 << (PLACE_BITS - 1)) - 1;
        for along in self.feet.to_array() {
            let steps = ((along * PLACE_STEPS).round() as i64).clamp(-most, most);
            to.bits(steps as u64, PLACE_BITS);
        }
        for along in self.velocity.to_array() {
            to.quantised(along, -MOST_SPEED, MOST_SPEED, SPEED_BITS);
        }
        write_way(self.facing, to);
        self.ground.write(to);
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        let mut feet = DVec3::ZERO;
        for axis in 0..3 {
            // The sign is the field's top bit: carried up through the 64.
            let steps = (from.bits(PLACE_BITS)? << (64 - PLACE_BITS)) as i64 >> (64 - PLACE_BITS);
            feet[axis] = steps as f64 / PLACE_STEPS;
        }
        let mut velocity = Vec3::ZERO;
        for axis in 0..3 {
            velocity[axis] = from.quantised(-MOST_SPEED, MOST_SPEED, SPEED_BITS)?;
        }
        Ok(Shown {
            feet,
            velocity,
            facing: read_way(from, Vec3::X)?,
            ground: Ground::read(from)?,
        })
    }
}

impl Shown {
    /// The body a share of the way from this state to a later one: what is drawn of another
    /// player between two words of the server (snapshot interpolation, D651).
    pub fn between(&self, later: &Shown, share: f32) -> Shown {
        Shown {
            feet: self.feet.lerp(later.feet, f64::from(share)),
            velocity: self.velocity.lerp(later.velocity, share),
            facing: self
                .facing
                .lerp(later.facing, share)
                .try_normalize()
                .unwrap_or(later.facing),
            ground: if share < 0.5 {
                self.ground
            } else {
                later.ground
            },
        }
    }
}

/// A body as its own player is told: exact, the server's answer to a tick's prediction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Own {
    pub position: DVec3,
    pub velocity: Vec3,
    pub facing: Vec3,
    pub ground: Ground,
}

impl Wire for Own {
    fn write(&self, to: &mut Writer) {
        self.position.to_array().write(to);
        write_exact(self.velocity, to);
        write_exact(self.facing, to);
        self.ground.write(to);
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        Ok(Own {
            position: DVec3::from_array(<[f64; 3]>::read(from)?),
            velocity: read_exact(from)?,
            facing: read_exact(from)?,
            ground: Ground::read(from)?,
        })
    }
}

/// How far a predicted body may be from the server's and still agree: 1 mm, 1 mm/s.
const AGREES_M: f64 = 1.0e-3;
const AGREES_M_S: f32 = 1.0e-3;

impl Body {
    pub fn shown(&self) -> Shown {
        Shown {
            feet: self.position,
            velocity: self.velocity,
            facing: self.facing,
            ground: self.ground,
        }
    }

    pub fn own(&self) -> Own {
        Own {
            position: self.position,
            velocity: self.velocity,
            facing: self.facing,
            ground: self.ground,
        }
    }

    /// Whether this body, as predicted, is the server's within the tolerance.
    pub fn agrees(&self, own: &Own) -> bool {
        self.ground == own.ground
            && self.position.distance_squared(own.position) <= AGREES_M * AGREES_M
            && self.velocity.distance_squared(own.velocity) <= AGREES_M_S * AGREES_M_S
    }

    /// Sets it to the server's answer: placed there with that velocity, what it stands on
    /// found again, as a teleport is ([`Bodies::place`]).
    pub fn correct(&mut self, own: &Own, world: &World) {
        settle(self, own.position, own.velocity, world);
        self.facing = own.facing;
    }
}

impl Bodies {
    /// Puts a body in as it is, in place of the one of its id if there is one: a predicted
    /// body taken up again after a replay.
    pub fn put(&mut self, body: Body) {
        match self.bodies.binary_search_by_key(&body.id, |held| held.id) {
            Ok(at) => self.bodies[at] = body,
            Err(at) => self.bodies.insert(at, body),
        }
    }
}

/// The most a pace's share is on the wire, and its bits.
const MOST_SCALE: f32 = 2.0;
const SCALE_BITS: u32 = 10;
const PITCH_BITS: u32 = 10;
const MOST_PITCH: f32 = std::f32::consts::FRAC_PI_2;

impl Intent {
    /// The intent as it reads back from the wire: what both ends run.
    pub fn wired(&self) -> Intent {
        Intent::decoded(&self.encoded()).unwrap_or_default()
    }
}

/// A standing, level, full-pace intent is seven bits; a walk is five bytes. `knocked` is the
/// server's to say and never sent.
impl Wire for Intent {
    fn write(&self, to: &mut Writer) {
        let moving = self.wish != Vec3::ZERO;
        to.bool(moving);
        if moving {
            write_way(self.wish, to);
        }
        to.bool(self.sprint);
        to.bool(self.jump);
        to.bool(self.rise);
        to.bool(self.fighting);
        to.bool(self.scale != 1.0);
        if self.scale != 1.0 {
            to.quantised(self.scale, 0.0, MOST_SCALE, SCALE_BITS);
        }
        to.bool(self.pitch != 0.0);
        if self.pitch != 0.0 {
            to.quantised(self.pitch, -MOST_PITCH, MOST_PITCH, PITCH_BITS);
        }
        to.bool(self.drive.is_some());
        if let Some(drive) = self.drive {
            write_exact(drive, to);
        }
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        let wish = if from.bool()? {
            read_way(from, Vec3::ZERO)?
        } else {
            Vec3::ZERO
        };
        let (sprint, jump, rise, fighting) =
            (from.bool()?, from.bool()?, from.bool()?, from.bool()?);
        let scale = if from.bool()? {
            from.quantised(0.0, MOST_SCALE, SCALE_BITS)?
        } else {
            1.0
        };
        let pitch = if from.bool()? {
            from.quantised(-MOST_PITCH, MOST_PITCH, PITCH_BITS)?
        } else {
            0.0
        };
        let drive = if from.bool()? {
            let drive = read_exact(from)?;
            // A stranger's number: nothing a body is driven at is not finite.
            if !drive.is_finite() {
                return Err(Malformed::Range);
            }
            Some(drive)
        } else {
            None
        };
        Ok(Intent {
            wish,
            sprint,
            jump,
            scale,
            drive,
            pitch,
            rise,
            fighting,
            knocked: false,
        })
    }
}

#[cfg(test)]
#[path = "net_tests.rs"]
mod tests;
