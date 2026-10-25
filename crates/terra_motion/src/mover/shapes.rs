//! What stops a body besides the ground (D53, docs/controller.md "What stops a body"): a
//! base's cubes and a door's leaves, props' boxes, and trees' and rocks' pieces, each answered
//! by its exact distance, so the mover's one query asks them as it asks the ground. Obstacles near
//! the capsule are picked by their bounds first, then answered as one exact field ([`Sdf`]) of
//! the union of them: the union of exact distances is exact outside it, so a floor of cubes
//! side by side has no seam to catch on, and a cube's edge on the floor is a crease the field
//! knows (`query`'s crease rule).
//!
//! A tree's or rock's solid is many convex pieces (`terra_core::Solid`, up to 32). As Jolt's
//! compound shapes cull their sub-shapes by bounds before testing one (`StaticCompoundShape`'s
//! bounding-volume tree, MIT) and Unreal tests a primitive only when its bounds overlap the
//! query (`FBodyInstance`'s per-shape bounds), a query keeps only the pieces whose bounds come
//! within its reach ([`Shaped::parts_near`]), nearest shape first, and the field skips a piece
//! whose bound is no nearer than the nearest found so far ([`Shaped::nearest_of`]): exact
//! still, as the union is, since a piece skipped is never the nearest.

use super::query::{Capsule, Contact, Hit, Query, Sdf};
use glam::{DVec3, Vec2, Vec3};
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
        self.distance_from(at - self.centre)
    }

    /// The exact distance from a point so far from its middle.
    fn distance_from(&self, d: Vec3) -> f32 {
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
    /// The distance from a world point in f64 (the mover's frames, `query`): a shape that can
    /// works its offset out in f64; others round the point to f32 (2 mm at 30 km).
    fn distance_at(&self, at: DVec3) -> f32 {
        self.distance(at.as_vec3())
    }
    /// The parts of it a query reaching so far from a point may touch, as [`Part`]s: one,
    /// all of it, for a shape that is not made of parts.
    fn parts_near(&self, at: Vec3, reach: f32, out: &mut Vec<Part>) {
        let _ = (at, reach);
        out.push(Part::WHOLE);
    }
    /// The least of `best` and the distance from a point to these parts of it: a part whose
    /// bound is no nearer than `best` is not asked.
    fn nearest_of(&self, parts: &[Part], at: DVec3, best: f32) -> f32 {
        let _ = parts;
        best.min(self.distance_at(at))
    }
}

/// A part of a shape a query keeps: its index, and its bounds as the shape says (a solid's
/// piece: its outline's middle in plan and how far round it reaches).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub index: u32,
    pub middle: Vec2,
    pub radius: f32,
}

impl Part {
    const WHOLE: Part = Part {
        index: 0,
        middle: Vec2::ZERO,
        radius: f32::INFINITY,
    };
}

/// The distance from a point in a solid's plan, so high over its ground, to a piece: a prism
/// of its rounded outline, open downward into the ground.
fn piece_distance(piece: &terra_core::Piece, plan: Vec2, height: f32) -> f32 {
    let w = Vec2::new(piece.gap(plan), height - piece.crest);
    w.max_element().min(0.0) + w.max(Vec2::ZERO).length()
}

impl Shaped for Block {
    fn distance(&self, at: Vec3) -> f32 {
        Block::distance(self, at)
    }

    fn distance_at(&self, at: DVec3) -> f32 {
        self.distance_from((at - self.centre.as_dvec3()).as_vec3())
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
            .map(|piece| piece_distance(piece, plan, height))
            .fold(f32::INFINITY, f32::min)
    }

