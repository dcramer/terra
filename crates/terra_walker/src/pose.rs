//! The layers that pose a body after its clips, the same in play (`WalkerPlugin`) and on the
//! model workbench, so a still from the workbench shows what the game shows: an upright carry
//! pose's arm held in the body's frame (`carry_arm`, D560), the spear and the bow carried by
//! the rule tipped at the wrist (`carry_hand`, D564), the
//! fists closed on what they hold (`grip`) and the shield arm fitted to its board's place
//! (`shield_arm`, D554). One list both call, so the two cannot drift; asked twice, it lays them
//! once. The head's turn toward the facing (`gaze`) is play's alone: the workbench has no foe.
//!
//! Every layer that changes a body after its clips is in its [`Layer`] set, and one switch
//! turns them off by name for a look at what each does (`--pose-layers`, a debug tool, D71,
//! D584): [`Switch`], read by one run condition on every set, configured here.

use bevy::prelude::*;

/// Marks an app whose pose layers are laid.
#[derive(Resource)]
pub struct PoseLayers;

/// A layer that changes a body after its clips, as `--pose-layers` names it. Its systems are
/// in its set, which runs while [`Switch`] has it on. Not among them, as they are not after
/// the clip: the hold layer (a stance's arms laid over the clips by a mask, part of what the
/// clips play), the stride's narrowing (D360, baked into the clips) and a blow's aim
/// (D573, the root turned); nor what moves the body or a look but no bone (the step's
/// smoothing, a held tool's tilt, the shield on its strap, loose things swinging).
#[derive(bevy::ecs::schedule::SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Layer {
    CarryArm,
    Joints,
    Lift,
    CarryHand,
    Grip,
    Feet,
    ShieldArm,
    Gaze,
    Swim,
}

impl Layer {
    /// Every layer, in the order they pose a body.
    pub const ALL: [Layer; 9] = [
        Layer::CarryArm,
        Layer::Joints,
        Layer::Lift,
        Layer::CarryHand,
        Layer::Grip,
        Layer::Feet,
        Layer::ShieldArm,
        Layer::Gaze,
        Layer::Swim,
    ];

    /// Its name, as `--pose-layers` and the bone audit say it.
    pub const fn name(self) -> &'static str {
        match self {
            Layer::CarryArm => "carry_arm",
            Layer::Joints => "joints",
            Layer::Lift => "lift",
            Layer::CarryHand => "carry_hand",
            Layer::Grip => "grip",
            Layer::Feet => "feet",
            Layer::ShieldArm => "shield_arm",
            Layer::Gaze => "gaze",
            Layer::Swim => "swim",
        }
    }

    /// What it does, and its system.
    pub const fn what(self) -> &'static str {
        match self {
            Layer::CarryArm => {
                "an upright carry pose's arm held in the body's frame (D560, hold_in_body)"
            }
            Layer::Joints => "the joints moved to the body's own proportions (a fit, hold_joints)",
            Layer::Lift => {
                "a thicker body lifted to meet the floor as the clip's (a fit, lift_bodies)"
            }
            Layer::CarryHand => {
                "a spear or bow tipped at the wrist, its forearm kept down (D564, tip_carried)"
            }
            Layer::Grip => "the fingers closed on what the hand holds (hand IK, close_hands)",
            Layer::Feet => "the foot lock and foot placement (D455, feet::keep)",
            Layer::ShieldArm => "the shield arm fitted to its board's place (D554, fit_shield_arm)",
            Layer::Gaze => "the head and neck turned toward the aim (look-at, keep_on_aim)",
            Layer::Swim => "a swimmer's whole figure laid along its stroke (D478, swim::lay)",
        }
    }

    /// The body's fit to its own shape rather than a pose: `none` leaves it on.
    pub const fn fits(self) -> bool {
        matches!(self, Layer::Joints | Layer::Lift)
    }

    const fn bit(self) -> u16 {
        1 << self as u16
    }
}

/// Which layers are off (`--pose-layers`, D71): by default none. Fixed for a run: a layer
/// turned on again mid-run would start from whatever it last wrote.
#[derive(Resource, Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Switch {
    off: u16,
}

impl Switch {
    /// These layers off, the rest on.
    pub fn off(layers: impl IntoIterator<Item = Layer>) -> Self {
        Self {
            off: layers.into_iter().fold(0, |off, layer| off | layer.bit()),
        }
    }

    /// Whether `layer` runs.
    pub fn on(self, layer: Layer) -> bool {
        self.off & layer.bit() == 0
    }

    /// The layers off, by name.
    pub fn names_off(self) -> Vec<&'static str> {
        Layer::ALL
            .into_iter()
            .filter(|&layer| !self.on(layer))
            .map(Layer::name)
            .collect()
    }

    /// From `--pose-layers`' value: `all` (as shipped), `none` (every pose layer off, the
    /// body's fit kept), `off=<name>,…` or `only=<name>,…`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let names = |list: &str| -> Result<Vec<Layer>, String> {
            list.split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(|name| {
                    Layer::ALL
                        .into_iter()
                        .find(|layer| layer.name() == name)
                        .ok_or_else(|| format!("no pose layer is named `{name}`.\n{}", help()))
                })
                .collect()
        };
        match text.trim() {
            "all" => Ok(Self::default()),
            "none" => Ok(Self::off(Layer::ALL.into_iter().filter(|l| !l.fits()))),
            text => match text.split_once('=') {
                Some(("off", list)) => Ok(Self::off(names(list)?)),
                Some(("only", list)) => {
                    let kept = names(list)?;
                    Ok(Self::off(
                        Layer::ALL.into_iter().filter(|l| !kept.contains(l)),
                    ))
                }
                _ => Err(format!(
                    "--pose-layers is all, none, off=<name>,… or only=<name>,…, not `{text}`.\n{}",
                    help()
                )),
            },
        }
    }
}

