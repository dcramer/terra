//! A mark shown on the arm (D590: "imprints a magic mark on your arm"): a patch of the mark's
//! glyph (`tools/characters/marks.ts`) wrapped round the back of the right forearm, riding
//! its bone, lit by nothing, the violet the gate's veil is. A player who bears a mark with a
//! glyph shows it; one who does not shows none. On this machine's own player only, until a
//! character's marks cross the network with the rest of it (D592).

use super::{Kinds, Marks};
use crate::play::Player;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// The forearm the mark is on, and the hand at its end, by the body's bone names.
const FOREARM: &str = "lowerarm_r";
const HAND: &str = "hand_r";
/// How far round the forearm the patch wraps, in radians, and how much of its length it
/// covers, about its middle.
const ARC: f32 = 2.2;
const ALONG: f32 = 0.5;
/// The forearm's radius where the patch lies, and how far off the skin it is held, in metres.
const RADIUS_M: f32 = 0.046;
const OFF_M: f32 = 0.003;
/// Which way round the bone the patch's middle faces, in radians from the bone's own side:
/// the back of the forearm, seen when the arm hangs.
const TURN: f32 = 0.0;
/// The mark's light: the gate's violet.
const GLOW: Color = Color::srgb(0.78, 0.5, 1.0);

/// The patch a mark is shown by, on a forearm.
#[derive(Component)]
struct Shown;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, show);
}

/// The patch's mesh in the forearm bone's frame: a strip wrapped part way round the bone's
/// line to the hand, its picture upright with the fingers down.
fn patch(to_hand: Vec3) -> Mesh {
    let length = to_hand.length();
    let along = to_hand / length;
    let side = Quat::from_axis_angle(along, TURN) * along.any_orthonormal_vector();
    let across = along.cross(side);
    const STEPS: u32 = 8;
    let (mut positions, mut normals, mut uvs) = (Vec::new(), Vec::new(), Vec::new());
    for step in 0..=STEPS {
        let share = step as f32 / STEPS as f32;
        let angle = (share - 0.5) * ARC;
        let out = side * libm::cosf(angle) + across * libm::sinf(angle);
        for (end, v) in [(0.5 - ALONG / 2.0, 0.0), (0.5 + ALONG / 2.0, 1.0)] {
            positions.push((along * (length * end) + out * (RADIUS_M + OFF_M)).to_array());
            normals.push(out.to_array());
            uvs.push([share, v]);
        }
    }
    let indices = (0..STEPS)
        .flat_map(|step| {
            let at = step * 2;
            [at, at + 1, at + 2, at + 2, at + 1, at + 3]
        })
        .collect();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// Each player's forearm shows the mark they bear, if it has a glyph, and none if they bear
/// none: put on once their figure stands, and again if it is stood anew.
#[allow(clippy::too_many_arguments)]
fn show(
    mut commands: Commands,
    players: Query<(Entity, &Marks), With<Player>>,
    children: Query<&Children>,
    names: Query<&Name>,
    places: Query<&Transform>,
    shown: Query<(), With<Shown>>,
    kinds: Res<Kinds>,
    assets: Res<AssetServer>,
    (mut meshes, mut materials): (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
) {
    for (player, marks) in &players {
        let glyph = kinds.shown(marks);
        let named =
            |entity: Entity, name: &str| names.get(entity).is_ok_and(|n| n.as_str() == name);
        let Some(forearm) = children
            .iter_descendants(player)
            .find(|&bone| named(bone, FOREARM))
        else {
            continue;
        };
        let under = || children.get(forearm).into_iter().flatten().copied();
        let patch_there = under().find(|&child| shown.contains(child));
        match (glyph, patch_there) {
            (Some(_), Some(_)) | (None, None) => {}
            (None, Some(there)) => commands.entity(there).despawn(),
            (Some(glyph), None) => {
                let Some(to_hand) = under()
                    .find(|&child| named(child, HAND))
                    .and_then(|hand| places.get(hand).ok())
                    .map(|hand| hand.translation)
                    .filter(|to_hand| to_hand.length() > 0.01)
                else {
                    continue;
                };
                let material = materials.add(StandardMaterial {
                    base_color: GLOW,
                    base_color_texture: Some(assets.load(glyph.to_string())),
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                });
                let patch = commands
                    .spawn((
                        Name::new("Mark"),
                        Shown,
                        Mesh3d(meshes.add(patch(to_hand))),
                        MeshMaterial3d(material),
                        bevy::light::NotShadowCaster,
                    ))
                    .id();
                // The body may be gone by the time this is done (its world left this
                // frame: a joining game crashed here): then the patch goes with it.
                commands.queue(move |world: &mut World| {
                    if world.get_entity(forearm).is_ok() {
                        world.entity_mut(forearm).add_child(patch);
                    } else if let Ok(patch) = world.get_entity_mut(patch) {
                        patch.despawn();
                    }
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn the_patch_wraps_the_forearm_about_its_middle_off_the_skin() {
        let to_hand = Vec3::new(0.0, 0.26, 0.0);
        let mesh = patch(to_hand);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("the patch has no positions");
        };
        for at in positions {
            let at = Vec3::from_array(*at);
            // Off the bone's line by the arm's radius and a little, between a quarter and
            // three quarters of the way to the hand.
            let off = Vec2::new(at.x, at.z).length();
            assert!((off - (RADIUS_M + OFF_M)).abs() < 1e-4, "{off}");
            assert!(
                (0.25 * 0.26 - 1e-4..=0.75 * 0.26 + 1e-4).contains(&at.y),
                "{}",
                at.y
            );
        }
    }
}
