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
//!
//! A capsule is asked in a frame of its own: its points are offsets from an `origin` held in
//! f64, and a contact's point is in the same frame. Positions on a world 30 km across are
//! 2 mm apart in f32, more than the query's millimetre; offsets of a few metres keep a
//! micrometre. Jolt's large worlds do the same (`JPH_DOUBLE_PRECISION`: a body's position is
//! a double, its shape queries run in floats relative to a base offset, Docs/BigWorlds.md).

use super::grid::{Gathered, Grid};
use glam::{DVec3, Vec3};
use std::borrow::Cow;

/// How far a contact may sink before a sweep calls it a hit, m (Jolt's collision tolerance).
pub const TOLERANCE: f32 = 1.0e-3;
/// The longest a sweep steps past a surface it is touching or leaving, m: a tenth of a voxel,
/// the ground's thinnest feature (D53), so nothing the ground holds is stepped over.
const GRAZE_STEP: f32 = 0.05;
/// The most steps a sweep takes; one that runs out stops where it got to.
const MARCHES: usize = 128;

/// An upright capsule: the segment from the bottom sphere's centre `a` to the top's `b`, and
/// its radius, `a` and `b` offsets from `origin` (the world's centre unless framed).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
    pub origin: DVec3,
}

impl Capsule {
    /// A body's capsule standing on its feet along its up.
    pub fn standing(feet: Vec3, up: Vec3, radius: f32, height: f32) -> Self {
        Capsule {
            a: feet + up * radius,
            b: feet + up * (height - radius).max(radius),
            radius,
            origin: DVec3::ZERO,
        }
    }

    /// The same capsule with its points offsets from an origin.
    pub fn at(self, origin: DVec3) -> Self {
        Capsule { origin, ..self }
    }

    /// The same capsule in another's frame: its points offsets from that origin.
    pub fn framed(self, origin: DVec3) -> Self {
        let by = (self.origin - origin).as_vec3();
        Capsule {
            origin,
            ..self.moved(by)
        }
    }

