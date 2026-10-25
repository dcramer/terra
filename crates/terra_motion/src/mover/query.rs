//! What stops a body: the one query trait the mover asks (D53, docs/controller.md "What stops
//! a body"), defined here because the mover is the side that asks (D198). It has two
//! questions, the contacts within a distance of a capsule and the first contact of a capsule
//! swept along a motion, and each source answers from what it already holds:
//!
//! - [`Sdf`]: ground as a signed distance field (voxel ground, dug or not, and the course's
//!   fixtures). A sweep advances the capsule by its distance over the field's steepest slope
//!   (`Field::steepest`), never more than a tenth of a voxel past a surface it is touching, so
//!   a wall a voxel thick cannot be skipped at any speed.
//! - [`Capsules`]: other bodies' pushboxes, exactly.
//!
//! Sources are combined with a pair `(&a, &b)`.

use glam::Vec3;

/// How far a contact may sink before a sweep calls it a hit, m (Jolt's collision tolerance).
pub const TOLERANCE: f32 = 1.0e-3;
/// The longest a sweep steps past a surface it is touching or leaving, m: a tenth of a voxel,
/// the ground's thinnest feature (D53), so nothing the ground holds is stepped over.
const GRAZE_STEP: f32 = 0.05;
/// The most steps a sweep takes; one that runs out stops where it got to.
const MARCHES: usize = 128;

/// An upright capsule: the segment from the bottom sphere's centre `a` to the top's `b`, and
/// its radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
}

impl Capsule {
    /// A body's capsule standing on its feet along its up.
    pub fn standing(feet: Vec3, up: Vec3, radius: f32, height: f32) -> Self {
        Capsule {
            a: feet + up * radius,
            b: feet + up * (height - radius).max(radius),
            radius,
        }
    }

    pub fn moved(self, by: Vec3) -> Self {
        Capsule {
            a: self.a + by,
            b: self.b + by,
            ..self
        }
    }

    /// The nearest point of its axis to a point.
    fn axis_near(&self, at: Vec3) -> Vec3 {
        let ab = self.b - self.a;
        let t = ((at - self.a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
        self.a + ab * t
    }
}

/// A contact: where the other is nearest, the way it pushes the body (from it toward the body,
/// unit), how far apart they are (negative: in it), how fast the other moves, and which body
/// it is when it is one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub at: Vec3,
    pub normal: Vec3,
    pub distance: f32,
    pub velocity: Vec3,
    pub body: Option<u32>,
}

/// The first contact of a sweep: how much of the motion may be made, and what is met there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub fraction: f32,
    pub contact: Contact,
}

/// The mover's query (D53): every source of what stops a body answers it.
pub trait Query {
    /// Every contact within `within` of a capsule's surface, one a face (nearly parallel
    /// faces' merged, as Jolt's hit reduction).
    fn contacts(&self, capsule: &Capsule, within: f32, out: &mut Vec<Contact>);

    /// How much of a motion a capsule makes before it touches something it starts clear of, or
    /// goes deeper into something it starts in; `None` when it makes all of it. What it starts
    /// touching or in and moves along or out of does not stop it.
    fn sweep(&self, capsule: &Capsule, motion: Vec3) -> Option<Hit>;
}

impl<A: Query + ?Sized, B: Query + ?Sized> Query for (&A, &B) {
    fn contacts(&self, capsule: &Capsule, within: f32, out: &mut Vec<Contact>) {
        self.0.contacts(capsule, within, out);
        self.1.contacts(capsule, within, out);
    }

    fn sweep(&self, capsule: &Capsule, motion: Vec3) -> Option<Hit> {
        match (self.0.sweep(capsule, motion), self.1.sweep(capsule, motion)) {
            (Some(a), Some(b)) => Some(if b.fraction < a.fraction { b } else { a }),
            (a, b) => a.or(b),
        }
    }
}

