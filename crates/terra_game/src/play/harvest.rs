//! Gathering: taking a tree, rock or plant out of the world for what is in it. You hold
//! the button until it is done; how long depends on the thing and on the tool. Each job
//! has its own tool, an axe for trees, a pickaxe for rock, a knife for plants, and a
//! tool's material decides what it can work (D96): a stone axe will not fell a great
//! tree.

use super::bag::Notice;
use super::combat::Swing;
use super::controls::{Action, Controls, Settings};
use super::dial::Dial;
use super::flora::{Growth, Obstacles};
use super::inventory::{Belongings, Cut, Cuttable, InventoryScreen, Tool};
use super::skills::{Did, ONCE, Skill, Skills};
use super::work::Hold;
use super::{IsLocalPlayer, Player, Stance};
use crate::planet::MainCamera;
use bevy::prelude::*;

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
        .add_systems(Update, (cut.in_set(Harvesting), fall));
}

fn cut(
    mut commands: Commands,
    time: Res<Time>,
    controls: Controls,
    dial: Res<Dial>,
    camera: Single<&GlobalTransform, With<MainCamera>>,
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
    player: Single<(&Transform, &mut Stance), (IsLocalPlayer, Without<Growth>)>,
    mut cutting: Single<&mut Cutting, IsLocalPlayer>,
    screen: Res<InventoryScreen>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
    mut obstacles: ResMut<Obstacles>,
    skills: Single<&Skills, IsLocalPlayer>,
    swing: Single<&Swing, IsLocalPlayer>,
    mut did: MessageWriter<Did>,
    mut gizmos: Gizmos,
) {
    let (player_at, mut stance) = player.into_inner();
    let feet = player_at.translation;
    cutting.target = None;
    let tool = belongings.holding().filter(|_| !screen.open && !dial.open);
    // Aimed at: anything the tool in hand is the kind of tool for, whether or not its
    // material is up to it, so the prompt can say why it will not cut.
    let works = |cut: &Cuttable| tool.is_some_and(|tool| tool.works(cut.what));
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
    let holding_button = controls.held(Action::Use) && swing.since > 0.6;
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
    let (_, mut hold) = *cutting.held.get_or_insert((
        entity,
        Hold::new(cut.seconds / (speed * skills.speed(skill_for(cut.what))).max(0.01)),
    ));
    let finished = hold.advance(time.delta_secs());
    cutting.held = Some((entity, hold));
    stance.since_acting = 0.0;
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
    belongings.wear_in_hand(1);
    if given < count {
        notice.say(format!(
            "Your backpack is full: {} {} left behind",
            count - given,
            raw.name()
        ));
    }
    if let Some(stood) = obstacle {
        obstacles.remove(stood);
    }
    info!("Cut something for {given} {}.", raw.name());
    did.write(Did(skill_for(cut.what), ONCE));
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
pub fn prompt(
    tool: Tool,
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
            format!("Too tough for a {}", tool.name().to_lowercase())
        }
        Some(cut) => format!(
            "{}  work this      Gives {} {}",
            settings.hold(Action::Use),
            cut.gives.1,
            cut.gives.0.name()
        ),
        None => match tool {
            Tool::StoneAxe | Tool::IronAxe => "Aim at a tree nearby".to_string(),
            Tool::StonePickaxe | Tool::IronPickaxe => "Aim at a rock nearby".to_string(),
            Tool::Knife => "Aim at a bush or plant nearby".to_string(),
            Tool::Hammer => String::new(),
            Tool::Torch => "Lights the way while it is in hand".to_string(),
            Tool::Spear => format!(
                "{}  swing at a creature in front of you",
                settings.key(Action::Use)
            ),
            Tool::Bow => format!(
                "{}  draw, and let go to shoot where you aim",
                settings.hold(Action::Use)
            ),
        },
    }
}

/// The skill gathering a sort of thing grows.
fn skill_for(what: Cut) -> Skill {
    match what {
        Cut::Tree | Cut::GreatTree => Skill::Chopping,
        Cut::Rock | Cut::Ore => Skill::Mining,
        Cut::Plant => Skill::Foraging,
    }
}
