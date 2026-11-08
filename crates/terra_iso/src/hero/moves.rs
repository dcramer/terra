//! The hero's moves and motion: the sword set's rules (terra_motion::moves' `Runner`, the
//! game's own: the light chain through each blow's cancel window, the heavy, the block held at
//! its hold, the roll's invulnerable tuck), his blows landing on what stands in their arc as
//! their clip's active window opens, and his body moved: eased toward the speed his intent
//! wishes, or driven ahead by the move (its `drive`), kept to the layout's ground.

use super::{Ability, HeroIntent, Kit};
use crate::play::{Fallen, Ground, Hero, facing, footprint, in_arc, step_to, strike};
use crate::ruin::fight::Pose;
use bevy::prelude::*;
use terra_motion::gaits::{JOG, SPRINT, WALK};
use terra_motion::moves::{Button, Ran, Runner};

/// How fast he gains speed and sheds it (m/s²): the brake D633's.
const ACCEL: f32 = 9.0;
const BRAKE: f32 = 17.0;
/// How fast he turns toward his way (per second), as play's.
const TURN: f32 = 14.0;

/// What the hero is doing: the move under way (the runner), his speed along his heading, the
/// way a move faces, a hit-stop's time left, and how many moves have started (so the clips
/// see a move start again by the same name).
#[derive(Component, Default, Debug)]
pub struct Doing {
    pub runner: Runner,
    pub speed: f32,
    pub heading: Vec2,
    face: Vec2,
    /// The foe a blow under way closes on (`lunge`), the nearest in reach as it was pressed.
    target: Option<Entity>,
    /// The way a pressed move waiting on the current one's window will face, and its foe.
    queued: Option<(Vec2, Option<Entity>)>,
    halt: f32,
    pub started: u32,
}

impl Doing {
    /// Holds his move for `s` seconds more, a hit-stop's (play/feel.rs); a longer one already
    /// under way stands.
    pub fn hold(&mut self, s: f32) {
        self.halt = self.halt.max(s);
    }

    /// Whether blows pass through him now (a roll's tuck).
    pub fn invulnerable(&self, kit_set: &terra_motion::moves::MoveSet) -> bool {
        self.runner
            .now(kit_set)
            .is_some_and(|m| m.invulnerable(self.runner.t()))
    }

    /// Whether his shield is up now.
    pub fn blocking(&self, kit_set: &terra_motion::moves::MoveSet) -> bool {
        self.runner.now(kit_set).is_some_and(|m| m.hold.is_some())
    }
}

/// The speed wished: none, a walk, a jog or a sprint.
pub fn wished(intent: &HeroIntent) -> f32 {
    if intent.way == Vec2::ZERO {
        0.0
    } else if intent.walk {
        WALK
    } else if intent.sprint {
        SPRINT
    } else {
        JOG
    }
}

/// A scripted walk's arrival (m, a route's `--goto`): he stops once this near its point.
pub const ARRIVE: f32 = 0.3;

/// The way a scripted walk asks from `here` to `to` at `speed` (a route's `--goto`, as an
/// ARPG's click-to-move): the unit way to it, or none once he is within `ARRIVE` of it or
/// near enough to coast there on his brakes (`BRAKE`), so he stops on it, not past it.
pub fn toward(here: Vec2, to: Vec2, speed: f32) -> Vec2 {
    let off = to - here;
    let d = off.length();
    if d <= ARRIVE || d <= speed * speed / (2.0 * BRAKE) {
        Vec2::ZERO
    } else {
        off / d
    }
}

/// `speed` eased toward `wish` over `dt`: gained at `ACCEL`, shed at `BRAKE`.
pub fn ease(speed: f32, wish: f32, dt: f32) -> f32 {
    if wish > speed {
        (speed + ACCEL * dt).min(wish)
    } else {
        (speed - BRAKE * dt).max(wish)
    }
}

fn button(a: Ability) -> Option<Button> {
    match a {
        Ability::Light => Some(Button::Main),
        Ability::Heavy => Some(Button::MainHeavy),
        Ability::Block => Some(Button::Off),
        Ability::Roll => Some(Button::Dodge),
        Ability::Walk | Ability::Sprint => None,
    }
}

