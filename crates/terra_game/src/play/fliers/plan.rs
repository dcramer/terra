//! A flier's flight plan (research/2026-10-17-large-flier-motion.md): where it will be and
//! how fast, worked out once when it is told a path, as a fixed-wing flier must fly it.
//!
//! - **Its course is a curve, not corners.** A cubic B-spline over the points, starting
//!   along the way it heads, which cuts each corner as a flier does rather than swinging wide
//!   to pass through it, its bend changing gradually: a heavy flier rolls into a turn, it
//!   cannot snap into one.
//! - **It turns no tighter than its bank lets it** (a coordinated turn: tan φ = v² κ / g), so
//!   where the course bends hard it slows to the speed that bend allows at its most bank, as
//!   a bird or an aircraft does, rather than skidding round at cruising speed.
//! - **It gathers speed and sheds it as something heavy does**: no faster than its wings
//!   accelerate it from where it starts, slowing no harder than it brakes into its hover,
//!   slower in a climb and faster in a dive.
//!
//! The same everywhere and without Bevy's world, so the trailer's scenes work the same plan
//! out in TypeScript (`tools/sandbox/trailer/flight.ts`) to aim the camera at it.

use bevy::math::Vec3;

/// How far apart the plan's points are, in metres.
pub const STEP_M: f32 = 0.5;
/// How far either side of a point its bend is read over, in metres.
const BEND_M: f32 = 2.0;
/// How far ahead of its start, along the way it heads, the course's second control point
/// is, in metres at most, and as a share of the way to its first point.
const LEAD_IN_M: f32 = 16.0;
const LEAD_IN_SHARE: f32 = 0.4;
/// How far either side of a point its speed is eased over, in metres, and how often: a
/// heavy body does not go from gathering speed to shedding it at once.
const EASE_SPEED_M: f32 = 8.0;
const EASE_SPEED_PASSES: usize = 2;
/// How much slower it flies in a climb, and faster in a dive, by the sine of its slope.
const CLIMB_SLOWS: f32 = 0.5;
const DIVE_SPEEDS: f32 = 0.3;
const GRAVITY: f32 = 9.81;

/// What a kind's wings let it do: the slowest it flies a path (its
/// hover's), the most it banks, in radians, and how hard it gathers speed and sheds it, in
/// metres a second a second.
#[derive(Clone, Copy, Debug)]
pub struct Wings {
    pub slowest: f32,
    pub most_bank: f32,
    pub accel: f32,
    pub brake: f32,
}

/// A planned flight: its points a step apart, its speed at each, when it is there from the
/// start, and its bend there on the level (one over the turn's radius, a left turn above
/// nought).
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub points: Vec<Vec3>,
    pub speed: Vec<f32>,
    pub time: Vec<f32>,
    pub bend: Vec<f32>,
}

/// Where a plan has its flier at a moment: the place, its way, its speed, how fast that is
/// changing, its bend on the level, and whether the plan is done.
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub at: Vec3,
    pub way: Vec3,
    pub speed: f32,
    pub accel: f32,
    pub bend: f32,
    pub ended: bool,
}

