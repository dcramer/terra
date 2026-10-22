//! Gathering: taking a tree, rock or plant out of the world for what is in it. You hold
//! the button until it is done; how long depends on the thing and on the tool. Each job
//! has its own tool, an axe for trees, a pickaxe for rock, a knife for plants, and a
//! tool's material decides what it can work (D96): a stone axe will not fell a great
//! tree.

use crate::inventory::{Belongings, Cuttable, Items};
use crate::kinds::{CutSkill, ToolDef};
use crate::registries::FloraGives;
use crate::skills::{Did, ONCE, Skills};
use crate::work::Hold;
use bevy::prelude::*;
use terra_walker::controls::{Action, Settings};
use terra_walker::intent::Intent;
use terra_walker::{LocalPlayer, Player, Stance};
use terra_world::flora::{Growth, Obstacles};
use terra_world::tick::Sim;

/// How far from the player a cutting tool reaches.
const REACH_M: f32 = 5.0;
/// How close something has to be to be cut without being aimed at.
const NEAR_M: f32 = 2.6;
/// How long a cut thing takes to fall and go.
const FALL_SECONDS: f32 = 0.55;

/// What the tool in hand is pointed at, and how far through cutting it is.
#[derive(Component, Default)]
pub struct Cutting {
    pub target: Option<Entity>,
    held: Option<(Entity, Hold)>,
}

impl Cutting {
    /// How far through the cut is, from 0 to 1, while one is under way.
    pub fn progress(&self) -> Option<f32> {
        self.held.map(|(_, hold)| hold.fraction())
    }

    /// What is being cut, while a cut is under way.
    pub fn held(&self) -> Option<Entity> {
        self.held.map(|(entity, _)| entity)
    }
}

/// When a player last swung the tool in hand, at a creature (`combat`) as with a cut, so
/// another waits its turn and gathering waits for it.
#[derive(Component)]
pub struct Swing {
    pub since: f32,
}

impl Default for Swing {
    fn default() -> Self {
        Self { since: f32::MAX }
    }
}

/// Something being cut: how it stood before it started to shake.
#[derive(Component)]
struct Shaken(Quat);

/// Something cut through, on its way down.
#[derive(Component)]
pub struct Felled {
    age: f32,
    /// The level direction it falls over, for things tall enough to fall.
    over: Option<Vec3>,
    from: Transform,
}

impl Felled {
    /// Whether it is big enough to fall over, like a tree, rather than just go.
    pub fn tall(&self) -> bool {
        self.over.is_some()
    }
}

/// Choosing what to cut and cutting it, which digging waits on: a pickaxe digs only where
/// nothing is to be cut.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Harvesting;

pub fn plugin(app: &mut App) {
    app.register_required_components::<Player, Cutting>()
        .register_required_components::<Player, Swing>()
        .add_systems(FixedUpdate, cut.in_set(Harvesting).in_set(Sim::Act))
        .add_systems(Update, fall);
}

