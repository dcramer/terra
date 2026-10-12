//! Doing it: building, taking apart and moving, over the moment it takes, and showing
//! the work on the thing itself.

use super::*;
use crate::build::Site;
use crate::planet::Planet;
use crate::play::catalogue::{self, Builds, Kind, Tab};
use crate::play::controls::Action;
use crate::play::inventory::Belongings;
use crate::play::noise::{Cue, Noise};
use crate::play::props::{self, Models, PlacedBy, Prop};
use crate::play::skills::{Did, ONCE, Skill};
use crate::play::told::Told;
use crate::play::work::Hold;
use crate::play::{IsLocalPlayer, Stance};
use terra_build::{BuildOp, Face, Panel, RoofSection, Side};

/// Starts, carries on and finishes building and taking apart. Both take a moment: the
/// button is held until the job is done, and letting go early leaves nothing done.
pub(super) fn act(
    mut commands: Commands,
    time: Res<Time>,
    mut planet: ResMut<Planet>,
    models: Res<Models>,
    builds: Res<Builds>,
    options: Res<crate::shell::Options>,
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
            if builder.is_drawing() {
                builder.drag = None;
                builder.laid = None;
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
                        builder.laid = None;
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
                    // On the yard, building costs nothing.
                    if options.yard() || belongings.pay(&item.cost, 1) {
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
        // A wall or room is drawn: pressed, dragged and let go of, it is laid where it was
        // drawn; clicked, it is laid at its own size where the view is. Held still on what
        // is laid, it is built: a click lays, work builds (D144).
        let mut begin = click && !builder.must_release;
        if builder.draws(&builds) {
            if begin {
                builder.drag = builder.point.map(|(point, level)| Drag {
                    from: point,
                    to: point,
                    level,
                    moved: false,
                    still_s: 0.0,
                });
                begin = false;
            } else if let Some(mut drag) = builder.drag.take() {
                if !held {
                    let (from, to, level) = match laid_as(builder.target, &builder.run) {
                        _ if drag.moved => (drag.from, drag.to, drag.level),
                        // A click on what is already laid leaves it as it is.
                        _ if builder.laid.is_some() => continue,
                        Some(shape) => shape,
                        None => continue,
                    };
                    builder.laid = Some(Drawn {
                        from,
                        to,
                        level,
                        part: builder.part,
                        variant: builder.variant(),
                    });
                    info!(
                        "Laid a {} from {from} to {to}, in cells.",
                        builder.picked(&builds).name
                    );
                    noise.write(Noise::at(Cue::Click, player_at));
                } else {
                    // Held still long enough on what is laid, it builds, once. The drag
                    // is kept, so if building cannot begin, moving on still draws.
                    let was = drag.still_s;
                    if !drag.moved {
                        drag.still_s += time.delta_secs();
                    }
                    begin = was < HOLD_TO_BUILD_S
                        && drag.still_s >= HOLD_TO_BUILD_S
                        && builder.laid.is_some_and(|laid| laid.near(drag.from));
                    builder.drag = Some(drag);
                }
            }
        }
        if !begin {
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
        // Fills every cell of the run.
        let fill = |run: &[Face], panel: Panel| -> Vec<BuildOp> {
            run.iter()
                .map(|&cell| BuildOp::SetWall(cell, Some(panel)))
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
            (Kind::Window(shapes), Target::Edge { .. }) if can_pay => (
                building(fill(&builder.run, shapes[builder.variant()].2)),
                false,
            ),
            (Kind::Door, Target::Edge { .. }) if can_pay => {
                (building(fill(&builder.run, Panel::Door)), false)
            }
            // A wall is built where there is none, and leaves alone what is already there.
            (Kind::Wall(_), Target::Edge { fits: true, .. }) if can_pay => (
                building(
                    builder
                        .run
                        .iter()
                        .filter(|&&cell| site.base.filling(cell).is_none())
                        .map(|&cell| BuildOp::SetWall(cell, Some(Panel::Solid)))
                        .collect(),
                ),
                false,
            ),
            // An opening is closed first; a plain wall is taken away.
            (Kind::Remove, Target::Edge { open: true, .. }) => (
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
                        .map(|&cell| BuildOp::SetWall(cell, None))
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
                Kind::Roof(shapes),
                Target::Ground {
                    corner,
                    tiles,
                    adds: true,
                    ..
                },
            ) if can_pay => {
                let (min, max) = (
                    corner.to_array(),
                    (corner + IVec3::new(tiles.x - 1, 0, tiles.y - 1)).to_array(),
                );
                let shape = &shapes[builder.variant()];
                let op = match shape.style {
                    // A gable's ridge runs along its longer side, unless turned.
                    Some(style) => BuildOp::AddRoof(RoofSection {
                        min,
                        max,
                        style,
                        along_x: (tiles.x >= tiles.y) != builder.room_turned,
                        pitch: shape.pitch,
                    }),
                    None => BuildOp::SetRoof {
                        min,
                        max,
                        laid: true,
                    },
                };
                (building(vec![op]), false)
            }
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
        // On the yard, building is all but instant: a moment, so a click is not a build.
        let seconds = if options.yard() { 0.1 } else { seconds };
        builder.drag = None;
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
    mut shapes: super::show::Shapes,
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
            let at = stood.with_scale(Vec3::splat(0.2 + 0.8 * eased));
            shapes.show(rising, mesh, models.lit(), at);
        }
        None => shapes.hide(rising),
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

/// The two points of the grid a room or wall shown at its own size would be drawn between,
/// and its floor, so a click can lay it.
fn laid_as(target: Target, run: &[Face]) -> Option<(IVec2, IVec2, i32)> {
    match target {
        Target::Ground { corner, tiles, .. } => {
            let from = IVec2::new(corner.x, corner.z);
            Some((from, from + tiles, corner.y))
        }
        Target::Edge { .. } => {
            let (first, last) = (run.first()?.tile(), run.last()?.tile());
            Some(if run[0].side == Side::South {
                (
                    IVec2::new(first.x, first.z + 1),
                    IVec2::new(last.x + 1, first.z + 1),
                    first.y,
                )
            } else {
                (
                    IVec2::new(first.x + 1, first.z),
                    IVec2::new(first.x + 1, last.z + 1),
                    first.y,
                )
            })
        }
        _ => None,
    }
}
