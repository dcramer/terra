//! The duel run in its simplest form (docs/movement.md, "Checked without a person"; D447):
//! two plain bodies on a line, facing each other, driven by scripted presses (never clips)
//! and stepped at 60 Hz through the rules the game runs: the runner and its buffer, a blow's
//! drive and its step in, the blade swept against the hurt capsule, the judge and what a blow
//! does, and what each costs in stamina. It logs every move start and end, contact and
//! outcome with the stamina left and what the contact met as it is heard (`struck`), and a
//! failed check prints the log. The scenarios run on
//! every core (D406). A body's stamina here is only spent, never refilled: a duel is
//! seconds long, and the refill is the walker's (`terra_survival::vitals`).

use crate::blade::{self, Edge};
use crate::contact::{self, Fighting, NEAREST_M};
use crate::moves::{self, Button, MoveSet, Phase, Ran, Taken};
use crate::stamina;
use crate::struck::{Struck, Surface};
use glam::{Quat, Vec3};

const DT: f32 = 1.0 / 60.0;
const UP: Vec3 = Vec3::Y;
/// A stand-in for a clip's sword hand: ahead of the body's middle at chest height, metres,
/// its blade swept right to left through the active frames over this many degrees either
/// side of ahead. With the blade's own length it reaches 1.33 m, as the clips do (1.2 to
/// 1.4 m, `tools/characters/phases.ts`).
const HAND_M: f32 = 0.5;
const CHEST_M: f32 = 1.1;
const ARC_DEG: f32 = 70.0;
/// Two bodies' pushboxes, middle to middle (the walker's, 0.35 m round).
const PUSHBOX_M: f32 = 0.7;
const HEALTH: f32 = 100.0;
/// A rested body's stamina, as the walker's.
const FULL: f32 = 100.0;
/// Where the second body starts: as far as the partner closes to before it guards.
const APART_M: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Press {
    Light,
    Heavy,
    /// A roll toward the other.
    Roll,
    /// The guard raised, and held until lowered.
    Guard,
}

/// When a scripted press comes: on a frame, or once the body is this far (its own seconds)
/// into a move.
#[derive(Clone, Copy, Debug)]
enum When {
    Frame(u32),
    Into(&'static str, f32),
}

struct Body {
    x: f32,
    facing: f32,
    fight: Fighting,
    /// The way its move drives it along the line.
    way: f32,
    was: Option<Edge>,
    guard_held: bool,
    stamina: f32,
    script: Vec<(When, Press)>,
}

#[derive(Clone, Debug, PartialEq)]
enum Event {
    Started(u32, usize, String),
    Ended(u32, usize, String),
    /// A blow reached a body: the frame, by, on, how it was taken, and the attacker's phase.
    Contact(u32, usize, usize, Taken, Phase),
    /// What that blow met, as it is heard.
    Heard(Struck),
}

struct Duel<'a> {
    set: &'a MoveSet,
    bodies: [Body; 2],
    frame: u32,
    events: Vec<Event>,
    log: Vec<String>,
    /// Each body's frames with no move on.
    idle: [Vec<bool>; 2],
}

const WHO: [&str; 2] = ["A", "B"];

impl<'a> Duel<'a> {
    fn new(set: &'a MoveSet, a: Vec<(When, Press)>, b: Vec<(When, Press)>) -> Self {
        let body = |x, facing, script| Body {
            x,
            facing,
            fight: Fighting::new(HEALTH),
            way: facing,
            was: None,
            guard_held: false,
            stamina: FULL,
            script,
        };
        Self {
            set,
            bodies: [body(0.0, 1.0, a), body(APART_M, -1.0, b)],
            frame: 0,
            events: Vec::new(),
            log: Vec::new(),
            idle: [Vec::new(), Vec::new()],
        }
    }

    /// Starts the two bodies with this much stamina.
    fn tired(mut self, a: f32, b: f32) -> Self {
        self.bodies[0].stamina = a;
        self.bodies[1].stamina = b;
        self
    }

    fn run(mut self, frames: u32) -> Self {
        for _ in 0..frames {
            self.step();
        }
        self
    }

    fn feet(&self, i: usize) -> Vec3 {
        Vec3::X * self.bodies[i].x
    }