/// Runs his moves, lands his blows and moves him.
pub fn act(
    mut commands: Commands,
    time: Res<Time>,
    kit: Res<Kit>,
    (ground, nav): (Option<Res<Ground>>, Option<Res<crate::nav::Nav>>),
    mut hero: Query<(&mut Transform, &HeroIntent, &mut Doing), With<Hero>>,
    (foes, footprints): (
        Query<(Entity, &Transform), (With<Pose>, Without<Hero>, Without<Fallen>)>,
        Query<(&footprint::Footprint, &GlobalTransform), footprint::Blocking>,
    ),
    mut was_blocking: Local<bool>,
) {
    let (Some(ground), Ok((mut tr, intent, mut doing))) = (ground, hero.single_mut()) else {
        return;
    };
    let set = &kit.set;
    let dt = time.delta_secs();
    let here = tr.translation.xz();
    let ahead = (tr.rotation * Vec3::Z).xz().normalize_or(Vec2::Y);
    // A press, or the block's button going down: asked of the set by its button.
    let block_down = intent.block && !*was_blocking;
    *was_blocking = intent.block;
    if let Some((ability, at)) = asked(intent, block_down)
        && let Some(b) = button(ability)
        && let Some(name) = doing.runner.pressed(set, b).map(str::to_string)
    {
        let target = nearest(here, foes.iter().map(|(e, t)| (e, t.translation.xz())));
        let face = face_for(ability, at, intent.way, (here, ahead), target.map(|t| t.1));
        let foe = target.map(|t| t.0);
        if doing.runner.ask(set, &name) {
            begin(&mut doing, &mut tr, (face, foe));
        } else {
            doing.queued = Some((face, foe));
        }
    }
    // The block held at its hold while its button stays down; the hit-stop holds the move.
    let mut own = dt;
    if doing.halt > 0.0 {
        doing.halt -= dt;
        own = 0.0;
    }
    if let Some(m) = doing.runner.now(set)
        && let Some(hold) = m.hold
        && intent.block
        && m.clip_at(doing.runner.t()) >= hold
    {
        let t = m.at_clip(hold);
        if let Some((_, at)) = &mut doing.runner.current {
            *at = t;
        }
        own = 0.0;
    }
    if let Ran::Started(_) = doing.runner.tick(set, own) {
        let face = doing.queued.take().unwrap_or((ahead, None));
        begin(&mut doing, &mut tr, face);
    }
    // His blow, once, as its clip's active window opens: what stands in its arc falls back.
    if let Some(m) = doing.runner.now(set)
        && let Some((from, _)) = m.active
        && !doing.runner.landed
        && m.clip_at(doing.runner.t()) >= from
    {
        doing.runner.landed = true;
        let t = m.clip_at(doing.runner.t());
        let mut bit = false;
        for (foe, ftr) in &foes {
            if in_arc(here, doing.face, ftr.translation.xz()) {
                commands.entity(foe).insert(strike::Blow {
                    from: here,
                    t,
                    weight: if m.heavy { 1.0 } else { 0.0 },
                });
                bit = true;
            }
        }
        if bit {
            doing.halt = m.hit_stop;
        }
    }
    // The motion: the move's drive along its facing (a blow's only to its foe, `lunge`), else
    // his own pace along his heading.
    let v = match doing.runner.now(set) {
        Some(m) => {
            doing.speed = 0.0;
            let drive = m.drive_at(doing.runner.t()) * if own > 0.0 { 1.0 } else { 0.0 };
            if m.active.is_some() && dt > 0.0 {
                let to = doing.target.and_then(|e| foes.get(e).ok());
                let to = to.map(|(_, t)| t.translation.xz());
                let landed = doing.runner.landed;
                doing.face * lunge(here, doing.face, to, landed, drive * dt) / dt
            } else {
                doing.face * drive
            }
        }
        None => {
            if intent.way != Vec2::ZERO {
                doing.heading = intent.way;
            }
            doing.speed = ease(doing.speed, wished(intent), dt);
            if doing.speed > 0.0 && doing.heading != Vec2::ZERO {
                let k = 1.0 - libm::expf(-TURN * dt);
                tr.rotation = tr.rotation.slerp(facing(doing.heading), k);
            }
            doing.heading * doing.speed
        }
    };
    if v == Vec2::ZERO {
        return;
    }
    let circles = footprint::world(&footprints);
    match step_to(&ground, nav.as_deref(), &circles, here, v * dt) {
        Some(to) => tr.translation = to.extend((ground.floor)(to)).xzy(),
        None => doing.speed = 0.0,
    }
}

