//! Moves as data and the move runner (docs/movement.md, "Moves as data" and the actions
//! layer): which move a body is in and how far, its phases (wind-up, active, recovery), its
//! cancel window, a press buffered into that window, its i-frames, and how a blow that meets
//! a guard is judged (blocked, parried), and a body too tired for a move's full cost
//! (`stamina`) running it slower and weaker.

use crate::struck::Kit;
use serde::Deserialize;
use std::collections::HashMap;

/// A move, as its set's file says it (`assets/data/moves/*.ron`, which explains each field).
/// Times are the clip's own seconds.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Move {
    pub clip: String,
    #[serde(default)]
    pub from: f32,
    #[serde(default = "one")]
    pub speed: f32,
    #[serde(default)]
    pub active: Option<(f32, f32)>,
    pub done: f32,
    #[serde(default)]
    pub cancel: Option<f32>,
    #[serde(default)]
    pub next: Option<String>,
    /// The same blow with its shield held out from its first frame, mixed into `clip` when
    /// this blow cuts short one still carrying its shield out (its `carry`), by how far out
    /// that one has it, so the shield goes on from where it is (D454).
    #[serde(default)]
    pub chained: Option<String>,
    /// How this move's clip carries its shield out: eased out over `in` and back over `out`.
    #[serde(default)]
    pub carry: Option<Carry>,
    #[serde(default)]
    pub damage: f32,
    #[serde(default)]
    pub hit_stop: f32,
    #[serde(default)]
    pub heavy: bool,
    #[serde(default)]
    pub drive: Vec<(f32, f32)>,
    #[serde(default)]
    pub warp: Option<Warp>,
    #[serde(default)]
    pub invulnerable: Option<(f32, f32)>,
    #[serde(default)]
    pub hold: Option<f32>,
    #[serde(default)]
    pub stamina: f32,
}

fn one() -> f32 {
    1.0
}

/// How a clip carries its shield out (its hold in art/animation/sets/fight.json, D454): eased
/// out over `in` and back over `out`, clip seconds, each by a smoothstep.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Carry {
    #[serde(rename = "in")]
    pub into: (f32, f32),
    pub out: (f32, f32),
}

impl Carry {
    /// How far out the shield is at a time in the clip: 0 at the guard, 1 carried.
    pub fn at(&self, clip: f32) -> f32 {
        let (a, b) = self.into;
        let (c, d) = self.out;
        terra_gen::smoothstep(a, b, clip) * (1.0 - terra_gen::smoothstep(c, d, clip))
    }
}

/// The clip a move plays, and the clip mixed into it with its share of the weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Takes<'a> {
    pub clip: &'a str,
    pub mix: Option<(&'a str, f32)>,
}

/// How a blow closes on a target during its wind-up.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Warp {
    pub within: f32,
    pub cone: f32,
    pub stop_at: f32,
    pub most_mps: f32,
}

/// Where a move is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    WindUp,
    Active,
    Recovery,
    Done,
}

impl Move {
    /// The clip's time this far (seconds of the body's own time) into the move.
    pub fn clip_at(&self, t: f32) -> f32 {
        self.from + t * self.speed
    }

    /// How long the move lasts, in the body's own seconds.
    pub fn lasts(&self) -> f32 {
        (self.done - self.from).max(0.0) / self.speed.max(1e-3)
    }

    /// The move's own seconds at a time in its clip.
    pub fn at_clip(&self, clip: f32) -> f32 {
        (clip - self.from) / self.speed.max(1e-3)
    }

    pub fn phase(&self, t: f32) -> Phase {
        let clip = self.clip_at(t);
        if clip >= self.done {
            return Phase::Done;
        }
        match self.active {
            Some((from, _)) if clip < from => Phase::WindUp,
            Some((_, to)) if clip <= to => Phase::Active,
            Some(_) => Phase::Recovery,
            None => Phase::Recovery,
        }
    }

    /// Whether another move may cut it short now.
    pub fn cancels(&self, t: f32) -> bool {
        self.cancel.is_some_and(|from| self.clip_at(t) >= from)
    }

