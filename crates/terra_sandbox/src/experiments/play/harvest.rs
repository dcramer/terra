//! Cutting: taking a tree, rock or bush out of the world for the raw items in it. You
//! hold the button until it is done; how long depends on the thing and on the tool. It is
//! the one way to get more of anything, and exists so that building has something to
//! spend. How gathering should really work is a design question (docs/player.md, D43).
//!
//! Any tool that says it can cut something is used this way, so an axe or a pickaxe
//! would need nothing new here.

use super::Player;
use super::bag::Notice;
use super::controls::{Action, Controls, Settings};
use super::dial::Dial;
use super::flora::{Growth, Obstacles};
use super::inventory::{Belongings, Cuttable, InventoryScreen};
use super::work::Hold;
use bevy::prelude::*;

/// How far from the player a cutting tool reaches.
const REACH_M: f32 = 5.0;
/// How close something has to be to be cut without being aimed at.
const NEAR_M: f32 = 2.6;
/// How long a cut thing takes to fall and go.
const FALL_SECONDS: f32 = 0.55;

/// What the tool in hand is pointed at, and how far through cutting it is.
#[derive(Resource, Default)]
pub struct Cutting {
    pub target: Option<Entity>,
    held: Option<(Entity, Hold)>,
}

impl Cutting {
    /// How far through the cut is, from 0 to 1, while one is under way.
    pub fn progress(&self) -> Option<f32> {
        self.held.map(|(_, hold)| hold.fraction())
    }
}

/// Something being cut: how it stood before it started to shake.
#[derive(Component)]
struct Shaken(Quat);

/// Something cut through, on its way down.
#[derive(Component)]
struct Felled {
    age: f32,
    /// The level direction it falls over, for things tall enough to fall.
    over: Option<Vec3>,
    from: Transform,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Cutting>()
        .add_systems(Update, (cut, fall));
}

