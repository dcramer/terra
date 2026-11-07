//! In a film the hero walks in, then fights: the desk's films showed him swinging on the
//! spot for 16 s, and the street's people walking in place (iso-kit j44). From the film's
//! first frame a body with a `WalkIn` walks from its `from` to its `to` at the walk clip's
//! own ground speed (`PACE`); the hero then turns to his foe over the last steps and his
//! walk gives way to his fight's clip in a short blend, and till he stands the effects hold
//! back his blow, slash and bolts (`Walking`), which are laid where he fights. A still
//! (`--shot` alone) never walks, so every still stays the same.

use super::fight::{Pose, Posed};
use crate::Args;
use bevy::diagnostic::FrameCount;
use bevy::prelude::*;

/// The knight's walk's ground speed (m/s): his planted foot slides back 0.64 to 0.67 m in
/// each 0.6 to 0.7 s stance (the GLB's feet read through its walk at 10 Hz, iso-kit j44).
pub const PACE: f32 = 1.05;
/// The turn to his foe over the walk's last seconds, or a stroller's to his way over its
/// first (s).
const TURN: f32 = 0.6;
/// The walk's blend into the fight (s).
const BLEND: f32 = 0.3;
/// Where in his fight's clip he begins once he stands: the attack's wind-up, 0.95 s before
/// the blow (hit.rs `BLOW`).
const FIGHT_FROM: f32 = 0.3;

/// The ground a walk keeps to.
#[derive(Clone, Copy)]
pub enum Floor {
    Flat(f32),
    Shape(fn(Vec2) -> f32),
}

impl Floor {
    fn at(self, p: Vec2) -> f32 {
        match self {
            Floor::Flat(y) => y,
            Floor::Shape(f) => f(p),
        }
    }
}

/// A film's walk: from `from` to `to`, then the body's `Pose` clip where the scene stood it
/// (`fight`: it must stand at `to`), or on past the film's end (a stroll: its pose is its
/// walk already).
#[derive(Component)]
pub struct WalkIn {
    pub from: Vec2,
    pub to: Vec2,
    pub floor: Floor,
    pub fight: bool,
}

/// Whether the hero is walking in now, and the game time (s) he stood at his place (0 when
/// he never walked), from which his bolts' volleys count (spell.rs).
#[derive(Resource, Default)]
pub struct Walking {
    pub now: bool,
    pub stood: f32,
}

/// A walk under way: the clips' nodes (the fight's), the body's player, and its turns.
#[derive(Component)]
struct Stride {
    walk: Option<(AnimationNodeIndex, AnimationNodeIndex)>,
    player: Entity,
    start: Quat,
    face: Quat,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Walking>()
        .add_systems(Update, (begin, walk).chain());
}

/// The film's time (s) at this frame, or none outside a film.
fn film_time(args: &Args, frame: &FrameCount) -> Option<f32> {
    (args.film > 0 && frame.0 >= args.frames)
        .then(|| (frame.0 - args.frames) as f32 / crate::FILM_FPS as f32)
}

