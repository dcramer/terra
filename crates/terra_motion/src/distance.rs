//! Distance matching (Lyra's distance-matched starts and stops: UE5's Animation Locomotion
//! Library, `DistanceMatchToTarget` and `AdvanceTimeByDistanceMatching`, its documentation;
//! D455): a start or a stop is a clip that keeps its root motion, its pelvis's travel baked
//! as a distance curve (`Markers::distance`, `tools/characters/markers.ts`). The game takes
//! the travel off the pelvis and moves the body itself, and picks the clip's time by distance,
//! not by the clock: a stop at the time whose distance left to the clip's own stop is the
//! body's predicted stopping distance, a start advanced by as far as the body went. So the
//! clip's planted feet stand still on the ground however hard the mover brakes or pulls away.
//! And a loop entered from a start takes the start's step: its time where it is as far between
//! the same feet's plants (Unreal's sync markers, `foot_down_l`/`foot_down_r`).

use crate::feet::Markers;

/// The clip has stopped travelling once its curve is this near its farthest, m.
pub const STOPPED_M: f32 = 0.01;
/// A start goes from where its curve first passes this, m: before it the clip stands.
pub const GOES_M: f32 = 0.01;
/// A start's play rate, as its distance asks, is held within this (Lyra's start's play rate
/// clamp): a body pulling away faster than the clip does not stop it, nor a slow one freeze it.
pub const RATE_CLAMP: (f32, f32) = (0.6, 5.0);
/// A start has reached its run once its speed is this share of its fastest.
pub const CRUISE_SHARE: f32 = 0.9;
/// Speeds are read over this many samples.
const SPAN: usize = 6;

impl Markers {
    /// Whether the clip keeps its root motion and has a distance curve.
    pub fn travels(&self) -> bool {
        self.distance.len() > 1
    }

    /// How far the clip's pelvis has gone along its way `t` seconds in, between samples; held
    /// at its ends.
    pub fn travelled(&self, t: f32) -> f32 {
        let d = &self.distance;
        let Some(&last) = d.last() else {
            return 0.0;
        };
        let at = t.clamp(0.0, self.length_s) * self.rate;
        let i = at as usize;
        if i + 1 >= d.len() {
            return last;
        }
        d[i] + (d[i + 1] - d[i]) * (at - i as f32)
    }