fn cut(
    mut commands: Commands,
    time: Res<Time>,
    controls: Controls,
    dial: Res<Dial>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    mut growth: Query<
        (
            Entity,
            &Growth,
            &mut Transform,
            &Visibility,
            Option<&Shaken>,
        ),
        (Without<Player>, Without<Felled>),
    >,
    player: Single<(&Transform, &mut Player), Without<Growth>>,
    mut cutting: ResMut<Cutting>,
    screen: Res<InventoryScreen>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
    mut obstacles: ResMut<Obstacles>,
    mut gizmos: Gizmos,
) {
    let (player_at, mut player) = player.into_inner();
    let feet = player_at.translation;
    cutting.target = None;
    let tool = belongings.holding().filter(|_| !screen.open && !dial.open);
    let speed_on = |cut: &Cuttable| tool.and_then(|tool| tool.cuts(cut.what));

    // The nearest thing this tool can cut that the view passes close to; failing that,
    // the nearest one just in front of the player, so small things need no careful aim.
    let origin = camera.translation();
    let direction = *camera.forward();
    let level = direction.reject_from(feet.normalize()).normalize_or_zero();
    let within_reach = || {
        growth
            .iter()
            .filter(|(_, _, _, visibility, _)| **visibility != Visibility::Hidden)
            .filter_map(|(entity, growth, transform, _, _)| {
                let cut = growth.cut.filter(|cut| speed_on(cut).is_some())?;
                let up = transform.translation.normalize();
                let middle = transform.translation + up * growth.middle_m;
                (middle.distance_squared(feet) < REACH_M * REACH_M).then_some((
                    entity,
                    cut,
                    middle,
                    growth.middle_m,
                    growth.obstacle,
                ))
            })
    };
    let on_view = within_reach()
        .filter_map(|found| {
            let (_, _, middle, size, _) = found;
            let along = (middle - origin).dot(direction);
            let off = (origin + direction * along).distance(middle);
            (along > 0.0 && off < size.max(0.5)).then_some((found, along))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let in_front = || {
        within_reach()
            .filter(|(_, _, middle, _, _)| {
                let toward = (*middle - feet).reject_from(feet.normalize());
                toward.length() < NEAR_M && toward.normalize_or_zero().dot(level) > 0.0
            })
            .map(|found| (found, found.2.distance(feet)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    };
    let aimed = on_view.or_else(in_front).map(|(found, _)| found);

    let holding_button = controls.held(Action::Use);
    // Whatever was being cut and no longer is stops shaking and stands as it did.
    let still = match (aimed, holding_button) {
        (Some((entity, ..)), true) => Some(entity),
        _ => None,
    };
    if let Some((was, _)) = cutting.held
        && still != Some(was)
    {
        if let Ok((_, _, mut transform, _, Some(shaken))) = growth.get_mut(was) {
            transform.rotation = shaken.0;
            commands.entity(was).remove::<Shaken>();
        }
        cutting.held = None;
    }
    let Some((entity, cut, middle, size, obstacle)) = aimed else {
        return;
    };
    cutting.target = Some(entity);
    let fraction = cutting.progress().unwrap_or(0.0);
    // A ring round what would be cut, filling as it is cut.
    let up = middle.normalize();
    let across = up.any_orthonormal_vector();
    let around = |turn: f32| {
        middle - up * size * 0.9
            + Quat::from_axis_angle(up, turn * std::f32::consts::TAU) * across * size.max(0.5)
    };
    let amber = Color::srgb(1.0, 0.78, 0.25);
    gizmos.linestrip(
        (0..=32).map(|step| around(step as f32 / 32.0)),
        amber.with_alpha(0.45),
    );
    if fraction > 0.0 {
        let steps = (fraction * 32.0).ceil() as usize;
        gizmos.linestrip(
            (0..=steps).map(|step| around(step as f32 / steps as f32 * fraction)),
            Color::WHITE,
        );
    }
    if !holding_button {
        return;
    }

    let Some(speed) = speed_on(&cut) else { return };
    let (_, mut hold) = *cutting
        .held
        .get_or_insert((entity, Hold::new(cut.seconds / speed.max(0.01))));
    let finished = hold.advance(time.delta_secs());
    cutting.held = Some((entity, hold));
    player.since_acting = 0.0;
    let Ok((_, _, mut transform, _, shaken)) = growth.get_mut(entity) else {
        return;
    };
    let rest = shaken.map_or(transform.rotation, |shaken| shaken.0);
    if shaken.is_none() {
        commands.entity(entity).insert(Shaken(rest));
    }
    if !finished {
        // It shudders under the tool, harder as it gives.
        let shudder = libm::sinf(time.elapsed_secs() * 38.0) * 0.012 * (0.4 + hold.fraction());
        transform.rotation = Quat::from_axis_angle(across, shudder) * rest;
        return;
    }

    let (raw, count) = cut.gives;
    // As in Subnautica, nothing is gathered into a full backpack. What does not fit is
    // left behind.
    if !belongings.room_for_one(raw) {
        notice.say("Your backpack is full");
        cutting.held = None;
        transform.rotation = rest;
        commands.entity(entity).remove::<Shaken>();
        return;
    }
    let given = belongings.give(raw, count);
    if given < count {
        notice.say(format!(
            "Your backpack is full: {} {} left behind",
            count - given,
            raw.name()
        ));
    }
    if let Some(stood) = obstacle {
        obstacles.0.retain(|(at, _)| *at != stood);
    }
    info!("Cut something for {given} {}.", raw.name());
    cutting.held = None;
    cutting.target = None;
    transform.rotation = rest;
    let from = *transform;
    // A tree goes over away from whoever cut it; smaller things just go.
    let away = (from.translation - feet)
        .reject_from(up)
        .normalize_or(across);
    commands
        .entity(entity)
        .remove::<(Growth, Shaken)>()
        .insert(Felled {
            age: 0.0,
            over: (size > 1.0).then_some(away),
            from,
        });
}

/// Brings down what has been cut through, then takes it out of the world.
fn fall(
    mut commands: Commands,
    time: Res<Time>,
    mut felled: Query<(Entity, &mut Felled, &mut Transform)>,
) {
    for (entity, mut felled, mut transform) in &mut felled {
        felled.age += time.delta_secs();
        let done = (felled.age / FALL_SECONDS).min(1.0);
        if done >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let up = felled.from.translation.normalize();
        // Slowly at first, then all at once.
        let tipped = done * done;
        if let Some(over) = felled.over {
            let hinge = up.cross(over).normalize_or_zero();
            transform.rotation = Quat::from_axis_angle(hinge, tipped * 1.45) * felled.from.rotation;
        }
        // It sinks out of sight over the last part of the fall.
        let gone = ((done - 0.6) / 0.4).clamp(0.0, 1.0);
        transform.scale = felled.from.scale * (1.0 - 0.85 * gone);
    }
}

/// What the tool in hand would do right now, for the prompt.
pub fn prompt(cutting: &Cutting, growth: &Query<&Growth>, settings: &Settings) -> String {
    if cutting.progress().is_some() {
        return "Cutting      Let go to stop".to_string();
    }
    match cutting
        .target
        .and_then(|entity| growth.get(entity).ok())
        .and_then(|growth| growth.cut)
    {
        Some(cut) => format!(
            "{}  cut this      Gives {} {}",
            settings.hold(Action::Use),
            cut.gives.1,
            cut.gives.0.name()
        ),
        None => "Aim at a tree, a rock or a bush nearby".to_string(),
    }
}
