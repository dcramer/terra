//! A body lifted by its size where it meets the floor (docs/characters.md, phase 4). The
//! clips are made on the mannequin; a thicker body rolling or kneeling would go into the floor
//! by however much thicker it is. The tools measure, for each clip, each frame and each morph,
//! how much more than the base body it needs lifting to meet the floor as the mannequin does
//! (`tools/characters/lift.py`); the base's own lift is in its clips already. This adds the
//! mix of the morphs' lifts, by the body's sliders, to the skeleton's own node after the
//! animation: nothing animates that node, so nothing else that writes the joints is disturbed.
//!
//! The curves are in the body file's glTF extras: `{"lifts": {"fps": 30, "clips": {"Roll":
//! {"first": 1, "heavy": [0.0, 0.004, ...]}}}}`, one value a frame from frame `first`.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::{
    gltf::{Gltf, GltfExtras},
    prelude::*,
    world_serialization::WorldInstanceReady,
};
use serde::Deserialize;

use crate::appearance::Appearance;
use crate::character::Body;

/// The lifts a body file carries, for each clip.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Lifts {
    pub fps: f32,
    pub clips: HashMap<String, ClipLifts>,
}

/// One clip's lifts: from frame `first`, one value a frame for each morph.
#[derive(Debug, Deserialize, PartialEq)]
pub struct ClipLifts {
    pub first: f32,
    #[serde(flatten)]
    pub morphs: HashMap<String, Vec<f32>>,
}

impl Lifts {
    /// How much a body with `morphs` (name and weight) is lifted `at` seconds into `clip`.
    pub fn at(&self, clip: &str, at: f32, morphs: &[(String, f32)]) -> f32 {
        let Some(lifts) = self.clips.get(clip) else {
            return 0.0;
        };
        let frame = at * self.fps - lifts.first;
        crate::appearance::mix(morphs, |name| Some(sample(lifts.morphs.get(name)?, frame)))
    }
}

/// `curve` at `frame`, between its frames, held at its ends.
fn sample(curve: &[f32], frame: f32) -> f32 {
    let Some(last) = curve.len().checked_sub(1) else {
        return 0.0;
    };
    let at = frame.clamp(0.0, last as f32);
    let below = at.floor() as usize;
    let above = (below + 1).min(last);
    let t = at - below as f32;
    curve[below] * (1.0 - t) + curve[above] * t
}

/// On a body whose file carries lifts: the skeleton's node they move, where it rests, and
/// the curves (shared by every body from the same file).
#[derive(Component)]
pub struct Lifted {
    node: Entity,
    rest: Vec3,
    lifts: Arc<Lifts>,
    player: Option<Entity>,
}

/// Each body file's lifts, parsed once for every body made from it.
#[derive(Resource, Default)]
struct Parsed(HashMap<String, Arc<Lifts>>);

/// Each clip's name, from the glTF files it came in.
#[derive(Resource, Default)]
struct ClipNames(HashMap<AssetId<AnimationClip>, String>);

pub fn plugin(app: &mut App) {
    app.init_resource::<Parsed>()
        .init_resource::<ClipNames>()
        .add_observer(find_lifts)
        .add_systems(Update, name_clips)
        .configure_sets(PostUpdate, crate::fitted())
        .add_systems(
            PostUpdate,
            lift_bodies
                .in_set(crate::Fitted)
                .in_set(crate::pose::Layer::Lift),
        );
}

/// When a body's model is in: the node carrying its file's lifts, if it has any.
fn find_lifts(
    ready: On<WorldInstanceReady>,
    bodies: Query<(), With<Body>>,
    children: Query<&Children>,
    extras: Query<&GltfExtras>,
    transforms: Query<&Transform>,
    mut parsed: ResMut<Parsed>,
    mut commands: Commands,
) {
    if !bodies.contains(ready.entity) {
        return;
    }
    for each in children.iter_descendants(ready.entity) {
        let Ok(extra) = extras.get(each) else {
            continue;
        };
        if !extra.value.contains("\"lifts\"") {
            continue;
        }
        let lifts = match parsed.0.get(&extra.value) {
            Some(lifts) => lifts.clone(),
            None => {
                let value: serde_json::Value = match serde_json::from_str(&extra.value) {
                    Ok(value) => value,
                    Err(error) => {
                        warn!("lift: a body's extras did not read: {error}");
                        return;
                    }
                };
                let Some(lifts) = value
                    .get("lifts")
                    .and_then(|v| serde_json::from_value::<Lifts>(v.clone()).ok())
                else {
                    warn!("lift: a body's lifts did not read");
                    return;
                };
                let lifts = Arc::new(lifts);
                parsed.0.insert(extra.value.clone(), lifts.clone());
                lifts
            }
        };
        let rest = transforms.get(each).map_or(Vec3::ZERO, |t| t.translation);
        commands.entity(ready.entity).insert(Lifted {
            node: each,
            rest,
            lifts,
            player: None,
        });
        return;
    }
}

