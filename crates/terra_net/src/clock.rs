//! The client's clock (D651): which tick it is simulating, kept ahead of the server's by the
//! round trip and a margin, so the intent for a tick reaches the server before that tick is
//! run. Every state packet says how many ticks early the newest intent came
//! ([`Stamp::margin`]); the client runs a little faster when that is under its target and a
//! little slower when over, at most [`MOST_NUDGE`] either way, as Overwatch and Rocket League
//! do. A margin far from the target is not walked to but jumped, the whole way on the first
//! reading: a welcome that came late (it rides the ordered lane) leaves the clock a third of
//! a second behind, and creeping there would cost seconds of late intents.
//!
//! What is held to the target is the lowest margin of the last second, not the usual one. A
//! game drawing slowly runs its ticks in bursts, several a frame, and so does a server; the
//! intent that matters is the last of a burst, which comes latest. Holding the usual margin
//! (a mean, widened by a measure of jitter) left those late on a loaded machine, each a
//! correction; holding the lowest covers whatever the bursts are, with nothing to tune.

use crate::proto::Stamp;

/// The most a client's tick rate is changed to hold its margin: 2%.
pub const MOST_NUDGE: f64 = 0.02;
/// How far ahead of the tick it was welcomed at a client starts: about a 100 ms round trip.
const FIRST_LEAD: u32 = 8;
/// The lowest margin aimed at, ticks: the next tick's intent, one spare, and two more: a client
/// sends every second tick, so when a packet is lost the intents it carried come with the
/// next, two ticks later, and must still be in time.
const LEAST_MARGIN: i32 = 4;
/// A lowest margin further than this from its target is jumped to, ticks.
const JUMP_PAST: i32 = 4;
/// The longest a jump's hold lasts, in the server's ticks, should the stamp that ends it
/// never come: four seconds.
const MOST_HOLD: u32 = 256;
/// The margins kept to take the lowest of: a second of stamps at 32 a second.
const KEPT: usize = 32;
/// How many margins must be kept before being early is believed enough to jump back: half a
/// second.
const SURE: usize = 16;
/// How many readings running must be far late before the clock jumps ahead: an eighth of a
/// second of them.
const LATE_RUN: u32 = 4;
/// The nudge a tick of error asks for.
const NUDGE_A_TICK: f64 = 0.01;

/// A client's tick against its server's.
#[derive(Clone, Debug, Default)]
pub struct Clock {
    /// The next tick to simulate; none until welcomed.
    next: Option<u32>,
    /// The last margins read, newest overwriting oldest, and how many are held.
    margins: [i8; KEPT],
    held: usize,
    at: usize,
    /// No margin has been read since the start or the last jump: the first is believed at
    /// once, early or late, so a clock far out is put right on its first reading.
    fresh: bool,
    /// How many readings running have been far late.
    late: u32,
    /// The newest stamp's tick taken.
    heard: Option<u32>,
    /// After a jump, stamps are not believed until the server holds an intent of the new
    /// run (one stamped this or later): until then they tell of the old run, and acting on
    /// them corrects twice. Whatever the round trip, that is exactly long enough. The tick
    /// is the end of the hold should that stamp never come.
    hold: Option<(u32, u32)>,
}

impl Clock {
    /// Welcomed at a tick of the server's.
    pub fn start(&mut self, server_tick: u32) {
        *self = Clock {
            next: Some(server_tick + FIRST_LEAD),
            fresh: true,
            ..Clock::default()
        };
    }

    /// The lowest margin of the last second, once any has been read.
    pub fn lowest(&self) -> Option<i32> {
        self.margins[..self.held]
            .iter()
            .min()
            .map(|&low| i32::from(low))
    }

    /// How far the lowest margin is over its target, ticks.
    fn over(&self) -> i32 {
        self.lowest().map_or(0, |low| low - LEAST_MARGIN)
    }

    /// Takes a state packet's stamp. Returns whether the clock jumped: the ticks it gives are
    /// then not the run they were, and what was predicted on the old run is void.
    pub fn hear(&mut self, stamp: Stamp) -> bool {
        let Some(next) = self.next else {
            return false;
        };
        if self.heard.is_some_and(|newest| stamp.tick <= newest) {
            return false;
        }
        self.heard = Some(stamp.tick);
        if let Some((need, until)) = self.hold {
            if stamp.heard.is_none_or(|heard| heard < need) && stamp.tick < until {
                return false;
            }
            self.hold = None;
        }
        // No intent held yet says nothing of how early they come.
        if stamp.heard.is_none() && stamp.margin == 0 {
            return false;
        }
        self.margins[self.at] = stamp.margin;
        self.at = (self.at + 1) % KEPT;
        self.held = (self.held + 1).min(KEPT);
        // A jump voids what was predicted, a dozen ticks of it, so it is for a clock truly
        // out: the first reading after a start or a jump; readings late several times
        // running (one late reading is a lost packet, which the nudge answers); or the
        // lowest of half a second still early.
        let reading = i32::from(stamp.margin) - LEAST_MARGIN;
        self.late = if reading < -JUMP_PAST {
            self.late + 1
        } else {
            0
        };
        let lowest = self.over();
        let over = if self.fresh && reading.abs() > JUMP_PAST || self.late >= LATE_RUN {
            reading
        } else if lowest > JUMP_PAST && self.held >= SURE {
            lowest
        } else {
            0
        };
        self.fresh = false;
        if over == 0 {
            return false;
        }
        self.late = 0;
        let jumped = next.saturating_add_signed(-over);
        self.next = Some(jumped);
        (self.held, self.at, self.fresh) = (0, 0, true);
        self.hold = Some((next.max(jumped), stamp.tick + MOST_HOLD));
        true
    }

