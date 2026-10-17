//! Waves on water (D322), the lakes' and the sea's: the trains of plane waves both surfaces are
//! summed from (`assets/shaders/waves.wesl`), the wind over them, and, for a surface too big to
//! lay its waves out from one point, a frame that travels with the camera keeping them where
//! they are in the world.
//!
//! The trains are summed as Finch sums them ("Effective Water Simulation from Physical
//! Models", GPU Gems, 2004): each a plane wave, its length from the longest to the shortest in
//! even ratios, a little jittered so no two beat in step, running at its own speed by water's
//! dispersion, so the pattern changes as it goes rather than sliding as one picture. Each runs
//! one fixed way, the wind deciding how strong it is by how near its way is to the wind's, as
//! a wave spectrum's spreading does (Tessendorf, "Simulating Ocean Water", 2001): turning the
//! trains with the wind instead swung every ripple's phase whenever the wind turned a hair.

use bevy::math::DVec3;
use bevy::prelude::*;

/// How many trains of waves make a surface, the first `SWELL` of them long ones.
pub const TRAINS: usize = 28;
/// How many of the trains are the swell, longer than 55 cm: always running, even on glassy
/// water, where the shorter ones come with the wind's gusts.
pub const SWELL: usize = 11;
/// The longest train's length and the shortest's, in metres: from the swell a breeze leaves
/// on a mountain lake down to the capillary ripples a gust raises.
const LONGEST_M: f32 = 3.2;
const SHORTEST_M: f32 = 0.035;

/// The trains, longest first: each one's wavenumber (radians a metre), angular frequency
/// (radians a second), the way it runs (radians from east toward north) and its phase.
pub fn trains() -> [Vec4; TRAINS] {
    let ratio = libm::powf(SHORTEST_M / LONGEST_M, 1.0 / (TRAINS - 1) as f32);
    std::array::from_fn(|i| {
        // A fixed scatter, the same every run, and uncorrelated from train to train: an even
        // one (the golden ratio's multiples) turned the trains in step, and their crests
        // crossed in a lattice.
        let scatter = |salt: u32| scatter(i as u32 * 3 + salt);
        let length = LONGEST_M * ratio.powi(i as i32) * (0.95 + 0.1 * scatter(0));
        let k = std::f32::consts::TAU / length;
        Vec4::new(
            k,
            angular_frequency(k),
            std::f32::consts::TAU * scatter(1),
            std::f32::consts::TAU * scatter(2),
        )
    })
}

