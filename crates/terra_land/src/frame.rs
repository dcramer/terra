//! Where a place is on its world: its latitude, which ways are north and east there, and
//! how its ground leans toward the equator. The climate asks only these (`terra_climate::Place`);
//! the world knows its own shape and answers them.
//!
//! The world turns about an axis fixed in its frame, the one the sky turns about (`daylight`):
//! chosen so the planet's own start stands at the start's latitude with the morning sun where
//! the art was lit (`SkyPath::over`). Every point's latitude follows from that axis, so on the
//! 30 km world a degree is 524 m walked north (docs/demos.md).

use glam::{Vec2, Vec3};

/// The start's latitude: the planet's own start stands this far north of the equator.
pub const START_LATITUDE: f32 = 40.0_f32.to_radians();
/// The hour of the morning whose sun the art was lit under, and where that sun stands seen
/// from the planet's start: low over the water (`scene::SUN_DIRECTION`).
pub const MORNING: f32 = 7.5;
pub const ART_SUN: Vec3 = Vec3::new(0.74, 0.33, 0.58);

/// A world's frame: the axis it turns about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// Toward the north pole, from the centre: what the pole star is seen along everywhere.
    pole: Vec3,
}

impl Frame {
    /// The frame whose sky over a point is turned so that, on a day of so much declination, the
    /// morning sun stands where the art put it, at so much latitude. Also returns that point's
    /// east and north.
    pub fn turned_for(up: Vec3, latitude: f32, declination: f32) -> (Self, Vec3, Vec3) {
        let up = up.normalize();
        let (x, y, _) = sun_local(latitude, MORNING, declination);
        let local = Vec2::new(x, y).normalize();
        let toward = ART_SUN.reject_from(up).normalize();
        let across = up.cross(toward);
        let east = toward * local.x - across * local.y;
        let north = toward * local.y + across * local.x;
        let pole = (north * libm::cosf(latitude) + up * libm::sinf(latitude)).normalize();
        (Self { pole }, east, north)
    }

    /// A frame turning about an axis toward the north pole.
    pub fn about(pole: Vec3) -> Self {
        Self {
            pole: pole.normalize(),
        }
    }

    pub fn pole(&self) -> Vec3 {
        self.pole
    }

    /// A point's latitude, in radians: north of the equator above 0.
    pub fn latitude(&self, at: Vec3) -> f32 {
        libm::asinf(at.normalize_or(Vec3::Y).dot(self.pole).clamp(-1.0, 1.0))
    }

    /// Which way is north at a point, level there: toward the pole along the ground. At a
    /// pole itself, any way.
    pub fn north(&self, at: Vec3) -> Vec3 {
        let up = at.normalize_or(Vec3::Y);
        self.pole
            .reject_from(up)
            .try_normalize()
            .unwrap_or_else(|| up.any_orthonormal_vector())
    }

    /// Which way is east at a point, level there.
    pub fn east(&self, at: Vec3) -> Vec3 {
        self.north(at).cross(at.normalize_or(Vec3::Y))
    }

    /// How far ground facing `normal` at a point leans toward the equator, in radians: its
    /// slope's angle times the cosine of how far it faces from the equator, below 0 where it
    /// leans toward the pole (`terra_climate::Place::sunward`, D323).
    pub fn sunward(&self, at: Vec3, normal: Vec3) -> f32 {
        let up = at.normalize_or(Vec3::Y);
        let n = normal.normalize_or(up);
        let level = n.reject_from(up);
        let along = level.length();
        if along < 1e-6 {
            return 0.0;
        }
        let slope = libm::atan2f(along, n.dot(up).max(0.0));
        // The equator is south in the north, north in the south.
        let toward_equator = if self.latitude(at) >= 0.0 { -1.0 } else { 1.0 } * self.north(at);
        slope * (level / along).dot(toward_equator)
    }
}

/// The sun at an hour, east, north and up over a place at a latitude, on a day of a
/// declination.
pub fn sun_local(latitude: f32, hour: f32, declination: f32) -> (f32, f32, f32) {
    let angle = (hour - 12.0) / 24.0 * core::f32::consts::TAU;
    let (sd, cd) = (libm::sinf(declination), libm::cosf(declination));
    let (sl, cl) = (libm::sinf(latitude), libm::cosf(latitude));
    let (sh, ch) = (libm::sinf(angle), libm::cosf(angle));
    (-cd * sh, sd * cl - cd * ch * sl, sd * sl + cd * ch * cl)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> (Frame, Vec3, Vec3) {
        Frame::turned_for(Vec3::Y, START_LATITUDE, 0.17)
    }

    #[test]
    fn the_start_is_at_its_latitude_and_walking_north_goes_further_north() {
        let (frame, east, north) = frame();
        assert!((frame.latitude(Vec3::Y * 30_000.0) - START_LATITUDE).abs() < 1e-5);
        assert!(frame.north(Vec3::Y).distance(north) < 1e-5);
        assert!(frame.east(Vec3::Y).distance(east) < 1e-5);
        // A kilometre north on a 30 km world: about two degrees, as an arc of it is.
        let walked = (Vec3::Y * 30_000.0 + north * 1000.0).normalize() * 30_000.0;
        let gained = (frame.latitude(walked) - START_LATITUDE).to_degrees();
        assert!(
            (gained - 1000.0 / 30_000.0 * 57.2958).abs() < 0.05,
            "{gained}"
        );
        // East keeps the latitude, nearly: a small circle bends away from a straight line.
        let eastward = (Vec3::Y * 30_000.0 + east * 100.0).normalize();
        assert!((frame.latitude(eastward) - START_LATITUDE).abs() < 1e-3);
    }

    #[test]
    fn the_morning_sun_stands_where_the_art_put_it() {
        let (_, east, north) = frame();
        let (x, y, z) = sun_local(START_LATITUDE, MORNING, 0.17);
        let sun = east * x + north * y + Vec3::Y * z;
        let flat = |v: Vec3| v.reject_from(Vec3::Y).normalize();
        assert!(flat(sun).distance(flat(ART_SUN)) < 1e-4);
    }

    #[test]
    fn ground_leaning_toward_the_equator_is_sunward_and_toward_the_pole_not() {
        let (frame, east, north) = frame();
        let at = Vec3::Y * 30_000.0;
        let tilt = 0.4_f32;
        let leaning = |toward: Vec3| Vec3::Y * libm::cosf(tilt) + toward * libm::sinf(tilt);
        assert!((frame.sunward(at, leaning(-north)) - tilt).abs() < 1e-4);
        assert!((frame.sunward(at, leaning(north)) + tilt).abs() < 1e-4);
        assert!(frame.sunward(at, leaning(east)).abs() < 1e-4);
        assert_eq!(frame.sunward(at, Vec3::Y), 0.0);
    }
}
