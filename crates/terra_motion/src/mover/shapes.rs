//! What stops a body besides the ground (D53, docs/controller.md "What stops a body"): a
//! base's cubes and a door's leaves, props' boxes, and trees' and rocks' pieces, each answered
//! by its exact distance, so the mover's one query asks them as it asks the ground. Obstacles near
//! the capsule are picked by their bounds first, then answered as one exact field ([`Sdf`]) of
//! the union of them: the union of exact distances is exact outside it, so a floor of cubes
//! side by side has no seam to catch on, and a cube's edge on the floor is a crease the field
//! knows (`query`'s crease rule).

use super::query::{Capsule, Contact, Hit, Query, Sdf};
use glam::{Vec2, Vec3};
use terra_core::Solid;

/// A box turned in the world: its middle, its three axes (orthonormal), and half its size
/// along each. A prop's box, a base's cube or a door's leaf.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Block {
    pub centre: Vec3,
    pub axes: [Vec3; 3],
    pub half: Vec3,
}

impl Block {
    /// The exact distance from a point to it, negative inside.
    pub fn distance(&self, at: Vec3) -> f32 {
        let d = at - self.centre;
        let local = Vec3::new(
            d.dot(self.axes[0]),
            d.dot(self.axes[1]),
            d.dot(self.axes[2]),
        );
        let q = local.abs() - self.half;
        q.max(Vec3::ZERO).length() + q.max_element().min(0.0)
    }
}

/// Something a body meets besides the ground.
#[derive(Clone, Debug)]
pub enum Obstacle {
    Block(Block),
    /// A tree's trunk, a rock, a stump or a log (`terra_core::solid`, D267): each of its convex
    /// pieces stands from under the ground up to its crest, its outline rounded in plan.
    Solid(Solid),
}

impl Obstacle {
    /// The exact distance from a point to it, negative inside.
    pub fn distance(&self, at: Vec3) -> f32 {
        Shaped::distance(self, at)
    }
}

/// What an obstacle is to the query: its exact distance from a point (negative inside) and a
/// sphere round all of it, its middle and radius. Owned or borrowed, so the world's obstacles
/// (flora's solids, kept in their own index) are asked where they lie, never copied a tick.
pub trait Shaped {
    fn distance(&self, at: Vec3) -> f32;
    fn bounds(&self) -> (Vec3, f32);
}

impl Shaped for Block {
    fn distance(&self, at: Vec3) -> f32 {
        Block::distance(self, at)
    }

    fn bounds(&self) -> (Vec3, f32) {
        (self.centre, self.half.length())
    }
}

impl Shaped for Solid {
    fn distance(&self, at: Vec3) -> f32 {
        let (plan, height) = self.plan(at);
        self.pieces
            .iter()
            .map(|piece| {
                // A prism of its rounded outline, open downward into the ground.
                let w = Vec2::new(piece.distance(plan).0, height - piece.crest);
                w.max_element().min(0.0) + w.max(Vec2::ZERO).length()
            })
            .fold(f32::INFINITY, f32::min)
    }

    fn bounds(&self) -> (Vec3, f32) {
        let crest = self.pieces.iter().map(|p| p.crest).fold(0.0, f32::max);
        let (_, height) = self.plan(self.at);
        (self.at, self.reach + crest.max(0.0) + height.abs())
    }
}

impl Shaped for Obstacle {
    fn distance(&self, at: Vec3) -> f32 {
        match self {
            Obstacle::Block(block) => block.distance(at),
            Obstacle::Solid(solid) => Shaped::distance(solid, at),
        }
    }

    fn bounds(&self) -> (Vec3, f32) {
        match self {
            Obstacle::Block(block) => Shaped::bounds(block),
            Obstacle::Solid(solid) => solid.bounds(),
        }
    }
}

impl<S: Shaped + ?Sized> Shaped for &S {
    fn distance(&self, at: Vec3) -> f32 {
        (**self).distance(at)
    }

    fn bounds(&self) -> (Vec3, f32) {
        (**self).bounds()
    }
}

/// Obstacles as the mover's query asks them: a slice of shapes, owned or borrowed (`&[&Solid]`
/// out of flora's index, `&[Block]` of a base's or props').
pub struct Obstacles<'a, S = Obstacle> {
    pub obstacles: &'a [S],
}

impl<S: Shaped> Obstacles<'_, S> {
    /// The shapes within a distance of a capsule swept along a motion.
    fn near(&self, capsule: &Capsule, motion: Vec3, within: f32) -> Vec<&S> {
        let middle = (capsule.a + capsule.b + motion) * 0.5;
        let reach = (capsule.b - capsule.a).length() * 0.5 + capsule.radius + within;
        let reach = reach + motion.length() * 0.5;
        self.obstacles
            .iter()
            .filter(|shape| {
                let (centre, radius) = shape.bounds();
                centre.distance(middle) < reach + radius
            })
            .collect()
    }
}

