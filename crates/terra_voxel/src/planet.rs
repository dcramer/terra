//! A generated planet with the edits made to it: what the game digs, fills and walks on.
//!
//! Edits are kept as the operations themselves, in the order they were applied, and laid
//! over the generated field whenever a point is asked about (D21). Nothing about the
//! ground is stored until it changes, and two planets that applied the same operations
//! in the same order agree everywhere. Applying a dig reports what it took out, by
//! material (D45).

use crate::field::{Field, Sample, material};
use glam::{Quat, Vec3};
use terra_core::{Ground, Harvested, MaterialId, Operation, Yield};

/// One change to the ground's shape.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Edit {
    Dig {
        centre: Vec3,
        radius: f32,
    },
    DigBox {
        centre: Vec3,
        half: Vec3,
        rotation: Quat,
    },
    Fill {
        centre: Vec3,
        radius: f32,
        material: MaterialId,
    },
}

/// How far a point is from a box, negative inside it.
fn box_distance(point: Vec3, centre: Vec3, half: Vec3, rotation: Quat) -> f32 {
    let beyond = (rotation.inverse() * (point - centre)).abs() - half;
    beyond.max(Vec3::ZERO).length() + beyond.max_element().min(0.0)
}

impl Edit {
    /// The middle of what it changes, and how far from there it reaches.
    fn centre_and_radius(&self) -> (Vec3, f32) {
        match *self {
            Edit::Dig { centre, radius } | Edit::Fill { centre, radius, .. } => (centre, radius),
            Edit::DigBox { centre, half, .. } => (centre, half.length()),
        }
    }

    /// The distance to the ground at a point after this edit, given what it was before.
    fn distance(&self, point: Vec3, before: f32) -> f32 {
        match *self {
            Edit::Dig { centre, radius } => before.max(radius - point.distance(centre)),
            Edit::DigBox {
                centre,
                half,
                rotation,
            } => before.max(-box_distance(point, centre, half, rotation)),
            Edit::Fill { centre, radius, .. } => before.min(point.distance(centre) - radius),
        }
    }

    /// Whether a point is inside what a dig takes away.
    fn takes(&self, point: Vec3) -> bool {
        match *self {
            Edit::Dig { centre, radius } => point.distance(centre) <= radius,
            Edit::DigBox {
                centre,
                half,
                rotation,
            } => box_distance(point, centre, half, rotation) <= 0.0,
            Edit::Fill { .. } => false,
        }
    }

    /// The ground at a point after this edit, given what it was before.
    fn apply(&self, point: Vec3, before: Sample) -> Sample {
        match *self {
            // Take the shape away: the ground is wherever it was and the shape is not.
            Edit::Dig { .. } | Edit::DigBox { .. } => Sample {
                distance: self.distance(point, before.distance),
                material: before.material,
            },
            // Add the sphere: the ground is wherever it was or the sphere is.
            Edit::Fill {
                centre,
                radius,
                material,
            } => {
                let sphere = point.distance(centre) - radius;
                if sphere < before.distance {
                    Sample {
                        distance: sphere,
                        material,
                    }
                } else {
                    before
                }
            }
        }
    }
}

/// How finely a dig is measured to say what it took out: the side of a measuring cell.
const HARVEST_STEP_M: f32 = 0.2;
/// Edits reach a little past their sphere, where the surface they leave is still bent.
const EDIT_MARGIN_M: f32 = 2.0;

/// A planet's ground: its generated field and the edits made to it.
#[derive(Clone)]
pub struct VoxelPlanet<F: Field> {
    field: F,
    edits: Vec<Edit>,
}

impl<F: Field> VoxelPlanet<F> {
    pub fn new(field: F) -> Self {
        Self {
            field,
            edits: Vec::new(),
        }
    }

    pub fn field(&self) -> &F {
        &self.field
    }

    /// The operations that shaped the ground, in the order they were applied: what a save
    /// keeps, and what replaying makes the same ground again (D21).
    pub fn operations(&self) -> Vec<Operation> {
        self.edits
            .iter()
            .map(|edit| match *edit {
                Edit::Dig { centre, radius } => Operation::Dig { centre, radius },
                Edit::DigBox {
                    centre,
                    half,
                    rotation,
                } => Operation::DigBox {
                    centre,
                    half,
                    rotation,
                },
                Edit::Fill {
                    centre,
                    radius,
                    material,
                } => Operation::Fill {
                    centre,
                    radius,
                    material,
                },
            })
            .collect()
    }

    /// How many edits have been made.
    pub fn edit_count(&self) -> usize {
        self.edits.len()
    }