    /// The first time from `from` on at which the clip has gone `d`, between samples; its end if
    /// it never does.
    pub fn time_at(&self, d: f32, from: f32) -> f32 {
        let c = &self.distance;
        let first = ((from.max(0.0) * self.rate) as usize).min(c.len().saturating_sub(1));
        if c.get(first).is_some_and(|&x| x >= d) {
            return from.max(0.0);
        }
        for i in first + 1..c.len() {
            if c[i] >= d {
                let before = c[i - 1];
                let s = if c[i] > before {
                    ((d - before) / (c[i] - before)).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                return ((i - 1) as f32 + s) / self.rate;
            }
        }
        self.length_s
    }

    /// Where the clip stops: the time it first comes within STOPPED_M of its farthest, and that
    /// farthest.
    pub fn stop(&self) -> (f32, f32) {
        let end = self.distance.iter().copied().fold(0.0, f32::max);
        (self.time_at(end - STOPPED_M, 0.0), end)
    }

    /// A stop's time with `left` metres to go (the body's predicted stopping distance, in the
    /// clip's units): where the clip's own distance to its stop is that, never back from `was`
    /// (Lyra's `DistanceMatchToTarget`). A stop shorter than the clip's braking plays from
    /// where its distance matches.
    pub fn stop_time(&self, left: f32, was: f32) -> f32 {
        let (at, end) = self.stop();
        self.time_at(end - left.max(0.0), 0.0).min(at).max(was)
    }

    /// Where a start goes from: its first time past GOES_M.
    pub fn goes(&self) -> f32 {
        self.time_at(GOES_M, 0.0)
    }

    /// A start's time `dt` on from `was`, the body having gone `moved` (the clip's units):
    /// where the clip has gone as much farther, its play rate held within RATE_CLAMP (Lyra's
    /// `AdvanceTimeByDistanceMatching`).
    pub fn start_time(&self, was: f32, moved: f32, dt: f32) -> f32 {
        if dt <= 0.0 {
            return was;
        }
        let to = self.time_at(self.travelled(was) + moved.max(0.0), was);
        let rate = ((to - was) / dt).clamp(RATE_CLAMP.0, RATE_CLAMP.1);
        (was + rate * dt).min(self.length_s)
    }

    /// The clip's speed `t` seconds in, over SPAN samples, m/s in its units.
    fn speed(&self, i: usize) -> f32 {
        let c = &self.distance;
        let j = (i + SPAN).min(c.len() - 1);
        if j <= i {
            return 0.0;
        }
        (c[j] - c[i]) * self.rate / (j - i) as f32
    }

    /// When a start has reached its run: the first time from where it goes that its speed is
    /// CRUISE_SHARE of its fastest.
    pub fn cruise(&self) -> f32 {
        if !self.travels() {
            return 0.0;
        }
        let fastest = (0..self.distance.len())
            .map(|i| self.speed(i))
            .fold(0.0, f32::max);
        let from = (self.goes() * self.rate) as usize;
        (from..self.distance.len())
            .find(|&i| self.speed(i) >= fastest * CRUISE_SHARE)
            .map_or(self.length_s, |i| i as f32 / self.rate)
    }

    /// Each plant's start, the clip's sync markers (foot, seconds), in order; a plant from the
    /// clip's first instant (a start's standing feet) is no step and none.
    fn steps(&self) -> Vec<(usize, f32)> {
        let mut out: Vec<(usize, f32)> = [&self.foot_down_l, &self.foot_down_r]
            .into_iter()
            .enumerate()
            .flat_map(|(side, spans)| spans.iter().map(move |span| (side, span[0])))
            .filter(|&(_, at)| at > 0.0)
            .collect();
        out.sort_by(|a, b| a.1.total_cmp(&b.1));
        out
    }

    /// Where the clip is in its steps `t` seconds in: the foot whose plant came last, and how
    /// far it is to the next plant, 0 to 1; none before its first or after its last.
    pub fn phase(&self, t: f32) -> Option<(usize, f32)> {
        let steps = self.steps();
        let i = steps.iter().rposition(|&(_, at)| at <= t)?;
        let (side, at) = steps[i];
        let next = steps.get(i + 1)?.1;
        Some((side, ((t - at) / (next - at).max(1e-3)).clamp(0.0, 1.0)))
    }

    /// A loop's plants (foot, seconds), in order, every one: in a loop a plant at its first
    /// instant is a step like any other.
    fn loop_steps(&self) -> Vec<(usize, f32)> {
        let mut out: Vec<(usize, f32)> = [&self.foot_down_l, &self.foot_down_r]
            .into_iter()
            .enumerate()
            .flat_map(|(side, spans)| spans.iter().map(move |span| (side, span[0])))
            .collect();
        out.sort_by(|a, b| a.1.total_cmp(&b.1));
        out
    }

    /// Where a loop is in its steps `t` seconds in, as `phase` has it but round the loop's end:
    /// before its first plant it is on from its last plant of the cycle before, after its last
    /// on to its first of the next. None if it has no plant.
    pub fn phase_in_loop(&self, t: f32) -> Option<(usize, f32)> {
        let steps = self.loop_steps();
        let length = self.length_s.max(1e-3);
        let t = t.rem_euclid(length);
        let (&(last_side, last_at), &(_, first_at)) = (steps.last()?, steps.first()?);
        let (side, at, next) = match steps.iter().rposition(|&(_, at)| at <= t) {
            Some(i) => {
                let next = steps.get(i + 1).map_or(first_at + length, |s| s.1);
                (steps[i].0, steps[i].1, next)
            }
            None => (last_side, last_at - length, first_at),
        };
        Some((side, ((t - at) / (next - at).max(1e-3)).clamp(0.0, 1.0)))
    }

    /// Sync groups' marker sync (Unreal's sync markers): the time in this loop at the same step
    /// as `from` is `t` seconds in, the same foot planted last and as far on to the next plant,
    /// so a stride loop taking over from another is not restarted out of step with its feet.
    /// None if either has no plants of that foot.
    pub fn synced(&self, from: &Markers, t: f32) -> Option<f32> {
        let (side, share) = from.phase_in_loop(t)?;
        self.loop_steps()
            .iter()
            .any(|&(s, _)| s == side)
            .then(|| self.time_in_loop(side, share))
    }

    /// The time in this loop as far (`share`, 0 to 1) from foot `side`'s plant to the next
    /// plant as a phase (`phase`) has it; its start if it has no such plant.
    pub fn time_in_loop(&self, side: usize, share: f32) -> f32 {
        let steps = self.loop_steps();
        let Some(i) = steps.iter().position(|&(s, _)| s == side) else {
            return 0.0;
        };
        let at = steps[i].1;
        // The next plant, round the loop's end if it is the last.
        let next = match steps.get(i + 1) {
            Some(&(_, next)) => next,
            None => steps[0].1 + self.length_s,
        };
        (at + share * (next - at)) % self.length_s.max(1e-3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clip braking from 2 m/s to a stop over its first second (d = 2t - t^2), then standing
    /// a second, at 60 samples a second.
    fn stop() -> Markers {
        let rate = 60.0;
        Markers {
            length_s: 2.0,
            rate,
            distance: (0..=120)
                .map(|i| {
                    let t = (i as f32 / rate).min(1.0);
                    2.0 * t - t * t
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn a_stop_plays_where_its_distance_left_is_the_bodys_and_never_goes_back() {
        let clip = stop();
        let (at, end) = clip.stop();
        assert!((end - 1.0).abs() < 1e-4);
        // Within a centimetre of its end: 1 - (1 - t)^2 = 0.99 at t = 0.9.
        assert!((at - 0.9).abs() < 0.02, "{at}");
        // 0.25 m left: d = 0.75, t = 0.5.
        let t = clip.stop_time(0.25, 0.0);
        assert!((t - 0.5).abs() < 0.01, "{t}");
        // A body braking harder than the clip: as far as its distance left, later in the clip.
        assert!(clip.stop_time(0.04, t) > 0.75);
        // Never back, and at most the stop.
        assert_eq!(clip.stop_time(0.5, 0.7), 0.7);
        assert!((clip.stop_time(0.0, 0.0) - at).abs() < 1e-4);
    }

    #[test]
    fn a_start_advances_by_the_bodys_distance_within_its_play_rates() {
        // Standing half a second, then 2 m/s.
        let rate = 60.0;
        let clip = Markers {
            length_s: 2.0,
            rate,
            distance: (0..=120)
                .map(|i| (i as f32 / rate - 0.5).max(0.0) * 2.0)
                .collect(),
            ..Default::default()
        };
        let goes = clip.goes();
        assert!((goes - 0.505).abs() < 0.01, "{goes}");
        // The body going 2 m/s plays it at its own pace; at 4 m/s twice as fast.
        let dt = 1.0 / 60.0;
        let t = clip.start_time(1.0, 2.0 * dt, dt);
        assert!((t - (1.0 + dt)).abs() < 1e-3, "{t}");
        let t = clip.start_time(1.0, 4.0 * dt, dt);
        assert!((t - (1.0 + 2.0 * dt)).abs() < 1e-3, "{t}");
        // Standing, it creeps on at the least rate; far too fast, the most.
        assert!((clip.start_time(1.0, 0.0, dt) - (1.0 + 0.6 * dt)).abs() < 1e-4);
        assert!((clip.start_time(1.0, 1.0, dt) - (1.0 + 5.0 * dt)).abs() < 1e-4);
        // It runs from where it first goes at its full speed.
        assert!((clip.cruise() - goes).abs() < 0.03, "{}", clip.cruise());
    }

    #[test]
    fn a_loop_entered_from_a_start_takes_the_starts_step() {
        // A start: standing feet from 0, then the left down at 1.0, the right at 1.4, the left at 1.8.
        let start = Markers {
            length_s: 2.0,
            rate: 60.0,
            foot_down_l: vec![[0.0, 0.9], [1.0, 1.2], [1.8, 2.0]],
            foot_down_r: vec![[0.0, 1.1], [1.4, 1.6]],
            ..Default::default()
        };
        // Halfway from the left's plant to the right's.
        let (side, share) = start.phase(1.2).unwrap();
        assert_eq!(side, 0);
        assert!((share - 0.5).abs() < 1e-4);
        assert_eq!(start.phase(0.5), None);
        // A loop: the right down at 0.3, the left at 0.7, 0.8 s long.
        let jog = Markers {
            length_s: 0.8,
            rate: 60.0,
            foot_down_l: vec![[0.7, 0.1]],
            foot_down_r: vec![[0.3, 0.5]],
            ..Default::default()
        };
        // Halfway from the left's (0.7) to the right's (1.1 round the end): 0.9, so 0.1.
        assert!((jog.time_in_loop(0, 0.5) - 0.1).abs() < 1e-4);
        assert!((jog.time_in_loop(1, 0.5) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn a_loop_taking_over_from_another_keeps_the_step_round_its_end() {
        // The jog above, and a sprint 1.0 s long with the left down at 0.1 and the right at 0.6.
        let jog = Markers {
            length_s: 0.8,
            foot_down_l: vec![[0.7, 0.1]],
            foot_down_r: vec![[0.3, 0.5]],
            ..Default::default()
        };
        let sprint = Markers {
            length_s: 1.0,
            foot_down_l: vec![[0.1, 0.4]],
            foot_down_r: vec![[0.6, 0.9]],
            ..Default::default()
        };
        // Past the jog's last plant and before its first, the phase goes on round the end:
        // 0.75 and 0.1 are an eighth and half of the way from the left's (0.7) to the right's (1.1).
        let (side, share) = jog.phase_in_loop(0.75).unwrap();
        assert_eq!(side, 0);
        assert!((share - 0.125).abs() < 1e-4, "{share}");
        let (side, share) = jog.phase_in_loop(0.1).unwrap();
        assert_eq!(side, 0);
        assert!((share - 0.5).abs() < 1e-4, "{share}");
        // The sprint taking over then has the same foot as far from its plant to the next:
        // the left from 0.1 to 0.6, the right from 0.6 to 1.1 round its end.
        for (t, want) in [(0.75, 0.1625), (0.1, 0.35), (0.4, 0.725)] {
            let at = sprint.synced(&jog, t).unwrap();
            assert!(
                (at - want).abs() < 1e-4,
                "jog {t} -> sprint {at}, not {want}"
            );
        }
        // A loop without plants has no step to keep.
        assert_eq!(sprint.synced(&Markers::default(), 0.3), None);
    }

    #[test]
    fn a_clip_played_in_place_has_no_travel() {
        let clip = Markers::default();
        assert!(!clip.travels());
        assert_eq!(clip.travelled(0.3), 0.0);
    }
}
