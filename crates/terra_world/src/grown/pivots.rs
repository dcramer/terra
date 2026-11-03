//! Wind in levels (D597): what each vertex of a grown tree needs for its limbs to turn about
//! their own roots, each with its own phase and stiffness, as SpeedTree's branch motion and
//! Unreal's Pivot Painter 2 move theirs. Pivot Painter bakes each element's pivot into
//! textures because its meshes come from a modelling package; we grow the tree, so the pivots
//! are known as it is built and ride in vertex attributes (`ATTRIBUTE_PIVOT`, `ATTRIBUTE_TWIG`),
//! read by `vertex_pivoted` in assets/shaders/foliage.wesl in every pass.
//!
//! Every tree gets its pivots from its own shape, whatever its kind (D136): a vertex's pivot is
//! the root of the great limb (depth 1) it grows from, in the tree's own space, and in w the
//! limb's length in decimetres plus its phase (0 to 1). A trunk's vertex, or a card on no
//! limb, has w -1: it leans with the trunk alone. A modelled tree, whose limbs were not grown
//! (a veteran oak's), takes each mass of its crown for a great limb, rooted at its fork
//! (`of_masses`).

use bevy::mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef, VertexFormat};
use bevy::prelude::*;
use bevy::render::render_resource::RenderPipelineDescriptor;

/// Each vertex's great limb's root, and its length and phase (the module's comment).
pub const ATTRIBUTE_PIVOT: MeshVertexAttribute =
    MeshVertexAttribute::new("Terra_Pivot", 597_000_001, VertexFormat::Float32x4);
/// The second level (the twig's): each clump of leaves turns about where it sits on its limb,
/// with a phase of its own, so the sprays of one great limb stir apart rather than ride its
/// turn as one block. In w, the clump's size in decimetres plus its phase; -1 on wood.
pub const ATTRIBUTE_TWIG: MeshVertexAttribute =
    MeshVertexAttribute::new("Terra_Twig", 597_000_002, VertexFormat::Float32x4);
/// For a vertex on no limb: the trunk, or a card with none near it.
pub const NO_PIVOT: [f32; 4] = [0.0, 0.0, 0.0, -1.0];
/// Where the shader reads them: past every location Bevy's own vertex inputs take.
const LOCATION: u32 = 15;
const TWIG_LOCATION: u32 = 14;

/// A limb as the pivots see it: its line of points from its root, and how deep it is (0 the
/// trunk, 1 a great limb).
pub struct Line<'a> {
    pub points: &'a [Vec3],
    pub depth: u32,
}

/// A clump of leaves: where, how big, and which way its twig runs outward (zero if unknown).
#[derive(Clone, Copy)]
pub struct Clump {
    pub at: Vec3,
    pub size: f32,
    pub along: Vec3,
}

/// The pivots of a tree's limbs, one a limb, and of its clumps of leaves, one a clump; and
/// each clump's own, where it sits on its limb. Empty for a mesh that sways as one (a bush's).
#[derive(Default)]
pub struct Pivots {
    pub limbs: Vec<[f32; 4]>,
    pub tips: Vec<[f32; 4]>,
    pub twigs: Vec<[f32; 4]>,
}

impl Pivots {
    /// Whether there are any, so a mesh built with them carries them.
    pub fn any(&self) -> bool {
        !self.limbs.is_empty() || !self.tips.is_empty()
    }

