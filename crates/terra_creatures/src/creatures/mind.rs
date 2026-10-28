//! What each creature does next (`think`): what it senses of those about it, then whether
//! it is up off the ground (`aloft`), what its temper makes of them (`shy`, `hostile`), or
//! else what it gets on with (`settle`).

use super::gait::{CROSSES, Pace, bolt, bolts_within};
use super::{Bitten, CLIMB_REACH_M, Creature, State};
use crate::species::{Bestiary, Moves, Refuge, SpeciesDef, Temper};
use crate::wildlife::Surroundings;
use bevy::prelude::*;
use terra_gen::Rng;
use terra_survival::hazards::Frightful;
use terra_walker::walker::Motion;
use terra_walker::{Downed, Player};
use terra_world::cine::{Cine, Subject};
use terra_world::flora::Trunk;
use terra_world::lod::Detail;
use terra_world::spatial::SpatialHash;

/// How far off a creature notices a player, or keeps track of one it chases.
pub const SENSE_M: f32 = 40.0;
/// A runner watches anyone coming from this many times as far as it flees them.
const WATCHES_WITHIN: f32 = 2.5;
/// Within this many times as far as it flees it stops feeding and stares; farther off, a
/// grazer looks up between bouts of feeding, and this share of the time it bends to feed
/// again rather than looking (feeding elk and roe deer spend most of their time head down
/// and a fifth to a third of it vigilant: Childress and Lung 2003, Benhaiem et al. 2008).
const STARES_WITHIN: f32 = 1.5;
const FEEDS_ON: f32 = 0.75;
/// How long a look at someone coming lasts at the most, seconds, before it goes back to its
/// business or, them close, turns and walks off: an alert look lasts seconds, not the whole
/// approach (a fox sat square to the lens through the lake film's last 2.5 s, r10).
pub const LOOKS_S: f32 = 1.2;
/// How far ahead a creature reckons where someone on the move is going, in seconds.
const RECKONS_AHEAD_S: f32 = 8.0;

/// What a creature makes of those about it as it thinks.
struct Sense {
    /// The time it thinks over.
    dt: f32,
    here: Vec3,
    up: Vec3,
    /// The nearest player it senses, and how far off across the ground (infinite for none).
    quarry: Option<Entity>,
    apart: f32,
    /// Toward the nearest player, along the ground.
    toward: Option<Vec3>,
    /// Away from the nearest player, and across their way and on ahead of them if they are
    /// coming on; or away from something frightful near.
    away: Option<Vec3>,
    /// How near the nearest player's way comes to it, kept to as they go now
    /// (`passes_within`): none, or as near as they stand, for one keeping still.
    passes: f32,
}

impl Sense {
    /// Whether a player is within so far of it.
    fn near(&self, within_m: f32) -> bool {
        self.quarry.is_some() && self.apart < within_m
    }
}

