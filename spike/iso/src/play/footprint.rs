//! What the player walks round (play.rs): every placed model's footprint on the ground, as a
//! row of circles along its long side, the cheap collider top-down games give props (Diablo's
//! and Path of Exile's props block by a footprint on the ground grid, not their mesh). Each is
//! taken once from the bounds of the model's solid meshes in its own frame: an opaque
//! material's (a fern's fronds and a tree's leaf cards are cut out by alpha, so the plants
//! stay walkable) and taller than a kerb (a manhole cover, a decal). A posed body's bounds
//! are its bind pose's, far wider than its feet (kit/contact.rs), so a creature takes one
//! circle at its middle, moving with it. What is smaller than a stone or bigger than a cart
//! (walls, ground, a tree whole) is left to each layout's `Ground`.

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

/// The circles along a box on the ground (lo, hi its corners): as many as its long side
/// holds of its short, each its half short side across, end to end.
pub fn circles(lo: Vec2, hi: Vec2) -> Vec<(Vec2, f32)> {
    let size = hi - lo;
    let (short, long) = (size.min_element(), size.max_element());
    if short < SHORT.0 || short > SHORT.1 || long > LONG {
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

/// Whether a mesh's material is solid: opaque, not cut out or blended (leaves, fronds,
/// glass); `None` while it loads.
fn solid(
    materials: &Assets<StandardMaterial>,
    m: &MeshMaterial3d<StandardMaterial>,
) -> Option<bool> {
    materials
        .get(&m.0)
        .map(|m| matches!(m.alpha_mode, AlphaMode::Opaque))
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
    meshes: Query<
        (
            Option<&Aabb>,
            &GlobalTransform,
            Option<&MeshMaterial3d<StandardMaterial>>,
        ),
        With<Mesh3d>,
    >,
    materials: Res<Assets<StandardMaterial>>,
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
        for e in children.iter_descendants(root) {
            let Ok((b, mg, m)) = meshes.get(e) else {
                continue;
            };
            any = true;
            // Its bounds and material not in yet: next frame. A material of the kit's own
            // (foliage, bark shaders) is no prop's solid body.
            let Some(b) = b else { continue 'root };
            match m.map(|m| solid(&materials, m)) {
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
        }
        if !any {
            continue; // Its meshes are not in yet.
        }
        let c: Vec<(Vec2, f32)> = if lo.x > hi.x {
            Vec::new()
        } else {
            // In the world's metres, for the sizes, then back to its own frame.
            circles(lo * scale, hi * scale)
                .into_iter()
                .map(|(p, r)| (p / scale, r / scale))
                .collect()
        };
        let size = (hi - lo).max(Vec2::ZERO) * scale;
        debug!(
            "footprint {:?} at {:.1}: {} circles, {:.2} x {:.2} m",
            asset.0.path(),
            g.translation().xz(),
            c.len(),
            size.x,
            size.y
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
        assert_eq!(solid(&mats, &opaque), Some(true));
        assert_eq!(solid(&mats, &mask), Some(false), "a fern's fronds");
        assert_eq!(solid(&mats, &blend), Some(false), "glass");
        let missing = of(&mut mats, AlphaMode::Opaque);
        mats.remove(&missing.0);
        assert_eq!(solid(&mats, &missing), None);
    }
}
