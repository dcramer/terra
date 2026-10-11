//! Doing it: building, taking apart and moving, over the moment it takes, and showing
//! the work on the thing itself.

use super::*;
use crate::experiments::build::Site;
use crate::experiments::play::Player;
use crate::experiments::play::bag::Notice;
use crate::experiments::play::catalogue::{self, Kind};
use crate::experiments::play::controls::{Action, Controls};
use crate::experiments::play::dial::Dial;
use crate::experiments::play::inventory::{Belongings, Cost, Tool};
use crate::experiments::play::noise::{Cue, Noise};
use crate::experiments::play::props::{self, Models, Prop};
use crate::experiments::play::skills::{Did, ONCE, Skill};
use crate::experiments::play::work::Hold;
use crate::planet::Planet;
use terra_build::{BuildOp, Face, Panel};

/// Starts, carries on and finishes building and taking apart. Both take a moment: the
/// button is held until the job is done, and letting go early leaves nothing done.
pub(super) fn act(
    mut commands: Commands,
    time: Res<Time>,
    controls: Controls,
    dial: Res<Dial>,
    planet: Res<Planet>,
    models: Res<Models>,
    props: Query<&Prop>,
    mut builder: ResMut<Builder>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
    mut site: ResMut<Site>,
    mut did: MessageWriter<Did>,
    mut noise: MessageWriter<Noise>,
    player: Single<(&Transform, &mut Player)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (player_at, mut player) = player.into_inner();
    let player_at = player_at.translation;
    // A tap of the right button: stop what is being done; or put back what was picked
    // up; or put the part away.
    if std::mem::take(&mut builder.cancel) {
        if builder.work.take().is_some() {
            return;
        }
        match builder.carrying.take() {
            Some(carried) => {
                props::place(
                    &mut commands,
                    &models,
                    &site,
                    &planet,
                    carried.item,
                    carried.at,
                    carried.lift,
                    carried.turn,
                    carried.indoors,
                );
            }
            None => builder.stowed = true,
        }
        return;
    }
    let held = controls.held(Action::Use);
    let click = controls.went_down(Action::Use);
    if !held {
        builder.must_release = false;
    }

    // Carry on with what is under way, or drop it.
    if let Some(mut work) = builder.work.take() {
        let stayed = work.began_at.distance(player_at) < WANDER_M;
        let in_hand = !dial.open && belongings.holding() == Some(Tool::Hammer);
        if !held || !stayed || !in_hand {
            return;
        }
        player.since_acting = 0.0;
        if !work.hold.advance(time.delta_secs()) {
            builder.work = Some(work);
            return;
        }
        builder.must_release = true;
        // Heard just in front of the player, where the work is.
        let work_at = player_at + player_at.normalize_or(Vec3::Y);
        match work.job {
            Job::Structure { ops, pay, refund } => {
                // As in Subnautica, nothing is taken apart if what it gives back would
                // not fit in the backpack.
                if let Some((cost, times)) = refund
                    && !belongings.room_for(cost, times)
                {
                    notice.say("No room in your backpack for what that gives back");
                    return;
                }
                if site.apply_all(&ops, &mut commands, &planet, &mut meshes) {
                    let cue = if refund.is_some() {
                        Cue::TakenApart
                    } else {
                        Cue::Built
                    };
                    noise.write(Noise::at(cue, work_at));
                    if pay.is_some() {
                        did.write(Did(Skill::Building, ONCE));
                    }
                    if let Some((cost, times)) = refund {
                        belongings.refund(cost, times);
                    }
                    if let Some((cost, times)) = pay {
                        belongings.pay(cost, times);
                    }
                }
            }
            Job::Place {
                item,
                at,
                lift,
                turn,
                indoors,
            } => {
                // Paid for on finishing. If what it costs has gone since the button
                // went down, nothing is built.
                if belongings.pay(item.cost, 1) {
                    props::place(
                        &mut commands,
                        &models,
                        &site,
                        &planet,
                        item,
                        at,
                        lift,
                        turn,
                        indoors,
                    );
                    info!("Placed a {} at {at}, {lift} up.", item.name);
                    did.write(Did(Skill::Building, ONCE));
                    noise.write(Noise::at(Cue::Placed, work_at));
                }
            }
            Job::TakeAway(entity) => {
                if let Ok(prop) = props.get(entity) {
                    if belongings.refund(prop.item.cost, 1) {
                        info!("Removed a {}.", prop.item.name);
                        noise.write(Noise::at(Cue::TakenApart, work_at));
                        commands.entity(entity).despawn();
                    } else {
                        notice.say("No room in your backpack for what that gives back");
                    }
                }
            }
        }
        return;
    }

    // The middle button: another of whatever the view is on.
    if controls.went_down(Action::Copy)
        && !builder.is_carrying()
        && belongings.holding() == Some(Tool::Hammer)
        && let Some(prop) = builder.viewed.and_then(|entity| props.get(entity).ok())
        && let Some((part, variant)) = catalogue::place_of(prop.item)
        // What is not in the build menu, such as a village's well, cannot be copied.
        && catalogue::PARTS[part].tab != catalogue::Tab::Hidden
    {
        builder.pick(part);
        builder.variants[part] = variant;
        builder.turn = prop.turn;
        return;
    }
    if !click || builder.must_release {
        return;
    }

    if let Some(reason) = builder.unclaimed {
        notice.say(reason);
        return;
    }

    // Moving is neither building nor taking apart, and happens at once.
    match (&builder.picked().kind, builder.target) {
        (Kind::Move, Target::Prop(entity)) => {
            if let Ok(prop) = props.get(entity) {
                info!("Picked up a {}.", prop.item.name);
                builder.turn = prop.turn;
                builder.carrying = Some(Carried {
                    item: prop.item,
                    at: prop.at,
                    lift: prop.lift,
                    turn: prop.turn,
                    indoors: prop.indoors,
                });
                commands.entity(entity).despawn();
                player.since_acting = 0.0;
            }
            return;
        }
        (
            Kind::Move,
            Target::Spot {
                at,
                lift,
                indoors,
                blocked: None,
            },
        ) => {
            if let Some(carried) = builder.carrying.take() {
                props::place(
                    &mut commands,
                    &models,
                    &site,
                    &planet,
                    carried.item,
                    at,
                    lift,
                    builder.turn,
                    indoors,
                );
                info!("Placed a {} at {at}, {lift} up.", carried.item.name);
                player.since_acting = 0.0;
            }
            return;
        }
        _ => {}
    }

    // Building is paid for from what is carried, and refused when that is not enough.
    // Taking apart gives everything back.
    let can_pay = builder.short.is_none();
    let pay = builder.price;
    // What closing a window or doorway gives back.
    let opening_worth = |run: &[Face]| match run.first().and_then(|&cell| site.base.filling(cell)) {
        Some(Panel::Window { .. }) => Some((catalogue::WINDOW, catalogue::window_units(run.len()))),
        Some(Panel::Door) => Some((catalogue::DOOR, 1)),
        _ => None,
    };
    // Fills every cell of the run, in an outside wall or an interior one.
    let fill = |run: &[Face], panel: Panel| -> Vec<BuildOp> {
        run.iter()
            .map(|&cell| {
                if site.base.is_hull(cell) {
                    BuildOp::SetPanel(cell, panel)
                } else {
                    BuildOp::SetPartition(cell, Some(panel))
                }
            })
            .collect()
    };
    let building = |ops: Vec<BuildOp>| Job::Structure {
        ops,
        pay,
        refund: None,
    };
    let unbuilding = |ops: Vec<BuildOp>, refund: Option<(Cost, u32)>| Job::Structure {
        ops,
        pay: None,
        refund,
    };
    let (job, removing) = match (&builder.picked().kind, builder.target) {
        (Kind::Reshape, Target::Wall(face)) if !builder.pull && can_pay => {
            (building(vec![BuildOp::Grow(face)]), false)
        }
        (Kind::Reshape, Target::Wall(face)) if builder.pull => {
            let floor = catalogue::floor_units(site.base.wall(face).len());
            let refund = Some((catalogue::FLOOR, floor));
            (unbuilding(vec![BuildOp::Shrink(face)], refund), true)
        }
        (Kind::Window(shapes), Target::Wall(_) | Target::Edge { .. }) if can_pay => (
            building(fill(&builder.run, shapes[builder.variant()].2)),
            false,
        ),
        (Kind::Door, Target::Wall(_) | Target::Edge { .. }) if can_pay => {
            (building(fill(&builder.run, Panel::Door)), false)
        }
        // A wall is built where there is none, and leaves alone what is already there.
        (Kind::Wall(_), Target::Edge { fits: true, .. }) if can_pay => (
            building(
                builder
                    .run
                    .iter()
                    .filter(|&&cell| site.base.partition(cell).is_none())
                    .map(|&cell| BuildOp::SetPartition(cell, Some(Panel::Solid)))
                    .collect(),
            ),
            false,
        ),
        // An opening is closed first; a plain interior wall is taken away.
        (Kind::Remove, Target::Wall(_) | Target::Edge { open: true, .. }) => (
            unbuilding(
                fill(&builder.run, Panel::Solid),
                opening_worth(&builder.run),
            ),
            true,
        ),
        (Kind::Remove, Target::Edge { .. }) => (
            unbuilding(
                builder
                    .run
                    .iter()
                    .map(|&cell| BuildOp::SetPartition(cell, None))
                    .collect(),
                Some((catalogue::WALL, catalogue::wall_units(builder.run.len()))),
            ),
            true,
        ),
        (
            Kind::Room(_),
            Target::Ground {
                corner,
                tiles,
                fits: true,
                adds: true,
            },
        ) if can_pay => (
            building(vec![BuildOp::AddRoom {
                min: corner.to_array(),
                max: (corner + IVec3::new(tiles.x - 1, 0, tiles.y - 1)).to_array(),
            }]),
            false,
        ),
        (
            Kind::Items(items),
            Target::Spot {
                at,
                lift,
                indoors,
                blocked: None,
            },
        ) if can_pay => (
            Job::Place {
                item: &items[builder.variant()],
                at,
                lift,
                turn: builder.turn,
                indoors,
            },
            false,
        ),
        (Kind::Remove, Target::Prop(entity)) => (Job::TakeAway(entity), true),
        _ => return,
    };
    // How long it takes follows from what it costs.
    let seconds = match &job {
        Job::Structure { ops, .. } if ops.is_empty() => return,
        Job::Structure {
            pay: Some((cost, times)),
            ..
        } => catalogue::build_seconds(cost, *times),
        Job::Structure {
            refund: Some((cost, times)),
            ..
        } => catalogue::take_apart_seconds(cost, *times),
        // Changing the shape of an opening already there costs nothing and is quick.
        Job::Structure { .. } => catalogue::build_seconds(&[], 1),
        Job::Place { item, .. } => catalogue::build_seconds(item.cost, 1),
        Job::TakeAway(entity) => match props.get(*entity) {
            Ok(prop) => catalogue::take_apart_seconds(prop.item.cost, 1),
            Err(_) => return,
        },
    };
    builder.work = Some(Work {
        job,
        hold: Hold::new(seconds),
        removing,
        began_at: player_at,
    });
}