    fn say(&mut self, line: String) {
        self.log.push(format!("frame {:3}: {line}", self.frame));
    }

    /// The presses due this frame, each once.
    fn due(&mut self, i: usize) -> Vec<Press> {
        let (frame, body) = (self.frame, &mut self.bodies[i]);
        let now = body.fight.runner.name().map(str::to_string);
        let t = body.fight.runner.t();
        let mut due = Vec::new();
        body.script.retain(|&(when, press)| {
            let fire = match when {
                When::Frame(at) => frame >= at,
                When::Into(name, s) => now.as_deref() == Some(name) && t >= s,
            };
            if fire {
                due.push(press);
            }
            !fire
        });
        due
    }

    /// A move started: its way, and its cost paid.
    fn begin(&mut self, i: usize, name: &str) {
        let body = &mut self.bodies[i];
        body.way = body.facing;
        body.fight.guard_s = None;
        body.was = None;
        let cost = self.set.get(name).map_or(0.0, |each| each.stamina);
        body.stamina = (body.stamina - cost).max(0.0);
        let tired = if body.fight.runner.tired {
            " tired"
        } else {
            ""
        };
        let left = body.stamina;
        self.events.push(Event::Started(self.frame, i, name.into()));
        self.say(format!("{} starts {name}{tired} (stamina {left})", WHO[i]));
    }

    fn step(&mut self) {
        let set = self.set;
        for i in [0, 1] {
            let presses = self.due(i);
            let foe = self.bodies[1 - i].x;
            let body = &mut self.bodies[i];
            body.fight.age(DT);
            let rate = body.fight.rate();
            for press in &presses {
                if *press == Press::Guard {
                    body.guard_held = true;
                }
            }
            if body.fight.down_s.is_some() {
                continue;
            }
            // The guard, as the game's: up while held, if not in a move or reeling.
            let free = body.fight.free();
            let guarding = free && body.guard_held && body.fight.runner.name().is_none();
            body.fight.guard_s = guarding.then(|| body.fight.guard_s.map_or(0.0, |s| s + DT));
            let asked = presses.iter().find_map(|press| {
                let button = match press {
                    Press::Roll => Button::Dodge,
                    Press::Heavy => Button::MainHeavy,
                    Press::Light => Button::Main,
                    Press::Guard => return None,
                };
                body.fight.runner.pressed(set, button).map(str::to_string)
            });
            if let Some(name) = asked.filter(|_| free) {
                let left = body.stamina;
                match stamina::ask(&mut body.fight.runner, set, &name, left) {
                    Some(true) => self.begin(i, &name),
                    Some(false) => {}
                    None => self.say(format!("{} too tired to {name} ({left})", WHO[i])),
                }
            }
            let body = &mut self.bodies[i];
            match body.fight.runner.tick(set, DT * rate) {
                Ran::Started(name) => self.begin(i, &name),
                Ran::Ended(name) => {
                    self.events.push(Event::Ended(self.frame, i, name.clone()));
                    self.say(format!("{} ends {name}", WHO[i]));
                }
                Ran::Nothing => {}
            }
            // Its move's drive, stepping in to land through the wind-up, never into the other.
            let body = &mut self.bodies[i];
            let to = foe - body.x;
            let far = to.abs();
            if let Some(each) = body.fight.runner.now(set) {
                let t = body.fight.runner.t();
                let mut speed = each.drive_at(t);
                if let (Some(warp), Some(left)) = (each.warp, each.until_active(t))
                    && far < warp.within
                    && body.way * to > 0.0
                {
                    speed += moves::warp_speed(far, warp.stop_at, left, warp.most_mps);
                }
                if each.damage > 0.0 && far < NEAREST_M && body.way * to > 0.0 {
                    speed = speed.min(0.0);
                }
                body.x += body.way * speed * rate * DT;
            } else if body.fight.shove_s > 0.0 {
                body.x += body.fight.shove.x * DT;
            }
        }
        // Pushboxes: two bodies never stand in each other.
        let [a, b] = &mut self.bodies;
        let gap = b.x - a.x;
        if gap.abs() < PUSHBOX_M {
            let push = (PUSHBOX_M - gap.abs()) * 0.5 * gap.signum();
            a.x -= push;
            b.x += push;
        }
        self.strike();
        for i in 0..2 {
            let idle = self.bodies[i].fight.runner.name().is_none();
            self.idle[i].push(idle);
        }
        self.frame += 1;
    }