    /// Whether blows pass through the body now.
    pub fn invulnerable(&self, t: f32) -> bool {
        let clip = self.clip_at(t);
        self.invulnerable
            .is_some_and(|(from, to)| clip >= from && clip <= to)
    }

    /// Seconds of the body's own time until its blade can strike, if it is still winding up.
    pub fn until_active(&self, t: f32) -> Option<f32> {
        let (from, _) = self.active?;
        let left = self.at_clip(from) - t;
        (left > 0.0).then_some(left)
    }

    /// Whether a body so far off (feet to feet, metres) and so many degrees off ahead is one
    /// the blow is made to land on: inside its step in's cone and no further than it steps
    /// in to (`warp`), so a blow with it in its active frames that meets nothing has missed
    /// (the duel run's check). A move without a step in reaches nothing.
    pub fn in_reach(&self, distance: f32, off_deg: f32) -> bool {
        self.damage > 0.0
            && self
                .warp
                .is_some_and(|warp| off_deg <= warp.cone && distance <= warp.stop_at)
    }

    /// The body's speed ahead, metres a second, from the move's drive.
    pub fn drive_at(&self, t: f32) -> f32 {
        let clip = self.clip_at(t);
        let points = &self.drive;
        match points.iter().position(|&(at, _)| at > clip) {
            None => points.last().map_or(0.0, |&(_, speed)| speed),
            Some(0) => points[0].1,
            Some(i) => {
                let (a, va) = points[i - 1];
                let (b, vb) = points[i];
                va + (vb - va) * ((clip - a) / (b - a).max(1e-6))
            }
        }
    }
}

/// A body's moves, and the windows every move of the set keeps.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MoveSet {
    pub parry_s: f32,
    pub buffer_s: f32,
    /// Stamina a blow taken on the guard costs, for each point of its damage.
    pub guard_stamina: f32,
    /// How a light blow comes out with too little stamina for it (D449's placeholder).
    pub tired: Tired,
    /// What the set strikes and guards with, as a blow meets it.
    pub kit: Kit,
    /// The clip a body stands on guard in between moves, the pose its blows are keyed to at
    /// both ends (D450).
    pub guard: String,
    /// What a blow that lands shows, in place of a long hit-stop (D456).
    pub landed: Landed,
    pub moves: HashMap<String, Move>,
}

/// What a blow that lands shows (D456, D487): a `burst` where the blade met the body (the
/// struck body keeps its own colours), and the player's view kicked by `kick_deg`, easing back
/// to rest over `kick_s`. Presentation only: the fight's rules never read it.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Landed {
    pub burst: Burst,
    pub kick_deg: f32,
    pub kick_s: f32,
}

/// What shows where a landed blade met the body (D487): a white-hot glow `glow_m` across for
/// `glow_s`, and `sparks` flung on along the blade's travel at `spark_mps`, each gone after
/// `spark_s`. A glow 0 across and no sparks show nothing.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Burst {
    pub glow_s: f32,
    pub glow_m: f32,
    pub sparks: u32,
    pub spark_mps: f32,
    pub spark_s: f32,
}

/// A blow thrown too tired: its own time runs at `pace`, and it does `might` of its damage.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Tired {
    pub pace: f32,
    pub might: f32,
}

impl MoveSet {
    pub fn read(text: &str) -> Result<Self, String> {
        let set: MoveSet = ron::from_str(text).map_err(|why| why.to_string())?;
        for (name, each) in &set.moves {
            if let Some(next) = &each.next
                && !set.moves.contains_key(next)
            {
                return Err(format!(
                    "{name} chains into {next}, which is not in the set"
                ));
            }
            if each.done <= each.from || each.speed <= 0.0 {
                return Err(format!("{name} is over before it starts"));
            }
            if each.stamina < 0.0 {
                return Err(format!("{name} gives stamina back"));
            }
        }
        let tired = set.tired;
        if !(tired.pace > 0.0 && tired.pace <= 1.0 && (0.0..=1.0).contains(&tired.might)) {
            return Err("a tired blow must be no faster or harder than a rested one".into());
        }
        Ok(set)
    }

    pub fn get(&self, name: &str) -> Option<&Move> {
        self.moves.get(name)
    }
}

/// What the runner did on a tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ran {
    Nothing,
    Started(String),
    Ended(String),
}

