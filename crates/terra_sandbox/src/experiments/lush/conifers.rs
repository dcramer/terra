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

use super::overgrowth::{self, Overgrowth};
use super::species::Species;
use super::{
    CROWN_TINTS, DETAIL_FOR_M, Foliage, KINDS, LEAF, LEAF_MEANS, Looks, TreeModel, draw,
    forest_seed, grow, scale_detail, wind,
};
use bevy::prelude::*;
use std::sync::Arc;
use terra_world::detail::DetailScale;
use terra_world::grown::Cards;
use terra_world::planet::Planet;
use terra_world::start::Start;
use terra_world::woods::{Conifer, ConiferModels, Conifers, Drawn, Planted, Planting};

/// A conifer shorter than this is drawn as a young spruce: a tall stand's model scaled down
/// to it would be a pole with a tuft on top.
const YOUNG_UNDER_M: f32 = 9.0;

pub fn plugin(app: &mut App) {
    // Unless the lush forest is on too, and brought them: its materials, and what keeps them
    // up with the climate and the quality preset.
    if !app.is_plugin_added::<MaterialPlugin<Foliage>>() {
        app.add_plugins(MaterialPlugin::<Foliage>::default())
            .add_systems(
                Update,
                super::seasons::follow_climate.run_if(resource_exists::<Cards>),
            )
            .add_systems(
                PostUpdate,
                terra_world::quality::follow_relief::<Foliage>.run_if(resource_exists::<Conifers>),
            );
    }
    app.add_systems(PostStartup, grow_conifers.before(Planting))
        .add_systems(Update, follow_detail);
}

/// The lush forest's conifers, grown, and which of them each kind of tree is drawn from.
struct LushConifers {
    models: Vec<TreeModel>,
    pools: Pools,
    looks: Looks,
}

/// The models a tall pine, a tall spruce and a young spruce are drawn from, each by height.
struct Pools {
    pines: Vec<(usize, f32)>,
    spruces: Vec<(usize, f32)>,
    young: Vec<(usize, f32)>,
}

impl Pools {
    /// From each model's kind, whether young, and height.
    fn of(models: impl Iterator<Item = (Species, bool, f32)>) -> Self {
        let mut pools = Self {
            pines: Vec::new(),
            spruces: Vec::new(),
            young: Vec::new(),
        };
        for (index, (species, young, height)) in models.enumerate() {
            let pool = match (species, young) {
                (_, true) => &mut pools.young,
                (Species::Pine, false) => &mut pools.pines,
                (Species::Spruce, false) => &mut pools.spruces,
                (Species::Oak, false) => continue,
            };
            pool.push((index, height));
        }
        for pool in [&mut pools.pines, &mut pools.spruces, &mut pools.young] {
            pool.sort_by(|a, b| a.1.total_cmp(&b.1));
        }
        pools
    }

    /// Which model a tree is drawn from, and as which kind: its own kind's, or a young
    /// spruce's if it is too short for a tall one's; the taller its luck makes it, the taller
    /// the model, so a model is scaled little.
    fn choose(&self, tree: &Planted) -> (usize, Conifer) {
        let (pool, kind) = if tree.height_m < YOUNG_UNDER_M {
            (&self.young, Conifer::Spruce)
        } else {
            match tree.kind {
                Conifer::Pine => (&self.pines, Conifer::Pine),
                Conifer::Spruce => (&self.spruces, Conifer::Spruce),
            }
        };
        let at = ((tree.luck * pool.len() as f32) as usize).min(pool.len() - 1);
        (pool[at].0, kind)
    }
}

