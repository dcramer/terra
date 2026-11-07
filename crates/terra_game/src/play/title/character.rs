//! The player's character on the title screen (D111): while its page is up the camera
//! stands before the figure behind the screen, which wears each choice as it is turned, as
//! Valheim's and Enshrouded's creators show the character being made. Leaving the page keeps
//! the choices in the profile; this machine's player wears the profile's in every world.

use super::Title;
use super::menu::Page;
use super::profile::Profile;
use crate::play::IsLocalPlayer;
use crate::shell::Options;
use bevy::prelude::*;
use terra_walker::creator::Features;
use terra_world::loading::Screen;
use terra_world::start::Start;

/// The fixed camera the title screen had before the character's page took it.
#[derive(Resource)]
struct Posing(Option<(Vec3, Vec3)>, bool);

/// Where the camera stands before the figure, in metres ahead of it, up, and to its right;
/// and how high on the figure it looks: head and shoulders, every choice being of the head,
/// and near enough that the land's branches seldom hang between (the figure stands wherever
/// its world's start is).
const BEFORE: Vec3 = Vec3::new(0.85, 1.66, 0.22);
const LOOKS_AT_M: f32 = 1.6;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (pose.run_if(in_state(Screen::Title)), wear));
}

/// This machine's player wears the profile's features on arriving in a world, and on the
/// character's page each as it is chosen. A run with no profile (no title screen) leaves its player as its look is.
fn wear(
    mut commands: Commands,
    profile: Option<Res<Profile>>,
    title: Option<Res<Title>>,
    screen: Res<State<Screen>>,
    players: Query<(Entity, Option<&Features>), IsLocalPlayer>,
) {
    let Some(profile) = profile else { return };
    // On its page the figure wears what is being chosen, kept or not.
    let choosing = title
        .as_deref()
        .filter(|title| *screen.get() == Screen::Title && title.0.page == Page::Character);
    let features = choosing.map_or(&profile.features, |title| &title.0.features);
    for (player, worn) in &players {
        // A player arriving has none yet; on the page each turn is worn. In a world what a
        // player wears is theirs to change (a barber's, later; a script's, now).
        if worn.is_none() || (choosing.is_some() && worn != Some(features)) {
            commands.entity(player).insert(features.clone());
        }
    }
}

/// The camera before the figure while the character's page is up, and back where the title
/// screen had it after; leaving the page keeps what was chosen.
fn pose(
    mut commands: Commands,
    title: Res<Title>,
    mut options: ResMut<Options>,
    mut profile: ResMut<Profile>,
    posing: Option<Res<Posing>>,
    start: Option<Res<Start>>,
    players: Query<&Transform, IsLocalPlayer>,
) {
    if title.0.page != Page::Character {
        if let Some(posing) = posing {
            (options.vantage, options.vantage_ground) = (posing.0, posing.1);
            commands.remove_resource::<Posing>();
            if profile.features != title.0.features {
                profile.features = title.0.features.clone();
                profile.save();
            }
        }
        return;
    }
    let (Some(start), Ok(player)) = (start, players.single()) else {
        return;
    };
    if posing.is_none() {
        commands.insert_resource(Posing(options.vantage, options.vantage_ground));
    }
    // The fixed camera is told in the start's own frame (`camera::follow`).
    let up = player.translation.normalize();
    let world = |ahead: f32, high: f32, right: f32| {
        player.translation + *player.forward() * ahead + up * high + *player.right() * right
    };
    let frame = start.compute_affine().inverse();
    let view = (
        frame.transform_point3(world(BEFORE.x, BEFORE.y, BEFORE.z)),
        frame.transform_point3(world(0.0, LOOKS_AT_M, 0.0)),
    );
    if options.vantage != Some(view) || options.vantage_ground {
        (options.vantage, options.vantage_ground) = (Some(view), false);
    }
}
