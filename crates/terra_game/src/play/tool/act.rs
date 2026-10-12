//! Doing it: building, taking apart and moving, over the moment it takes, and showing
//! the work on the thing itself.

use super::*;
use crate::build::Site;
use crate::planet::Planet;
use crate::play::bag::Told;
use crate::play::catalogue::{self, Builds, Kind, Tab};
use crate::play::controls::Action;
use crate::play::inventory::Belongings;
use crate::play::noise::{Cue, Noise};
use crate::play::props::{self, Models, PlacedBy, Prop};
use crate::play::skills::{Did, ONCE, Skill};
use crate::play::work::Hold;
use crate::play::{IsLocalPlayer, Stance};
use terra_build::{BuildOp, Face, Panel};

/// Starts, carries on and finishes building and taking apart. Both take a moment: the
/// button is held until the job is done, and letting go early leaves nothing done.
pub(super) fn act(
    mut commands: Commands,
    time: Res<Time>,
    mut planet: ResMut<Planet>,
    models: Res<Models>,
    builds: Res<Builds>,
    props: Query<&Prop>,
    mut players: Query<(Entity, &Transform, &Intent, &mut Builder, &mut Stance), With<Player>>,
    mut belongings: Belongings,
    mut told: MessageWriter<Told>,
    mut site: ResMut<Site>,
    mut did: MessageWriter<Did>,
    mut noise: MessageWriter<Noise>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (player, player_at, intent, mut builder, mut stance) in &mut players {
        let Some(mut belongings) = belongings.of(player) else {
            continue;
        };
        let player_at = player_at.translation;
        // A tap of the right button: stop what is being done; or put back what was picked
        // up; or put the part away.
        if std::mem::take(&mut builder.cancel) {
            if builder.work.take().is_some() {
                continue;
            }
            match builder.carrying.take() {
                Some(carried) => {
                    if let Some(placed) = props::place(
                        &mut commands,
                        &models,
                        &site,
                        &planet,
                        &carried.item,
                        carried.at,
                        carried.lift,
                        carried.turn,
                        carried.indoors,
                    ) {
                        commands.entity(placed).insert(PlacedBy(player));
                    }
                }
                None => builder.stowed = true,
            }
            continue;
        }
        // Undo the last thing built or taken apart, within a while (D144).
        if intent.starts(Action::Undo)
            && builder.work.is_none()
            && belongings.tool_in_hand().is_some_and(|tool| tool.builds)
        {
            let said = match builder.done.take() {
                Some(done) if time.elapsed_secs() - done.at_s <= UNDO_S => match done.what {
                    Undoable::Structure {
                        before,
                        paid,
                        refunded,
                    } => {
                        // What taking it apart gave back is taken again first.
                        if let Some((cost, times)) = &refunded
                            && !belongings.pay(cost, *times)
                        {
                            "You no longer have what taking it apart gave back"
                        } else {
                            site.replace_with(&before, &mut commands, &planet, &mut meshes);
                            if let Some((cost, times)) = &paid {
                                belongings.refund(cost, *times);
                            }
                            "Undone"
                        }
                    }
                    Undoable::Placed(entity) => match props.get(entity) {
                        Ok(prop) => {
                            belongings.refund(&prop.item.cost, 1);
                            commands.entity(entity).despawn();
                            "Undone"
                        }
                        Err(_) => "It is gone already",
                    },
                    Undoable::TakenAway(was) => {
                        if belongings.pay(&was.item.cost, 1) {
                            if let Some(placed) = props::place(
                                &mut commands,
                                &models,
                                &site,
                                &planet,
                                &was.item,
                                was.at,
                                was.lift,
                                was.turn,
                                was.indoors,
                            ) {
                                commands.entity(placed).insert(PlacedBy(player));
                            }
                            "Undone"
                        } else {
                            "You no longer have what taking it apart gave back"
                        }
                    }
                },
                Some(_) => "Too long ago to undo",
                None => "Nothing to undo",
            };
            info!("Undo: {said}.");
            told.write(Told::new(player, said));
            continue;
        }
        let held = intent.held(Action::Use);
        let click = intent.pressed(Action::Use);
        if !held {
            builder.must_release = false;
        }

        // Carry on with what is under way, or drop it.
        if let Some(mut work) = builder.work.take() {
            let stayed = work.began_at.distance(player_at) < WANDER_M;
            let in_hand =
                !intent.choosing && belongings.tool_in_hand().is_some_and(|tool| tool.builds);
            if !held || !stayed || !in_hand {
                continue;
            }
            stance.since_acting = 0.0;
            if !work.hold.advance(time.delta_secs()) {
                builder.work = Some(work);
                continue;
            }
            builder.must_release = true;
            // Heard just in front of the player, where the work is.
            let work_at = player_at + player_at.normalize_or(Vec3::Y);
            match work.job {
                Job::Structure { ops, pay, refund } => {
                    let before = site.operations.clone();
                    // A room set into the ground cuts the ground away where it stands.
                    for op in &ops {
                        if let BuildOp::AddRoom { min, max } = *op {
                            let cut = site.cut_for_room(IVec3::from(min), IVec3::from(max));
                            planet.apply(&cut);
                        }
                    }
                    if site.apply_all(&ops, &mut commands, &planet, &mut meshes) {
                        let cue = if refund.is_some() {
                            Cue::TakenApart
                        } else {
                            Cue::Built
                        };
                        noise.write(Noise::at(cue, work_at));
                        if pay.is_some() {
                            did.write(Did(belongings.player(), Skill::Building, ONCE));
                            belongings.wear_in_hand(1);
                        }
                        if let Some((cost, times)) = &refund {
                            belongings.refund(cost, *times);
                        }
                        if let Some((cost, times)) = &pay {
                            belongings.pay(cost, *times);
                        }
                        builder.done = Some(Done {
                            what: Undoable::Structure {
                                before,
                                paid: pay,
                                refunded: refund,
                            },
                            at_s: time.elapsed_secs(),
                        });
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
                    if belongings.pay(&item.cost, 1) {
                        if let Some(placed) = props::place(
                            &mut commands,
                            &models,
                            &site,
                            &planet,
                            &item,
                            at,
                            lift,
                            turn,
                            indoors,
                        ) {
                            commands.entity(placed).insert(PlacedBy(player));
                            builder.done = Some(Done {
                                what: Undoable::Placed(placed),
                                at_s: time.elapsed_secs(),
                            });
                        }
                        info!(
                            "Placed a {} at {}, {lift} up.",
                            item.name,
                            from_start(&site, at)
                        );
                        did.write(Did(belongings.player(), Skill::Building, ONCE));
                        belongings.wear_in_hand(1);
                        noise.write(Noise::at(Cue::Placed, work_at));
                    }
                }
                Job::TakeAway(entity) => {
                    if let Ok(prop) = props.get(entity) {
                        // What does not fit in the backpack drops at the player's feet.
                        belongings.refund(&prop.item.cost, 1);
                        info!("Removed a {}.", prop.item.name);
                        builder.done = Some(Done {
                            what: Undoable::TakenAway(Carried {
                                item: prop.item.clone(),
                                at: prop.at,
                                lift: prop.lift,
                                turn: prop.turn,
                                indoors: prop.indoors,
                            }),
                            at_s: time.elapsed_secs(),
                        });
                        noise.write(Noise::at(Cue::TakenApart, work_at));
                        commands.entity(entity).despawn();
                    }
                }
            }
            continue;
        }

        // The middle button: another of whatever the view is on.
        if intent.pressed(Action::Copy)
        && !builder.is_carrying()
        && belongings.tool_in_hand().is_some_and(|tool| tool.builds)
        && let Some(prop) = builder.viewed.and_then(|entity| props.get(entity).ok())
        && let Some((part, variant)) = builds.place_of(&prop.item)
        // What is not in the build menu, such as a village's well, cannot be copied.
        && builds.part(part).tab != Tab::Hidden
        {
            builder.pick(part, &builds);
            builder.choose_variant(part, variant, &builds);
            builder.turn = prop.turn;
            continue;
        }
        if !click || builder.must_release {
            continue;
        }

        if let Some(reason) = builder.unclaimed {
            told.write(Told::new(player, reason));
            continue;
        }

        // Moving is neither building nor taking apart, and happens at once.
        match (&builder.picked(&builds).kind, builder.target) {
            (Kind::Move, Target::Prop(entity)) => {
                if let Ok(prop) = props.get(entity) {
                    info!("Picked up a {}.", prop.item.name);
                    builder.turn = prop.turn;
                    builder.carrying = Some(Carried {
                        item: prop.item.clone(),
                        at: prop.at,
                        lift: prop.lift,
                        turn: prop.turn,
                        indoors: prop.indoors,
                    });
                    commands.entity(entity).despawn();
                    stance.since_acting = 0.0;
                }
                continue;
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
                    if let Some(placed) = props::place(
                        &mut commands,
                        &models,
                        &site,
                        &planet,
                        &carried.item,
                        at,
                        lift,
                        builder.turn,
                        indoors,
                    ) {
                        commands.entity(placed).insert(PlacedBy(player));
                    }
                    info!(
                        "Placed a {} at {}, {lift} up.",
                        carried.item.name,
                        from_start(&site, carried.at)
                    );
                    stance.since_acting = 0.0;
                }
                continue;
            }
            _ => {}
        }

        // Building is paid for from what is carried, and refused when that is not enough.
        // Taking apart gives everything back.
        let can_pay = builder.short.is_none();
        let pay = builder.price.clone();
        // What closing a window or doorway gives back.
        let opening_worth =
            |run: &[Face]| match run.first().and_then(|&cell| site.base.filling(cell)) {
                Some(Panel::Window { .. }) => {
                    Some((builds.window().to_vec(), catalogue::window_units(run.len())))
                }
                Some(Panel::Door) => Some((builds.door().to_vec(), 1)),
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
            pay: pay.clone(),
            refund: None,
        };
        let unbuilding = |ops: Vec<BuildOp>, refund: Option<Price>| Job::Structure {
            ops,
            pay: None,
            refund,
        };
        let (job, removing) = match (&builder.picked(&builds).kind, builder.target) {
            (Kind::Reshape, Target::Wall(face)) if !builder.pull && can_pay => {
                (building(vec![BuildOp::Grow(face)]), false)
            }
            (Kind::Reshape, Target::Wall(face)) if builder.pull => {
                let floor = catalogue::floor_units(site.base.wall(face).len());
                let refund = Some((builds.floor().to_vec(), floor));
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
                    Some((
                        builds.wall().to_vec(),
                        catalogue::wall_units(builder.run.len()),
                    )),
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
                    clear: true,
                    item: None,
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
                    item: items[builder.variant()].clone(),
                    at,
                    lift,
                    turn: builder.turn,
                    indoors,
                },
                false,
            ),
            (Kind::Remove, Target::Prop(entity)) => (Job::TakeAway(entity), true),
            _ => continue,
        };
        // How long it takes follows from what it costs.
        let seconds = match &job {
            Job::Structure { ops, .. } if ops.is_empty() => continue,
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
            Job::Place { item, .. } => catalogue::build_seconds(&item.cost, 1),
            Job::TakeAway(entity) => match props.get(*entity) {
                Ok(prop) => catalogue::take_apart_seconds(&prop.item.cost, 1),
                Err(_) => continue,
            },
        };
        builder.work = Some(Work {
            job,
            hold: Hold::new(seconds),
            removing,
            began_at: player_at,
        });
    }
}

/// Shows work on the thing itself: an item being built grows inside its ghost, and one
/// being taken apart shrinks. Either goes back to how it was if the work is dropped.
pub(super) fn animate(
    mut commands: Commands,
    site: Res<Site>,
    planet: Res<Planet>,
    models: Res<Models>,
    mut builder: Single<&mut Builder, IsLocalPlayer>,
    drawn: Res<BuilderLook>,
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
            Some((item.clone(), *at, *lift, *turn, *indoors, hold.fraction())),
            None,
        ),
        Some(Work {
            job: Job::TakeAway(entity),
            hold,
            ..
        }) => (None, Some((*entity, hold.fraction()))),
        _ => (None, None),
    };
    let rising = drawn.rising;
    let mesh = |item: &Item| models.of(item)?.solid.clone();
    match growing.and_then(|growing| Some((mesh(&growing.0)?, growing))) {
        Some((mesh, (item, at, lift, turn, indoors, done))) => {
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

/// A place in the base's own space as metres east, up and south of the start, which is how
/// scripts and the log give places, wherever the base was begun.
fn from_start(site: &Site, at: Vec3) -> Vec3 {
    let world = site.frame.transform_point(at);
    site.start
        .compute_affine()
        .inverse()
        .transform_point3(world)
}
