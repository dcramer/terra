//! Dungeons by rule ([docs/dungeons.md](../../../docs/dungeons.md), D473, D474): a theme and a
//! seed make a dungeon in stages, each its own module and each checked alone.
//!
//! 1. `graph`: the mission graph, rooms by role and how they are linked, with a lock and its
//!    keys, a loop, a shortcut, a secret, and the mobs and loot in each room.
//! 2. `place` (with `route`): the graph laid on a grid of 4 m cells, 6 m levels (D527), with corridors
//!    and stairs.
//! 3. `dress`: every cell given a module of the theme's set by wave function collapse.
//! 4. `theme`: themes as data, `assets/data/dungeons/*.ron`.
//! 5. `stamp`: the dressed dungeon laid in building's cubes, with what is spawned in it (props,
//!    lights, mobs, chests, the goal, keys, door leaves), the start and a tour.
//!
//! `sheet` measures many seeds and draws their maps (`cargo run -p terra_dungeon --bin
//! dungeons`). Plain Rust, no Bevy (D28): the sandbox and the server lay what this makes.

pub mod door;
pub mod dress;
mod font;
pub mod graph;
pub mod map;
pub mod place;
mod route;
pub mod sheet;
pub mod stamp;
pub mod theme;

use terra_gen::Seed;

pub use dress::{Dressing, ModuleSet};
pub use graph::Graph;
pub use place::Layout;
pub use theme::Theme;

/// A whole dungeon.
#[derive(Clone, Debug)]
pub struct Dungeon {
    pub graph: Graph,
    pub layout: Layout,
    pub dressing: Dressing,
}

/// A theme's dungeon from a seed, or why placement gave it up.
pub fn generate(theme: &Theme, set: &ModuleSet, seed: Seed) -> Result<Dungeon, String> {
    let graph = graph::generate(theme, seed);
    let layout = place::place(&graph, theme, seed)?;
    let dressing = dress::dress(&layout, &graph, theme, set, seed);
    Ok(Dungeon {
        graph,
        layout,
        dressing,
    })
}