/// Where a walk stands at time `t` (s), how far through it is (0 to 1), and its heading.
fn place(w: &WalkIn, t: f32) -> (Vec2, f32, Quat) {
    let d = w.to - w.from;
    let along = (t * PACE / d.length().max(1e-3)).clamp(0.0, 1.0);
    (w.from + d * along, along, Quat::from_rotation_y(d.x.atan2(d.y)))
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// At the film's first frame: each walker at its start, the hero on his walk.
#[allow(clippy::too_many_arguments)]
fn begin(
    mut commands: Commands,
    args: Res<Args>,
    frame: Res<FrameCount>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut walking: ResMut<Walking>,
    mut bodies: Query<(Entity, &WalkIn, &Pose, &mut Transform), (With<Posed>, Without<Stride>)>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
) {
    if film_time(&args, &frame).is_none() {
        return;
    }
    for (body, w, pose, mut tr) in &mut bodies {
        let Some(gltf) = gltfs.get(&pose.gltf) else {
            continue;
        };
        let Some(player) = children
            .iter_descendants(body)
            .find(|e| players.contains(*e))
        else {
            continue;
        };
        let (from, _, heading) = place(w, 0.0);
        let mut stride = Stride {
            walk: None,
            player,
            start: tr.rotation,
            face: tr.rotation,
        };
        if w.fight {
            let (Some(walk), Some(fight)) = (
                gltf.named_animations.get("walk"),
                gltf.named_animations.get(pose.clip),
            ) else {
                warn!("{:?} has no walk to walk in on", pose.gltf.path());
                commands.entity(body).remove::<WalkIn>();
                continue;
            };
            let (graph, nodes) = AnimationGraph::from_clips([walk.clone(), fight.clone()]);
            let mut p = players.get_mut(player).unwrap();
            p.stop_all();
            p.play(nodes[0]).repeat();
            commands
                .entity(player)
                .insert(AnimationGraphHandle(graphs.add(graph)));
            stride.walk = Some((nodes[0], nodes[1]));
            stride.start = heading;
            tr.rotation = heading;
            walking.now = true;
        }
        tr.translation = from.extend(w.floor.at(from)).xzy();
        commands.entity(body).insert(stride);
    }
}

/// Each walker along its way; the hero, once he stands, turned to his foe and into his fight.
fn walk(
    mut commands: Commands,
    args: Res<Args>,
    frame: Res<FrameCount>,
    time: Res<Time>,
    mut walking: ResMut<Walking>,
    mut bodies: Query<(Entity, &WalkIn, &Stride, &mut Transform)>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Some(t) = film_time(&args, &frame) else {
        return;
    };
    for (body, w, s, mut tr) in &mut bodies {
        let (at, _, heading) = place(w, t);
        tr.translation = at.extend(w.floor.at(at)).xzy();
        let secs = w.from.distance(w.to) / PACE;
        let Some((walk, fight)) = s.walk else {
            tr.rotation = s.start.slerp(heading, smooth(t / TURN));
            continue;
        };
        tr.rotation = heading.slerp(s.face, smooth((t - secs + TURN) / TURN));
        let b = (t - secs) / BLEND;
        let Ok(mut p) = players.get_mut(s.player) else {
            continue;
        };
        if b < 0.0 {
            continue;
        }
        if !p.is_playing_animation(fight) {
            p.play(fight).seek_to(FIGHT_FROM).repeat();
        }
        if b >= 1.0 {
            p.stop(walk);
            if let Some(a) = p.animation_mut(fight) {
                a.set_weight(1.0);
            }
            walking.now = false;
            walking.stood = time.elapsed_secs();
            commands.entity(body).remove::<(WalkIn, Stride)>();
        } else {
            if let Some(a) = p.animation_mut(walk) {
                a.set_weight(1.0 - b);
            }
            if let Some(a) = p.animation_mut(fight) {
                a.set_weight(b);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A walk goes at the clip's pace from its start and stands at its end once there.
    #[test]
    fn a_walk_keeps_pace_and_stands_at_its_end() {
        let w = WalkIn {
            from: Vec2::new(0.0, 4.0),
            to: Vec2::ZERO,
            floor: Floor::Flat(0.0),
            fight: true,
        };
        assert_eq!(place(&w, 0.0).0, w.from);
        let (one, along, _) = place(&w, 1.0);
        assert!((one.distance(w.from) - PACE).abs() < 1e-4, "{one}");
        assert!((along - PACE / 4.0).abs() < 1e-4);
        assert_eq!(place(&w, 60.0).0, w.to);
        // Heading -z, the way he goes (the models face +Z).
        let fwd = place(&w, 0.0).2 * Vec3::Z;
        assert!(fwd.distance(Vec3::NEG_Z) < 1e-4, "{fwd}");
    }
}
