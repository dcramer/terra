//! What a blow does when its blade reaches a body (D405): each fighter's sword, from hilt to
//! tip in its hand socket's frame, is swept from last tick to this one through its blow's
//! active frames against the other's hurt capsule (`blade`), once a blow. Judged (`moves::
//! judge`): a hit costs health, holds both in a hit-stop and makes the struck flinch; a
//! block costs nothing and shoves both apart with a clash; a parry staggers the attacker with
//! a flash and a ring; a heavy blow breaks a guard held too long; a roll's i-frames let it by.
//! Every outcome is logged as `Melee: ...` with the scripted run's frame (D69).

use super::blade::{self, Edge};
use super::moves::Taken;
use super::partner::{Beat, Partner};
use super::{DOWN_S, Drawn, Fighter, HEALTH, Set, show};
use crate::staged::Staged;
use bevy::prelude::*;
use terra_survival::noise::{Cue, Noise};
use terra_survival::told::Notice;
use terra_walker::figure::Acting;
use terra_walker::{Motion, Player};
use terra_world::shell::ScriptFrame;

/// The sword's blade along its hand socket's up: from the hilt to the tip, metres, as
/// `tools/characters/phases.ts` measured the clips with it.
const HILT_M: f32 = 0.15;
const TIP_M: f32 = 0.83;
/// How far either side of straight ahead a guard covers, degrees.
const GUARD_DEG: f32 = 75.0;
/// How long a blow taken full on makes a body flinch, a parried attacker stagger, and a
/// broken guard reel, seconds.
const FLINCH_S: f32 = 0.45;
const STAGGER_S: f32 = 1.2;
const BROKEN_S: f32 = 0.8;
/// A parry's flash, seconds.
const FLASH_S: f32 = 0.18;

/// A flash where a parry turned a blade: a light dying away.
#[derive(Component)]
pub struct Flash {
    left: f32,
}

/// How bright the screen's white flash is, from 1 down (`hud`).
#[derive(Resource, Default)]
pub struct Glare(pub f32);

type Body<'a> = (
    Entity,
    &'a Transform,
    &'a mut Fighter,
    Option<&'a Motion>,
    Option<&'a mut Staged>,
    Option<&'a mut Acting>,
    Option<&'a mut Drawn>,
    Option<&'a mut Partner>,
    Has<Player>,
);

/// A fighter as the blades see it this tick.
struct Seen {
    entity: Entity,
    feet: Vec3,
    facing: Vec3,
    edge: Option<Edge>,
    was: Option<Edge>,
    open: bool,
    invulnerable: bool,
    guard_s: Option<f32>,
    player: bool,
}

/// A blow that reached a body.
struct Landed {
    by: usize,
    on: usize,
    at: Vec3,
    taken: Taken,
    damage: f32,
    hit_stop: f32,
}