/// Plans a flight from `from`, heading `way` at `speed_now`, through the points, at most
/// at `cruise`, to slow to its slowest at the last. `up` is the world's up there.
pub fn plan(
    from: Vec3,
    way: Vec3,
    speed_now: f32,
    through: &[Vec3],
    cruise: f32,
    wings: &Wings,
    up: Vec3,
) -> Plan {
    let mut control = vec![from];
    control.extend_from_slice(through);
    control.dedup_by(|a, b| a.distance(*b) < 1e-3);
    if control.len() < 2 {
        let speed = vec![speed_now.min(wings.slowest)];
        return Plan {
            points: control,
            speed,
            time: vec![0.0],
            bend: vec![0.0],
        };
    }
    let way = way
        .normalize_or(control[1] - control[0])
        .normalize_or(Vec3::Z);
    let lead = (control[1].distance(from) * LEAD_IN_SHARE).min(LEAD_IN_M);
    control.insert(1, from + way * lead);
    // It comes into its hover level, as it leaves one: the last control point before the
    // end as high as the end, so it does not tip from a climb or a dive into its hover.
    let n = control.len();
    let (last, before) = (control[n - 1], control[n - 2]);
    let level_way = (last - before).reject_from(up).normalize_or_zero();
    let lead_out = (last.distance(before) * LEAD_IN_SHARE).min(LEAD_IN_M);
    control.insert(n - 1, last - level_way * lead_out);
    let points = resample(&spline(&control));
    let n = points.len();
    let tangent = |i: usize| {
        let (a, b) = (i.saturating_sub(1), (i + 1).min(n - 1));
        (points[b] - points[a]).normalize_or(way)
    };
    let k = ((BEND_M / STEP_M).round() as usize).max(1);
    let mut bend = Vec::with_capacity(n);
    let mut limit = Vec::with_capacity(n);
    for i in 0..n {
        let (a, b) = (i.saturating_sub(k), (i + k).min(n - 1));
        let span = (b - a).max(1) as f32 * STEP_M;
        let (ta, tb) = (tangent(a), tangent(b));
        let whole = (tb - ta).length() / span;
        bend.push(ta.cross(tb).dot(up) / span);
        let slope = tangent(i).dot(up);
        let by_slope = if slope > 0.0 {
            1.0 - CLIMB_SLOWS * slope
        } else {
            1.0 - DIVE_SPEEDS * slope
        };
        let by_bend = (GRAVITY * libm::tanf(wings.most_bank) / whole.max(1e-5)).sqrt();
        limit.push((cruise * by_slope).min(by_bend).max(wings.slowest));
    }
    // Gathering speed from where it is, then shedding it into its hover at the end.
    let bound = |speed: &mut Vec<f32>| {
        speed[0] = speed[0].min(speed_now.max(0.0));
        for i in 1..n {
            let ds = points[i].distance(points[i - 1]);
            let most = (speed[i - 1] * speed[i - 1] + 2.0 * wings.accel * ds).sqrt();
            speed[i] = speed[i].min(most);
        }
        // Come to a stop in its hover, not at its slowest.
        speed[n - 1] = 0.0;
        for i in (0..n - 1).rev() {
            let ds = points[i + 1].distance(points[i]);
            let most = (speed[i + 1] * speed[i + 1] + 2.0 * wings.brake * ds).sqrt();
            speed[i] = speed[i].min(most);
        }
    };
    // What its turns allow, rounded at its least, each the mean of the least either side,
    // so it is never more than allowed and comes down into a turn and up out of it gently.
    let reach = (EASE_SPEED_M / STEP_M).round() as usize;
    let least: Vec<f32> = (0..n)
        .map(|i| {
            limit[i.saturating_sub(reach)..(i + reach + 1).min(n)]
                .iter()
                .copied()
                .fold(f32::MAX, f32::min)
        })
        .collect();
    let limit = ease_speed(&least, reach);
    let mut speed = limit.clone();
    bound(&mut speed);
    // Eased, so it never turns from gathering speed to shedding it in a step, then held
    // within what its turns and its wings allow again.
    for _ in 0..EASE_SPEED_PASSES {
        speed = ease_speed(&speed, reach);
    }
    for (v, most) in speed.iter_mut().zip(&limit) {
        *v = v.min(*most);
    }
    bound(&mut speed);
    let mut time = vec![0.0];
    for i in 1..n {
        let length = points[i].distance(points[i - 1]);
        let mean = ((speed[i] + speed[i - 1]) * 0.5).max(0.05);
        time.push(time[i - 1] + length / mean);
    }
    Plan {
        points,
        speed,
        time,
        bend,
    }
}

impl Plan {
    /// How long the flight takes, in seconds.
    pub fn seconds(&self) -> f32 {
        self.time.last().copied().unwrap_or(0.0)
    }

    /// How long it is, in metres.
    pub fn metres(&self) -> f32 {
        self.points.windows(2).map(|p| p[0].distance(p[1])).sum()
    }

