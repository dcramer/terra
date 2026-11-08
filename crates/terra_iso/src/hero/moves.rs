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
    /// The way a pressed move waiting on the current one's window will face.
    queued: Option<Vec2>,
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
        let target = nearest(here, foes.iter().map(|(_, t)| t.translation.xz()));
        let face = face_for(ability, at, intent.way, (here, ahead), target);
        if doing.runner.ask(set, &name) {
            begin(&mut doing, &mut tr, face);
        } else {
            doing.queued = Some(face);
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
        let face = doing.queued.take().unwrap_or(ahead);
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
    // The motion: the move's drive along its facing, else his own pace along his heading.
    let v = match doing.runner.now(set) {
        Some(m) => {
            doing.speed = 0.0;
            doing.face * m.drive_at(doing.runner.t()) * if own > 0.0 { 1.0 } else { 0.0 }
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

/// The nearest of `foes` within a blow's reach of `here`, if any.
pub fn nearest(here: Vec2, foes: impl Iterator<Item = Vec2>) -> Option<Vec2> {
    foes.map(|p| (p.distance(here), p))
        .filter(|(d, _)| *d <= crate::play::REACH)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, p)| p)
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
fn begin(doing: &mut Doing, tr: &mut Transform, face: Vec2) {
    doing.face = face;
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
        assert_eq!(nearest(here, foes.into_iter()), Some(Vec2::new(1.0, -0.5)));
        assert_eq!(nearest(here, [Vec2::new(4.5, 1.0)].into_iter()), None);
        assert_eq!(nearest(here, std::iter::empty()), None);
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
