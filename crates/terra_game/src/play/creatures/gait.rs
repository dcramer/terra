//! How a shy creature makes off: which way (`bolt`) and how fast, by why it goes (`Pace`,
//! D384). One put up a while ago and crossing someone's way goes at a brisk walk, and breaks
//! into a run only once they come within the distance its kind flees at; one startled by
//! someone coming runs off short of its full speed; only one hurt, or frightened by a dragon
//! or a fire, runs flat out. Shipped open worlds stage their ambient wildlife so (Red Dead
//! Redemption 2's deer walking across the trail and bounding off as the rider comes near):
//! an animal always at a full gallop reads as panic, not as a forest going about its day.

use crate::play::species::SpeciesDef;
use bevy::prelude::*;

/// Why a creature is on the move, which sets how fast it goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pace {
    /// Put up a while ago and crossing someone's way, unhurried.
    Calm,
    /// Moving off at a walk from someone come within the distance its kind flees at, a duck
    /// paddling and a heron stalking away, before they come close (`BOLTS_WITHIN`).
    Wary,
    /// Startled by someone coming.
    Startled,
    /// Hurt, or frightened by a dragon or a fire.
    Flat,
}

/// A calm crossing walks so many times its walking speed, a brisk walk its walk clip keeps
/// up with, and no slower than so many metres a second, or its run if that is slower: a
/// squirrel's or a hare's walk is a crawl, its run a scamper, not a panic.
const CALM_WALK: f32 = 1.6;
const CALM_LEAST_MPS: f32 = 1.2;
/// A calm flier crossing someone's way flies at so much of its speed, a startled one so much.
const CALM_FLIGHT: f32 = 0.7;
const STARTLED_FLIGHT: f32 = 0.85;
/// Startled, a runner runs off at so much of its full run.
const STARTLED_RUN: f32 = 0.75;
/// A wary one walks off at so many times its walking speed.
const WARY_WALK: f32 = 1.4;
/// Someone within the distance its kind flees at, an unhurt one first walks, paddles or wades
/// off (`Pace::Wary`), and runs, climbs or takes wing only once they come within so much of
/// it: wildlife keeps its distance from a passer-by and bolts only from one who comes close,
/// as ducks paddle off from a walker on the bank and a heron stalks away along the shallows
/// before either flushes (D384). A small animal's flight distance is short already: it bolts
/// from no nearer than so many metres, or at once within its distance if that is shorter.
const BOLTS_WITHIN: f32 = 0.4;
const BOLTS_LEAST_M: f32 = 4.0;

/// How near someone comes before a wary one of a kind fleeing at this distance bolts.
pub fn bolts_within(flees_within_m: f32) -> f32 {
    (flees_within_m * BOLTS_WITHIN)
        .max(BOLTS_LEAST_M)
        .min(flees_within_m)
}
/// Its walk clip plays up to so many times its own pace before its run clip takes over.
const WALK_CLIP_UP_TO: f32 = 1.8;

impl Pace {
    /// How fast one of a kind goes on foot at this pace, metres a second.
    pub fn runs_mps(self, about: &SpeciesDef) -> f32 {
        match self {
            Pace::Calm => (about.walk_mps * CALM_WALK).max(about.run_mps.min(CALM_LEAST_MPS)),
            Pace::Wary => about.walk_mps * WARY_WALK,
            Pace::Startled => about.run_mps * STARTLED_RUN,
            Pace::Flat => about.run_mps,
        }
        .min(about.run_mps)
    }

    /// How fast it flies at this pace, given its kind's own speed on the wing.
    pub fn flies_mps(self, mps: f32) -> f32 {
        match self {
            Pace::Calm | Pace::Wary => mps * CALM_FLIGHT,
            Pace::Startled => mps * STARTLED_FLIGHT,
            Pace::Flat => mps,
        }
    }

    /// Its pace once something more rouses it: hurt, flat out; someone come near (within the
    /// distance it flees at for a calm one crossing, close for a wary one), startled.
    pub fn roused(self, near: bool, hurt: bool) -> Pace {
        match self {
            _ if hurt => Pace::Flat,
            Pace::Calm | Pace::Wary if near => Pace::Startled,
            pace => pace,
        }
    }
}

/// Which clip shows one going so fast on foot, whether its walk (else its run), and how fast
/// it plays: its walk while that keeps up, its run beyond, each as fast as it goes over the
/// clip's own pace (`paces`, walk and run), so its feet keep to the ground.
pub fn on_foot(speed: f32, about: &SpeciesDef) -> (bool, f32) {
    let paces = about.clips.paces;
    let walk_pace = paces.map_or(about.walk_mps, |p| p.0);
    let walks = speed <= walk_pace * WALK_CLIP_UP_TO;
    let pace = paces.map(|p| if walks { p.0 } else { p.1 });
    let rate = pace.map_or(1.0, |pace| (speed / pace.max(0.05)).clamp(0.4, 2.5));
    (walks, rate)
}

