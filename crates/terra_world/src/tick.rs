//! The game's tick (D138): gameplay runs at a fixed rate, in `FixedUpdate`, apart from
//! what is shown, which runs every frame. A server ticks the same way, and a client
//! predicts its own player with the same ticks (D51).
//!
//! Each tick runs in stages, in order (`Sim`): players act on their intent, then the world
//! answers and goes on, then what happened is settled. What players mean to do is read
//! before the ticks of a frame (`intent::read`), and a press lasts until a tick has seen
//! it.
//!
//! Bodies move tick by tick, which at the screen's rate would stutter, so a body that moves
//! (`Smoothed`) is shown between where it stood after the last tick and where it stands
//! after this one, by how far the frame is into the next tick. Gameplay always sees where
//! it truly stands. This is what `bevy_transform_interpolation` does.
//!
//! So a body's `Transform` is last written each frame by its drawing (`show`), not by what
//! moved it. Engines with interpolated physics keep the two apart (Unity's Rigidbody
//! `position` and its interpolated `Transform`, Unreal's physics body and its smoothed
//! mesh), and so does `Smoothed`: it keeps the last change to where the body truly stands
//! made by anything but this module (`Moved`), the system in a tick or the teleport outside
//! them, which `sandbox.why` tells beside the drawing (D548).

use bevy::{
    ecs::change_detection::{MaybeLocation, Tick},
    prelude::*,
};

/// The stages of a tick, in order.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sim {
    /// Players act on their intent: moving, using tools, building, interacting.
    Act,
    /// The world answers what players did, and goes on: creatures, people, stations, falls.
    World,
    /// What happened is settled: blows taken, bodies kept apart.
    Resolve,
    /// The tick is over: presses it has seen are spent.
    End,
}

/// The most ticks a frame catches up on, as Unreal caps its substeps (`MaxSubsteps`, 6 by
/// default) and Unity its catch-up (`Maximum Allowed Timestep`): a frame that took longer
/// leaves the rest of its time untaken, so the game slows while frames are that slow rather
/// than running a burst of ticks that makes the next frame slower still. Bevy's own cap,
/// `Time<Virtual>`'s `max_delta`, is 250 ms, sixteen ticks at 64 a second.
pub const MOST_TICKS_A_FRAME: u32 = 6;

/// How many ticks have run, for what acts once in so many.
#[derive(Resource, Default)]
pub struct Ticks(pub u64);

/// Something that moves tick by tick and is shown smoothly between ticks: where it stood
/// before the tick and where it stands after it.
#[derive(Component, Default, Clone, Copy)]
pub struct Smoothed {
    before: Option<Transform>,
    after: Option<Transform>,
    /// What was last shown, so a move made outside the ticks (a teleport) is noticed and
    /// not undone.
    shown: Option<Transform>,
    /// The last move of where it truly stands, by anything but its drawing.
    moved: Option<Moved>,
    /// The change tick the start of this tick stood it where it truly is on, so a move the
    /// tick made since is told from that.
    restored: Tick,
}

/// A move of where a smoothed body truly stands: by which line of code (kept only by a
/// build with Bevy's `track_location`, as the sandbox's dev builds are), on which change
/// tick, and whether a tick made it or something outside the ticks did (a teleport).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moved {
    pub by: MaybeLocation,
    pub tick: Tick,
    pub in_tick: bool,
}

impl Smoothed {
    /// The last move of where the body truly stands, by anything but its drawing between
    /// ticks; None if nothing has moved it since it was spawned.
    pub fn moved(&self) -> Option<Moved> {
        self.moved
    }
}

impl Moved {
    fn of(transform: &impl DetectChanges, in_tick: bool) -> Self {
        Self {
            by: transform.changed_by(),
            tick: transform.last_changed(),
            in_tick,
        }
    }
}

pub fn plugin(app: &mut App) {
    app.configure_sets(
        FixedUpdate,
        (Sim::Act, Sim::World, Sim::Resolve, Sim::End)
            .chain()
            // The simulation is of the world: none while it is generated (D623).
            .run_if(crate::instance::in_world),
    )
    .init_resource::<Ticks>()
    .init_resource::<TickTiming>()
    .init_resource::<FrameTicks>()
    .add_systems(Startup, cap_catch_up)
    .add_systems(FixedFirst, (count, take_up))
    .add_systems(FixedLast, put_down)
    .add_systems(
        RunFixedMainLoop,
        (
            time_ticks.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            (show, behind).in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
        ),
    );
}

