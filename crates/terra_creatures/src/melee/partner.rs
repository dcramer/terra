//! The sparring partner (D405): a swordsman with a round shield who closes on a drawn player,
//! guards while circling, then strikes from the moves data, every third blow the heavy one,
//! each with a wind-up long enough to read, and stands open after it, the window to punish.
//! No chance in it, so a scripted run sees the same fight every time (`--steady`). It has a
//! stamina of its own, spent and refilled by the same rules as the player's: too tired for
//! the heavy blow it throws a light one, and a guard it cannot pay for breaks.

use super::{Fighter, Set};
use crate::staged::{self, Staged};
use bevy::prelude::*;
use serde_json::json;
use terra_motion::moves::MoveSet;
use terra_motion::stamina;
use terra_survival::vitals;
use terra_walker::bodies::Body;
use terra_walker::character;
use terra_walker::figure::CLIPS;
use terra_walker::{Player, Stamina, Stance};
use terra_world::shell::ScriptFrame;

/// Its name on the stage.
const NAME: &str = "sparring_partner";
/// How far ahead of the player it steps up, metres.
const AHEAD_M: f32 = 4.0;
/// How near it closes before it guards, and how far off it closes again.
const CLOSE_M: f32 = 2.2;
const OPEN_M: f32 = 3.2;
/// How long it guards between blows, and stands open after one, seconds.
const GUARD_S: f32 = 1.4;
const RECOVER_S: f32 = 1.0;
/// How fast it walks in, and circles on its guard, metres a second.
const WALK_MPS: f32 = 1.8;
const CIRCLE_MPS: f32 = 0.7;

/// Where it is in its rhythm, how long it has been there, how many blows it has thrown,
/// and the clip it was last told.
#[derive(Component, Default)]
pub struct Partner {
    pub beat: Beat,
    pub beat_s: f32,
    blows: u32,
    pub showing: Option<String>,
    /// The nearest the player has come since its last wind-up, feet to feet, metres:
    /// logged with the next, so a scripted run shows whether the bodies ever overlap.
    closest_m: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Beat {
    #[default]
    Close,
    Guard,
    Strike,
    Recover,
}

impl Partner {
    /// Tells its figure a clip unless it plays it already.
    pub fn show(&mut self, staged: &mut Staged, clip: &str, speed: f32, once: bool) {
        if self.showing.as_deref() == Some(clip) {
            staged.set_clip_speed(speed);
            return;
        }
        self.showing = Some(clip.to_string());
        staged.tell_play(clip, speed, once, 200, 0.0);
    }

    pub fn to(&mut self, beat: Beat) {
        self.beat = beat;
        self.beat_s = 0.0;
    }
}

/// Stands a partner a few metres ahead of a player, facing them: the base body as the
/// player's template, dressed, a sword in its right hand and a round shield strapped to its
/// left forearm.
pub fn bring(commands: &mut Commands, at: Vec3, facing: Vec3) {
    let up = at.normalize();
    let spot = at
        + facing
            .reject_from(up)
            .normalize_or(up.any_orthonormal_vector())
            * AHEAD_M;
    let call = json!({
        "name": NAME,
        "model": terra_walker::appearance::BASE,
        "template": "male",
        "clips": CLIPS,
        "wears": ["shirt", "trousers", "boots", "leather_coat"],
        "hold": { "socket.hand.R": super::SWORD, "socket.hand.L": super::SHIELD },
        "at": spot.to_array(),
        "facing": at.to_array(),
        "turn_deg_s": 260.0,
    });
    commands.queue(move |world: &mut World| {
        let spawned = staged::spawn(In(Some(call)), world);
        let entity = spawned
            .ok()
            .and_then(|done| done["entity"].as_u64())
            .and_then(Entity::try_from_bits);
        match entity {
            Some(entity) => {
                world.entity_mut(entity).insert((
                    Partner::default(),
                    Fighter::new(None),
                    Stamina::default(),
                    Body::fixed(0.35),
                ));
                info!("Melee: a sparring partner steps up");
            }
            None => warn!("Melee: the sparring partner could not be stood up"),
        }
    });
}

type Them<'a> = (
    Entity,
    &'a Transform,
    &'a mut Staged,
    &'a mut Fighter,
    &'a mut Partner,
    Option<&'a mut Stamina>,
);

