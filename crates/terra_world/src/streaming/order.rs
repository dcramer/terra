//! The order work after the load is started and let into the world in (D608), apart from
//! Bevy so it is tested alone: priority classes, nearest first within each, a floor of one
//! task under way for every class with work so none starves, and a meter of the main thread's
//! milliseconds and the bytes to upload a frame.
//!
//! As shipped engines stream: Unreal's async loading has a time limit a frame on the game
//! thread (`s.AsyncLoadingTimeLimit`, with `s.PriorityAsyncLoadingExtraTime` for what is
//! high priority and one budget for both under `UseUnifiedTimeBudgetForStreaming`, larger
//! behind a loading screen) and its texture streaming a pool of bytes; Unity caps the main
//! thread's integration of background loads (`Application.backgroundLoadingPriority`: 2, 4,
//! 10 or 50 ms a frame). Frostbite and Decima (Guerrilla's "Streaming the World of Horizon
//! Zero Dawn", GDC 2017) order their streaming requests by priority and then by distance from
//! the camera. Godot's `WorkerThreadPool` keeps low-priority tasks to a share of its threads
//! but always at least one (`low_priority_thread_ratio`), the floor here.

use bevy::math::Vec3;

/// What is streamed after the load, the most wanted first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Class {
    /// The ground round the player.
    NearGround,
    /// The woods' grown trees round the player (D622), before they come within their last
    /// level's reach.
    NearWoods,
    /// The woods' far cards, kilometres off: the skyline's trees.
    FarWoods,
    /// The ground's far levels.
    FarGround,
    /// What grows near, only for looks.
    Flora,
    /// Anything else streamed after the load.
    Other,
}

impl Class {
    pub(crate) const ALL: [Class; 6] = [
        Class::NearGround,
        Class::NearWoods,
        Class::FarWoods,
        Class::FarGround,
        Class::Flora,
        Class::Other,
    ];
    pub(crate) const COUNT: usize = Self::ALL.len();

    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

/// Where a task's work lies, for how far it is from the camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Place {
    /// A box, by its nearest point.
    Bounds(Vec3, Vec3),
    /// A ring about a middle, by its inner edge.
    Ring { middle: Vec3, inner_m: f32 },
}

impl Place {
    pub(crate) fn away(self, viewer: Vec3) -> f32 {
        match self {
            Place::Bounds(low, high) => viewer.clamp(low, high).distance(viewer),
            Place::Ring { middle, inner_m } => (inner_m - middle.distance(viewer)).max(0.0),
        }
    }
}

/// Which of `waiting` (each its class and place, in the order asked) to start when `room`
/// more may run and `underway` of each class already do: first one for every class with work
/// and none under way, the most wanted class first, so a class behind a long one still moves
/// (Godot's floor); then the rest by class, nearest `viewer` first, ties in the order asked.
/// Gives indices into `waiting`, in the order to start them.
pub(crate) fn next(
    waiting: &[(Class, Place)],
    underway: [usize; Class::COUNT],
    viewer: Vec3,
    room: usize,
) -> Vec<usize> {
    let mut ranked: Vec<(Class, f32, usize)> = waiting
        .iter()
        .enumerate()
        .map(|(i, &(class, place))| (class, place.away(viewer), i))
        .collect();
    ranked.sort_by(|a, b| {
        (a.0.cmp(&b.0))
            .then(a.1.total_cmp(&b.1))
            .then(a.2.cmp(&b.2))
    });
    let mut taken = Vec::with_capacity(room.min(ranked.len()));
    let mut floored = [false; Class::COUNT];
    for &(class, _, i) in &ranked {
        if taken.len() == room {
            return taken;
        }
        if underway[class.index()] == 0 && !floored[class.index()] {
            floored[class.index()] = true;
            taken.push(i);
        }
    }
    for &(_, _, i) in &ranked {
        if taken.len() == room {
            break;
        }
        if !taken.contains(&i) {
            taken.push(i);
        }
    }
    taken
}

/// `count` rings of one area from `inner_m` to `outer_m`, so each holds about as much of what
/// is spread evenly over the band (the woods' far trees, a task a ring), the nearest first,
/// each ending just short of the next (`Woods::trees` keeps both ends), so nothing is in two.
pub(crate) fn rings(inner_m: f32, outer_m: f32, count: usize) -> Vec<(f32, f32)> {
    let count = count.max(1);
    let (low, high) = (inner_m * inner_m, outer_m * outer_m);
    let edge = |i: usize| (low + (high - low) * i as f32 / count as f32).sqrt();
    (0..count)
        .map(|i| {
            let outer = if i + 1 == count {
                outer_m
            } else {
                edge(i + 1).next_down()
            };
            (if i == 0 { inner_m } else { edge(i) }, outer)
        })
        .collect()
}

/// What may enter the world a frame (`assets/data/perf/budgets.ron`'s `streaming`).
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
pub(crate) struct Budget {
    /// The main thread's milliseconds a frame putting streamed results in the world.
    pub frame_ms: f32,
    /// The mebibytes of mesh a frame they bring for the GPU.
    pub upload_mib: f32,
    /// The same while the loading screen shows, which hides a long frame.
    pub loading_frame_ms: f32,
    pub loading_upload_mib: f32,
}

/// What has entered the world this frame, by class, against the budget.
#[derive(Clone, Debug, Default)]
pub(crate) struct Meter {
    pub ms: f32,
    pub bytes: usize,
    entered: [usize; Class::COUNT],
}

