use crate::MaterialId;
use glam::{Quat, Vec3};

/// A change to the world.
///
/// Changes are sent between players and saved as these, never as the data they produce
/// (D21). The host decides their order; everyone applies them in that order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Operation {
    /// Remove a sphere of ground.
    Dig { centre: Vec3, radius: f32 },
    /// Remove a box of ground, `half` its size each way and turned by `rotation`: room
    /// cut for something built into the ground.
    DigBox {
        centre: Vec3,
        half: Vec3,
        rotation: Quat,
    },
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

impl Operation {
    /// Where it changes the world, for who is near enough to be told (D652). None for an
    /// operation on a thing named by its number, whose place is the thing's.
    pub fn place(&self) -> Option<Vec3> {
        match *self {
            Operation::Dig { centre, .. }
            | Operation::DigBox { centre, .. }
            | Operation::Fill { centre, .. }
            | Operation::ClearCover { centre, .. } => Some(centre),
            Operation::RemoveObject { .. } => None,
        }
    }
}

/// The widest an operation from the wire may reach, m: a stranger's numbers are held to what
/// a tool or a plan's cut could ask.
const MOST_REACH_M: f32 = 64.0;

fn write_vec3(vector: Vec3, to: &mut terra_net::wire::Writer) {
    use terra_net::wire::Wire;
    vector.to_array().write(to);
}

fn read_vec3(from: &mut terra_net::wire::Reader) -> Result<Vec3, terra_net::wire::Malformed> {
    use terra_net::wire::Wire;
    let vector = Vec3::from_array(<[f32; 3]>::read(from)?);
    if vector.is_finite() {
        Ok(vector)
    } else {
        Err(terra_net::wire::Malformed::Range)
    }
}

/// A size from the wire: a number, over nothing and within the most.
fn read_size(from: &mut terra_net::wire::Reader) -> Result<f32, terra_net::wire::Malformed> {
    use terra_net::wire::Wire;
    let size = f32::read(from)?;
    if size > 0.0 && size <= MOST_REACH_M {
        Ok(size)
    } else {
        Err(terra_net::wire::Malformed::Range)
    }
}

/// An operation on the wire (D21, docs/netcode.md): exact, so every game makes the same
/// ground of it, and refused where its numbers are none or past what could be asked.
impl terra_net::wire::Wire for Operation {
    fn write(&self, to: &mut terra_net::wire::Writer) {
        match *self {
            Operation::Dig { centre, radius } => {
                to.ranged(0, 4);
                write_vec3(centre, to);
                radius.write(to);
            }
            Operation::DigBox {
                centre,
                half,
                rotation,
            } => {
                to.ranged(1, 4);
                write_vec3(centre, to);
                write_vec3(half, to);
                rotation.to_array().write(to);
            }
            Operation::Fill {
                centre,
                radius,
                material,
            } => {
                to.ranged(2, 4);
                write_vec3(centre, to);
                radius.write(to);
                material.0.write(to);
            }
            Operation::RemoveObject { id } => {
                to.ranged(3, 4);
                id.0.write(to);
            }
            Operation::ClearCover { centre, radius } => {
                to.ranged(4, 4);
                write_vec3(centre, to);
                radius.write(to);
            }
        }
    }

    fn read(from: &mut terra_net::wire::Reader) -> Result<Self, terra_net::wire::Malformed> {
        use terra_net::wire::Malformed;
        Ok(match from.ranged(4)? {
            0 => Operation::Dig {
                centre: read_vec3(from)?,
                radius: read_size(from)?,
            },
            1 => {
                let (centre, half) = (read_vec3(from)?, read_vec3(from)?);
                let rotation = Quat::from_array(<[f32; 4]>::read(from)?);
                let sized = half.min_element() > 0.0 && half.max_element() <= MOST_REACH_M;
                if !sized || !rotation.is_finite() || !rotation.is_normalized() {
                    return Err(Malformed::Range);
                }
                Operation::DigBox {
                    centre,
                    half,
                    rotation,
                }
            }
            2 => Operation::Fill {
                centre: read_vec3(from)?,
                radius: read_size(from)?,
                material: MaterialId(u16::read(from)?),
            },
            3 => Operation::RemoveObject {
                id: ObjectId(u64::read(from)?),
            },
            _ => Operation::ClearCover {
                centre: read_vec3(from)?,
                radius: read_size(from)?,
            },
        })
    }
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
