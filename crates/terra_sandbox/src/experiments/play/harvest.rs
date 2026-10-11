//! The cutter: a tool that takes a tree, rock or bush out of the world and gives the raw
//! items it was made of. It is the one way to get more of anything, and exists so that
//! building has something to spend. How gathering should really work is a design question
//! (docs/player.md, D43).

use super::Player;
use super::dial::Dial;
use super::flora::{Growth, Obstacles};
use super::inventory::{Inventory, Tool};
use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

/// How far from the player the cutter reaches.
const REACH_M: f32 = 5.0;
/// How close something has to be to be cut without being aimed at.
const NEAR_M: f32 = 2.6;

/// What the cutter is pointed at, for the prompt.
#[derive(Resource, Default)]
pub struct Cutting(pub Option<Entity>);

pub fn plugin(app: &mut App) {
    app.init_resource::<Cutting>().add_systems(Update, cut);
}

fn cut(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions>,
    dial: Res<Dial>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    growth: Query<(Entity, &Growth, &Transform, &Visibility)>,
    mut player: Single<(&Transform, &mut Player)>,
    mut cutting: ResMut<Cutting>,
    mut inventory: ResMut<Inventory>,
    mut obstacles: ResMut<Obstacles>,
    mut gizmos: Gizmos,
) {
    cutting.0 = None;
    if inventory.open || dial.open || inventory.holding() != Some(Tool::Cutter) {
        return;
    }
    let origin = camera.translation();
    let direction = *camera.forward();
    let feet = player.0.translation;
    // The nearest thing worth cutting that the view passes close to; failing that, the
    // nearest one just in front of the player, so small things need no careful aim.
    let level = direction.reject_from(feet.normalize()).normalize_or_zero();
    let within_reach = || {
        growth
            .iter()
            .filter(|(_, growth, _, visibility)| {
                growth.gives.is_some() && **visibility != Visibility::Hidden
            })
            .map(|(entity, growth, transform, _)| {
                let up = transform.translation.normalize();
                (entity, growth, transform.translation + up * growth.middle_m)
            })
            .filter(|(_, _, middle)| middle.distance_squared(feet) < REACH_M * REACH_M)
    };
    let on_view = within_reach()
        .filter_map(|(entity, growth, middle)| {
            let along = (middle - origin).dot(direction);
            let off = (origin + direction * along).distance(middle);
            (along > 0.0 && off < growth.middle_m.max(0.5))
                .then_some((entity, growth, middle, along))
        })
        .min_by(|a, b| a.3.total_cmp(&b.3));
    let in_front = || {
        within_reach()
            .filter(|(_, _, middle)| {
                let toward = (*middle - feet).reject_from(feet.normalize());
                toward.length() < NEAR_M && toward.normalize_or_zero().dot(level) > 0.0
            })
            .map(|(entity, growth, middle)| (entity, growth, middle, middle.distance(feet)))
            .min_by(|a, b| a.3.total_cmp(&b.3))
    };
    let aimed = on_view.or_else(in_front);
    let Some((entity, growth, middle, _)) = aimed else {
        return;
    };
    cutting.0 = Some(entity);
    // A ring round what would be cut.
    let up = middle.normalize();
    let across = up.any_orthonormal_vector();
    let ring: Vec<Vec3> = (0..=24)
        .map(|step| {
            let turn = Quat::from_axis_angle(up, step as f32 / 24.0 * std::f32::consts::TAU);
            middle - up * growth.middle_m * 0.9 + turn * across * growth.middle_m.max(0.5)
        })
        .collect();
    gizmos.linestrip(ring, Color::srgb(1.0, 0.78, 0.25));

    let captured = cursor.grab_mode != CursorGrabMode::None;
    let click =
        keys.just_pressed(KeyCode::KeyE) || (captured && mouse.just_pressed(MouseButton::Left));
    if click && let Some((raw, count)) = growth.gives {
        inventory.add(raw, count);
        if let Some(stood) = growth.obstacle {
            obstacles.0.retain(|(at, _)| *at != stood);
        }
        commands.entity(entity).despawn();
        player.1.since_acting = 0.0;
        info!("Cut something for {count} {}.", raw.name());
    }
}

/// What the cutter would do right now, for the prompt.
pub fn prompt(cutting: &Cutting, growth: &Query<&Growth>) -> String {
    match cutting.0.and_then(|entity| growth.get(entity).ok()) {
        Some(Growth {
            gives: Some((raw, count)),
            ..
        }) => format!("Left click  cut this      Gives {count} {}", raw.name()),
        _ => "Aim at a tree, a rock or a bush nearby".to_string(),
    }
}