/// Decides what each creature does next.
#[allow(clippy::too_many_arguments)]
pub fn think(
    time: Res<Time>,
    bestiary: Res<Bestiary>,
    around: Surroundings,
    players: Query<
        (Entity, &Transform, Option<&Motion>, Has<Subject>),
        (With<Player>, Without<Downed>),
    >,
    mut creatures: Query<(&mut Creature, &Transform, &Detail), Without<Player>>,
    frights: Query<(&Frightful, &GlobalTransform)>,
    mut bitten: MessageWriter<Bitten>,
    mut awake: Local<SpatialHash<Entity>>,
    filming: Option<Res<Cine>>,
) {
    let tick = time.delta_secs().min(0.1);
    // A shot that hides its player has them out of play (`Cine::hides_player`).
    let out_of_play = filming.is_some_and(|cine| cine.hides_player());
    // Players filed by place, so each creature looks only at those near it (D127).
    awake.clear();
    awake.extend(
        players
            .iter()
            .filter(|(_, _, _, filmed)| !(out_of_play && *filmed))
            .map(|(player, at, _, _)| (at.translation, player)),
    );
    for (mut creature, at, detail) in &mut creatures {
        // Far from everyone it thinks less often, over the ticks it waited (D145).
        creature.lived += 1;
        let Some(dt) = detail.thinks(creature.lived, creature.turn(), tick) else {
            continue;
        };
        // Its own dice, rolled here and put back (`Creature::dice`).
        let mut dice = creature.dice.clone();
        let here = at.translation;
        let up = here.normalize();
        let nearest_player = nearest(&awake, here);
        let (quarry, apart) = nearest_player.unzip();
        creature.quarry = quarry;
        let about = bestiary.get(creature.species);
        let player = nearest_player.and_then(|(player, _)| {
            players
                .get(player)
                .ok()
                .map(|(player, at, motion, _)| (player, at, motion))
        });
        let toward = player
            .map(|(_, p, _)| (p.translation - here).reject_from(up).normalize_or_zero())
            .filter(|toward| *toward != Vec3::ZERO);
        let passes = player.map_or(f32::INFINITY, |(_, p, motion)| {
            passes_within(
                here - p.translation,
                motion.map_or(Vec3::ZERO, |m| m.velocity),
                up,
            )
        });
        let mut away = player.map(|(_, p, motion)| {
            let from = (here - p.translation).reject_from(up);
            let heading = motion.map_or(Vec3::ZERO, |m| m.velocity.reject_from(up));
            bolt(from, heading, up, dice.unit() < CROSSES)
        });
        // Something frightful near (a dragon, a fire) startles a shy creature as a blow
        // does, and it goes away from it.
        if matches!(about.temper, Temper::Shy { .. })
            && !matches!(creature.state, State::Flee(_) | State::Fly { .. })
            && let Some(from) = nearest_fright(&frights, here)
        {
            creature.hurt = true;
            away = Some((here - from).reject_from(up).normalize_or_zero());
        }
        let sense = Sense {
            dt,
            here,
            up,
            quarry,
            apart: apart.unwrap_or(f32::INFINITY),
            toward,
            away,
            passes,
        };
        let next = decide(
            &mut creature,
            about,
            &sense,
            &around,
            &mut dice,
            &mut bitten,
        );
        creature.dice = dice;
        if bestiary.hostile(creature.species) && !matches!(next, State::Chase | State::Attack(_)) {
            creature.hurt = false;
        }
        if std::mem::discriminant(&next) != std::mem::discriminant(&creature.state) {
            debug!(
                "{} {:?} -> {:?}, the nearest player {:.1} m off",
                bestiary.name(creature.species),
                creature.state,
                next,
                sense.apart
            );
        }
        creature.state = next;
    }
}

/// How near someone so far `from` a creature (from them to it) and going so fast comes to it
/// as they keep on their way over the next few seconds: none for one standing about, who may
/// come any way.
fn passes_within(from: Vec3, velocity: Vec3, up: Vec3) -> f32 {
    let from = from.reject_from(up);
    let heading = velocity.reject_from(up);
    let speed = heading.length();
    if speed < 0.5 {
        return 0.0;
    }
    let ahead_s = (from.dot(heading) / (speed * speed)).clamp(0.0, RECKONS_AHEAD_S);
    (from - heading * ahead_s).length()
}

/// What one creature does next, from what it is doing and what it senses.
fn decide(
    creature: &mut Creature,
    about: &SpeciesDef,
    s: &Sense,
    around: &Surroundings,
    dice: &mut Rng,
    bitten: &mut MessageWriter<Bitten>,
) -> State {
    if let Some(next) = aloft(creature, about, s, around, dice) {
        return next;
    }
    let roused = match about.temper {
        Temper::Shy {
            flees_within_m,
            flees_s,
            ..
        } => shy(creature, about, s, (flees_within_m, flees_s), around, dice),
        Temper::Hostile { .. } => hostile(creature, about.temper, s, bitten),
    };
    roused.unwrap_or_else(|| settle(creature, about, s, around, dice))
}

