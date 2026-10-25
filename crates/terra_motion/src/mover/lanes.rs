//! A tick's steps shared over threads (D53): every body steps against the others as the tick
//! began (Jacobi), so its steps may be shared out in any order to the same bits. The threads are
//! the caller's, through [`Executor`]: the game's are Bevy's compute pool, which runs its systems
//! too, so the bodies' tick adds no pool of its own beside it; tests and benches hand it rayon's.

use super::push::{self, Ahead};
use super::{Bodies, Body, Intent, PUSH_ROUNDS, RUN, World, steps};
use std::cmp::Reverse;
use std::ops::Range;
use std::sync::{Mutex, MutexGuard, PoisonError};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What runs a tick's lanes: `lane(k)` for every `k` below `lanes`, as many at once as it has
/// threads, back once every one has run. A parallel-for, as a job system's (Unreal's
/// `ParallelFor`, Bevy's `TaskPool::scope`, rayon's `scope`).
pub trait Executor: Sync {
    fn each(&self, lanes: usize, lane: &(dyn Fn(usize) + Sync));
}

/// One lane after another on the calling thread.
pub struct InTurn;

impl Executor for InTurn {
    fn each(&self, lanes: usize, lane: &(dyn Fn(usize) + Sync)) {
        (0..lanes).for_each(lane);
    }
}

/// What builds the world a lane's bodies move through, from what is the lane's own, and hands
/// it on.
pub type Lanes<'a, L> = dyn Fn(&mut L, &mut dyn FnMut(&World)) + Sync + 'a;

impl Bodies {
    /// [`Bodies::tick`] with the bodies' steps shared over a lane a thread of `on`, in runs of
    /// neighbouring IDs, each taken by the next lane free: `world` builds the world a lane's
    /// bodies move through from what is the lane's own (its ground's memo, say) and hands it
    /// on. The first lane also makes the pushes' lists when they are due, from where the bodies
    /// stood as the tick began, and takes the pushes after. The same bits as [`Bodies::tick`]
    /// whatever the executor and the number of lanes.
    pub fn tick_on<L: Send>(
        &mut self,
        on: &dyn Executor,
        lanes: &mut [L],
        world: &Lanes<'_, L>,
        intents: &[(u32, Intent)],
        dt: f32,
    ) {
        assert!(!lanes.is_empty(), "a tick needs a lane to run on");
        let before = &self.before(intents, dt);
        let ahead: Option<Ahead> = push::ahead(&self.bodies, &self.pairs);
        let lists = Mutex::new(&mut self.pairs);
        // Runs of neighbouring bodies, each lane given a block of them, the same from tick to
        // tick, so its memo of the ground holds what its bodies ask; a lane done with its own
        // takes from the back of the block with most left, so a crowd's thick middle, or a lane
        // started late, does not hold up the tick.
        let runs: Vec<Mutex<Option<&mut [Body]>>> = self
            .bodies
            .chunks_mut(RUN)
            .map(|run| Mutex::new(Some(run)))
            .collect();
        let used = lanes.len().min(runs.len()).max(1);
        let blocks: Vec<Mutex<Range<usize>>> = (0..used)
            .map(|k| Mutex::new(k * runs.len() / used..(k + 1) * runs.len() / used))
            .collect();
        let block = |j: usize| lock(&blocks[j]);
        let next = |k: usize| {
            let mut at = block(k).next();
            while at.is_none() {
                let most = (0..used).max_by_key(|&j| (block(j).len(), Reverse(j)))?;
                if block(most).is_empty() {
                    return None;
                }
                at = block(most).next_back();
            }
            at.and_then(|at| Some((at * RUN, lock(&runs[at]).take()?)))
        };
        let own: Vec<Mutex<&mut L>> = lanes[..used].iter_mut().map(Mutex::new).collect();
        on.each(used, &|k| {
            if k == 0
                && let Some(ahead) = &ahead
            {
                ahead.make(&mut lock(&lists));
            }
            let mut lane = lock(&own[k]);
            world(&mut lane, &mut |w| {
                while let Some((from, run)) = next(k) {
                    steps(run, from, before, w, dt);
                }
            });
        });
        drop(own);
        let (bodies, pairs) = (&mut self.bodies, &mut self.pairs);
        world(&mut lanes[0], &mut |w| {
            push::separate(bodies, pairs, w.ground, PUSH_ROUNDS)
        });
    }
}