/// The nearest of `foes` (each a key and its place) within a blow's reach of `here`, if any.
pub fn nearest<K>(here: Vec2, foes: impl Iterator<Item = (K, Vec2)>) -> Option<(K, Vec2)> {
    foes.map(|(k, p)| (p.distance(here), (k, p)))
        .filter(|(d, _)| *d <= crate::play::REACH)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, f)| f)
}

/// How near a blow's lunge closes on its foe (m, centre to centre): a pack's body (0.65 to
/// 0.8 m, forest.ron) and his (0.3) apart by about a blade's length, inside the ring a pack
/// stands on to swing (demo/steer.rs `Mob::stand`, about 2 m).
pub const STRIKE: f32 = 1.6;
/// What of a blow's drive he takes with no foe in reach: a step into the swing, not a lunge.
pub const ALONE: f32 = 0.15;

/// How far a blow's `step` of drive (m) carries him along `face` from `here`: toward its foe at
/// `to` until `STRIKE` of it along his way and no further, as ARPG melee lunges close to striking
/// distance (Diablo's, Lost Ark's); `ALONE` of it with none; nothing once it has `landed`, so a
/// foe knocked back (play/knock.rs) is not followed.
pub fn lunge(here: Vec2, face: Vec2, to: Option<Vec2>, landed: bool, step: f32) -> f32 {
    match to {
        _ if landed => 0.0,
        Some(p) => step.min(((p - here).dot(face) - STRIKE).max(0.0)),
        None => step * ALONE,
    }
}

/// The move asked of the set this frame: a press, else the shield as its button goes down,
/// each with its aim point (a script's `--do block:X,Z`).
fn asked(intent: &HeroIntent, block_down: bool) -> Option<(Ability, Option<Vec2>)> {
    intent
        .press
        .or(block_down.then_some((Ability::Block, intent.block_at)))
}

/// The way a move asked faces as it starts: a move aimed at a point toward it (a roll too); a
/// roll unaimed along the way held (else ahead); a blow or the shield with no aim point toward the nearest foe
/// in reach, `target` (an action RPG's soft target), so a script's or pad's press neither
/// swings at the air nor raises the shield to the side the blow does not come from; else ahead.
fn face_for(
    ability: Ability,
    at: Option<Vec2>,
    way: Vec2,
    (here, ahead): (Vec2, Vec2),
    target: Option<Vec2>,
) -> Vec2 {
    match (ability, at) {
        (_, Some(p)) => (p - here).normalize_or(ahead),
        (Ability::Roll, None) => way.normalize_or(ahead),
        (Ability::Light | Ability::Heavy | Ability::Block, None) => {
            target.map_or(ahead, |p| (p - here).normalize_or(ahead))
        }
        _ => ahead,
    }
}

