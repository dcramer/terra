//! An item is solid: where its box may stand in a base, what a ray meets of it, and how
//! it keeps a walker out.

use super::{Prop, STEP_OVER_M};
use crate::play::Footprint;
use crate::play::bodies::Solid;
use crate::play::flora::Obstacles;
use bevy::prelude::*;
use terra_build::{Base, CELL_M};

/// An item's box as it would stand: where, how big, and how turned, in the base's own
/// space.
#[derive(Clone, Copy)]
pub struct Standing {
    pub at: Vec3,
    pub lift: f32,
    pub turn: f32,
    pub half: Vec3,
    pub indoors: bool,
}

impl Standing {
    pub fn of(prop: &Prop) -> Self {
        Standing {
            at: prop.at,
            lift: prop.lift,
            turn: prop.turn,
            half: prop.half,
            indoors: prop.indoors,
        }
    }

    /// The four corners of its foot, east and south.
    fn corners(&self) -> [Vec2; 4] {
        let turned = Quat::from_rotation_y(self.turn.to_radians());
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(x, z)| {
            let corner = self.at + turned * Vec3::new(x * self.half.x, 0.0, z * self.half.z);
            corner.xz()
        })
    }

    /// How far its box reaches from its middle in a level direction.
    pub fn reach_toward(&self, direction: Vec3) -> f32 {
        let turned = Quat::from_rotation_y(self.turn.to_radians());
        self.half.x * (turned * Vec3::X).dot(direction).abs()
            + self.half.z * (turned * Vec3::Z).dot(direction).abs()
    }

    fn is_solid(&self) -> bool {
        self.half.y * 2.0 > STEP_OVER_M
    }

    /// Whether a wall runs through the box's foot, near enough: whether any tile its
    /// outline covers is a wall (D181).
    fn crosses_a_wall(&self, base: &Base) -> bool {
        let corners = self.corners();
        let low = corners
            .iter()
            .fold(Vec2::MAX, |low, corner| low.min(*corner));
        let high = corners
            .iter()
            .fold(Vec2::MIN, |high, corner| high.max(*corner));
        let cell = |at: Vec2| (at / CELL_M).floor().as_ivec2();
        let (low, high) = (cell(low + 0.02), cell(high - 0.02));
        let storey = (self.at.y / terra_build::STOREY_M).round() as i32;
        (low.x..=high.x)
            .any(|x| (low.y..=high.y).any(|z| base.filling(IVec3::new(x, storey, z)).is_some()))
    }

    /// Whether two boxes share space. Boxes that only touch do not.
    pub fn overlaps(&self, other: &Standing) -> bool {
        const TOUCHING: f32 = 0.03;
        let (low, high) = (self.lift, self.lift + self.half.y * 2.0);
        let (other_low, other_high) = (other.lift, other.lift + other.half.y * 2.0);
        if self.indoors != other.indoors
            || high < other_low + TOUCHING
            || other_high < low + TOUCHING
        {
            return false;
        }
        // Two rectangles are apart if some edge of either has the other wholly beyond it.
        let (ours, theirs) = (self.corners(), other.corners());
        let apart_along = |corners: &[Vec2; 4]| {
            (0..2).any(|edge| {
                let axis = (corners[edge + 1] - corners[edge])
                    .perp()
                    .normalize_or_zero();
                let span = |shape: &[Vec2; 4]| {
                    shape
                        .iter()
                        .map(|corner| corner.dot(axis))
                        .fold((f32::MAX, f32::MIN), |(low, high), at| {
                            (low.min(at), high.max(at))
                        })
                };
                let (a, b) = (span(&ours), span(&theirs));
                a.1 < b.0 + TOUCHING || b.1 < a.0 + TOUCHING
            })
        };
        !apart_along(&ours) && !apart_along(&theirs)
    }

    /// Whether a circle on the ground, given in the base's own space, reaches into the
    /// box's foot.
    fn meets_circle(&self, middle: Vec3, radius: f32) -> bool {
        let turned_back = Quat::from_rotation_y(-self.turn.to_radians());
        let local = turned_back * (middle - self.at);
        let outside = (Vec2::new(local.x.abs(), local.z.abs())
            - Vec2::new(self.half.x, self.half.z))
        .max(Vec2::ZERO);
        outside.length() < radius
    }
}