    /// Where a body's blade is: held back on its right until its blow's active frames, swept
    /// across its front through them, and on its left after.
    fn edge(&self, i: usize) -> Edge {
        let body = &self.bodies[i];
        let k = body.fight.runner.now(self.set).map_or(0.0, |each| {
            each.active.map_or(0.0, |(from, to)| {
                let clip = each.clip_at(body.fight.runner.t());
                ((clip - from) / (to - from)).clamp(0.0, 1.0)
            })
        });
        let angle = (ARC_DEG * (1.0 - 2.0 * k)).to_radians();
        let dir = Quat::from_axis_angle(UP, angle) * (Vec3::X * body.facing);
        let hand = self.feet(i) + UP * CHEST_M + dir * HAND_M;
        Edge {
            hilt: hand + dir * blade::HILT_M,
            tip: hand + dir * blade::TIP_M,
        }
    }

    /// Each blade in its blow's active frames swept against the other's capsule, as the
    /// game's `contact::strike`, and what each blow that reached it does.
    fn strike(&mut self) {
        let set = self.set;
        let edges = [self.edge(0), self.edge(1)];
        let mut landed = Vec::new();
        for by in [0, 1] {
            let on = 1 - by;
            let attacker = &self.bodies[by].fight;
            let Some(each) = attacker.runner.now(set) else {
                continue;
            };
            let t = attacker.runner.t();
            let open = each.phase(t) == Phase::Active && each.damage > 0.0;
            let Some(was) = self.bodies[by]
                .was
                .filter(|_| open && !attacker.runner.landed)
            else {
                continue;
            };
            let body = &self.bodies[on];
            let capsule = blade::body_at(self.feet(on), UP);
            if blade::sweep(was, edges[by], capsule).is_none() {
                continue;
            }
            let from = (self.feet(by) - self.feet(on)).normalize_or_zero();
            let facing = contact::guards(Vec3::X * body.facing, from);
            let invulnerable = body
                .fight
                .runner
                .now(set)
                .is_some_and(|move_| move_.invulnerable(body.fight.runner.t()));
            let taken = moves::judge(
                invulnerable,
                body.fight.guard_s,
                facing,
                each.heavy,
                set.parry_s,
            );
            let damage = each.damage * attacker.runner.might(set);
            let (taken, charge) = stamina::guarded(set, taken, damage, body.stamina);
            let struck = Struck::of(taken, each, &set.kit, &set.kit, body.fight.body);
            landed.push((
                by,
                taken,
                damage,
                each.hit_stop,
                each.phase(t),
                charge,
                struck,
            ));
        }
        for (body, edge) in self.bodies.iter_mut().zip(edges) {
            body.was = Some(edge);
        }
        for (by, taken, damage, hit_stop, phase, charge, struck) in landed {
            let on = 1 - by;
            let away = (self.feet(on) - self.feet(by)).normalize_or_zero();
            let out = contact::outcome(taken, damage, hit_stop);
            self.bodies[by].fight.runner.landed = true;
            self.bodies[on].stamina -= charge;
            for i in [by, on] {
                self.bodies[i].fight.stop(out.stop_s);
            }
            self.bodies[on].fight.take(out.on, away);
            self.bodies[by].fight.take(out.by, away);
            self.events
                .push(Event::Contact(self.frame, by, on, taken, phase));
            if let Some(struck) = struck {
                self.events.push(Event::Heard(struck));
                self.say(format!("heard {struck:?}"));
            }
            let (health, left) = (self.bodies[on].fight.health, self.bodies[on].stamina);
            let apart = (self.bodies[on].x - self.bodies[by].x).abs();
            self.say(format!(
                "{} {taken:?} by {} ({damage} damage, health {health}, stamina {left}, \
                 {apart:.2} m apart)",
                WHO[on], WHO[by]
            ));
        }
    }

    fn starts(&self, who: usize) -> Vec<(u32, &str)> {
        self.events
            .iter()
            .filter_map(|event| match event {
                Event::Started(frame, i, name) if *i == who => Some((*frame, name.as_str())),
                _ => None,
            })
            .collect()
    }