    /// A point of its frame in the world, in f64.
    pub fn world(&self, at: Vec3) -> DVec3 {
        self.origin + at.as_dvec3()
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

/// A contact: where the other is nearest (in the asking capsule's frame), the way it pushes
/// the body (from it toward the body, unit), how far apart they are (negative: in it), how
/// fast the other moves, and which body it is when it is one.
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

    /// Whether a body whose feet stand at `feet` may step up onto what it touches at `at`,
    /// world points (Unreal's `CanCharacterStepUpOn`): yes, unless a source says otherwise, as
    /// a thing of flora taller than the step does (`shapes`).
    fn steps_onto(&self, at: DVec3, feet: DVec3, step: f32) -> bool {
        let _ = (at, feet, step);
        true
    }
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

    fn steps_onto(&self, at: DVec3, feet: DVec3, step: f32) -> bool {
        self.0.steps_onto(at, feet, step) && self.1.steps_onto(at, feet, step)
    }
}

/// A source whose distance to a capsule's surface is known at any place: the sweep is
/// common to all of them, a march by distance.
trait Distance {
    /// The capsule surface's distance to the source, and the contact it is nearest: of two
    /// faces as near (a crease), the one a motion goes into most.
    fn nearest(&self, capsule: &Capsule, motion: Vec3) -> Option<Contact>;
    /// The distance [`Distance::nearest`]'s contact has, alone: what a march step needs, the
    /// contact's way only where the step went in.
    fn distance(&self, capsule: &Capsule) -> Option<f32> {
        self.nearest(capsule, Vec3::ZERO).map(|c| c.distance)
    }
    /// How far the true distance may be from the one answered, as a factor (1 when exact).
    fn steepest(&self) -> f32;
}

/// The march every [`Distance`] sweeps by: steps of the distance over the steepest slope, a
/// graze step past a surface the capsule is touching but not approaching, and a bisection of
/// the last step when it went in. A step asks the distance alone, and the contact (its way, of
/// a crease's faces) only where the step went deeper than it started: the same hit as asking
/// the contact at every step, for a fraction of the field's calls.
fn march(source: &impl Distance, capsule: &Capsule, motion: Vec3) -> Option<Hit> {
    let length = motion.length();
    if length < 1e-6 {
        return None;
    }
    let at = |t: f32| capsule.moved(motion * t);
    let contact = |t: f32| source.nearest(&at(t), motion);
    let distance = |t: f32| source.distance(&at(t));
    let start = distance(0.0)?;
    let limit = start.min(0.0) - TOLERANCE;
    // In, and going in: a face the motion leaves or runs along is not a hit. A step that
    // ended inside from a place approaching a face went in through it, whatever face is
    // nearest inside: a riser a centimetre beside a step's top is nearer than the top a
    // graze step under it, and its level normal would let the march run through the step.
    let into = |c: &Contact| c.normal.dot(motion) < 0.0;
    let steep = source.steepest().max(1.0);
    // Where the march is, its distance, and its contact once asked.
    let (mut t, mut d, mut last) = (0.0_f32, start, None::<Contact>);
    for _ in 0..MARCHES {
        if t >= 1.0 {
            return None;
        }
        // Nothing is nearer than the distance; a surface it grazes is stepped along, and one
        // stepped into is found by the bisection.
        let step = ((d - limit) / steep).max(GRAZE_STEP);
        let next = (t + step / length).min(1.0);
        let ahead = distance(next)?;
        if ahead >= limit {
            (t, d, last) = (next, ahead, None);
            continue;
        }
        let from = match last {
            Some(c) => c,
            None => contact(t)?,
        };
        let met = contact(next)?;
        let from_in = into(&from);
        if !(from_in || into(&met)) {
            (t, d, last) = (next, ahead, Some(met));
            continue;
        }
        // The surface lies within the last step: bisect it. Approaching a face where the step
        // began, deeper is in, whichever way the face met there points.
        let (mut lo, mut hi, mut met) = (t, next, Some(met));
        while (hi - lo) * length > TOLERANCE * 0.5 {
            let mid = (lo + hi) * 0.5;
            let went_in = if distance(mid)? >= limit {
                None
            } else if from_in {
                Some(None)
            } else {
                let c = contact(mid)?;
                into(&c).then_some(Some(c))
            };
            match went_in {
                Some(c) => (hi, met) = (mid, c),
                None => lo = mid,
            }
        }
        let contact = match met {
            Some(c) => c,
            None => contact(hi)?,
        };
        return Some(Hit {
            fraction: lo,
            contact,
        });
    }
    let contact = match last {
        Some(c) => c,
        None => contact(t)?,
    };
    Some(Hit {
        fraction: t,
        contact,
    })
}

/// Ground as a signed distance field: the distance from a world point, in f64, to the solid
/// (negative inside) and the field's steepest slope (`Field::steepest`, 1 for an exact field).
/// The point is a capsule's origin and an offset added in f64, so a field that works it out
/// in f64 (a world's `|p| - radius`) answers to a micrometre at any radius.
pub struct Sdf<F> {
    pub distance: F,
    pub steepest: f32,
}

/// A field as a capsule's frame sees it: its distance at an offset from the frame's origin.
struct On<'s, F> {
    distance: &'s F,
    origin: DVec3,
}

impl<F: Fn(DVec3) -> f32> Sdf<F> {
    fn on(&self, capsule: &Capsule) -> On<'_, F> {
        On {
            distance: &self.distance,
            origin: capsule.origin,
        }
    }
}

/// The step of the field's gradient, m.
const GRADIENT: f32 = 1.0e-2;

/// How far an exact field's central difference may fall below its unit slope before a point is
/// called a crease: as near two faces as each other (a wall meeting the floor), where the
/// difference blends the two into a slope that is neither, which a body would ride up.
const CREASE: f32 = 0.9;

/// How far an axis's two one-sided differences may part, m, before one of them is suspected of
/// reaching over an edge, and how far a side may bend from a line and still be one, near the
/// frame's origin: a fiftieth of a millimetre, a 0.06 deg tilt over the step. Further from it
/// it is the f32 noise of the offset (eight of its ulps, [`kink`]): a side chosen by noise
/// would tilt a floor more than the centred difference does. The field is asked in f64 at the
/// origin plus the offset, so the world's radius adds none.
const KINK: f32 = TOLERANCE * 0.02;

/// [`KINK`] at a point, an offset from a capsule's origin: the least part of the differences
/// that is not the offset's f32 noise (the field is asked in f64 at the origin plus the offset,
/// so only the offset's own rounding is noise).
fn kink(at: Vec3) -> f32 {
    KINK.max(at.abs().max_element() * f32::EPSILON * 8.0)
}

