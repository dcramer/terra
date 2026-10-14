//! Doing it: laying plans at a click, building, taking apart and moving over the moment
//! each takes, and showing the work on the thing itself. What is done to the base goes
//! through `build::act::Building`, which checks it and settles what it costs.

use super::*;
use crate::build::Site;
use crate::build::act::Building;
use crate::planet::Planet;
use crate::play::catalogue::{self, Builds, Kind, Tab};
use crate::play::controls::Action;
use crate::play::noise::{Cue, Noise};
use crate::play::props::{self, Models, PlacedBy, Prop};
use crate::play::skills::{Did, ONCE, Skill};
use crate::play::work::Hold;
use crate::play::{IsLocalPlayer, Stance};
use crate::start::Start;
use terra_build::Bill;

/// Why storage with anything in it is not moved or taken apart (D193).
const EMPTY_FIRST: &str = "Empty it first: nothing is taken apart or moved while it holds anything";

/// How long a piece of work on the base takes: a moment, and a little more for every item
/// it takes or gives back, up to a few seconds. A stand-in, like the costs.
fn seconds(doing: Doing, done: &terra_build::ops::Done) -> f32 {
    let items = |bill: &Bill| -> f32 {
        bill.0
            .values()
            .map(|amount| {
                let (num, den) = amount.parts();
                num as f32 / den as f32
            })
            .sum()
    };
    let build = (0.3 + 0.5 * items(&done.took)).min(2.5);
    match doing {
        Doing::TakeApart => (0.3 + 0.3 * items(&done.gave)).min(1.5),
        _ => build,
    }
}