/// Dead, on the wing, up in a crown or on a trunk: what it does there, or none for one on
/// the ground.
fn aloft(
    creature: &mut Creature,
    about: &SpeciesDef,
    s: &Sense,
    around: &Surroundings,
    dice: &mut Rng,
) -> Option<State> {
    let flees_within_m = match about.temper {
        Temper::Shy { flees_within_m, .. } => Some(flees_within_m),
        Temper::Hostile { .. } => None,
    };
    Some(match creature.state {
        State::Dead(lying) => State::Dead(lying + s.dt),
        // Gone where it went to be safe, it stays there while it plays out.
        State::Fly { from, to, along } if along < 1.0 => State::Fly { from, to, along },
        State::Fly { to, .. } => landed(creature, to, around, dice),
        // Sitting up there, it lets someone come nearer than on the ground, then clatters
        // off, as pigeons go up off a roof over someone passing under.
        State::Roost { .. } if flees_within_m.is_some_and(|m| s.near(m * ROOSTING_LETS)) => {
            let away = s.away.unwrap_or(creature.facing);
            (creature.bolt, creature.pace) = (away, Pace::Startled);
            refuge(about, s.here, away, around, dice).unwrap_or(State::Roost {
                at: s.here,
                left: 2.0,
            })
        }
        State::Roost { at, left } if left > 0.0 => State::Roost {
            at,
            left: left - s.dt,
        },
        State::Roost { .. } => leave_roost(about, s.here, around, dice),
        State::Perch { trunk, left } if flees_within_m.is_some() => {
            if flees_within_m.is_some_and(|m| s.near(m * 1.6)) {
                State::Perch {
                    trunk,
                    left: left.max(2.0),
                }
            } else if left > 0.0 {
                State::Perch {
                    trunk,
                    left: left - s.dt,
                }
            } else {
                State::Climb { trunk, height: 0.0 }
            }
        }
        State::Climb { trunk, height } => {
            if height > 0.0 && creature.aloft >= height - 0.05 {
                State::Perch {
                    trunk,
                    left: dice.range(6.0, 14.0),
                }
            } else if height <= 0.0 && creature.aloft <= 0.0 {
                State::Idle(dice.range(1.0, 3.0))
            } else {
                State::Climb { trunk, height }
            }
        }
        _ => return None,
    })
}

/// Come down where it flew to: in a crown it sits a while among the leaves, on the ground
/// or the water it is home there.
fn landed(creature: &mut Creature, to: Vec3, around: &Surroundings, dice: &mut Rng) -> State {
    let ground = around.ground(to);
    creature.home = ground;
    // Over the ground or the water's surface, whichever is higher.
    let under = ground.length() + around.depth_m(ground).max(0.0);
    if to.length() - under > ROOSTS_OVER_M {
        State::Roost {
            at: to,
            left: dice.range(4.0, 16.0),
        }
    } else {
        State::Idle(dice.range(1.0, 3.0))
    }
}

/// Done sitting in a crown: down to the ground again, or across to another crown.
fn leave_roost(about: &SpeciesDef, here: Vec3, around: &Surroundings, dice: &mut Rng) -> State {
    let up = here.normalize();
    let turn = Quat::from_axis_angle(up, dice.range(0.0, std::f32::consts::TAU));
    let way = turn * up.any_orthonormal_vector();
    let crown = if dice.unit() < 0.5 {
        up_high(here, way, (6.0, 25.0), around, dice)
    } else {
        None
    };
    crown
        .or_else(|| flight(about, here, way, (4.0, 14.0), around, dice))
        .unwrap_or(State::Roost {
            at: here,
            left: 3.0,
        })
}