impl ConiferModels for LushConifers {
    fn draw(&self, entity: &mut EntityCommands, tree: &Planted) -> Drawn {
        let (index, kind) = self.pools.choose(tree);
        let model = &self.models[index];
        let scale = tree.height_m / model.height;
        let up = tree.foot.normalize();
        // Each a little stouter or more slender than the next, its crown with it (D276), by
        // where it stands; and squat toward its limit, as the woods say.
        let by_foot = Vec2::new(tree.foot.x + tree.foot.y, tree.foot.z);
        let wide = scale * (0.85 + 0.33 * overgrowth::chance(by_foot, 5.0)) * tree.squat;
        entity.insert(Transform {
            // Sunk a little, so the root flare meets uneven ground.
            translation: tree.foot - up * 0.3,
            rotation: tree.rotation,
            scale: Vec3::new(wide, scale, wide),
        });
        // In steps of a twentieth, so trees share their distances (D230).
        let size = ((tree.height_m / DETAIL_FOR_M).clamp(0.45, 1.4) * 20.0).round() / 20.0;
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
        // Out to where its crown's cards end, the lush forest's own trees' distances; past
        // them its far card stands for it, as a game's last level of a tree is its billboard.
        // Its crown's lumps beyond, as the lush forest draws them to 420 m for a 20 m tree,
        // made the woods' 17,000 trees some 60 million triangles a frame.
        let mut to_m = 0.0;
        entity.with_children(|children| {
            to_m = draw(children, model, &self.looks, size, false, moss_tag, None);
        });
        Drawn { to_m, kind }
    }

    /// As the lush forest's far forest colours its pines and spruces.
    fn far_crowns(&self) -> [Vec3; 2] {
        [1, 2].map(|k| LEAF_MEANS[k] * CROWN_TINTS[k] * (1.0 - LEAF.y))
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
    let seed = forest_seed();
    let models: Vec<TreeModel> = (0..KINDS.len())
        .filter(|&kind| KINDS[kind].0 != Species::Oak)
        .map(|kind| grow(kind, seed, &looks, None, &mut meshes).0)
        .collect();
    let pools = Pools::of(models.iter().map(|m| (m.species, m.young, m.height)));
    info!(
        "Woods: the lush forest's conifers for the woods, {} pines, {} spruces and {} young spruces, in {:.1} s",
        pools.pines.len(),
        pools.spruces.len(),
        pools.young.len(),
        began.elapsed().as_secs_f32()
    );
    commands.insert_resource(Dropping {
        leaves: looks.dropping_leaves.clone(),
        needles: looks.dropping_needles.clone(),
        stand_in: looks.stand_in_leaves.clone(),
        lumps: looks.shadow_lumps.clone(),
    });
    commands.insert_resource(Conifers(Arc::new(LushConifers {
        models,
        pools,
        looks,
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
                .filter(|kind| kind.0 != Species::Oak)
                .map(|&(species, _, young, (low, high))| (species, young, (low + high) / 2.0)),
        )
    }

    fn tree(kind: Conifer, height_m: f32, luck: f32) -> Planted {
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
        let kinds: Vec<(Species, bool)> = KINDS
            .iter()
            .filter(|kind| kind.0 != Species::Oak)
            .map(|kind| (kind.0, kind.2))
            .collect();
        assert!(pools.pines.len() >= 4 && pools.spruces.len() >= 3 && pools.young.len() >= 2);
        for k in 0..50 {
            let luck = k as f32 / 49.0;
            let (pine, as_kind) = pools.choose(&tree(Conifer::Pine, 28.0, luck));
            assert_eq!(
                (kinds[pine], as_kind),
                ((Species::Pine, false), Conifer::Pine)
            );
            let (spruce, as_kind) = pools.choose(&tree(Conifer::Spruce, 24.0, luck));
            assert_eq!(
                (kinds[spruce], as_kind),
                ((Species::Spruce, false), Conifer::Spruce)
            );
            // Stunted toward the tree line, either kind is a young spruce.
            for kind in [Conifer::Pine, Conifer::Spruce] {
                let (young, as_kind) = pools.choose(&tree(kind, 5.0, luck));
                assert_eq!(
                    (kinds[young], as_kind),
                    ((Species::Spruce, true), Conifer::Spruce)
                );
            }
        }
        // Every variant of a kind is drawn somewhere, a taller tree from a taller one.
        let chosen: Vec<usize> = (0..200)
            .map(|k| pools.choose(&tree(Conifer::Pine, 28.0, k as f32 / 199.0)).0)
            .collect();
        for (index, _) in &pools.pines {
            assert!(chosen.contains(index));
        }
        let height = |index: usize| pools.pines.iter().find(|p| p.0 == index).unwrap().1;
        assert!(chosen.windows(2).all(|w| height(w[0]) <= height(w[1])));
    }
}
