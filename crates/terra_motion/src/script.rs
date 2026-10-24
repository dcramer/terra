//! A fight script (docs/movement.md, "Checked without a person"): what a scripted fighter
//! means to do, as intents a player's keys give, never clips. Each step comes on a time or
//! on what the fight is doing, as a player times it: so long after the last step, once its
//! own move's cancel window is near (the next of a chain asked inside the set's buffer), once
//! the other's next blow is so long from its active frames (a guard raised before it, a
//! parry, a roll through it), or once it is free. Plain, so the game's scripted duel
//! (`--duel`, `terra_creatures::melee`) and a test step it alike.
//!
//! A script's file is RON: `(steps: [(name: "…", when: After(0.5), press: [Light]), …])`;
//! `assets/data/duels/` holds them.

use crate::contact::Fighting;
use crate::moves::MoveSet;
use serde::Deserialize;

/// What a fighter means to do, as a player's keys say it.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Intent {
    /// Draw the sword, or sheathe it.
    Draw,
    /// Lock on to the nearest foe, or let go.
    Lock,
    /// The next blow of the light chain.
    Light,
    /// The heavy blow.
    Heavy,
    Roll,
    /// The guard, while held.
    Guard,
    Forward,
    Back,
    Left,
    Right,
}

/// When a step comes.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum When {
    /// So many seconds after the last step.
    After(f32),
    /// Once its own move, started since the last step, is this many seconds (its own time)
    /// from its cancel window.
    Chain(f32),
    /// Once the other's next blow, started since the last step, is this many seconds (its
    /// own time) from its active frames; at once if its wind-up is shorter.
    Blow(f32),
    /// Once it is free: in no move, not reeling, not down.
    Free,
}

/// One step: its name, when it comes, and the intents it gives, pressed, or held so long.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Step {
    pub name: String,
    pub when: When,
    pub press: Vec<Intent>,
    /// Seconds held; a press is held for one tick.
    #[serde(default)]
    pub hold: f32,
}

#[derive(Deserialize)]
struct File {
    steps: Vec<Step>,
}

/// A move as a script watches it: its name and how far into it.
type Seen = Option<(String, f32)>;

/// A script being run: the next step, the time since the last, what is held, and whether
/// each body has started a move since the last step.
#[derive(Clone, Debug)]
pub struct Script {
    pub steps: Vec<Step>,
    next: usize,
    since_s: f32,
    holding: Vec<(Intent, f32)>,
    was: [Seen; 2],
    started: [bool; 2],
}

/// This tick's intents: each held, and whether it was pressed on this tick, as a key is;
/// and the step that came on it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tick {
    pub intents: Vec<(Intent, bool)>,
    pub step: Option<String>,
}

impl Script {
    pub fn new(steps: Vec<Step>) -> Self {
        Self {
            steps,
            next: 0,
            since_s: 0.0,
            holding: Vec::new(),
            was: [None, None],
            started: [false, false],
        }
    }

    pub fn read(text: &str) -> Result<Self, String> {
        let file: File = ron::from_str(text).map_err(|why| why.to_string())?;
        Ok(Self::new(file.steps))
    }

    /// Whether every step has come and nothing is held.
    pub fn done(&self) -> bool {
        self.next >= self.steps.len() && self.holding.is_empty()
    }

    /// One tick of `dt` seconds, with what the scripted body (`me`) and the other are doing:
    /// the intents it gives on it.
    pub fn tick(
        &mut self,
        set: &MoveSet,
        me: &Fighting,
        other: Option<&Fighting>,
        dt: f32,
    ) -> Tick {
        for (i, body) in [Some(me), other].into_iter().enumerate() {
            let now =
                body.and_then(|body| Some((body.runner.name()?.to_string(), body.runner.t())));
            let began = match (&self.was[i], &now) {
                (_, None) => false,
                (None, Some(_)) => true,
                (Some((was, was_t)), Some((name, t))) => was != name || t < was_t,
            };
            self.started[i] |= began;
            self.was[i] = now;
        }
        // Held for its seconds, to a millisecond either way of the ticks' sum.
        for (_, left) in &mut self.holding {
            *left -= dt;
        }
        self.holding.retain(|(_, left)| *left > 1e-3);
        let mut intents: Vec<(Intent, bool)> = self
            .holding
            .iter()
            .map(|(intent, _)| (*intent, false))
            .collect();
        self.since_s += dt;
        let mut fired = None;
        if let Some(step) = self.steps.get(self.next)
            && self.due(step.when, set, me, other)
        {
            for &intent in &step.press {
                intents.retain(|(held, _)| *held != intent);
                intents.push((intent, true));
                self.holding.retain(|(held, _)| *held != intent);
                if step.hold > dt {
                    self.holding.push((intent, step.hold));
                }
            }
            fired = Some(step.name.clone());
            self.next += 1;
            self.since_s = 0.0;
            self.started = [false, false];
        }
        Tick {
            intents,
            step: fired,
        }
    }