/// A shy one on the ground: watching someone coming, walking off from one within the
/// distance it flees at, bolting from one close or once hurt (D384); none if nobody is near.
fn shy(
    creature: &mut Creature,
    about: &SpeciesDef,
    s: &Sense,
    (flees_within_m, flees_s): (f32, f32),
    around: &Surroundings,
    dice: &mut Rng,
) -> Option<State> {
    let bolts_m = bolts_within(flees_within_m);
    Some(match creature.state {
        // A calm one crossing, or a wary one walking off, keeps to a walk until someone comes
        // close, and then it runs, climbs or takes wing as one startled does; a crossing stag
        // that broke into a run within its whole flight distance dashed back across the
        // runner's way and stood beside it (lake film r9).
        State::Flee(left) if left > 0.0 => {
            let wary = creature.pace == Pace::Wary;
            let near = s.near(bolts_m);
            creature.pace = creature.pace.roused(near, creature.hurt);
            if wary && creature.pace != Pace::Wary {
                creature.hurt = false;
                creature.bolt = s.away.unwrap_or(creature.bolt);
                let away = s.away.unwrap_or(creature.facing);
                refuge(about, s.here, away, around, dice).unwrap_or(State::Flee(flees_s))
            } else {
                State::Flee(left - s.dt)
            }
        }
        // One that has noticed someone coming keeps feeding while they are well off, its head
        // down most of the time and up now and then to look, as grazers share their time
        // between feeding and vigilance; only once they come nearer does it stop, turn to face
        // them and watch, before it makes off, as a hare, a fox or a heron does. Staring at
        // anyone within the whole distance it watches from, every animal beside a runner's way
        // stood head up toward him, side by side, as if posed for the camera (lake film r7).
        // Someone going by on their way, which keeps wider of it than it bolts at, it lets
        // be and goes on with its day, as deer beside a trail feed on while walkers pass and
        // flee a walker coming straight at them from twice as far (Taylor and Knight 2003;
        // tangential against direct approach, Stankowich 2008). Watching every runner on his
        // way, each animal beside the lake turned to face him, head up (lake film r8).
        State::Idle(_) | State::Graze(_) | State::Wander(_)
            if s.near(flees_within_m * WATCHES_WITHIN)
                && !s.near(flees_within_m)
                && !creature.hurt
                && s.passes >= bolts_m =>
        {
            return None;
        }
        State::Idle(_) | State::Graze(_) | State::Wander(_)
            if s.near(flees_within_m * WATCHES_WITHIN)
                && !s.near(flees_within_m)
                && !creature.hurt =>
        {
            let close = s.near(flees_within_m * STARES_WITHIN);
            let feeds = about.clips.eat.is_some();
            match creature.state {
                State::Graze(left) if !close && left > 0.0 => State::Graze(left - s.dt),
                State::Idle(left) if left > 0.0 => {
                    look_toward(creature, s);
                    State::Idle(left.min(LOOKS_S) - s.dt)
                }
                State::Wander(to) if !close && (to - s.here).length() > 1.0 => State::Wander(to),
                _ if !close && feeds && dice.unit() < FEEDS_ON => {
                    State::Graze(dice.range(4.0, 9.0))
                }
                // One that does not graze, a fox, goes about its business between looks,
                // nosing along the ground away from them, as a grazer feeds.
                _ if !close && dice.unit() < FEEDS_ON => about_its_business(s, dice),
                _ if !close => {
                    look_toward(creature, s);
                    State::Idle(dice.range(0.6, LOOKS_S))
                }
                // Them close and its look over, it turns and walks off.
                State::Idle(_) => walk_off(creature, s, flees_s, dice),
                _ => {
                    look_toward(creature, s);
                    State::Idle(LOOKS_S)
                }
            }
        }
        // Within the distance it flees at but not close: it walks, paddles or wades off
        // from them, straight away, and keeps an eye on them.
        // Each of a group its own way off, within a quarter turn of straight away: all
        // straight away from one far off, a group of ducks paddled off as a row of clones in
        // step (lake film r5).
        _ if !creature.hurt && s.near(flees_within_m) && !s.near(bolts_m) => {
            walk_off(creature, s, flees_s, dice)
        }
        _ if creature.hurt || s.near(flees_within_m) => {
            creature.pace = Pace::Startled.roused(true, creature.hurt);
            creature.hurt = false;
            creature.bolt = s.away.unwrap_or(Vec3::ZERO);
            let away = s.away.unwrap_or(creature.facing);
            refuge(about, s.here, away, around, dice).unwrap_or(State::Flee(flees_s))
        }
        _ => return None,
    })
}