    /// Where the edits made since the first so many were, as spheres, with the margin
    /// past them that their bent surface reaches: what to draw again after them.
    pub fn edits_since(&self, first: usize) -> impl Iterator<Item = (Vec3, f32)> + '_ {
        self.edits.iter().skip(first).map(|edit| {
            let (centre, radius) = edit.centre_and_radius();
            (centre, radius + EDIT_MARGIN_M)
        })
    }

    /// The ground at a point, with every edit laid over the generated field in order.
    pub fn sample(&self, point: Vec3) -> Sample {
        self.sample_in(point, &mut F::Memo::default())
    }

    /// The edits that reach a point, in the order they were made.
    fn edits_at(&self, point: Vec3) -> impl Iterator<Item = &Edit> {
        self.edits.iter().filter(move |edit| {
            let (centre, radius) = edit.centre_and_radius();
            let reach = radius + EDIT_MARGIN_M;
            point.distance_squared(centre) <= reach * reach
        })
    }

    /// Applies an operation to the ground and returns what it took out: for a dig, the
    /// volume of each material removed, in cubic metres. Operations that do not shape the
    /// ground leave it as it was and take nothing.
    pub fn apply(&mut self, operation: &Operation) -> Yield {
        let edit = match *operation {
            Operation::Dig { centre, radius } => Edit::Dig { centre, radius },
            Operation::DigBox {
                centre,
                half,
                rotation,
            } => Edit::DigBox {
                centre,
                half,
                rotation,
            },
            Operation::Fill {
                centre,
                radius,
                material,
            } => Edit::Fill {
                centre,
                radius,
                material,
            },
            Operation::RemoveObject { .. } | Operation::ClearCover { .. } => {
                return Yield::default();
            }
        };
        let taken = match edit {
            Edit::Fill { .. } => Yield::default(),
            _ => self.measure_dig(&edit),
        };
        self.edits.push(edit);
        taken
    }

    /// What a dig would take out: every measuring cell inside it that is solid now, by
    /// its material.
    fn measure_dig(&self, dig: &Edit) -> Yield {
        let (centre, radius) = dig.centre_and_radius();
        let mut taken = Yield::default();
        let mut memo = F::Memo::default();
        let cells = (radius / HARVEST_STEP_M).ceil() as i32;
        let cell_volume = HARVEST_STEP_M * HARVEST_STEP_M * HARVEST_STEP_M;
        for x in -cells..cells {
            for y in -cells..cells {
                for z in -cells..cells {
                    let offset = (Vec3::new(x as f32, y as f32, z as f32) + 0.5) * HARVEST_STEP_M;
                    if !dig.takes(centre + offset) {
                        continue;
                    }
                    let before = self.sample_in(centre + offset, &mut memo);
                    if before.solid() {
                        taken.add(Harvested::Material(before.material), cell_volume);
                    }
                }
            }
        }
        taken
    }
}

impl<F: Field> Field for VoxelPlanet<F> {
    type Memo = F::Memo;

    fn memo_for(&self, spacing_m: f32) -> F::Memo {
        self.field.memo_for(spacing_m)
    }

    fn distance_in(&self, point: Vec3, memo: &mut F::Memo) -> f32 {
        let mut distance = self.field.distance_in(point, memo);
        for edit in self.edits_at(point) {
            distance = edit.distance(point, distance);
        }
        distance
    }

    fn sample_in(&self, point: Vec3, memo: &mut F::Memo) -> Sample {
        let mut sample = self.field.sample_in(point, memo);
        for edit in self.edits_at(point) {
            sample = edit.apply(point, sample);
        }
        sample
    }
}

impl<F: Field> Ground for VoxelPlanet<F> {
    fn distance(&self, point: Vec3) -> f32 {
        self.distance_in(point, &mut F::Memo::default())
    }

