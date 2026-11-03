//! Work shared out over every core: a map's cells, or a test's sweep over every case of
//! something (D406). The result is in the items' order whatever the threads did, so what is
//! worked out stays repeatable.

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

thread_local! {
    /// Whether [`map`] on this thread works its items here alone ([`keep_to_this_thread`]).
    static KEPT_HERE: Cell<bool> = const { Cell::new(false) };
}

/// From now on, [`map`] on this thread works every item on this thread alone, never fanning
/// out over every core: for a background pool's threads (the far view streamed in while the
/// game plays, D603), whose work must leave the game's main and render threads their cores.
/// A pool of a few threads stays a few, as Unreal's background task threads do; on the lake
/// the woods' far cards, made on the streaming pool, spread over all 16 cores and held 286 of
/// lake-play's frames over twice the budget (perf-regress j1).
pub fn keep_to_this_thread() {
    KEPT_HERE.with(|kept| kept.set(true));
}

/// `f` over every item, on every core, in order. The items are taken in runs, a thread taking
/// the next run not yet taken when it is done with its own, so a few slow items (a castle among
/// cabins) hold up only the thread working them; a run is a single item when there are few. A
/// panic in `f` stops the others taking more and is the caller's, with its own message.
pub fn map<T: Sync, U: Send>(items: &[T], f: impl Fn(&T) -> U + Sync) -> Vec<U> {
    let cores = if KEPT_HERE.with(Cell::get) {
        1
    } else {
        std::thread::available_parallelism().map_or(4, |n| n.get())
    };
    let threads = cores.min(items.len()).max(1);
    // Four runs a thread, so the last runs left over are short.
    let run = items.len().div_ceil(threads * 4).max(1);
    let next = AtomicUsize::new(0);
    let work = || {
        let mut done: Vec<(usize, Vec<U>)> = Vec::new();
        loop {
            let start = next.fetch_add(run, Ordering::Relaxed);
            let Some(taken) = items.get(start..items.len().min(start + run)) else {
                return Ok(done);
            };
            let worked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                taken.iter().map(&f).collect::<Vec<U>>()
            }));
            match worked {
                Ok(out) => done.push((start, out)),
                Err(panic) => {
                    next.store(items.len(), Ordering::Relaxed);
                    return Err(panic);
                }
            }
        }
    };
    let mut done: Vec<(usize, Vec<U>)> = Vec::new();
    let mut failed = None;
    std::thread::scope(|scope| {
        let workers: Vec<_> = (1..threads).map(|_| scope.spawn(work)).collect();
        let mut results = vec![work()];
        for worker in workers {
            // `work` catches its own panics, so a thread ends with what it did.
            results.push(worker.join().unwrap_or_else(Err));
        }
        for result in results {
            match result {
                Ok(runs) => done.extend(runs),
                Err(panic) => {
                    failed.get_or_insert(panic);
                }
            }
        }
    });
    if let Some(panic) = failed {
        std::panic::resume_unwind(panic);
    }
    done.sort_unstable_by_key(|(start, _)| *start);
    done.into_iter().flat_map(|(_, out)| out).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_is_worked_once_in_order() {
        for len in [0, 1, 3, 100, 10_007] {
            let items: Vec<usize> = (0..len).collect();
            assert_eq!(
                map(&items, |&i| i * 2),
                (0..len).map(|i| i * 2).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn a_thread_kept_to_itself_works_every_item_alone_and_others_still_fan_out() {
        let items: Vec<usize> = (0..64).collect();
        // How many threads worked the items, each long enough that idle threads take some.
        let threads = |items: &[usize]| {
            let ids = map(items, |_| {
                std::thread::sleep(std::time::Duration::from_millis(1));
                std::thread::current().id()
            });
            let mut ids: Vec<_> = ids.into_iter().map(|id| format!("{id:?}")).collect();
            ids.sort();
            ids.dedup();
            ids.len()
        };
        let kept = std::thread::spawn({
            let items = items.clone();
            move || {
                keep_to_this_thread();
                let here = format!("{:?}", std::thread::current().id());
                let ids = map(&items, |_| format!("{:?}", std::thread::current().id()));
                (
                    ids.iter().all(|id| *id == here),
                    threads(&items),
                    map(&items, |&i| i * 2),
                )
            }
        })
        .join()
        .expect("the kept thread ends");
        assert!(kept.0, "an item went to another thread");
        assert_eq!(kept.1, 1);
        assert_eq!(kept.2, items.iter().map(|i| i * 2).collect::<Vec<_>>());
        // Another thread is not kept: on a machine of several cores its map fans out.
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        assert_eq!(threads(&items) > 1, cores > 1);
    }

    #[test]
    fn a_panic_in_one_item_is_the_callers_with_its_message() {
        let items: Vec<usize> = (0..64).collect();
        let caught = std::panic::catch_unwind(|| {
            map(&items, |&i| {
                assert!(i != 40, "item {i} is wrong");
                i
            })
        });
        let panic = caught.expect_err("the panic reaches the caller");
        let said = panic.downcast_ref::<String>().cloned().unwrap_or_default();
        assert_eq!(said, "item 40 is wrong");
    }
}
