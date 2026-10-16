//! How many of the game's threads drawing may take beside its workers ([`Spare`]), and work
//! shared out among them.

use super::*;

/// Threads a process may start beside its workers to draw a section's parts side by side,
/// all of them together: a few, so a storey's step comes back in a part's time without the
/// main thread waiting for a core behind dozens of drawing threads (D308).
pub(super) const SPARE_THREADS: usize = 4;

pub(super) static SPARE_IN_USE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

impl Spare {
    pub fn take(wanted: usize) -> Spare {
        use std::sync::atomic::Ordering;
        let mut taken = 0;
        let _ = SPARE_IN_USE.fetch_update(Ordering::AcqRel, Ordering::Acquire, |in_use| {
            taken = wanted.min(SPARE_THREADS.saturating_sub(in_use));
            Some(in_use + taken)
        });
        Spare(taken)
    }

    /// How many threads were taken.
    pub fn count(&self) -> usize {
        self.0
    }
}

impl Drop for Spare {
    fn drop(&mut self) {
        SPARE_IN_USE.fetch_sub(self.0, std::sync::atomic::Ordering::AcqRel);
    }
}

/// Splits `items` into as many runs, of nearly the same size, as threads to work them: this
/// one and those spare.
pub fn shares<T: Copy>(items: &[T], threads: usize) -> Vec<Vec<T>> {
    let threads = threads.clamp(1, items.len().max(1));
    (0..threads)
        .map(|t| items.iter().copied().skip(t).step_by(threads).collect())
        .collect()
}