    /// The pieces whose outline comes within the reach of the point in plan, and whose crest
    /// is not more than the reach under it, nearest first.
    fn parts_near(&self, at: Vec3, reach: f32, out: &mut Vec<Part>) {
        let (plan, height) = self.plan(at);
        let from = out.len();
        for (index, piece) in self.pieces.iter().enumerate() {
            if height - piece.crest >= reach {
                continue;
            }
            let (middle, radius) = piece.bounds();
            if plan.distance(middle) - radius < reach {
                out.push(Part {
                    index: index as u32,
                    middle,
                    radius,
                });
            }
        }
        let apart = |part: &Part| plan.distance(part.middle) - part.radius;
        out[from..].sort_by(|a, b| apart(a).total_cmp(&apart(b)));
    }

    /// Each piece's distance is no less than how far the point is outside the circle round its
    /// outline in plan, nor than how far over its crest it is: a piece no nearer by that than
    /// `best` is skipped.
    fn nearest_of(&self, parts: &[Part], at: DVec3, best: f32) -> f32 {
        let (plan, height) = self.plan(at.as_vec3());
        let mut best = best;
        for part in parts {
            let piece = &self.pieces[part.index as usize];
            let below = (plan.distance(part.middle) - part.radius).max(height - piece.crest);
            if below < best {
                best = best.min(piece_distance(piece, plan, height));
            }
        }
        best
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

    fn distance_at(&self, at: DVec3) -> f32 {
        match self {
            Obstacle::Block(block) => Shaped::distance_at(block, at),
            Obstacle::Solid(solid) => Shaped::distance_at(solid, at),
        }
    }

    fn bounds(&self) -> (Vec3, f32) {
        match self {
            Obstacle::Block(block) => Shaped::bounds(block),
            Obstacle::Solid(solid) => solid.bounds(),
        }
    }

    fn parts_near(&self, at: Vec3, reach: f32, out: &mut Vec<Part>) {
        match self {
            Obstacle::Block(block) => block.parts_near(at, reach, out),
            Obstacle::Solid(solid) => solid.parts_near(at, reach, out),
        }
    }

    fn nearest_of(&self, parts: &[Part], at: DVec3, best: f32) -> f32 {
        match self {
            Obstacle::Block(block) => block.nearest_of(parts, at, best),
            Obstacle::Solid(solid) => solid.nearest_of(parts, at, best),
        }
    }
}

impl<S: Shaped + ?Sized> Shaped for &S {
    fn distance(&self, at: Vec3) -> f32 {
        (**self).distance(at)
    }

    fn distance_at(&self, at: DVec3) -> f32 {
        (**self).distance_at(at)
    }

    fn bounds(&self) -> (Vec3, f32) {
        (**self).bounds()
    }

    fn parts_near(&self, at: Vec3, reach: f32, out: &mut Vec<Part>) {
        (**self).parts_near(at, reach, out)
    }

    fn nearest_of(&self, parts: &[Part], at: DVec3, best: f32) -> f32 {
        (**self).nearest_of(parts, at, best)
    }
}

/// Obstacles as the mover's query asks them: a slice of shapes, owned or borrowed (`&[&Solid]`
/// out of flora's index, `&[Block]` of a base's or props').
pub struct Obstacles<'a, S = Obstacle> {
    pub obstacles: &'a [S],
}

/// The shapes a query may touch, nearest first, each with its parts that it may.
struct Near<'a, S> {
    shapes: Vec<(&'a S, std::ops::Range<usize>)>,
    parts: Vec<Part>,
}

impl<S: Shaped> Obstacles<'_, S> {
    /// The shapes, and their parts, within a distance of a capsule swept along a motion.
    fn near(&self, capsule: &Capsule, motion: Vec3, within: f32) -> Near<'_, S> {
        let middle = capsule
            .world((capsule.a + capsule.b + motion) * 0.5)
            .as_vec3();
        let reach = (capsule.b - capsule.a).length() * 0.5 + capsule.radius + within;
        let reach = reach + motion.length() * 0.5;
        let mut by: Vec<(f32, &S)> = self
            .obstacles
            .iter()
            .filter_map(|shape| {
                let (centre, radius) = shape.bounds();
                let apart = centre.distance(middle) - radius;
                (apart < reach).then_some((apart, shape))
            })
            .collect();
        by.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut near = Near {
            shapes: Vec::with_capacity(by.len()),
            parts: Vec::new(),
        };
        for (_, shape) in by {
            let from = near.parts.len();
            shape.parts_near(middle, reach, &mut near.parts);
            if near.parts.len() > from {
                near.shapes.push((shape, from..near.parts.len()));
            }
        }
        near
    }
}

