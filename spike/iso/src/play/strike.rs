//! The hero's blows landing in play, sold as the staged hit sells its one (ruin/effects/
//! hit.rs): each struck body lit white then red (the hit flash, on its own copies of its
//! materials: a glTF's are shared by every copy of the model, so the staged ogre's would
//! light every ogre), thrown back a few degrees and recovering (the flinch), its hit clip
//! played and then its own again; the `BLOWS`th blow plays its death, held. The blow's
//! sparks, flash, lights and dust are the biome's presets (kit/effects.rs, `Cues::blow`),
//! moved to where the blade meets the body (`Cues::contact`). Nothing here runs without
//! `--play`, so the staged scenes are as before.

use super::{Fallen, Hero};
use crate::kit::effects::{Contact, Cues};
use crate::ruin::effects::hit::{FLINCH, flash, flinch};
use crate::ruin::effects::slash::Blade;
use crate::ruin::fight::Pose;
use bevy::prelude::*;
use std::time::Duration;

/// The blows a body takes before it falls: two hits, then its death.
pub const BLOWS: u32 = 3;
/// How far in from a body's middle the blade bites it (m), and the bite's height when the
/// blade's path is unknown (no sword baked).
const RADIUS: f32 = 0.4;
const HEIGHT: f32 = 1.3;
/// Into a hit or death, and back to its own clip after the hit.
const INTO: Duration = Duration::from_millis(60);
const BACK: Duration = Duration::from_millis(250);

/// A blow landing on a body this frame: where the hero stood, and the attack clip's time
/// of the blow (control in play.rs inserts it).
#[derive(Component)]
pub struct Blow {
    pub from: Vec2,
    pub t: f32,
}

/// A body the hero has struck: the blows it took, the last one's time, its rest rotation,
/// the way it was struck, its own materials and its clips on its own graph.
#[derive(Component)]
pub struct Struck {
    pub blows: u32,
    at: f32,
    base: Quat,
    away: Vec3,
    skin: Vec<Handle<StandardMaterial>>,
    player: Entity,
    own: Option<AnimationNodeIndex>,
    hit: Option<AnimationNodeIndex>,
    death: Option<AnimationNodeIndex>,
}

/// What a body does on its `n`th blow (from 1).
#[derive(Debug, PartialEq)]
pub enum React {
    Hit,
    Death,
    /// No clip for it: the flash and flinch alone.
    Flinch,
}

pub fn react(n: u32, hit: bool, death: bool) -> React {
    if n >= BLOWS && death {
        React::Death
    } else if hit {
        React::Hit
    } else {
        React::Flinch
    }
}

/// Where a blow from `hero` bites the body standing at `foe` (ground points): on its near
/// side, `RADIUS` in from its middle, at the height where the blade's edge (`edge`, its
/// inner and outer points in the world) passes nearest the body's axis.
pub fn bite(hero: Vec2, foe: Vec2, edge: Option<(Vec3, Vec3)>, floor: f32) -> Vec3 {
    let d = foe - hero;
    let toward = d.normalize_or(Vec2::X);
    let at = foe - toward * RADIUS.min(d.length() / 2.0);
    let y = edge.map_or(floor + HEIGHT, |(a, b)| {
        let ab = (b - a).xz();
        let k = ((foe - a.xz()).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
        (a.y + (b.y - a.y) * k).clamp(floor + 0.4, floor + 2.2)
    });
    Vec3::new(at.x, y, at.y)
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (land, recover, show).chain().after(super::control));
}

