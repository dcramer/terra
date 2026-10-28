//! The hold layer (holding.md section 2, D458): a stance's arms laid over the shared
//! locomotion clips at run time by a bone mask, so a drawn body walks in the library's walk
//! with the guard's arms rather than in a clip baked for it. It is Unreal's Layered Blend per
//! Bone (Unity's avatar masks, Godot's filtered Blend2, ALS's overlay layer) on Bevy's mask
//! groups: every bone is in one of two groups, the held arms or the rest; a clip played under
//! a hold plays a copy masked off the arms, and the hold's clip, held at its first pose, a copy
//! masked off everything else, taking the weight the transitions give the clips under it. The
//! arms then show the hold alone, and blend from and to whatever played before as the legs do.
//!
//! A stance's hold lays both arms; a carry's (D560, a lit torch on a raised arm) only the arm
//! that carries, the other left to the clip, as Unreal's per-bone layer takes a branch per arm.

use bevy::{
    animation::{AnimationTargetId, graph::AnimationMask},
    gltf::GltfNode,
    platform::collections::{HashMap, HashSet},
    prelude::*,
};

/// The bones a hold lays its pose on, with every bone below them: the left arm and the right
/// from the clavicles (D458). Its twin is `tools/characters/glb.ts`'s `HOLD_MASK`, which the
/// move check lays by.
pub const HOLD_MASK: [&str; 2] = ["clavicle_l", "clavicle_r"];

/// The mask groups: the rest of the body, and each arm a hold lays its pose on.
const REST: u32 = 0;
const LEFT: u32 = 1;
const RIGHT: u32 = 2;
/// A clip played under a stance's hold: masked off both arms.
pub(crate) const UNDER: AnimationMask = Arms::Both.groups();
/// A stance's hold: masked off all but the arms.
pub(crate) const OVER: AnimationMask = Arms::Both.over();

/// Which arms a hold lays its pose on: none, one (a carry's), or both (a stance's).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Arms {
    #[default]
    None,
    Left,
    Right,
    Both,
}

impl Arms {
    /// Its mask groups.
    const fn groups(self) -> AnimationMask {
        match self {
            Arms::None => 0,
            Arms::Left => 1 << LEFT,
            Arms::Right => 1 << RIGHT,
            Arms::Both => (1 << LEFT) | (1 << RIGHT),
        }
    }

    /// A hold on these arms: masked off every other group.
    const fn over(self) -> AnimationMask {
        ((1 << REST) | (1 << LEFT) | (1 << RIGHT)) & !self.groups()
    }

    /// These arms and `other`'s.
    pub fn and(self, other: Arms) -> Arms {
        match self.groups() | other.groups() {
            0 => Arms::None,
            g if g == Arms::Left.groups() => Arms::Left,
            g if g == Arms::Right.groups() => Arms::Right,
            _ => Arms::Both,
        }
    }
}

/// A clip's places in a figure's graph: whole twice, so a clip played again blends from
/// itself; masked off both arms twice, played under a stance's hold; on the arms alone, as a
/// stance's hold; and for each arm alone (left, right), masked off it twice, played under a
/// carry's hold on it, and on it alone, as that hold.
#[derive(Clone, Copy, Debug)]
pub struct Nodes {
    pub whole: [AnimationNodeIndex; 2],
    pub under: [AnimationNodeIndex; 2],
    pub over: AnimationNodeIndex,
    pub under_one: [[AnimationNodeIndex; 2]; 2],
    pub over_one: [AnimationNodeIndex; 2],
}

impl Nodes {
    /// The copy `copy` of the clip, whole or under a stance's hold.
    pub fn of(&self, copy: usize, held: bool) -> AnimationNodeIndex {
        self.played(copy, if held { Arms::Both } else { Arms::None })
    }

    /// The copy `copy` of the clip, under holds on `arms`.
    pub fn played(&self, copy: usize, arms: Arms) -> AnimationNodeIndex {
        match arms {
            Arms::None => self.whole[copy],
            Arms::Left => self.under_one[0][copy],
            Arms::Right => self.under_one[1][copy],
            Arms::Both => self.under[copy],
        }
    }

    /// The clip as a hold on one arm, `Left` or `Right` (a stance's on both for `Both`).
    pub fn over(&self, arms: Arms) -> Option<AnimationNodeIndex> {
        match arms {
            Arms::None => None,
            Arms::Left => Some(self.over_one[0]),
            Arms::Right => Some(self.over_one[1]),
            Arms::Both => Some(self.over),
        }
    }
}

