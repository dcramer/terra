//! The hold layer (holding.md section 2, D458): a stance's arms laid over the shared
//! locomotion clips at run time by a bone mask, so a drawn body walks in the library's walk
//! with the guard's arms rather than in a clip baked for it. It is Unreal's Layered Blend per
//! Bone (Unity's avatar masks, Godot's filtered Blend2, ALS's overlay layer) on Bevy's mask
//! groups: every bone is in one of two groups, the held arms or the rest; a clip played under
//! a hold plays a copy masked off the arms, and the hold's clip, held at its first pose, a copy
//! masked off everything else, taking the weight the transitions give the clips under it. The
//! arms then show the hold alone, and blend from and to whatever played before as the legs do.

use bevy::{
    animation::{AnimationTargetId, graph::AnimationMask},
    gltf::GltfNode,
    platform::collections::{HashMap, HashSet},
    prelude::*,
};

/// The bones a hold lays its pose on, with every bone below them: both arms from the clavicles
/// (D458). Its twin is `tools/characters/glb.ts`'s `HOLD_MASK`, which the move check lays by.
pub const HOLD_MASK: [&str; 2] = ["clavicle_l", "clavicle_r"];

/// The mask groups: the rest of the body, and the arms a hold lays its pose on.
const REST: u32 = 0;
const ARMS: u32 = 1;
/// A clip played under a hold: masked off the arms.
pub(crate) const UNDER: AnimationMask = 1 << ARMS;
/// The hold: masked off all but the arms.
pub(crate) const OVER: AnimationMask = 1 << REST;

/// A clip's places in a figure's graph: whole twice, so a clip played again blends from
/// itself; masked off the arms twice, played under a hold; and on the arms alone, as a hold.
#[derive(Clone, Copy, Debug)]
pub struct Nodes {
    pub whole: [AnimationNodeIndex; 2],
    pub under: [AnimationNodeIndex; 2],
    pub over: AnimationNodeIndex,
}

impl Nodes {
    /// The copy `copy` of the clip, whole or under a hold.
    pub fn of(&self, copy: usize, held: bool) -> AnimationNodeIndex {
        if held {
            self.under[copy]
        } else {
            self.whole[copy]
        }
    }
}

/// The graph of every clip of a file, each at its places (`Nodes`), every bone in its mask
/// group.
pub fn graph(gltf: &Gltf, nodes: &Assets<GltfNode>) -> (AnimationGraph, HashMap<String, Nodes>) {
    let mut graph = AnimationGraph::new();
    let root = graph.root;
    let mut clips = HashMap::new();
    for (name, clip) in &gltf.named_animations {
        let mut add = |mask| graph.add_clip_with_mask(clip.clone(), mask, 1.0, root);
        let nodes = Nodes {
            whole: [add(0), add(0)],
            under: [add(UNDER), add(UNDER)],
            over: add(OVER),
        };
        clips.insert(name.to_string(), nodes);
    }
    let paths = paths(gltf, nodes);
    mask(&mut graph, paths.iter().map(Vec::as_slice));
    (graph, clips)
}

/// Puts every bone, by its path of names from its file's root (as the glTF loader names
/// animation targets), in the arms' group when the path passes through `HOLD_MASK`, else in
/// the rest's. A bone in no group could not be masked.
pub(crate) fn mask<'a>(graph: &mut AnimationGraph, paths: impl IntoIterator<Item = &'a [Name]>) {
    for path in paths {
        let held = path.iter().any(|name| HOLD_MASK.contains(&name.as_str()));
        let target = AnimationTargetId::from_names(path.iter());
        graph.add_target_to_mask_group(target, if held { ARMS } else { REST });
    }
}

/// Every node's path of names from its file's root, as the glTF loader builds them.
fn paths(gltf: &Gltf, nodes: &Assets<GltfNode>) -> Vec<Vec<Name>> {
    let children: HashSet<AssetId<GltfNode>> = gltf
        .nodes
        .iter()
        .filter_map(|node| nodes.get(node))
        .flat_map(|node| node.children.iter().map(Handle::id))
        .collect();
    let mut out = Vec::new();
    let mut stack: Vec<(&Handle<GltfNode>, Vec<Name>)> = gltf
        .nodes
        .iter()
        .filter(|node| !children.contains(&node.id()))
        .map(|node| (node, Vec::new()))
        .collect();
    while let Some((handle, mut path)) = stack.pop() {
        let Some(node) = nodes.get(handle) else {
            continue;
        };
        path.push(Name::new(node.name.clone()));
        stack.extend(node.children.iter().map(|child| (child, path.clone())));
        out.push(path);
    }
    out
}

/// A hold laid over clips playing under it: its node, and the nodes it is laid over.
pub struct Held {
    pub over: AnimationNodeIndex,
    pub under: Vec<AnimationNodeIndex>,
}

