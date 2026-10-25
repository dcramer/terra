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
    ///
    /// A fill counts everywhere: the ground it adds is nearer some points in the air than
    /// the ground under them, and leaving it out there would say the ground is farther
    /// than it is, so a ray could step clean over it. Beyond its reach a fill only lowers
    /// a distance that was more than the margin, and never below it, so nothing near the
    /// surface changes there and nothing there needs drawing again.
    fn edits_at(&self, point: Vec3) -> impl Iterator<Item = &Edit> {
        self.edits.iter().filter(move |edit| {
            let (centre, radius) = edit.centre_and_radius();
            let reach = radius + EDIT_MARGIN_M;
            matches!(edit, Edit::Fill { .. }) || point.distance_squared(centre) <= reach * reach
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

    fn steepest(&self) -> f32 {
        self.field.steepest()
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
    fn a_dig_at_the_surface_takes_the_ground_below_it_by_layer() {
        let mut planet = planet();
        // On level ground, where the layers lie flat.
        let ground = (0..400)
            .map(|i| {
                let up = Vec3::new(i as f32 * 0.003, 1.0, i as f32 * 0.002).normalize();
                up * (2000.0 + planet.field().height_m(up))
            })
            .find(|&at| planet.normal(at).dot(at.normalize()) > 0.998)
            .expect("somewhere level");
        let radius = 2.5;
        let taken = planet.apply(&Operation::Dig {
            centre: ground,
            radius,
        });
        let amount = |materials: &[MaterialId]| -> f32 {
            materials
                .iter()
                .map(|&m| taken.amount(Harvested::Material(m)))
                .sum()
        };
        // Ground through the middle of a ball halves it, however it slopes: the half
        // above was air, and none of that is taken.
        let half = volume(radius) / 2.0;
        let total: f32 = taken.iter().map(|(_, amount)| amount).sum();
        assert!(
            (total - half).abs() < half * 0.03,
            "took {total} m³ of {half}"
        );
        // Under the topsoil is stone: the cap of the ball deeper than the topsoil goes.
        let deep = radius - GeneratedPlanet::TOPSOIL_M;
        let cap = std::f32::consts::PI * deep * deep * (3.0 * radius - deep) / 3.0;
        let stone = amount(&[material::STONE, material::ORE]);
        let topsoil = amount(&[material::SOIL, material::SAND]);
        assert!(
            (stone - cap).abs() < cap * 0.05,
            "{stone} m³ of stone, not {cap}"
        );
        assert!(
            (topsoil - (half - cap)).abs() < (half - cap) * 0.03,
            "{topsoil} m³ of topsoil"
        );
    }

    #[test]
    fn digging_then_filling_puts_the_ground_back_in_the_new_material() {
        let mut planet = planet();
        let (ground, up) = surface(&planet);
        let deep = ground - up * 30.0;
        let line = |planet: &VoxelPlanet<GeneratedPlanet>| -> Vec<bool> {
            (0..64)
                .map(|i| {
                    planet
                        .sample(deep + Vec3::new(i as f32 * 0.1 - 3.2, 0.3, 0.1))
                        .solid()
                })
                .collect()
        };
        let before = line(&planet);
        let beside = deep + Vec3::X * 2.5;
        let was = planet.material(beside);
        planet.apply(&Operation::Dig {
            centre: deep,
            radius: 2.0,
        });
        // Sand, which is never found this deep, so it can only be the fill's.
        planet.apply(&Operation::Fill {
            centre: deep,
            radius: 2.0,
            material: material::SAND,
        });
        assert_eq!(line(&planet), before);
        for offset in [Vec3::ZERO, up * 1.8, Vec3::X * -1.8] {
            assert_eq!(planet.material(deep + offset), material::SAND);
        }
        assert_eq!(planet.material(beside), was, "outside the fill as it was");
    }

    #[test]
    fn replaying_the_operations_gives_the_same_ground_and_their_order_matters() {
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
            Operation::DigBox {
                centre: ground + Vec3::X,
                half: Vec3::new(0.5, 1.0, 1.5),
                rotation: Quat::from_rotation_y(0.4),
            },
        ];
        let mut one = planet();
        for operation in &operations {
            one.apply(operation);
        }
        // Replaying what it kept gives the same ground, everywhere the edits reached.
        let mut replayed = planet();
        for operation in one.operations() {
            replayed.apply(&operation);
        }
        let untouched = planet();
        let mut edited = 0;
        for i in 0..200 {
            let point = ground + Vec3::new(i as f32 * 0.035 - 3.5, i as f32 * 0.025 - 2.5, 0.2);
            assert_eq!(replayed.sample(point), one.sample(point), "at {point}");
            edited += usize::from(one.sample(point) != untouched.sample(point));
        }
        assert!(
            edited > 100,
            "the edits reach the points compared: {edited}"
        );
        // In another order, another ground: the order is part of the history.
        let mut reversed = planet();
        for operation in operations.iter().rev() {
            reversed.apply(operation);
        }
        assert_ne!(one.sample(ground + up), reversed.sample(ground + up));
    }

    #[test] // Slow: minutes.
    fn what_to_draw_again_holds_everything_the_edits_changed_near_the_surface() {
        let mut planet = planet();
        let (ground, up) = surface(&planet);
        // An earlier edit, already drawn.
        planet.apply(&Operation::Dig {
            centre: ground - up * 30.0,
            radius: 2.0,
        });
        let before = planet.clone();
        let first = planet.edit_count();
        let operations = [
            Operation::Dig {
                centre: ground - up * 0.5,
                radius: 1.5,
            },
            Operation::Fill {
                centre: ground + Vec3::X * 3.0,
                radius: 1.0,
                material: material::STONE,
            },
            Operation::DigBox {
                centre: ground - up * 20.0,
                half: Vec3::splat(1.0),
                rotation: Quat::IDENTITY,
            },
        ];
        for operation in &operations {
            planet.apply(operation);
        }
        let regions: Vec<(Vec3, f32)> = planet.edits_since(first).collect();
        assert_eq!(regions.len(), operations.len());
        assert!(planet.edits_since(planet.edit_count()).next().is_none());
        // Every point whose ground changed, where a mesh would see it, is in a region.
        let mut changed = 0;
        for centre in [ground, ground - up * 20.0] {
            for x in -24..=24 {
                for y in -24..=24 {
                    for z in -24..=24 {
                        let point = centre + Vec3::new(x as f32, y as f32, z as f32) * 0.25;
                        let (was, now) = (before.sample(point), planet.sample(point));
                        let near_surface = was.distance.abs().min(now.distance.abs()) < 2.0;
                        let differs = was.solid() != now.solid()
                            || (now.solid() && was.material != now.material)
                            || (near_surface && was.distance != now.distance);
                        if differs {
                            changed += 1;
                            assert!(
                                regions
                                    .iter()
                                    .any(|&(at, reach)| point.distance(at) <= reach),
                                "{point} changed outside every region"
                            );
                        }
                    }
                }
            }
        }
        assert!(changed > 1000, "{changed} points changed");
    }

    #[test]
    fn one_memo_kept_over_many_asks_answers_as_each_ask_alone() {
        // A body's mover asks the ground hundreds of times a tick through one memo kept from
        // tick to tick (D53): every answer must be the one a lone ask gives, edits included,
        // and a memo trimmed back must answer the same again.
        let mut planet = planet();
        let (ground, up) = surface(&planet);
        planet.apply(&Operation::Dig {
            centre: ground,
            radius: 1.5,
        });
        let side = up.any_orthonormal_vector();
        let mut memo = crate::HeightMemo::default();
        for i in 0..2000 {
            let (a, b) = (i as f32 * 0.37, i as f32 * 0.011);
            let at = ground + side * (b * libm::cosf(a) * 8.0) + up * libm::sinf(a * 3.0) * 2.0;
            let alone = Ground::distance(&planet, at);
            assert_eq!(planet.distance_in(at, &mut memo), alone, "at {at}");
            memo.keep_within(64);
        }
        assert!(memo.held() <= 64 + 4, "{} points held", memo.held());
    }

    #[test]
    fn filled_ground_is_walked_on_and_made_of_the_fill() {
        let mut planet = planet();
        let (ground, up) = surface(&planet);
        let centre = ground + up * 1.5;
        planet.apply(&Operation::Fill {
            centre,
            radius: 1.0,
            material: material::ORE,
        });
        // A ray straight down lands on top of the fill, which faces up there.
        let hit = planet.raycast(ground + up * 10.0, -up, 20.0).unwrap();
        assert!((hit.point.distance(centre) - 1.0).abs() < 0.05, "{hit:?}");
        assert!(planet.normal(hit.point).dot(up) > 0.95);
        assert_eq!(planet.material(hit.point - up * 0.1), material::ORE);
    }

    #[test]
    fn a_ray_does_not_pass_through_a_fill_high_above_the_ground() {
        // A fill well above the ground, as the end of a bridge would be: a ray toward it
        // meets it however far off it starts.
        let mut planet = planet();
        let (ground, up) = surface(&planet);
        let centre = ground + up * 20.0;
        planet.apply(&Operation::Fill {
            centre,
            radius: 0.6,
            material: material::STONE,
        });
        let across = up.any_orthonormal_vector();
        for i in 0..40 {
            let start = 20.0 + i as f32 * 0.37;
            let hit = planet.raycast(centre - across * start, across, 40.0);
            let hit = hit.unwrap_or_else(|| panic!("a ray from {start} m off missed"));
            assert!((hit.distance - (start - 0.6)).abs() < 0.05, "{hit:?}");
        }
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
