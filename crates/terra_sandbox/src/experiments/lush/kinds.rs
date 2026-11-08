//! The forest's kinds of tree, grown once for a seed and drawn in each world's materials.
//!
//! The kinds are grown from the forest's own seed (`forest_seed`, the world's derived), so
//! they are the same in every world of that seed it is planted in; only their materials
//! differ, as those take the world's wind. So their meshes are grown on the first entry and
//! kept until a world of another seed is entered, as a streaming game
//! keeps the meshes its cells share resident while cells come and go (Unreal's World
//! Partition streams actors, not the static meshes they instance): grown again on each
//! entry they were about a second of the crossing's longest frame, and each crossing gave
//! the footings' `Shapes` new meshes to remember, about 2.4 MiB never let go (instances j9).

use super::footing::{Shapes, SolidShape};
use super::{
    BARKS, Foliage, FoliageExtension, KINDS, Leaf, Looks, SHADOW_LUMP, Species, TreeModel,
    VETERAN_M, VETERAN_TRUNK_M, VETERANS, bark_material, crown_clumps, crown_far, crown_near,
    crown_stand_in, floor, grow, lumps_of, species,
};
use bevy::prelude::*;
use std::sync::Arc;
use terra_gen::Seed;
use terra_world::mesh_sink::MeshSink;
use terra_world::sky_occlusion;

/// Every kind's model, the grown `KINDS` then the `VETERANS`, without its materials, by the
/// forest's seed they grew from, the world played last; empty until a forest is first
/// planted.
#[derive(Resource, Default)]
pub(super) struct Grown(Vec<(Seed, Vec<TreeModel>)>);

/// How many worlds' kinds are kept grown: the one played and the one last left, so a
/// crossing back (the way a portal is used, D470) grows nothing again, a third of a second
/// of every crossing's cover (D501) before.
const WORLDS_KEPT: usize = 2;

/// Which of `BARKS` a grown kind's wood is drawn in, and which species' leaves, tint and far
/// crown it takes (0 oak, 1 pine, 2 spruce).
pub(super) fn materials_of(kind: usize) -> (usize, usize) {
    match KINDS[kind].0 {
        Species::Oak => (kind % 3, 0),
        Species::Pine => (3 + kind % 2, 1),
        // Dark green, about sRGB 0.15. The needle picture is the deep green of Norway
        // spruce, and the tint pushes it further from blue: needles this dark are lit
        // mostly by the blue sky, which turned them teal (D247, D248).
        Species::Spruce => (5 + kind % 2, 2),
    }
}

/// What the kinds are grown and drawn with.
pub(super) struct Grow<'a> {
    pub(super) seed: Seed,
    pub(super) looks: &'a Looks,
    pub(super) wind: &'a FoliageExtension,
    pub(super) ivy_leaves: &'a [Leaf; 3],
    pub(super) assets: &'a AssetServer,
}

/// Every kind of tree in this world's materials, grown first if the app has not yet: the
/// grown kinds, then the veterans.
pub(super) fn trees(
    grown: &mut Grown,
    with: &Grow,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<Foliage>,
    shapes: &mut Shapes,
) -> Vec<TreeModel> {
    // The world's kinds, the newest last; a world not kept is grown now, the oldest let go.
    let kept = grown.0.iter().position(|(seed, _)| *seed == with.seed);
    let models = match kept {
        Some(kept) => grown.0.remove(kept).1,
        None => {
            // Every kind at once, over every core, as the woods' conifers are grown.
            let kinds: Vec<usize> = (0..KINDS.len()).collect();
            let (seed, looks, ivy_leaves) = (with.seed, with.looks, with.ivy_leaves);
            let grown_kinds = MeshSink::making(meshes, |sink| {
                terra_gen::par::map(&kinds, |&kind| {
                    grow(kind, seed, looks, Some(ivy_leaves), sink)
                })
            });
            let mut models = Vec::new();
            for (model, trunk, seen) in grown_kinds {
                shapes.remember(&model.wood[0], trunk);
                shapes.remember(&model.crown_near, seen);
                models.push(model.bare());
            }
            for n in 0..VETERANS.len() {
                models.push(veteran(n, with, meshes, shapes));
            }
            models
        }
    };
    grown.0.push((with.seed, models));
    if grown.0.len() > WORLDS_KEPT {
        grown.0.remove(0);
    }
    let mut trees: Vec<TreeModel> = grown
        .0
        .last()
        .map_or_else(Vec::new, |(_, models)| models.to_vec());
    for (kind, model) in trees.iter_mut().enumerate() {
        if kind < KINDS.len() {
            let (bark, species) = materials_of(kind);
            model.bark = with.looks.barks[bark].0.clone();
            model.far = with.looks.far_crowns[species].clone();
        } else {
            model.bark = veteran_bark(VETERANS[kind - KINDS.len()].2, with, materials);
            model.far = with.looks.far_crowns[0].clone();
        }
    }
    trees
}

