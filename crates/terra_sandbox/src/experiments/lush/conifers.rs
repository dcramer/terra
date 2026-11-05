//! The woods' conifers (D366): on a world with mountains, terra_world's woods stand Scots
//! pines and Norway spruces where the land's rules say (`terra_world::woods`), and these are
//! what they are drawn as: the lush forest's own, grown from its seed in its materials, each
//! kind's variants (`KINDS`) with their levels of detail and shadow stand-ins, so the mountain
//! lake's pines are the lush forest's. The woods ask; the sandbox, above them, answers
//! (`Conifers`), as it answers the ground cover's question with the forest's map.
//!
//! A tree the land stunts below a tall stand's height, toward the tree line, is drawn as a
//! young spruce, as the lush forest's young conifers are; a tall one as the variant of its
//! kind nearest its height, scaled to it. They wear no ivy, which does not climb at the tree
//! line; a band of moss round the foot, as their kind's do.

use super::felled::{self, StumpLooks};
use super::overgrowth::{self, Overgrowth};
use super::species::Species;
use super::{
    CROWN_TINTS, DETAIL_FOR_M, FloorLooks, Foliage, KINDS, LEAF, LEAF_MEANS, Looks, TreeModel,
    draw, far_from, floor, forest_seed, grow, scale_detail, wind,
};
use bevy::prelude::*;
use std::sync::Arc;
use terra_world::detail::DetailScale;
use terra_world::flora_kinds::{CutRule, Flora};
use terra_world::grown::Cards;
use terra_world::instance::{Building, InWorld};
use terra_world::mesh_sink::MeshSink;
use terra_world::parting::ObstructionFade;
use terra_world::planet::Planet;
use terra_world::start::Start;
use terra_world::woods::{ConiferModels, Conifers, Drawn, Planted, Planting, TreeKind};

/// A conifer shorter than this is drawn as a young spruce: a tall stand's model scaled down
/// to it would be a pole with a tuft on top.
const YOUNG_UNDER_M: f32 = 9.0;

pub fn plugin(app: &mut App) {
    // The woods are the world layer's (`terra_world::woods`, in `WorldPlugin`), as are the grown
    // plants' material and the detail scale they are drawn with: with no world layer on (the
    // sandbox with no experiment) none are planted, so there is nothing to draw.
    if !app.is_plugin_added::<terra_world::world::WorldPlugin>() {
        return;
    }
    // Their material, its climate and its relief are grown plants' (`terra_world::grown`). In
    // each world entered, before its woods are planted (D469).
    app.add_systems(
        OnEnter(InWorld),
        grow_conifers.in_set(Building::View).before(Planting),
    )
    .add_systems(Update, follow_detail);
    // Their floor, laid by the forest walk's rules, and what stops a walker on it (D619): on
    // always, the forest walk's too.
    app.add_plugins((
        super::floor::shared,
        super::footing::plugin,
        super::woods_floor::plugin,
        felled::plugin,
    ));
}

/// The lush forest's conifers, grown, and which of them each kind of tree is drawn from;
/// and how an oak and a conifer are cut, as the flora's kinds say (`felled::cut_kind`).
struct LushConifers {
    models: Vec<TreeModel>,
    pools: Pools,
    looks: Looks,
    cuts: [Option<CutRule>; 2],
}

/// The models a tall oak, pine and spruce, and a young oak and spruce, are drawn from, each
/// by height.
struct Pools {
    oaks: Vec<(usize, f32)>,
    pines: Vec<(usize, f32)>,
    spruces: Vec<(usize, f32)>,
    young: Vec<(usize, f32)>,
    young_oaks: Vec<(usize, f32)>,
}

