//! Doing it: laying plans at a click, building, taking apart and moving over the moment
//! each takes, and showing the work on the thing itself. What is done to the base goes
//! through `act::Building`, which checks it and settles what it costs.

use super::*;
use crate::act::Building;
use bevy::ecs::system::SystemParam;
use terra_bases::Site;
use terra_bases::hammer::Hammer;
use terra_build::Bill;
use terra_survival::catalogue::{self, Builds, Kind, Tab};
use terra_survival::noise::{Cue, Noise};
use terra_survival::props::{self, Models, PlacedBy, Prop};
use terra_survival::skills::{Did, ONCE, Skill};
use terra_survival::work::Hold;
use terra_walker::controls::Action;
use terra_walker::{IsLocalPlayer, Stance};
use terra_world::planet::Planet;
use terra_world::start::Start;

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

/// What `act` works with besides the players: the base, the items standing about, and the
/// cues it sends.
#[derive(SystemParam)]
pub(super) struct Acting<'w, 's> {
    commands: Commands<'w, 's>,
    time: Res<'w, Time>,
    models: Res<'w, Models>,
    builds: Res<'w, Builds>,
    props: Query<'w, 's, &'static Prop>,
    building: Building<'w, 's>,
    start: Res<'w, Start>,
    did: MessageWriter<'w, Did>,
    noise: MessageWriter<'w, Noise>,
}

/// A player acting: who, where, and where in the base's own space.
#[derive(Clone, Copy)]
struct Actor {
    player: Entity,
    at: Vec3,
    near: Vec3,
}

/// Whether a player has a building tool in hand.
fn in_hand(building: &mut Building, player: Entity) -> bool {
    building
        .belongings
        .of(player)
        .is_some_and(|belongings| belongings.tool_in_hand().is_some_and(|tool| tool.builds))
}

/// Starts, carries on and finishes building and taking apart. Both take a moment: the
/// button is held until the job is done, and letting go early leaves nothing done. Each
/// step below either deals with the player this frame or passes them on to the next.
pub(super) fn act(
    mut players: Query<
        (
            Entity,
            &Transform,
            &Intent,
            &mut Builder,
            &mut Stance,
            &mut Hammer,
        ),
        With<Player>,
    >,
    mut cx: Acting,
) {
    for (player, player_at, intent, mut builder, mut stance, mut hammer) in &mut players {
        'acting: {
            let player_at = player_at.translation;
            let near = cx.building.local(player_at);
            let me = Actor {
                player,
                at: player_at,
                near,
            };
            if cancel(&mut cx, me, &mut builder) || undo(&mut cx, me, intent, &mut builder) {
                break 'acting;
            }
            let held = intent.held(Action::Use);
            let click = intent.pressed(Action::Use);
            if !held {
                builder.must_release = false;
            }
            if carry_on(&mut cx, me, intent, held, &mut builder, &mut stance)
                || copy(&mut cx, me, intent, &mut builder)
            {
                break 'acting;
            }
            let Some(building_held) = draw(&mut cx, me, click, held, &mut builder) else {
                break 'acting;
            };
            if let Some(reason) = builder.unclaimed {
                cx.building.tell(player, reason);
                break 'acting;
            }
            if move_item(&mut cx, me, &mut builder, &mut stance)
                || begin_on_base(&mut cx, me, building_held, &mut builder)
            {
                break 'acting;
            }
            begin_on_items(&mut cx, me, &mut builder);
        }
        // What was picked up, put away or copied is shown to the layers below at once.
        hammer.set_if_neq(builder.hammer());
    }
}

/// A tap of the right button: stop what is being done; or put back what was picked up; or
/// put the part away. True if it was tapped.
fn cancel(cx: &mut Acting, me: Actor, builder: &mut Mut<Builder>) -> bool {
    if !std::mem::take(&mut builder.cancel) {
        return false;
    }
    if builder.work.take().is_some() {
        return true;
    }
    if builder.is_drawing() {
        builder.drag = None;
        return true;
    }
    match builder.carrying.take() {
        Some(carried) => {
            if let Some(placed) = props::place(
                &mut cx.commands,
                &cx.models,
                &cx.building.site,
                &cx.building.planet,
                &carried.item,
                carried.at,
                carried.lift,
                carried.turn,
                carried.indoors,
            ) {
                cx.commands.entity(placed).insert(PlacedBy(me.player));
            }
        }
        None => builder.stowed = true,
    }
    true
}

