//! The client's clock (D651): which tick it is simulating, kept ahead of the server's by the
//! round trip and a margin, so the intent for a tick reaches the server just before that tick
//! is run. Every state packet says how many ticks early the newest intent came
//! ([`Stamp::margin`]); the client runs a little faster when that is under its target and a
//! little slower when over, at most [`MOST_NUDGE`] either way, as Overwatch and Rocket League
//! do. A margin far from the target is not walked to but jumped, the whole way on the first
//! reading: a welcome that came late (it rides the ordered lane) leaves the clock a third of
//! a second behind, and creeping there would cost seconds of late intents.

use crate::proto::Stamp;

/// The most a client's tick rate is changed to hold its margin: 2%.
pub const MOST_NUDGE: f64 = 0.02;
/// How far ahead of the tick it was welcomed at a client starts: about a 100 ms round trip.
const FIRST_LEAD: u32 = 8;
/// The margin aimed at with no jitter, ticks: the next tick's intent and one spare.
const LEAST_MARGIN: f32 = 2.0;
/// The most the measured jitter adds to the target, ticks.
const MOST_JITTER: f32 = 8.0;
/// A smoothed margin further than this from its target is jumped to, ticks.
const JUMP_PAST: f32 = 4.0;
/// How many of the server's ticks after a jump its stamps are ignored: they tell of intents
/// from before it.
const HOLD_TICKS: u32 = 32;
/// The share a new reading moves the smoothed margin and jitter.
const EASE: f32 = 0.1;
/// The nudge a tick of error asks for.
const NUDGE_A_TICK: f64 = 0.01;

/// A client's tick against its server's.
#[derive(Clone, Debug, Default)]
pub struct Clock {
    /// The next tick to simulate; none until welcomed.
    next: Option<u32>,
    margin: f32,
    jitter: f32,
    /// The newest stamp's tick taken, and the tick stamps are ignored until.
    heard: Option<u32>,
    hold_until: u32,
    /// No margin has been read since the start or the last jump: the next is taken as it
    /// is, not eased toward, so a clock far out jumps the whole way at once.
    fresh: bool,
}

impl Clock {
    /// Welcomed at a tick of the server's.
    pub fn start(&mut self, server_tick: u32) {
        *self = Clock {
            next: Some(server_tick + FIRST_LEAD),
            margin: LEAST_MARGIN,
            jitter: 0.0,
            heard: None,
            hold_until: 0,
            fresh: true,
        };
    }

    /// The margin aimed at: wider on a link whose packets come unevenly.
    pub fn target(&self) -> f32 {
        LEAST_MARGIN + (2.0 * self.jitter).min(MOST_JITTER)
    }

    /// Takes a state packet's stamp. Returns whether the clock jumped: the ticks it gives are
    /// then not the run they were, and what was predicted on the old run is void.
    pub fn hear(&mut self, stamp: Stamp) -> bool {
        let Some(next) = self.next else {
            return false;
        };
        if self.heard.is_some_and(|newest| stamp.tick <= newest) || stamp.tick < self.hold_until {
            return false;
        }
        self.heard = Some(stamp.tick);
        // No intent held yet says nothing of how early they come.
        if stamp.heard.is_none() && stamp.margin == 0 {
            return false;
        }
        let margin = f32::from(stamp.margin);
        if self.fresh {
            self.fresh = false;
            self.margin = margin;
        } else {
            self.jitter += ((margin - self.margin).abs() - self.jitter) * EASE;
            self.margin += (margin - self.margin) * EASE;
        }
        let over = self.margin - self.target();
        if over.abs() <= JUMP_PAST {
            return false;
        }
        self.next = Some(next.saturating_add_signed(-(over as i32)));
        self.margin = self.target();
        self.hold_until = stamp.tick + HOLD_TICKS;
        self.fresh = true;
        true
    }

    /// How fast to tick, as a share of the server's rate: under 1 when intents come earlier
    /// than they need to, over 1 when later.
    pub fn rate(&self) -> f64 {
        let over = f64::from(self.margin - self.target());
        1.0 - (over * NUDGE_A_TICK).clamp(-MOST_NUDGE, MOST_NUDGE)
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

    /// A server a round trip away, as the clock sees it: the client ticks at the clock's
    /// rate, its intents take `up` ticks to arrive, and each stamp tells the margin `down`
    /// ticks later. Returns the margins the server saw over the last second.
    fn held(up: u32, down: u32, seconds: u32) -> Vec<i32> {
        let mut clock = Clock::default();
        clock.start(1000);
        let (mut budget, mut sent, mut stamps, mut margins) =
            (0.0, Vec::new(), Vec::new(), Vec::new());
        for server in 1000..1000 + seconds * 64 {
            budget += clock.rate();
            while budget >= 1.0 {
                budget -= 1.0;
                sent.push((server + up, clock.advance().expect("started")));
            }
            let newest = sent
                .iter()
                .filter(|(arrives, _)| *arrives <= server)
                .map(|(_, tick)| *tick)
                .max();
            let margin = newest.map_or(0, |newest| {
                (i64::from(newest) - i64::from(server)).clamp(-128, 127) as i8
            });
            stamps.push((
                server + down,
                Stamp {
                    tick: server,
                    heard: newest,
                    margin,
                },
            ));
            for (_, stamp) in stamps.iter().filter(|(arrives, _)| *arrives == server) {
                clock.hear(*stamp);
            }
            if server >= 1000 + (seconds - 1) * 64 {
                margins.push(i32::from(margin));
            }
        }
        margins
    }

    #[test]
    fn the_margin_is_held_near_two_ticks_on_any_round_trip() {
        // 16 ms, 100 ms, 300 ms and 600 ms there and back.
        for (up, down) in [(1, 0), (3, 3), (10, 9), (19, 19)] {
            let margins = held(up, down, 20);
            assert!(
                margins.iter().all(|margin| (1..=3).contains(margin)),
                "{up}+{down} ticks: {margins:?}"
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
                heard: Some(tick + 3),
                margin: 5,
            });
            assert!((clock.rate() - 1.0).abs() <= MOST_NUDGE + 1e-12);
        }
        assert!(clock.rate() < 1.0, "early intents slow the clock");
    }
}