/// Walks off from the nearest player, wary, within a quarter turn of straight away.
fn walk_off(creature: &mut Creature, s: &Sense, flees_s: f32, dice: &mut Rng) -> State {
    let off = Quat::from_axis_angle(s.up, dice.range(-0.5, 0.5));
    (creature.bolt, creature.pace) = (s.toward.map_or(Vec3::ZERO, |t| off * -t), Pace::Wary);
    State::Flee(flees_s)
}

/// A few metres on its own way, never toward the nearest player: a fox quartering a slope.
fn about_its_business(s: &Sense, dice: &mut Rng) -> State {
    let turn = Quat::from_axis_angle(s.up, dice.range(0.0, std::f32::consts::TAU));
    let mut way = turn * s.up.any_orthonormal_vector();
    if let Some(toward) = s.toward
        && way.dot(toward) > 0.0
    {
        way = way.reject_from(toward) * 2.0 - way;
    }
    State::Wander(s.here + way * dice.range(4.0, 10.0))
}

/// Turns a creature a little more toward the nearest player, level, for the time it thinks.
fn look_toward(creature: &mut Creature, s: &Sense) {
    if let Some(toward) = s.toward {
        let turn = 1.0 - libm::expf(-s.dt * 3.0);
        creature.facing = creature
            .facing
            .lerp(toward, turn)
            .reject_from(s.up)
            .normalize_or(toward);
    }
}

/// A hostile one: biting, chasing whoever comes near or hurt it, giving up when led too far;
/// none if nobody is near.
fn hostile(
    creature: &mut Creature,
    temper: Temper,
    s: &Sense,
    bitten: &mut MessageWriter<Bitten>,
) -> Option<State> {
    let Temper::Hostile {
        bite,
        bite_at_s,
        bite_reach_m,
        attack_s,
        attacks_within_m,
        notices_m,
        leash_m,
        ..
    } = temper
    else {
        return None;
    };
    let from_home = (s.here - creature.home).length();
    Some(match creature.state {
        State::Attack(through) => {
            let through = through + s.dt;
            // The bite lands partway through, if its quarry is still there.
            if through - s.dt < bite_at_s
                && through >= bite_at_s
                && s.apart < bite_reach_m
                && let Some(player) = s.quarry
            {
                bitten.write(Bitten {
                    player,
                    damage: bite,
                });
            }
            if through > attack_s {
                State::Chase
            } else {
                State::Attack(through)
            }
        }
        _ if s.near(attacks_within_m) => State::Attack(0.0),
        _ if s.quarry.is_some()
            && (s.apart < notices_m || creature.hurt)
            && from_home < leash_m =>
        {
            State::Chase
        }
        State::Chase => State::Return,
        _ => return None,
    })
}

