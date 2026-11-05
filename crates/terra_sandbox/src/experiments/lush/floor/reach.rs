//! How far the undergrowth is drawn, as one tunable scale over each plant's own distances
//! (`floor.undergrowth_reach`), as Unreal's foliage scalability scales every foliage type's
//! cull distances (`foliage.LODDistanceScale`): so a probe measures what the undergrowth costs
//! at a shorter reach in one load (D609), and at 0 what it costs at all.

use bevy::camera::visibility::VisibilityRange;
use bevy::prelude::*;
use terra_gen::tune::Tunable;

/// Every plant of the undergrowth is drawn to its own distances times this; at 0 none is.
pub(super) static REACH: Tunable = Tunable::new(
    "floor.undergrowth_reach",
    1.0,
    0.0..=2.0,
    "the undergrowth's draw distances as a share of each plant's own; 0 draws none",
);

/// A part of the undergrowth's distances as laid, before the reach.
#[derive(Component, Clone)]
pub(super) struct Laid(pub VisibilityRange);

pub(super) fn plugin(app: &mut App) {
    terra_gen::tune::register(&[&REACH]);
    app.add_systems(Update, follow);
}

/// When the reach changes, every part of the undergrowth is given its distances times it, and
/// a part laid since, while it is not 1, at once.
fn follow(
    mut commands: Commands,
    mut last: Local<Option<f32>>,
    parts: Query<(Entity, &Laid)>,
    new: Query<(Entity, &Laid), Added<Laid>>,
) {
    let reach = REACH.get();
    // As laid until it is first changed.
    let was = last.unwrap_or(1.0);
    *last = Some(reach);
    let changed: Vec<(Entity, &Laid)> = if was != reach {
        parts.iter().collect()
    } else if reach != 1.0 {
        new.iter().collect()
    } else {
        return;
    };
    for (entity, laid) in changed {
        let mut entity = commands.entity(entity);
        if reach <= 0.0 {
            entity.insert(Visibility::Hidden);
            continue;
        }
        entity.insert(Visibility::Inherited);
        terra_world::detail::set_range(&mut entity, scaled(&laid.0, reach));
    }
}

fn scaled(range: &VisibilityRange, by: f32) -> VisibilityRange {
    VisibilityRange {
        start_margin: range.start_margin.start * by..range.start_margin.end * by,
        end_margin: range.end_margin.start * by..range.end_margin.end * by,
        use_aabb: range.use_aabb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_gen::tune::SetBy;

    /// A plant laid to 80 m is drawn to 40 at half the reach, not at all at none, and to its
    /// own 80 again at the default; one laid while the reach is short is drawn short at once.
    #[test]
    fn the_reach_scales_each_plants_own_distances() {
        let mut app = App::new();
        app.add_systems(Update, follow);
        let range = VisibilityRange {
            start_margin: 0.0..0.0,
            end_margin: 64.0..80.0,
            use_aabb: false,
        };
        let plant = app
            .world_mut()
            .spawn((Laid(range.clone()), range.clone(), Visibility::Inherited))
            .id();
        let end = |app: &App, plant| {
            app.world()
                .get::<VisibilityRange>(plant)
                .unwrap()
                .end_margin
                .end
        };
        let shown =
            |app: &App, plant| app.world().get::<Visibility>(plant) == Some(&Visibility::Inherited);
        app.update();
        assert_eq!(end(&app, plant), 80.0);
        REACH.set(0.5, SetBy::CommandLine).unwrap();
        app.update();
        assert_eq!(end(&app, plant), 40.0);
        let late = app.world_mut().spawn((Laid(range.clone()), range)).id();
        app.update();
        assert_eq!(end(&app, late), 40.0, "laid while the reach is short");
        REACH.set(0.0, SetBy::CommandLine).unwrap();
        app.update();
        assert!(!shown(&app, plant));
        REACH.reset();
        app.update();
        assert!(shown(&app, plant));
        assert_eq!(end(&app, plant), 80.0);
    }
}
