//! What a film sees of the wildlife (D69): while a shot rolls, twice a second of it, the
//! animals the camera draws and nothing hides from it, how big each is on the picture and how
//! far off, told in the log for the trailer to sum up (`tools/sandbox/trailer.ts`). An
//! encounter is staged well when it is seen, big enough to read, for long enough (wildlife's
//! meetings); this measures that rather than squinting at a film.
//!
//! Hidden is what the world knows stands in the way: a trunk, a rock or a log, a tree's crown
//! (a young tree's beside a path is leaves to the ground), the ground, the player's own body,
//! and grass taller than most of the animal in it. Its size is its skeleton's span on the
//! picture, as posed, so a hare sitting up and one stretched out running are each measured as
//! they are.

use super::creatures::Creature;
use super::town_animals::Beast;
use super::walker::IsLocalPlayer;
use super::wildlife::{Asked, Surroundings};
use crate::planet::MainCamera;
use bevy::{
    camera::{primitives::Aabb, visibility::VisibilitySystems},
    mesh::skinning::SkinnedMesh,
    prelude::*,
    transform::TransformSystems,
};
use terra_world::cine::Cine;

/// How often, in seconds of the shot, what is seen is told.
const EVERY_S: f32 = 0.5;
/// The player's body, as the camera sees past it: a post this wide and tall from their feet.
const BODY_M: (f32, f32) = (0.32, 1.8);
/// An animal is hidden in grass when its top is under so much of the grass's height: blades
/// thin out up their length and stand apart, so one with its back at their height shows.
const HIDDEN_UNDER_GRASS: f32 = 0.85;
/// A skeleton's span on the picture leaves out the fur round its bones: so much more.
const FUR: f32 = 1.25;
/// An animal's bones are within this of where it stands.
const JOINTS_WITHIN_M: f32 = 3.0;

pub fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        watch
            .after(TransformSystems::Propagate)
            .after(VisibilitySystems::CheckVisibility)
            .run_if(resource_exists::<Cine>),
    );
}

/// One animal as the picture has it: its kind, how many pixels its widest side spans, and how
/// far it is from the eye.
struct Seen {
    name: String,
    px: f32,
    metres: f32,
}