/// The union of some shapes' parts as one exact field.
fn field<'a, S: Shaped>(near: &'a Near<'a, S>) -> Sdf<impl Fn(DVec3) -> f32 + 'a> {
    Sdf {
        distance: move |at: DVec3| {
            near.shapes
                .iter()
                .fold(f32::INFINITY, |best, (shape, parts)| {
                    shape.nearest_of(&near.parts[parts.clone()], at, best)
                })
        },
        steepest: 1.0,
    }
}

impl<S: Shaped> Query for Obstacles<'_, S> {
    fn contacts(&self, capsule: &Capsule, within: f32, out: &mut Vec<Contact>) {
        let near = self.near(capsule, Vec3::ZERO, within);
        if !near.shapes.is_empty() {
            field(&near).contacts(capsule, within, out);
        }
    }

    fn sweep(&self, capsule: &Capsule, motion: Vec3) -> Option<Hit> {
        let near = self.near(capsule, motion, super::query::TOLERANCE);
        if near.shapes.is_empty() {
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
    fn a_query_kept_to_the_pieces_near_it_answers_as_all_of_them() {
        // A trunk and its root flare: a round piece, and outlines of 3 to 8 corners round it,
        // some low enough to step onto, on ground tilted as on a hillside.
        let mut pieces = vec![Piece::round(Vec2::ZERO, 0.3, 2.5)];
        for i in 0..8 {
            let a = i as f32 * 0.785;
            let (c, s) = (libm::cosf(a), libm::sinf(a));
            let corners = 3 + i % 6;
            let hull = (0..corners)
                .map(|k| {
                    let b = k as f32 * std::f32::consts::TAU / corners as f32;
                    Vec2::new(c, s) * 0.6 + Vec2::new(libm::cosf(b), libm::sinf(b)) * 0.25
                })
                .collect();
            let top = 0.1 + 0.15 * i as f32;
            pieces.push(Piece {
                hull,
                radius: 0.02,
                top,
                crest: top,
            });
        }
        let up = Vec3::new(0.2, 1.0, 0.1).normalize();
        let trunk = Solid::of_pieces(up * 100.0, pieces);
        let (reach, mut worst) = (1.2, 0.0_f32);
        for i in 0..400 {
            // Spots round it, up to 2 m off and from under its foot to over its crown.
            let (a, r, h) = (
                i as f32 * 2.399,
                (i % 20) as f32 * 0.1,
                (i % 7) as f32 * 0.5 - 0.5,
            );
            let x = up.any_orthonormal_vector();
            let z = up.cross(x);
            let middle = trunk.at + (x * libm::cosf(a) + z * libm::sinf(a)) * r + up * h;
            let mut parts = Vec::new();
            trunk.parts_near(middle, reach, &mut parts);
            for d in [Vec3::ZERO, x * 0.4, z * -0.5, up * 0.3] {
                let at = middle + d;
                let whole = Shaped::distance(&trunk, at);
                let kept = trunk.nearest_of(&parts, at.as_dvec3(), f32::INFINITY);
                if whole < reach - d.length() {
                    worst = worst.max((whole - kept).abs());
                } else {
                    assert!(kept >= whole - 1e-5, "kept {kept} nearer than all {whole}");
                }
            }
        }
        assert!(worst < 1e-5, "off by {worst} m within the query's reach");
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
                origin: DVec3::ZERO,
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