/// Each partner's tick: closing, guarding, striking and recovering on its rhythm against
/// the nearest drawn player; a blow's drive and its step in to land, in its own time.
pub fn think(
    time: Res<Time>,
    set: Res<Set>,
    mut partners: Query<Them<'_>, Without<Player>>,
    players: Query<(&Transform, &Fighter), (With<Player>, With<Stance>, With<super::Drawn>)>,
    children: Query<&Children>,
    names: Query<&Name>,
    frame: Option<Res<ScriptFrame>>,
) {
    let frame = frame.map_or(0, |frame| frame.0);
    let dt = time.delta_secs();
    let set = &set.0;
    for (entity, place, mut staged, mut fighter, mut partner, mut pool) in &mut partners {
        fighter.age(dt);
        if let Some(pool) = pool.as_deref_mut() {
            vitals::rest(pool, dt);
        }
        let rate = fighter.rate();
        staged.tell_rate(rate);
        if fighter.hand.is_none() {
            fighter.hand = character::find(entity, character::HAND_R, &children, &names);
        }
        let here = place.translation;
        let up = here.normalize();
        if fighter.down_s.is_some() {
            staged.tell_go(None, 0.0);
            partner.show(&mut staged, "Death01", 1.0, true);
            continue;
        }
        if !fighter.free() {
            let shoved = (fighter.shove_s > 0.0).then_some(fighter.shove);
            staged.tell_go(shoved.map(|v| here + v), shoved.map_or(0.0, Vec3::length));
            continue;
        }
        let foe = players
            .iter()
            .map(|(at, _)| at.translation)
            .filter(|at| at.distance(here) < 30.0)
            .min_by(|a, b| a.distance(here).total_cmp(&b.distance(here)));
        let Some(foe) = foe else {
            fighter.guard_s = None;
            staged.tell_go(None, 0.0);
            partner.show(&mut staged, &set.guard, 1.0, false);
            continue;
        };
        let to = (foe - here).reject_from(up);
        let far = to.length();
        partner.closest_m = Some(partner.closest_m.map_or(far, |was| was.min(far)));
        let toward = to / far.max(1e-3);
        if fighter.shove_s > 0.0 {
            staged.tell_go(Some(here + fighter.shove), fighter.shove.length());
        }
        // In a blow: it tracks the player through the wind-up, then commits.
        if let Some(each) = fighter.runner.now(set) {
            let t = fighter.runner.t();
            let left = each.until_active(t);
            if left.is_some() {
                fighter.way = toward;
            }
            let mut speed = each.drive_at(t);
            if let (Some(warp), Some(left)) = (each.warp, left)
                && far < warp.within
            {
                speed += super::moves::warp_speed(far, warp.stop_at, left, warp.most_mps);
            }
            staged.tell_face(here + fighter.way);
            // Never into the player: a blow's drive stops at arm's length.
            if far < super::NEAREST_M {
                speed = 0.0;
            }
            if fighter.shove_s <= 0.0 {
                staged.tell_go(Some(here + fighter.way * 2.0), speed);
            }
            if let super::Ran::Ended(_) = fighter.runner.tick(set, dt * rate) {
                partner.to(Beat::Recover);
            }
            continue;
        }
        staged.tell_face(foe);
        partner.beat_s += dt;
        match partner.beat {
            Beat::Close => {
                fighter.guard_s = None;
                if far > CLOSE_M {
                    staged.tell_go(Some(foe - toward * (CLOSE_M - 0.2)), WALK_MPS);
                    partner.show(&mut staged, "Walk_Loop", 1.0, false);
                } else {
                    partner.to(Beat::Guard);
                }
            }
            Beat::Guard => {
                fighter.guard_s = Some(fighter.guard_s.map_or(0.0, |held| held + dt));
                // Circles on its guard, one way and then the other.
                let side = toward.cross(up) * if partner.blows % 2 == 0 { 1.0 } else { -1.0 };
                if fighter.shove_s <= 0.0 {
                    staged.tell_go(Some(here + side), CIRCLE_MPS);
                }
                let clip = if partner.blows % 2 == 0 {
                    "Fight_Strafe_R"
                } else {
                    "Fight_Strafe_L"
                };
                partner.show(&mut staged, clip, 1.0, false);
                if far > OPEN_M {
                    partner.to(Beat::Close);
                } else if partner.beat_s > GUARD_S {
                    partner.to(Beat::Strike);
                }
            }
            Beat::Strike => wind_up(
                set,
                &mut fighter,
                &mut partner,
                &mut staged,
                pool.as_deref_mut(),
                (toward, far),
                frame,
            ),
            Beat::Recover => {
                fighter.guard_s = None;
                staged.tell_go(None, 0.0);
                partner.show(&mut staged, &set.guard, 1.0, false);
                if partner.beat_s > RECOVER_S {
                    partner.to(if far > CLOSE_M {
                        Beat::Close
                    } else {
                        Beat::Guard
                    });
                }
            }
        }
    }
}

/// The partner starts a blow: every third the heavy one, the light one when too tired for it
/// (itself tired if need be), its stamina paid and its clip told.
fn wind_up(
    set: &MoveSet,
    fighter: &mut Fighter,
    partner: &mut Partner,
    staged: &mut Staged,
    mut pool: Option<&mut Stamina>,
    (toward, far): (Vec3, f32),
    frame: u32,
) {
    partner.blows += 1;
    let wanted = if partner.blows.is_multiple_of(3) {
        "partner_heavy"
    } else {
        "partner_slash"
    };
    let left = pool.as_ref().map_or(f32::MAX, |pool| pool.left);
    let Some(name) = [wanted, "partner_slash"]
        .into_iter()
        .find(|name| stamina::start(&mut fighter.runner, set, name, left))
    else {
        partner.to(Beat::Guard);
        return;
    };
    let Some(each) = set.get(name) else { return };
    if let Some(pool) = pool.as_deref_mut() {
        pool.spend(each.stamina);
    }
    fighter.guard_s = None;
    fighter.way = toward;
    fighter.was = None;
    partner.showing = Some(each.clip.clone());
    let speed = each.speed * fighter.runner.pace(set);
    staged.tell_play(&each.clip, speed, true, 150, each.from);
    info!(
        "Melee: frame {frame}: the partner winds up {name}{}, {far:.2} m off \
         (closest since its last {:.2} m), {:.0} stamina left",
        if fighter.runner.tired { " tired" } else { "" },
        partner.closest_m.take().unwrap_or(far),
        pool.map_or(f32::MAX, |pool| pool.left)
    );
}