/// The move a body is in, how far into it in the body's own time, and a press waiting for
/// its cancel window (with its age, and whether it was asked too tired).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Runner {
    pub current: Option<(String, f32)>,
    /// The move the current one cut short, and how far into it, as it started.
    pub was: Option<(String, f32)>,
    buffered: Option<(String, f32, bool)>,
    /// Whether the current move's blow has landed, so it lands once.
    pub landed: bool,
    /// Whether the current move was started too tired for its full cost: slower, weaker.
    pub tired: bool,
}

impl Runner {
    pub fn name(&self) -> Option<&str> {
        self.current.as_ref().map(|(name, _)| name.as_str())
    }

    pub fn t(&self) -> f32 {
        self.current.as_ref().map_or(0.0, |(_, t)| *t)
    }

    pub fn now<'a>(&self, set: &'a MoveSet) -> Option<&'a Move> {
        set.get(self.name()?)
    }

    /// What the current move plays: its own clip, keyed from the guard, with its `chained`
    /// take mixed in by how far out the move it cut short still carried its shield, so the
    /// shield goes on from where it was rather than back to the guard and out again (D454): a
    /// layer's weight carried across the change, as a layered blend per bone keeps its own.
    /// All the chained take when that one had its shield all the way out.
    pub fn takes<'a>(&self, set: &'a MoveSet) -> Option<Takes<'a>> {
        let now = self.now(set)?;
        let out = self.was.as_ref().map_or(0.0, |(name, t)| {
            set.get(name)
                .and_then(|was| Some(was.carry?.at(was.clip_at(*t))))
                .unwrap_or(0.0)
        });
        Some(match &now.chained {
            Some(chained) if out >= 0.999 => Takes {
                clip: chained,
                mix: None,
            },
            Some(chained) if out > 0.001 => Takes {
                clip: &now.clip,
                mix: Some((chained, out)),
            },
            _ => Takes {
                clip: &now.clip,
                mix: None,
            },
        })
    }

    /// Asks for a move: it starts now if the body is free or its move may be cut short,
    /// or waits for the window if it opens soon enough. Says whether it started.
    pub fn ask(&mut self, set: &MoveSet, name: &str) -> bool {
        self.ask_as(set, name, false)
    }

    /// Asks for a move, rested or too tired for its full cost (`stamina::ask`).
    pub fn ask_as(&mut self, set: &MoveSet, name: &str, tired: bool) -> bool {
        if set.get(name).is_none() {
            return false;
        }
        match self.now(set) {
            Some(now) if !now.cancels(self.t()) => {
                self.buffered = Some((name.to_string(), 0.0, tired));
                false
            }
            _ => {
                self.start_as(name, tired);
                true
            }
        }
    }

    /// Starts a move at once, whatever the body was doing: a reaction, or a forced stop.
    pub fn start(&mut self, name: &str) {
        self.start_as(name, false);
    }

    /// Starts a move at once, rested or too tired.
    pub fn start_as(&mut self, name: &str, tired: bool) {
        self.was = self.current.replace((name.to_string(), 0.0));
        self.buffered = None;
        self.landed = false;
        self.tired = tired;
    }

    /// How fast the current move's own time runs against the body's: slower when tired.
    pub fn pace(&self, set: &MoveSet) -> f32 {
        if self.tired { set.tired.pace } else { 1.0 }
    }

    /// The share of its damage the current move does: less when tired.
    pub fn might(&self, set: &MoveSet) -> f32 {
        if self.tired { set.tired.might } else { 1.0 }
    }

    /// Ends whatever move there is, and any press waiting.
    pub fn clear(&mut self) {
        self.current = None;
        self.was = None;
        self.buffered = None;
    }

    /// The light blow a press asks for: the next of a chain while a chain's blow is on,
    /// otherwise the first.
    pub fn chained<'a>(&'a self, set: &'a MoveSet, first: &'a str) -> &'a str {
        let waiting = self.buffered.as_ref().map(|(name, ..)| name.as_str());
        let after = waiting.or(self.name());
        after
            .and_then(|name| set.get(name))
            .and_then(|each| each.next.as_deref())
            .unwrap_or(first)
    }

    /// Moves the body's own time on: a waiting press starts when its window opens, or is
    /// dropped once stale; a move ends when it is done. A tired move's own time runs slower.
    pub fn tick(&mut self, set: &MoveSet, dt: f32) -> Ran {
        let pace = self.pace(set);
        if let Some((_, age, _)) = &mut self.buffered {
            *age += dt;
            if *age > set.buffer_s {
                self.buffered = None;
            }
        }
        let Some((name, t)) = &mut self.current else {
            return Ran::Nothing;
        };
        *t += dt * pace;
        let Some(now) = set.get(name) else {
            self.current = None;
            return Ran::Nothing;
        };
        if let Some((next, _, tired)) = &self.buffered
            && now.cancels(*t)
        {
            let (next, tired) = (next.clone(), *tired);
            self.start_as(&next, tired);
            return Ran::Started(next);
        }
        if *t >= now.lasts() {
            let ended = name.clone();
            self.current = None;
            return Ran::Ended(ended);
        }
        Ran::Nothing
    }
}

