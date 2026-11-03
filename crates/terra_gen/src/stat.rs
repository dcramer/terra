//! Scoped timers by named group (D607): how much of each frame a group of systems took, in
//! every build, cheaply enough to stay on in a benchmark. A system of a group opens its
//! group's scope as its first line (`terra_gen::stat_scope!("creatures")`); the time until
//! the scope closes is added to the group, every thread's summed; once a frame the game
//! takes the totals (`take`) and the bench holds each group's median to its budget
//! (`assets/data/perf/budgets.ron`).
//!
//! After Unreal's stats system, where a cycle counter declared in a stats group
//! (`DECLARE_STATS_GROUP`, `DECLARE_CYCLE_STAT`) is timed by `SCOPE_CYCLE_COUNTER` and its
//! CSV profiler's `CSV_SCOPED_TIMING_STAT`, read once a frame against a budget a group. The
//! frame recorder (D576) times every system only in a build with Bevy's spans, at some 10 ms
//! a frame, so too slow for a bench; a scope here is two clock reads and two atomic adds.

use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Instant;

/// One place a group is timed: its time and scopes since the last `take`.
pub struct Stat {
    group: &'static str,
    ns: AtomicU64,
    scopes: AtomicU64,
    listed: AtomicBool,
}

/// Every stat that has been timed, so `take` finds it.
static LISTED: Mutex<Vec<&'static Stat>> = Mutex::new(Vec::new());

impl Stat {
    pub const fn new(group: &'static str) -> Self {
        Self {
            group,
            ns: AtomicU64::new(0),
            scopes: AtomicU64::new(0),
            listed: AtomicBool::new(false),
        }
    }

    /// Times its group until the scope is dropped.
    pub fn scope(&'static self) -> Scope {
        if !self.listed.load(Ordering::Relaxed) && !self.listed.swap(true, Ordering::AcqRel) {
            LISTED
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(self);
        }
        Scope {
            stat: self,
            start: Instant::now(),
        }
    }
}

/// A group timed until it is dropped.
pub struct Scope {
    stat: &'static Stat,
    start: Instant,
}

impl Drop for Scope {
    fn drop(&mut self) {
        let ns = u64::try_from(self.start.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.stat.ns.fetch_add(ns, Ordering::Relaxed);
        self.stat.scopes.fetch_add(1, Ordering::Relaxed);
    }
}

/// One group's time since the last take.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Taken {
    pub group: &'static str,
    pub ns: u64,
    pub scopes: u64,
}

/// Every group timed so far, its time and scopes since the last take (zero for a group that
/// did not run since), by name; and starts them all again from zero.
pub fn take() -> Vec<Taken> {
    let listed = LISTED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut taken: Vec<Taken> = Vec::new();
    for stat in listed.iter() {
        let (ns, scopes) = (
            stat.ns.swap(0, Ordering::Relaxed),
            stat.scopes.swap(0, Ordering::Relaxed),
        );
        match taken.iter_mut().find(|t| t.group == stat.group) {
            Some(group) => {
                group.ns += ns;
                group.scopes += scopes;
            }
            None => taken.push(Taken {
                group: stat.group,
                ns,
                scopes,
            }),
        }
    }
    taken.sort_by_key(|t| t.group);
    taken
}

/// Times the rest of the enclosing block as the named group's (`stat`): the first line of a
/// system of the group.
#[macro_export]
macro_rules! stat_scope {
    ($group:literal) => {
        static STAT: $crate::stat::Stat = $crate::stat::Stat::new($group);
        let _stat = STAT.scope();
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn group_of(taken: &[Taken], group: &str) -> Option<Taken> {
        taken.iter().copied().find(|t| t.group == group)
    }

    fn sleeps_in_test_group(ms: u64) {
        crate::stat_scope!("stat-test");
        std::thread::sleep(Duration::from_millis(ms));
    }

    #[test]
    fn a_group_sums_its_scopes_on_every_thread_and_starts_again_when_taken() {
        let threads: Vec<_> = (0..3)
            .map(|_| std::thread::spawn(|| sleeps_in_test_group(20)))
            .collect();
        threads.into_iter().for_each(|t| t.join().unwrap());
        sleeps_in_test_group(20);
        let taken = group_of(&take(), "stat-test").unwrap();
        assert_eq!(taken.scopes, 4);
        // Four sleeps of 20 ms each, on three threads and this one: 80 ms or more summed.
        assert!(taken.ns >= 80_000_000, "{taken:?}");
        let again = group_of(&take(), "stat-test").unwrap();
        assert_eq!((again.ns, again.scopes), (0, 0));
    }

    #[test]
    fn a_scope_costs_well_under_a_microsecond() {
        static COST: Stat = Stat::new("stat-cost");
        let n = 200_000u32;
        let start = Instant::now();
        for _ in 0..n {
            let _scope = COST.scope();
        }
        let each_ns = start.elapsed().as_nanos() as f64 / f64::from(n);
        println!("a scope costs {each_ns:.0} ns");
        // Two clock reads and two atomic adds: tens of ns; a debug build's are slower.
        assert!(each_ns < 1000.0, "a scope took {each_ns:.0} ns");
    }
}