fn player_of(
    body: Entity,
    children: &Query<&Children>,
    players: &Query<(&mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
) -> Option<Entity> {
    children
        .iter_descendants(body)
        .find(|e| players.contains(*e))
}

/// Each blow: the body's first takes its materials and clips over; then its reaction, and
/// the blow's effects cued at the bite.
#[allow(clippy::too_many_arguments)]
fn land(
    mut commands: Commands,
    time: Res<Time>,
    (gltfs, mut graphs, mut materials): (
        Res<Assets<Gltf>>,
        ResMut<Assets<AnimationGraph>>,
        ResMut<Assets<StandardMaterial>>,
    ),
    mut blown: Query<(Entity, &Blow, &Pose, &Transform, Option<&mut Struck>)>,
    children: Query<&Children>,
    skins: Query<&MeshMaterial3d<StandardMaterial>>,
    mut players: Query<(&mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
    (blade, hero, cues): (
        Option<Res<Blade>>,
        Query<&GlobalTransform, With<Hero>>,
        Option<ResMut<Cues>>,
    ),
) {
    let now = time.elapsed_secs();
    let mut contact = None;
    for (body, blow, pose, tr, struck) in &mut blown {
        // A struck body stops walking in (walk.rs).
        commands
            .entity(body)
            .remove::<(Blow, crate::ruin::walk::WalkIn)>();
        let foe = tr.translation.xz();
        let away = (foe - blow.from).normalize_or(Vec2::X);
        let edge = blade
            .as_ref()
            .and_then(|b| b.edge(blow.t))
            .zip(hero.single().ok())
            .map(|((i, o), g)| (g.transform_point(i), g.transform_point(o)));
        contact = Some(Contact {
            point: bite(blow.from, foe, edge, tr.translation.y),
            floor: tr.translation.y,
            toward: away,
        });
        // The first blow takes the body over; its transitions are inserted with it.
        let mut fresh = None;
        let s = match struck {
            Some(s) => s.into_inner(),
            None => {
                let Some(n) = take_over(
                    body,
                    pose,
                    tr.rotation,
                    (&gltfs, &mut graphs, &mut materials),
                    &children,
                    &skins,
                    &mut commands,
                    &players,
                ) else {
                    continue;
                };
                fresh.insert(n)
            }
        };
        s.blows += 1;
        s.at = now;
        s.away = away.extend(0.0).xzy();
        let Ok((mut p, tx)) = players.get_mut(s.player) else {
            continue;
        };
        let mut new = AnimationTransitions::new();
        let fell = match tx {
            Some(mut tx) => start(s, &mut p, &mut tx),
            None => {
                p.stop_all();
                let fell = start(s, &mut p, &mut new);
                commands.entity(s.player).insert(new);
                fell
            }
        };
        if fell {
            commands.entity(body).insert(Fallen);
        }
        if let Some(n) = fresh {
            commands.entity(body).insert(n);
        }
    }
    if let (Some(c), Some(mut cues)) = (contact, cues) {
        cues.blow = now;
        cues.contact = Some(c);
    }
}

/// Plays a struck body's reaction to its latest blow; whether it fell.
fn start(s: &Struck, p: &mut AnimationPlayer, tx: &mut AnimationTransitions) -> bool {
    match react(s.blows, s.hit.is_some(), s.death.is_some()) {
        React::Death => {
            tx.play(p, s.death.unwrap(), INTO);
            true
        }
        React::Hit => {
            tx.play(p, s.hit.unwrap(), INTO);
            false
        }
        React::Flinch => false,
    }
}

/// A body's first blow: its clips (its own, hit, death) on a graph of its own, and its
/// materials copied, each lit by its own albedo when it flashes (as hit.rs's ogre).
#[allow(clippy::too_many_arguments)]
fn take_over(
    body: Entity,
    pose: &Pose,
    base: Quat,
    (gltfs, graphs, materials): (
        &Assets<Gltf>,
        &mut Assets<AnimationGraph>,
        &mut Assets<StandardMaterial>,
    ),
    children: &Query<&Children>,
    skins: &Query<&MeshMaterial3d<StandardMaterial>>,
    commands: &mut Commands,
    players: &Query<(&mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
) -> Option<Struck> {
    let gltf = gltfs.get(&pose.gltf)?;
    let player = player_of(body, children, players)?;
    let mut graph = AnimationGraph::new();
    let root = graph.root;
    let mut node = |n: &str| {
        gltf.named_animations
            .get(n)
            .map(|c| graph.add_clip(c.clone(), 1.0, root))
    };
    let (own, hit, death) = (node(pose.clip), node("hit"), node("death"));
    commands
        .entity(player)
        .insert(AnimationGraphHandle(graphs.add(graph)));
    let mut copies: Vec<(AssetId<StandardMaterial>, Handle<StandardMaterial>)> = Vec::new();
    for e in children.iter_descendants(body) {
        let Ok(m) = skins.get(e) else { continue };
        let copy = match copies.iter().find(|c| c.0 == m.0.id()) {
            Some(c) => c.1.clone(),
            None => {
                let Some(mut mat) = materials.get(&m.0).cloned() else {
                    continue;
                };
                if mat.emissive_texture.is_none() {
                    mat.emissive_texture = mat.base_color_texture.clone();
                }
                mat.emissive = LinearRgba::BLACK;
                let h = materials.add(mat);
                copies.push((m.0.id(), h.clone()));
                h
            }
        };
        commands.entity(e).insert(MeshMaterial3d(copy));
    }
    Some(Struck {
        blows: 0,
        at: -10.0,
        base,
        away: Vec3::X,
        skin: copies.into_iter().map(|c| c.1).collect(),
        player,
        own,
        hit,
        death,
    })
}

/// A body back on its own clip once its hit has played.
fn recover(
    bodies: Query<&Struck, Without<Fallen>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for s in &bodies {
        let (Some(hit), Some(own)) = (s.hit, s.own) else {
            continue;
        };
        let Ok((mut p, mut tx)) = players.get_mut(s.player) else {
            continue;
        };
        if tx.get_main_animation() == Some(hit) && p.animation(hit).is_none_or(|a| a.is_finished())
        {
            tx.play(&mut p, own, BACK).repeat();
        }
    }
}

/// The flash and flinch since each body's last blow.
fn show(
    time: Res<Time>,
    mut bodies: Query<(&Struck, &mut Transform)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let now = time.elapsed_secs();
    for (s, mut tr) in &mut bodies {
        let a = now - s.at;
        let f = flinch(a);
        tr.rotation =
            Quat::from_axis_angle(Vec3::Y.cross(s.away).normalize_or(Vec3::X), FLINCH * f) * s.base;
        let e = flash(a);
        for h in &s.skin {
            if materials.get(h).is_some_and(|m| m.emissive != e)
                && let Some(mut m) = materials.get_mut(h)
            {
                m.emissive = e;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two hits, then the death on the third blow; a body without a hit clip only flinches
    /// until it dies, and one without a death never falls.
    #[test]
    fn three_blows_fell_a_body() {
        let seq: Vec<_> = (1..=BLOWS).map(|n| react(n, true, true)).collect();
        assert_eq!(seq, [React::Hit, React::Hit, React::Death]);
        assert_eq!(react(1, false, true), React::Flinch);
        assert_eq!(react(BLOWS, false, true), React::Death);
        assert_eq!(react(BLOWS + 2, true, false), React::Hit);
    }

    /// The bite is on the body's near side, toward the hero, at the height where the blade
    /// passes its axis, kept on the body (between the knees and over the head).
    #[test]
    fn the_bite_is_where_the_blade_meets_the_body() {
        let (hero, foe) = (Vec2::ZERO, Vec2::new(0.0, 2.0));
        // The blade level across the body's axis at 1.1 m, its outer half past it.
        let edge = (Vec3::new(-1.0, 1.1, 2.0), Vec3::new(1.0, 1.1, 2.0));
        let b = bite(hero, foe, Some(edge), 0.0);
        assert!(b.distance(Vec3::new(0.0, 1.1, 2.0 - RADIUS)) < 1e-5, "{b}");
        // Sloping from 2 m at the hilt to 0 m at the tip, halfway across at the axis.
        let edge = (Vec3::new(-1.0, 2.0, 2.0), Vec3::new(1.0, 0.0, 2.0));
        assert!((bite(hero, foe, Some(edge), 0.5).y - 1.0).abs() < 1e-5);
        // Over the head and unknown.
        let high = (Vec3::new(-1.0, 5.0, 2.0), Vec3::new(1.0, 5.0, 2.0));
        assert!((bite(hero, foe, Some(high), 0.0).y - 2.2).abs() < 1e-5);
        assert!((bite(hero, foe, None, 0.3).y - (0.3 + HEIGHT)).abs() < 1e-5);
        // A body pressed against him: the bite stays between them.
        let b = bite(hero, Vec2::new(0.0, 0.3), None, 0.0);
        assert!(b.z > 0.0 && b.z < 0.3, "{b}");
    }
}
