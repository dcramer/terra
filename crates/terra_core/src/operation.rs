use crate::MaterialId;
use glam::Vec3;

/// A change to the world.
///
/// Changes are sent between players and saved as these, never as the data they produce
/// (D21). The host decides their order; everyone applies them in that order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Operation {
    /// Remove a sphere of ground.
    Dig { centre: Vec3, radius: f32 },
    /// Add a sphere of ground.
    Fill {
        centre: Vec3,
        radius: f32,
        material: MaterialId,
    },
    /// Remove one placed object, such as a tree or a boulder.
    RemoveObject { id: ObjectId },
    /// Clear ground cover, such as grass, within a circle on the surface.
    ClearCover { centre: Vec3, radius: f32 },
}

/// One generated object. Objects are placed by rule from the seed, so an id is enough to
/// say which one, and nothing about an object is stored until it changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectId(pub u64);

/// A kind of object or cover. Which kinds exist is design (D43).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KindId(pub u16);

/// Something an operation took out of the world. The engine reports what was removed;
/// turning that into resources a player holds is the game's job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Harvested {
    /// Ground, measured in cubic metres.
    Material(MaterialId),
    /// Objects, counted.
    Object(KindId),
    /// Cover, measured in square metres.
    Cover(KindId),
}

/// What applying an operation produced (D45). Empty for operations that take nothing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Yield {
    amounts: Vec<(Harvested, f32)>,
}

impl Yield {
    pub fn add(&mut self, what: Harvested, amount: f32) {
        if amount <= 0.0 {
            return;
        }
        match self.amounts.iter_mut().find(|(kind, _)| *kind == what) {
            Some((_, total)) => *total += amount,
            None => self.amounts.push((what, amount)),
        }
    }

    pub fn amount(&self, what: Harvested) -> f32 {
        self.amounts
            .iter()
            .find(|(kind, _)| *kind == what)
            .map_or(0.0, |(_, amount)| *amount)
    }

    pub fn is_empty(&self) -> bool {
        self.amounts.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Harvested, f32)> + '_ {
        self.amounts.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yields_add_up_by_kind() {
        let rock = Harvested::Material(MaterialId(1));
        let ore = Harvested::Material(MaterialId(2));
        let mut harvest = Yield::default();
        assert!(harvest.is_empty());
        harvest.add(rock, 2.0);
        harvest.add(ore, 0.5);
        harvest.add(rock, 1.0);
        harvest.add(ore, 0.0);
        assert_eq!(harvest.amount(rock), 3.0);
        assert_eq!(harvest.amount(ore), 0.5);
        assert_eq!(harvest.amount(Harvested::Object(KindId(0))), 0.0);
        assert_eq!(harvest.iter().count(), 2);
    }
}