impl Pools {
    /// From each model's kind, whether young, and height.
    fn of(models: impl Iterator<Item = (Species, bool, f32)>) -> Self {
        let mut pools = Self {
            oaks: Vec::new(),
            pines: Vec::new(),
            spruces: Vec::new(),
            young: Vec::new(),
            young_oaks: Vec::new(),
        };
        for (index, (species, young, height)) in models.enumerate() {
            let pool = match (species, young) {
                (Species::Oak, true) => &mut pools.young_oaks,
                (_, true) => &mut pools.young,
                (Species::Pine, false) => &mut pools.pines,
                (Species::Spruce, false) => &mut pools.spruces,
                (Species::Oak, false) => &mut pools.oaks,
            };
            pool.push((index, height));
        }
        for pool in [
            &mut pools.oaks,
            &mut pools.pines,
            &mut pools.spruces,
            &mut pools.young,
            &mut pools.young_oaks,
        ] {
            pool.sort_by(|a, b| a.1.total_cmp(&b.1));
        }
        pools
    }

    /// Which model a tree is drawn from, and as which kind: its own kind's, or if it is too
    /// short for a tall one's a young oak's for an oak and a young spruce's for a conifer;
    /// the taller its luck makes it, the taller the model, so a model is scaled little.
    fn choose(&self, tree: &Planted) -> (usize, TreeKind) {
        let (pool, kind) = match (tree.kind, tree.height_m < YOUNG_UNDER_M) {
            (TreeKind::Oak, true) => (&self.young_oaks, TreeKind::Oak),
            (_, true) => (&self.young, TreeKind::Spruce),
            (TreeKind::Oak, false) => (&self.oaks, TreeKind::Oak),
            (TreeKind::Pine, false) => (&self.pines, TreeKind::Pine),
            (TreeKind::Spruce, false) => (&self.spruces, TreeKind::Spruce),
        };
        let at = ((tree.luck * pool.len() as f32) as usize).min(pool.len() - 1);
        (pool[at].0, kind)
    }
}

/// How a conifer is drawn from its model: which model, as which kind, how much it is scaled up
/// and across, and the height its levels' distances are for.
struct Shape {
    index: usize,
    kind: TreeKind,
    scale: f32,
    wide: f32,
    size: f32,
}

impl LushConifers {
    fn shape(&self, tree: &Planted) -> Shape {
        let (index, kind) = self.pools.choose(tree);
        let model = &self.models[index];
        let scale = tree.height_m / model.height;
        // Each a little stouter or more slender than the next, its crown with it (D276), by
        // where it stands; and squat toward its limit, as the woods say.
        let wide = scale * (0.85 + 0.33 * overgrowth::chance(by_foot(tree), 5.0)) * tree.squat;
        // In steps of a twentieth, so trees share their distances (D230).
        let size = ((tree.height_m / DETAIL_FOR_M).clamp(0.45, 1.4) * 20.0).round() / 20.0;
        Shape {
            index,
            kind,
            scale,
            wide,
            size,
        }
    }
}

/// Where a tree stands, as its own variations are drawn by.
fn by_foot(tree: &Planted) -> Vec2 {
    Vec2::new(tree.foot.x + tree.foot.y, tree.foot.z)
}

impl ConiferModels for LushConifers {
    /// Out to where its crown's cards end, the lush forest's own trees' distances; past them
    /// its far card stands for it, as a game's last level of a tree is its billboard. Its
    /// crown's lumps beyond, as the lush forest draws them to 420 m for a 20 m tree, made the
    /// woods' 17,000 trees some 60 million triangles a frame.
    fn drawn(&self, tree: &Planted) -> Drawn {
        let Shape {
            index,
            kind,
            scale,
            wide,
            size,
        } = self.shape(tree);
        let model = &self.models[index];
        // Its crown and trunk as drawn, for the floor under it (D619).
        Drawn {
            to_m: far_from(size),
            kind,
            crown_off: tree.rotation * (model.crown.0 * Vec3::new(wide, scale, wide)),
            crown_m: model.crown.1 * wide,
            trunk_m: model.trunk_m * wide,
            young: model.young,
            solid: true,
        }
    }