/// Sweeps every blade in its blow's active frames against every other fighter, and does
/// what each blow that reached one does.
pub fn strike(
    mut commands: Commands,
    set: Res<Set>,
    frame: Option<Res<ScriptFrame>>,
    mut fighters: Query<Body<'_>>,
    hands: Query<&GlobalTransform>,
    mut noises: MessageWriter<Noise>,
    mut notice: ResMut<Notice>,
    mut glare: ResMut<Glare>,
) {
    let set = &set.0;
    let frame = frame.map_or(0, |frame| frame.0);
    let mut seen = Vec::new();
    for (entity, place, fighter, motion, staged, _, _, _, player) in &fighters {
        let edge = fighter
            .hand
            .and_then(|hand| hands.get(hand).ok())
            .map(|hand| Edge {
                hilt: hand.transform_point(Vec3::Y * HILT_M),
                tip: hand.transform_point(Vec3::Y * TIP_M),
            });
        let now = fighter.runner.now(set);
        let t = fighter.runner.t();
        let facing = motion
            .map(|motion| motion.facing)
            .or(staged.map(|staged| staged.facing()))
            .unwrap_or(Vec3::Z);
        seen.push(Seen {
            entity,
            feet: place.translation,
            facing,
            edge,
            was: fighter.was,
            open: now.is_some_and(|each| {
                each.phase(t) == super::moves::Phase::Active && each.damage > 0.0
            }) && !fighter.runner.landed,
            invulnerable: now.is_some_and(|each| each.invulnerable(t)),
            guard_s: fighter.guard_s,
            player,
        });
    }
    let mut landed = Vec::new();
    for (by, attacker) in seen.iter().enumerate() {
        let (Some(was), Some(now), true) = (attacker.was, attacker.edge, attacker.open) else {
            continue;
        };
        for (on, body) in seen.iter().enumerate() {
            if on == by || attacker.player == body.player {
                continue;
            }
            let up = body.feet.normalize();
            let Some(at) = blade::sweep(was, now, blade::body_at(body.feet, up)) else {
                continue;
            };
            let from = (attacker.feet - body.feet)
                .reject_from(up)
                .normalize_or_zero();
            let facing = body.facing.dot(from) > libm::cosf(GUARD_DEG.to_radians());
            let Ok((.., fighter, _, _, _, _, _, _)) = fighters.get(attacker.entity) else {
                continue;
            };
            let Some(each) = fighter.runner.now(set) else {
                continue;
            };
            let taken = super::moves::judge(
                body.invulnerable,
                body.guard_s,
                facing,
                each.heavy,
                set.parry_s,
            );
            landed.push(Landed {
                by,
                on,
                at,
                taken,
                damage: each.damage,
                hit_stop: each.hit_stop,
            });
            break;
        }
    }
    for (index, each) in seen.iter().enumerate() {
        if let Ok((.., mut fighter, _, _, _, _, _, _)) = fighters.get_mut(each.entity) {
            fighter.was = each.edge;
            if landed.iter().any(|blow| blow.by == index) {
                fighter.runner.landed = true;
            }
        }
    }
    let mut out = Out {
        commands: &mut commands,
        noises: &mut noises,
        notice: &mut notice,
        glare: &mut glare,
    };
    for blow in landed {
        settle(&mut out, &mut fighters, &seen, blow, frame);
    }
}

/// Where what a blow does goes: things spawned, noises, the notice and the screen's flash.
struct Out<'a, 'w, 's, 'n, 'm, 'g> {
    commands: &'a mut Commands<'w, 's>,
    noises: &'a mut MessageWriter<'n, Noise>,
    notice: &'a mut ResMut<'m, Notice>,
    glare: &'a mut ResMut<'g, Glare>,
}

/// Does what a blow that reached a body does, as it was taken.
fn settle(out: &mut Out, fighters: &mut Query<Body<'_>>, seen: &[Seen], blow: Landed, frame: u32) {
    let (by, on) = (&seen[blow.by], &seen[blow.on]);
    let away = (on.feet - by.feet)
        .reject_from(on.feet.normalize())
        .normalize_or_zero();
    let who = |player: bool| if player { "you" } else { "the partner" };
    info!(
        "Melee: frame {frame}: {} {:?} by {} ({} damage, {:.2} m apart)",
        who(on.player),
        blow.taken,
        who(by.player),
        blow.damage,
        (on.feet - by.feet)
            .reject_from(on.feet.normalize())
            .length()
    );
    match blow.taken {
        Taken::Hit => {
            out.noises.write(Noise::at(Cue::Hit, blow.at));
            stop(fighters, [by.entity, on.entity], blow.hit_stop);
            hurt(fighters, on.entity, blow.damage, "Hit_Chest", FLINCH_S, 1.0);
        }
        Taken::Blocked => {
            out.noises.write(Noise::at(Cue::Clash, blow.at));
            stop(fighters, [by.entity, on.entity], blow.hit_stop * 0.6);
            shove(fighters, on.entity, away * 2.6);
            shove(fighters, by.entity, -away * 1.2);
        }
        Taken::Parried => {
            out.noises.write(Noise::at(Cue::Parry, blow.at));
            out.noises.write(Noise::at(Cue::Clash, blow.at));
            stop(fighters, [by.entity, on.entity], 0.12);
            hurt(fighters, by.entity, 0.0, "Hit_Head", STAGGER_S, 0.35);
            out.commands.spawn((
                PointLight {
                    intensity: 400_000.0,
                    color: Color::srgb(1.0, 0.9, 0.7),
                    range: 6.0,
                    ..default()
                },
                Transform::from_translation(blow.at),
                Flash { left: FLASH_S },
            ));
            out.glare.0 = 1.0;
            out.notice.say(if on.player {
                "Parried!"
            } else {
                "Your blow is parried"
            });
        }
        Taken::GuardBroken => {
            out.noises.write(Noise::at(Cue::Clash, blow.at));
            stop(fighters, [by.entity, on.entity], blow.hit_stop);
            hurt(
                fighters,
                on.entity,
                blow.damage * 0.3,
                "Hit_Head",
                BROKEN_S,
                0.6,
            );
            if on.player {
                out.notice.say("Your guard is broken");
            }
        }
        Taken::Dodged => {}
    }
}