    /// How fast to tick, as a share of the server's rate: under 1 when even the latest intents
    /// come earlier than they need to, over 1 when they come too near their tick.
    pub fn rate(&self) -> f64 {
        1.0 - (f64::from(self.over()) * NUDGE_A_TICK).clamp(-MOST_NUDGE, MOST_NUDGE)
    }

    /// The tick to simulate now; the next call gives the one after. None until welcomed.
    pub fn advance(&mut self) -> Option<u32> {
        let tick = self.next?;
        self.next = Some(tick + 1);
        Some(tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A server a round trip away, as the clock sees it: the client's ticks are run at the
    /// clock's rate but only every `burst` ticks, all at once (a game drawing slowly), its
    /// intents take `up` ticks to arrive, and each stamp, sent every second tick, tells the
    /// margin `down` ticks later. Returns, over the last two seconds, each tick's margin when
    /// the server came to run it: under 0 is an intent that was not there in time.
    fn held(up: u32, down: u32, burst: u32, seconds: u32) -> Vec<i32> {
        let mut clock = Clock::default();
        clock.start(1000);
        let (mut budget, mut sent, mut stamps, mut margins) =
            (0.0, Vec::new(), Vec::new(), Vec::new());
        for server in 1000..1000 + seconds * 64 {
            budget += clock.rate();
            if server % burst == 0 {
                while budget >= 1.0 {
                    budget -= 1.0;
                    sent.push((server + up, clock.advance().expect("started")));
                }
            }
            let arrived = sent.iter().filter(|(arrives, _)| *arrives <= server);
            let newest = arrived.map(|(_, tick)| *tick).max();
            let margin = newest.map_or(0, |newest| {
                (i64::from(newest) - i64::from(server)).clamp(-128, 127) as i8
            });
            if server % 2 == 0 {
                stamps.push((
                    server + down,
                    Stamp {
                        tick: server,
                        heard: newest,
                        margin,
                    },
                ));
            }
            for (_, stamp) in stamps.iter().filter(|(arrives, _)| *arrives == server) {
                clock.hear(*stamp);
            }
            if server >= 1000 + (seconds - 2) * 64 {
                margins.push(i32::from(margin));
            }
        }
        margins
    }

    #[test]
    fn the_margin_is_held_near_four_ticks_on_any_round_trip() {
        // 16 ms, 100 ms, 300 ms and 600 ms there and back, a tick at a time.
        for (up, down) in [(1, 0), (3, 3), (10, 9), (19, 19)] {
            let margins = held(up, down, 1, 20);
            assert!(
                margins.iter().all(|margin| (3..=6).contains(margin)),
                "{up}+{down} ticks: {margins:?}"
            );
        }
    }

    /// A game drawing at 21, 16 and 11 frames a second runs three, four and six ticks a frame.
    /// The last intent of each burst is still there in time, and no more margin is kept than
    /// the burst needs.
    #[test]
    fn no_intent_is_late_however_many_ticks_a_frame_carries() {
        for burst in [3, 4, 6] {
            let margins = held(3, 3, burst, 30);
            let (low, high) = (margins.iter().min().unwrap(), margins.iter().max().unwrap());
            assert!(
                *low >= 0,
                "{burst} ticks a frame: an intent {low} ticks late"
            );
            assert!(
                *high <= burst as i32 + 8,
                "{burst} ticks a frame: {high} ticks early at most"
            );
        }
    }

    /// The planted fault (D632): a clock that never hears is never corrected, and on a long
    /// round trip its intents come late; the same check must fail it.
    #[test]
    fn a_clock_that_hears_nothing_runs_late_on_a_long_round_trip() {
        let mut clock = Clock::default();
        clock.start(1000);
        let late = (1000..1640u32).filter(|server| {
            let tick = clock.advance().expect("started");
            // Sent at `server`, arriving 19 ticks on: late if its tick has been run by then.
            tick < server + 19
        });
        assert_eq!(late.count(), 640);
    }

    #[test]
    fn the_rate_is_never_changed_by_more_than_two_percent() {
        let mut clock = Clock::default();
        clock.start(0);
        for tick in 40..400 {
            clock.hear(Stamp {
                tick,
                heard: Some(tick + 5),
                margin: 5,
            });
            assert!((clock.rate() - 1.0).abs() <= MOST_NUDGE + 1e-12);
        }
        assert!(clock.rate() < 1.0, "early intents slow the clock");
    }
}