/// Why an item cannot stand where it is offered.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Blocked {
    /// It would stick through a wall, or stand half in the base and half out.
    Structure,
    /// Another item is there.
    Item(Entity),
    /// A tree or a boulder is there.
    Nature,
    Water,
}

/// What stops an item standing somewhere in a base, if anything, given the base's shape
/// and where it stands on the ground. It has to be wholly indoors or wholly clear of the
/// base, and not in the same space as another item, a tree or a boulder. Rugs and other
/// flat things share space with anything.
pub fn blocked<'a>(
    (base, footprint): (&Base, &Footprint),
    standing: &Standing,
    others: impl Iterator<Item = (Entity, &'a Prop)>,
    obstacles: &Obstacles,
) -> Option<Blocked> {
    let frame = footprint.frame;
    let placed = standing.corners().iter().all(|corner| {
        let at = Vec3::new(corner.x, 0.0, corner.y);
        if standing.indoors {
            base.contains((at / CELL_M).floor().as_ivec3().with_y(0))
        } else {
            !footprint.covers(frame.transform_point(at), 0.05)
        }
    });
    if !placed || standing.crosses_a_wall(base) {
        return Some(Blocked::Structure);
    }
    if !standing.is_solid() {
        return None;
    }
    if let Some((entity, _)) = others
        .filter(|(_, other)| other.is_solid())
        .find(|(_, other)| standing.overlaps(&Standing::of(other)))
    {
        return Some(Blocked::Item(entity));
    }
    let to_base = frame.compute_affine().inverse();
    let middle = frame.transform_point(standing.at);
    (!standing.indoors
        && obstacles
            .near(middle, standing.half.length())
            .any(|(at, radius)| {
                standing.meets_circle(to_base.transform_point3(at).with_y(0.0), radius)
            }))
    .then_some(Blocked::Nature)
}

/// Where a ray first meets an item's box, as the distance along the ray and which way
/// the side it met faces. The ray and the answer are in the world's own space.
pub fn ray_box(
    transform: &Transform,
    half: Vec3,
    origin: Vec3,
    direction: Vec3,
) -> Option<(f32, Vec3)> {
    let turned_back = transform.rotation.inverse();
    let from = turned_back * (origin - transform.translation);
    let along = turned_back * direction;
    let low = Vec3::new(-half.x, 0.0, -half.z);
    let high = Vec3::new(half.x, half.y * 2.0, half.z);
    let (mut near, mut far, mut side) = (0.0_f32, f32::MAX, Vec3::ZERO);
    for axis in 0..3 {
        if along[axis].abs() < 1e-6 {
            if from[axis] < low[axis] || from[axis] > high[axis] {
                return None;
            }
            continue;
        }
        let enter = (low[axis] - from[axis]) / along[axis];
        let leave = (high[axis] - from[axis]) / along[axis];
        let (enter, leave, facing) = if enter <= leave {
            (enter, leave, -1.0)
        } else {
            (leave, enter, 1.0)
        };
        if enter > near {
            near = enter;
            side = Vec3::ZERO;
            side[axis] = facing;
        }
        far = far.min(leave);
        if near > far {
            return None;
        }
    }
    // Starting inside the box meets no side of it.
    (side != Vec3::ZERO).then(|| (near, transform.rotation * side))
}