impl TreeModel {
    /// Its meshes and measures alone, its materials left for each world to give it.
    fn bare(self) -> Self {
        Self {
            bark: Handle::default(),
            far: Handle::default(),
            ..self
        }
    }
}

/// A veteran oak's bark, `BARKS[which]`, laid on by where each point is.
fn veteran_bark(which: usize, with: &Grow, materials: &mut Assets<Foliage>) -> Handle<Foliage> {
    let (name, metres, (r, g, b), relief, moss) = BARKS[which];
    let bark = bark_material(
        with.assets,
        materials,
        with.wind,
        name,
        Color::srgb(r, g, b),
        relief / metres,
        moss,
    );
    if let Some(mut material) = materials.get_mut(&bark) {
        // From metres to the picture's repeats, as the shader projects it.
        material.base.uv_transform = bevy::math::Affine2::from_scale(Vec2::splat(1.0 / metres));
        // Not relief-mapped: its bark is read seven times a pixel, hex-tiled from three
        // sides (D294), and a parallax search in each read made a veteran filling the view
        // a 7.7 ms opaque pass, 16 to 17 ms a frame, on the RTX 3080 (D309). Its own
        // modelled bulk and the bark's normals carry its relief.
        material.base.depth_map = None;
    }
    bark
}

/// Veteran oak `n`: a trunk and limbs modelled (Tripo, from a concept drawn to the style
/// guide, art/props/oak_bough), far more gnarled and burred than a grown one can be, its
/// crown grown on it as cards at the clumps tools/blender/tips.py found along its outer
/// branches (D235). The model's own texture read as blue-black clay beside the grown
/// trunks, its baked shadows black: it is drawn in the grown oaks' bark instead (D264).
fn veteran(n: usize, with: &Grow, meshes: &mut Assets<Mesh>, shapes: &mut Shapes) -> TreeModel {
    let (model, tips, _) = VETERANS[n];
    let trunk = |file: String| -> Handle<Mesh> {
        with.assets.load(
            GltfAssetLabel::Primitive {
                mesh: 0,
                primitive: 0,
            }
            .from_asset(file),
        )
    };
    let near_trunk = trunk(format!("models/nature/forest/{model}.glb"));
    let far_trunk = trunk(format!("models/nature/forest/{model}_far.glb"));
    let tips: Vec<species::Tip> = terra_world::data::read(&format!("trees/{tips}.ron"));
    let tree = species::modelled(&tips, VETERAN_TRUNK_M);
    let mut dice = with.seed.derive("veteran").index(n as u64).rng();
    let mut cards = species::crown_cards(&mut dice, &tree, VETERAN_M, with.looks.leaf_of[0]);
    for card in cards.iter_mut().flatten() {
        card.tint *= super::warmed(Vec3::new(0.9, 0.95, 0.85));
        // Its trunk does not sway, so nor do its leaves much with it.
        card.sway.x *= 0.25;
    }
    // Its limbs are modelled, so each mass of its crown turns as a great limb from its fork
    // and each clump at the twig's level (D597's wind in levels), its lumps with them.
    let pivots = super::pivots::of(&tree);
    let tips: Vec<[[f32; 4]; 2]> = pivots
        .tips
        .iter()
        .zip(&pivots.twigs)
        .map(|(&limb, &twig)| [limb, twig])
        .collect();
    let near = crown_near(&cards, 2, &tips);
    info!(
        "lush: veteran oak {model}, {VETERAN_M:.0} m: {} clumps, crown {} near",
        tree.tips.len(),
        near.triangles()
    );
    TreeModel {
        wood: [near_trunk, far_trunk.clone(), far_trunk],
        crown_near: {
            let seen = SolidShape::crown(&near.positions, &near.indices);
            let mesh = meshes.add(near.mesh());
            shapes.remember(&mesh, seen);
            mesh
        },
        crown_far: meshes.add(crown_far(&cards, 2, &tips).mesh()),
        shadow: meshes.add(crown_stand_in(&cards, &tips).mesh()),
        clumps: meshes.add(crown_clumps(&tree, 1.0, false, &pivots.tips).mesh()),
        lumps: meshes.add(crown_clumps(&tree, SHADOW_LUMP, true, &pivots.tips).mesh()),
        shadow_lumps: Arc::new(sky_occlusion::Lumps::new(lumps_of(&tree, SHADOW_LUMP))),
        height: VETERAN_M,
        trunk_m: tree.trunk_m,
        crown: floor::crown_of(&tree.tips),
        skirt_m: 0.0,
        species: Species::Oak,
        young: false,
        bark: Handle::default(),
        far: Handle::default(),
        modelled: true,
        fine: None,
        ivy: Vec::new(),
    }
}