    /// Clump `i`'s great limb's and own pivot, as a vertex of its cards takes them.
    pub fn of_clump(&self, i: usize) -> (Option<[f32; 4]>, Option<[f32; 4]>) {
        (self.tips.get(i).copied(), self.twigs.get(i).copied())
    }
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

/// The limb nearest `p` of those `allowed`.
fn nearest(limbs: &[Line], p: Vec3, allowed: impl Fn(usize) -> bool) -> Option<usize> {
    (0..limbs.len())
        .filter(|&j| allowed(j))
        .map(|j| (j, nearest_point(limbs[j].points, p).distance(p)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(j, _)| j)
}

/// A great limb's pivot: its root, and in w its length in decimetres plus its phase, a golden
/// ratio's step by its index, so neighbouring limbs are never in step.
fn pack(root: Vec3, length: f32, index: usize) -> [f32; 4] {
    let phase = (index as f32 * 0.618_034).fract();
    [
        root.x,
        root.y,
        root.z,
        (length * 10.0).round().max(1.0) + phase,
    ]
}

/// A clump's own pivot, where it sits (`at`), its phase the plastic constant's steps by its
/// index, apart from the limbs' golden ones.
fn twig(at: Vec3, size: f32, index: usize) -> [f32; 4] {
    let phase = (index as f32 * 0.754_877_7).fract();
    [at.x, at.y, at.z, (size * 10.0).round().max(1.0) + phase]
}

/// Each limb's and each clump's pivot. A limb's great limb is found by where its root sits:
/// on the nearest limb of a lesser depth, whose great limb it shares. A clump's is the
/// nearest limb's to it.
pub fn of(limbs: &[Line], clumps: &[Clump]) -> Pivots {
    let n = limbs.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| limbs[i].depth);
    let mut great: Vec<Option<usize>> = vec![None; n];
    for &i in &order {
        let limb = &limbs[i];
        great[i] = match limb.depth {
            0 => None,
            1 => Some(i),
            depth => match limb.points.first() {
                Some(&root) => {
                    nearest(limbs, root, |j| limbs[j].depth < depth).and_then(|j| great[j])
                }
                None => None,
            },
        };
    }
    let packed = |great: Option<usize>| {
        great.map_or(NO_PIVOT, |j| {
            let points = limbs[j].points;
            let length: f32 = points.windows(2).map(|ab| ab[0].distance(ab[1])).sum();
            pack(points.first().copied().unwrap_or(Vec3::ZERO), length, j)
        })
    };
    let on = |clump: &Clump| nearest(limbs, clump.at, |j| limbs[j].depth >= 1);
    Pivots {
        limbs: great.iter().map(|&g| packed(g)).collect(),
        tips: clumps
            .iter()
            .map(|clump| packed(on(clump).and_then(|j| great[j])))
            .collect(),
        twigs: clumps
            .iter()
            .enumerate()
            .map(|(i, clump)| {
                // Where it sits on its limb; with none grown, its size back along its twig.
                let at = match on(clump) {
                    Some(j) => nearest_point(limbs[j].points, clump.at),
                    None => clump.at - clump.along * clump.size,
                };
                twig(at, clump.size, i)
            })
            .collect(),
    }
}

/// A modelled tree's pivots, whose limbs were not grown but whose clumps are known by the mass
/// of its crown each is in (a veteran oak's, its tips from tools/blender/tips.py): each mass
/// turns as a great limb rooted at the `fork`, where the bole parts, as long as its middle is
/// far from it; each clump about its size back along its twig.
pub fn of_masses(fork: Vec3, masses: &[Vec3], mass_of: &[usize], clumps: &[Clump]) -> Pivots {
    Pivots {
        limbs: Vec::new(),
        tips: clumps
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let m = mass_of.get(i).copied().unwrap_or(0);
                masses
                    .get(m)
                    .map_or(NO_PIVOT, |&middle| pack(fork, middle.distance(fork), m))
            })
            .collect(),
        twigs: clumps
            .iter()
            .enumerate()
            .map(|(i, clump)| twig(clump.at - clump.along * clump.size, clump.size, i))
            .collect(),
    }
}