/// A source whose distance to a capsule's surface is known at any place: the sweep is
/// common to all of them, a march by distance.
trait Distance {
    /// The capsule surface's distance to the source, and the contact it is nearest: of two
    /// faces as near (a crease), the one a motion goes into most.
    fn nearest(&self, capsule: &Capsule, motion: Vec3) -> Option<Contact>;
    /// How far the true distance may be from the one answered, as a factor (1 when exact).
    fn steepest(&self) -> f32;
}

/// The march every [`Distance`] sweeps by: steps of the distance over the steepest slope, a
/// graze step past a surface the capsule is touching but not approaching, and a bisection of
/// the last step when it went in.
fn march(source: &impl Distance, capsule: &Capsule, motion: Vec3) -> Option<Hit> {
    let length = motion.length();
    if length < 1e-6 {
        return None;
    }
    let start = source.nearest(capsule, motion)?;
    let limit = start.distance.min(0.0) - TOLERANCE;
    let into = |c: &Contact| c.distance < limit && c.normal.dot(motion) < 0.0;
    let steep = source.steepest().max(1.0);
    let (mut t, mut last) = (0.0_f32, start);
    for _ in 0..MARCHES {
        if t >= 1.0 {
            return None;
        }
        // Nothing is nearer than the distance; a surface it grazes is stepped along, and one
        // stepped into is found by the bisection.
        let step = ((last.distance - limit) / steep).max(GRAZE_STEP);
        let next = (t + step / length).min(1.0);
        let at = source.nearest(&capsule.moved(motion * next), motion)?;
        if into(&at) {
            // The surface lies within the last step: bisect it.
            let (mut lo, mut hi, mut met) = (t, next, at);
            while (hi - lo) * length > TOLERANCE * 0.5 {
                let mid = (lo + hi) * 0.5;
                let c = source.nearest(&capsule.moved(motion * mid), motion)?;
                if into(&c) {
                    (hi, met) = (mid, c);
                } else {
                    lo = mid;
                }
            }
            return Some(Hit {
                fraction: lo,
                contact: met,
            });
        }
        (t, last) = (next, at);
    }
    Some(Hit {
        fraction: t,
        contact: last,
    })
}

/// Ground as a signed distance field: the distance from a point to the solid (negative inside)
/// and the field's steepest slope (`Field::steepest`, 1 for an exact field).
pub struct Sdf<F> {
    pub distance: F,
    pub steepest: f32,
}

/// The step of the field's gradient, m.
const GRADIENT: f32 = 1.0e-2;

/// How far an exact field's central difference may fall below its unit slope before a point is
/// called a crease: as near two faces as each other (a wall meeting the floor), where the
/// difference blends the two into a slope that is neither, which a body would ride up.
const CREASE: f32 = 0.9;

impl<F: Fn(Vec3) -> f32> Sdf<F> {
    /// The field's gradient at a point by central differences (unit for an exact field, less
    /// at a crease).
    fn gradient(&self, at: Vec3) -> Vec3 {
        let d = |axis: Vec3| {
            (self.distance)(at + axis * GRADIENT) - (self.distance)(at - axis * GRADIENT)
        };
        Vec3::new(d(Vec3::X), d(Vec3::Y), d(Vec3::Z)) / (2.0 * GRADIENT)
    }

    /// The faces nearest a point `d` from the field: its gradient's way, or at a crease each of
    /// the one-sided differences (ahead and behind, which there are each one face's) that is a
    /// face's: the field falls along it at an exact field's unit rate. Ours, not Jolt's: a mesh
    /// gives a contact a triangle, a field one distance a point.
    fn faces(&self, at: Vec3, d: f32) -> [Option<Vec3>; 2] {
        let gradient = self.gradient(at);
        if gradient.length() >= CREASE || d <= 0.0 {
            return [gradient.try_normalize(), None];
        }
        let here = (self.distance)(at);
        let side = |sign: f32| {
            let d = |axis: Vec3| sign * ((self.distance)(at + axis * (sign * GRADIENT)) - here);
            Vec3::new(d(Vec3::X), d(Vec3::Y), d(Vec3::Z)).try_normalize()
        };
        let face = |normal: Option<Vec3>| {
            normal.filter(|n| {
                let half = (self.distance)(at - *n * (d * 0.5));
                (half - d * 0.5).abs() < TOLERANCE
            })
        };
        match [face(side(1.0)), face(side(-1.0))] {
            [None, None] => [gradient.try_normalize(), None],
            faces => faces,
        }
    }