impl<F: Fn(DVec3) -> f32> On<'_, F> {
    fn at(&self, offset: Vec3) -> f32 {
        (self.distance)(self.origin + offset.as_dvec3())
    }

    /// The field's gradient at a point `here` from the field by differences (unit for an exact
    /// field, less at a crease): central along each axis, or the one-sided difference of the
    /// side the field is straight along where the other bends within the step. Just past a
    /// box's convex edge (a step climbed a tick ago, its edge 8 mm behind) the centred step
    /// reaches over the edge, where the field rounds it, and tilts a floor's normal 3-5 deg,
    /// enough to throw the body off its floor; the side away from the edge is the top's alone.
    /// The ENO choice of stencil (Harten, Engquist, Osher and Chakravarthy 1987; Osher and
    /// Fedkiw, Level Set Methods and Dynamic Implicit Surfaces, 2003, 3.3): of the two, the
    /// stencil the field is smoother across. Only where the field rounds an edge (bends convex:
    /// the slope ahead exceeds the slope behind); a crease (bends concave, where two faces' fields
    /// meet) keeps the centred blend, for `faces` to tell its faces apart. Jolt and Unreal take a
    /// floor's normal from the face hit (`mSurfaceNormal`, `FindFloor`'s line trace
    /// `ImpactNormal`); a field has no face, so its straight side stands for one.
    fn gradient(&self, at: Vec3, here: f32) -> Vec3 {
        let f = |p: Vec3| self.at(p);
        let kink = kink(at);
        let d = |axis: Vec3| {
            let (ahead, behind) = (f(at + axis * GRADIENT), f(at - axis * GRADIENT));
            let (fore, back) = (ahead - here, here - behind);
            let centred = (fore + back) * 0.5;
            if fore - back <= kink {
                return centred;
            }
            // How far each side's middle lies over the line through its ends (under: convex).
            let fore_bend = f(at + axis * (GRADIENT * 0.5)) - here - fore * 0.5;
            let back_bend = f(at - axis * (GRADIENT * 0.5)) - here + back * 0.5;
            if back_bend < -(fore_bend.abs() * 4.0 + kink) {
                fore
            } else if fore_bend < -(back_bend.abs() * 4.0 + kink) {
                back
            } else {
                centred
            }
        };
        Vec3::new(d(Vec3::X), d(Vec3::Y), d(Vec3::Z)) / GRADIENT
    }

    /// The faces nearest a point `d` from the field: its gradient's way, or at a crease each of
    /// the one-sided differences (ahead and behind, which there are each one face's) that is a
    /// face's: the field falls along it at an exact field's unit rate. Ours, not Jolt's: a mesh
    /// gives a contact a triangle, a field one distance a point.
    fn faces(&self, at: Vec3, d: f32) -> [Option<Vec3>; 2] {
        let gradient = self.gradient(at, d);
        if gradient.length() >= CREASE || d <= 0.0 {
            return [gradient.try_normalize(), None];
        }
        let here = d;
        let side = |sign: f32| {
            let d = |axis: Vec3| sign * (self.at(at + axis * (sign * GRADIENT)) - here);
            Vec3::new(d(Vec3::X), d(Vec3::Y), d(Vec3::Z)).try_normalize()
        };
        let face = |normal: Option<Vec3>| {
            normal.filter(|n| {
                let half = self.at(at - *n * (d * 0.5));
                (half - d * 0.5).abs() < TOLERANCE
            })
        };
        match [face(side(1.0)), face(side(-1.0))] {
            [None, None] => [gradient.try_normalize(), None],
            faces => faces,
        }
    }

    /// The field along the axis: the samples, no more than half a radius apart, and the
    /// fraction of the axis between them; a sphere's (a probe's) is one.
    fn samples(&self, capsule: &Capsule) -> (usize, f32, impl Fn(f32) -> Vec3 + '_) {
        let ab = capsule.b - capsule.a;
        let n = if ab.length_squared() > 0.0 {
            ((ab.length() / (capsule.radius * 0.5)).ceil() as usize).max(1) + 1
        } else {
            1
        };
        let a = capsule.a;
        (n, 1.0 / (n.max(2) - 1) as f32, move |t: f32| a + ab * t)
    }

    /// The axis point nearest the field, and the field there: the least sample, refined.
    fn least(&self, capsule: &Capsule) -> (Vec3, f32) {
        let (n, step, along) = self.samples(capsule);
        if n == 1 {
            return (capsule.a, self.at(capsule.a));
        }
        let (i, least) = (0..n)
            .map(|i| (i, self.at(along(i as f32 * step))))
            .fold((0, f32::INFINITY), |m, e| if e.1 < m.1 { e } else { m });
        let t = i as f32 * step;
        self.refine(&along, (t, least), (t - step).max(0.0), (t + step).min(1.0))
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
        let f = |t: f32| self.at(along(t));
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
        let off = self.at(foot);
        let slope = self.gradient(foot, off);
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

impl<F: Fn(DVec3) -> f32> Distance for Sdf<F> {
    fn nearest(&self, capsule: &Capsule, motion: Vec3) -> Option<Contact> {
        let on = self.on(capsule);
        let (axis, d) = on.least(capsule);
        let normal = on
            .faces(axis, d)
            .into_iter()
            .flatten()
            .min_by(|a, b| a.dot(motion).total_cmp(&b.dot(motion)))
            .unwrap_or(Vec3::ZERO);
        Some(on.contact(capsule, axis, d, normal))
    }

    fn distance(&self, capsule: &Capsule) -> Option<f32> {
        Some(self.on(capsule).least(capsule).1 - capsule.radius)
    }

    fn steepest(&self) -> f32 {
        self.steepest
    }
}

impl<F: Fn(DVec3) -> f32> Query for Sdf<F> {
    fn contacts(&self, capsule: &Capsule, within: f32, out: &mut Vec<Contact>) {
        let on = self.on(capsule);
        let (n, step, along) = on.samples(capsule);
        let reach = capsule.radius + within;
        let start = out.len();
        for i in 0..n {
            let t = i as f32 * step;
            let d = on.at(along(t));
            if d >= reach {
                continue;
            }
            let (axis, least) = on.refine(&along, (t, d), (t - step).max(0.0), (t + step).min(1.0));
            if least - capsule.radius >= within {
                continue;
            }
            for normal in on.faces(axis, least).into_iter().flatten() {
                let found = on.contact(capsule, axis, least, normal);
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
/// skipped by `skip`. With `near`, the bodies by the middles of their capsules' axes (the
/// broadphase, [`Grid`]), only bodies in cells near a query are looked at; the answers are the
/// same as looking at every one, in the same order.
pub struct Capsules<'a> {
    pub bodies: &'a [(u32, Capsule, Vec3)],
    pub skip: u32,
    pub near: Option<&'a Grid>,
    /// The bodies `near` gathered around this body for its tick, if it did.
    pub gathered: Option<&'a Gathered>,
}

/// How much farther than its exact reach a query's box is looked up, m: the f32 offsets'
/// rounding.
const NEAR_SLACK: f64 = 0.05;

impl Capsules<'_> {
    /// The bodies whose surfaces may come within `reach` of a capsule's axis, but its own, in
    /// order: those the grid has in a box around it (from those gathered already when the
    /// box is within the gathered one), or every one.
    fn within<'s>(
        &'s self,
        capsule: &Capsule,
        reach: f32,
    ) -> impl Iterator<Item = &'s (u32, Capsule, Vec3)> + 's {
        let chosen = self.near.map(|grid| {
            let (a, b) = (capsule.world(capsule.a), capsule.world(capsule.b));
            let r = DVec3::splat(f64::from(reach.max(0.0) + grid.extent) + NEAR_SLACK);
            let (lo, hi) = (a.min(b) - r, a.max(b) + r);
            let gathered = self.gathered.and_then(|g| g.within(lo, hi));
            let (list, kept) = gathered.map_or_else(
                || {
                    let mut out = Vec::new();
                    grid.near(lo, hi, &mut out);
                    (Cow::Owned(out), None)
                },
                |gathered| (Cow::Borrowed(gathered), Some((grid, lo, hi))),
            );
            (0..list.len())
                .map(move |k| list[k])
                .filter(move |&i| kept.is_none_or(|(grid, lo, hi)| grid.inside(i, lo, hi)))
        });
        let every = chosen.is_none().then(|| self.bodies.iter());
        chosen
            .into_iter()
            .flatten()
            .map(|i| &self.bodies[i as usize])
            .chain(every.into_iter().flatten())
            .filter(|b| b.0 != self.skip)
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
        let c = *capsule;
        out.extend(
            self.within(capsule, within + capsule.radius)
                .map(|(id, other, v)| between(&c, &other.framed(c.origin), *id, *v))
                .filter(|c| c.distance < within),
        );
    }

    fn sweep(&self, capsule: &Capsule, motion: Vec3) -> Option<Hit> {
        // Only bodies the swept capsule can reach.
        let reach = motion.length() + capsule.radius;
        let length = (capsule.b - capsule.a).length();
        self.within(capsule, reach + length)
            .map(|(id, other, v)| (id, other.framed(capsule.origin), v))
            .filter(|(_, other, _)| {
                let mid = (capsule.a + capsule.b) * 0.5 + motion * 0.5;
                let near = other.axis_near(mid);
                (near - capsule.axis_near(near)).length()
                    < reach + other.radius + (capsule.b - capsule.a).length()
            })
            .filter_map(|(id, other, v)| march(&One(&other, *id, *v), capsule, motion))
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

    fn floor(at: DVec3) -> f32 {
        at.y as f32
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
    fn crease(at: DVec3) -> f32 {
        let p = at.as_vec3() - FAR;
        p.y.min(1.0 - p.x)
    }

    /// The floor and a step up 0.55 m at x = 1 (exact outside).
    fn step(at: DVec3) -> f32 {
        let p = at.as_vec3() - FAR;
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
            distance: |at: DVec3| at.as_vec3().y.min(1.0 - at.as_vec3().x),
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
            distance: |at: DVec3| {
                let at = at.as_vec3();
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
            near: None,
            gathered: None,
        };
        let hit = source
            .sweep(&man(Vec3::ZERO), Vec3::X * 60.0)
            .expect("stopped");
        assert!((60.0 * hit.fraction - 2.4).abs() < 2e-3);
        assert_eq!(hit.contact.body, Some(1));
    }

    #[test]
    fn the_grids_bodies_answer_exactly_as_every_body_does() {
        // A crowd far from the origin, some leaning (a round world's ups), a few small.
        let base = DVec3::new(30_000.0, 120.0, -400.0);
        let r = |i: u32, s: u32| terra_gen::hash::unit(terra_gen::hash::of(&[i, s]));
        let bodies: Vec<(u32, Capsule, Vec3)> = (0..400u32)
            .map(|i| {
                let feet = Vec3::new(r(i, 1) * 24.0, r(i, 2) * 3.0, r(i, 3) * 24.0);
                let up = Vec3::new(r(i, 4) - 0.5, 4.0, r(i, 5) - 0.5).normalize();
                let (radius, height) = if i % 7 == 0 { (0.15, 0.6) } else { (0.3, 1.8) };
                let capsule = Capsule::standing(Vec3::ZERO, up, radius, height);
                (i * 3 + 1, capsule.at(base + feet.as_dvec3()), up * r(i, 6))
            })
            .collect();
        let middles = bodies.iter().map(|(_, c, _)| c.world((c.a + c.b) * 0.5));
        let grid = Grid::of(middles, 0.9);
        let (mut touched, mut met) = (0, 0);
        for (k, (id, me, _)) in bodies.iter().enumerate().step_by(5) {
            let all = Capsules {
                bodies: &bodies,
                skip: *id,
                near: None,
                gathered: None,
            };
            let near = Capsules {
                near: Some(&grid),
                ..all
            };
            for within in [-0.1, 0.05, 0.6] {
                let (mut a, mut b) = (Vec::new(), Vec::new());
                all.contacts(me, within, &mut a);
                near.contacts(me, within, &mut b);
                assert_eq!(a, b, "{k} {within}");
                touched += a.len();
            }
            let k = k as u32;
            let motion = Vec3::new(r(k, 7) - 0.5, r(k, 8) - 0.5, r(k, 9) - 0.5) * 6.0;
            let hit = all.sweep(me, motion);
            assert_eq!(hit, near.sweep(me, motion), "{k}");
            met += usize::from(hit.is_some());
        }
        // Not empty answers: many contacts, most sweeps stopped.
        assert!(touched > 200 && met > 40, "{touched} {met}");
    }
}
