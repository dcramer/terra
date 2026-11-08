//! The fight: the knight at the middle of the floor against an ogre swinging at him, a tree
//! monster striding in from the breach in the back wall. Each body plays one clip from a
//! fixed time; in a still (`--shot`) the clip holds there, so every still is the same pose.

use crate::Args;
use crate::scene::model;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins((super::held::plugin, super::walk::plugin))
        .add_systems(Update, (cast, pose));
}

/// Every body the fight poses, in every scene that stages it (the ruin, the forest, the
/// marsh, a street's people), takes the fighters' rim (rim.rs; the scene adds `rim::plugin`),
/// so it lifts off the ground as the ARPGs' bodies do: the three biome judges found the
/// fighters lost in it (iso-art j28). How dark and how rimmed is data: the body's own
/// `Toned` (play's roster entry) or its biome's `bodies` for its model (kit/biome.rs `Tone`,
/// D136), else as it comes.
fn cast(
    mut commands: Commands,
    posed: Query<(Entity, &Pose, Option<&Toned>), Added<Pose>>,
    biome: Option<Res<crate::kit::biome::Biome>>,
) {
    for (body, pose, own) in &posed {
        let name = pose.gltf.path().map(|p| p.to_string()).unwrap_or_default();
        let tone = tone_of(own, biome.as_deref(), &name);
        commands
            .entity(body)
            .insert(crate::rim::Fighter::valued(tone.value, tone.rim).with_fill(tone.fill));
    }
}

/// A posed body's tone: its own, else its biome's for its model, else as it comes.
fn tone_of(
    own: Option<&Toned>,
    biome: Option<&crate::kit::biome::Biome>,
    model: &str,
) -> crate::kit::biome::Tone {
    own.map(|t| t.0)
        .or_else(|| biome.map(|b| b.tone(model)))
        .unwrap_or_default()
}

/// A posed body's own tone, over its biome's for its model (a roster entry's `tone`).
#[derive(Component, Clone, Copy)]
pub struct Toned(pub crate::kit::biome::Tone);

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
        let yaw = libm::atan2f(d.x, d.y);
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
        if name == "knight" {
            // In a film he walks in from the front of the court, clear of the pool, the
            // brazier, the skull and the barrels (walk.rs).
            commands.entity(body).insert(super::walk::WalkIn {
                from: Vec2::new(0.8, 4.0),
                to: place,
                floor: super::walk::Floor::Flat(super::ground::TOP),
                fight: true,
            });
        }
    }
}

/// A cool back light on a fighter, so its edge reads against the floor as the ARPGs'
/// heroes' do: a narrow spot from 5 m behind it as the camera sees it, level with its
/// chest, so the cone meets the floor only far beyond and at a graze (Bevy lights every
/// mesh in reach, rim.rs; a level spot keeps the spill off the floor near the fight).
fn rim(commands: &mut Commands, place: Vec2) {
    let yaw = super::camera::YAW.to_radians();
    let toward = Vec3::new(libm::sinf(yaw), 0.0, libm::cosf(yaw));
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
pub struct Posed;

/// Each posed body's clip played from its time, held there in a still; the street's rigged
/// props take it too (kit/street.rs).
pub fn pose(
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
        if args.still() {
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

    /// A body's tone is data: its biome's for its model, its own (a roster entry's) over
    /// that, and a model the biome does not name as it comes.
    #[test]
    fn a_bodys_tone_is_its_biomes_or_its_own() {
        use crate::kit::biome::{Biome, Tone};
        let mut biome = Biome::load("forest").expect("the forest's file reads");
        let dark = Tone {
            value: 0.25,
            rim: 2.0,
            fill: 0.0,
        };
        biome.bodies.insert("models/iso/a.glb".into(), dark);
        assert_eq!(tone_of(None, Some(&biome), "models/iso/a.glb"), dark);
        let own = Toned(Tone {
            value: 0.6,
            rim: 0.5,
            fill: 0.4,
        });
        assert_eq!(tone_of(Some(&own), Some(&biome), "models/iso/a.glb"), own.0);
        assert_eq!(
            tone_of(None, Some(&biome), "models/iso/unnamed.glb"),
            Tone::default()
        );
        assert_eq!(tone_of(None, None, "models/iso/a.glb"), Tone::default());
    }
}
