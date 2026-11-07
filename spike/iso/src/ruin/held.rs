//! The knight's sword in his right hand: Tripo's rig carries no weapon and its knight was
//! modelled empty-handed, so the sword (assets/models/iso/sword.glb, Tripo's medieval sword
//! exported by tools/iso-art) hangs on the hand bone as the shipped games' weapon sockets
//! do (Unreal's `hand_r` socket, Unity's held-prop bone). The grip was measured on the
//! knight's GLB in Node: in `hand_r`'s frame the fingers run along +y to the knuckles at
//! y 0.06, the palm faces -z, the thumb is on -x; the blade leaves the fist along +y tipped
//! toward the thumb, which over the attack clip sweeps up and back at the wind-up (1.0 s),
//! forward at the chop (1.3 s) and down after it (1.4 s). The sword's own frame: blade along
//! +y, edges along x, its grip 0.05 to 0.15 m up from the pommel at y 0. The hand's world
//! scale (the knight's armature is scaled to life size) is taken back out, so the sword is
//! its own 0.95 m whatever body holds it. Every scene staging the fight (ruin, forest,
//! marsh) gets it through fight.rs's plugin.

use super::effects::Stage;
use bevy::prelude::*;

const SWORD: &str = "models/iso/sword.glb";
const HAND: &str = "hand_r";
/// The fist's hollow, where the grip's middle sits, in the hand bone's frame.
const FIST: Vec3 = Vec3::new(-0.006, 0.042, -0.02);
/// How far the blade tips from the fingers' line toward the thumb (a hammer grip).
const TIP: f32 = 0.35;
/// The middle of the sword's grip, up its blade from the pommel (m).
const GRIP: f32 = 0.10;

/// A held thing hanging on a bone; its transform keeps the bone's scale out.
#[derive(Component)]
pub struct Held;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (hold, fit).chain());
}

/// The sword's place in the hand bone's frame, given the bone's world scale.
fn grip(scale: f32) -> Transform {
    let blade = Vec3::new(-TIP, 1.0, 0.0).normalize();
    let rotation = Quat::from_mat3(&Mat3::from_cols(blade.cross(Vec3::Z), blade, Vec3::Z));
    let s = 1.0 / scale.max(1e-3);
    Transform {
        translation: FIST - blade * GRIP * s,
        rotation,
        scale: Vec3::splat(s),
    }
}

/// Once the knight's skeleton is in, hangs the sword on his right hand.
fn hold(
    mut commands: Commands,
    asset: Res<AssetServer>,
    stage: Res<Stage>,
    bodies: Query<(Entity, &Transform), (With<WorldAssetRoot>, Without<ChildOf>)>,
    children: Query<&Children>,
    names: Query<&Name>,
    held: Query<(), With<Held>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some((knight, _)) = bodies
        .iter()
        .find(|(_, t)| t.translation.xz().distance(stage.knight) < 0.3)
    else {
        return;
    };
    let Some(hand) = children
        .iter_descendants(knight)
        .find(|e| names.get(*e).is_ok_and(|n| n.as_str() == HAND))
    else {
        return;
    };
    if children.iter_descendants(hand).any(|e| held.contains(e)) {
        *done = true;
        return;
    }
    commands.spawn((
        WorldAssetRoot(asset.load(GltfAssetLabel::Scene(0).from_asset(SWORD))),
        grip(1.0),
        Held,
        ChildOf(hand),
    ));
    *done = true;
}

/// Keeps each held thing at its grip with its bone's world scale taken out.
fn fit(mut held: Query<(&ChildOf, &mut Transform), With<Held>>, bones: Query<&GlobalTransform>) {
    for (parent, mut tr) in &mut held {
        let Ok(bone) = bones.get(parent.parent()) else {
            continue;
        };
        let want = grip(bone.scale().x);
        if tr.scale.distance(want.scale) > 1e-4 {
            *tr = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The grip puts the sword's grip in the fist and its blade out of the hand along the
    /// fingers tipped to the thumb, its edges in the swing's plane, at its own size.
    #[test]
    fn the_sword_sits_in_the_fist() {
        let scale = 1.89;
        let g = grip(scale);
        let into_hand = |p: Vec3| g.transform_point(p);
        assert!(into_hand(Vec3::Y * GRIP).distance(FIST) < 1e-4);
        let blade = (into_hand(Vec3::Y) - into_hand(Vec3::ZERO)).normalize();
        assert!(blade.y > 0.9 && blade.x < -0.2, "blade along {blade}");
        let flat = (into_hand(Vec3::Z) - into_hand(Vec3::ZERO)).normalize();
        assert!(flat.z > 0.99, "the blade's flat faces the palm: {flat}");
        let tip = into_hand(Vec3::Y * 0.95).distance(into_hand(Vec3::ZERO)) * scale;
        assert!((tip - 0.95).abs() < 1e-3, "{tip} m long in the world");
    }
}
