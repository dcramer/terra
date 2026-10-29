//! Schedule marks (D576): the frame recorder's timing in a build without Bevy's `trace`,
//! whose spans cost about 15% of a frame compiled in. Each world's top-level schedules
//! (`MainScheduleOrder`, and the render thread's `RenderScheduleOrder`) are run with a mark
//! before the first and after each: a schedule of one system that reads the clock. The time
//! from one mark to the next is the schedule between them, and the first to the last is the
//! world's update, so a freeze names the schedule that held it in every build, for a few
//! microseconds a frame. A `hitch-log` build has Bevy's own spans and goes without.
//!
//! After Unreal's always-on frame timing (`stat unit`'s game and draw threads), which names
//! the thread and stage of a slow frame in a shipping build, where its stats are compiled out.

use super::{GAME_FRAME, Held, RENDER_FRAME, SCHEDULE, Shared, Span};
use bevy::{
    app::MainScheduleOrder,
    ecs::schedule::{InternedScheduleLabel, ScheduleLabel, SingleThreadedExecutor},
    prelude::*,
    render::{RenderApp, RenderScheduleOrder},
};
use std::sync::{Arc, Mutex};

/// Puts the marks in once every plugin has put its schedules in each world's order.
pub(super) struct Marks(pub(super) Arc<Shared>);

impl Plugin for Marks {
    fn build(&self, _: &mut App) {}

    fn cleanup(&self, app: &mut App) {
        let shared = &self.0;
        if let Some(mut order) = app.world_mut().get_resource_mut::<MainScheduleOrder>() {
            let marks = mark(shared, Held::Game, &mut order.labels);
            for schedule in marks {
                app.add_schedule(schedule);
            }
        }
        // The render world is still the main app's here: pipelined rendering takes it to its
        // thread in its own cleanup, after this plugin's (the log's plugin comes first).
        if let Some(render) = app.get_sub_app_mut(RenderApp)
            && let Some(mut order) = render.world_mut().get_resource_mut::<RenderScheduleOrder>()
        {
            let marks = mark(shared, Held::Render, &mut order.labels);
            for schedule in marks {
                render.add_schedule(schedule);
            }
        }
    }
}

/// A mark's schedule: which world's order, and which place in it.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct Mark(Held, usize);

/// One world's clock: when its update began and its last mark, and its spans not yet drained.
struct Clock {
    began: u64,
    last: u64,
    done: Arc<Mutex<Vec<Span>>>,
}

/// Puts a mark before the first of `labels` and after each, and returns their schedules.
fn mark(
    shared: &Arc<Shared>,
    world: Held,
    labels: &mut Vec<InternedScheduleLabel>,
) -> Vec<Schedule> {
    let clock = Arc::new(Mutex::new(Clock {
        began: 0,
        last: 0,
        done: shared.buffer(),
    }));
    let frame = match world {
        Held::Game => GAME_FRAME,
        Held::Render => RENDER_FRAME,
    };
    let names: Vec<u32> = (labels.iter())
        .map(|label| shared.name_in(format!("{label:?}{SCHEDULE}"), world))
        .collect();
    let mut marked = Vec::with_capacity(labels.len() * 2 + 1);
    let mut schedules = Vec::with_capacity(labels.len() + 1);
    for at in 0..=labels.len() {
        let label = Mark(world, at);
        marked.push(label.intern());
        marked.extend(labels.get(at).copied());
        let mut schedule = Schedule::new(label);
        schedule.set_executor(SingleThreadedExecutor::new());
        let (shared, clock) = (shared.clone(), clock.clone());
        let ended = at.checked_sub(1).map(|before| names[before]);
        let last = at == labels.len();
        schedule.add_systems(move || {
            let now = shared.now();
            let mut clock = clock
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(name) = ended else {
                (clock.began, clock.last) = (now, now);
                return;
            };
            let mut spans = vec![Span {
                name,
                start: clock.last,
                end: now,
                own: now - clock.last,
            }];
            if last {
                // The update's own time is what the marks took between the schedules.
                spans.push(Span {
                    name: frame,
                    start: clock.began,
                    end: now,
                    own: 0,
                });
            }
            clock.last = now;
            (clock.done.lock())
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .extend(spans);
        });
        schedules.push(schedule);
    }
    *labels = marked;
    schedules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_time_each_schedule_and_the_update_in_the_world_that_ran_them() {
        let shared = Arc::new(Shared::new());
        let mut app = App::new();
        app.add_plugins(Marks(shared.clone()))
            .add_systems(Update, || {
                std::thread::sleep(std::time::Duration::from_millis(20));
            });
        app.finish();
        app.cleanup();
        app.update();
        app.update();
        shared.drain();
        let state = shared.state();
        let names = shared.names();
        let named = |shown: &str| {
            let at = names.shown.iter().position(|name| name == shown);
            at.map(|at| at as u32)
        };
        let update = named("Update schedule").expect("Update is marked");
        assert_eq!(names.world[update as usize], Some(Held::Game));
        let runs = |name: u32| state.recent.iter().filter(move |span| span.name == name);
        assert_eq!(runs(GAME_FRAME).count(), 2, "an update a frame");
        assert_eq!(runs(update).count(), 2);
        for (frame, update) in runs(GAME_FRAME).zip(runs(update)) {
            assert!(update.end - update.start >= 20_000_000, "{update:?}");
            assert!(frame.start <= update.start && update.end <= frame.end);
        }
        let first = named("First schedule").expect("First is marked");
        assert!(runs(first).all(|span| span.end - span.start < 20_000_000));
    }
}