/// Undo the last thing built or taken apart, within a while (D144): the newer of an item
/// placed or taken away, and a change to the base. True if it was asked for.
fn undo(cx: &mut Acting, me: Actor, intent: &Intent, builder: &mut Mut<Builder>) -> bool {
    let player = me.player;
    if !(intent.starts(Action::Undo) && builder.work.is_none() && in_hand(&mut cx.building, player))
    {
        return false;
    }
    let item_s = builder.done.as_ref().map(|done| done.at_s);
    let base_s = cx.building.last_change_s(player);
    let said = if base_s.is_some_and(|base| item_s.is_none_or(|item| base >= item)) {
        match cx.building.undo(player) {
            Ok(_) => "Undone".to_string(),
            Err(why) => why,
        }
    } else {
        undo_item(
            builder,
            &cx.time,
            &cx.props,
            &mut cx.commands,
            &cx.models,
            &mut cx.building,
            player,
        )
        .to_string()
    };
    info!("Undo: {said}.");
    cx.building.tell(player, said);
    true
}

/// Work already begun: the next step once one is in, while the button is still held; or
/// what is under way carried on, finished or dropped. True if there was any.
fn carry_on(
    cx: &mut Acting,
    me: Actor,
    intent: &Intent,
    held: bool,
    builder: &mut Mut<Builder>,
    stance: &mut Mut<Stance>,
) -> bool {
    let Actor { player, near, .. } = me;
    // Still held after a step went off to be worked out: the next once it is in, as soon
    // as the base is as that step left it, its moment shortened by the wait.
    if let Some(mut following) = builder.following.take() {
        let stayed = following.began_at.distance(me.at) < WANDER_M;
        if !held || !stayed || intent.choosing || !in_hand(&mut cx.building, player) {
            return true;
        }
        stance.since_acting = 0.0;
        if cx.building.busy() {
            following.waited_s += cx.time.delta_secs();
            builder.must_release = true;
            builder.following = Some(following);
            return true;
        }
        builder.work = follow(&mut cx.building, player, near, &following);
        builder.must_release = builder.work.is_none();
        return true;
    }
    // Carry on with what is under way, or drop it.
    let Some(mut work) = builder.work.take() else {
        return false;
    };
    let stayed = work.began_at.distance(me.at) < WANDER_M;
    let in_hand = !intent.choosing && in_hand(&mut cx.building, player);
    if !held || !stayed || !in_hand {
        return true;
    }
    stance.since_acting = 0.0;
    if !work.hold.advance(cx.time.delta_secs()) {
        builder.work = Some(work);
        return true;
    }
    builder.must_release = true;
    finish(cx, me, work, builder);
    true
}

/// Work whose moment is up, done: a step on the base sent off to be worked out, an item
/// placed and paid for, or one taken away and given back.
fn finish(cx: &mut Acting, me: Actor, work: Work, builder: &mut Mut<Builder>) {
    let Actor {
        player,
        at: player_at,
        near,
    } = me;
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
            // Checked here, so a refusal is told at once; done on a worker and swapped in
            // when it is worked out, so a step on a big base never holds up the frame
            // (D339).
            let done = match cx.building.check(Some(player), &ops) {
                Ok(done) => done,
                Err(why) => {
                    cx.building.tell(player, why.message());
                    return;
                }
            };
            cx.building.act_later(Some(player), &what, ops);
            let cue = if doing == Doing::TakeApart {
                Cue::TakenApart
            } else {
                Cue::Built
            };
            cx.noise.write(Noise::at(cue, work_at));
            if !done.took.is_empty()
                && let Some(mut belongings) = cx.building.belongings.of(player)
            {
                cx.did.write(Did(player, Skill::Building, ONCE));
                belongings.wear_in_hand(1);
            }
            // Still held, the next goes in, or comes out, at once (building in bulk,
            // docs/building.md): chosen once this step is in.
            let following = Following {
                doing,
                part,
                cube,
                began_at: work.began_at,
                waited_s: 0.0,
            };
            if cx.building.busy() {
                builder.following = Some(following);
            } else {
                builder.work = follow(&mut cx.building, player, near, &following);
                builder.must_release = builder.work.is_none();
            }
        }
        Job::Place {
            item,
            at,
            lift,
            turn,
            indoors,
        } => {
            // Paid for on finishing, from the pack and then the claim's storage (D141). If
            // what it costs has gone since the button went down, nothing is built. On the
            // yard, building costs nothing.
            let paid = cx.building.pay(player, &item.cost, 1);
            if paid {
                if let Some(placed) = props::place(
                    &mut cx.commands,
                    &cx.models,
                    &cx.building.site,
                    &cx.building.planet,
                    &item,
                    at,
                    lift,
                    turn,
                    indoors,
                ) {
                    cx.commands.entity(placed).insert(PlacedBy(player));
                    builder.done = Some(Done {
                        what: Undoable::Placed(placed),
                        at_s: cx.time.elapsed_secs(),
                    });
                }
                info!(
                    "Placed a {} at {}, {lift} up.",
                    item.name,
                    from_start(&cx.building.site, &cx.start, at)
                );
                cx.did.write(Did(player, Skill::Building, ONCE));
                if let Some(mut belongings) = cx.building.belongings.of(player) {
                    belongings.wear_in_hand(1);
                }
                cx.noise.write(Noise::at(Cue::Placed, work_at));
            }
        }
        Job::TakeAway(entity) => {
            if let Ok(prop) = cx.props.get(entity) {
                // What does not fit in the backpack drops at the player's feet.
                if let Some(mut belongings) = cx.building.belongings.of(player) {
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
                    at_s: cx.time.elapsed_secs(),
                });
                cx.noise.write(Noise::at(Cue::TakenApart, work_at));
                cx.commands.entity(entity).despawn();
            }
        }
    }
}

