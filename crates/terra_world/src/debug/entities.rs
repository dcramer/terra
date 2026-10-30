//! The readout's count of entities, taken only while F3 shows it. Bevy's count walks every
//! entity slot a frame (about 0.6 ms in the town at dusk, 190k slots), so it waits for the
//! line it feeds: `Diagnostics::add_measurement` skips a disabled diagnostic's closure.

use super::Detail;
use bevy::{
    diagnostic::{DiagnosticsStore, EntityCountDiagnosticsPlugin},
    prelude::*,
};

pub fn plugin(app: &mut App) {
    app.add_plugins(EntityCountDiagnosticsPlugin::default())
        .init_resource::<Detail>()
        .add_systems(First, count_while_shown);
}

/// Turns the count on with the readout's detail and off with it.
fn count_while_shown(detail: Res<Detail>, mut store: ResMut<DiagnosticsStore>) {
    if !detail.is_changed() {
        return;
    }
    if let Some(count) = store.get_mut(&EntityCountDiagnosticsPlugin::ENTITY_COUNT) {
        count.is_enabled = detail.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::diagnostic::DiagnosticsPlugin;

    fn counted(app: &App) -> Option<f64> {
        app.world()
            .resource::<DiagnosticsStore>()
            .get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT)
            .and_then(|d| d.value())
    }

    #[test]
    fn entities_are_counted_only_while_the_detail_shows() {
        let mut app = App::new();
        app.add_plugins((DiagnosticsPlugin, plugin));
        app.world_mut()
            .spawn_batch((0..5).map(|_| Transform::default()));
        app.update();
        assert_eq!(counted(&app), None, "counted with the detail hidden");
        app.world_mut().resource_mut::<Detail>().0 = true;
        app.update();
        assert!(
            counted(&app).is_some_and(|n| n >= 5.0),
            "{:?}",
            counted(&app)
        );
    }
}