/// The graph of every clip of a file, each at its places (`Nodes`), every bone in its mask
/// group.
pub fn graph(gltf: &Gltf, nodes: &Assets<GltfNode>) -> (AnimationGraph, HashMap<String, Nodes>) {
    graph_of(&[gltf], nodes)
}

/// The graph of every clip of `files`, a name given more than once taken from the first file
/// that has it: a model's own clips, then a clips file's added on the same skeleton, as the
/// figure plays the fight file's on the base body (the workbench, `shares_skeleton`).
pub fn graph_of(
    files: &[&Gltf],
    nodes: &Assets<GltfNode>,
) -> (AnimationGraph, HashMap<String, Nodes>) {
    let mut graph = AnimationGraph::new();
    let root = graph.root;
    let mut clips = HashMap::new();
    let named = first_named(files.iter().map(|gltf| {
        gltf.named_animations
            .iter()
            .map(|(name, clip)| (name.as_ref(), clip))
    }));
    for (name, clip) in named {
        let mut add = |mask| graph.add_clip_with_mask(clip.clone(), mask, 1.0, root);
        let (left, right) = (Arms::Left, Arms::Right);
        let nodes = Nodes {
            whole: [add(0), add(0)],
            under: [add(UNDER), add(UNDER)],
            over: add(OVER),
            under_one: [
                [add(left.groups()), add(left.groups())],
                [add(right.groups()), add(right.groups())],
            ],
            over_one: [add(left.over()), add(right.over())],
        };
        clips.insert(name.to_string(), nodes);
    }
    for gltf in files {
        let paths = paths(gltf, nodes);
        mask(&mut graph, paths.iter().map(Vec::as_slice));
    }
    (graph, clips)
}

/// Each name of `files`' clips once, with the first file's clip of it.
fn first_named<'a, T>(
    files: impl IntoIterator<Item = impl IntoIterator<Item = (&'a str, T)>>,
) -> Vec<(&'a str, T)> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for file in files {
        for (name, clip) in file {
            if seen.insert(name) {
                out.push((name, clip));
            }
        }
    }
    out
}

/// Whether `model` has every bone `clips` animates by the same path of names, so `clips`'
/// animations play on it (the glTF loader targets a bone by that path).
pub fn shares_skeleton(model: &Gltf, clips: &Gltf, nodes: &Assets<GltfNode>) -> bool {
    has_every_path(&paths(model, nodes), &paths(clips, nodes))
}

fn has_every_path(model: &[Vec<Name>], clips: &[Vec<Name>]) -> bool {
    let model: HashSet<&[Name]> = model.iter().map(Vec::as_slice).collect();
    !clips.is_empty() && clips.iter().all(|path| model.contains(path.as_slice()))
}