    /// The field along the axis: the samples, no more than half a radius apart.
    fn samples(&self, capsule: &Capsule) -> (usize, impl Fn(f32) -> Vec3 + '_) {
        let ab = capsule.b - capsule.a;
        let n = ((ab.length() / (capsule.radius * 0.5)).ceil() as usize).max(1) + 1;
        let a = capsule.a;
        (n, move |t: f32| a + ab * t)
    }

    /// The least of the field near an axis sample `(t, d)`, by golden section over its
    /// neighbours, never more than the sample's own: the field along an axis by a wall is flat
    /// and only a floor's corner below it falls away, so a millimetre's noise on the flat can
    /// lead the section away from the floor, and the floor's contact would be lost.
    fn refine(
        &self,
        along: &impl Fn(f32) -> Vec3,
        (t, d): (f32, f32),
        lo: f32,
        hi: f32,
    ) -> (Vec3, f32) {
        const R: f32 = 0.618_034;
        let f = |t: f32| (self.distance)(along(t));
        let (mut lo, mut hi) = (lo, hi);
        let (mut x1, mut x2) = (hi - R * (hi - lo), lo + R * (hi - lo));
        let (mut f1, mut f2) = (f(x1), f(x2));
        for _ in 0..8 {
            if f1 <= f2 {
                hi = x2;
                (x2, f2) = (x1, f1);
                x1 = hi - R * (hi - lo);
                f1 = f(x1);
            } else {
                lo = x1;
                (x1, f1) = (x2, f2);
                x2 = lo + R * (hi - lo);
                f2 = f(x2);
            }
        }
        let ends = [(t, d), (lo, f(lo)), (hi, f(hi)), (x1, f1), (x2, f2)];
        let (t, d) = ends
            .into_iter()
            .fold((lo, f32::INFINITY), |m, e| if e.1 < m.1 { e } else { m });
        (along(t), d)
    }

    /// A contact through the surface point nearest an axis point `d` from the field along a
    /// face's normal: the normal's foot moved onto the surface by one Newton step (exact for an
    /// exact field), and the normal taken from that point to the axis. A gradient by
    /// differences carries the field's noise over its step (in f32 a tenth of a millimetre at
    /// a world's radius over 4 mm, a few percent), which put an edge's point millimetres off;
    /// the step leaves it second order.
    fn contact(&self, capsule: &Capsule, axis: Vec3, d: f32, normal: Vec3) -> Contact {
        let foot = axis - normal * d;
        let (off, slope) = ((self.distance)(foot), self.gradient(foot));
        let (mut at, mut normal) = (foot, normal);
        if slope.length_squared() > 0.25 && d.abs() > TOLERANCE {
            let on = foot - slope * (off / slope.length_squared());
            let toward = (axis - on) * d.signum();
            // Kept where the point is as far as the field says: where a field only bounds the
            // distance (a hollow's rim), the gradient's own foot and way.
            if (toward.length() - d.abs()).abs() < TOLERANCE
                && let Some(n) = toward.try_normalize()
            {
                (at, normal) = (on, n);
            }
        }
        Contact {
            at,
            normal,
            distance: d - capsule.radius,
            velocity: Vec3::ZERO,
            body: None,
        }
    }
}