/// How long frames must have run past `MOST_TICKS_A_FRAME` on end before the log says the
/// game is behind: a hitch is not, a death spiral is (play at the lake, 2026-11-02: 4 fps for
/// minutes with nothing in the log naming the ticks).
const BEHIND_TELL_S: f32 = 1.0;
/// How often it says so again while the game stays behind.
const BEHIND_AGAIN_S: f32 = 10.0;

/// When this frame's ticks began, and how many had run by then, for `behind` on the far side
/// of them.
#[derive(Resource, Default)]
struct TickTiming {
    began: Option<(bevy::platform::time::Instant, u64)>,
}

/// The ticks of the frame just run: how many, and how long the fixed loop took to run them,
/// for a benchmark to hold a tick to its timestep (`bench_run`).
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct FrameTicks {
    pub ran: u64,
    pub seconds: f32,
}

impl FrameTicks {
    /// How long a tick of this frame took, on average; None in a frame that ran none.
    pub fn tick_ms(&self) -> Option<f32> {
        (self.ran > 0).then(|| self.seconds * 1000.0 / self.ran as f32)
    }
}

/// A run of frames on end that took more than the ticks a frame catches up on.
#[derive(Default, Debug, PartialEq)]
struct Behind {
    /// Real seconds of the run's frames.
    for_s: f32,
    frames: u32,
    /// Seconds the fixed loop took over the run, and its ticks.
    ticking_s: f32,
    ticks: u64,
    /// When in the run it last said so.
    told_at_s: Option<f32>,
}

/// What the log says of a run of frames past the tick cap.
#[derive(Debug, PartialEq)]
enum Said {
    Behind {
        for_s: f32,
        frame_ms: f32,
        tick_ms: f32,
    },
    CaughtUp {
        after_s: f32,
    },
}

impl Behind {
    /// A frame: whether it hit the cap, how long it took, and how long its ticks took and
    /// how many ran. Says when the run has lasted `BEHIND_TELL_S`, every `BEHIND_AGAIN_S`
    /// after, and once when a run it told of ends.
    fn frame(&mut self, capped: bool, frame_s: f32, ticking_s: f32, ticks: u64) -> Option<Said> {
        if !capped {
            let said = self.told_at_s.map(|_| Said::CaughtUp {
                after_s: self.for_s,
            });
            *self = Behind::default();
            return said;
        }
        self.for_s += frame_s;
        self.frames += 1;
        self.ticking_s += ticking_s;
        self.ticks += ticks;
        let due = match self.told_at_s {
            None => self.for_s >= BEHIND_TELL_S,
            Some(at) => self.for_s - at >= BEHIND_AGAIN_S,
        };
        due.then(|| {
            self.told_at_s = Some(self.for_s);
            Said::Behind {
                for_s: self.for_s,
                frame_ms: self.for_s * 1000.0 / self.frames as f32,
                tick_ms: self.ticking_s * 1000.0 / self.ticks.max(1) as f32,
            }
        })
    }
}

fn time_ticks(ticks: Res<Ticks>, mut timing: ResMut<TickTiming>) {
    timing.began = Some((bevy::platform::time::Instant::now(), ticks.0));
}

/// Says in the log when the game has fallen behind the clock: frames slower than the ticks a
/// frame catches up on (`MOST_TICKS_A_FRAME`) for a second on end, so the game runs slower
/// than time, naming how long a tick takes against its rate, so the next death spiral names
/// itself (D138).
fn behind(
    real: Res<Time<Real>>,
    clock: Res<Time<Virtual>>,
    tick: Res<Time<Fixed>>,
    ticks: Res<Ticks>,
    timing: Res<TickTiming>,
    mut run: Local<Behind>,
    mut frame_ticks: ResMut<FrameTicks>,
) {
    let capped =
        !clock.is_paused() && real.delta().mul_f64(clock.relative_speed_f64()) > clock.max_delta();
    let (ticking_s, ran) = timing.began.map_or((0.0, 0), |(at, before)| {
        (at.elapsed().as_secs_f32(), ticks.0.saturating_sub(before))
    });
    *frame_ticks = FrameTicks {
        ran,
        seconds: ticking_s,
    };
    match run.frame(capped, real.delta_secs(), ticking_s, ran) {
        Some(Said::Behind {
            for_s,
            frame_ms,
            tick_ms,
        }) => warn!(
            "The game is behind: frames have taken {frame_ms:.0} ms for {for_s:.0} s on end, \
             past the {MOST_TICKS_A_FRAME} ticks of {:.1} ms a frame catches up on, so the game \
             runs slower than the clock (D138). A tick takes {tick_ms:.1} ms against its {:.1}: \
             {}. The session report (`cargo xtask profile play`) names the systems.",
            tick.timestep().as_secs_f32() * 1000.0,
            tick.timestep().as_secs_f32() * 1000.0,
            if tick_ms > tick.timestep().as_secs_f32() * 1000.0 {
                "the ticks themselves are too slow, so each frame runs more of them (a death spiral)"
            } else {
                "the ticks keep up, so what is slow is the rest of the frame"
            }
        ),
        Some(Said::CaughtUp { after_s }) => {
            info!("The game has caught up with the clock after {after_s:.1} s behind.")
        }
        None => {}
    }
}