/// Puts every bone, by its path of names from its file's root (as the glTF loader names
/// animation targets), in its arm's group when the path passes through that arm's clavicle
/// (`HOLD_MASK`), else in the rest's. A bone in no group could not be masked.
pub(crate) fn mask<'a>(graph: &mut AnimationGraph, paths: impl IntoIterator<Item = &'a [Name]>) {
    for path in paths {
        let through = |root: &str| path.iter().any(|name| name.as_str() == root);
        let group = if through(HOLD_MASK[0]) {
            LEFT
        } else if through(HOLD_MASK[1]) {
            RIGHT
        } else {
            REST
        };
        let target = AnimationTargetId::from_names(path.iter());
        graph.add_target_to_mask_group(target, group);
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
    lay_all(holds, animator, &[over], under);
}

/// Lays each hold of `overs` (one an arm) over `under`, which has just started.
pub fn lay_all(
    holds: &mut Vec<Held>,
    animator: &mut AnimationPlayer,
    overs: &[AnimationNodeIndex],
    under: AnimationNodeIndex,
) {
    for held in holds.iter_mut() {
        held.under.retain(|&node| node != under);
    }
    for &over in overs {
        lay_one(holds, animator, over, under);
    }
}

fn lay_one(
    holds: &mut Vec<Held>,
    animator: &mut AnimationPlayer,
    over: AnimationNodeIndex,
    under: AnimationNodeIndex,
) {
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

    #[test]
    fn a_models_own_clip_wins_and_a_clips_files_others_are_added() {
        let model = [("Idle_Loop", 1), ("Walk_Loop", 2)];
        let clips = [("Idle_Loop", 10), ("Low_Guard", 11), ("Walk_Loop", 12)];
        let mut named = first_named([model.to_vec(), clips.to_vec()]);
        named.sort();
        assert_eq!(
            named,
            vec![("Idle_Loop", 1), ("Low_Guard", 11), ("Walk_Loop", 2)]
        );
    }

    #[test]
    fn clips_play_on_a_model_only_with_every_bone_they_name() {
        let path = |names: &[&'static str]| names.iter().map(|n| Name::new(*n)).collect::<Vec<_>>();
        let body = [
            path(&["Rig"]),
            path(&["Rig", "pelvis"]),
            path(&["Rig", "pelvis", "Body_Mesh"]),
        ];
        let fight = [path(&["Rig"]), path(&["Rig", "pelvis"])];
        let wolf = [path(&["Wolf"]), path(&["Wolf", "pelvis"])];
        assert!(has_every_path(&body, &fight));
        assert!(!has_every_path(&wolf, &fight));
        assert!(!has_every_path(&fight, &body));
        assert!(!has_every_path(&body, &[]));
    }

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

    /// A carry's hold on one arm (D560): that arm holds the carry, the other arm and the legs
    /// walk; and two one-arm holds laid over the same clip each keep their own arm.
    #[test]
    fn a_hold_on_one_arm_leaves_the_other_arm_to_the_clip() {
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
        let (left, right, leg) = (
            names(&["Rig", "spine", "clavicle_l", "hand_l"]),
            names(&["Rig", "spine", "clavicle_r", "hand_r"]),
            names(&["Rig", "thigh_l"]),
        );
        let walk = [Vec3::X, Vec3::NEG_X, Vec3::Y];
        let torch = [Vec3::Z, Vec3::NEG_Z, Vec3::NEG_Y];
        let shield = [Vec3::ONE, Vec3::NEG_ONE, Vec3::splat(2.0)];
        let clip_of = |at: [Vec3; 3]| clip(&[(&left, at[0]), (&right, at[1]), (&leg, at[2])]);
        let mut clips = app.world_mut().resource_mut::<Assets<AnimationClip>>();
        let (walk_clip, torch_clip, shield_clip) = (
            clips.add(clip_of(walk)),
            clips.add(clip_of(torch)),
            clips.add(clip_of(shield)),
        );
        let mut graph = AnimationGraph::new();
        let root = graph.root;
        let under_right =
            graph.add_clip_with_mask(walk_clip.clone(), Arms::Right.groups(), 1.0, root);
        let under_both = graph.add_clip_with_mask(walk_clip, Arms::Both.groups(), 1.0, root);
        let over_right = graph.add_clip_with_mask(torch_clip, Arms::Right.over(), 1.0, root);
        let over_left = graph.add_clip_with_mask(shield_clip, Arms::Left.over(), 1.0, root);
        mask(
            &mut graph,
            [left.as_slice(), right.as_slice(), leg.as_slice()],
        );
        let graph = app
            .world_mut()
            .resource_mut::<Assets<AnimationGraph>>()
            .add(graph);
        let mut animator = AnimationPlayer::default();
        let mut holds = Vec::new();
        animator.start(under_right).repeat().set_weight(1.0);
        lay(&mut holds, &mut animator, over_right, under_right);
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
        let bones = [
            bone(&mut app, &left),
            bone(&mut app, &right),
            bone(&mut app, &leg),
        ];
        let shown = |app: &App| bones.map(|b| app.world().get::<Transform>(b).unwrap().translation);
        app.update();
        app.update();
        assert_eq!(shown(&app), [walk[0], torch[1], walk[2]]);
        // Both arms carrying: the clip under both, each hold on its own arm.
        let world = app.world_mut();
        let mut figure_at = world.entity_mut(figure);
        let mut animator = figure_at.take::<AnimationPlayer>().unwrap();
        let mut holds = figure_at.take::<Holds>().unwrap();
        animator.stop(under_right);
        animator.start(under_both).repeat().set_weight(1.0);
        lay_all(
            &mut holds.0,
            &mut animator,
            &[over_left, over_right],
            under_both,
        );
        world.entity_mut(figure).insert((animator, holds));
        app.update();
        app.update();
        assert_eq!(shown(&app), [shield[0], torch[1], walk[2]]);
    }

    #[derive(Component)]
    struct Holds(Vec<Held>);
}
