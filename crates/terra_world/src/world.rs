//! The world around a player: the kinds of thing from the data files, the light and air,
//! the day, what grows, the game's tick and its levels of detail. It stands on the planet,
//! the terrain and the start (`planet`, `terrain`, `start`), and needs nothing from the
//! layers above it, so an experiment can put it on screen alone, to look at or walk
//! through (`walk`), and test it there.

use super::{
    adaptation, air, cine, climate, cover, data, daylight, detail, dither, drift, fire, flora,
    grading, grown, hable, lod, parting, quality, rises, scatter, scene, sky_occlusion, skylight,
    smoke, tick, upscale, wakes, weather, wind, woods,
};
use bevy::prelude::*;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        data::plugin(app);
        quality::plugin(app);
        upscale::plugin(app);
        scene::plugin(app);
        dither::plugin(app);
        grading::plugin(app);
        hable::plugin(app);
        scatter::plugin(app);
        air::plugin(app);
        skylight::plugin(app);
        sky_occlusion::plugin(app);
        daylight::plugin(app);
        climate::plugin(app);
        wind::plugin(app);
        weather::plugin(app);
        parting::plugin(app);
        grown::plugin(app);
        flora::plugin(app);
        woods::plugin(app);
        cover::plugin(app);
        tick::plugin(app);
        lod::plugin(app);
        detail::plugin(app);
        smoke::plugin(app);
        drift::plugin(app);
        rises::plugin(app);
        wakes::plugin(app);
        adaptation::plugin(app);
        fire::plugin(app);
        cine::plugin(app);
    }
}
