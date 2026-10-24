//! The scripted duel's driver (`--duel <file>`, D69): a fight script (`terra_motion::script`,
//! a file under assets/data/duels/) run for the player, its intents pressed as the keys the
//! controls bind to them, after the run's own scripted keys, so they come in as a player's
//! do and every check sees what a person's presses would give. Each step is said as it
//! comes, with the run's frame, and kept in the fight's record (`record`).

use super::{Fighter, Set, partner::Partner, record::Said};
use bevy::{input::InputSystems, prelude::*};
use terra_motion::contact::Fighting;
use terra_motion::script::{Intent, Script};
use terra_walker::Player;
use terra_walker::controls::{Action, Binding, Settings};
use terra_world::shell::{Options, ScriptFrame, scripted_presses};

/// The script being run.
#[derive(Resource)]
struct Driving(Script);

pub fn plugin(app: &mut App) {
    let Some(path) = app
        .world()
        .get_resource::<Options>()
        .and_then(|options| options.duel.clone())
    else {
        return;
    };
    let file = path.to_string_lossy().to_string();
    let text = std::fs::read_to_string(terra_world::data::path(&file))
        .unwrap_or_else(|problem| panic!("--duel: could not read {file}: {problem}"));
    let script = Script::read(&text).unwrap_or_else(|problem| panic!("--duel {file}: {problem}"));
    info!(
        "Melee: the duel script {file}, {} steps",
        script.steps.len()
    );
    app.insert_resource(Driving(script))
        .add_systems(PreUpdate, drive.after(scripted_presses).after(InputSystems));
}

/// The actions an intent is, as the controls name them: the heavy blow is the light one with
/// sprint held.
fn actions(intent: Intent) -> &'static [Action] {
    match intent {
        Intent::Draw => &[Action::Draw],
        Intent::Lock => &[Action::TakeApart],
        Intent::Light => &[Action::Use],
        Intent::Heavy => &[Action::Use, Action::Sprint],
        Intent::Roll => &[Action::Dodge],
        Intent::Guard => &[Action::BuildMenu],
        Intent::Forward => &[Action::Forward],
        Intent::Back => &[Action::Back],
        Intent::Left => &[Action::Left],
        Intent::Right => &[Action::Right],
    }
}

/// Each counted frame, the script's step and intents, pressed as keys.
fn drive(
    frame: Res<ScriptFrame>,
    set: Res<Set>,
    settings: Res<Settings>,
    mut driving: ResMut<Driving>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    players: Query<&Fighter, With<Player>>,
    partners: Query<&Fighter, With<Partner>>,
    said: Option<ResMut<Said>>,
) {
    if !frame.is_changed() || driving.0.done() {
        return;
    }
    // Sheathed, the player is in no fight yet: free, in no move.
    let sheathed = Fighting::new(super::HEALTH);
    let me = players
        .iter()
        .next()
        .map_or(&sheathed, |fighter| &fighter.fight);
    let other = partners.iter().next().map(|fighter| &fighter.fight);
    let tick = driving.0.tick(&set.0, me, other, 1.0 / 60.0);
    for (intent, pressed) in tick.intents {
        for &action in actions(intent) {
            let key = settings
                .bindings(action)
                .into_iter()
                .flatten()
                .find_map(|binding| match binding {
                    Binding::Key(key) => Some(key),
                    _ => None,
                });
            let Some(key) = key else {
                warn!("Melee: the duel script's {intent:?} has no key bound");
                continue;
            };
            keys.press(key);
            if !pressed {
                keys.clear_just_pressed(key);
            }
        }
    }
    if let Some(step) = tick.step {
        info!("Melee: frame {}: the script's {step}", frame.0);
        if let Some(mut said) = said {
            said.step = Some(step);
        }
    }
}