impl Meter {
    /// Whether one more of `class` may enter: the first of each class every frame does, so
    /// none starves behind another, and the near ground always does (Unreal's extra time for
    /// what is high priority); the rest while the frame's time and bytes are under `budget`.
    pub(crate) fn may_enter(&self, class: Class, budget: (f32, usize)) -> bool {
        class == Class::NearGround
            || self.entered[class.index()] == 0
            || (self.ms < budget.0 && self.bytes < budget.1)
    }

    /// Counts one of `class` in, which took `ms` of the main thread and brings `bytes`.
    pub(crate) fn entered(&mut self, class: Class, ms: f32, bytes: usize) {
        self.ms += ms;
        self.bytes += bytes;
        self.entered[class.index()] += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32) -> Place {
        Place::Bounds(Vec3::new(x, 0.0, 0.0), Vec3::new(x + 1.0, 1.0, 1.0))
    }

    #[test]
    fn work_starts_by_class_then_nearest_first() {
        let waiting = [
            (Class::FarGround, at(900.0)),
            (Class::FarGround, at(100.0)),
            (Class::Other, at(10.0)),
            (
                Class::FarWoods,
                Place::Ring {
                    middle: Vec3::ZERO,
                    inner_m: 1600.0,
                },
            ),
            (Class::NearGround, at(50.0)),
            (Class::FarGround, at(500.0)),
        ];
        // Every class under way already: the floor gives nothing, the order is plain.
        let order = next(&waiting, [1; Class::COUNT], Vec3::ZERO, 6);
        assert_eq!(order, [4, 3, 1, 5, 0, 2]);
        // From the far side, the far ground nearest it first.
        let order = next(&waiting, [1; Class::COUNT], Vec3::X * 1000.0, 6);
        assert_eq!(order[2..5], [0, 5, 1]);
    }

    #[test]
    fn a_class_behind_a_long_one_still_starts_one() {
        // A thousand far chunks and one far card ring: room for two, the woods get one.
        let mut waiting: Vec<(Class, Place)> = (0..1000)
            .map(|i| (Class::FarGround, at(i as f32)))
            .collect();
        waiting.push((Class::Other, at(0.0)));
        let mut underway = [0; Class::COUNT];
        underway[Class::FarGround.index()] = 1;
        let order = next(&waiting, underway, Vec3::ZERO, 2);
        assert_eq!(order, [1000, 0], "the other class starts beside the ground");
        // With no room for more nothing starts, and with one the more wanted class has it.
        assert!(next(&waiting, [0; Class::COUNT], Vec3::ZERO, 0).is_empty());
        assert_eq!(next(&waiting, [0; Class::COUNT], Vec3::ZERO, 1), [0]);
    }

    #[test]
    fn nothing_starves_while_more_wanted_work_keeps_coming() {
        // Each round, more far ground comes than the pool takes; the flora still finishes.
        let mut waiting: Vec<(Class, Place)> = (0..5).map(|_| (Class::Flora, at(0.0))).collect();
        for round in 0..20 {
            waiting.extend((0..8).map(|i| (Class::FarGround, at(i as f32))));
            // What starts in a round finishes before the next.
            let mut order = next(&waiting, [0; Class::COUNT], Vec3::ZERO, 4);
            order.sort_unstable_by(|a, b| b.cmp(a));
            for i in order {
                waiting.remove(i);
            }
            if !waiting.iter().any(|(class, _)| *class == Class::Flora) {
                assert!(round < 5, "the flora took {round} rounds");
                return;
            }
        }
        panic!("the flora starved behind the ground");
    }

    #[test]
    fn rings_part_the_band_into_even_areas_with_no_distance_in_two() {
        let (inner_m, outer_m) = (1600.0, 4500.0);
        let rings = rings(inner_m, outer_m, 8);
        assert_eq!(rings.len(), 8);
        assert_eq!(
            (rings[0].0, rings[7].1),
            (inner_m, outer_m),
            "the whole band"
        );
        let area = |(inner, outer): (f32, f32)| outer * outer - inner * inner;
        let whole = area((inner_m, outer_m)) / 8.0;
        for pair in rings.windows(2) {
            // The next starts at the first distance past this one's end.
            assert_eq!(pair[0].1.next_up(), pair[1].0, "{pair:?}");
        }
        for ring in &rings {
            assert!((area(*ring) / whole - 1.0).abs() < 1e-3, "{ring:?}");
        }
    }

    #[test]
    fn a_frame_lets_in_what_its_budget_holds_and_the_first_of_each_class() {
        let budget = (2.0, 8 << 20);
        let mut meter = Meter::default();
        let mut entered = 0;
        while meter.may_enter(Class::FarGround, budget) {
            meter.entered(Class::FarGround, 0.1, 3 << 20);
            entered += 1;
            assert!(entered < 100);
        }
        // 3 MiB each: three bring 9 MiB, past the 8, and no more come.
        assert_eq!(entered, 3);
        assert!(!meter.may_enter(Class::FarGround, budget));
        // The woods' first still comes this frame, and the near ground always does.
        assert!(meter.may_enter(Class::FarWoods, budget));
        meter.entered(Class::FarWoods, 5.0, 0);
        assert!(!meter.may_enter(Class::FarWoods, budget));
        assert!(meter.may_enter(Class::NearGround, budget));
        // The time holds it too.
        let mut meter = Meter::default();
        meter.entered(Class::Flora, 2.5, 0);
        assert!(!meter.may_enter(Class::Flora, budget));
    }
}