/// How a blow that reached a body was taken.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Taken {
    /// Full on the body.
    Hit,
    /// On the guard: no harm, both pushed apart.
    Blocked,
    /// On a guard raised just before it: the attacker staggers.
    Parried,
    /// A heavy blow through a guard: the guard is broken.
    GuardBroken,
    /// Through a roll's i-frames.
    Dodged,
}

/// Judges a blow reaching a body: rolling clear of it, its guard up (and for how long, in
/// seconds) facing it, and whether the blow is heavy.
pub fn judge(
    invulnerable: bool,
    guard_held_s: Option<f32>,
    facing: bool,
    heavy: bool,
    parry_s: f32,
) -> Taken {
    if invulnerable {
        return Taken::Dodged;
    }
    match guard_held_s {
        Some(held) if facing && held <= parry_s => Taken::Parried,
        Some(_) if facing && heavy => Taken::GuardBroken,
        Some(_) if facing => Taken::Blocked,
        _ => Taken::Hit,
    }
}

/// How fast a blow steps its body toward a target in its wind-up so that it stands
/// `stop_at` from it as the blade arrives: never back, never faster than `most_mps`.
pub fn warp_speed(distance: f32, stop_at: f32, until_active: f32, most_mps: f32) -> f32 {
    ((distance - stop_at) / until_active.max(0.05)).clamp(0.0, most_mps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set() -> MoveSet {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/data/moves/sword_and_shield.ron"
        );
        MoveSet::read(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn a_blow_reaches_what_stands_in_its_step_ins_cone_and_reach_and_a_roll_nothing() {
        let set = set();
        let slash = set.get("slash").unwrap();
        let warp = slash.warp.unwrap();
        assert!(slash.in_reach(warp.stop_at, 0.0));
        assert!(slash.in_reach(1.0, warp.cone - 1.0));
        assert!(!slash.in_reach(warp.stop_at + 0.05, 0.0), "too far");
        assert!(!slash.in_reach(1.0, warp.cone + 1.0), "beside it");
        assert!(!set.get("roll").unwrap().in_reach(0.5, 0.0), "no blade");
    }

    #[test]
    fn the_shipped_set_reads_and_every_blow_has_a_wind_up_before_its_blade_strikes() {
        let set = set();
        for (name, each) in &set.moves {
            if let Some((from, to)) = each.active {
                assert!(each.from < from && from < to && to < each.done, "{name}");
                assert_eq!(each.phase(0.0), Phase::WindUp, "{name}");
            }
        }
    }

    #[test]
    fn a_move_goes_through_its_phases_at_its_clip_speed() {
        let slash = Move {
            clip: "Slash".into(),
            from: 0.2,
            speed: 2.0,
            active: Some((0.6, 0.8)),
            done: 1.4,
            cancel: Some(1.0),
            next: None,
            chained: None,
            carry: None,
            damage: 0.0,
            hit_stop: 0.0,
            heavy: false,
            drive: vec![(0.2, 0.0), (0.6, 2.0), (1.0, 0.0)],
            warp: None,
            invulnerable: None,
            hold: None,
            stamina: 0.0,
        };
        // Clip seconds 0.2 + 2t: active from t = 0.2 to 0.3, over at 0.6.
        assert!((slash.lasts() - 0.6).abs() < 1e-6);
        assert_eq!(slash.phase(0.1), Phase::WindUp);
        assert_eq!(slash.phase(0.25), Phase::Active);
        assert_eq!(slash.phase(0.35), Phase::Recovery);
        assert_eq!(slash.phase(0.61), Phase::Done);
        assert!(!slash.cancels(0.35) && slash.cancels(0.41));
        assert!((slash.until_active(0.05).unwrap() - 0.15).abs() < 1e-6);
        assert_eq!(slash.until_active(0.25), None);
        // Its drive rises to 2 m/s at clip 0.6 (t = 0.2), half that at t = 0.1.
        assert!((slash.drive_at(0.1) - 1.0).abs() < 1e-5);
        assert!((slash.drive_at(0.2) - 2.0).abs() < 1e-5);
        assert_eq!(slash.drive_at(0.6), 0.0);
    }

    #[test]
    fn a_roll_is_untouchable_only_between_its_tuck_and_its_get_up() {
        let set = set();
        let roll = set.get("roll").unwrap();
        let (from, to) = roll.invulnerable.unwrap();
        assert!(!roll.invulnerable(0.0));
        assert!(roll.invulnerable(roll.at_clip(from) + 0.01));
        assert!(roll.invulnerable(roll.at_clip(to) - 0.01));
        assert!(!roll.invulnerable(roll.at_clip(to) + 0.01));
        assert!(!roll.invulnerable(roll.lasts()));
        // The i-frames are a third of a second or more: rolling through a blow is a matter
        // of timing, not luck.
        assert!(roll.at_clip(to) - roll.at_clip(from) > 0.3);
    }

    #[test]
    fn a_guard_raised_inside_the_window_parries_and_one_raised_earlier_blocks() {
        let parry = 0.18;
        assert_eq!(judge(false, Some(0.05), true, false, parry), Taken::Parried);
        assert_eq!(judge(false, Some(0.18), true, false, parry), Taken::Parried);
        assert_eq!(judge(false, Some(0.19), true, false, parry), Taken::Blocked);
        // A heavy blow on a guard held a while breaks it; a parry still stops it.
        assert_eq!(
            judge(false, Some(0.5), true, true, parry),
            Taken::GuardBroken
        );
        assert_eq!(judge(false, Some(0.1), true, true, parry), Taken::Parried);
        // A guard facing away stops nothing; no guard is a hit; a roll's i-frames win.
        assert_eq!(judge(false, Some(0.05), false, false, parry), Taken::Hit);
        assert_eq!(judge(false, None, true, false, parry), Taken::Hit);
        assert_eq!(judge(true, None, false, true, parry), Taken::Dodged);
    }

    #[test]
    fn a_blow_chained_while_the_last_still_carries_its_shield_mixes_in_its_chained_take() {
        // A two-blow chain read from a set: the first carries its shield out over clip 0-0.5
        // and back over 1.2-1.6 (its window opens at 1.0, it is done at 2.0); the second has
        // a chained take.
        let set = MoveSet::read(
            r#"#![enable(implicit_some)]
            (parry_s: 0.2, buffer_s: 0.3, guard_stamina: 1.0, tired: (pace: 0.7, might: 0.5),
                kit: (blade: Steel, face: Wood, boss: Iron), guard: "Guard",
                landed: (burst: (glow_s: 0.0, glow_m: 0.0, sparks: 0, spark_mps: 0.0, spark_s: 0.0),
                         kick_deg: 0.0, kick_s: 0.0),
                moves: {
                    "first": (clip: "First", done: 2.0, cancel: 1.0, next: "second",
                              carry: (in: (0.0, 0.5), out: (1.2, 1.6))),
                    "second": (clip: "Second", done: 2.0, chained: "Second_Held"),
                })"#,
        )
        .unwrap();
        let played = |press: Option<f32>| {
            let mut runner = Runner::default();
            if let Some(press) = press {
                runner.start("first");
                runner.tick(&set, press);
            }
            runner.ask(&set, "second");
            let takes = runner.takes(&set).unwrap();
            (
                takes.clip.to_string(),
                takes.mix.map(|(c, w)| (c.to_string(), w)),
            )
        };
        // At the window's opening, the shield all the way out: the held take alone.
        assert_eq!(played(Some(1.0)), ("Second_Held".into(), None));
        // Halfway back (the middle of the out ease): half and half.
        let (clip, mix) = played(Some(1.4));
        let (held, share) = mix.unwrap();
        assert_eq!((clip.as_str(), held.as_str()), ("Second", "Second_Held"));
        assert!((share - 0.5).abs() < 1e-4, "{share}");
        // A quarter of the way back, a smoothstep's 0.15625 in: 0.84375 out.
        let (_, mix) = played(Some(1.3));
        assert!((mix.unwrap().1 - 0.84375).abs() < 1e-4);
        // Back at the guard, from the guard, or after the first is over: its own.
        assert_eq!(played(Some(1.7)), ("Second".into(), None));
        assert_eq!(played(None), ("Second".into(), None));
        assert_eq!(played(Some(2.1)), ("Second".into(), None));
    }

    #[test]
    fn a_press_in_a_blows_wind_up_waits_and_starts_the_next_of_the_chain_in_its_window() {
        let set = set();
        let mut runner = Runner::default();
        assert!(runner.ask(&set, runner.chained(&set, "slash").to_string().as_str()));
        assert_eq!(runner.name(), Some("slash"));
        // Pressed again late in the wind-up, a frame before the strike, within the buffer of
        // the slash's window: the backhand waits for it.
        let slash = set.get("slash").unwrap();
        let press = slash.at_clip(slash.active.unwrap().0) - 1.0 / 60.0;
        runner.tick(&set, press);
        assert_eq!(runner.now(&set).unwrap().phase(runner.t()), Phase::WindUp);
        let next = runner.chained(&set, "slash").to_string();
        assert_eq!(next, "backhand");
        assert!(!runner.ask(&set, &next));
        assert_eq!(runner.name(), Some("slash"));
        let window = slash.at_clip(slash.cancel.unwrap());
        let mut started = None;
        let mut t = press;
        while started.is_none() && t < 2.0 {
            t += 1.0 / 60.0;
            if let Ran::Started(name) = runner.tick(&set, 1.0 / 60.0) {
                started = Some((name, t));
            }
        }
        let (name, at) = started.unwrap();
        assert_eq!(name, "backhand");
        // In its window, never before it, and within a frame of it opening; the buffer
        // keeps the press that long.
        assert!(
            at + 1e-4 >= window && at < window + 1.0 / 30.0,
            "{at} {window}"
        );
    }

    #[test]
    fn a_stale_press_is_dropped_and_a_move_left_alone_ends() {
        let set = set();
        let mut runner = Runner::default();
        runner.start("heavy");
        assert!(!runner.ask(&set, "slash"));
        // Longer than the buffer keeps it, and the heavy's window has not opened.
        runner.tick(&set, set.buffer_s + 0.05);
        let heavy = set.get("heavy").unwrap();
        let mut ended = false;
        for _ in 0..400 {
            match runner.tick(&set, 1.0 / 60.0) {
                Ran::Started(name) => panic!("{name} started from a stale press"),
                Ran::Ended(name) => {
                    assert_eq!(name, "heavy");
                    ended = true;
                    break;
                }
                Ran::Nothing => {}
            }
        }
        assert!(ended && runner.name().is_none());
        assert!(heavy.lasts() > 1.0);
        // Free again: the chain starts from its first blow.
        assert_eq!(runner.chained(&set, "slash"), "slash");
    }

    #[test]
    fn a_blow_steps_in_to_stand_where_its_blade_lands_and_no_further() {
        // 3 m off, 0.5 s to go, stopping 1.5 m away: 3 m/s.
        assert!((warp_speed(3.0, 1.5, 0.5, 4.0) - 3.0).abs() < 1e-6);
        assert_eq!(warp_speed(1.2, 1.5, 0.5, 4.0), 0.0);
        assert_eq!(warp_speed(9.0, 1.5, 0.5, 4.0), 4.0);
    }
}