/// How often an animal startled ahead of a player coming on dashes across their way to the
/// other side, rather than breaking away on its own side.
pub const CROSSES: f32 = 0.4;
/// It dashes across only with so long a lead on them, metres along their way.
const CROSSES_AHEAD_M: f32 = 6.0;

/// Which way a startled animal makes off from a player, given the way from them to it and
/// their heading (none if they stand): straight away from one standing; from one coming on,
/// at an angle across their way and on ahead of them, as prey breaks from what comes at it
/// rather than running down its path, and now and then, with the lead to make it, across to
/// the other side of their way, as a hare dashes across a track. Shipped open worlds stage
/// their wildlife so (Red Dead Redemption 2's animals breaking across the trail): it crosses
/// the player's sight near, in the open, rather than shrinking straight away.
pub fn bolt(from: Vec3, heading: Vec3, up: Vec3, crosses: bool) -> Vec3 {
    let away = from.normalize_or_zero();
    let speed = heading.length();
    if speed < 0.5 || away == Vec3::ZERO {
        return away;
    }
    let ahead = heading / speed;
    let lead = from.dot(ahead);
    if lead < 0.0 {
        // Behind or beside them as they pass: away.
        return away;
    }
    let beside = from - ahead * lead;
    let mut side = if beside.length() > 0.3 {
        beside.normalize()
    } else {
        ahead.cross(up).normalize_or_zero()
    };
    if crosses && lead > CROSSES_AHEAD_M {
        side = -side;
    }
    (ahead * 0.6 + side * 0.8).normalize_or(away)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_animal_ahead_of_a_player_coming_on_breaks_across_their_way_and_on_ahead() {
        let up = Vec3::Y;
        let heading = Vec3::NEG_Z * 4.0;
        // Ten metres ahead and two to the right of a player running north.
        let from = Vec3::new(2.0, 0.0, -10.0);
        let breaks = bolt(from, heading, up, false);
        assert!(breaks.z < -0.3, "on ahead: {breaks}");
        assert!(
            breaks.x > 0.6,
            "out to its own side, across their way: {breaks}"
        );
        let dashes = bolt(from, heading, up, true);
        assert!(
            dashes.x < -0.6 && dashes.z < -0.3,
            "across to the other side: {dashes}"
        );
        // Too near to make it across, it keeps to its side.
        let near = bolt(Vec3::new(2.0, 0.0, -3.0), heading, up, true);
        assert!(near.x > 0.6, "{near}");
        // From someone standing, or once they have passed it, straight away.
        assert_eq!(bolt(from, Vec3::ZERO, up, true), from.normalize());
        let behind = Vec3::new(2.0, 0.0, 3.0);
        assert_eq!(bolt(behind, heading, up, true), behind.normalize());
    }

    #[test]
    fn a_crossing_walks_until_someone_comes_near_and_only_the_hurt_run_flat_out() {
        let calm = Pace::Calm;
        assert_eq!(calm.roused(false, false), Pace::Calm);
        assert_eq!(calm.roused(true, false), Pace::Startled);
        assert_eq!(calm.roused(false, true), Pace::Flat);
        assert_eq!(Pace::Startled.roused(true, false), Pace::Startled);
        assert!(Pace::Calm.flies_mps(8.0) < Pace::Startled.flies_mps(8.0));
        assert!(Pace::Startled.flies_mps(8.0) < Pace::Flat.flies_mps(8.0));
    }

    #[test]
    fn a_crossing_stag_walks_briskly_and_startled_gallops_short_of_flat_out() {
        let bestiary = crate::play::species::Bestiary::read(&Default::default());
        let stag = bestiary.get(bestiary.named("Red deer stag").unwrap());
        let (walks, rate) = on_foot(Pace::Calm.runs_mps(stag), stag);
        assert!(
            walks && rate > 1.2,
            "a brisk walk, its walk clip quickened: {rate}"
        );
        let startled = Pace::Startled.runs_mps(stag);
        let (walks, rate) = on_foot(startled, stag);
        assert!(
            !walks && rate < 0.9,
            "a gallop slower than its clip's: {rate}"
        );
        assert!(startled < Pace::Flat.runs_mps(stag));
        let (walks, rate) = on_foot(stag.run_mps, stag);
        assert!(
            !walks && (rate - 1.0).abs() < 0.01,
            "flat out, its clip's own: {rate}"
        );
    }
}