    fn draw(&self, entity: &mut EntityCommands, tree: &Planted) -> Drawn {
        let Shape {
            index,
            scale,
            wide,
            size,
            ..
        } = self.shape(tree);
        let model = &self.models[index];
        let up = tree.foot.normalize();
        let by_foot = by_foot(tree);
        let place = Transform {
            // Sunk a little, so the root flare meets uneven ground.
            translation: tree.foot - up * 0.3,
            rotation: tree.rotation,
            scale: Vec3::new(wide, scale, wide),
        };
        // Felled as the forest walk's trees are, stopping a walker at its trunk till then.
        let cut = self.cuts[usize::from(model.species != Species::Oak)];
        entity.insert((
            place,
            felled::standing(model, cut.map(|rule| rule.at(tree.height_m)), &place),
        ));
        // A young tree is a bush to the camera, as the forest walk's are (D274).
        if model.young {
            entity.insert(ObstructionFade::default());
        }
        let grows = Overgrowth::of(model.species);
        let moss_tag = if !model.young && overgrowth::chance(by_foot, 3.0) < grows.moss {
            let (low, high) = grows.moss_m;
            overgrowth::moss_tag(
                low + (high - low) * overgrowth::chance(by_foot, 4.0),
                model.height,
            )
        } else {
            0
        };
        // Without its lumps (`drawn`).
        entity.with_children(|children| {
            draw(children, model, &self.looks, size, false, moss_tag, None);
        });
        self.drawn(tree)
    }

    /// As the lush forest's far forest colours its oaks, pines and spruces.
    fn far_crowns(&self) -> [Vec3; 3] {
        [0, 1, 2].map(|k| LEAF_MEANS[k] * CROWN_TINTS[k] * (1.0 - LEAF.y))
    }
}

/// The conifers' materials whose distances follow how far detail reaches (D230).
#[derive(Resource)]
struct Dropping {
    leaves: Handle<Foliage>,
    needles: Handle<Foliage>,
    stand_in: Handle<Foliage>,
    lumps: Handle<Foliage>,
}

/// Grows the lush forest's pines and spruces for the woods, where there are woods: on a world
/// with mountains, with the world's grown plants on.
fn grow_conifers(
    mut commands: Commands,
    planet: Res<Planet>,
    start: Res<Start>,
    cards: Option<Res<Cards>>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Foliage>>,
    mut plain: ResMut<Assets<StandardMaterial>>,
    parting: Option<Res<terra_world::parting::Parting>>,
    flora: Option<Res<Flora>>,
) {
    let Some(cards) = cards else {
        return;
    };
    if planet.round().is_none() {
        return;
    }
    let began = std::time::Instant::now();
    let wind = wind(
        start.translation.normalize().any_orthonormal_vector(),
        parting.map(|parting| parting.0.clone()).unwrap_or_default(),
    );
    let looks = Looks::new(&assets, &mut materials, &mut plain, &cards, &wind);
    let seed = forest_seed(&planet);
    // Every kind at once, over every core: one after the next they were 2.1 s of the frame
    // the world is entered in (crossing-freeze j1).
    let kinds: Vec<usize> = (0..KINDS.len()).collect();
    let models: Vec<TreeModel> = MeshSink::making(&mut meshes, |sink| {
        terra_gen::par::map(&kinds, |&kind| grow(kind, seed, &looks, None, sink).0)
    });
    let pools = Pools::of(models.iter().map(|m| (m.species, m.young, m.height)));
    info!(
        "Woods: the lush forest's trees for the woods, {} oaks, {} pines, {} spruces, {} young spruces and {} young oaks, in {:.1} s",
        pools.oaks.len(),
        pools.pines.len(),
        pools.spruces.len(),
        pools.young.len(),
        pools.young_oaks.len(),
        began.elapsed().as_secs_f32()
    );
    commands.insert_resource(Dropping {
        leaves: looks.dropping_leaves.clone(),
        needles: looks.dropping_needles.clone(),
        stand_in: looks.stand_in_leaves.clone(),
        lumps: looks.shadow_lumps.clone(),
    });
    let floor_looks = FloorLooks::new(&assets, &mut materials, &wind, &looks);
    commands.insert_resource(StumpLooks::of(
        &floor_looks,
        floor::end_grain(&assets, &mut plain),
    ));
    commands.insert_resource(floor_looks);
    let cuts = [Species::Oak, Species::Pine].map(|species| {
        flora
            .as_ref()
            .and_then(|flora| flora.cut_rule(felled::cut_kind(species)))
    });
    commands.insert_resource(Conifers(Arc::new(LushConifers {
        models,
        pools,
        looks,
        cuts,
    })));
}