/// Holds fighters in a hit-stop.
fn stop(fighters: &mut Query<Body<'_>>, who: [Entity; 2], seconds: f32) {
    for entity in who {
        if let Ok((.., mut fighter, _, _, _, _, _, _)) = fighters.get_mut(entity) {
            fighter.stop_s = fighter.stop_s.max(seconds);
        }
    }
}

/// Shoves a fighter along the ground for a moment, metres a second.
fn shove(fighters: &mut Query<Body<'_>>, entity: Entity, velocity: Vec3) {
    if let Ok((.., mut fighter, _, _, _, _, _, _)) = fighters.get_mut(entity) {
        fighter.shove = velocity;
        fighter.shove_s = 0.15;
    }
}

/// A fighter hurt or thrown off its feet: health lost, its move cut short, its guard down,
/// and a reaction clip for a while; or down, if its health is gone.
fn hurt(
    fighters: &mut Query<Body<'_>>,
    entity: Entity,
    damage: f32,
    clip: &str,
    seconds: f32,
    speed: f32,
) {
    let Ok((_, _, mut fighter, _, staged, acting, drawn, partner, _)) = fighters.get_mut(entity)
    else {
        return;
    };
    fighter.health = (fighter.health - damage).max(0.0);
    fighter.runner.clear();
    fighter.guard_s = None;
    let (clip, speed) = if fighter.health <= 0.0 {
        fighter.down_s = Some(0.0);
        info!(
            "Melee: {} down",
            if drawn.is_some() {
                "you are"
            } else {
                "the partner is"
            }
        );
        ("Death01", 1.0)
    } else {
        fighter.stagger_s = seconds;
        (clip, speed)
    };
    if let (Some(mut drawn), Some(mut acting)) = (drawn, acting) {
        drawn.showing = None;
        show(&mut drawn, &mut acting, clip, speed, true, 0.0, Some(80));
    }
    if let (Some(mut staged), Some(mut partner)) = (staged, partner) {
        partner.showing = Some(clip.to_string());
        staged.tell_play(clip, speed, true, 80, 0.0);
        partner.to(Beat::Recover);
    }
}

/// Once a fighter has lain down a while, the spar starts again: everyone up, whole.
pub fn again(mut fighters: Query<Body<'_>>) {
    if !fighters
        .iter()
        .any(|(_, _, fighter, ..)| fighter.down_s.is_some_and(|down| down > DOWN_S))
    {
        return;
    }
    info!("Melee: the spar starts again");
    for (_, _, mut fighter, _, staged, _, drawn, partner, _) in &mut fighters {
        fighter.health = HEALTH;
        fighter.down_s = None;
        fighter.stagger_s = 0.0;
        fighter.runner.clear();
        if let Some(mut drawn) = drawn {
            drawn.showing = None;
        }
        if let (Some(_), Some(mut partner)) = (staged, partner) {
            partner.showing = None;
            partner.to(Beat::Close);
        }
    }
}

/// A parry's flash dies away.
pub fn fade(
    mut commands: Commands,
    time: Res<Time>,
    mut flashes: Query<(Entity, &mut Flash, &mut PointLight)>,
) {
    for (entity, mut flash, mut light) in &mut flashes {
        flash.left -= time.delta_secs();
        if flash.left <= 0.0 {
            commands.entity(entity).despawn();
        } else {
            light.intensity = 400_000.0 * flash.left / FLASH_S;
        }
    }
}