/// Starts, carries on and finishes building and taking apart. Both take a moment: the
/// button is held until the job is done, and letting go early leaves nothing done.
pub(super) fn act(
    mut commands: Commands,
    time: Res<Time>,
    models: Res<Models>,
    builds: Res<Builds>,
    props: Query<&Prop>,
    mut players: Query<(Entity, &Transform, &Intent, &mut Builder, &mut Stance), With<Player>>,
    mut building: Building,
    start: Res<Start>,
    mut did: MessageWriter<Did>,
    mut noise: MessageWriter<Noise>,
) {
    for (player, player_at, intent, mut builder, mut stance) in &mut players {
        let in_hand = |building: &mut Building| {
            building
                .belongings
                .of(player)
                .is_some_and(|belongings| belongings.tool_in_hand().is_some_and(|tool| tool.builds))
        };
        let player_at = player_at.translation;
        let near = building.local(player_at);
        // A tap of the right button: stop what is being done; or put back what was picked
        // up; or put the part away.
        if std::mem::take(&mut builder.cancel) {
            if builder.work.take().is_some() {
                continue;
            }
            if builder.is_drawing() {
                builder.drag = None;
                continue;
            }
            match builder.carrying.take() {
                Some(carried) => {
                    if let Some(placed) = props::place(
                        &mut commands,
                        &models,
                        &building.site,
                        &building.planet,
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
        // Undo the last thing built or taken apart, within a while (D144): the newer of
        // an item placed or taken away, and a change to the base.
        if intent.starts(Action::Undo) && builder.work.is_none() && in_hand(&mut building) {
            let item_s = builder.done.as_ref().map(|done| done.at_s);
            let base_s = building.last_change_s(player);
            let said = if base_s.is_some_and(|base| item_s.is_none_or(|item| base >= item)) {
                match building.undo(player) {
                    Ok(_) => "Undone".to_string(),
                    Err(why) => why,
                }
            } else {
                undo_item(
                    &mut builder,
                    &time,
                    &props,
                    &mut commands,
                    &models,
                    &mut building,
                    player,
                )
                .to_string()
            };
            info!("Undo: {said}.");
            building.tell(player, said);
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
            let in_hand = !intent.choosing && in_hand(&mut building);
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
                Job::Base {
                    doing,
                    what,
                    ops,
                    part,
                    cube,
                } => {
                    let Ok(done) = building.act(Some(player), &what, &ops) else {
                        continue;
                    };
                    let cue = if doing == Doing::TakeApart {
                        Cue::TakenApart
                    } else {
                        Cue::Built
                    };
                    noise.write(Noise::at(cue, work_at));
                    if !done.took.is_empty()
                        && let Some(mut belongings) = building.belongings.of(player)
                    {
                        did.write(Did(player, Skill::Building, ONCE));
                        belongings.wear_in_hand(1);
                    }
                    // Still held, the next goes in, or comes out, at once (building in
                    // bulk, docs/building.md).
                    let next = match doing {
                        Doing::Build => building.next_work(Some(player), part, near),
                        Doing::TakeApart => {
                            cube.and_then(|cube| building.last_work(Some(player), cube, near))
                        }
                        _ => None,
                    };
                    if let Some(next) = next {
                        match &next.check {
                            Ok(done) => {
                                builder.must_release = false;
                                builder.work = Some(Work {
                                    hold: Hold::new(job_seconds(doing, done, &building)),
                                    job: Job::Base {
                                        doing,
                                        what: next.what.clone(),
                                        ops: next.ops.clone(),
                                        part,
                                        cube,
                                    },
                                    removing: doing == Doing::TakeApart,
                                    began_at: work.began_at,
                                });
                            }
                            Err(why) => building.tell(player, why.message()),
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
                    // Paid for on finishing, from the pack and then the claim's storage
                    // (D141). If what it costs has gone since the button went down,
                    // nothing is built. On the yard, building costs nothing.
                    let paid = building.pay(player, &item.cost, 1);
                    if paid {
                        if let Some(placed) = props::place(
                            &mut commands,
                            &models,
                            &building.site,
                            &building.planet,
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
                            from_start(&building.site, &start, at)
                        );
                        did.write(Did(player, Skill::Building, ONCE));
                        if let Some(mut belongings) = building.belongings.of(player) {
                            belongings.wear_in_hand(1);
                        }
                        noise.write(Noise::at(Cue::Placed, work_at));
                    }
                }
                Job::TakeAway(entity) => {
                    if let Ok(prop) = props.get(entity) {
                        // What does not fit in the backpack drops at the player's feet.
                        if let Some(mut belongings) = building.belongings.of(player) {
                            belongings.refund(&prop.item.cost, 1);
                        }
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

        // The middle button on a cube of the base: its material in hand (D232), as
        // Enshrouded samples a block's.
        if intent.pressed(Action::Copy)
            && builder.viewed.is_none()
            && in_hand(&mut building)
            && let Some(kind) = builder
                .aimed
                .and_then(|cube| building.site.base.cube(cube))
                .map(|cube| cube.kind)
                .filter(|kind| building.rules.names.contains_key(kind))
        {
            builder.material = Some(kind);
            building.tell(
                player,
                format!(
                    "Material: {}",
                    crate::play::controls::capitalised(building.rules.name(kind).to_string())
                ),
            );
            continue;
        }
        // The middle button: another of whatever the view is on.
        if intent.pressed(Action::Copy)
        && !builder.is_carrying()
        && in_hand(&mut building)
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
        // A wall, room or roof is drawn: pressed, dragged and let go of, its plan is laid
        // where it was drawn; clicked, at its own size where the view is. Held still on a
        // plan, what goes in next is built: a click lays, work builds (D144).
        let mut begin = click && !builder.must_release;
        let mut building_held = false;
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
                    // Let go: the plan drawn, or the part's own size, is laid.
                    if drag.still_s < HOLD_TO_BUILD_S
                        && let Some(planned) = builder.planned.clone()
                        && planned.doing == Doing::Lay
                    {
                        if let Some(reason) = builder.unclaimed {
                            building.tell(player, reason);
                            continue;
                        }
                        lay(&mut building, player, &builder, &builds, planned);
                        noise.write(Noise::at(Cue::Click, player_at));
                    }
                } else {
                    // Held still long enough on a plan, what goes in next is built.
                    let was = drag.still_s;
                    if !drag.moved {
                        drag.still_s += time.delta_secs();
                    }
                    let on_plan = builder.aimed.is_some_and(|cube| {
                        building
                            .site
                            .base
                            .cube(cube)
                            .is_some_and(|cube| cube.part.is_some())
                    });
                    begin = was < HOLD_TO_BUILD_S && drag.still_s >= HOLD_TO_BUILD_S && on_plan;
                    building_held = begin;
                    builder.drag = Some(drag);
                }
            }
        }
        if !begin {
            continue;
        }

        if let Some(reason) = builder.unclaimed {
            building.tell(player, reason);
            continue;
        }

        // Moving is neither building nor taking apart, and happens at once.
        match (&builder.picked(&builds).kind, builder.target) {
            (Kind::Move, Target::Prop(entity)) if building.stores.holding(entity) => {
                building.tell(player, EMPTY_FIRST);
                continue;
            }
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
                        &building.site,
                        &building.planet,
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
                        from_start(&building.site, &start, carried.at)
                    );
                    stance.since_acting = 0.0;
                }
                continue;
            }
            _ => {}
        }

        // Work on the base: what was worked out as it was aimed, or, held still on a plan
        // with a wall, room or roof in hand, what goes in next.
        let planned = if building_held {
            let part = builder
                .aimed
                .and_then(|cube| building.site.base.cube(cube))
                .and_then(|cube| cube.part);
            building
                .next_work(Some(player), part, near)
                .map(|work| Planned {
                    doing: Doing::Build,
                    work,
                })
        } else {
            builder.planned.clone()
        };
        if let Some(planned) = planned.filter(|_| !matches!(builder.target, Target::Prop(_))) {
            match (&planned.work.check, planned.doing) {
                (Err(why), _) => {
                    building.tell(player, why.message());
                    builder.must_release = true;
                }
                // A plan, or an opening cut into one, costs nothing and is laid at once.
                (Ok(_), Doing::Lay) => {
                    lay(&mut building, player, &builder, &builds, planned);
                    noise.write(Noise::at(Cue::Click, player_at));
                    builder.must_release = true;
                }
                (Ok(done), doing) => {
                    let part = match &planned.work.ops[..] {
                        [terra_build::ops::Op::Fill { cubes }] => building
                            .site
                            .base
                            .cube(IVec3::from_array(cubes[0]))
                            .and_then(|cube| cube.part),
                        _ => None,
                    };
                    let cube = match builder.target {
                        Target::Structure { cube } => Some(cube),
                        _ => None,
                    };
                    builder.drag = None;
                    builder.work = Some(Work {
                        hold: Hold::new(job_seconds(doing, done, &building)),
                        job: Job::Base {
                            doing,
                            what: planned.work.what.clone(),
                            ops: planned.work.ops.clone(),
                            part,
                            cube,
                        },
                        removing: doing == Doing::TakeApart,
                        began_at: player_at,
                    });
                }
            }
            continue;
        }

        // Items: built from what is carried, and refused when that is not enough; taken
        // apart, everything comes back.
        let can_pay = builder.short.is_none();
        let (job, removing) = match (&builder.picked(&builds).kind, builder.target) {
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
            (Kind::Remove, Target::Prop(entity)) if building.stores.holding(entity) => {
                building.tell(player, EMPTY_FIRST);
                continue;
            }
            (Kind::Remove, Target::Prop(entity)) => (Job::TakeAway(entity), true),
            _ => continue,
        };
        // How long it takes follows from what it costs.
        let seconds = match &job {
            Job::Place { item, .. } => catalogue::build_seconds(&item.cost, 1),
            Job::TakeAway(entity) => match props.get(*entity) {
                Ok(prop) => catalogue::take_apart_seconds(&prop.item.cost, 1),
                Err(_) => continue,
            },
            Job::Base { .. } => continue,
        };
        // On the yard, building is all but instant: a moment, so a click is not a build.
        let seconds = if building.free() { 0.1 } else { seconds };
        builder.drag = None;
        builder.work = Some(Work {
            job,
            hold: Hold::new(seconds),
            removing,
            began_at: player_at,
        });
    }
}

/// How long work on the base takes, all but instant on the yard.
fn job_seconds(doing: Doing, done: &terra_build::ops::Done, building: &Building) -> f32 {
    if building.free() {
        0.1
    } else {
        seconds(doing, done)
    }
}

/// Lays a plan, or cuts an opening into one, at once; a room set into the ground cuts it
/// away first (`Building::lay`).
fn lay(
    building: &mut Building,
    player: Entity,
    builder: &Builder,
    builds: &Builds,
    planned: Planned,
) {
    let room = match (&builder.picked(builds).kind, builder.target) {
        (
            Kind::Room(_),
            Target::Ground {
                corner,
                tiles,
                fits: true,
                clear: true,
                item: None,
                ..
            },
        ) => Some((corner, corner + IVec3::new(tiles.x - 1, 0, tiles.y - 1))),
        (Kind::Room(_), Target::Ground { .. }) => return,
        _ => None,
    };
    let _ = building.lay(Some(player), &planned.work.what, planned.work.ops, room);
}

/// Undoes the last item placed or taken away, within a while; says how it went.
fn undo_item(
    builder: &mut Builder,
    time: &Time,
    props: &Query<&Prop>,
    commands: &mut Commands,
    models: &Models,
    building: &mut Building,
    player: Entity,
) -> &'static str {
    match builder.done.take() {
        Some(done) if time.elapsed_secs() - done.at_s <= UNDO_S => match done.what {
            Undoable::Placed(entity) => match props.get(entity) {
                Ok(prop) => {
                    if let Some(mut belongings) = building.belongings.of(player) {
                        belongings.refund(&prop.item.cost, 1);
                    }
                    commands.entity(entity).despawn();
                    "Undone"
                }
                Err(_) => "It is gone already",
            },
            Undoable::TakenAway(was) => {
                let paid = building.pay(player, &was.item.cost, 1);
                if paid {
                    if let Some(placed) = props::place(
                        commands,
                        models,
                        &building.site,
                        &building.planet,
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
fn from_start(site: &Site, start: &Start, at: Vec3) -> Vec3 {
    start.local(site.frame.transform_point(at))
}