/// Learns each clip's name as its glTF file loads.
fn name_clips(
    mut events: MessageReader<AssetEvent<Gltf>>,
    gltfs: Res<Assets<Gltf>>,
    mut names: ResMut<ClipNames>,
) {
    for event in events.read() {
        if let AssetEvent::LoadedWithDependencies { id } | AssetEvent::Added { id } = event
            && let Some(gltf) = gltfs.get(*id)
        {
            for (name, clip) in &gltf.named_animations {
                names.0.insert(clip.id(), name.to_string());
            }
        }
    }
}

/// Each lifted body's skeleton raised by the mix of its morphs' lifts in what it plays.
fn lift_bodies(
    mut bodies: Query<(Entity, &mut Lifted, Option<&Appearance>)>,
    children: Query<&Children>,
    players: Query<(&AnimationPlayer, &AnimationGraphHandle)>,
    graphs: Res<Assets<AnimationGraph>>,
    names: Res<ClipNames>,
    mut transforms: Query<&mut Transform>,
) {
    for (body, mut lifted, appearance) in &mut bodies {
        if lifted.player.is_none() {
            lifted.player = children
                .iter_descendants(body)
                .find(|each| players.contains(*each));
        }
        let morphs = appearance.map_or(&[][..], |a| &a.morphs[..]);
        let mut lift = 0.0;
        if !morphs.is_empty()
            && let Some((player, graph)) = lifted.player.and_then(|p| players.get(p).ok())
            && let Some(graph) = graphs.get(graph)
        {
            for (node, active) in player.playing_animations() {
                let Some(AnimationNodeType::Clip(clip)) = graph.get(*node).map(|n| &n.node_type)
                else {
                    continue;
                };
                let Some(name) = names.0.get(&clip.id()) else {
                    continue;
                };
                lift += active.weight() * lifted.lifts.at(name, active.seek_time(), morphs);
            }
        }
        if let Ok(mut transform) = transforms.get_mut(lifted.node) {
            // Up in the model's own space, which is glTF's +Y.
            let at = lifted.rest + Vec3::Y * lift;
            if transform.translation != at {
                transform.translation = at;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lifts() -> Lifts {
        serde_json::from_value(serde_json::json!({
            "fps": 30.0,
            "clips": {"Roll": {"first": 1.0, "heavy": [0.0, 0.03, 0.06], "thin": [0.0, -0.01, -0.02]}}
        }))
        .unwrap()
    }

    #[test]
    fn a_body_is_lifted_by_its_morphs_curves_at_the_frame() {
        let lifts = lifts();
        // Frame 3 is the curves' third value: 0.06 heavy, -0.02 thin.
        let at = 3.0 / 30.0;
        let heavy = lifts.at("Roll", at, &[("heavy".into(), 1.0)]);
        assert!((heavy - 0.06).abs() < 1e-5, "{heavy}");
        let mixed = lifts.at("Roll", at, &[("heavy".into(), 0.5), ("thin".into(), 0.5)]);
        assert!((mixed - 0.02).abs() < 1e-5, "{mixed}");
    }

    #[test]
    fn the_base_bodys_lifts_are_for_its_clips_and_morphs() {
        let base = crate::model_files::json(crate::appearance::BASE);
        // As `find_lifts` reads them, which warns and lifts nothing when they do not read.
        let lifts: Lifts = serde_json::from_value(
            crate::model_files::extra(&base, "lifts").expect("the base body carries no lifts"),
        )
        .unwrap();
        assert!(lifts.fps > 0.0);
        assert!(!lifts.clips.is_empty());
        let clips = crate::model_files::animations(&base);
        let morphs = crate::model_files::morphs(&base);
        for (clip, curves) in &lifts.clips {
            assert!(clips.contains(clip), "lifts for no clip {clip}");
            for morph in curves.morphs.keys() {
                assert!(morphs.contains(morph), "{clip}: lifts for no morph {morph}");
            }
        }
    }

    #[test]
    fn between_frames_it_eases_and_past_the_ends_it_holds() {
        let lifts = lifts();
        let half = lifts.at("Roll", 2.5 / 30.0, &[("heavy".into(), 1.0)]);
        assert!((half - 0.045).abs() < 1e-5, "{half}");
        let after = lifts.at("Roll", 10.0, &[("heavy".into(), 1.0)]);
        assert!((after - 0.06).abs() < 1e-5, "{after}");
        assert_eq!(lifts.at("Roll", 0.0, &[("heavy".into(), 1.0)]), 0.0);
    }

    #[test]
    fn a_clip_or_morph_without_a_lift_lifts_nothing() {
        let lifts = lifts();
        assert_eq!(lifts.at("Walk_Loop", 0.5, &[("heavy".into(), 1.0)]), 0.0);
        assert_eq!(lifts.at("Roll", 0.05, &[("strong".into(), 1.0)]), 0.0);
    }
}
