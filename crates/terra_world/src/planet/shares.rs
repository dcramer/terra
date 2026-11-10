//! How much of the first realm's plants a world grows, as its realm says (D642).

use super::Planet;

impl Planet {
    /// The share of the first realm's plants that grow on this world.
    pub fn plants(&self) -> f32 {
        self.plants
    }

    /// The share of the first realm's trees that stand on this world: its plants' share
    /// unless its realm gives its trees their own.
    pub fn trees(&self) -> f32 {
        self.trees.unwrap_or(self.plants)
    }

    /// Sets how much of the first realm's plants grow here, and of its trees where they
    /// differ, as its realm says.
    pub fn set_plants(&mut self, share: f32, trees: Option<f32>) {
        self.plants = share.clamp(0.0, 1.0);
        self.trees = trees.map(|share| share.clamp(0.0, 1.0));
    }

    /// Its realm's own undergrowth, which its floor lays in place of the first realm's.
    pub fn undergrowth(&self) -> Option<&'static crate::realm::Undergrowth> {
        self.undergrowth
    }

    pub fn set_undergrowth(&mut self, own: Option<&'static crate::realm::Undergrowth>) {
        self.undergrowth = own;
    }
}
