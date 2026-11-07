//! What the player walks round (play.rs): every placed model's footprint on the ground, as a
//! row of circles along its long side, the cheap collider top-down games give props (Diablo's
//! and Path of Exile's props block by a footprint on the ground grid, not their mesh). Each is
//! taken once from the bounds of the model's solid meshes in its own frame: an opaque
//! material's (a fern's fronds and a tree's leaf cards are cut out by alpha, so the plants
//! stay walkable) and taller than a kerb (a manhole cover, a decal). A posed body's bounds
//! are its bind pose's, far wider than its feet (kit/contact.rs), so a creature takes one
//! circle at its middle, moving with it. A thin thing (a fallen log) blocks if it is long; one
//! bigger than a cart (a dead tree's branches, a wagon) blocks by its foot, its points half a
//! metre to a metre up (above a tree's roots, below its branches); what is smaller than a stone or still bigger (walls, ground)
//! is left to each layout's `Ground`.

use crate::rim::RimMaterial;
use crate::ruin::fight::Pose;
use bevy::camera::primitives::Aabb;
use bevy::prelude::*;

/// The sizes a footprint is kept for (m): its short side from a stone to a cart's width, its
/// long side to a car's length; a mesh lower than `LOW` is walked over.
const SHORT: (f32, f32) = (0.3, 2.5);
const LONG: f32 = 6.0;
const LOW: f32 = 0.15;
/// The share of its half width a circle takes: models' bounds are wider than their bodies.
const FILL: f32 = 0.85;
/// A posed body's circle (m, at scale 1).
const CREATURE: f32 = 0.45;
/// The player's own radius (m).
pub const BODY: f32 = 0.3;

/// A model's footprint: circles (centre, radius) in its own frame on the ground.
#[derive(Component)]
pub struct Footprint(pub Vec<(Vec2, f32)>);

/// A thin thing's sizes kept too (m): a pole or a fallen log's short side, if it is long.
const THIN: (f32, f32) = (0.12, 0.6);
/// The band above its lowest point a model too big for a footprint is cut at for its foot
/// (m): a dead tree's trunk above its root flare (1.4 to 2.8 m across at the ground, j7's
/// marsh probe; at 0.5 to 1.0 m still 0.9 to 1.5 m, the marsh sinking its trees 0.3 m, j8's)
/// and below its spreading branches.
const FOOT: (f32, f32) = (1.0, 1.5);
/// The share of a foot's points left out on each side of it (`foot`).
const TRIM: f32 = 0.1;

/// The circles along a box on the ground (lo, hi its corners): as many as its long side
/// holds of its short, each its half short side across, end to end.
pub fn circles(lo: Vec2, hi: Vec2) -> Vec<(Vec2, f32)> {
    let size = hi - lo;
    let (short, long) = (size.min_element(), size.max_element());
    let thin = short >= THIN.0 && long >= THIN.1;
    if (short < SHORT.0 && !thin) || short > SHORT.1 || long > LONG {
        return Vec::new();
    }
    let along = if size.x >= size.y { Vec2::X } else { Vec2::Y };
    let n = (long / short).ceil().max(1.0) as usize;
    let r = short / 2.0 * FILL;
    let first = lo + (size - along * long) / 2.0 + along * (long / n as f32) / 2.0;
    (0..n)
        .map(|j| (first + along * (long / n as f32) * j as f32, r))
        .collect()
}

/// Whether a step from `here` to `to` is blocked: into a circle, or deeper into one he
/// already stands in (so a body placed in one can always walk out).
pub fn blocked(circles: &[(Vec2, f32)], here: Vec2, to: Vec2) -> bool {
    circles.iter().any(|&(c, r)| {
        let r = r + BODY;
        to.distance(c) < r && to.distance(c) < here.distance(c)
    })
}

/// The box on the ground of a model's foot: its points (in its frame) in the `FOOT` band over
/// the lowest, between their `TRIM` and `1 - TRIM` quantiles on each side (a stray root or
/// low branch reaching into the band left out), at least `SHORT.0` across (a trunk thinner
/// than a stone still blocks); `None` with none there.
pub fn foot(points: &[Vec3], scale: f32) -> Option<(Vec2, Vec2)> {
    let base = points.iter().map(|p| p.y).reduce(f32::min)?;
    let band = |p: &&Vec3| (FOOT.0..FOOT.1).contains(&((p.y - base) * scale));
    let (mut xs, mut zs): (Vec<f32>, Vec<f32>) =
        points.iter().filter(band).map(|p| (p.x, p.z)).unzip();
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(f32::total_cmp);
    zs.sort_by(f32::total_cmp);
    let at = |v: &[f32], q: f32| v[((v.len() - 1) as f32 * q).round() as usize];
    let lo = Vec2::new(at(&xs, TRIM), at(&zs, TRIM));
    let hi = Vec2::new(at(&xs, 1.0 - TRIM), at(&zs, 1.0 - TRIM));
    let grow = ((Vec2::splat(SHORT.0 / scale) - (hi - lo)) / 2.0).max(Vec2::ZERO);
    Some((lo - grow, hi + grow))
}