/// Shows work on the thing itself: an item being built grows inside its ghost, and one
/// being taken apart shrinks. Either goes back to how it was if the work is dropped.
pub(super) fn animate(
    mut commands: Commands,
    site: Res<Site>,
    planet: Res<Planet>,
    models: Res<Models>,
    mut builder: ResMut<Builder>,
    mut props: Query<&mut Transform, With<Prop>>,
) {
    let (growing, taking) = match &builder.work {
        Some(Work {
            job:
                Job::Place {
                    item,
                    at,
                    lift,
                    turn,
                    indoors,
                },
            hold,
            ..
        }) => (
            Some((*item, *at, *lift, *turn, *indoors, hold.fraction())),
            None,
        ),
        Some(Work {
            job: Job::TakeAway(entity),
            hold,
            ..
        }) => (None, Some((*entity, hold.fraction()))),
        _ => (None, None),
    };
    let rising = builder.rising;
    match growing.and_then(|growing| Some((growing, models.of(growing.0)?.solid.clone()?))) {
        Some(((item, at, lift, turn, indoors, done), mesh)) => {
            let stood = props::standing(&site, &planet, at, lift + item.lift, turn, indoors);
            // Quick at first, easing to full size.
            let eased = 1.0 - (1.0 - done) * (1.0 - done);
            commands.entity(rising).insert((
                Mesh3d(mesh),
                MeshMaterial3d(models.lit()),
                stood.with_scale(Vec3::splat(0.2 + 0.8 * eased)),
                Visibility::Visible,
            ));
        }
        None => {
            commands.entity(rising).insert(Visibility::Hidden);
        }
    }
    if let Some(was) = builder.shrinking
        && taking.map(|(entity, _)| entity) != Some(was)
    {
        if let Ok(mut transform) = props.get_mut(was) {
            transform.scale = Vec3::ONE;
        }
        builder.shrinking = None;
    }
    if let Some((entity, done)) = taking
        && let Ok(mut transform) = props.get_mut(entity)
    {
        transform.scale = Vec3::splat(1.0 - 0.7 * done * done);
        builder.shrinking = Some(entity);
    }
}
