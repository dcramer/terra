//! The fight: the knight at the middle of the floor against an ogre swinging at him, a tree
//! monster striding in from the breach in the back wall. Each body plays one clip from a
//! fixed time; in a still (`--shot`) the clip holds there, so every still is the same pose.

use crate::Args;
use crate::scene::model;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, pose);
}

/// A body's clip and where in it to stand.
#[derive(Component)]
pub struct Pose {
    gltf: Handle<Gltf>,
    clip: &'static str,
    at: f32,
}

/// Who fights where: model, clip, the clip's time, the body's place and what it faces.
const CAST: [(&str, &str, f32, Vec2, Vec2); 3] = [
    // The hero mid-swing at the ogre (the attack's blow falls at 0.75 to 1.25 s).
    (
        "knight",
        "attack",
        1.0,
        Vec2::new(0.0, 0.0),
        Vec2::new(2.0, -1.4),
    ),
    // The ogre winding up its own blow.
    (
        "ogre",
        "attack",
        0.45,
        Vec2::new(2.0, -1.4),
        Vec2::new(0.0, 0.0),
    ),
    // The tree monster striding in from the back, toward the hero.
    (
        "tree-monster",
        "walk",
        0.3,
        Vec2::new(-2.8, -3.4),
        Vec2::new(0.0, 0.0),
    ),
];

pub fn spawn(mut commands: Commands, asset: Res<AssetServer>) {
    for (name, clip, at, place, face) in CAST {
        let path = format!("models/iso/{name}.glb");
        let d = face - place;
        // The models face +Z.
        let yaw = d.x.atan2(d.y);
        let body = model(
            &mut commands,
            &asset,
            &path,
            place.extend(super::ground::TOP).xzy(),
            yaw,
            1.0,
        );
        commands.entity(body).insert(Pose {
            gltf: asset.load(path),
            clip,
            at,
        });
    }
}

#[derive(Component)]
struct Posed;

fn pose(
    mut commands: Commands,
    args: Res<Args>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    bodies: Query<(Entity, &Pose), Without<Posed>>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
) {
    for (body, pose) in &bodies {
        let Some(gltf) = gltfs.get(&pose.gltf) else {
            continue;
        };
        let Some(player) = children
            .iter_descendants(body)
            .find(|e| players.contains(*e))
        else {
            continue;
        };
        let Some(clip) = gltf.named_animations.get(pose.clip) else {
            let have: Vec<_> = gltf.named_animations.keys().collect();
            warn!(
                "no clip {} in {:?}; it has {have:?}",
                pose.clip,
                pose.gltf.path()
            );
            commands.entity(body).insert(Posed);
            continue;
        };
        let (graph, node) = AnimationGraph::from_clip(clip.clone());
        let mut p = players.get_mut(player).unwrap();
        let playing = p.play(node).repeat().seek_to(pose.at);
        if args.shot.is_some() && args.film == 0 {
            playing.set_speed(0.0);
        }
        commands
            .entity(player)
            .insert(AnimationGraphHandle(graphs.add(graph)));
        commands.entity(body).insert(Posed);
    }
}
