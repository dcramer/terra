//! The woods' trees out to `COARSE_M`, laid out on the land's own thread once the land is made
//! (D623, `planet::entering::generating::WithTheLand`), so the frame the world is entered in
//! only plants them: laid out there they were 3.2 s of a 12 s frame at the lake, the loading
//! screen frozen meanwhile (crossing-freeze j1), as shipped games generate behind their
//! loading screen and only spawn on the game thread. A run with no loading screen lays them
//! out as it enters, as before.

use crate::planet::Planet;
use crate::planet::entering::Land;
use crate::planet::entering::generating::PutIn;
use crate::start::{Sited, Start};
use bevy::prelude::*;
use terra_land::woods::Tree;

/// The trees laid out round `middle`, for `plant` to take.
#[derive(Resource)]
pub(super) struct LaidOut {
    pub(super) middle: Vec3,
    pub(super) trees: Vec<Tree>,
}

impl LaidOut {
    /// The trees, if laid out round this middle; taken, so a world entered again lays its own.
    pub(super) fn take(laid_out: Option<ResMut<LaidOut>>, middle: Vec3) -> Option<Vec<Tree>> {
        let mut laid_out = laid_out?;
        (laid_out.middle == middle).then(|| std::mem::take(&mut laid_out.trees))
    }
}

/// Where the woods are planted round: the site's woods, else the start.
pub(super) fn middle(planet: &Planet, start: &Start, sited: Option<&Sited>) -> Vec3 {
    sited
        .and_then(|sited| sited.woods_middle(planet))
        .unwrap_or(start.translation)
}

/// Lays the trees out from the land made, on its thread.
pub(super) fn with_the_land((planet, start, sited, _): &Land) -> PutIn {
    let Some(round) = planet.round() else {
        return Box::new(|_| {});
    };
    let middle = middle(planet, start, sited.as_ref());
    let trees = super::stood(planet, round, middle, planet.seed().derive("woods"));
    Box::new(move |world: &mut World| world.insert_resource(LaidOut { middle, trees }))
}
