//! Which of the game's clips shows what the hero does, played as the game's figure plays them
//! (terra_walker::figure): his guard (`Low_Guard`, the set's `guard`) when he stands, the
//! walk, jog and sprint loops by his speed at their gait's rate (`pace::gait`, D585) under the
//! guard's arms (`layer::lay`, the stance's hold), a loop taking over from a loop at the same
//! step (`feet::markers`) by a crossfade, every other change inertialized (D631); a move's
//! clip whole, at the runner's own time, so a hit-stop and the block's hold show as they are.

use super::moves::Doing;
use super::{GameBody, Kit};
use bevy::prelude::*;
use std::collections::HashMap;
use std::time::Duration;
use terra_motion::feet::Markers;
use terra_motion::gaits::{JOG, SPRINT_FROM, WALK};
use terra_walker::inertia::Inertia;
use terra_walker::layer::{self, Held, Nodes};
use terra_walker::pace;

/// A loop to a loop crossfades over this, in step; every other change is inertialized over it.
const BLEND: Duration = Duration::from_millis(200);
/// Below this speed (m/s) he stands on guard.
const STANDS: f32 = 0.15;

/// His clips on his player and what plays.
#[derive(Component)]
pub struct Rig {
    player: Entity,
    // terra_walker's layer graph keys its clips in Bevy's map, the rest in std's.
    nodes: bevy::platform::collections::HashMap<String, Nodes>,
    paces: HashMap<String, f32>,
    markers: HashMap<String, Markers>,
    holds: Vec<Held>,
    /// The clip playing, its copy, and the move count it started at.
    playing: Option<(String, usize, u32)>,
}

/// The loop his speed shows: his guard standing, else the walk, jog or sprint by the speed
/// between theirs.
pub fn stride(speed: f32) -> &'static str {
    if speed < STANDS {
        "Low_Guard"
    } else if speed < (WALK + JOG) / 2.0 {
        "Walk_Loop"
    } else if speed < SPRINT_FROM {
        "Jog_Fwd_Loop"
    } else {
        "Sprint_Loop"
    }
}

/// Once his body's bones and clips are in: the body file's graph on his player.
pub fn rig(
    mut commands: Commands,
    kit: Res<Kit>,
    gltfs: Res<Assets<Gltf>>,
    gltf_nodes: Res<Assets<bevy::gltf::GltfNode>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    bodies: Query<Entity, (With<GameBody>, Without<Rig>)>,
    children: Query<&Children>,
    players: Query<(), With<AnimationPlayer>>,
) {
    let Some(gltf) = gltfs.get(&kit.gltf) else {
        return;
    };
    for body in &bodies {
        let Some(player) = children
            .iter_descendants(body)
            .find(|e| players.contains(*e))
        else {
            continue;
        };
        let (graph, nodes) = layer::graph(gltf, &gltf_nodes);
        for clip in [
            kit.set.guard.as_str(),
            "Walk_Loop",
            "Jog_Fwd_Loop",
            "Sprint_Loop",
        ] {
            if !nodes.contains_key(clip) {
                warn!(
                    "{} has no clip {clip}: the hero cannot show it",
                    super::BODY
                );
            }
        }
        commands.entity(player).insert((
            AnimationGraphHandle(graphs.add(graph)),
            AnimationTransitions::new(),
            Inertia::default(),
        ));
        commands.entity(body).insert(Rig {
            player,
            paces: pace::paces(gltf, &gltf_nodes),
            markers: terra_walker::feet::markers(gltf, &gltf_nodes),
            nodes,
            holds: Vec::new(),
            playing: None,
        });
    }
}

/// Plays the clip for what he does, at its rate or the move's time.
pub fn show(
    kit: Res<Kit>,
    mut heroes: Query<(&Doing, &mut Rig)>,
    mut players: Query<(
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &mut Inertia,
    )>,
) {
    let set = &kit.set;
    for (doing, mut rig) in &mut heroes {
        let rig = &mut *rig;
        let Ok((mut p, mut tx, mut inertia)) = players.get_mut(rig.player) else {
            continue;
        };
        let now = doing.runner.now(set);
        let clip = now.map_or_else(|| stride(doing.speed), |m| m.clip.as_str());
        let started = if now.is_some() { doing.started } else { 0 };
        let Some(nodes) = rig.nodes.get(clip).copied() else {
            continue;
        };
        let guard = set.guard.as_str();
        // A stride loop plays under the guard's arms; the guard and every move whole.
        let held = now.is_none() && clip != guard;
        let changed = rig
            .playing
            .as_ref()
            .is_none_or(|(c, _, s)| c != clip || *s != started);
        if changed {
            let copy = match &rig.playing {
                Some((c, copy, _)) if c == clip => 1 - copy,
                _ => 0,
            };
            // A stride loop after a stride loop goes on at the same step.
            let synced = rig.playing.as_ref().and_then(|(from, ..)| {
                let (Some(_), Some(_)) = (rig.paces.get(from.as_str()), rig.paces.get(clip)) else {
                    return None;
                };
                let t = p.animation(tx.get_main_animation()?)?.seek_time();
                rig.markers
                    .get(clip)?
                    .synced(rig.markers.get(from.as_str())?, t)
            });
            let node = nodes.of(copy, held);
            let fade = inertia.play(synced.is_some(), BLEND);
            let playing = tx.play(&mut p, node, fade);
            match now {
                Some(m) => {
                    playing.seek_to(m.clip_at(doing.runner.t())).set_speed(0.0);
                }
                None => {
                    playing.repeat();
                    if let Some(at) = synced {
                        playing.seek_to(at);
                    }
                }
            }
            if held && let Some(g) = rig.nodes.get(guard) {
                layer::lay(&mut rig.holds, &mut p, g.over, node);
            }
            rig.playing = Some((clip.to_string(), copy, started));
        }
        let Some(main) = tx.get_main_animation() else {
            continue;
        };
        if let Some(a) = p.animation_mut(main) {
            match now {
                // The move's clip at the runner's own time: held still by a hit-stop or the
                // block's hold, as the runner is.
                Some(m) => {
                    a.seek_to(m.clip_at(doing.runner.t()));
                }
                None if held => {
                    let pace = rig.paces.get(clip).copied().unwrap_or(0.0);
                    a.set_speed(pace::gait(doing.speed, pace, 1.0).rate);
                }
                None => {}
            }
        }
        layer::ride(&mut rig.holds, &mut p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_motion::gaits::SPRINT;

    /// Each gait's own speed shows its own loop, and standing shows the guard.
    #[test]
    fn each_speed_shows_its_gait() {
        assert_eq!(stride(0.0), "Low_Guard");
        assert_eq!(stride(WALK), "Walk_Loop");
        assert_eq!(stride(JOG), "Jog_Fwd_Loop");
        assert_eq!(stride(SPRINT), "Sprint_Loop");
    }
}
