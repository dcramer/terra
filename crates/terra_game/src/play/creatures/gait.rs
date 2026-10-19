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

/// A calm crossing would walk so many times its walking speed, a brisk walk, as brisk as its
/// walk clip keeps up with (`keeps_to`), and no slower than so many metres a second, or its run if that is slower: a
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
/// A gait's clip plays from so much to so much of its own rate: past that its legs blur or
/// crawl. Shipped games keep a clip so (Unreal's speed-matched blend spaces and stride
/// warping, Red Dead Redemption 2's animals): its rate follows the ground speed over the
/// stride it was made with, and past its band the creature changes gait, walk to run, or
/// goes no faster, rather than quickening its legs. A big animal's clips have long, slow
/// strides, so a stag crossing calmly walks with a stag's beat (forest walk review: "the legs
/// are moving too quickly vs. more majestically slower movement of limbs").
pub const RATE: (f32, f32) = (0.8, 1.2);
/// Below its slowest gait's band it slows its walk so, coming to a stop or setting off.
const SLOWEST_RATE: f32 = 0.4;

impl Pace {
    /// How fast one of a kind goes on foot at this pace, metres a second.
    pub fn runs_mps(self, about: &SpeciesDef) -> f32 {
        let wanted = match self {
            Pace::Calm => (about.walk_mps * CALM_WALK).max(about.run_mps.min(CALM_LEAST_MPS)),
            Pace::Wary => about.walk_mps * WARY_WALK,
            Pace::Startled => about.run_mps * STARTLED_RUN,
            Pace::Flat => about.run_mps,
        }
        .min(about.run_mps);
        keeps_to(wanted, about)
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

/// Its gaits on foot, slowest first: the ground speed each one's clip covers at its own rate
/// (`paces`), walk then run. A trot or a canter would be one more clip and pace here.
fn gaits(about: &SpeciesDef) -> Option<[f32; 2]> {
    about
        .clips
        .paces
        .map(|(walk, run)| [walk.max(0.05), run.max(0.05)])
}

/// How fast one of a kind goes on foot when it would go `wanted`: as fast as that while some
/// gait keeps up within its rates (`RATE`), or below its walk's; between one gait's band and
/// the next, the nearer band's edge, the slower on a tie; past its fastest gait's band, that
/// band's top. A kind with no paces measured goes as asked.
pub fn keeps_to(wanted: f32, about: &SpeciesDef) -> f32 {
    let Some(gaits) = gaits(about) else {
        return wanted;
    };
    let mut best = f32::NAN;
    for (i, pace) in gaits.into_iter().enumerate() {
        let least = if i == 0 { 0.0 } else { pace * RATE.0 };
        let fits = wanted.clamp(least, pace * RATE.1);
        if best.is_nan() || (fits - wanted).abs() < (best - wanted).abs() {
            best = fits;
        }
    }
    best
}

/// Which clip shows one going so fast on foot, whether its walk (else its run), and how fast
/// it plays: the slowest gait that keeps up within its rates, as fast as it goes over that
/// clip's own pace, so its feet keep to the ground; a speed come to by `keeps_to` always
/// plays within `RATE`.
pub fn on_foot(speed: f32, about: &SpeciesDef) -> (bool, f32) {
    let Some([walk, run]) = gaits(about) else {
        return (speed <= about.walk_mps * RATE.1, 1.0);
    };
    let walks = speed <= walk * RATE.1;
    let pace = if walks { walk } else { run };
    (walks, (speed / pace).clamp(SLOWEST_RATE, RATE.1))
}

/// How often an animal startled ahead of a player coming on dashes across their way to the
/// other side, rather than breaking away on its own side: seldom, since one dashing across
/// ends beside the runner's way ahead of him (lake film r9).
pub const CROSSES: f32 = 0.15;
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
    fn a_crossing_stag_walks_with_a_stags_beat_and_startled_gallops_short_of_flat_out() {
        let bestiary = crate::play::species::Bestiary::read(&Default::default());
        let stag = bestiary.get(bestiary.named("Red deer stag").unwrap());
        let calm = Pace::Calm.runs_mps(stag);
        let (walks, rate) = on_foot(calm, stag);
        assert!(
            walks && rate > 1.0 && rate <= RATE.1 + 1e-4,
            "a brisk walk, its legs no quicker than its clip allows: {rate}"
        );
        assert!(calm > stag.walk_mps, "brisker than its stroll: {calm}");
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

    #[test]
    fn no_creature_quickens_a_clip_past_its_rate_and_its_feet_keep_to_the_ground() {
        let bestiary = crate::play::species::Bestiary::read(&Default::default());
        let paces = [Pace::Calm, Pace::Wary, Pace::Startled, Pace::Flat];
        let mut checked = 0;
        for kind in bestiary.all() {
            let about = bestiary.get(kind);
            let Some((walk, run)) = about.clips.paces else {
                continue;
            };
            let speeds = paces
                .iter()
                .map(|pace| pace.runs_mps(about))
                .chain([about.walk_mps, about.run_mps].map(|mps| keeps_to(mps, about)));
            for speed in speeds {
                let (walks, rate) = on_foot(speed, about);
                let pace = if walks { walk } else { run };
                assert!(
                    rate <= RATE.1 + 1e-4,
                    "{}: {speed} m/s plays its clip at {rate}",
                    about.name
                );
                // Stride length times cadence is its speed: the clip's own pace times its rate.
                assert!(
                    (pace * rate - speed).abs() < 1e-3 * speed.max(1.0),
                    "{}: its feet skate at {speed} m/s, its clip covering {}",
                    about.name,
                    pace * rate
                );
                if !walks {
                    assert!(
                        rate >= RATE.0 - 1e-4,
                        "{}: a crawling run, {rate}",
                        about.name
                    );
                }
                checked += 1;
            }
        }
        assert!(checked > 40, "the bestiary's paced kinds: {checked}");
    }
}
