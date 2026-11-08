//! Prediction by replay (D651): a client simulates its own state at once on its intent and
//! keeps, for each tick the server has not answered, the intent and the state after it. When
//! the server's answer for a tick differs, the state is set to the answer and the later
//! intents are run again (Unreal's saved moves, Overwatch's rollback, Gambetta's
//! reconciliation). The game says what "differs" and "run" mean; this keeps the history.

use std::collections::VecDeque;

/// The most ticks kept unanswered: two seconds. Past it the oldest are dropped, as lost.
const MOST_KEPT: usize = 128;

struct Entry<I, S> {
    tick: u32,
    intent: I,
    after: S,
}

/// What a client predicted and has not yet been answered: `I` its intent, `S` its state.
pub struct History<I, S> {
    entries: VecDeque<Entry<I, S>>,
    /// How many answers differed and were replayed from.
    pub corrections: u64,
    /// How many answers were for a tick the history does not hold, so nothing could be
    /// checked against them: after a jump, or when ticks were never recorded.
    pub missed: u64,
}

impl<I, S> Default for History<I, S> {
    fn default() -> Self {
        History {
            entries: VecDeque::new(),
            corrections: 0,
            missed: 0,
        }
    }
}

impl<I, S: Clone> History<I, S> {
    /// A tick was simulated: its intent and the state after it. Ticks come in order; one out
    /// of order (the clock jumped) voids what was kept.
    pub fn record(&mut self, tick: u32, intent: I, after: S) {
        if self
            .entries
            .back()
            .is_some_and(|last| last.tick + 1 != tick)
        {
            self.entries.clear();
        }
        if self.entries.len() == MOST_KEPT {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry {
            tick,
            intent,
            after,
        });
    }

    /// Forgets everything: the clock jumped, or the state was set from outside.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// How many ticks are unanswered.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The server's answer for the state after a tick. Where what was predicted for that tick
    /// is the `same`, nothing changes and None is returned. Where it differs, the predicted
    /// state is `correct`ed to the answer and each later intent is run again by `step` (the
    /// state before it and the intent, giving the state after): the state as it should be
    /// now is returned, for the game to take up. An answer for a tick not kept (older than
    /// the history, or from before a jump) is None.
    pub fn reconcile<A>(
        &mut self,
        tick: u32,
        answer: &A,
        same: impl Fn(&S, &A) -> bool,
        correct: impl Fn(&mut S, &A),
        mut step: impl FnMut(&S, &I) -> S,
    ) -> Option<S> {
        while self.entries.front().is_some_and(|entry| entry.tick < tick) {
            self.entries.pop_front();
        }
        if self.entries.front().is_none_or(|entry| entry.tick != tick) {
            self.missed += 1;
            return None;
        }
        let first = self.entries.front_mut()?;
        if same(&first.after, answer) {
            self.entries.pop_front();
            return None;
        }
        self.corrections += 1;
        correct(&mut first.after, answer);
        let mut state = first.after.clone();
        self.entries.pop_front();
        for entry in &mut self.entries {
            state = step(&state, &entry.intent);
            entry.after = state.clone();
        }
        Some(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A toy: the state is a place, an intent a step, and the server's world has a wall the
    /// client does not know of at 9.
    fn walled(place: &i32, step: &i32) -> i32 {
        (place + step).min(9)
    }

    #[test]
    fn an_answer_the_same_changes_nothing_and_one_that_differs_is_replayed_from() {
        let mut history = History::default();
        let mut place = 0;
        for tick in 1..=8 {
            place += 2;
            history.record(tick, 2, place);
        }
        let same = |mine: &i32, theirs: &i32| mine == theirs;
        let correct = |mine: &mut i32, theirs: &i32| *mine = *theirs;
        // Ticks 1 to 4 were as predicted.
        for tick in 1..=4 {
            assert_eq!(
                history.reconcile(tick, &(tick as i32 * 2), same, correct, walled),
                None
            );
        }
        assert_eq!((history.len(), history.corrections), (4, 0));
        // An answer for a tick already answered is not kept and changes nothing.
        assert_eq!(history.reconcile(2, &99, same, correct, walled), None);
        // At tick 5 the server stopped it at the wall: 9, and the three steps since replayed
        // against the wall leave it there, not at the 16 predicted.
        assert_eq!(history.reconcile(5, &9, same, correct, walled), Some(9));
        assert_eq!((history.len(), history.corrections), (3, 1));
        // The replay rewrote what was kept, so the next answers agree.
        assert_eq!(history.reconcile(6, &9, same, correct, walled), None);
    }

    #[test]
    fn a_tick_out_of_order_voids_the_history() {
        let mut history = History::default();
        history.record(5, 1, 1);
        history.record(6, 1, 2);
        history.record(20, 1, 3);
        assert_eq!(history.len(), 1);
        let same = |mine: &i32, theirs: &i32| mine == theirs;
        assert_eq!(
            history.reconcile(
                6,
                &50,
                same,
                |mine, theirs| *mine = *theirs,
                |place, step| place + step
            ),
            None
        );
    }
}