/// Gives an item put down the walker's [`Solid`] if it is too tall to step over, and takes
/// it away if it is not, so walkers go round it (`bodies::push_out_of_solids`).
pub(super) fn solid_or_not(put: On<Insert<Prop>>, props: Query<&Prop>, mut commands: Commands) {
    let Ok(prop) = props.get(put.entity) else {
        return;
    };
    if prop.is_solid() {
        commands
            .entity(put.entity)
            .insert(Solid { half: prop.half });
    } else {
        commands.entity(put.entity).remove::<Solid>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_put_down_is_solid_and_a_rug_is_walked_over() {
        let mut world = World::new();
        world.add_observer(solid_or_not);
        let chest = world.spawn(Prop::boxed(Vec3::splat(0.4), None)).id();
        let flat = || {
            let mut rug = Prop::boxed(Vec3::splat(0.4), None);
            rug.half.y = 0.01;
            rug
        };
        let rug = world.spawn(flat()).id();
        world.flush();
        assert_eq!(
            world.get::<Solid>(chest),
            Some(&Solid {
                half: Vec3::splat(0.4)
            })
        );
        assert_eq!(world.get::<Solid>(rug), None);
        // Put back down as a rug, the chest is walked over too.
        world.entity_mut(chest).insert(flat());
        world.flush();
        assert_eq!(world.get::<Solid>(chest), None);
    }

    fn standing(east: f32, turn: f32) -> Standing {
        Standing {
            at: Vec3::new(east, 0.0, 0.0),
            lift: 0.0,
            turn,
            half: Vec3::new(1.0, 0.4, 0.3),
            indoors: true,
        }
    }

    #[test]
    fn boxes_overlap_only_when_they_share_space() {
        let here = standing(0.0, 0.0);
        assert!(here.overlaps(&standing(1.5, 0.0)));
        // Side by side and touching is not overlapping.
        assert!(!here.overlaps(&standing(2.0, 0.0)));
        // Turned a quarter circle it is narrow, and fits closer.
        assert!(!here.overlaps(&standing(1.4, 90.0)));
        assert!(here.overlaps(&standing(1.2, 90.0)));
        // Stood on top of it, or in a different place altogether.
        let above = Standing { lift: 0.8, ..here };
        assert!(!here.overlaps(&above));
        let outside = Standing {
            indoors: false,
            ..here
        };
        assert!(!here.overlaps(&outside));
    }

    #[test]
    fn a_box_knows_how_far_it_reaches_and_what_circles_touch_it() {
        let here = standing(0.0, 0.0);
        assert!((here.reach_toward(Vec3::X) - 1.0).abs() < 1e-5);
        assert!((here.reach_toward(Vec3::Z) - 0.3).abs() < 1e-5);
        assert!((standing(0.0, 90.0).reach_toward(Vec3::X) - 0.3).abs() < 1e-5);
        assert!(here.meets_circle(Vec3::new(1.2, 0.0, 0.0), 0.3));
        assert!(!here.meets_circle(Vec3::new(1.4, 0.0, 0.0), 0.3));
        assert!(!here.meets_circle(Vec3::new(0.0, 0.0, 0.7), 0.3));
    }

    #[test]
    fn a_ray_meets_the_nearest_side_of_a_box() {
        let stood = Transform::from_xyz(5.0, 0.0, 0.0);
        let half = Vec3::new(0.5, 0.4, 0.5);
        // Level, from the west: its west side, half a metre short of its middle.
        let (distance, side) = ray_box(&stood, half, Vec3::new(0.0, 0.4, 0.0), Vec3::X).unwrap();
        assert!((distance - 4.5).abs() < 1e-5 && side.distance(Vec3::NEG_X) < 1e-5);
        // From above: its top.
        let (distance, side) =
            ray_box(&stood, half, Vec3::new(5.0, 3.0, 0.0), Vec3::NEG_Y).unwrap();
        assert!((distance - 2.2).abs() < 1e-5 && side.distance(Vec3::Y) < 1e-5);
        // Past it, or away from it.
        assert!(ray_box(&stood, half, Vec3::new(0.0, 0.4, 2.0), Vec3::X).is_none());
        assert!(ray_box(&stood, half, Vec3::new(0.0, 0.4, 0.0), Vec3::NEG_X).is_none());
    }

    /// A room ten tiles by ten outside, eight by eight inside, at the world's middle,
    /// with a stub of wall standing alone in it.
    fn room() -> (Base, Footprint) {
        use terra_build::{BuildOp, Kind, Panel};
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 9],
            kind: Kind::PLAIN,
        });
        base.apply(BuildOp::SetWall {
            tile: [5, 0, 5],
            panel: Some(Panel::Solid),
            kind: Kind::PLAIN,
        });
        assert!(base.contains(IVec3::new(4, 0, 4)) && !base.contains(IVec3::new(5, 0, 5)));
        let footprint = Footprint {
            frame: Transform::IDENTITY,
            cell_m: CELL_M,
            ground: base.ground_columns().collect(),
            standing: base.occupied_columns().collect(),
        };
        (base, footprint)
    }

    /// A box half a metre across and a metre high, its middle at a place.
    fn boxed(east: f32, south: f32, indoors: bool) -> Standing {
        Standing {
            at: Vec3::new(east, 0.0, south),
            lift: 0.0,
            turn: 0.0,
            half: Vec3::new(0.25, 0.5, 0.25),
            indoors,
        }
    }

    #[test]
    fn an_item_stands_wholly_indoors_or_wholly_out() {
        let (base, footprint) = room();
        let blocked = |standing| {
            blocked(
                (&base, &footprint),
                &standing,
                std::iter::empty(),
                &Obstacles::default(),
            )
        };
        // Inside the room, clear of the walls; and well out of the base.
        assert_eq!(blocked(boxed(1.5, 1.5, true)), None);
        assert_eq!(blocked(boxed(10.0, 10.0, false)), None);
        // Through the room's wall, from either side, and outdoors in the room.
        assert_eq!(blocked(boxed(0.6, 1.5, true)), Some(Blocked::Structure));
        assert_eq!(blocked(boxed(-0.1, 1.5, false)), Some(Blocked::Structure));
        // Indoors where there is no room.
        assert_eq!(blocked(boxed(10.0, 10.0, true)), Some(Blocked::Structure));
        assert_eq!(blocked(boxed(1.5, 1.5, false)), Some(Blocked::Structure));
        // Round a wall standing in the room, every corner of it in the room.
        let over_the_stub = Standing {
            half: Vec3::new(0.6, 0.5, 0.6),
            ..boxed(2.75, 2.75, true)
        };
        assert_eq!(blocked(over_the_stub), Some(Blocked::Structure));
    }

    #[test]
    fn a_solid_item_is_kept_out_of_another_but_a_rug_is_not() {
        let (base, footprint) = room();
        let mut world = World::new();
        let mut put = |at: Vec3, half: Vec3| {
            let prop = Prop {
                at,
                indoors: true,
                ..Prop::boxed(half, None)
            };
            world.spawn(prop).id()
        };
        let chest = put(Vec3::new(1.5, 0.0, 1.5), Vec3::new(0.25, 0.5, 0.25));
        // Low enough to step over, though not so low it shares no space with a box.
        let rug = put(Vec3::new(3.5, 0.0, 1.5), Vec3::new(0.5, 0.1, 0.5));
        let props: Vec<(Entity, &Prop)> = [chest, rug]
            .map(|entity| (entity, world.get::<Prop>(entity).unwrap()))
            .to_vec();
        let blocked = |standing| {
            blocked(
                (&base, &footprint),
                &standing,
                props.iter().copied(),
                &Obstacles::default(),
            )
        };
        assert_eq!(blocked(boxed(1.7, 1.5, true)), Some(Blocked::Item(chest)));
        assert_eq!(blocked(boxed(3.5, 1.5, true)), None, "on the rug");
        let flat = Standing {
            half: Vec3::new(0.25, 0.1, 0.25),
            ..boxed(1.5, 1.5, true)
        };
        assert_eq!(blocked(flat), None, "a rug under the chest");
    }

    #[test]
    fn an_item_is_kept_out_of_a_tree_outdoors() {
        let (base, footprint) = room();
        let mut trees = Obstacles::default();
        trees.add(Vec3::new(10.0, 0.0, 10.0), 0.3);
        let blocked =
            |standing| blocked((&base, &footprint), &standing, std::iter::empty(), &trees);
        assert_eq!(blocked(boxed(10.4, 10.0, false)), Some(Blocked::Nature));
        assert_eq!(blocked(boxed(11.0, 10.0, false)), None);
    }
}