/// Lays the hold `over` over `under`, which has just started: started held at its first
/// pose, or ridden on by the hold already laid.
pub fn lay(
    holds: &mut Vec<Held>,
    animator: &mut AnimationPlayer,
    over: AnimationNodeIndex,
    under: AnimationNodeIndex,
) {
    for held in holds.iter_mut() {
        held.under.retain(|&node| node != under);
    }
    match holds.iter_mut().find(|held| held.over == over) {
        Some(held) => held.under.push(under),
        None => {
            animator
                .start(over)
                .seek_to(0.0)
                .set_speed(0.0)
                .set_weight(0.0);
            holds.push(Held {
                over,
                under: vec![under],
            });
        }
    }
}

/// Each hold takes the summed weight the transitions give the clips under it, so its arms come
/// and go with them, and stops with the last of them.
pub fn ride(holds: &mut Vec<Held>, animator: &mut AnimationPlayer) {
    holds.retain_mut(|held| {
        held.under
            .retain(|&node| animator.animation(node).is_some());
        let whole: f32 = held
            .under
            .iter()
            .filter_map(|&node| animator.animation(node))
            .map(|a| a.weight())
            .sum();
        if held.under.is_empty() {
            animator.stop(held.over);
            return false;
        }
        match animator.animation_mut(held.over) {
            Some(over) => {
                over.set_weight(whole);
                true
            }
            None => false,
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::animation::{AnimatedBy, AnimationPlugin, animated_field};
    use bevy::app::ScheduleRunnerPlugin;
    use bevy::asset::AssetPlugin;

    /// A clip holding each named bone at its own place.
    fn clip(at: &[(&[Name], Vec3)]) -> AnimationClip {
        let mut clip = AnimationClip::default();
        for (path, place) in at {
            clip.add_curve_to_target(
                AnimationTargetId::from_names(path.iter()),
                AnimatableCurve::new(
                    animated_field!(Transform::translation),
                    ConstantCurve::new(Interval::UNIT, *place),
                ),
            );
        }
        clip
    }

    #[test]
    fn a_hold_lays_its_arms_over_a_clip_whose_legs_play_and_goes_with_it() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_once()),
            AssetPlugin::default(),
            AnimationPlugin,
        ));
        app.add_systems(
            PostUpdate,
            (|mut figures: Query<(&mut AnimationPlayer, &mut Holds)>| {
                for (mut animator, mut holds) in &mut figures {
                    ride(&mut holds.0, &mut animator);
                }
            })
            .after(bevy::animation::transition::advance_transitions)
            .before(bevy::animation::animate_targets),
        );
        let names = |path: &[&'static str]| path.iter().map(|n| Name::new(*n)).collect::<Vec<_>>();
        let (arm, leg) = (
            names(&["Rig", "spine", "clavicle_l", "hand_l"]),
            names(&["Rig", "thigh_l"]),
        );
        let (walk_arm, walk_leg, guard_arm, guard_leg) = (Vec3::X, Vec3::Y, Vec3::Z, Vec3::NEG_Y);
        let mut clips = app.world_mut().resource_mut::<Assets<AnimationClip>>();
        let walk = clips.add(clip(&[(&arm, walk_arm), (&leg, walk_leg)]));
        let guard = clips.add(clip(&[(&arm, guard_arm), (&leg, guard_leg)]));
        let mut graph = AnimationGraph::new();
        let under = graph.add_clip_with_mask(walk, UNDER, 1.0, graph.root);
        let over = graph.add_clip_with_mask(guard, OVER, 1.0, graph.root);
        mask(&mut graph, [arm.as_slice(), leg.as_slice()]);
        let graph = app
            .world_mut()
            .resource_mut::<Assets<AnimationGraph>>()
            .add(graph);
        let mut animator = AnimationPlayer::default();
        let mut holds = Vec::new();
        animator.start(under).repeat().set_weight(1.0);
        lay(&mut holds, &mut animator, over, under);
        let figure = app
            .world_mut()
            .spawn((animator, AnimationGraphHandle(graph), Holds(holds)))
            .id();
        let bone = |app: &mut App, path: &[Name]| {
            app.world_mut()
                .spawn((
                    Transform::default(),
                    AnimationTargetId::from_names(path.iter()),
                    AnimatedBy(figure),
                ))
                .id()
        };
        let (hand, thigh) = (bone(&mut app, &arm), bone(&mut app, &leg));
        let at = |app: &App, bone| app.world().get::<Transform>(bone).unwrap().translation;
        app.update();
        app.update();
        // The arm holds the guard, the leg walks.
        assert!(
            at(&app, hand).distance(guard_arm) < 1e-5,
            "{}",
            at(&app, hand)
        );
        assert!(
            at(&app, thigh).distance(walk_leg) < 1e-5,
            "{}",
            at(&app, thigh)
        );
        // The clip under it stops: the hold stops with it and is forgotten.
        app.world_mut()
            .get_mut::<AnimationPlayer>(figure)
            .unwrap()
            .stop(under);
        app.update();
        let player = app.world().get::<AnimationPlayer>(figure).unwrap();
        assert!(player.animation(over).is_none());
        assert!(app.world().get::<Holds>(figure).unwrap().0.is_empty());
    }

    #[derive(Component)]
    struct Holds(Vec<Held>);
}