/// The layers `--pose-layers` names, a line each with what it does.
pub fn help() -> String {
    let mut text = String::from("The pose layers, in the order they pose a body:\n");
    for layer in Layer::ALL {
        text += &format!("  {:<11} {}\n", layer.name(), layer.what());
    }
    text
}

/// The run condition on `layer`'s set: on unless the switch turned it off.
fn runs(layer: Layer) -> impl FnMut(Option<Res<Switch>>) -> bool + Clone {
    move |switch: Option<Res<Switch>>| switch.is_none_or(|switch| switch.on(layer))
}

/// The switch from the run's options, and the one run condition on every layer's set, in
/// each schedule a layer runs in.
fn switch(app: &mut App) {
    let asked = app
        .world()
        .get_resource::<terra_world::shell::Options>()
        .and_then(|options| options.pose_layers.clone());
    let switch = asked.map_or_else(Switch::default, |text| {
        Switch::parse(&text).unwrap_or_else(|problem| panic!("--pose-layers: {problem}"))
    });
    if switch != Switch::default() {
        info!(
            "Pose layers off (--pose-layers, a debug switch): {}",
            switch.names_off().join(", ")
        );
    }
    configure(app, switch);
}

/// `switch` in `app`, and the run condition on every layer's set.
pub(crate) fn configure(app: &mut App, switch: Switch) {
    app.insert_resource(switch);
    for layer in Layer::ALL {
        app.configure_sets(PostUpdate, layer.run_if(runs(layer)))
            .configure_sets(Update, layer.run_if(runs(layer)));
    }
}

pub fn plugin(app: &mut App) {
    if app.world().contains_resource::<PoseLayers>() {
        return;
    }
    app.insert_resource(PoseLayers);
    switch(app);
    crate::carry_arm::plugin(app);
    crate::carry_hand::plugin(app);
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
pub const SYSTEMS: [&str; 4] = [
    "hold_in_body",
    "tip_carried",
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

    /// Each layer's set holds the system that lays it, so the switch reaches every one.
    #[test]
    fn every_layers_set_holds_its_system() {
        use bevy::ecs::schedule::{ScheduleLabel, SystemSet};
        let mut app = App::new();
        plugin(&mut app);
        crate::feet::plugin(&mut app);
        crate::gaze::plugin(&mut app);
        crate::appearance::plugin(&mut app);
        crate::lift::plugin(&mut app);
        crate::figure::plugin(&mut app);
        let expected = [
            (Layer::CarryArm, "hold_in_body"),
            (Layer::Joints, "hold_joints"),
            (Layer::Lift, "lift_bodies"),
            (Layer::CarryHand, "tip_carried"),
            (Layer::Grip, "close_hands"),
            (Layer::Feet, "keep"),
            (Layer::ShieldArm, "fit_shield_arm"),
            (Layer::Gaze, "keep_on_aim"),
            (Layer::Swim, "lay"),
        ];
        for (layer, system) in expected {
            let schedule = if layer == Layer::Swim {
                Update.intern()
            } else {
                PostUpdate.intern()
            };
            let names = app.world_mut().schedule_scope(schedule, |world, schedule| {
                schedule.initialize(world).expect("the schedule builds");
                // Built, the systems are the executable's: named from there by key.
                let in_set = schedule
                    .graph()
                    .systems_in_set(layer.intern())
                    .cloned()
                    .unwrap_or_default();
                schedule
                    .systems()
                    .expect("the schedule is built")
                    .filter(|(key, _)| in_set.contains(key))
                    .map(|(_, s)| s.name().to_string())
                    .collect::<Vec<_>>()
            });
            assert!(
                names.len() == 1 && names[0].ends_with(&format!("::{system}")),
                "{layer:?}'s set holds {names:?}, not {system}"
            );
        }
    }

    /// The switch's words: names off, only some on, none (the fit kept), all; a wrong name
    /// says every name there is.
    #[test]
    fn the_switch_reads_its_words() {
        let only = Switch::parse("only=feet,gaze,grip").unwrap();
        let on: Vec<_> = Layer::ALL.into_iter().filter(|&l| only.on(l)).collect();
        assert_eq!(on, [Layer::Grip, Layer::Feet, Layer::Gaze]);
        let off = Switch::parse("off=carry_hand, gaze").unwrap();
        assert_eq!(off.names_off(), ["carry_hand", "gaze"]);
        let none = Switch::parse("none").unwrap();
        assert_eq!(
            Layer::ALL
                .into_iter()
                .filter(|&l| none.on(l))
                .collect::<Vec<_>>(),
            [Layer::Joints, Layer::Lift]
        );
        assert_eq!(Switch::parse("all").unwrap(), Switch::default());
        let wrong = Switch::parse("off=feat").unwrap_err();
        for layer in Layer::ALL {
            assert!(wrong.contains(layer.name()), "{wrong}");
        }
        assert!(Switch::parse("feet").is_err());
    }

    /// The game's help names every layer the switch knows.
    #[test]
    fn the_help_names_every_layer() {
        let usage = terra_world::shell::USAGE;
        for layer in Layer::ALL {
            assert!(
                usage.contains(layer.name()),
                "--pose-layers' help lacks {layer:?}"
            );
        }
    }
}