/// A frame's time taken up to `MOST_TICKS_A_FRAME` ticks of it, at the tick's rate.
fn cap_catch_up(mut clock: ResMut<Time<Virtual>>, tick: Res<Time<Fixed>>) {
    clock.set_max_delta(tick.timestep() * MOST_TICKS_A_FRAME);
}

fn count(mut ticks: ResMut<Ticks>) {
    ticks.0 += 1;
}

/// Before a tick: each body stands where it truly is, which is where it starts the tick.
/// One moved outside the ticks since it was shown stays where it was moved to.
fn take_up(mut bodies: Query<(&mut Transform, &mut Smoothed)>) {
    for (mut transform, mut smoothed) in &mut bodies {
        let moved = smoothed.shown.is_some_and(|shown| shown != *transform);
        if moved {
            smoothed.moved = Some(Moved::of(&transform, false));
        } else if let Some(after) = smoothed.after {
            *transform = after;
        }
        smoothed.restored = transform.last_changed();
        smoothed.before = Some(*transform);
        smoothed.after = None;
    }
}

/// After a tick: where each body stands now, and what in the tick moved it, if anything: a
/// system that wrote the place it already stood in (as one that walks a standing body
/// does, each tick) changed the component but did not move it.
fn put_down(mut bodies: Query<(Ref<Transform>, &mut Smoothed)>) {
    for (transform, mut smoothed) in &mut bodies {
        if transform.last_changed() != smoothed.restored && smoothed.before != Some(*transform) {
            smoothed.moved = Some(Moved::of(&transform, true));
        }
        smoothed.after = Some(*transform);
        smoothed.shown = None;
    }
}

/// Each frame, after the ticks: each body is shown as far between its last two places as
/// the frame is into the next tick.
fn show(time: Res<Time<Fixed>>, mut bodies: Query<(&mut Transform, &mut Smoothed)>) {
    let along = time.overstep_fraction();
    for (mut transform, mut smoothed) in &mut bodies {
        let (Some(before), Some(after)) = (smoothed.before, smoothed.after) else {
            continue;
        };
        // Moved outside the ticks since: shown where it was moved to.
        if smoothed.shown.is_some_and(|shown| shown != *transform) {
            smoothed.moved = Some(Moved::of(&transform, false));
            smoothed.before = Some(*transform);
            smoothed.after = Some(*transform);
            smoothed.shown = Some(*transform);
            continue;
        }
        let between = between(&before, &after, along);
        if *transform != between {
            *transform = between;
        }
        smoothed.shown = Some(between);
    }
}

