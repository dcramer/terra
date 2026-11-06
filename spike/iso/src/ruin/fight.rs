//! The fight: the knight at the middle of the floor against an ogre swinging at him, a tree
//! monster striding in from the breach in the back wall. Each body plays one clip from a
//! fixed time; in a still (`--shot`) the clip holds there, so every still is the same pose.

use crate::Args;
use crate::scene::model;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins(super::held::plugin)
        .add_systems(Update, (cast, pose));
}

/// Every body the fight poses, in every scene that stages it (the ruin, the forest, the
/// marsh), takes the fighters' rim (rim.rs; the scene adds `rim::plugin`), so it lifts off
/// the ground as the ARPGs' bodies do: the three biome judges found the fighters lost in
/// it (iso-art j28).
fn cast(mut commands: Commands, posed: Query<(Entity, &Pose), Added<Pose>>) {
    for (body, pose) in &posed {
        let name = pose.gltf.path().map(|p| p.to_string()).unwrap_or_default();
        let (albedo, rim) = value(&name);
        commands
            .entity(body)
            .insert(crate::rim::Fighter::valued(albedo, rim));
    }
}

/// How much a body's albedo and rim are scaled. The ogre's beige coat is the floor's tone
/// in every scene (the judges' gap), so it stands darker; the pale treant stood at the
/// forest's rocks' value and blew out white under the marsh's frost (iso-art j30's
/// judges). The tree monster's crown is all leaf edges, each of which takes the band, so
/// it frosts: a share of it. The rest as they come.
fn value(path: &str) -> (f32, f32) {
    if path.contains("ogre") {
        (0.5, 1.0)
    } else if path.contains("treant-a") {
        (0.55, 1.0)
    } else if path.contains("tree-monster") {
        (1.0, 0.35)
    } else {
        (1.0, 1.0)
    }
}

/// A body's clip and where in it to stand.
#[derive(Component)]
pub struct Pose {
    pub gltf: Handle<Gltf>,
    pub clip: &'static str,
    pub at: f32,
    /// Played once and held at its end (a death), not looped.
    pub once: bool,
}

/// Who fights where: model, clip, the clip's time, the body's place and what it faces.
const CAST: [(&str, &str, f32, Vec2, Vec2); 3] = [
    // The hero's chop coming down on the ogre: in the 2 s attack the arms are up at 1.1 s,
    // the blade comes down through 1.25 s and he is bent over it at 1.4 s (a sheet of
    // stills at 0.6 to 1.4 s); the blow lands at 1.25 (effects::hit's `BLOW`) and a still
    // stands 0.05 s after it.
    (
        "knight",
        "attack",
        1.3,
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
            once: false,
        });
        if name != "tree-monster" {
            rim(&mut commands, place);
        }
    }
}

/// A cool back light on a fighter, so its edge reads against the floor as the ARPGs'
/// heroes' do: a narrow spot from 5 m behind it as the camera sees it, level with its
/// chest, so the cone meets the floor only far beyond and at a graze (Bevy lights every
/// mesh in reach, rim.rs; a level spot keeps the spill off the floor near the fight).
fn rim(commands: &mut Commands, place: Vec2) {
    let yaw = super::camera::YAW.to_radians();
    let toward = Vec3::new(yaw.sin(), 0.0, yaw.cos());
    let chest = place.extend(super::ground::TOP + 1.3).xzy();
    commands.spawn((
        SpotLight {
            color: Color::srgb(0.6, 0.75, 1.0),
            intensity: 900_000.0,
            range: 9.0,
            radius: 0.2,
            inner_angle: 0.08,
            outer_angle: 0.17,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(chest - toward * 5.0).looking_at(chest, Vec3::Y),
    ));
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
        let playing = p.play(node).seek_to(pose.at);
        if !pose.once {
            playing.repeat();
        }
        if args.shot.is_some() && args.film == 0 {
            playing.set_speed(0.0);
        }
        commands
            .entity(player)
            .insert(AnimationGraphHandle(graphs.add(graph)));
        commands.entity(body).insert(Posed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every body the fight poses takes the fighters' rim, in whichever scene stages it.
    #[test]
    fn every_posed_body_is_a_fighter() {
        let mut app = App::new();
        app.add_systems(Update, cast);
        let body = app
            .world_mut()
            .spawn(Pose {
                gltf: Handle::default(),
                clip: "walk",
                at: 0.0,
                once: false,
            })
            .id();
        app.update();
        assert!(app.world().get::<crate::rim::Fighter>(body).is_some());
    }
}