impl<F: Fn(Vec3) -> f32> Distance for Sdf<F> {
    fn nearest(&self, capsule: &Capsule, motion: Vec3) -> Option<Contact> {
        let (n, along) = self.samples(capsule);
        let step = 1.0 / (n - 1) as f32;
        let (i, least) = (0..n)
            .map(|i| (i, (self.distance)(along(i as f32 * step))))
            .fold((0, f32::INFINITY), |m, e| if e.1 < m.1 { e } else { m });
        let t = i as f32 * step;
        let (axis, d) = self.refine(&along, (t, least), (t - step).max(0.0), (t + step).min(1.0));
        let normal = self
            .faces(axis, d)
            .into_iter()
            .flatten()
            .min_by(|a, b| a.dot(motion).total_cmp(&b.dot(motion)))
            .unwrap_or(Vec3::ZERO);
        Some(self.contact(capsule, axis, d, normal))
    }

    fn steepest(&self) -> f32 {
        self.steepest
    }
}

impl<F: Fn(Vec3) -> f32> Query for Sdf<F> {
    fn contacts(&self, capsule: &Capsule, within: f32, out: &mut Vec<Contact>) {
        let (n, along) = self.samples(capsule);
        let step = 1.0 / (n - 1) as f32;
        let reach = capsule.radius + within;
        let start = out.len();
        for i in 0..n {
            let t = i as f32 * step;
            let d = (self.distance)(along(t));
            if d >= reach {
                continue;
            }
            let (axis, least) =
                self.refine(&along, (t, d), (t - step).max(0.0), (t + step).min(1.0));
            if least - capsule.radius >= within {
                continue;
            }
            for normal in self.faces(axis, least).into_iter().flatten() {
                let found = self.contact(capsule, axis, least, normal);
                // One contact a face: the nearest sample of it.
                let same = out[start..]
                    .iter()
                    .position(|c: &Contact| c.normal.dot(found.normal) > 0.999);
                match same {
                    Some(k) if found.distance < out[start + k].distance => out[start + k] = found,
                    Some(_) => {}
                    None => out.push(found),
                }
            }
        }
    }

    fn sweep(&self, capsule: &Capsule, motion: Vec3) -> Option<Hit> {
        march(self, capsule, motion)
    }
}

/// Other bodies' pushboxes: each an ID, its capsule and its velocity. A body's own is
/// skipped by `skip`.
pub struct Capsules<'a> {
    pub bodies: &'a [(u32, Capsule, Vec3)],
    pub skip: u32,
}

impl Capsules<'_> {
    fn each(&self, capsule: &Capsule) -> impl Iterator<Item = Contact> + '_ {
        let capsule = *capsule;
        self.bodies
            .iter()
            .filter(move |b| b.0 != self.skip)
            .map(move |(id, other, velocity)| between(&capsule, other, *id, *velocity))
    }
}

/// The contact between two capsules: their axes' nearest points, less both radii.
fn between(capsule: &Capsule, other: &Capsule, id: u32, velocity: Vec3) -> Contact {
    let (mine, theirs) = nearest_points(capsule, other);
    let apart = mine - theirs;
    let length = apart.length();
    let normal = if length > 1e-6 {
        apart / length
    } else {
        // Coincident axes: apart along the first level way.
        let up = (capsule.b - capsule.a).normalize_or(Vec3::Y);
        up.any_orthonormal_vector()
    };
    Contact {
        at: theirs + normal * other.radius,
        normal,
        distance: length - capsule.radius - other.radius,
        velocity,
        body: Some(id),
    }
}

