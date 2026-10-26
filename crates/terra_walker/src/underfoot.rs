//! The ground under a foot, as the walk's own world has it (`terra_motion::placement::probe`):
//! the planet's field, dug or not, and the boxes the last tick gathered round the players
//! (their base's cubes, props), the same the mover stands a body on. Presentation only: what a
//! foot finds goes to the figure's legs (`feet`) and the walk record, never back to the mover.

use std::cell::RefCell;

use bevy::{ecs::system::SystemParam, prelude::*};
use terra_motion::mover::{Obstacles, Sdf};
use terra_motion::placement::{self, Under};
use terra_world::planet::{HeightMemo, Planet};

use crate::movement::Movers;

/// How many of the land's lattice points the probes' memo keeps before it forgets them.
const MEMO_MOST: usize = 1024;

/// The ground under a foot: ask it with `under`.
#[derive(SystemParam)]
pub struct Underfoot<'w, 's> {
    planet: Option<Res<'w, Planet>>,
    movers: Option<Res<'w, Movers>>,
    memo: Local<'s, HeightMemo>,
}

impl Underfoot<'_, '_> {
    /// The ground under `at`, over `root`'s level along `up` (`placement::probe`); none with no
    /// planet, or none found.
    pub fn under(&mut self, at: Vec3, root: Vec3, up: Vec3) -> Option<Under> {
        let planet = self.planet.as_deref()?;
        self.memo.keep_within(MEMO_MOST);
        let memo = RefCell::new(&mut *self.memo);
        let field = Sdf {
            distance: |p| planet.distance_in(p, &mut memo.borrow_mut()),
            steepest: planet.steepest(),
        };
        let blocks = Obstacles {
            obstacles: self.movers.as_deref().map_or(&[][..], Movers::blocks),
        };
        placement::probe(&(&field, &blocks), at.as_dvec3(), root.as_dvec3(), up)
    }
}