/// Going on with what it was doing, or else resting a while, grazing if it can, or wandering
/// somewhere near home; one that flies flits to its next place rather than walking far.
fn settle(
    creature: &Creature,
    about: &SpeciesDef,
    s: &Sense,
    around: &Surroundings,
    dice: &mut Rng,
) -> State {
    // Where the nearest player is and how wide of them it keeps, if they are near enough
    // to mind: it never makes its way toward them, so a stag grazing above a runner's path
    // walked down to the water beside him (lake film r8).
    let minds = match about.temper {
        Temper::Shy { flees_within_m, .. } if s.near(flees_within_m * WATCHES_WITHIN) => s
            .toward
            .map(|toward| (s.here + toward * s.apart, toward, flees_within_m)),
        _ => None,
    };
    let toward_them =
        |to: Vec3| minds.is_some_and(|(them, _, keeps_m)| (to - them).length() < keeps_m * 1.2);
    match creature.state {
        State::Return if (s.here - creature.home).length() > 2.0 => return State::Return,
        State::Idle(left) if left > 0.0 => return State::Idle(left - s.dt),
        State::Graze(left) if left > 0.0 => return State::Graze(left - s.dt),
        State::Wander(to) if (to - s.here).length() > 1.0 && !toward_them(to) => {
            return State::Wander(to);
        }
        _ => {}
    }
    // One that can graze stands idle less than it feeds, as a grazer's day goes; standing
    // head up two times in five, a herd beside a path read as on watch (lake film r7).
    let roll = dice.unit();
    let idles = if about.clips.eat.is_some() { 0.25 } else { 0.4 };
    if roll < idles {
        return State::Idle(dice.range(2.0, 5.0));
    }
    if roll < 0.75 && about.clips.eat.is_some() {
        return State::Graze(dice.range(3.0, 7.0));
    }
    let up = creature.home.normalize();
    let turn = Quat::from_axis_angle(up, dice.range(0.0, std::f32::consts::TAU));
    let mut way = turn * up.any_orthonormal_vector();
    if let Some((_, toward, _)) = minds
        && way.dot(toward) > 0.0
    {
        way = way.reject_from(toward) * 2.0 - way;
    }
    let here = s.here;
    match about.moves {
        // Up into a crown now and then, among trees.
        Moves::Flies(_) if dice.unit() < 0.2 => {
            up_high(here, way, (5.0, 20.0), around, dice).unwrap_or(State::Idle(1.0))
        }
        Moves::Flies(_) if dice.unit() < 0.5 => {
            flight(about, here, way, (4.0, 14.0), around, dice).unwrap_or(State::Idle(1.0))
        }
        Moves::Flies(_) => State::Wander(creature.home + way * dice.range(0.8, 3.0)),
        _ => {
            let to = creature.home + way * dice.range(2.0, 12.0);
            if toward_them(to) && about.clips.eat.is_some() {
                State::Graze(dice.range(3.0, 7.0))
            } else if toward_them(to) {
                State::Idle(dice.range(2.0, 5.0))
            } else {
                State::Wander(to)
            }
        }
    }
}

/// Where a shy creature goes when it is startled, by its kind's refuge: up the nearest trunk,
/// away on the wing, or none, for one that runs.
fn refuge(
    about: &SpeciesDef,
    here: Vec3,
    away: Vec3,
    around: &Surroundings,
    dice: &mut Rng,
) -> Option<State> {
    match about.refuge {
        Refuge::Runs => None,
        Refuge::Climbs => {
            let up = here.normalize();
            // The nearest, as near goes the way it breaks: a trunk across the path rather
            // than one back toward whoever startled it.
            around
                .trunks(here, CLIMB_REACH_M)
                .min_by(|a, b| {
                    let far = |t: &Trunk| {
                        let off = (t.foot - here).reject_from(up);
                        off.length() * (1.5 - off.normalize_or_zero().dot(away))
                    };
                    far(a).total_cmp(&far(b))
                })
                .map(|trunk| State::Climb {
                    trunk,
                    height: (trunk.height_m * dice.range(0.3, 0.55)).clamp(2.0, 9.0),
                })
        }
        // Among trees a startled bird as often as not goes up into a crown.
        Refuge::Flies(near, far) => {
            // Not a swimmer, which comes down on the water.
            let up_a_tree = if !matches!(about.moves, Moves::Swims(_)) && dice.unit() < 0.5 {
                up_high(here, away, (near * 0.6, far), around, dice)
            } else {
                None
            };
            up_a_tree.or_else(|| flight(about, here, away, (near, far), around, dice))
        }
    }
}