/// The nearest points of two capsules' axes (Ericson, Real-Time Collision Detection 5.1.9).
fn nearest_points(p: &Capsule, q: &Capsule) -> (Vec3, Vec3) {
    let (d1, d2, r) = (p.b - p.a, q.b - q.a, p.a - q.a);
    let (a, e, f) = (d1.length_squared(), d2.length_squared(), d2.dot(r));
    let (s, t) = if a <= 1e-12 && e <= 1e-12 {
        (0.0, 0.0)
    } else if a <= 1e-12 {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= 1e-12 {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut s = if denom > 1e-12 {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
            (s, t)
        }
    };
    (p.a + d1 * s, q.a + d2 * t)
}

/// One body's capsule as a [`Distance`], for the march.
struct One<'a>(&'a Capsule, u32, Vec3);

impl Distance for One<'_> {
    fn nearest(&self, capsule: &Capsule, _: Vec3) -> Option<Contact> {
        Some(between(capsule, self.0, self.1, self.2))
    }
    fn steepest(&self) -> f32 {
        1.0
    }
}

impl Query for Capsules<'_> {
    fn contacts(&self, capsule: &Capsule, within: f32, out: &mut Vec<Contact>) {
        out.extend(self.each(capsule).filter(|c| c.distance < within));
    }

    fn sweep(&self, capsule: &Capsule, motion: Vec3) -> Option<Hit> {
        // Only bodies the swept capsule can reach.
        let reach = motion.length() + capsule.radius;
        self.bodies
            .iter()
            .filter(|b| b.0 != self.skip)
            .filter(|(_, other, _)| {
                let mid = (capsule.a + capsule.b) * 0.5 + motion * 0.5;
                let near = other.axis_near(mid);
                (near - capsule.axis_near(near)).length()
                    < reach + other.radius + (capsule.b - capsule.a).length()
            })
            .filter_map(|(id, other, v)| march(&One(other, *id, *v), capsule, motion))
            .fold(None, |best: Option<Hit>, hit| match best {
                Some(b) if b.fraction <= hit.fraction => Some(b),
                _ => Some(hit),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn floor(at: Vec3) -> f32 {
        at.y
    }

    fn man(feet: Vec3) -> Capsule {
        Capsule::standing(feet, Vec3::Y, 0.3, 1.8)
    }

    #[test]
    fn a_capsule_over_a_floor_has_one_contact_as_far_as_its_feet_are() {
        let ground = Sdf {
            distance: floor,
            steepest: 1.0,
        };
        let mut out = Vec::new();
        ground.contacts(&man(Vec3::new(0.0, 0.05, 0.0)), 0.1, &mut out);
        assert_eq!(out.len(), 1);
        assert!((out[0].distance - 0.05).abs() < 1e-4);
        assert!(out[0].normal.dot(Vec3::Y) > 0.999);
    }

    /// Where these fixtures stand: 2 km from the origin, a world's radius, where f32 keeps a
    /// tenth of a millimetre and a field's differences carry that noise.
    const FAR: Vec3 = Vec3::new(0.0, 2000.0, 0.0);

    /// The floor and a wall at x = 1, met in a crease.
    fn crease(at: Vec3) -> f32 {
        let p = at - FAR;
        p.y.min(1.0 - p.x)
    }

    /// The floor and a step up 0.55 m at x = 1 (exact outside).
    fn step(at: Vec3) -> f32 {
        let p = at - FAR;
        let q = Vec2::new(1.0 - p.x, p.y - 0.55);
        let block = q.max(Vec2::ZERO).length() + q.max_element().min(0.0);
        block.min(p.y)
    }

    #[test]
    fn a_body_in_a_crease_meets_the_floor_and_the_wall_and_no_slope_between() {
        let ground = Sdf {
            distance: crease,
            steepest: 1.0,
        };
        // Touching both, and sunk a few millimetres into the floor against the wall.
        for sunk in [0.0, 0.004] {
            let feet = FAR + Vec3::new(0.7, -sunk, 0.0);
            let mut out = Vec::new();
            ground.contacts(&man(feet), 0.1, &mut out);
            let floor = out.iter().find(|c| c.normal.y > 0.99).expect("the floor");
            assert!((floor.distance + sunk).abs() < 1e-3, "{out:?}");
            assert!(out.iter().any(|c| c.normal.x < -0.99), "the wall: {out:?}");
            let blended = |c: &&Contact| c.normal.y < 0.99 && c.normal.x > -0.99;
            assert!(out.iter().find(blended).is_none(), "{out:?}");
        }
        // Swept down the wall, it stays on the floor; along the floor, it stops at the wall.
        let hit = ground.sweep(&man(FAR + Vec3::new(0.7, 0.0, 0.0)), Vec3::NEG_Y * 0.5);
        assert!(hit.is_none_or(|h| h.fraction * 0.5 < 2e-3), "{hit:?}");
        let hit = ground
            .sweep(&man(FAR + Vec3::new(0.6, 0.0, 0.0)), Vec3::X * 0.3)
            .expect("the wall");
        assert!(hit.contact.normal.x < -0.99, "{hit:?}");
        assert!((hit.fraction * 0.3 - 0.1).abs() < 2e-3, "{hit:?}");
    }

    #[test]
    fn an_edge_contact_lies_on_the_edge_and_faces_from_it() {
        let ground = Sdf {
            distance: step,
            steepest: 1.0,
        };
        // The bottom sphere over the step's edge, its centre ahead of and above it.
        let feet = FAR + Vec3::new(0.85, 0.3, 0.0);
        let mut out = Vec::new();
        ground.contacts(&man(feet), 0.1, &mut out);
        let edge = out.iter().find(|c| c.normal.x < -0.1).expect("the edge");
        let corner = FAR + Vec3::new(1.0, 0.55, 0.0);
        assert!((edge.at - corner).length() < 2e-4, "{edge:?}");
        let way = (feet + Vec3::Y * 0.3 - corner).normalize();
        assert!(edge.normal.dot(way) > 0.99999, "{edge:?}");
    }

    #[test]
    fn a_wall_beside_a_body_on_the_floor_is_a_second_contact() {
        let ground = Sdf {
            distance: |at: Vec3| at.y.min(1.0 - at.x),
            steepest: 1.0,
        };
        let mut out = Vec::new();
        ground.contacts(&man(Vec3::new(0.65, 0.0, 0.0)), 0.1, &mut out);
        assert_eq!(out.len(), 2, "{out:?}");
        let wall = out.iter().find(|c| c.normal.x < -0.9).expect("the wall");
        assert!((wall.distance - 0.05).abs() < 1e-3);
    }

    #[test]
    fn a_sweep_at_a_thin_wall_stops_at_it_at_any_speed() {
        // A wall a voxel thick at x 3 to 3.5, and the floor.
        let ground = Sdf {
            distance: |at: Vec3| {
                let q = (at.x - 3.25).abs() - 0.25;
                at.y.min(q)
            },
            steepest: 1.0,
        };
        for speed in [5.0, 20.0, 60.0, 600.0] {
            let motion = Vec3::X * speed;
            let hit = ground.sweep(&man(Vec3::ZERO), motion).expect("stopped");
            let x = speed * hit.fraction;
            assert!(x < 2.7 + 1e-3 && x > 2.69, "{speed}: {x}");
            assert!(hit.contact.normal.x < -0.9);
        }
        // Along the floor it touches, nothing stops it.
        assert!(ground.sweep(&man(Vec3::ZERO), Vec3::Z * 2.0).is_none());
    }

    #[test]
    fn a_body_swept_at_another_stops_at_its_pushbox() {
        let other = man(Vec3::new(3.0, 0.0, 0.0));
        let bodies = [(1, other, Vec3::ZERO)];
        let source = Capsules {
            bodies: &bodies,
            skip: 0,
        };
        let hit = source
            .sweep(&man(Vec3::ZERO), Vec3::X * 60.0)
            .expect("stopped");
        assert!((60.0 * hit.fraction - 2.4).abs() < 2e-3);
        assert_eq!(hit.contact.body, Some(1));
    }
}
