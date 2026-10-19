//! The ground of a walled town's gardens (D381, `terra_town::town::Gardens`): beds of
//! vegetables in rows with earth between, greens kept short and lush, and ground fresh dug,
//! each plot within a balk of the meadow's grass, laid out as the house whose it is is
//! finished.

use std::collections::HashMap;
use terra_town::town::{BALK_M, GardenKind, Gardens};
use terra_world::cover::Growing;

/// How far apart a garden's beds are, middle to middle, and the share of that which is earth
/// between them.
const BED_PITCH_M: f32 = 1.1;
const BED_EARTH: f32 = 0.4;

/// The gardens laid out so far, by their cells on the town's grid.
#[derive(Clone, Default)]
pub struct GardenGround {
    grid: Gardens,
    cells: HashMap<[i32; 2], GardenKind>,
}

impl GardenGround {
    /// The gardens of a town whose homes `laid` says are finished.
    pub fn new(gardens: &Gardens, laid: impl Fn(usize) -> bool) -> Self {
        GardenGround {
            grid: Gardens {
                plots: Vec::new(),
                ..gardens.clone()
            },
            cells: gardens
                .plots
                .iter()
                .filter(|g| laid(g.home))
                .map(|g| (g.cell, g.kind))
                .collect(),
        }
    }

    /// What grows at a point in the town's metres east and south, if it is in a garden.
    pub fn grow(&self, p: bevy::math::Vec2, growing: &mut Growing) {
        if self.cells.is_empty() {
            return;
        }
        let (cell, within) = self.grid.cell_at(p);
        let Some(&kind) = self.cells.get(&cell) else {
            return;
        };
        let size = Gardens::plot_m();
        let m = within * size;
        // The balk round the plot is the meadow's.
        if m.x < BALK_M || m.y < BALK_M || m.x > size.x - BALK_M || m.y > size.y - BALK_M {
            return;
        }
        match kind {
            GardenKind::Beds => {
                let row = (m.x - BALK_M) / BED_PITCH_M;
                if row - libm::floorf(row) < BED_EARTH {
                    // Earth hoed between the rows.
                    growing.grass = 0.05;
                    growing.trodden = growing.trodden.max(0.55);
                } else {
                    // A row of greens: short, thick and lush.
                    growing.grass = 1.0;
                    growing.lush = 1.0;
                    growing.tall = 0.0;
                    growing.shrub = 0.0;
                }
            }
            GardenKind::Green => {
                growing.grass = growing.grass.max(0.95);
                growing.lush = 1.0;
                growing.tall = 0.0;
                growing.shrub = 0.0;
            }
            GardenKind::Dug => {
                growing.grass *= 0.15;
                growing.trodden = growing.trodden.max(0.5);
                growing.roots = growing.roots.max(0.2);
                growing.tall = 0.0;
                growing.shrub = 0.0;
            }
        }
    }
}
