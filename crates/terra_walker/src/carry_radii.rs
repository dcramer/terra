//! How thick a body's legs and trunk are, read once from its skin: the radii of the capsules
//! `carry_arm` keeps a carried look off (`carry_clear`; held-constraint j2). A capsule's axis
//! is its bones' joints as posed each frame; its radius is measured here at the bind pose,
//! from the vertices whose heaviest joint is one of the capsule's bones, as
//! `tools/characters/skin.ts` gives a vertex to a part (its `BODY` bones, of which these are
//! the thighs, the calves and the trunk; the twin there must name the same bones).
//!
//! Not read: the body's morphs and a template's joint offsets (the mesh as loaded is measured,
//! in its own units, taken to be the body's), and the neck, head and feet, which are no capsule.

use bevy::{
    mesh::{
        VertexAttributeValues,
        skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    },
    prelude::*,
};

/// The capsules, in this order: the left thigh and calf, the right thigh and calf, the trunk.
pub const PARTS: usize = 5;

/// The share of a part's vertices its capsule's radius holds within it: the rest are a few
/// far points (a buttock on a thigh, a belly on a trunk) a capsule on the bone cannot follow.
pub const WITHIN: f32 = 0.9;

/// Which capsule a bone's skin is part of, by the bone's name; none for any other bone.
pub fn part(bone: &str) -> Option<usize> {
    Some(match bone {
        "thigh_l" | "thigh_twist_01_l" => 0,
        "calf_l" | "calf_twist_01_l" => 1,
        "thigh_r" | "thigh_twist_01_r" => 2,
        "calf_r" | "calf_twist_01_r" => 3,
        "pelvis" => 4,
        spine if spine.starts_with("spine_0") => 4,
        _ => return None,
    })
}

/// A capsule's radius from where its part's vertices stand off its axis (`off_axis`): the
/// distance from their middle (their mean) that `WITHIN` of them are inside; 0 for none.
///
/// From their middle, not from the axis: a limb's joints do not run down its middle (the
/// calf's run down the shin, 3.4 cm in front of the calf's middle on the base body), and the
/// distance from such an axis reads the far side's, a calf of 10.9 cm where its section at
/// mid-length is 6.7 by 6.8 cm of half-width (held-constraint j4; 8.2 cm measured so). The
/// capsule still stands on the joints, so it is that much proud on the near side and short on
/// the far one.
pub fn radius(off: &[Vec3]) -> f32 {
    if off.is_empty() {
        return 0.0;
    }
    let middle = off.iter().sum::<Vec3>() / off.len() as f32;
    let mut far: Vec<f32> = off.iter().map(|o| o.distance(middle)).collect();
    far.sort_by(f32::total_cmp);
    far[((far.len() - 1) as f32 * WITHIN) as usize]
}

/// Where `p` stands off the line through `a` and `b`, square to it; none past either end (a
/// buttock weighted to a thigh, a heel to a calf: skin the capsule's length does not reach,
/// which would be measured to the end's point) and for an axis with no length.
pub fn off_axis(p: Vec3, a: Vec3, b: Vec3) -> Option<Vec3> {
    let along = b - a;
    if along.length_squared() <= 0.0 {
        return None;
    }
    let t = (p - a).dot(along) / along.length_squared();
    (0.0..=1.0).contains(&t).then(|| p - a - along * t)
}