    fn material(&self, point: Vec3) -> MaterialId {
        let sample = self.sample(point);
        if sample.solid() {
            sample.material
        } else {
            material::AIR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GeneratedPlanet;
    use terra_core::PlanetSpec;
    use terra_gen::Seed;

    fn planet() -> VoxelPlanet<GeneratedPlanet> {
        VoxelPlanet::new(GeneratedPlanet::new(
            PlanetSpec {
                seed: Seed(7),
                radius_m: 2000.0,
            },
            150.0,
            Some(-20.0),
        ))
    }

    /// A point on the ground straight up from the centre, and the way up there.
    fn surface(planet: &VoxelPlanet<GeneratedPlanet>) -> (Vec3, Vec3) {
        let up = Vec3::Y;
        (up * (2000.0 + planet.field().height_m(up)), up)
    }

    fn volume(radius: f32) -> f32 {
        4.0 / 3.0 * std::f32::consts::PI * radius * radius * radius
    }

    #[test]
    fn a_dig_takes_out_what_disappeared() {
        let mut planet = planet();
        let (ground, up) = surface(&planet);
        let deep = ground - up * 30.0;
        let taken = planet.apply(&Operation::Dig {
            centre: deep,
            radius: 2.0,
        });
        let total: f32 = taken.iter().map(|(_, amount)| amount).sum();
        assert!(
            (total - volume(2.0)).abs() < volume(2.0) * 0.05,
            "took {total} m³"
        );
        assert!(!planet.sample(deep).solid());
        assert!(
            planet.sample(deep + up * 3.0).solid(),
            "only the sphere went"
        );
        // Digging the same place again takes nothing more.
        let again = planet.apply(&Operation::Dig {
            centre: deep,
            radius: 2.0,
        });
        assert!(again.is_empty());
    }

    #[test]
    fn a_dig_at_the_surface_splits_by_material_and_takes_no_air() {
        let mut planet = planet();
        let (ground, _) = surface(&planet);
        let taken = planet.apply(&Operation::Dig {
            centre: ground,
            radius: 2.5,
        });
        let total: f32 = taken.iter().map(|(_, amount)| amount).sum();
        // About half the sphere was above ground.
        assert!(total > volume(2.5) * 0.35 && total < volume(2.5) * 0.65);
        let topsoil = taken.amount(Harvested::Material(material::SOIL))
            + taken.amount(Harvested::Material(material::SAND));
        assert!(topsoil > 0.0, "the top layer is in it");
        assert!(taken.amount(Harvested::Material(material::AIR)) == 0.0);
    }

    #[test]
    fn digging_then_filling_puts_the_ground_back() {
        let mut planet = planet();
        let (ground, up) = surface(&planet);
        let deep = ground - up * 30.0;
        let before: Vec<bool> = (0..64)
            .map(|i| {
                planet
                    .sample(deep + Vec3::new(i as f32 * 0.1 - 3.2, 0.3, 0.1))
                    .solid()
            })
            .collect();
        planet.apply(&Operation::Dig {
            centre: deep,
            radius: 2.0,
        });
        planet.apply(&Operation::Fill {
            centre: deep,
            radius: 2.0,
            material: material::STONE,
        });
        let after: Vec<bool> = (0..64)
            .map(|i| {
                planet
                    .sample(deep + Vec3::new(i as f32 * 0.1 - 3.2, 0.3, 0.1))
                    .solid()
            })
            .collect();
        assert_eq!(before, after);
    }

    #[test]
    fn the_same_operations_in_the_same_order_give_the_same_ground() {
        let (ground, up) = surface(&planet());
        let operations = [
            Operation::Dig {
                centre: ground,
                radius: 3.0,
            },
            Operation::Fill {
                centre: ground + up,
                radius: 1.5,
                material: material::ORE,
            },
            Operation::Dig {
                centre: ground - up,
                radius: 1.0,
            },
        ];
        let (mut one, mut two) = (planet(), planet());
        for operation in &operations {
            one.apply(operation);
            two.apply(operation);
        }
        for i in 0..100 {
            let point = ground + Vec3::new(i as f32 * 0.07 - 3.5, i as f32 * 0.05 - 2.5, 0.2);
            assert_eq!(one.sample(point), two.sample(point));
        }
        // Replaying what one kept gives the same ground again.
        let mut replayed = planet();
        for operation in one.operations() {
            replayed.apply(&operation);
        }
        assert_eq!(replayed.sample(ground), one.sample(ground));
        // And in another order, another ground: the order is part of the history.
        let mut three = planet();
        for operation in operations.iter().rev() {
            three.apply(operation);
        }
        assert_ne!(one.sample(ground + up), three.sample(ground + up));
    }

    #[test]
    fn it_answers_as_the_ground_does() {
        let mut planet = planet();
        let (ground, up) = surface(&planet);
        let hit = planet.raycast(ground + up * 10.0, -up, 20.0).unwrap();
        assert!(hit.point.distance(ground) < 0.05);
        planet.apply(&Operation::Dig {
            centre: ground,
            radius: 3.0,
        });
        let hit = planet.raycast(ground + up * 10.0, -up, 20.0).unwrap();
        assert!(
            (hit.point.distance(ground) - 3.0).abs() < 0.05,
            "the ray falls into the hole"
        );
        assert_eq!(planet.material(ground + up * 10.0), material::AIR);
    }

    #[test]
    fn a_box_dug_into_the_ground_is_flat_inside_and_measured() {
        let mut planet = planet();
        let (ground, up) = surface(&planet);
        // A room two by three metres and two high, its floor a metre below the ground,
        // turned a little about the way up.
        let rotation = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(0.4);
        let half = Vec3::new(1.0, 1.0, 1.5);
        let centre = ground;
        let taken = planet.apply(&Operation::DigBox {
            centre,
            half,
            rotation,
        });
        // About the half of the box below the ground, 6 cubic metres, came out.
        let volume: f32 = taken.iter().map(|(_, amount)| amount).sum();
        assert!((volume - 6.0).abs() < 0.5, "{volume}");
        // Inside, the ground is the box's floor; just outside it, untouched.
        let floor = centre - up * 1.0;
        assert!((planet.distance(floor + up * 0.5) - 0.5).abs() < 0.01);
        assert!(planet.distance(floor - up * 0.1) < 0.0);
        let outside = centre + rotation * Vec3::new(0.0, 0.0, 2.5) - up * 0.3;
        let undug = planet.field().distance_in(outside, &mut Default::default());
        assert_eq!(planet.distance(outside), undug);
        // Saved as it was given.
        assert_eq!(
            planet.operations(),
            vec![Operation::DigBox {
                centre,
                half,
                rotation
            }]
        );
    }
}