/// A mesh carrying pivots is read by the shader's `vertex_pivoted`, in every pass: the main
/// one, the depth and motion prepass and the shadows, so what is drawn and what casts the
/// shadow move together. Every foliage material calls it from its `specialize`.
pub fn specialize(descriptor: &mut RenderPipelineDescriptor, layout: &MeshVertexBufferLayoutRef) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A trunk with two great limbs, a branch on the second, and a clump at each end.
    fn tree() -> (Vec<Vec<Vec3>>, Vec<u32>) {
        let trunk = vec![Vec3::ZERO, Vec3::Y * 4.0, Vec3::Y * 8.0];
        let east = vec![Vec3::Y * 4.0, Vec3::new(3.0, 6.0, 0.0)];
        let west = vec![Vec3::Y * 5.0, Vec3::new(-3.0, 7.0, 0.0)];
        // From part-way along the west limb, onward.
        let twig = vec![Vec3::new(-1.5, 6.0, 0.0), Vec3::new(-2.0, 7.5, 1.0)];
        (vec![trunk, east, west, twig], vec![0, 1, 1, 2])
    }

    fn lines<'a>(points: &'a [Vec<Vec3>], depths: &[u32]) -> Vec<Line<'a>> {
        points
            .iter()
            .zip(depths)
            .map(|(points, &depth)| Line { points, depth })
            .collect()
    }

    #[test]
    fn a_branch_turns_with_the_great_limb_it_grows_from_and_the_trunk_with_none() {
        let (points, depths) = tree();
        let clumps = [
            Clump {
                at: Vec3::new(3.1, 6.1, 0.0),
                size: 0.5,
                along: Vec3::ZERO,
            },
            Clump {
                at: Vec3::new(-2.0, 7.6, 1.0),
                size: 0.5,
                along: Vec3::ZERO,
            },
        ];
        let pivots = of(&lines(&points, &depths), &clumps);
        assert_eq!(pivots.limbs[0], NO_PIVOT, "the trunk leans alone");
        // The branch's root is on the west limb, so it takes the west limb's pivot.
        assert_eq!(pivots.limbs[3], pivots.limbs[2]);
        assert_eq!(&pivots.limbs[2][..3], &[0.0, 5.0, 0.0]);
        assert_eq!(&pivots.limbs[1][..3], &[0.0, 4.0, 0.0]);
        // The two great limbs are out of step.
        assert!((pivots.limbs[1][3].fract() - pivots.limbs[2][3].fract()).abs() > 0.1);
        // The east clump turns with the east limb, the other with the west through its branch.
        assert_eq!(pivots.tips[0], pivots.limbs[1]);
        assert_eq!(pivots.tips[1], pivots.limbs[2]);
        // Each clump's own pivot is where it sits on its limb, and they are out of step.
        assert!(Vec3::from_slice(&pivots.twigs[0][..3]).distance(Vec3::new(3.0, 6.0, 0.0)) < 0.2);
        assert!((pivots.twigs[0][3].fract() - pivots.twigs[1][3].fract()).abs() > 0.1);
    }

    #[test]
    fn a_modelled_trees_masses_turn_from_its_fork_each_with_its_own_phase() {
        let fork = Vec3::Y * 6.0;
        let masses = [Vec3::new(4.0, 9.0, 0.0), Vec3::new(-4.0, 9.0, 0.0)];
        let clump = |x: f32| Clump {
            at: Vec3::new(x, 9.5, 0.0),
            size: 0.6,
            along: Vec3::X * x.signum(),
        };
        let pivots = of_masses(
            fork,
            &masses,
            &[0, 1, 1],
            &[clump(4.5), clump(-4.5), clump(-3.5)],
        );
        assert_eq!(&pivots.tips[0][..3], &[0.0, 6.0, 0.0]);
        assert_eq!(pivots.tips[1], pivots.tips[2], "one mass turns as one limb");
        assert!((pivots.tips[0][3].fract() - pivots.tips[1][3].fract()).abs() > 0.1);
        // 5 m from the fork to each mass's middle: 50 decimetres.
        assert_eq!(pivots.tips[0][3].floor(), 50.0);
        // Each clump's own pivot is its size back along its twig.
        assert!(
            Vec3::from_slice(&pivots.twigs[0][..3]).distance(Vec3::new(3.9, 9.5, 0.0)) < 1.0e-4
        );
    }
}
