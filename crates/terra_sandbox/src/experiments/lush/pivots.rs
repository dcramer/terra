//! Wind in levels (D597, an experiment): what each vertex of a grown tree needs for its
//! limbs to turn about their own roots, each with its own phase and stiffness, as SpeedTree's
//! branch motion and Unreal's Pivot Painter 2 move theirs. Pivot Painter bakes each element's
//! pivot into textures because its meshes come from a modelling package; we grow the tree,
//! so the pivots are known as it is built and ride in a vertex attribute
//! (`ATTRIBUTE_PIVOT`), read by `vertex_pivoted` in assets/shaders/foliage.wesl.
//!
//! A vertex's pivot is the root of the great limb (depth 1) it grows from, in the tree's
//! own space; in w, the limb's length in decimetres plus its phase (0 to 1). A trunk's
//! vertex, or a card on no limb, has w -1: it leans with the trunk alone.

use super::Tree;
use bevy::mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef, VertexFormat};
use bevy::prelude::*;
use bevy::render::render_resource::RenderPipelineDescriptor;

/// Each vertex's great limb's root, and its length and phase (the module's comment).
pub(super) const ATTRIBUTE_PIVOT: MeshVertexAttribute =
    MeshVertexAttribute::new("Terra_Pivot", 597_000_001, VertexFormat::Float32x4);
/// For a vertex on no limb: the trunk, or a card with none near it.
pub(super) const NO_PIVOT: [f32; 4] = [0.0, 0.0, 0.0, -1.0];
/// Where the shader reads it: past every location Bevy's own vertex inputs take.
const LOCATION: u32 = 15;
/// The second level (the twig's): each clump of leaves turns about where it sits on its limb,
/// with a phase of its own, so the sprays of one great limb stir apart rather than ride its
/// turn as one block. In w, the clump's size in decimetres plus its phase; -1 on wood.
pub(super) const ATTRIBUTE_TWIG: MeshVertexAttribute =
    MeshVertexAttribute::new("Terra_Twig", 597_000_002, VertexFormat::Float32x4);
const TWIG_LOCATION: u32 = 14;

/// The pivots of a tree's limbs, one a limb, and of its clumps of leaves, one a tip; and
/// each clump's own, where it sits on its limb.
pub(super) struct Pivots {
    pub(super) limbs: Vec<[f32; 4]>,
    pub(super) tips: Vec<[f32; 4]>,
    pub(super) twigs: Vec<[f32; 4]>,
}

/// The point of the line nearest `p`.
fn nearest_point(points: &[Vec3], p: Vec3) -> Vec3 {
    if points.len() < 2 {
        return points.first().copied().unwrap_or(p);
    }
    points
        .windows(2)
        .map(|ab| {
            let (a, b) = (ab[0], ab[1]);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1.0e-8)).clamp(0.0, 1.0);
            a + ab * t
        })
        .min_by(|a, b| a.distance(p).total_cmp(&b.distance(p)))
        .unwrap_or(p)
}

/// How far `p` is from the line of points.
fn distance_to(points: &[Vec3], p: Vec3) -> f32 {
    if points.len() < 2 {
        return points.first().map_or(f32::MAX, |a| a.distance(p));
    }
    points
        .windows(2)
        .map(|ab| {
            let (a, b) = (ab[0], ab[1]);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1.0e-8)).clamp(0.0, 1.0);
            (a + ab * t).distance(p)
        })
        .fold(f32::MAX, f32::min)
}

/// The limb nearest `p` of those `allowed`.
fn nearest(tree: &Tree, p: Vec3, allowed: impl Fn(usize) -> bool) -> Option<usize> {
    (0..tree.limbs.len())
        .filter(|&j| allowed(j))
        .map(|j| (j, distance_to(&tree.limbs[j].points, p)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(j, _)| j)
}

/// Each limb's and each clump's pivot. A limb's great limb is found by where its root sits:
/// on the nearest limb of a lesser depth, whose great limb it shares. A clump's is the
/// nearest limb's to it.
pub(super) fn of(tree: &Tree) -> Pivots {
    let n = tree.limbs.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| tree.limbs[i].depth);
    let mut great: Vec<Option<usize>> = vec![None; n];
    for &i in &order {
        let limb = &tree.limbs[i];
        great[i] = match limb.depth {
            0 => None,
            1 => Some(i),
            depth => nearest(tree, limb.points[0], |j| tree.limbs[j].depth < depth)
                .and_then(|j| great[j]),
        };
    }
    let pack = |great: Option<usize>| {
        great.map_or(NO_PIVOT, |j| {
            let limb = &tree.limbs[j];
            let root = limb.points[0];
            let length: f32 = limb.points.windows(2).map(|ab| ab[0].distance(ab[1])).sum();
            // Phases a golden ratio apart, so neighbouring limbs are never in step.
            let phase = (j as f32 * 0.618_034).fract();
            [
                root.x,
                root.y,
                root.z,
                (length * 10.0).round().max(1.0) + phase,
            ]
        })
    };
    Pivots {
        limbs: great.iter().map(|&g| pack(g)).collect(),
        tips: tree
            .tips
            .iter()
            .map(|&(tip, _)| {
                pack(nearest(tree, tip, |j| tree.limbs[j].depth >= 1).and_then(|j| great[j]))
            })
            .collect(),
        twigs: tree
            .tips
            .iter()
            .enumerate()
            .map(|(i, &(tip, size))| {
                // Where it sits on its limb; on a modelled tree, which has none grown (a
                // veteran oak's), a clump's size back along its twig.
                let at = match nearest(tree, tip, |j| tree.limbs[j].depth >= 1) {
                    Some(j) => nearest_point(&tree.limbs[j].points, tip),
                    None => tip - tree.along.get(i).copied().unwrap_or(Vec3::ZERO) * size,
                };
                // The plastic constant's steps, apart from the limbs' golden ones.
                let phase = (i as f32 * 0.754_877_7).fract();
                [at.x, at.y, at.z, (size * 10.0).round().max(1.0) + phase]
            })
            .collect(),
    }
}

/// A mesh carrying pivots is read by the shader's `vertex_pivoted`, in every pass: the main
/// one, the depth and motion prepass and the shadows, so what is drawn and what casts the
/// shadow move together.
pub(super) fn specialize(
    descriptor: &mut RenderPipelineDescriptor,
    layout: &MeshVertexBufferLayoutRef,
) {
    // The shader has two vertex entry points, so wgpu cannot pick one on its own.
    descriptor.vertex.entry_point = Some("vertex".into());
    if !layout.0.contains(ATTRIBUTE_PIVOT) {
        return;
    }
    let Ok(extra) = layout.0.get_layout(&[
        ATTRIBUTE_PIVOT.at_shader_location(LOCATION),
        ATTRIBUTE_TWIG.at_shader_location(TWIG_LOCATION),
    ]) else {
        return;
    };
    if let Some(buffer) = descriptor.vertex.buffers.first_mut() {
        buffer.attributes.extend(extra.attributes);
        descriptor.vertex.entry_point = Some("vertex_pivoted".into());
    }
}
