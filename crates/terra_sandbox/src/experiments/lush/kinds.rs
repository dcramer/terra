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
    crown_stand_in, floor, forest_seed_of, grow_bare, ivy_leaves, leaf_of, lumps_of, species,
};
use bevy::prelude::*;
use std::sync::Arc;
use std::thread::JoinHandle;
use terra_gen::Seed;
use terra_world::grown::Cards;
use terra_world::instance::{Generating, WorldSpec};
use terra_world::loading::Screen;
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

/// Each grown kind's model without its materials, and what stops a walker at its trunk and
/// its crown.
type Kinds = Vec<(TreeModel, SolidShape, SolidShape)>;

/// The kinds of the world being entered, grown on a thread of their own while its land is
/// made behind the loading screen (D623), with the meshes to put in when the forest is
/// planted: grown in `plant` they were 368 to 448 ms of the frame a realm is first entered
/// in, the loading screen's ring standing still (portal-freeze j2).
#[derive(Resource)]
pub(super) struct Ahead {
    seed: Seed,
    thread: Option<JoinHandle<(Kinds, Vec<(AssetId<Mesh>, Mesh)>)>>,
}

/// Every grown kind at once, over every core, as the woods' conifers are grown.
fn grow_kinds(seed: Seed, leaf_of: &[Leaf; 3], ivy_leaves: &[Leaf; 3], sink: &MeshSink) -> Kinds {
    let kinds: Vec<usize> = (0..KINDS.len()).collect();
    terra_gen::par::map(&kinds, |&kind| {
        grow_bare(kind, seed, leaf_of, Some(ivy_leaves), sink)
    })
}

/// Starts growing the kinds of the world about to be entered, unless they are kept, where a
/// loading screen is drawn meanwhile; with none they are grown as the forest is planted.
pub(super) fn ahead(
    mut commands: Commands,
    spec: Option<Res<WorldSpec>>,
    screen: Option<Res<State<Screen>>>,
    grown: Res<Grown>,
    meshes: Res<Assets<Mesh>>,
    mut generating: ResMut<Generating>,
) {
    commands.remove_resource::<Ahead>();
    let (Some(spec), Some(_)) = (spec, screen) else {
        return;
    };
    let seed = forest_seed_of(Seed(spec.seed));
    if grown.0.iter().any(|(kept, _)| *kept == seed) {
        return;
    }
    let (handles, kind) = (meshes.get_handle_provider(), spec.kind.clone());
    let thread = std::thread::Builder::new()
        .name("terra-kinds".into())
        .spawn(move || {
            let began = std::time::Instant::now();
            let cards = Cards::of(&kind);
            let litter = Cards::read_file(terra_world::cover::litter::FILE);
            let sink = MeshSink::apart(handles);
            let kinds = grow_kinds(seed, &leaf_of(&cards), &ivy_leaves(&litter), &sink);
            info!(
                "lush: the kinds of tree grown in {:.2} s on their own thread, ahead of the \
                 forest's planting.",
                began.elapsed().as_secs_f32()
            );
            (kinds, sink.made())
        })
        .unwrap_or_else(|why| {
            panic!(
                "the thread growing the forest's kinds could not be started ({why}); the \
                 machine is out of threads or memory"
            )
        });
    commands.insert_resource(Ahead {
        seed,
        thread: Some(thread),
    });
    // The world is entered once they are grown, as once its land is made (D623): joined
    // in `plant` while still growing, they held the frame it is entered in all the same,
    // 207 ms of it with the land made ahead of the crossing.
    generating.begin(AHEAD);
}

/// What the world waits on while its kinds of tree are grown, by name.
const AHEAD: &str = "the forest's kinds of tree";

/// Says the kinds are grown once their thread has ended, so the world is entered.
pub(super) fn grown_ahead(ahead: Option<Res<Ahead>>, mut generating: ResMut<Generating>) {
    let growing = ahead
        .and_then(|ahead| ahead.thread.as_ref().map(JoinHandle::is_finished))
        .is_some_and(|finished| !finished);
    if !growing {
        generating.done(AHEAD);
    }
}

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
    (grown, ahead): (&mut Grown, Option<&mut Ahead>),
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
            let (seed, looks, ivy_leaves) = (with.seed, with.looks, with.ivy_leaves);
            let ahead = ahead
                .filter(|ahead| ahead.seed == seed)
                .and_then(|ahead| ahead.thread.take());
            let grown_kinds = match ahead {
                // Grown ahead, or nearly: waited for here, it is no longer than growing them.
                Some(thread) => {
                    // A panic growing them goes on on the main thread, as it did there.
                    let (kinds, made) = thread
                        .join()
                        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
                    MeshSink::put_in(made, meshes);
                    kinds
                }
                None => MeshSink::making(meshes, |sink| {
                    grow_kinds(seed, &looks.leaf_of, ivy_leaves, sink)
                }),
            };
            let mut models = Vec::new();
            for (model, trunk, seen) in grown_kinds {
                shapes.remember(&model.wood[0], trunk);
                shapes.remember(&model.crown_near, seen);
                models.push(model);
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