/// A move starts: he turns to face its way at once.
fn begin(doing: &mut Doing, tr: &mut Transform, (face, target): (Vec2, Option<Entity>)) {
    doing.face = face;
    doing.target = target;
    doing.heading = face;
    doing.started += 1;
    doing.halt = 0.0;
    tr.rotation = facing(face);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// He gains speed at the accel and sheds it faster, never past what is wished; standing
    /// is wished with no way, a walk, jog or sprint by what is held.
    /// An unaimed blow's target: the nearest foe within reach, none past it.
    #[test]
    fn an_unaimed_blow_turns_to_the_nearest_foe_in_reach() {
        let here = Vec2::new(1.0, 1.0);
        let foes = [
            Vec2::new(3.5, 1.0),
            Vec2::new(1.0, -0.5),
            Vec2::new(9.0, 9.0),
        ];
        let near = nearest(here, foes.into_iter().enumerate());
        assert_eq!(near, Some((1, Vec2::new(1.0, -0.5))));
        assert_eq!(nearest(here, [((), Vec2::new(4.5, 1.0))].into_iter()), None);
        assert_eq!(nearest(here, std::iter::empty::<((), Vec2)>()), None);
    }

    /// Every blow's lunge closes on its foe until `STRIKE` of it and no further, nor after it
    /// once the blow lands and knocks it back (1 m over 0.3 s, play/knock.rs's heavy); with its
    /// foe already in striking distance he stands; with none in reach he takes a step, not a
    /// lunge. On the drive alone (j33's) each blow fails, driving on in striking distance and
    /// at the air; without the `landed` stop the heavy follows its knocked foe.
    #[test]
    fn a_blow_lunges_to_striking_distance_and_no_further() {
        let set = terra_motion::moves::MoveSet::read(include_str!(
            "../../../../assets/data/moves/sword.shielded.ron"
        ))
        .unwrap();
        let dt = 1.0 / 60.0;
        // He and his foe `start` m ahead over the move, knocked `knock` m from the blow on:
        // where he ends, how near it he came, and the move's whole drive.
        let travel = |name: &str, start: Option<f32>, knock: f32| {
            let m = set.get(name).unwrap();
            let hits = m.at_clip(m.active.unwrap().0);
            let (mut at, mut t, mut closest, mut full) = (Vec2::ZERO, 0.0, f32::INFINITY, 0.0);
            while t < m.lasts() {
                let slid = knock * ((t - hits) / 0.3).clamp(0.0, 1.0);
                let foe = start.map(|d| Vec2::X * (d + slid));
                let step = m.drive_at(t) * dt;
                full += step;
                at += Vec2::X * lunge(at, Vec2::X, foe, t >= hits, step);
                if let Some(f) = foe {
                    closest = closest.min(f.x - at.x);
                }
                t += dt;
            }
            (at.x, closest, full)
        };
        for name in ["slash", "backhand", "finisher", "heavy"] {
            let (went, closest, full) = travel(name, Some(2.4), 0.0);
            assert!(
                went <= 2.4 - STRIKE + 1e-3 && closest >= STRIKE - 1e-3 && went > 0.1,
                "{name}: from 2.4 m he went {went:.2} of its {full:.2} m, closest {closest:.2}"
            );
            let (knocked, ..) = travel(name, Some(2.4), 1.0);
            assert!(
                (knocked - went).abs() < 1e-4,
                "{name}: he followed a foe knocked back, {knocked:.2} m against {went:.2}"
            );
            let (went, ..) = travel(name, Some(STRIKE - 0.1), 0.0);
            assert!(went < 1e-3, "{name}: in striking reach he went {went:.2} m");
            let (went, _, full) = travel(name, None, 0.0);
            assert!(
                went > 0.0 && went <= 0.35 && went < full * 0.2,
                "{name}: at the air he went {went:.2} of its {full:.2} m"
            );
        }
    }

    /// An unaimed shield turns to the nearest foe in reach as a blow does; a roll keeps to the
    /// way held, an aimed move to its point, and with no foe in reach he keeps his facing.
    #[test]
    fn an_unaimed_block_faces_the_nearest_foe() {
        let (here, ahead) = (Vec2::ZERO, Vec2::X);
        let foe = Some(Vec2::new(0.0, 2.0));
        for a in [Ability::Block, Ability::Light, Ability::Heavy] {
            assert_eq!(
                face_for(a, None, Vec2::ZERO, (here, ahead), foe),
                Vec2::Y,
                "{a:?}"
            );
            assert_eq!(
                face_for(a, None, Vec2::ZERO, (here, ahead), None),
                ahead,
                "{a:?}"
            );
        }
        let aimed = Some(Vec2::new(0.0, -3.0));
        assert_eq!(
            face_for(Ability::Block, aimed, Vec2::ZERO, (here, ahead), foe),
            Vec2::NEG_Y
        );
        assert_eq!(
            face_for(Ability::Roll, None, Vec2::NEG_X, (here, ahead), foe),
            Vec2::NEG_X
        );
    }

    /// A script's aim point is used: `--do roll:X,Z` rolls toward it, not along the way held,
    /// and `--do block:X,Z` raises the shield toward it, not toward the nearest foe, from the
    /// frame its button goes down while it is held.
    #[test]
    fn a_scripted_aim_point_is_used() {
        let (here, ahead) = (Vec2::ZERO, Vec2::X);
        let foe = Some(Vec2::new(0.0, 2.0));
        let point = Vec2::new(-3.0, 0.0);
        let mut i = HeroIntent::default();
        i.ask(Ability::Roll, Some(point));
        let (a, at) = asked(&i, false).expect("the roll pressed");
        assert_eq!(
            face_for(a, at, Vec2::Y, (here, ahead), foe),
            Vec2::NEG_X,
            "a roll aimed at (-3, 0) goes toward it"
        );
        let mut i = HeroIntent::default();
        i.ask(Ability::Block, Some(point));
        assert_eq!(
            asked(&i, false),
            None,
            "held on, the shield is not asked again"
        );
        let (a, at) = asked(&i, true).expect("the shield's button goes down");
        assert_eq!(
            face_for(a, at, Vec2::ZERO, (here, ahead), foe),
            Vec2::NEG_X,
            "a block aimed at (-3, 0) faces it, not the foe at (0, 2)"
        );
    }

    #[test]
    fn speed_eases_toward_the_wish() {
        assert!((ease(0.0, JOG, 0.1) - 0.9).abs() < 1e-5);
        assert_eq!(ease(4.4, JOG, 0.1), JOG);
        assert!((ease(JOG, 0.0, 0.1) - (JOG - 1.7)).abs() < 1e-5);
        assert_eq!(ease(0.5, 0.0, 0.1), 0.0);
        let mut i = HeroIntent::default();
        assert_eq!(wished(&i), 0.0);
        i.way = Vec2::X;
        assert_eq!(wished(&i), JOG);
        i.sprint = true;
        assert_eq!(wished(&i), SPRINT);
        i.walk = true;
        assert_eq!(wished(&i), WALK);
    }

    /// A roll aimed past a mob's body, its line inside the two bodies' reach, slides round it
    /// (play.rs `step_to`): at each step he stays outside its circle and his, and he covers
    /// most of the ground the same roll covers on open ground, not stopped at its bark
    /// (iso-forest-demo j20: the forest's foes are solid circles of 0.65 to 0.8 m).
    #[test]
    fn a_roll_slides_round_a_body() {
        let set = terra_motion::moves::MoveSet::read(include_str!(
            "../../../../assets/data/moves/sword.shielded.ron"
        ))
        .unwrap();
        let roll = set.get("roll").unwrap();
        let ground = Ground {
            floor: Box::new(|_| 0.0),
            open: Box::new(|_| true),
        };
        let dt = 1.0 / 60.0;
        let body = 0.8;
        let along = |circles: &[(Vec2, f32)], from: Vec2, way: Vec2| {
            let (mut at, mut t, mut nearest) = (from, 0.0, f32::INFINITY);
            while t < roll.lasts() {
                if let Some(to) = step_to(&ground, None, circles, at, way * roll.drive_at(t) * dt) {
                    at = to;
                }
                nearest = nearest.min(at.length());
                t += dt;
            }
            ((at - from).dot(way), nearest)
        };
        let open = along(&[], Vec2::ZERO, Vec2::X).0;
        assert!(open > 2.0, "the roll covers {open:.2} m on open ground");
        for way in [
            Vec2::X,
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, -0.3),
            Vec2::NEG_Y,
        ] {
            let way = way.normalize();
            for off in [-0.8, -0.4, 0.4, 0.8] {
                let from = -way * 1.6 + way.perp() * off;
                let (went, nearest) = along(&[(Vec2::ZERO, body)], from, way);
                assert!(
                    nearest >= body + footprint::BODY - 1e-3,
                    "way {way:.2} off {off}: he rolled {nearest:.2} m from its middle, into it"
                );
                assert!(
                    went > open * 0.7,
                    "way {way:.2} off {off}: stopped at its bark after {went:.2} of {open:.2} m"
                );
            }
        }
    }

    /// A route's `--goto` brings him to its point and stops him there (j24): walked as `act`
    /// walks him (his speed eased, his heading kept while he coasts, `step_to` round a body
    /// on his line), from far and near, at 30 and 60 frames a second, he stands still within
    /// 0.5 m of it. Its fault, a held key's fixed way (the open-loop route), slides round
    /// the body and passes the point.
    #[test]
    fn a_goto_arrives() {
        let ground = Ground {
            floor: Box::new(|_| 0.0),
            open: Box::new(|_| true),
        };
        let body = [(Vec2::new(4.0, 0.2), 0.8)];
        let walk = |from: Vec2, to: Vec2, dt: f32, way_to: &dyn Fn(Vec2, Vec2, f32) -> Vec2| {
            let (mut at, mut speed, mut heading) = (from, 0.0, Vec2::ZERO);
            for _ in 0..(12.0 / dt) as usize {
                let way = way_to(at, to, speed);
                if way != Vec2::ZERO {
                    heading = way;
                }
                speed = ease(speed, if way == Vec2::ZERO { 0.0 } else { JOG }, dt);
                if let Some(q) = step_to(&ground, None, &body, at, heading * speed * dt) {
                    at = q;
                } else {
                    speed = 0.0;
                }
                assert!(at.distance(body[0].0) >= body[0].1 + footprint::BODY - 1e-3);
            }
            (at.distance(to), speed)
        };
        for dt in [1.0 / 30.0, 1.0 / 60.0] {
            for (from, to) in [
                (Vec2::ZERO, Vec2::new(9.0, 0.0)),
                (Vec2::new(-3.0, 5.0), Vec2::new(8.0, -2.0)),
                (Vec2::new(8.0, 0.4), Vec2::new(8.6, 0.0)),
                (Vec2::new(1.0, 1.0), Vec2::new(1.2, 1.0)),
            ] {
                let (off, speed) = walk(from, to, dt, &toward);
                assert!(
                    off < 0.5,
                    "from {from} to {to} at {dt:.3}: stood {off:.2} m off it"
                );
                assert_eq!(speed, 0.0, "from {from} to {to}: still moving");
            }
        }
        // The fault: a held key's way, the start's, not steered to the point after the body.
        let held = |h: Vec2, t: Vec2, _: f32| {
            if h.distance(t) <= ARRIVE {
                Vec2::ZERO
            } else {
                Vec2::X
            }
        };
        let (off, _) = walk(Vec2::ZERO, Vec2::new(9.0, 0.0), 1.0 / 30.0, &held);
        assert!(
            off > 0.5,
            "the fault: a held way slid round the body ends {off:.2} m off"
        );
    }

    /// The set's buttons ask for the light chain, heavy, block and roll: each ability that is a
    /// move names one the set has, and the light chain goes on through its windows.
    #[test]
    fn each_move_ability_asks_the_set() {
        let set = terra_motion::moves::MoveSet::read(include_str!(
            "../../../../assets/data/moves/sword.shielded.ron"
        ))
        .unwrap();
        let runner = Runner::default();
        for a in Ability::ALL {
            let Some(b) = button(a) else { continue };
            let name = runner.pressed(&set, b).expect("a move for the button");
            assert!(
                set.get(name).is_some(),
                "{a:?} asks for {name}, not in the set"
            );
        }
        let mut r = Runner::default();
        assert!(r.ask(&set, "slash"));
        let slash = r.now(&set).unwrap().clone();
        // Pressed again inside its window, the chain goes on to the next blow.
        let into = slash.at_clip(slash.cancel.unwrap()) + 0.01;
        r.tick(&set, into);
        let next = r.pressed(&set, Button::Main).unwrap().to_string();
        assert_eq!(Some(next.as_str()), slash.next.as_deref());
    }
}