/// The radii of a body's capsules whose axes run between the joints `axes` (each a part's two
/// ends), from every skinned mesh under `root`; 0 for a part with no skin or a joint no skin has.
pub fn measure(
    root: Entity,
    axes: [[Entity; 2]; PARTS],
    children: &Query<&Children>,
    names: &Query<&Name>,
    skins: &Query<(&SkinnedMesh, &Mesh3d)>,
    meshes: &Assets<Mesh>,
    bindposes: &Assets<SkinnedMeshInverseBindposes>,
) -> [f32; PARTS] {
    let mut off: [Vec<Vec3>; PARTS] = Default::default();
    for each in children.iter_descendants(root) {
        let Ok((skin, mesh)) = skins.get(each) else {
            continue;
        };
        let (Some(mesh), Some(poses)) =
            (meshes.get(&mesh.0), bindposes.get(&skin.inverse_bindposes))
        else {
            continue;
        };
        let Some(VertexAttributeValues::Float32x3(at)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        // A joint's place at the bind pose, in the mesh's frame.
        let joint = |bone: Entity| {
            let k = skin.joints.iter().position(|j| *j == bone)?;
            Some(poses.get(k)?.inverse().w_axis.truncate())
        };
        let ends = axes.map(|[a, b]| joint(a).zip(joint(b)));
        let parts: Vec<Option<usize>> = skin
            .joints
            .iter()
            .map(|j| names.get(*j).ok().and_then(|n| part(n.as_str())))
            .collect();
        // Each vertex's heaviest joint, over both sets of four where the mesh has eight (D601).
        let mut heaviest = vec![(0.0_f32, 0_usize); at.len()];
        for (index, weight) in [
            (Mesh::ATTRIBUTE_JOINT_INDEX, Mesh::ATTRIBUTE_JOINT_WEIGHT),
            (
                Mesh::ATTRIBUTE_JOINT_INDEX_1,
                Mesh::ATTRIBUTE_JOINT_WEIGHT_1,
            ),
        ] {
            let (
                Some(VertexAttributeValues::Uint16x4(j)),
                Some(VertexAttributeValues::Float32x4(w)),
            ) = (mesh.attribute(index), mesh.attribute(weight))
            else {
                continue;
            };
            for ((most, j), w) in heaviest.iter_mut().zip(j).zip(w) {
                for k in 0..4 {
                    if w[k] > most.0 {
                        *most = (w[k], j[k] as usize);
                    }
                }
            }
        }
        for (p, (weight, j)) in at.iter().zip(heaviest) {
            if weight <= 0.0 {
                continue;
            }
            if let Some(k) = parts.get(j).copied().flatten()
                && let Some((a, b)) = ends[k]
                && let Some(o) = off_axis(Vec3::from(*p), a, b)
            {
                off[k].push(o);
            }
        }
    }
    off.map(|o| radius(&o))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn a_radius_holds_nine_in_ten_of_its_vertices_and_only_its_parts_bones_count() {
        // Known another way: ten pairs of vertices 1 to 10 cm either side of the axis (their
        // middle on it), nine pairs within 9 cm.
        let off: Vec<Vec3> = (1..=10)
            .flat_map(|k| [Vec3::X, Vec3::NEG_X].map(|side| side * k as f32 * 0.01))
            .collect();
        assert!((radius(&off) - 0.09).abs() < 1e-6);
        assert_eq!(radius(&[]), 0.0);
        assert_eq!(part("thigh_twist_01_r"), Some(2));
        assert_eq!(part("spine_02"), Some(4));
        assert_eq!((part("foot_l"), part("upperarm_l")), (None, None));
        // A vertex stands off the line square to it; past a bone's end it is not the part's.
        let o = off_axis(Vec3::new(0.03, 0.4, 0.0), Vec3::ZERO, Vec3::Y).unwrap();
        assert!(o.distance(Vec3::new(0.03, 0.0, 0.0)) < 1e-6);
        assert_eq!(
            off_axis(Vec3::new(0.03, 1.04, 0.0), Vec3::ZERO, Vec3::Y),
            None
        );
        assert_eq!(off_axis(Vec3::X, Vec3::Y, Vec3::Y), None);
    }

    /// The fault planted (held-constraint j4): a calf 5 cm in radius whose joints run 4 cm in
    /// front of its middle, as a shin's do, is 5 cm, not the 9 its far side stands off them.
    #[test]
    fn a_limb_whose_joints_run_off_its_middle_is_as_thick_as_it_is() {
        let ring: Vec<Vec3> = (0..16)
            .map(|k| {
                let a = k as f32 * std::f32::consts::TAU / 16.0;
                let at = Vec3::new(0.05 * libm::cosf(a), 0.3, -0.04 + 0.05 * libm::sinf(a));
                off_axis(at, Vec3::ZERO, Vec3::Y).unwrap()
            })
            .collect();
        let from_the_axis = ring.iter().map(|o| o.length()).fold(0.0, f32::max);
        assert!((from_the_axis - 0.09).abs() < 1e-5, "{from_the_axis}");
        assert!((radius(&ring) - 0.05).abs() < 1e-5, "{}", radius(&ring));
    }

    /// A skin of a thigh and a calf end to end along -Y, four vertices on each, the thigh's
    /// 8 cm off its axis and the calf's 5: measured so, and a vertex an arm's bone weighs
    /// most, planted 40 cm out, is not counted.
    #[test]
    fn a_bodys_radii_are_measured_from_its_skin() {
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let bone = |world: &mut World, name: &'static str| {
            world.spawn((Name::new(name), ChildOf(root))).id()
        };
        let [thigh, calf, foot, arm] =
            ["thigh_l", "calf_l", "foot_l", "upperarm_l"].map(|n| bone(&mut world, n));
        let places = [Vec3::Y, Vec3::Y * 0.5, Vec3::ZERO, Vec3::new(0.4, 1.0, 0.0)];
        let poses: Vec<Mat4> = places
            .iter()
            .map(|p| Mat4::from_translation(*p).inverse())
            .collect();
        let (mut at, mut joints) = (Vec::new(), Vec::new());
        for (j, y, r) in [(0_u16, 0.75, 0.08), (1, 0.25, 0.05)] {
            for side in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
                at.push((Vec3::Y * y + side * r).to_array());
                joints.push([j, 3, 0, 0]);
            }
        }
        // Two of them: counted, they would be two of the thigh's six and its radius 40 cm.
        let mut weights = vec![[0.7_f32, 0.3, 0.0, 0.0]; 8];
        for _ in 0..2 {
            at.push([0.4, 0.75, 0.0]);
            joints.push([0, 3, 0, 0]);
            weights.push([0.2, 0.8, 0.0, 0.0]);
        }
        let mesh = Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, at)
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_JOINT_INDEX,
            VertexAttributeValues::Uint16x4(joints),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights);
        let mut meshes = Assets::<Mesh>::default();
        let mut bindposes = Assets::<SkinnedMeshInverseBindposes>::default();
        let skin = SkinnedMesh {
            inverse_bindposes: bindposes.add(SkinnedMeshInverseBindposes::from(poses)),
            joints: vec![thigh, calf, foot, arm],
        };
        world.spawn((Mesh3d(meshes.add(mesh)), skin, ChildOf(root)));
        world.insert_resource(meshes);
        world.insert_resource(bindposes);
        let none = [arm, arm];
        let radii = world
            .run_system_once(
                move |children: Query<&Children>,
                      names: Query<&Name>,
                      skins: Query<(&SkinnedMesh, &Mesh3d)>,
                      meshes: Res<Assets<Mesh>>,
                      bindposes: Res<Assets<SkinnedMeshInverseBindposes>>| {
                    measure(
                        root,
                        [[thigh, calf], [calf, foot], none, none, none],
                        &children,
                        &names,
                        &skins,
                        &meshes,
                        &bindposes,
                    )
                },
            )
            .unwrap();
        assert!((radii[0] - 0.08).abs() < 1e-5, "thigh {}", radii[0]);
        assert!((radii[1] - 0.05).abs() < 1e-5, "calf {}", radii[1]);
        assert_eq!(radii[2..], [0.0; 3]);
    }
}