    /// Where it is `seconds` into the flight.
    pub fn at(&self, seconds: f32) -> Sample {
        let n = self.points.len();
        let i = self
            .time
            .partition_point(|&t| t <= seconds)
            .clamp(1, n.max(2) - 1);
        if n < 2 {
            return Sample {
                at: self.points.first().copied().unwrap_or(Vec3::ZERO),
                way: Vec3::Z,
                speed: 0.0,
                accel: 0.0,
                bend: 0.0,
                ended: true,
            };
        }
        let (t0, t1) = (self.time[i - 1], self.time[i]);
        let f = ((seconds - t0) / (t1 - t0).max(1e-5)).clamp(0.0, 1.0);
        let way = (self.points[i] - self.points[i - 1]).normalize_or(Vec3::Z);
        Sample {
            at: self.points[i - 1].lerp(self.points[i], f),
            way,
            speed: self.speed[i - 1] + (self.speed[i] - self.speed[i - 1]) * f,
            accel: (self.speed[i] - self.speed[i - 1]) / (t1 - t0).max(1e-5),
            bend: self.bend[i - 1] + (self.bend[i] - self.bend[i - 1]) * f,
            ended: seconds >= self.seconds(),
        }
    }
}

/// Each speed the mean of those `reach` either side of it, fewer near the ends, so the first
/// and last stay as they are.
fn ease_speed(speed: &[f32], reach: usize) -> Vec<f32> {
    let n = speed.len();
    let mut sums = vec![0.0f32];
    for v in speed {
        sums.push(sums.last().unwrap() + v);
    }
    (0..n)
        .map(|i| {
            let k = reach.min(i).min(n - 1 - i);
            (sums[i + k + 1] - sums[i - k]) / (2 * k + 1) as f32
        })
        .collect()
}

/// A cubic B-spline over the control points, finely, clamped so it starts at the first and
/// ends at the last: it cuts each corner rather than passing through it, so it never swings
/// wide or doubles back between points, and its bend changes smoothly all along.
fn spline(control: &[Vec3]) -> Vec<Vec3> {
    let n = control.len();
    let at = |i: isize| control[i.clamp(0, n as isize - 1) as usize];
    let mut out = vec![control[0]];
    for i in -1..n as isize {
        let p = [at(i - 1), at(i), at(i + 1), at(i + 2)];
        let steps = ((p[3] - p[0]).length() / (STEP_M * 0.25)).ceil().max(1.0) as usize;
        for s in 1..=steps {
            let t = s as f32 / steps as f32;
            let u = 1.0 - t;
            let b0 = u * u * u / 6.0;
            let b1 = (3.0 * t * t * t - 6.0 * t * t + 4.0) / 6.0;
            let b2 = (-3.0 * t * t * t + 3.0 * t * t + 3.0 * t + 1.0) / 6.0;
            let b3 = t * t * t / 6.0;
            out.push(p[0] * b0 + p[1] * b1 + p[2] * b2 + p[3] * b3);
        }
    }
    out
}

