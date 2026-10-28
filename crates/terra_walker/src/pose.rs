//! The layers that pose a body after its clips, the same in play (`WalkerPlugin`) and on the
//! model workbench, so a still from the workbench shows what the game shows: a carried look's
//! arm held clear of the body (`carry_arm`, D560), a tool carried by the rule tipped at the
//! wrist with its forearm kept down (`carry_hand`, D564), the arms kept off the hips (`arms`), the
//! fists closed on what they hold (`grip`) and the shield arm fitted to its board's place
//! (`shield_arm`, D554). One list both call, so the two cannot drift; asked twice, it lays them
//! once. The head's turn toward the facing (`gaze`) is play's alone: the workbench has no foe.

use bevy::prelude::*;

/// Marks an app whose pose layers are laid.
#[derive(Resource)]
pub struct PoseLayers;

pub fn plugin(app: &mut App) {
    if app.world().contains_resource::<PoseLayers>() {
        return;
    }
    app.insert_resource(PoseLayers);
    crate::carry_arm::plugin(app);
    crate::carry_hand::plugin(app);
    crate::arms::plugin(app);
    crate::grip::plugin(app);
    crate::shield_arm::plugin(app);
}

/// The names of the systems `schedule` runs in `app`, once it is built.
pub fn system_names(
    app: &mut App,
    schedule: impl bevy::ecs::schedule::ScheduleLabel,
) -> Vec<String> {
    let world = app.world_mut();
    world.schedule_scope(schedule, |world, schedule| {
        schedule.initialize(world).expect("the schedule builds");
        schedule
            .systems()
            .map(|systems| systems.map(|(_, s)| s.name().to_string()).collect())
            .unwrap_or_default()
    })
}

/// The systems the pose layers run, each in its own plugin.
pub const SYSTEMS: [&str; 5] = [
    "hold_in_body",
    "tip_carried",
    "keep_off_hips",
    "close_hands",
    "fit_shield_arm",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pose_layer_runs_once_however_often_it_is_asked_for() {
        let mut app = App::new();
        plugin(&mut app);
        plugin(&mut app);
        let names = system_names(&mut app, PostUpdate);
        for layer in SYSTEMS {
            let found = names
                .iter()
                .filter(|n| n.ends_with(&format!("::{layer}")))
                .count();
            assert_eq!(found, 1, "{layer} in {names:?}");
        }
    }
}
