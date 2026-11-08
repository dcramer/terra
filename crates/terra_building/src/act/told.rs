//! An act another game says was done to the base (`terra_game::play::base_net`).

use super::Building;
use std::time::Instant;
use terra_build::ops::{Context, Done, Op, Refusal};

impl Building<'_, '_> {
    /// Does operations another game says were done to the base (the host's word, or a
    /// guest's act on the host: `play::base_net`): as one act, checked against the base as
    /// it is but against no claim and at no cost, as a saved base's operations are when it
    /// is loaded: who may build where, and what it took, was settled where the act was made.
    pub fn act_as_told(&mut self, what: &str, ops: &[Op]) -> Result<Done, Refusal> {
        self.settle_all();
        let began = Instant::now();
        let done = self.site.act(ops, &Context::free(&self.rules.rules));
        match &done {
            Ok(done) => self.done(None, what, done, began, true),
            Err(why) => self.refused(None, what, why),
        }
        done
    }
}
