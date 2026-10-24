//! What a blow shows when its blade reaches a body (D405): each fighter's sword, from hilt to
//! tip in its hand socket's frame, is swept from last tick to this one through its blow's
//! active frames against the other's hurt capsule, once a blow, and judged, by
//! `terra_motion`'s rules (`blade`, `moves::judge`, `contact::outcome`). Here: the reaction
//! clips, the swing and what the blow met as noises (`struck`: the sound is chosen from that
//! in data, docs/audio.md), the parry's flash, and what the player is told. Every outcome
//! is logged as `Melee: ...` with the scripted run's frame (D69).

use super::partner::{Beat, Partner};
use super::{DOWN_S, Drawn, Fighter, HEALTH, Set, show};
use crate::staged::Staged;
use bevy::prelude::*;
use terra_motion::blade::{self, Edge, HILT_M, TIP_M};
use terra_motion::contact::{self as rules, Effect, Reel, Taking};
use terra_motion::moves::{self, Phase, Taken};
use terra_motion::stamina;
use terra_motion::struck::Struck;
use terra_survival::noise::{Cue, Noise};
use terra_survival::told::Notice;
use terra_walker::figure::Acting;
use terra_walker::{Motion, Player, Stamina};
use terra_world::shell::ScriptFrame;

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
    Option<&'a mut Stamina>,
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
    /// In a blow's active frames, and whether it is heavy: its swing is heard as they open.
    swinging: Option<bool>,
    guard_s: Option<f32>,
    player: bool,
    /// Stamina left to hold its guard with; a body without any never tires.
    stamina: f32,
}

/// A blow that reached a body.
struct Landed {
    by: usize,
    on: usize,
    at: Vec3,
    taken: Taken,
    damage: f32,
    hit_stop: f32,
    /// Stamina the blow takes from the struck's guard.
    charge: f32,
    /// What it met, as it is heard; nothing when dodged.
    struck: Option<Struck>,
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
    for (entity, place, fighter, motion, staged, _, _, _, player, pool) in &fighters {
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
            open: now.is_some_and(|each| each.phase(t) == Phase::Active && each.damage > 0.0)
                && !fighter.runner.landed,
            invulnerable: now.is_some_and(|each| each.invulnerable(t)),
            swinging: now
                .filter(|each| each.phase(t) == Phase::Active && each.damage > 0.0)
                .map(|each| each.heavy),
            guard_s: fighter.guard_s,
            player,
            stamina: pool.map_or(f32::MAX, |pool| pool.left),
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
            let facing = rules::guards(body.facing, from);
            let Ok((_, _, fighter, ..)) = fighters.get(attacker.entity) else {
                continue;
            };
            let Some(each) = fighter.runner.now(set) else {
                continue;
            };
            let taken = moves::judge(
                body.invulnerable,
                body.guard_s,
                facing,
                each.heavy,
                set.parry_s,
            );
            // A tired blow is weaker; a guard with too little stamina for it breaks.
            let damage = each.damage * fighter.runner.might(set);
            let (taken, charge) = stamina::guarded(set, taken, damage, body.stamina);
            // Both fight with the set's kit; the struck body is its own.
            let struck = fighters
                .get(body.entity)
                .ok()
                .and_then(|(_, _, on, ..)| Struck::of(taken, each, &set.kit, &set.kit, on.body));
            landed.push(Landed {
                by,
                on,
                at,
                taken,
                damage,
                hit_stop: each.hit_stop,
                charge,
                struck,
            });
            break;
        }
    }
    for (index, each) in seen.iter().enumerate() {
        if let Ok((_, _, mut fighter, ..)) = fighters.get_mut(each.entity) {
            fighter.was = each.edge;
            // The swing heard once a blow, as its active frames open, from the blade.
            if let (Some(heavy), false) = (each.swinging, fighter.swung)
                && let Some(edge) = each.edge
            {
                noises.write(Noise::at(
                    Cue::Swing { heavy },
                    edge.tip.lerp(edge.hilt, 0.5),
                ));
            }
            fighter.swung = each.swinging.is_some();
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
        "Melee: frame {frame}: {} {:?} by {} ({} damage, {:.0} stamina, {:.2} m apart)",
        who(on.player),
        blow.taken,
        who(by.player),
        blow.damage,
        blow.charge,
        (on.feet - by.feet)
            .reject_from(on.feet.normalize())
            .length()
    );
    let result = rules::outcome(blow.taken, blow.damage, blow.hit_stop);
    if blow.charge > 0.0
        && let Ok((.., Some(mut pool))) = fighters.get_mut(on.entity)
    {
        pool.spend(blow.charge);
    }
    for entity in [by.entity, on.entity] {
        if let Ok((_, _, mut fighter, ..)) = fighters.get_mut(entity) {
            fighter.stop(result.stop_s);
        }
    }
    hurt(fighters, on.entity, result.on, away);
    hurt(fighters, by.entity, result.by, away);
    if let Some(struck) = blow.struck {
        out.noises.write(Noise::at(Cue::Struck(struck), blow.at));
    }
    match blow.taken {
        Taken::Parried => {
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
        Taken::GuardBroken if on.player => out.notice.say("Your guard is broken"),
        _ => {}
    }
}

/// The clip a reel shows, and how fast.
fn reel_clip(reel: Reel) -> (&'static str, f32) {
    match reel {
        Reel::Flinch => ("Hit_Chest", 1.0),
        Reel::Stagger => ("Hit_Head", 0.35),
        Reel::Broken => ("Hit_Head", 0.6),
    }
}

/// A fighter takes a blow's effect (`terra_motion`'s rules: shoved, or hurt, its move cut
/// short and its guard down, reeling or down) and shows it.
fn hurt(fighters: &mut Query<Body<'_>>, entity: Entity, effect: Effect, away: Vec3) {
    let Ok((_, _, mut fighter, _, staged, acting, drawn, partner, ..)) = fighters.get_mut(entity)
    else {
        return;
    };
    let Some(taking) = fighter.take(effect, away) else {
        return;
    };
    let (clip, speed) = match taking {
        Taking::Down => {
            info!(
                "Melee: {} down",
                if drawn.is_some() {
                    "you are"
                } else {
                    "the partner is"
                }
            );
            ("Death01", 1.0)
        }
        Taking::Reel(reel) => reel_clip(reel),
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
    for (_, _, mut fighter, _, staged, _, drawn, partner, ..) in &mut fighters {
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