/// Whether a mesh's material is solid, by its alpha mode: opaque, not cut out or blended
/// (leaves, fronds, glass); `None` while it loads.
fn solid(alpha: Option<AlphaMode>) -> Option<bool> {
    alpha.map(|a| matches!(a, AlphaMode::Opaque))
}

/// A mesh's alpha mode: its standard material's, or the base's of the rim that replaced it
/// (rim.rs); `None` with neither (a kit shader), `Some(None)` while it loads.
fn alpha(
    m: Option<&MeshMaterial3d<StandardMaterial>>,
    rim: Option<&MeshMaterial3d<RimMaterial>>,
    materials: &Assets<StandardMaterial>,
    rims: Option<&Assets<RimMaterial>>,
) -> Option<Option<AlphaMode>> {
    match (m, rim) {
        (Some(m), _) => Some(materials.get(&m.0).map(|m| m.alpha_mode)),
        (None, Some(r)) => Some(rims.and_then(|a| a.get(&r.0)).map(|m| m.base.alpha_mode)),
        (None, None) => None,
    }
}

/// Once a placed model's meshes are in: its footprint, from its solid meshes' bounds in its
/// frame, or a creature's circle.
#[allow(clippy::type_complexity)]
pub(super) fn take(
    mut commands: Commands,
    roots: Query<
        (Entity, &GlobalTransform, &WorldAssetRoot, Has<Pose>),
        (Without<ChildOf>, Without<Footprint>, Without<super::Hero>),
    >,
    children: Query<&Children>,
    meshes: Query<(
        &Mesh3d,
        Option<&Aabb>,
        &GlobalTransform,
        Option<&MeshMaterial3d<StandardMaterial>>,
        Option<&MeshMaterial3d<RimMaterial>>,
    )>,
    materials: Res<Assets<StandardMaterial>>,
    rims: Option<Res<Assets<RimMaterial>>>,
    data: Res<Assets<Mesh>>,
) {
    'root: for (root, g, asset, posed) in &roots {
        // A posed body's bounds are its bind pose's: one circle at its feet, once it is in.
        if posed {
            if children.iter_descendants(root).any(|e| meshes.contains(e)) {
                debug!(
                    "footprint {:?} at {:.1}: a creature",
                    asset.0.path(),
                    g.translation().xz()
                );
                commands
                    .entity(root)
                    .insert(Footprint(vec![(Vec2::ZERO, CREATURE)]));
            }
            continue;
        }
        let scale = g.scale().x.max(1e-3);
        let to_local = g.affine().inverse();
        let (mut lo, mut hi) = (Vec2::MAX, Vec2::MIN);
        let mut any = false;
        let mut parts = Vec::new();
        for e in children.iter_descendants(root) {
            let Ok((mesh, b, mg, m, rim)) = meshes.get(e) else {
                continue;
            };
            any = true;
            // Its bounds and material not in yet: next frame. A stone's or log's material
            // swapped for its rim (rim.rs) is its base's; one of the kit's own (foliage, bark
            // shaders) is no prop's solid body.
            let Some(b) = b else { continue 'root };
            match alpha(m, rim, &materials, rims.as_deref()).map(solid) {
                Some(None) => continue 'root,
                None | Some(Some(false)) => continue,
                Some(Some(true)) => {}
            }
            let (mut blo, mut bhi) = (Vec3::MAX, Vec3::MIN);
            for k in 0..8 {
                let s = Vec3::new(
                    if k & 1 == 0 { -1.0 } else { 1.0 },
                    if k & 2 == 0 { -1.0 } else { 1.0 },
                    if k & 4 == 0 { -1.0 } else { 1.0 },
                );
                let corner = Vec3::from(b.center) + Vec3::from(b.half_extents) * s;
                let p = to_local.transform_point3(mg.transform_point(corner));
                (blo, bhi) = (blo.min(p), bhi.max(p));
            }
            if (bhi.y - blo.y) * scale < LOW {
                continue;
            }
            (lo, hi) = (lo.min(blo.xz()), hi.max(bhi.xz()));
            parts.push((mesh.0.id(), to_local * mg.affine()));
        }
        if !any {
            continue; // Its meshes are not in yet.
        }
        // In the world's metres, for the sizes, then back to its own frame.
        let ring = |lo: Vec2, hi: Vec2| -> Vec<(Vec2, f32)> {
            circles(lo * scale, hi * scale)
                .into_iter()
                .map(|(p, r)| (p / scale, r / scale))
                .collect()
        };
        let size = (hi - lo).max(Vec2::ZERO) * scale;
        let mut c = if lo.x > hi.x {
            Vec::new()
        } else {
            ring(lo, hi)
        };
        // Too big for one (a dead tree's spread branches): its foot, from its points.
        let big = size.max_element() > LONG || size.min_element() > SHORT.1;
        let mut by_foot = String::new();
        if c.is_empty() && big {
            let points: Vec<Vec3> = parts
                .iter()
                .filter_map(|(id, to)| Some((data.get(*id)?, to)))
                .filter_map(|(m, to)| {
                    let p = m.try_attribute(Mesh::ATTRIBUTE_POSITION).ok()?;
                    Some(
                        p.as_float3()?
                            .iter()
                            .map(|&p| to.transform_point3(p.into())),
                    )
                })
                .flatten()
                .collect();
            if let Some((flo, fhi)) = foot(&points, scale) {
                c = ring(flo, fhi);
                let (s, mid) = ((fhi - flo) * scale, (flo + fhi) / 2.0 * scale);
                by_foot = format!(
                    ", by its foot {:.2} x {:.2} m at {mid:.2} of {} points",
                    s.x,
                    s.y,
                    points.len()
                );
            }
        }
        debug!(
            "footprint {:?} at {:.1}: {} circles, {:.2} x {:.2} m{by_foot}",
            asset.0.path(),
            g.translation().xz(),
            c.len(),
            size.x,
            size.y,
        );
        commands.entity(root).insert(Footprint(c));
    }
}

