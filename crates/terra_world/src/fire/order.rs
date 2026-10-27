//! The fires' cards drawn in one order every run (D472). Every card mesh stands at the world's
//! origin, its depth on the GPU, so the transparent pass's sort by distance ties all those of a
//! sort bias (flames, hearth flames, embers, hearth embers and drops at 1.0). Bevy 0.20's
//! `Transparent3d::sort` is stable over a retained `IndexMap` whose removals are
//! `swap_remove`, so tied cards keep whatever order the map was left in, and that moves each
//! time some other transparent leaves the phase, as loading decides, so two runs of one build
//! may draw the flames, the masks and the sparks over one another in different orders. It was
//! not what made the motion gate's two films differ (D472's A/A, determinism round 3: 123 of
//! 359 mattes differed without it, 124 with), but storage order is no draw order to rely on.
//! So each card carries its [`Rank`], its place in spawning, and the
//! phase is sorted again after Bevy's sort, before batching, by distance and then rank, as
//! Unreal's translucency sort priority and Godot's `render_priority` break a tie by a number
//! the author gives, never by storage order.

use bevy::{
    core_pipeline::core_3d::Transparent3d,
    math::FloatOrd,
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        render_phase::{SortedRenderPhase, ViewSortedRenderPhases, sort_phase_system},
        sync_world::{MainEntity, MainEntityHashMap},
    },
};

/// A card's place among the fires' cards: ties in the transparent sort go lowest first.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Rank(pub u32);

/// Each ranked card's rank, by its main entity, this frame.
#[derive(Resource, Default)]
struct Ranks(MainEntityHashMap<u32>);

pub(super) fn plugin(app: &mut App) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .init_resource::<Ranks>()
        .add_systems(ExtractSchedule, extract)
        .add_systems(
            Render,
            resort
                .in_set(RenderSystems::PhaseSort)
                .after(sort_phase_system::<Transparent3d>),
        );
}

fn extract(mut ranks: ResMut<Ranks>, cards: Extract<Query<(Entity, &Rank)>>) {
    ranks.0.clear();
    ranks
        .0
        .extend(cards.iter().map(|(e, r)| (MainEntity::from(e), r.0)));
}

fn resort(ranks: Res<Ranks>, mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>) {
    if ranks.0.is_empty() {
        return;
    }
    for phase in phases.values_mut() {
        order(phase, &ranks.0);
    }
}

/// Sorts a sorted phase again, by distance and then rank: an unranked item before a ranked
/// one it ties with, unranked items among themselves as they stood (the sort is stable).
pub(super) fn order(phase: &mut SortedRenderPhase<Transparent3d>, ranks: &MainEntityHashMap<u32>) {
    let rank = |item: &Transparent3d| ranks.get(&item.entity.1).copied();
    phase.items.sort_by(|_, a, _, b| {
        FloatOrd(a.distance)
            .cmp(&FloatOrd(b.distance))
            .then_with(|| rank(a).cmp(&rank(b)))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        core_pipeline::core_3d::TransparentSortingInfo3d,
        render::render_phase::{DrawFunctionId, PhaseItemExtraIndex},
        render::render_resource::CachedRenderPipelineId,
    };

    fn card(entity: Entity, distance: f32) -> Transparent3d {
        Transparent3d {
            sorting_info: TransparentSortingInfo3d::AlwaysOnTop,
            distance,
            pipeline: CachedRenderPipelineId::INVALID,
            entity: (entity, MainEntity::from(entity)),
            draw_function: DrawFunctionId(0),
            batch_range: 0..1,
            extra_index: PhaseItemExtraIndex::None,
            indexed: true,
        }
    }

    /// A phase sorted as Bevy sorts it, then by rank, its items' entities in draw order.
    fn drawn(items: &[(Entity, f32)], ranks: &MainEntityHashMap<u32>) -> Vec<Entity> {
        let mut phase = SortedRenderPhase::<Transparent3d>::default();
        for &(e, d) in items {
            phase.add_retained(card(e, d));
        }
        phase.sort();
        order(&mut phase, ranks);
        phase.items.values().map(|i| i.entity.0).collect()
    }

    #[test]
    fn tied_cards_draw_in_rank_order_whatever_order_the_phase_held_them_in() {
        let mut world = World::new();
        let [lake, flames, embers, drops, mask] = [(); 5].map(|_| world.spawn_empty().id());
        let ranks: MainEntityHashMap<u32> = [(flames, 0), (embers, 3), (drops, 7), (mask, 9)]
            .into_iter()
            .map(|(e, r)| (MainEntity::from(e), r))
            .collect();
        // The four cards tie at one distance; the lake stands further off (drawn first).
        let one = [
            (lake, -5.0),
            (flames, 1.0),
            (embers, 1.0),
            (drops, 1.0),
            (mask, 1.0),
        ];
        let other = [
            (drops, 1.0),
            (mask, 1.0),
            (lake, -5.0),
            (embers, 1.0),
            (flames, 1.0),
        ];
        let bevy_alone = [one, other].map(|items| {
            let mut phase = SortedRenderPhase::<Transparent3d>::default();
            for (e, d) in items {
                phase.add_retained(card(e, d));
            }
            phase.sort();
            phase.items.values().map(|i| i.entity.0).collect::<Vec<_>>()
        });
        // Bevy's sort alone keeps the held order among ties: the premise.
        assert_ne!(bevy_alone[0], bevy_alone[1]);
        assert_eq!(drawn(&one, &ranks), vec![lake, flames, embers, drops, mask]);
        assert_eq!(drawn(&other, &ranks), drawn(&one, &ranks));
        // Distance still comes first: a nearer card draws last whatever its rank.
        let nearer = [(flames, 2.0), (embers, 1.0), (mask, 1.0)];
        assert_eq!(drawn(&nearer, &ranks), vec![embers, mask, flames]);
    }
}