/// So far from one place and turn to another. A body that did not move in the tick is shown
/// exactly where it stands, and one that did, by the step added to where it was
/// (`from + (to - from) * along`, glam's `lerp_monotonic`) and never by the two places
/// weighted (`from * (1 - along) + to * along`, its `lerp`): a place on a planet is tens of
/// kilometres from the origin, kept to 2 mm at 20 km, and the two weighted places round apart,
/// so a standing body was shown a step or two of that off where it stood, differently each
/// frame.
fn between(from: &Transform, to: &Transform, along: f32) -> Transform {
    if from == to {
        return *to;
    }
    Transform {
        translation: from.translation.lerp_monotonic(to.translation, along),
        rotation: from.rotation.slerp(to.rotation, along),
        scale: from.scale.lerp_monotonic(to.scale, along),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slow_frame_catches_up_on_so_many_ticks_and_no_more() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins((bevy::time::TimePlugin, plugin));
        let ticks = |app: &App| app.world().resource::<Ticks>().0;
        // A frame of a second, then one of a tenth, at Bevy's 64 ticks a second.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs(1)));
        app.update();
        app.update();
        let slow = ticks(&app);
        assert_eq!(slow, u64::from(MOST_TICKS_A_FRAME), "a second's frame");
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            50,
        )));
        app.update();
        assert_eq!(ticks(&app) - slow, 3, "a frame of 50 ms has its own ticks");
    }

    #[test]
    fn frames_past_the_cap_for_a_second_say_the_game_is_behind_then_that_it_caught_up() {
        let mut run = Behind::default();
        // Frames of 125 ms, each running six ticks of 20 ms: said once a second has gone,
        // at the eighth.
        let frame = |run: &mut Behind| run.frame(true, 0.125, 0.12, 6);
        let said: Vec<_> = (0..12).filter_map(|_| frame(&mut run)).collect();
        let [
            Said::Behind {
                for_s,
                frame_ms,
                tick_ms,
            },
        ] = said[..]
        else {
            panic!("said once: {said:?}");
        };
        assert_eq!((for_s, frame_ms), (1.0, 125.0));
        assert!((tick_ms - 20.0).abs() < 0.01, "{said:?}");
        // Said again ten seconds on, not before: 1.5 s in, a hundred frames more.
        assert_eq!((0..100).filter_map(|_| frame(&mut run)).count(), 1);
        // A frame in time ends the run, said once.
        let ended = run.frame(false, 0.01, 0.0, 1);
        assert_eq!(ended, Some(Said::CaughtUp { after_s: 14.0 }));
        assert_eq!(run.frame(false, 0.01, 0.0, 1), None);
        // A hitch of over half a second is not a game behind, and its end says nothing.
        assert!((0..5).all(|_| frame(&mut run).is_none()));
        assert_eq!(run.frame(false, 0.01, 0.0, 1), None);
    }

    /// A body standing still on a planet, 20 km from its centre, where a place is kept to
    /// 2 mm: shown anywhere between two ticks it is exactly where it stands, never a step of
    /// the place's precision off it (held-jitter j1: the player's figure and the camera on it
    /// hopped 2 to 4 mm up and down four frames in five while standing). And one that moves
    /// a step of that precision in a tick is shown at one end or the other, in order.
    #[test]
    fn a_body_standing_far_from_the_origin_is_shown_where_it_stands_at_any_frame() {
        let stands = Transform::from_xyz(-7_321.4, 19_876.543, 3_210.987)
            .with_rotation(Quat::from_rotation_y(0.7));
        let up = stands.translation.y;
        let next = Transform::from_xyz(-7_321.4, up.next_up(), 3_210.987);
        let mut last = up;
        for k in 0..=1000 {
            let along = k as f32 / 1000.0;
            let shown = between(&stands, &stands, along);
            assert_eq!(
                shown.translation, stands.translation,
                "a standing body shown {along} of the way between ticks moved"
            );
            assert!(shown.rotation.angle_between(stands.rotation) < 1e-3);
            let stepped = between(&stands, &next, along).translation.y;
            assert!(
                stepped >= last && stepped <= up.next_up(),
                "at {along}: {stepped} after {last}"
            );
            last = stepped;
        }
    }

    #[test]
    fn a_body_is_shown_as_far_between_ticks_as_the_frame_is_and_a_teleport_stays() {
        use std::f32::consts::FRAC_PI_2;
        use std::time::Duration;
        let mut world = World::new();
        world.insert_resource(Time::<Fixed>::from_seconds(0.1));
        let after =
            Transform::from_xyz(2.0, 0.0, 0.0).with_rotation(Quat::from_rotation_y(FRAC_PI_2));
        let body = world
            .spawn((
                after,
                Smoothed {
                    before: Some(Transform::IDENTITY),
                    after: Some(after),
                    ..default()
                },
            ))
            .id();
        let frame = |world: &mut World, seconds: f32| {
            world
                .resource_mut::<Time<Fixed>>()
                .accumulate_overstep(Duration::from_secs_f32(seconds));
            world.run_system_cached(show).unwrap();
            *world.get::<Transform>(body).unwrap()
        };
        // A quarter of the way into the next tick: a quarter of the way along, and turned.
        let shown = frame(&mut world, 0.025);
        assert!((shown.translation.x - 0.5).abs() < 1e-5, "{shown:?}");
        assert!(
            shown
                .rotation
                .angle_between(Quat::from_rotation_y(FRAC_PI_2 / 4.0))
                < 1e-4
        );
        // Three quarters in.
        let shown = frame(&mut world, 0.05);
        assert!((shown.translation.x - 1.5).abs() < 1e-5, "{shown:?}");
        // Moved outside the ticks, as by a warp: it is shown where it was moved to, and
        // stays there.
        world.get_mut::<Transform>(body).unwrap().translation.x = 10.0;
        assert_eq!(frame(&mut world, 0.0).translation.x, 10.0);
        assert_eq!(frame(&mut world, 0.01).translation.x, 10.0);
    }

    #[test]
    fn a_tick_starts_where_the_body_truly_is_and_a_teleport_stays() {
        let mut world = World::new();
        let body = world
            .spawn((Transform::from_xyz(1.0, 0.0, 0.0), Smoothed::default()))
            .id();
        // A tick moves it from 1 to 3; it is shown halfway between at 2.
        world.run_system_cached(take_up).unwrap();
        world.get_mut::<Transform>(body).unwrap().translation.x = 3.0;
        world.run_system_cached(put_down).unwrap();
        let smoothed = *world.get::<Smoothed>(body).unwrap();
        let shown = between(&smoothed.before.unwrap(), &smoothed.after.unwrap(), 0.5);
        *world.get_mut::<Transform>(body).unwrap() = shown;
        world.get_mut::<Smoothed>(body).unwrap().shown = Some(shown);
        // The next tick starts where it truly is, not where it was shown.
        world.run_system_cached(take_up).unwrap();
        assert_eq!(world.get::<Transform>(body).unwrap().translation.x, 3.0);
        // Moved outside the ticks, as by a warp: it stays where it was moved to.
        world.run_system_cached(put_down).unwrap();
        world.get_mut::<Smoothed>(body).unwrap().shown = Some(Transform::from_xyz(3.0, 0.0, 0.0));
        world.get_mut::<Transform>(body).unwrap().translation.x = 10.0;
        world.run_system_cached(take_up).unwrap();
        assert_eq!(world.get::<Transform>(body).unwrap().translation.x, 10.0);
    }

    /// What moved a body is kept apart from its drawing: a tick's move is kept, the ticks
    /// that follow (which stand it where it truly is again) and the drawing between them are
    /// not counted, and a teleport outside the ticks is kept as one.
    #[test]
    fn a_body_keeps_what_moved_it_apart_from_its_drawing() {
        let mut world = World::new();
        world.insert_resource(Time::<Fixed>::from_seconds(0.1));
        let body = world
            .spawn((Transform::default(), Smoothed::default()))
            .id();
        let moved = |world: &World| world.get::<Smoothed>(body).unwrap().moved();
        let changed = |world: &World| {
            world
                .entity(body)
                .get_change_ticks::<Transform>()
                .unwrap()
                .changed
        };
        // A tick, moving it so far, then the frame's drawing; the change tick of its move.
        let tick = |world: &mut World, step: Option<f32>| {
            world.run_system_cached(take_up).unwrap();
            if let Some(step) = step {
                world.get_mut::<Transform>(body).unwrap().translation.x += step;
            }
            let at = changed(world);
            world.run_system_cached(put_down).unwrap();
            world.run_system_cached(show).unwrap();
            at
        };
        tick(&mut world, None);
        assert_eq!(moved(&world), None, "nothing has moved it");
        let stepped = tick(&mut world, Some(1.0));
        let by_tick = moved(&world).expect("the tick moved it");
        assert!(by_tick.in_tick);
        assert_eq!(by_tick.tick, stepped);
        assert_ne!(changed(&world), stepped, "the drawing wrote it last");
        // Ticks that do not move it, each drawing it between, one of them writing the place
        // it stands in again: still the tick's move.
        for step in [None, Some(0.0), None] {
            tick(&mut world, step);
        }
        assert_eq!(moved(&world), Some(by_tick));
        // A teleport between frames, outside the ticks.
        world.get_mut::<Transform>(body).unwrap().translation.x = 10.0;
        let teleported = changed(&world);
        world.run_system_cached(show).unwrap();
        let outside = moved(&world).expect("the teleport moved it");
        assert!(!outside.in_tick);
        assert_eq!(outside.tick, teleported);
        tick(&mut world, None);
        assert_eq!(moved(&world), Some(outside));
    }
}