/// The middle button: on a cube of the base, its material in hand (D232), as Enshrouded
/// samples a block's; on an item, another of it. True if either was taken up.
fn copy(cx: &mut Acting, me: Actor, intent: &Intent, builder: &mut Mut<Builder>) -> bool {
    let player = me.player;
    let building = &mut cx.building;
    if intent.pressed(Action::Copy)
        && builder.viewed.is_none()
        && in_hand(building, player)
        && let Some(kind) = builder
            .aimed
            .and_then(|cube| building.site.base.cube(cube))
            .map(|cube| cube.kind)
            .filter(|kind| building.rules.hammer.contains(kind))
    {
        builder.material = Some(kind);
        building.tell(
            player,
            format!(
                "Material: {}",
                terra_walker::controls::capitalised(building.rules.name(kind).to_string())
            ),
        );
        return true;
    }
    let builds = &cx.builds;
    if intent.pressed(Action::Copy)
    && !builder.is_carrying()
    && in_hand(building, player)
    && let Some(prop) = builder.viewed.and_then(|entity| cx.props.get(entity).ok())
    && let Some((part, variant)) = builds.place_of(&prop.item)
    // What is not in the build menu, such as a village's well, cannot be copied.
    && builds.part(part).tab != Tab::Hidden
    {
        builder.pick(part, builds);
        builder.choose_variant(part, variant, builds);
        builder.turn = prop.turn;
        return true;
    }
    false
}

/// A wall, room or roof is drawn: pressed, dragged and let go of, its plan is laid where it
/// was drawn; clicked, at its own size where the view is. Held still on a plan, what goes
/// in next is built: a click lays, work builds (D144). Whether work begins now, and if so
/// whether by holding still on a plan; none if it neither begins nor may.
fn draw(
    cx: &mut Acting,
    me: Actor,
    click: bool,
    held: bool,
    builder: &mut Mut<Builder>,
) -> Option<bool> {
    let mut begin = click && !builder.must_release;
    let mut building_held = false;
    if builder.draws(&cx.builds) {
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
                        cx.building.tell(me.player, reason);
                        return None;
                    }
                    lay(&mut cx.building, me.player, builder, &cx.builds, planned);
                    cx.noise.write(Noise::at(Cue::Click, me.at));
                }
            } else {
                // Held still long enough on a plan, what goes in next is built.
                let was = drag.still_s;
                if !drag.moved {
                    drag.still_s += cx.time.delta_secs();
                }
                let on_plan = builder.aimed.is_some_and(|cube| {
                    cx.building
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
    begin.then_some(building_held)
}

/// Moving is neither building nor taking apart, and happens at once: an item picked up, or
/// put down where the view is. True if the move tool was used.
fn move_item(
    cx: &mut Acting,
    me: Actor,
    builder: &mut Mut<Builder>,
    stance: &mut Mut<Stance>,
) -> bool {
    match (&builder.picked(&cx.builds).kind, builder.target) {
        (Kind::Move, Target::Prop(entity)) if cx.building.stores.holding(entity) => {
            cx.building.tell(me.player, EMPTY_FIRST);
            true
        }
        (Kind::Move, Target::Prop(entity)) => {
            if let Ok(prop) = cx.props.get(entity) {
                info!("Picked up a {}.", prop.item.name);
                builder.turn = prop.turn;
                builder.carrying = Some(Carried {
                    item: prop.item.clone(),
                    at: prop.at,
                    lift: prop.lift,
                    turn: prop.turn,
                    indoors: prop.indoors,
                });
                cx.commands.entity(entity).despawn();
                stance.since_acting = 0.0;
            }
            true
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
                    &mut cx.commands,
                    &cx.models,
                    &cx.building.site,
                    &cx.building.planet,
                    &carried.item,
                    at,
                    lift,
                    builder.turn,
                    indoors,
                ) {
                    cx.commands.entity(placed).insert(PlacedBy(me.player));
                }
                info!(
                    "Placed a {} at {}, {lift} up.",
                    carried.item.name,
                    from_start(&cx.building.site, &cx.start, carried.at)
                );
                stance.since_acting = 0.0;
            }
            true
        }
        _ => false,
    }
}

