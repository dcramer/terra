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
        Query<(&footprint::Footprint, &GlobalTransform), Without<Hero>>,
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
    let asked = intent
        .press
        .or(block_down.then_some((Ability::Block, None)));
    if let Some((ability, at)) = asked
        && let Some(b) = button(ability)
        && let Some(name) = doing.runner.pressed(set, b).map(str::to_string)
    {
        let face = match (ability, at) {
            (Ability::Roll, _) => intent.way.normalize_or(ahead),
            (_, Some(p)) => (p - here).normalize_or(ahead),
            // A blow with no aim point turns to the nearest foe in reach (an action RPG's
            // soft target), so a script's or pad's press does not swing at the air.
            (Ability::Light | Ability::Heavy, None) => {
                nearest(here, foes.iter().map(|(_, t)| t.translation.xz()))
                    .map_or(ahead, |p| (p - here).normalize_or(ahead))
            }
            _ => ahead,
        };
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
                commands.entity(foe).insert(strike::Blow { from: here, t });
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