/// A number from 0 to 1 for each whole number, by Wellons's lowbias32 hash.
fn scatter(n: u32) -> f32 {
    let x = terra_gen::hash::lowbias32(n.wrapping_add(0x9e37_79b9));
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// How fast a wave of so many radians a metre turns, radians a second, on deep water:
/// ω² = gk + (σ/ρ)k³, gravity's term and surface tension's (water's 0.0728 N/m over 1000
/// kg/m³), which takes over below about 1.7 cm.
pub fn angular_frequency(k: f32) -> f32 {
    (9.81 * k + 7.28e-5 * k * k * k).sqrt()
}

/// The wind over water as the shaders take it: which way it blows, level, and how hard, 0
/// still to 1 a stiff breeze of 8 m/s.
pub fn over_water(wind: Option<&crate::wind::WorldWind>) -> Vec4 {
    wind.map_or(Vec4::ZERO, |wind| {
        wind.toward.extend((wind.speed_m_s / 8.0).clamp(0.0, 1.0))
    })
}

/// A frame that travels with the camera over a surface too big to lay its waves out from one
/// point, as Bruneton's ocean on a planet does (Proland): its origin under the camera and its
/// east and north there, so the shader works in small numbers near the eye, and each train's
/// phase moved on as the origin moves, worked in double precision, so the waves stay where
/// they are in the world. Moving the origin keeps every wave exactly where it was, at any
/// speed; turning the frame to stay level as it goes round the planet moves the waves away
/// from the origin a little, as much as laying flat waves on a sphere must, so it turns by the
/// least turn, never spinning even over a pole, and eases level after a jump (a warp)
/// rather than turning at once.
#[derive(Clone, Debug)]
pub struct Travelling {
    origin: DVec3,
    east: DVec3,
    north: DVec3,
    phases: [f64; TRAINS],
}

/// How far the frame turns a step to come level: all of the way while it is within this, which
/// a run round a 2 km world turns it a step, else a share of the way.
const TURN_RAD: f64 = 1e-4;
const EASE: f64 = 0.05;

impl Travelling {
    /// A frame at a point, with east and north there.
    pub fn new(at: Vec3, east: Vec3) -> Self {
        let up = at.as_dvec3().normalize();
        let east = east.as_dvec3().reject_from(up).normalize();
        Self {
            origin: at.as_dvec3(),
            east,
            north: up.cross(east),
            phases: [0.0; TRAINS],
        }
    }

    /// Moves the frame to a point, carrying the trains' phases with it, and turns it toward
    /// level there.
    pub fn follow(&mut self, at: Vec3, trains: &[Vec4; TRAINS]) {
        let at = at.as_dvec3();
        let moved = at - self.origin;
        let (east, north) = (moved.dot(self.east), moved.dot(self.north));
        for (phase, train) in self.phases.iter_mut().zip(trains) {
            let way = f64::from(train.z);
            let along = east * libm::cos(way) + north * libm::sin(way);
            *phase = (*phase + f64::from(train.x) * along).rem_euclid(std::f64::consts::TAU);
        }
        self.origin = at;
        let (up, level) = (self.east.cross(self.north), at.normalize());
        let angle = up.angle_between(level);
        if angle > 1e-12 {
            let turn = bevy::math::DQuat::from_axis_angle(
                up.cross(level).normalize(),
                angle.min(TURN_RAD.max(angle * EASE)),
            );
            self.east = turn * self.east;
            self.north = turn * self.north;
        }
    }

    /// How far the frame is from level where it is, radians.
    pub fn tilt(&self) -> f64 {
        self.east
            .cross(self.north)
            .angle_between(self.origin.normalize())
    }

    pub fn origin(&self) -> Vec3 {
        self.origin.as_vec3()
    }

    pub fn east(&self) -> Vec3 {
        self.east.as_vec3()
    }

    pub fn north(&self) -> Vec3 {
        self.north.as_vec3()
    }

    /// The trains with their phases where the frame is now.
    pub fn trains(&self, trains: &[Vec4; TRAINS]) -> [Vec4; TRAINS] {
        std::array::from_fn(|i| {
            let phase = (f64::from(trains[i].w) + self.phases[i]) % std::f64::consts::TAU;
            trains[i].with_w(phase as f32)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waves_run_at_water_speeds() {
        // A deep-water wave 1.56 m long comes round once a second (c = √(gλ/2π)).
        let period = std::f32::consts::TAU / angular_frequency(std::f32::consts::TAU / 1.56);
        assert!((period - 1.0).abs() < 0.01, "{period}");
        // Surface tension makes the slowest wave on water a ripple about 1.7 cm long, running
        // at about 23 cm/s; longer and shorter ones both run faster.
        let (length, speed) = (5..60)
            .map(|mm| {
                let k = std::f32::consts::TAU / (mm as f32 * 0.001);
                (mm as f32 * 0.001, angular_frequency(k) / k)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        assert!((0.016..0.019).contains(&length), "{length}");
        assert!((speed - 0.231).abs() < 0.005, "{speed}");
    }

    #[test]
    fn the_swell_is_the_long_trains_and_the_ripples_the_short() {
        let trains = trains();
        let length = |t: &Vec4| std::f32::consts::TAU / t.x;
        for (i, train) in trains.iter().enumerate() {
            assert_eq!(
                i < SWELL,
                length(train) > 0.55,
                "train {i}, {} m",
                length(train)
            );
            // Each runs by its own length's speed, so the pattern changes as it goes.
            assert_eq!(train.y, angular_frequency(train.x));
        }
        assert!(trains.windows(2).all(|w| w[0].x < w[1].x), "longest first");
        assert!(
            length(&trains[TRAINS - 1]) < 0.05,
            "down to a gust's ripples"
        );
    }

    /// A train's phase at a point of the world, as the shader works it out from a frame.
    fn phase_at(frame: &Travelling, train: usize, point: Vec3) -> f64 {
        let t = frame.trains(&trains())[train];
        let off = point.as_dvec3() - frame.origin;
        let (east, north) = (off.dot(frame.east), off.dot(frame.north));
        let way = f64::from(t.z);
        (f64::from(t.x) * (east * libm::cos(way) + north * libm::sin(way)) + f64::from(t.w))
            .rem_euclid(std::f64::consts::TAU)
    }

    #[test]
    fn the_waves_stay_where_they_are_as_the_frame_travels() {
        // A camera walking 600 m at 5 m/s round a 2 km world, a frame at a time: the finest
        // ripple 12 m from it moves at most a sliver of its turn from one frame to the next, as
        // much as laying flat waves on so small a sphere must, where without its phase carried
        // it would jump a dozen turns.
        let radius = 2_000.0;
        let start = Vec3::new(0.3, 0.8, 0.5).normalize() * radius;
        let east = Vec3::Y.cross(start).normalize();
        let mut frame = Travelling::new(start, east);
        let heading = start.cross(Vec3::X).normalize();
        // The finest ripple, 3.5 cm, which a pixel still shows 12 m off.
        let shown = TRAINS - 1;
        let mut at = start;
        let mut worst: f64 = 0.0;
        for _ in 0..7200 {
            let forward = at
                .normalize()
                .cross(heading.cross(at.normalize()))
                .normalize();
            at = (at + forward * (5.0 / 60.0)).normalize() * radius;
            let near = (at + forward * 8.0 + east * 6.0).normalize() * radius;
            let before = phase_at(&frame, shown, near);
            frame.follow(at, &trains());
            let off = (phase_at(&frame, shown, near) - before).rem_euclid(std::f64::consts::TAU);
            worst = worst.max(off.min(std::f64::consts::TAU - off));
        }
        assert!(
            worst < 0.2,
            "a ripple 12 m off jumped {worst} rad in a frame"
        );
        assert!((frame.origin() - at).length() < 0.01);
        assert!(frame.tilt() < 0.001, "{}", frame.tilt());
        // Warped a kilometre round, it comes level within a few seconds.
        let warped = (at + heading * 1000.0).normalize() * radius;
        for _ in 0..180 {
            frame.follow(warped, &trains());
        }
        assert!(frame.tilt() < 0.01, "{}", frame.tilt());
    }
}