    fn due(&self, when: When, set: &MoveSet, me: &Fighting, other: Option<&Fighting>) -> bool {
        match when {
            When::After(s) => self.since_s >= s,
            When::Free => me.free() && me.runner.name().is_none(),
            When::Chain(lead) => {
                self.started[0]
                    && me.runner.now(set).is_some_and(|now| {
                        now.cancel
                            .is_some_and(|cancel| me.runner.t() >= now.at_clip(cancel) - lead)
                    })
            }
            When::Blow(before) => {
                self.started[1]
                    && other.is_some_and(|other| {
                        other.runner.now(set).is_some_and(|now| {
                            now.damage > 0.0
                                && now
                                    .until_active(other.runner.t())
                                    .is_none_or(|left| left <= before)
                        })
                    })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moves::Ran;

    const DT: f32 = 1.0 / 60.0;

    fn set() -> MoveSet {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/data/moves/sword_and_shield.ron"
        ))
        .unwrap();
        MoveSet::read(&text).unwrap()
    }

    /// Runs a script against a body that starts each move it is asked for, and another that
    /// starts `blow` on a tick: the ticks each step came on, and what each gave.
    /// A step that came: its tick, its name and what it gave.
    type Came = (u32, String, Vec<(Intent, bool)>);

    fn run(text: &str, blow: Option<(&str, u32)>, ticks: u32) -> Vec<Came> {
        let set = set();
        let mut script = Script::read(text).unwrap();
        let mut me = Fighting::new(100.0);
        let mut other = Fighting::new(100.0);
        let mut came = Vec::new();
        for tick in 0..ticks {
            if let Some((name, at)) = blow
                && tick == at
            {
                other.runner.start(name);
            }
            let given = script.tick(&set, &me, Some(&other), DT);
            for &(intent, pressed) in &given.intents {
                if pressed && intent == Intent::Light {
                    let next = me.runner.chained(&set, "slash").to_string();
                    me.runner.ask(&set, &next);
                }
            }
            if let Some(step) = given.step {
                came.push((tick, step, given.intents.clone()));
            }
            let _: Ran = me.runner.tick(&set, DT);
            other.runner.tick(&set, DT);
        }
        came
    }

    #[test]
    fn steps_come_after_their_time_inside_the_buffer_and_before_a_blow_lands() {
        let set = set();
        let script = "(steps: [
            (name: \"first\", when: After(0.5), press: [Light]),
            (name: \"second\", when: Chain(0.1), press: [Light]),
            (name: \"guard\", when: Blow(0.3), press: [Guard], hold: 0.5),
        ])";
        let came = run(script, Some(("partner_slash", 100)), 200);
        let ticks: Vec<u32> = came.iter().map(|(tick, ..)| *tick).collect();
        // Half a second in, a press.
        assert_eq!(came[0].1, "first");
        assert!((29..=30).contains(&ticks[0]), "{came:?}");
        // The slash's cancel window, its own seconds, less the lead: asked inside the buffer.
        let slash = set.get("slash").unwrap();
        let cancel = slash.at_clip(slash.cancel.unwrap());
        let chained = (ticks[1] - ticks[0]) as f32 * DT;
        assert!(
            chained >= cancel - 0.1 - DT && chained <= cancel - 0.1 + 2.0 * DT,
            "chain asked {chained:.3} s in, its window {cancel:.3} s"
        );
        assert!(cancel - chained < set.buffer_s);
        // The guard 0.3 s of the blow's own time before its active frames.
        let blow = set.get("partner_slash").unwrap();
        let active = blow.at_clip(blow.active.unwrap().0);
        let before = active - (ticks[2] - 100) as f32 * DT;
        assert!(
            (before - 0.3).abs() <= 2.0 * DT,
            "guard {before:.3} s before"
        );
    }

    #[test]
    fn a_hold_is_pressed_once_then_held_for_its_time() {
        let script = "(steps: [(name: \"guard\", when: After(0.0), press: [Guard], hold: 0.25)])";
        let set = set();
        let mut script = Script::read(script).unwrap();
        let me = Fighting::new(100.0);
        let ticks: Vec<Tick> = (0..30).map(|_| script.tick(&set, &me, None, DT)).collect();
        assert_eq!(ticks[0].intents, [(Intent::Guard, true)]);
        assert_eq!(ticks[1].intents, [(Intent::Guard, false)]);
        let held = ticks.iter().filter(|tick| !tick.intents.is_empty()).count();
        assert_eq!(held, 15, "a quarter of a second at 60 a second");
        assert!(script.done());
    }

    #[test]
    fn a_blow_step_waits_for_a_blow_started_after_the_last_step() {
        // The other's blow started before the lock: the parry waits for the next blow.
        let script = "(steps: [
            (name: \"lock\", when: After(0.2), press: [Lock]),
            (name: \"parry\", when: Blow(0.1), press: [Guard]),
        ])";
        let came = run(script, Some(("partner_slash", 0)), 100);
        assert_eq!(came.len(), 1, "{came:?}");
    }
}