/// The union of some shapes as one exact field.
fn field<'a, S: Shaped>(near: &'a [&'a S]) -> Sdf<impl Fn(Vec3) -> f32 + 'a> {
    Sdf {
        distance: move |at: Vec3| {
            near.iter()
                .map(|shape| shape.distance(at))
                .fold(f32::INFINITY, f32::min)
        },
        steepest: 1.0,
    }
}

impl<S: Shaped> Query for Obstacles<'_, S> {
    fn contacts(&self, capsule: &Capsule, within: f32, out: &mut Vec<Contact>) {
        let near = self.near(capsule, Vec3::ZERO, within);
        if !near.is_empty() {
            field(&near).contacts(capsule, within, out);
        }
    }

    fn sweep(&self, capsule: &Capsule, motion: Vec3) -> Option<Hit> {
        let near = self.near(capsule, motion, super::query::TOLERANCE);
        if near.is_empty() {
            return None;
        }
        field(&near).sweep(capsule, motion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::Piece;

    #[test]
    fn a_turned_box_and_a_round_rock_are_their_distances_by_hand() {
        let (s, c) = (libm::sinf(0.5), libm::cosf(0.5));
        let x = Vec3::new(c, 0.0, -s);
        let block = Block {
            centre: Vec3::new(1.0, 2.0, 3.0),
            axes: [x, Vec3::Y, x.cross(Vec3::Y)],
            half: Vec3::new(1.0, 0.5, 2.0),
        };
        // 0.25 m out of its top, 0.5 m out of its turned side, and 0.5 m in from its top.
        assert!((block.distance(Vec3::new(1.0, 2.75, 3.0)) - 0.25).abs() < 1e-5);
        assert!((block.distance(block.centre + x * 1.5) - 0.5).abs() < 1e-5);
        assert!((block.distance(Vec3::new(1.0, 2.0, 3.0)) + 0.5).abs() < 1e-5);
        // A round rock 1 m across, 0.4 m tall, on the level at y = 10.
        let rock = Obstacle::Solid(Solid::of_pieces(
            Vec3::Y * 10.0,
            vec![Piece::round(Vec2::ZERO, 0.5, 0.4)],
        ));
        assert!((rock.distance(Vec3::new(0.0, 10.6, 0.0)) - 0.2).abs() < 1e-4);
        assert!((rock.distance(Vec3::new(0.8, 10.2, 0.0)) - 0.3).abs() < 1e-4);
        // Past its top's edge: the distance to the rim.
        let rim = Vec2::new(0.3, 0.3).length();
        assert!((rock.distance(Vec3::new(0.8, 10.7, 0.0)) - rim).abs() < 1e-4);
        assert!(
            rock.distance(Vec3::new(0.0, 9.0, 0.0)) < 0.0,
            "open into the ground"
        );
    }

    #[test]
    fn a_rock_borrowed_from_where_it_is_kept_stops_a_body_as_one_owned() {
        let rock = Solid::of_pieces(Vec3::ZERO, vec![Piece::round(Vec2::ZERO, 0.5, 1.5)]);
        let owned = [Obstacle::Solid(rock.clone())];
        let borrowed = [&rock];
        let (owned, borrowed) = (
            Obstacles { obstacles: &owned },
            Obstacles {
                obstacles: &borrowed,
            },
        );
        let capsule = Capsule::standing(Vec3::new(-2.0, 0.01, 0.0), Vec3::Y, 0.3, 1.8);
        let (a, b) = (
            owned
                .sweep(&capsule, Vec3::X * 3.0)
                .expect("it meets the rock"),
            borrowed
                .sweep(&capsule, Vec3::X * 3.0)
                .expect("and borrowed"),
        );
        // Its face at x = -0.5, the capsule's 0.3 m short of it: 1.2 m of the 3.
        assert!((a.fraction - 1.2 / 3.0).abs() < 0.01, "{}", a.fraction);
        assert_eq!(a.fraction, b.fraction);
        let near = capsule.moved(Vec3::X * 1.1);
        let (mut x, mut y) = (Vec::new(), Vec::new());
        owned.contacts(&near, 0.1, &mut x);
        borrowed.contacts(&near, 0.1, &mut y);
        assert!(!x.is_empty() && x == y, "{x:?} {y:?}");
    }

    #[test]
    fn cubes_side_by_side_are_one_floor_with_no_seam() {
        let cube = |x: f32| {
            Obstacle::Block(Block {
                centre: Vec3::new(x, -0.25, 0.0),
                axes: [Vec3::X, Vec3::Y, Vec3::Z],
                half: Vec3::splat(0.25),
            })
        };
        let shapes = [cube(0.25), cube(0.75)];
        let query = Obstacles { obstacles: &shapes };
        // A capsule standing on the seam meets one floor, facing up.
        let capsule = Capsule::standing(Vec3::new(0.5, 0.001, 0.0), Vec3::Y, 0.3, 1.8);
        let mut out = Vec::new();
        query.contacts(&capsule, 0.1, &mut out);
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(out[0].normal.dot(Vec3::Y) > 0.999);
        // Swept along it, nothing stops it.
        assert!(query.sweep(&capsule, Vec3::X * 0.3).is_none());
    }

    #[test]
    fn a_probe_swept_down_by_a_riser_stops_on_the_step_not_under_it() {
        // Stair cubes: a step a quarter of a metre up from x = 0, as the union of two boxes
        // running on, the higher overlapping the lower.
        let from = |x: f32, top: f32| {
            Obstacle::Block(Block {
                centre: Vec3::new(x + 2.0, top * 0.5 - 0.25, 0.0),
                axes: [Vec3::X, Vec3::Y, Vec3::Z],
                half: Vec3::new(2.0, top * 0.5 + 0.25, 1.0),
            })
        };
        let shapes = [from(-0.25, 0.25), from(0.0, 0.5)];
        let query = Obstacles { obstacles: &shapes };
        // A thin probe a centimetre or two past the riser, within a graze step of it: the
        // riser is nearer than the top just under the top, but the step's top stops it.
        for x in [0.005, 0.0227, 0.04] {
            let at = Vec3::new(x, 0.9445, 0.0);
            let probe = Capsule {
                a: at,
                b: at,
                radius: 0.002,
            };
            let hit = query.sweep(&probe, Vec3::NEG_Y * 0.8).expect("the step");
            let stops = at.y - 0.8 * hit.fraction - 0.002;
            assert!((stops - 0.5).abs() < 2.0e-3, "x {x}: stopped at {stops}");
            assert!(hit.contact.normal.dot(Vec3::Y) > 0.99, "{hit:?}");
        }
    }
}