/// Every footprint's circles in the world.
pub fn world<F: bevy::ecs::query::QueryFilter>(
    footprints: &Query<(&Footprint, &GlobalTransform), F>,
) -> Vec<(Vec2, f32)> {
    let mut out = Vec::new();
    for (f, g) in footprints {
        let s = g.scale().x;
        out.extend(
            f.0.iter()
                .map(|&(p, r)| (g.transform_point(Vec3::new(p.x, 0.0, p.y)).xz(), r * s)),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A car's box (1.8 x 4.4 m) is a row of circles along its length that covers its middle
    /// and both ends and nothing a stride beside it; a pebble and a house take none.
    #[test]
    fn a_box_is_circles_along_its_long_side() {
        let c = circles(Vec2::new(-0.9, -2.2), Vec2::new(0.9, 2.2));
        assert_eq!(c.len(), 3);
        let inside = |p: Vec2| c.iter().any(|&(q, r)| p.distance(q) < r);
        for p in [Vec2::ZERO, Vec2::new(0.0, 1.9), Vec2::new(0.0, -1.9)] {
            assert!(inside(p), "{p} not covered");
        }
        assert!(!inside(Vec2::new(1.3, 0.0)), "beside it");
        assert!(circles(Vec2::ZERO, Vec2::splat(0.1)).is_empty(), "a pebble");
        assert!(
            circles(Vec2::ZERO, Vec2::new(8.0, 12.0)).is_empty(),
            "a house"
        );
    }

    /// A step into a prop is blocked, past it is not, and out of one he stands in is free.
    #[test]
    fn steps_into_a_footprint_are_blocked_and_out_of_one_free() {
        let c = [(Vec2::ZERO, 0.5)];
        let here = Vec2::new(-1.0, 0.0);
        assert!(blocked(&c, here, Vec2::new(-0.7, 0.0)), "into it");
        assert!(!blocked(&c, here, Vec2::new(-1.0, 1.0)), "past it");
        let stuck = Vec2::new(0.2, 0.0);
        assert!(!blocked(&c, stuck, Vec2::new(0.3, 0.0)), "out of it");
        assert!(blocked(&c, stuck, Vec2::new(0.1, 0.0)), "deeper in");
    }

    /// Only an opaque material is solid: a fern's cut-out fronds and a blended pane are
    /// walked through, and a material not yet loaded waits.
    #[test]
    fn only_opaque_meshes_block() {
        let mut mats = Assets::<StandardMaterial>::default();
        let of = |mats: &mut Assets<StandardMaterial>, a: AlphaMode| {
            MeshMaterial3d(mats.add(StandardMaterial {
                alpha_mode: a,
                ..default()
            }))
        };
        let (opaque, mask, blend) = (
            of(&mut mats, AlphaMode::Opaque),
            of(&mut mats, AlphaMode::Mask(0.5)),
            of(&mut mats, AlphaMode::Blend),
        );
        let missing = of(&mut mats, AlphaMode::Opaque);
        mats.remove(&missing.0);
        let is = |m: &MeshMaterial3d<StandardMaterial>| solid(alpha(Some(m), None, &mats, None)?);
        assert_eq!(is(&opaque), Some(true));
        assert_eq!(is(&mask), Some(false), "a fern's fronds");
        assert_eq!(is(&blend), Some(false), "glass");
        assert_eq!(is(&missing), None);
    }

    /// A forest stone whose material rim.rs swapped for its rim is solid by its base (it took
    /// no footprint while only the standard material was read), and a kit shader's is none.
    #[test]
    fn a_rimmed_stone_is_solid_by_its_base() {
        let mats = Assets::<StandardMaterial>::default();
        let mut rims = Assets::<RimMaterial>::default();
        let mut of = |a: AlphaMode| {
            MeshMaterial3d(rims.add(bevy::pbr::ExtendedMaterial {
                base: StandardMaterial {
                    alpha_mode: a,
                    ..default()
                },
                extension: crate::rim::RimExt::default(),
            }))
        };
        let (stone, frond) = (of(AlphaMode::Opaque), of(AlphaMode::Mask(0.5)));
        let is = |r| alpha(None, Some(r), &mats, Some(&rims)).map(solid);
        assert_eq!(is(&stone), Some(Some(true)), "a stone");
        assert_eq!(is(&frond), Some(Some(false)), "a frond");
        assert_eq!(alpha(None, None, &mats, Some(&rims)), None, "a kit shader");
    }

    /// A fallen log a quarter metre thick blocks along its length; a twig does not.
    #[test]
    fn a_thin_log_blocks_along_its_length() {
        let log = circles(Vec2::new(-1.5, -0.125), Vec2::new(1.5, 0.125));
        assert!(!log.is_empty(), "a log 3 x 0.25 m");
        let inside = |p: Vec2| log.iter().any(|&(q, r)| p.distance(q) < r + BODY);
        for x in [-1.3, 0.0, 1.3] {
            assert!(inside(Vec2::new(x, 0.0)), "{x} along it");
        }
        assert!(!inside(Vec2::new(0.0, 0.6)), "a stride beside it");
        assert!(
            circles(Vec2::ZERO, Vec2::new(0.4, 0.05)).is_empty(),
            "a twig"
        );
        assert!(
            circles(Vec2::ZERO, Vec2::new(0.4, 0.15)).is_empty(),
            "a short stick"
        );
    }

    /// A dead tree's foot is its trunk: points under its spread branches, not their 5 m;
    /// a trunk thinner than a stone still takes one.
    #[test]
    fn a_trees_foot_is_its_trunk() {
        let mut points = Vec::new();
        for k in 0..8 {
            let a = k as f32 * core::f32::consts::FRAC_PI_4;
            let (s, c) = a.sin_cos();
            points.push(Vec3::new(0.9 * c, 0.0, 0.8 * s)); // roots flared at the ground
            points.push(Vec3::new(0.5 * c, 0.6, 0.5 * s));
            for h in [1.1, 1.2, 1.3, 1.4] {
                points.push(Vec3::new(0.2 * c, h, 0.2 * s)); // its trunk
            }
            points.push(Vec3::new(2.5 * c, 3.0, 1.3 * s)); // branches
        }
        // One low branch reaching out through the trunk's height.
        points.push(Vec3::new(1.8, 1.25, 0.0));
        let (lo, hi) = foot(&points, 1.0).unwrap();
        assert!((hi - lo).max_element() < 0.5, "{lo} {hi}: its trunk");
        let c = circles(lo, hi);
        assert_eq!(c.len(), 1, "{c:?}");
        assert!(c[0].0.length() < 0.05, "at its middle");
        let thin: Vec<Vec3> = points
            .iter()
            .map(|p| *p * Vec3::new(0.3, 1.0, 0.3))
            .collect();
        let (lo, hi) = foot(&thin, 1.0).unwrap();
        assert!(!circles(lo, hi).is_empty(), "a thin trunk");
        let flat = [Vec3::ZERO, Vec3::new(4.0, 0.3, 4.0)];
        assert_eq!(foot(&flat, 1.0), None, "nothing at a trunk's height");
    }
}