/// The conifers' cards drop out, and their shadows hand over, at distances scaled as every
/// other level of detail is (D230), as the lush forest's own do.
fn follow_detail(
    detail: Option<Res<DetailScale>>,
    dropping: Option<Res<Dropping>>,
    mut materials: ResMut<Assets<Foliage>>,
) {
    let (Some(detail), Some(dropping)) = (detail, dropping) else {
        return;
    };
    if !(detail.is_changed() || dropping.is_added()) {
        return;
    }
    scale_detail(
        [&dropping.leaves, &dropping.needles],
        [&dropping.stand_in, &dropping.lumps],
        detail.0,
        &mut materials,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pools() -> Pools {
        Pools::of(
            KINDS
                .iter()
                .map(|&(species, _, young, (low, high))| (species, young, (low + high) / 2.0)),
        )
    }

    fn tree(kind: TreeKind, height_m: f32, luck: f32) -> Planted {
        Planted {
            foot: Vec3::new(0.0, 30_000.0, 0.0),
            rotation: Quat::IDENTITY,
            kind,
            height_m,
            squat: 1.0,
            luck,
        }
    }

    #[test]
    fn the_woods_pines_are_the_lush_forests_pines_and_their_spruces_its_spruces() {
        let pools = pools();
        let kinds: Vec<(Species, bool)> = KINDS.iter().map(|kind| (kind.0, kind.2)).collect();
        assert!(pools.pines.len() >= 4 && pools.spruces.len() >= 3 && pools.young.len() >= 2);
        for k in 0..50 {
            let luck = k as f32 / 49.0;
            let (pine, as_kind) = pools.choose(&tree(TreeKind::Pine, 28.0, luck));
            assert_eq!(
                (kinds[pine], as_kind),
                ((Species::Pine, false), TreeKind::Pine)
            );
            let (spruce, as_kind) = pools.choose(&tree(TreeKind::Spruce, 24.0, luck));
            assert_eq!(
                (kinds[spruce], as_kind),
                ((Species::Spruce, false), TreeKind::Spruce)
            );
            let (oak, as_kind) = pools.choose(&tree(TreeKind::Oak, 22.0, luck));
            assert_eq!(
                (kinds[oak], as_kind),
                ((Species::Oak, false), TreeKind::Oak)
            );
            let (sapling, as_kind) = pools.choose(&tree(TreeKind::Oak, 6.0, luck));
            assert_eq!(
                (kinds[sapling], as_kind),
                ((Species::Oak, true), TreeKind::Oak)
            );
            // Stunted toward the tree line, either kind is a young spruce.
            for kind in [TreeKind::Pine, TreeKind::Spruce] {
                let (young, as_kind) = pools.choose(&tree(kind, 5.0, luck));
                assert_eq!(
                    (kinds[young], as_kind),
                    ((Species::Spruce, true), TreeKind::Spruce)
                );
            }
        }
        // Every variant of a kind is drawn somewhere, a taller tree from a taller one.
        let chosen: Vec<usize> = (0..200)
            .map(|k| {
                pools
                    .choose(&tree(TreeKind::Pine, 28.0, k as f32 / 199.0))
                    .0
            })
            .collect();
        for (index, _) in &pools.pines {
            assert!(chosen.contains(index));
        }
        let height = |index: usize| pools.pines.iter().find(|p| p.0 == index).unwrap().1;
        assert!(chosen.windows(2).all(|w| height(w[0]) <= height(w[1])));
    }
}