/// The line through the points laid out again a step apart along it, its ends kept.
fn resample(line: &[Vec3]) -> Vec<Vec3> {
    let Some(&first) = line.first() else {
        return Vec::new();
    };
    let mut out = vec![first];
    let mut carried = 0.0;
    for pair in line.windows(2) {
        let length = pair[0].distance(pair[1]);
        if length <= 0.0 {
            continue;
        }
        let mut along = STEP_M - carried;
        while along <= length {
            out.push(pair[0].lerp(pair[1], along / length));
            along += STEP_M;
        }
        carried = length - (along - STEP_M);
    }
    let last = *line.last().unwrap();
    if out.last().is_none_or(|p| p.distance(last) > STEP_M * 0.25) {
        out.push(last);
    } else if let Some(end) = out.last_mut() {
        *end = last;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINGS: Wings = Wings {
        slowest: 3.0,
        most_bank: 0.733,
        accel: 4.0,
        brake: 5.0,
    };

    /// A course with a hard corner: north 120 m, then east 120 m.
    fn corner() -> Plan {
        plan(
            Vec3::ZERO,
            Vec3::NEG_Z,
            24.0,
            &[Vec3::new(0.0, 0.0, -120.0), Vec3::new(120.0, 0.0, -120.0)],
            24.0,
            &WINGS,
            Vec3::Y,
        )
    }

    #[test]
    fn it_never_turns_tighter_than_its_most_bank_lets_it_at_its_speed() {
        let plan = corner();
        let most = libm::tanf(WINGS.most_bank);
        for i in 0..plan.points.len() {
            // The bank its speed and bend ask for, by the coordinated turn's law.
            let asks = plan.speed[i] * plan.speed[i] * plan.bend[i].abs() / GRAVITY;
            assert!(asks <= most * 1.05, "point {i} asks a bank of {asks}");
        }
        // The corner was taken slower than it cruises, and right-handed.
        let slowest = plan.speed.iter().copied().fold(f32::MAX, f32::min);
        assert!(slowest < 20.0, "it took the corner at {slowest} m/s");
        assert!(plan.bend.iter().any(|&b| b < -0.01));
        assert!(plan.bend.iter().all(|&b| b < 0.005));
    }

    #[test]
    fn it_eases_from_slowing_into_a_turn_to_speeding_out_of_it() {
        let plan = corner();
        // At the corner, half way, not braking into its hover at the end.
        let n = plan.speed.len();
        let slowest = (n / 4..n * 3 / 4)
            .min_by(|&a, &b| plan.speed[a].total_cmp(&plan.speed[b]))
            .unwrap();
        let accel =
            |i: usize| (plan.speed[i] - plan.speed[i - 1]) / (plan.time[i] - plan.time[i - 1]);
        // Through the turn, how fast its speed changes changes little from point to point:
        // a step from braking at 5 m/s² to gathering speed at 4 would be nearly 9.
        for i in slowest - 30..slowest + 30 {
            let change = (accel(i + 1) - accel(i)).abs();
            assert!(change < 0.5, "its acceleration stepped by {change} at {i}");
        }
    }

    #[test]
    fn its_speed_changes_no_faster_than_it_gathers_and_sheds_speed() {
        let plan = plan(
            Vec3::ZERO,
            Vec3::NEG_Z,
            1.0,
            &[Vec3::new(0.0, 0.0, -300.0)],
            24.0,
            &WINGS,
            Vec3::Y,
        );
        for i in 1..plan.points.len() {
            let dt = plan.time[i] - plan.time[i - 1];
            let a = (plan.speed[i] - plan.speed[i - 1]) / dt;
            assert!(
                (-WINGS.brake * 1.05..=WINGS.accel * 1.05).contains(&a),
                "{a} m/s² at {i}"
            );
        }
        assert!(
            plan.speed.iter().any(|&v| v > 23.5),
            "it never reached cruise"
        );
        assert!(*plan.speed.last().unwrap() <= WINGS.slowest + 1e-3);
        // From 1 m/s to 24 at 4 m/s² takes about 6 s.
        let reached = plan.speed.iter().position(|&v| v > 23.5).unwrap();
        assert!(
            (plan.time[reached] - 5.75).abs() < 0.5,
            "{}",
            plan.time[reached]
        );
    }

    #[test]
    fn its_course_starts_where_it_is_ends_at_the_last_point_and_bends_smoothly() {
        let plan = corner();
        assert!(plan.points[0].distance(Vec3::ZERO) < 1e-3);
        let end = *plan.points.last().unwrap();
        assert!(end.distance(Vec3::new(120.0, 0.0, -120.0)) < 1e-3, "{end}");
        for pair in plan.points.windows(2) {
            let step = pair[0].distance(pair[1]);
            assert!(step <= STEP_M * 1.3, "a step of {step}");
        }
        // Its bend changes by little from one point to the next: no corner.
        for pair in plan.bend.windows(2) {
            assert!((pair[1] - pair[0]).abs() < 0.01, "{pair:?}");
        }
        let half = plan.at(plan.seconds() * 0.5);
        assert!(!half.ended && half.speed > 0.0);
        assert!(plan.at(plan.seconds() + 1.0).ended);
    }
}
