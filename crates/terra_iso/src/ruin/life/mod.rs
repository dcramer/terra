//! The ruin's ambient life beside the fight (the developer's "multiple monsters and
//! animals, living nature"): spiders scuttling over the walls' faces that run from the
//! fight's blows (spiders.rs), pigeons on the wall tops that take off and circle at them
//! (birds.rs). The creatures are the repository's own animals (assets/models/creatures,
//! not the Tripo pipeline), each playing its own clips, a fixed pool spawned at the start.
//! Each hears the knight's blows land (`Blows`) and goes through a short script from that
//! moment, so a still is one moment of it and a film plays it.

mod birds;
mod spiders;

use super::effects::kit::Clock;
use super::fight::Pose;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.init_resource::<Blows>()
        .add_systems(Startup, (spiders::spawn, birds::spawn))
        .add_systems(Update, (hear, (spiders::go, birds::go), gait).chain());
}

/// When in the knight's attack his blade meets the ogre (effects::hit's `BLOW`, private to
/// it there): the creatures start at the sound of it.
const BLOW: f32 = 1.25;

/// The knight's blows as the creatures hear them: how many have landed and the clock's
/// time of the last.
#[derive(Resource, Default)]
pub struct Blows {
    seek: Option<f32>,
    pub count: u32,
    pub last: f32,
}

/// Counts a blow each time the knight's attack clip passes `BLOW`, as effects::hit does.
fn hear(
    clock: Res<Clock>,
    mut blows: ResMut<Blows>,
    bodies: Query<(Entity, &Pose)>,
    children: Query<&Children>,
    players: Query<&AnimationPlayer>,
) {
    let knight = bodies.iter().find(|(_, p)| {
        p.gltf
            .path()
            .is_some_and(|path| path.to_string().contains("knight"))
    });
    let Some((knight, _)) = knight else {
        return;
    };
    let Some(seek) = children
        .iter_descendants(knight)
        .find_map(|e| players.get(e).ok())
        .and_then(|p| p.playing_animations().next().map(|(_, a)| a.seek_time()))
    else {
        return;
    };
    let Some(last) = blows.seek.replace(seek) else {
        return;
    };
    let crossed = (last < BLOW && seek >= BLOW) || (seek < last && (last < BLOW || seek >= BLOW));
    if crossed {
        blows.count += 1;
        blows.last = clock.t;
    }
}

/// A creature's two clips, one while it moves and one while it rests, and which it plays:
/// the moving clip at `speed` (its gait matched to its pace), each from `phase` in.
#[derive(Component)]
pub struct Gait {
    pub gltf: Handle<Gltf>,
    pub clips: [&'static str; 2],
    pub moving: bool,
    pub speed: f32,
    pub phase: f32,
    shown: Option<bool>,
    nodes: Option<(Entity, [AnimationNodeIndex; 2])>,
}

impl Gait {
    pub fn new(gltf: Handle<Gltf>, clips: [&'static str; 2], phase: f32) -> Self {
        Self {
            gltf,
            clips,
            moving: false,
            speed: 1.0,
            phase,
            shown: None,
            nodes: None,
        }
    }
}

/// Plays each creature's clip for what it is doing, held in a still as the fight's are.
fn gait(
    mut commands: Commands,
    clock: Res<Clock>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut bodies: Query<(Entity, &mut Gait)>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
) {
    for (body, mut g) in &mut bodies {
        if g.nodes.is_none() {
            let Some(gltf) = gltfs.get(&g.gltf) else {
                continue;
            };
            let Some(player) = children
                .iter_descendants(body)
                .find(|e| players.contains(*e))
            else {
                continue;
            };
            let clip = |name: &str| {
                gltf.named_animations.get(name).cloned().unwrap_or_else(|| {
                    let have: Vec<_> = gltf.named_animations.keys().collect();
                    warn!("no clip {name} in {:?}; it has {have:?}", g.gltf.path());
                    gltf.animations[0].clone()
                })
            };
            let (graph, nodes) = AnimationGraph::from_clips([clip(g.clips[0]), clip(g.clips[1])]);
            commands
                .entity(player)
                .insert(AnimationGraphHandle(graphs.add(graph)));
            g.nodes = Some((player, [nodes[0], nodes[1]]));
        }
        let Some((player, nodes)) = g.nodes else {
            continue;
        };
        let Ok(mut p) = players.get_mut(player) else {
            continue;
        };
        let node = nodes[if g.moving { 0 } else { 1 }];
        if g.shown != Some(g.moving) {
            p.stop_all();
            p.play(node).repeat().seek_to(g.phase);
            g.shown = Some(g.moving);
        }
        let speed = match (clock.still, g.moving) {
            (true, _) => 0.0,
            (false, true) => g.speed,
            (false, false) => 1.0,
        };
        if let Some(a) = p.animation_mut(node) {
            a.set_speed(speed);
        }
    }
}

/// A piecewise straight script of how far along its way a creature is (0 to 1) against the
/// time since it started (s): before the first key and after the last it is at their ends.
pub fn along(keys: &[(f32, f32)], tau: f32) -> f32 {
    let last = keys[keys.len() - 1];
    if tau >= last.0 {
        return last.1;
    }
    for w in keys.windows(2) {
        let ((t0, a0), (t1, a1)) = (w[0], w[1]);
        if tau < t1 {
            let k = ((tau - t0) / (t1 - t0)).clamp(0.0, 1.0);
            return a0 + (a1 - a0) * k;
        }
    }
    last.1
}

/// Which way along its way it heads at `tau`: the way it moves, or while it rests the way
/// it last moved (back toward the start before it has moved at all).
pub fn heading(keys: &[(f32, f32)], tau: f32) -> f32 {
    let mut sign = -1.0;
    for w in keys.windows(2) {
        let ((t0, a0), (_, a1)) = (w[0], w[1]);
        if t0 > tau {
            break;
        }
        if a1 != a0 {
            sign = (a1 - a0).signum();
        }
    }
    sign
}

/// A point `frac` (0 to 1) of the way along a polyline, by length, and the polyline's length.
pub fn point(path: &[Vec2], frac: f32) -> (Vec2, f32) {
    let len: f32 = path.windows(2).map(|w| w[0].distance(w[1])).sum();
    let mut left = frac.clamp(0.0, 1.0) * len;
    for w in path.windows(2) {
        let d = w[0].distance(w[1]);
        if left <= d {
            return (w[0].lerp(w[1], left / d.max(1e-6)), len);
        }
        left -= d;
    }
    (path[path.len() - 1], len)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A script runs straight between its keys and holds its ends.
    #[test]
    fn a_script_runs_between_its_keys() {
        let keys = [(0.0, 0.0), (1.0, 1.0), (2.0, 1.0), (4.0, 0.0)];
        assert_eq!(along(&keys, -1.0), 0.0);
        assert!((along(&keys, 0.5) - 0.5).abs() < 1e-6);
        assert_eq!(along(&keys, 1.5), 1.0);
        assert!((along(&keys, 3.0) - 0.5).abs() < 1e-6);
        assert_eq!(along(&keys, 9.0), 0.0);
        assert_eq!(heading(&keys, 0.5), 1.0);
        assert_eq!(heading(&keys, 1.5), 1.0);
        assert_eq!(heading(&keys, 3.0), -1.0);
        assert_eq!(heading(&keys, -1.0), -1.0);
    }
}