#[allow(clippy::too_many_arguments)]
fn watch(
    cine: Res<Cine>,
    mut told_at: Local<Option<f32>>,
    around: Surroundings,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    player: Query<&GlobalTransform, IsLocalPlayer>,
    animals: Query<
        (Entity, &Name, &GlobalTransform, Option<&Asked>),
        Or<(With<Creature>, With<Beast>)>,
    >,
    children: Query<&Children>,
    meshes: Query<
        (
            &GlobalTransform,
            &Aabb,
            &ViewVisibility,
            Option<&SkinnedMesh>,
        ),
        With<Mesh3d>,
    >,
    joints: Query<&GlobalTransform>,
) {
    let Some(t) = cine.t else { return };
    if told_at.is_some_and(|last| t < last + EVERY_S - 1e-3) {
        return;
    }
    *told_at = Some(t);
    let Ok((camera, eye)) = camera.single() else {
        return;
    };
    let Some(size) = camera.logical_viewport_size() else {
        return;
    };
    let from = eye.translation();
    let body = player.single().ok().map(|at| at.translation());
    let mut seen = Vec::new();
    let mut hidden = 0;
    // The biggest seen of each meeting the shot asked for (wildlife's `Asked`).
    let mut asked: Vec<(usize, f32)> = Vec::new();
    for (entity, name, root, meeting) in &animals {
        // Its bones where it has them, else its meshes' boxes.
        let mut points = Vec::new();
        let mut drawn = false;
        for mesh in children.iter_descendants(entity) {
            let Ok((at, aabb, visible, skin)) = meshes.get(mesh) else {
                continue;
            };
            drawn |= visible.get();
            if let Some(skin) = skin {
                points.extend(
                    skin.joints
                        .iter()
                        .filter_map(|&joint| joints.get(joint).ok())
                        .map(|joint| joint.translation())
                        // Not an armature's root left at its model's origin.
                        .filter(|joint| joint.distance(root.translation()) < JOINTS_WITHIN_M),
                );
            } else {
                let (middle, half) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
                points.extend((0..8).map(|i| {
                    let sign = Vec3::new(
                        if i & 1 == 0 { -1.0 } else { 1.0 },
                        if i & 2 == 0 { -1.0 } else { 1.0 },
                        if i & 4 == 0 { -1.0 } else { 1.0 },
                    );
                    at.transform_point(middle + half * sign)
                }));
            }
        }
        if !drawn || points.is_empty() {
            continue;
        }
        let middle = points.iter().sum::<Vec3>() / points.len() as f32;
        let on_picture: Vec<Vec2> = points
            .iter()
            .filter_map(|&point| camera.world_to_viewport(eye, point).ok())
            .collect();
        let (low, high) = on_picture.iter().fold(
            (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
            |(low, high), &p| (low.min(p), high.max(p)),
        );
        let inside = |p: Vec2| p.x >= 0.0 && p.y >= 0.0 && p.x <= size.x && p.y <= size.y;
        if on_picture.len() < points.len() || !inside((low + high) * 0.5) {
            continue;
        }
        // How high its top stands over the ground, against the grass there.
        let ground = around.ground(root.translation());
        let up = ground.normalize_or(Vec3::Y);
        let top = points
            .iter()
            .map(|p| (*p - ground).dot(up))
            .fold(0.05f32, f32::max);
        let grass = around.grass_m(ground);
        let in_grass = top < grass * HIDDEN_UNDER_GRASS;
        let behind_player = body.is_some_and(|feet| behind_body(from, middle, feet));
        if in_grass || behind_player || !around.in_view(from, middle) {
            hidden += 1;
            continue;
        }
        let px = (high - low).max_element() * FUR;
        if let Some(&Asked(index)) = meeting {
            match asked.iter_mut().find(|(each, _)| *each == index) {
                Some((_, most)) => *most = most.max(px),
                None => asked.push((index, px)),
            }
        }
        seen.push(Seen {
            // A town's animals are named with their number in the town: "Hen 12".
            name: name
                .trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ')
                .to_string(),
            px,
            metres: from.distance(middle),
        });
    }
    seen.sort_by(|a, b| b.px.total_cmp(&a.px));
    let list: Vec<String> = seen
        .iter()
        .map(|s| format!("{} {:.0} px {:.0} m", s.name, s.px, s.metres))
        .collect();
    info!(
        "Wildlife seen at {t:.1} s: {} ({} hidden){}{}",
        seen.len(),
        hidden,
        if list.is_empty() { "" } else { ": " },
        list.join(", ")
    );
    asked.sort_by_key(|(index, _)| *index);
    for (index, px) in asked {
        info!("Meeting {index} seen at {t:.1} s: {px:.0} px");
    }
}

/// Whether the line from an eye to a point passes behind a person standing with their feet at
/// a point, nearer the eye than it.
fn behind_body(eye: Vec3, at: Vec3, feet: Vec3) -> bool {
    let up = feet.normalize_or(Vec3::Y);
    let line = at - eye;
    // The nearest the line comes to the post of their body, from their feet up.
    (0..=8).any(|i| {
        let point = feet + up * BODY_M.1 * (i as f32 / 8.0);
        let s = ((point - eye).dot(line) / line.length_squared().max(1e-6)).clamp(0.0, 1.0);
        s < 0.98 && (eye + line * s).distance(point) < BODY_M.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_animal_straight_past_the_player_is_hidden_by_them_and_one_aside_is_not() {
        let feet = Vec3::Y * 2000.0;
        let ahead = Vec3::NEG_Z;
        let eye = feet + Vec3::Y * 2.2 - ahead * 4.6;
        let past = feet + ahead * 15.0 + Vec3::Y * 0.2;
        assert!(behind_body(eye, past, feet));
        assert!(!behind_body(eye, past + Vec3::X * 3.0, feet));
        // Nearer the eye than they are, it is in front of them.
        assert!(!behind_body(eye, feet - ahead * 2.0 + Vec3::Y * 1.0, feet));
    }
}