    fn contacts(&self) -> Vec<(u32, usize, Taken, Phase)> {
        self.events
            .iter()
            .filter_map(|event| match event {
                Event::Contact(frame, by, _, taken, phase) => Some((*frame, *by, *taken, *phase)),
                _ => None,
            })
            .collect()
    }

    /// What each contact met, as it is heard: the surface and whether the blow was heavy.
    fn heard(&self) -> Vec<(Taken, Surface, bool)> {
        self.events
            .iter()
            .filter_map(|event| match event {
                Event::Heard(struck) => Some((struck.taken, struck.on, struck.heavy)),
                _ => None,
            })
            .collect()
    }

    /// The log, for a failed check.
    fn told(&self) -> String {
        self.log.join("\n")
    }
}

fn set() -> MoveSet {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/data/moves/sword.shielded.ron"
    );
    MoveSet::read(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Where a move's cancel window opens, in its own seconds.
fn window(set: &MoveSet, name: &str) -> f32 {
    let each = set.get(name).unwrap();
    each.at_clip(each.cancel.unwrap())
}

/// A slashes at B standing still 2 m off: the blow steps in and lands, full on, in its
/// active frames. The frame it lands on times the parry and the roll.
fn lands(set: &MoveSet) -> u32 {
    let duel = Duel::new(set, vec![(When::Frame(0), Press::Light)], vec![]).run(120);
    let log = duel.told();
    let contacts = duel.contacts();
    assert_eq!(contacts.len(), 1, "one blow, one contact:\n{log}");
    let (frame, by, taken, phase) = contacts[0];
    assert_eq!((by, taken, phase), (0, Taken::Hit, Phase::Active), "{log}");
    assert_eq!(duel.bodies[1].fight.health, HEALTH - 12.0, "{log}");
    // Heard as a light blade on flesh.
    assert_eq!(duel.heard(), [(Taken::Hit, Surface::Flesh, false)], "{log}");
    frame
}

/// A runs the light chain and then the heavy blow, each pressed within its buffer before
/// the window opens, into B's raised guard.
fn chains(set: &MoveSet) {
    let early = set.buffer_s - 0.15;
    let a = vec![
        (When::Frame(0), Press::Light),
        (
            When::Into("slash", window(set, "slash") - early),
            Press::Light,
        ),
        (
            When::Into("backhand", window(set, "backhand") - early),
            Press::Light,
        ),
        (
            When::Into("finisher", window(set, "finisher") - early),
            Press::Heavy,
        ),
    ];
    let duel = Duel::new(set, a, vec![(When::Frame(0), Press::Guard)]).run(360);
    let log = duel.told();
    let starts = duel.starts(0);
    let names: Vec<&str> = starts.iter().map(|(_, name)| *name).collect();
    assert_eq!(names, ["slash", "backhand", "finisher", "heavy"], "{log}");
    // No idle frame from the first blow to the heavy: each started as the last's window
    // opened.
    let (first, last) = (starts[0].0, starts[3].0);
    let idle = (first..=last).find(|&frame| duel.idle[0][frame as usize]);
    assert_eq!(idle, None, "an idle frame inside the chain:\n{log}");
    // Each blow reached the guard in its active frames: the three light ones blocked, the
    // heavy one breaking it.
    let taken: Vec<(Taken, Phase)> = duel
        .contacts()
        .iter()
        .filter(|contact| contact.1 == 0)
        .map(|contact| (contact.2, contact.3))
        .collect();
    let blocked = (Taken::Blocked, Phase::Active);
    let broken = (Taken::GuardBroken, Phase::Active);
    assert_eq!(taken, [blocked, blocked, blocked, broken], "{log}");
    // Heard on the shield's wooden face, the last a heavy blow breaking it.
    let on_face = (Taken::Blocked, Surface::Wood, false);
    let breaks = (Taken::GuardBroken, Surface::Wood, true);
    assert_eq!(duel.heard(), [on_face, on_face, on_face, breaks], "{log}");
    assert!(duel.bodies[1].fight.health < HEALTH, "{log}");
    // Each move paid its cost as it started, and each blow on the guard its damage's worth.
    let cost = |names: &[&str]| -> f32 {
        names
            .iter()
            .map(|name| set.get(name).unwrap().stamina)
            .sum()
    };
    let paid = FULL - duel.bodies[0].stamina;
    assert!((paid - cost(&names)).abs() < 1e-4, "{log}");
    let guarded = set.guard_stamina * (12.0 + 12.0 + 18.0);
    assert!(
        (FULL - duel.bodies[1].stamina - guarded).abs() < 1e-4,
        "{log}"
    );
}

/// Out of stamina (the placeholder, D449): A with 5 left asks for the heavy blow, which does
/// not start, then a light one, which comes out tired, slower and weaker; B with none left
/// cannot roll, and its guard breaks under the tired blow.
fn tires(set: &MoveSet) {
    let a = vec![
        (When::Frame(0), Press::Heavy),
        (When::Frame(5), Press::Light),
    ];
    let b = vec![
        (When::Frame(0), Press::Guard),
        (When::Frame(2), Press::Roll),
    ];
    let duel = Duel::new(set, a, b).tired(5.0, 0.0).run(160);
    let log = duel.told();
    assert_eq!(duel.starts(0), [(5, "slash")], "{log}");
    assert_eq!(duel.starts(1), [], "{log}");
    let contacts = duel.contacts();
    assert_eq!(contacts.len(), 1, "{log}");
    assert_eq!(
        (contacts[0].1, contacts[0].2),
        (0, Taken::GuardBroken),
        "{log}"
    );
    let might = 12.0 * set.tired.might * 0.3;
    assert!(
        (HEALTH - duel.bodies[1].fight.health - might).abs() < 1e-4,
        "{log}"
    );
    let ended = duel.events.iter().find_map(|event| match event {
        Event::Ended(frame, 0, _) => Some(*frame),
        _ => None,
    });
    let slash = set.get("slash").unwrap();
    let tired_frames = (slash.lasts() / set.tired.pace * 60.0) as u32;
    assert!(
        ended.is_some_and(|frame| frame - 5 >= tired_frames),
        "{log}"
    );
    assert_eq!(duel.bodies[0].stamina, 0.0, "{log}");
}

/// B raises its guard 0.1 s before A's slash lands: parried, and A staggers, its next press
/// coming to nothing while it reels.
fn parries(set: &MoveSet, lands_at: u32) {
    let a = vec![
        (When::Frame(0), Press::Light),
        (When::Frame(lands_at + 10), Press::Light),
    ];
    let b = vec![(When::Frame(lands_at - 6), Press::Guard)];
    let duel = Duel::new(set, a, b).run(lands_at + 30);
    let log = duel.told();
    let contacts = duel.contacts();
    assert_eq!(contacts.len(), 1, "{log}");
    assert_eq!((contacts[0].1, contacts[0].2), (0, Taken::Parried), "{log}");
    // Heard on the shield's iron boss: steel ringing on metal, not the wooden face.
    assert_eq!(
        duel.heard(),
        [(Taken::Parried, Surface::Iron, false)],
        "{log}"
    );
    assert_eq!(duel.starts(0).len(), 1, "A acted while staggered:\n{log}");
    assert!(!duel.bodies[0].fight.free(), "{log}");
    assert_eq!(duel.bodies[1].fight.health, HEALTH, "{log}");
}

/// B rolls into A's slash: the blade passes through it in the roll's i-frames.
fn dodges(set: &MoveSet, lands_at: u32) {
    let a = vec![(When::Frame(0), Press::Light)];
    let b = vec![(When::Frame(lands_at - 17), Press::Roll)];
    let duel = Duel::new(set, a, b).run(lands_at + 40);
    let log = duel.told();
    let contacts = duel.contacts();
    assert_eq!(contacts.len(), 1, "{log}");
    assert_eq!((contacts[0].1, contacts[0].2), (0, Taken::Dodged), "{log}");
    assert_eq!(duel.bodies[1].fight.health, HEALTH, "{log}");
    assert_eq!(duel.heard(), [], "a dodged blow meets nothing:\n{log}");
}

#[test]
fn the_duel_chains_lands_parries_dodges_and_tires() {
    let set = set();
    let lands_at = lands(&set);
    let scenarios: [&(dyn Fn() + Sync); 4] = [
        &|| chains(&set),
        &|| tires(&set),
        &|| parries(&set, lands_at),
        &|| dodges(&set, lands_at),
    ];
    terra_gen::par::map(&scenarios, |run| run());
}