fn cut(
    mut commands: Commands,
    time: Res<Time>,
    flora_gives: Res<FloraGives>,
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
    mut players: Query<
        (
            Entity,
            &Transform,
            &Intent,
            &Skills,
            &Swing,
            &mut Stance,
            &mut Cutting,
            Has<LocalPlayer>,
        ),
        (With<Player>, Without<Growth>),
    >,
    mut belongings: Belongings,
    mut obstacles: ResMut<Obstacles>,
    mut did: MessageWriter<Did>,
    mut gizmos: Gizmos,
) {
    for (player, player_at, intent, skills, swing, mut stance, mut cutting, ours) in &mut players {
        let Some(mut belongings) = belongings.of(player) else {
            continue;
        };
        let feet = player_at.translation;
        cutting.target = None;
        let tool = belongings.tool_in_hand().cloned().filter(|_| intent.free());
        // Aimed at: anything the tool in hand is the kind of tool for, whether or not its
        // material is up to it, so the prompt can say why it will not cut.
        let works = |cut: &Cuttable| tool.as_ref().is_some_and(|tool| tool.works(cut.what));
        let speed_on = |cut: &Cuttable| tool.as_ref().and_then(|tool| tool.cuts(cut.what));

        // The nearest thing this tool can cut that the view passes close to; failing that,
        // the nearest one just in front of the player, so small things need no careful aim.
        let Some(aim) = intent.aim else {
            continue;
        };
        let (origin, direction) = (aim.origin, *aim.direction);
        let level = direction.reject_from(feet.normalize()).normalize_or_zero();
        let within_reach = || {
            growth
                .iter()
                .filter(|(_, _, _, visibility, _)| **visibility != Visibility::Hidden)
                .filter_map(|(entity, growth, transform, _, _)| {
                    let cut = growth.cut.filter(|cut| works(cut))?;
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

        // A click that swung at a creature is not a cut.
        let holding_button = intent.held(Action::Use) && swing.since > 0.6;
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
            continue;
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
        if ours {
            gizmos.linestrip(
                (0..=32).map(|step| around(step as f32 / 32.0)),
                amber.with_alpha(0.45),
            );
        }
        if ours && fraction > 0.0 {
            let steps = (fraction * 32.0).ceil() as usize;
            gizmos.linestrip(
                (0..=steps).map(|step| around(step as f32 / steps as f32 * fraction)),
                Color::WHITE,
            );
        }
        if !holding_button {
            continue;
        }

        let Some(speed) = speed_on(&cut) else { return };
        let (_, mut hold) = *cutting.held.get_or_insert((
            entity,
            Hold::new(cut.seconds / (speed * skills.speed(cut.what.skill())).max(0.01)),
        ));
        let finished = hold.advance(time.delta_secs());
        cutting.held = Some((entity, hold));
        stance.since_acting = 0.0;
        let Ok((_, _, mut transform, _, shaken)) = growth.get_mut(entity) else {
            continue;
        };
        let rest = shaken.map_or(transform.rotation, |shaken| shaken.0);
        if shaken.is_none() {
            commands.entity(entity).insert(Shaken(rest));
        }
        if !finished {
            // It shudders under the tool, harder as it gives.
            let shudder = libm::sinf(time.elapsed_secs() * 38.0) * 0.012 * (0.4 + hold.fraction());
            transform.rotation = Quat::from_axis_angle(across, shudder) * rest;
            continue;
        }

        let (flora, count) = cut.gives;
        let kind = flora_gives.item(flora);
        let gives = belongings.items().name(kind).to_string();
        // What does not fit in the backpack drops at the player's feet (`pile`).
        let given = belongings.give(kind, count);
        belongings.wear_in_hand(1);
        if let Some(stood) = obstacle {
            obstacles.remove(stood);
        }
        info!("Cut something for {given} {gives}.");
        did.write(Did(player, cut.what.skill(), ONCE));
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

/// What the tool in hand would do right now, for the prompt: a kind of tool called
/// `name`, which does `tool`.
pub fn prompt(
    items: &Items,
    flora_gives: &FloraGives,
    name: &str,
    tool: &ToolDef,
    cutting: &Cutting,
    growth: &Query<&Growth>,
    settings: &Settings,
) -> String {
    if cutting.progress().is_some() {
        return "Working      Let go to stop".to_string();
    }
    let aimed = cutting
        .target
        .and_then(|entity| growth.get(entity).ok())
        .and_then(|growth| growth.cut);
    match aimed {
        Some(cut) if tool.cuts(cut.what).is_none() => {
            format!("Too tough for a {}", name.to_lowercase())
        }
        Some(cut) => format!(
            "{}  work this      Gives {} {}",
            settings.hold(Action::Use),
            cut.gives.1,
            items.name(flora_gives.item(cut.gives.0))
        ),
        None => tool
            .hint(&settings.key(Action::Use), &settings.hold(Action::Use))
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_player_comes_with_a_swing_long_past_so_the_first_click_may_cut() {
        let mut app = App::new();
        plugin(&mut app);
        let player = app.world_mut().spawn(Player).id();
        let swing = app
            .world()
            .get::<Swing>(player)
            .expect("gathering gives a swing");
        assert!(swing.since > 0.6);
    }
}