/// Where the nearest frightful thing is that a creature here is within reach of, if any.
fn nearest_fright(frights: &Query<(&Frightful, &GlobalTransform)>, here: Vec3) -> Option<Vec3> {
    frights
        .iter()
        .map(|(fright, at)| (fright, at.translation()))
        .filter(|(fright, at)| at.distance(here) < fright.within_m)
        .map(|(_, at)| at)
        .min_by(|a, b| a.distance(here).total_cmp(&b.distance(here)))
}

/// A flight up off the ground roughly the way given, within so far: into a tree's crown, or
/// where there are no trees, onto a roof's ridge.
fn up_high(
    here: Vec3,
    way: Vec3,
    reach: (f32, f32),
    around: &Surroundings,
    dice: &mut Rng,
) -> Option<State> {
    crown(here, way, reach, around, dice).or_else(|| {
        around.perch(here, way, reach, dice).map(|to| State::Fly {
            from: here,
            to,
            along: 0.0,
        })
    })
}

/// A flier is in a crown once it is so far over the ground.
pub const ROOSTS_OVER_M: f32 = 1.5;
/// One sitting up off the ground lets someone come this share of its flight distance.
const ROOSTING_LETS: f32 = 0.8;

/// A flight up into the crown of a tree roughly the way given, within so far: out from its
/// trunk, two thirds of the way up it, where its branches are.
fn crown(
    here: Vec3,
    way: Vec3,
    (near, far): (f32, f32),
    around: &Surroundings,
    dice: &mut Rng,
) -> Option<State> {
    let up = here.normalize();
    let trunk = around
        .trunks(here, far)
        .filter(|trunk| trunk.height_m > 6.0)
        .filter(|trunk| {
            let off = (trunk.foot - here).reject_from(up);
            off.length() >= near && off.normalize_or_zero().dot(way) > -0.2
        })
        .max_by(|a, b| {
            let lean = |t: &Trunk| (t.foot - here).reject_from(up).normalize_or_zero().dot(way);
            lean(a).total_cmp(&lean(b))
        })?;
    let foot_up = trunk.foot.normalize();
    let turn = Quat::from_axis_angle(foot_up, dice.range(0.0, std::f32::consts::TAU));
    let side = turn * foot_up.any_orthonormal_vector();
    let to = trunk.foot
        + foot_up * trunk.height_m * dice.range(0.5, 0.75)
        + side * (trunk.radius_m + dice.range(0.6, 2.2));
    Some(State::Fly {
        from: here,
        to,
        along: 0.0,
    })
}

/// A flight from here to somewhere it keeps to, roughly the way given and within so far: the
/// first of a few tries that lands where its niche is, or else as dry or wet as it lives.
fn flight(
    about: &SpeciesDef,
    here: Vec3,
    way: Vec3,
    (near, far): (f32, f32),
    around: &Surroundings,
    dice: &mut Rng,
) -> Option<State> {
    let up = here.normalize();
    let mut fallback = None;
    for _ in 0..6 {
        let turn = Quat::from_axis_angle(up, dice.range(-0.7, 0.7));
        let to = around.ground(here + turn * way * dice.range(near, far));
        let wet = matches!(about.moves, Moves::Swims(_));
        let depth = around.depth_m(to);
        if (depth > 0.4) != wet {
            continue;
        }
        // A swimmer comes down on the water, not on the bed under it.
        let to = if wet { to + to.normalize() * depth } else { to };
        let state = State::Fly {
            from: here,
            to,
            along: 0.0,
        };
        if about
            .niche
            .as_ref()
            .is_none_or(|niche| around.fits(niche, to))
        {
            return Some(state);
        }
        fallback.get_or_insert(state);
    }
    fallback
}

/// The nearest awake player a creature at a place can sense, and how far off across the
/// ground.
pub fn nearest(awake: &SpatialHash<Entity>, here: Vec3) -> Option<(Entity, f32)> {
    let up = here.normalize();
    awake
        .near(here, SENSE_M)
        .map(|&(at, player)| (player, (at - here).reject_from(up).length()))
        .filter(|&(_, apart)| apart <= SENSE_M)
        .min_by(|a, b| a.1.total_cmp(&b.1))
}
