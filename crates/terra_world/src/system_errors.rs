//! Every system that fails, in one run (D71, D627): by default a system whose parameters are
//! not there (a world's resource read while that world is being made, D469) panics the game,
//! so a crossing with five such systems takes five runs to find them. With
//! `TERRA_SYSTEM_ERRORS=list` each failing system is said once in the log, with what failed,
//! and the game goes on: one run lists them all (`grep 'SYSTEM ERROR' shots/remote.log`).
//! For finding them, never for play: a system that failed did not do its work.

use bevy::ecs::error::{BevyError, ErrorContext};
use bevy::prelude::*;
use std::collections::HashSet;
use std::sync::Mutex;

static SAID: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn list(error: BevyError, context: ErrorContext) {
    let name = context.name().to_string();
    let mut said = SAID.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if said.get_or_insert_with(HashSet::new).insert(name.clone()) {
        let what = error.to_string();
        let what = what.lines().next().unwrap_or_default();
        error!("SYSTEM ERROR ({}) {name}: {what}", context.kind());
    }
}

/// Lists failing systems rather than panicking on the first, when the environment asks.
pub fn plugin(app: &mut App) {
    if std::env::var("TERRA_SYSTEM_ERRORS").is_ok_and(|mode| mode == "list") {
        app.set_error_handler(list);
    }
}