/// Work on the base: what was worked out as it was aimed, or, held still on a plan with a
/// wall, room or roof in hand, what goes in next. True if there was work on the base.
fn begin_on_base(
    cx: &mut Acting,
    me: Actor,
    building_held: bool,
    builder: &mut Mut<Builder>,
) -> bool {
    let Actor {
        player,
        at: player_at,
        near,
    } = me;
    let building = &mut cx.building;
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
    let Some(planned) = planned.filter(|_| !matches!(builder.target, Target::Prop(_))) else {
        return false;
    };
    match (&planned.work.check, planned.doing) {
        (Err(why), _) => {
            building.tell(player, why.message());
            builder.must_release = true;
        }
        // A plan, or an opening cut into one, costs nothing and is laid at once.
        (Ok(_), Doing::Lay) => {
            lay(building, player, builder, &cx.builds, planned);
            cx.noise.write(Noise::at(Cue::Click, player_at));
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
                hold: Hold::new(job_seconds(doing, done, building)),
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
    true
}

/// Items: built from what is carried, and refused when that is not enough; taken apart,
/// everything comes back.
fn begin_on_items(cx: &mut Acting, me: Actor, builder: &mut Mut<Builder>) {
    let can_pay = builder.short.is_none();
    let (job, removing) = match (&builder.picked(&cx.builds).kind, builder.target) {
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
        (Kind::Remove, Target::Prop(entity)) if cx.building.stores.holding(entity) => {
            cx.building.tell(me.player, EMPTY_FIRST);
            return;
        }
        (Kind::Remove, Target::Prop(entity)) => (Job::TakeAway(entity), true),
        _ => return,
    };
    // How long it takes follows from what it costs.
    let seconds = match &job {
        Job::Place { item, .. } => catalogue::build_seconds(&item.cost, 1),
        Job::TakeAway(entity) => match cx.props.get(*entity) {
            Ok(prop) => catalogue::take_apart_seconds(&prop.item.cost, 1),
            Err(_) => return,
        },
        Job::Base { .. } => return,
    };
    // On the yard, building is all but instant: a moment, so a click is not a build.
    let seconds = if cx.building.free() { 0.1 } else { seconds };
    builder.drag = None;
    builder.work = Some(Work {
        job,
        hold: Hold::new(seconds),
        removing,
        began_at: me.at,
    });
}

/// The next step of work after one is in, if there is one and it is allowed: its moment
/// already as far through as the step before was waited on; else why not is told.
fn follow(
    building: &mut Building,
    player: Entity,
    near: Vec3,
    following: &Following,
) -> Option<Work> {
    let Following {
        doing,
        part,
        cube,
        began_at,
        waited_s,
    } = *following;
    let next = match doing {
        Doing::Build => building.next_work(Some(player), part, near),
        Doing::TakeApart => cube.and_then(|cube| building.last_work(Some(player), cube, near)),
        _ => None,
    }?;
    match &next.check {
        Ok(done) => {
            let mut hold = Hold::new(job_seconds(doing, done, building));
            hold.advance(waited_s);
            Some(Work {
                hold,
                job: Job::Base {
                    doing,
                    what: next.what.clone(),
                    ops: next.ops.clone(),
                    part,
                    cube,
                },
                removing: doing == Doing::TakeApart,
                began_at,
            })
        }
        Err(why) => {
            building.tell(player, why.message());
            None
        }
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
    // It rises in its own look: its body, textured where it is.
    let body = |item: &Item| {
        let body = models.of(item)?.body()?;
        Some((body.mesh.clone(), body.material.clone()))
    };
    match growing.and_then(|growing| Some((body(&growing.0)?, growing))) {
        Some(((mesh, material), (_, at, lift, turn, indoors, done))) => {
            let stood = props::standing(&site, &planet, at, lift, turn, indoors);
            // Quick at first, easing to full size.
            let eased = 1.0 - (1.0 - done) * (1.0 - done);
            let at = stood.with_scale(Vec3::splat(0.2 + 0.8 * eased));
            shapes.show(rising, mesh, material, at);
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
