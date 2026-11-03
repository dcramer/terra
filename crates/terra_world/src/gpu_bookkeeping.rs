//! The GPU's bookkeeping before it draws (D613): Bevy's bin unpacking and uniform
//! allocation shaders run once for every (view, phase, mesh class), their batch sets laid
//! end to end, not once a batch set (`patches/bevy_render`'s `batching::merged_unpacking`).
//! The tunable `render.merged_unpacking` sets it, so a probe measures the one against the
//! other in one load (D609).

use bevy::prelude::*;
use bevy::render::batching::merged_unpacking::MERGE_SCENE_UNPACKING;
use std::sync::atomic::Ordering;
use terra_gen::tune::Tunable;

/// Whether a (view, phase, mesh class)'s batch sets share one dispatch of each
/// bookkeeping shader (1) or each has its own, as Bevy 0.20 had them (0).
pub static MERGED: Tunable = Tunable::new(
    "render.merged_unpacking",
    1.0,
    0.0..=1.0,
    "1 unpacks bins and allocates uniforms in one dispatch a view, phase and mesh class; 0 one a batch set",
);

/// Hands the tunable to the render world, which reads it as it writes the bins.
fn follow() {
    MERGE_SCENE_UNPACKING.store(MERGED.get() >= 0.5, Ordering::Relaxed);
}

pub(crate) fn plugin(app: &mut App) {
    terra_gen::tune::register(&[&MERGED]);
    app.add_systems(Update, follow);
}
