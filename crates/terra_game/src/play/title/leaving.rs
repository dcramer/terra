//! Leaving a world for the title screen without ending the game (D579, v0's Front): the
//! world is saved, its player goes with it, and the land is made again under the loading
//! screen, which comes down on the title screen. As Valheim's and Enshrouded's "log out"
//! do, nothing of the world played is in the next: what a world owns goes as any crossing
//! takes it (D469), and the player's body, which a crossing carries, is despawned here, so
//! the next world's player is a new one, given what its save holds.

use super::super::save::{SavePath, save_on_exit};
use super::super::worlds::{self, Playing, Root};
use super::menu::Menu;
use super::{Chosen, Title};
use crate::play::IsLocalPlayer;
use crate::shell::Options;
use bevy::prelude::*;
use terra_world::instance::{Cross, WorldSpec};
use terra_world::loading::{Screen, Then};

/// Asked to leave the world played for the title screen, and what to tell the player
/// there: why a hosted world's link ended, or nothing when they chose to go.
#[derive(Resource, Default)]
pub struct Leaving(pub Option<String>);

/// What the run was started with that playing a world changes, given back on leaving it.
#[derive(Resource)]
struct Asked {
    free_build: bool,
}

pub fn plugin(app: &mut App) {
    app.add_systems(PostStartup, remember).add_systems(
        Last,
        leave.after(save_on_exit).run_if(resource_exists::<Leaving>),
    );
}

fn remember(mut commands: Commands, options: Res<Options>, playing: Option<Res<Playing>>) {
    // A world chosen by a flag has already made the run build free if it is Creative.
    let creative = playing.is_some_and(|playing| playing.0.meta.creative);
    commands.insert_resource(Asked {
        free_build: options.free_build && !creative,
    });
}

/// This machine's player goes: the next world entered makes its own (`walker::spawn`). Before
/// a crossing from the title screen or to it, so that while the next land is made there is
/// no player, as at the first load: what acts on a player in the world then waits for both.
pub(super) fn nobody(world: &mut World) {
    let mut players = world.query_filtered::<Entity, IsLocalPlayer>();
    for player in players.iter(world).collect::<Vec<_>>() {
        world.despawn(player);
    }
}

/// Leaves the world: after its save is written (`save_on_exit`), in the same frame.
fn leave(world: &mut World) {
    let Some(Leaving(notice)) = world.remove_resource::<Leaving>() else {
        return;
    };
    // Only a world that is played is left: the title screen is already up otherwise.
    if *world.resource::<State<Screen>>().get() != Screen::Playing {
        return;
    }
    // A session goes with the world: a host's guests are told the host left.
    terra_walker::net::session::close(world);
    world.remove_resource::<super::joining::Session>();
    world.remove_resource::<SavePath>();
    world.remove_resource::<Playing>();
    nobody(world);
    if let Some(free_build) = world.get_resource::<Asked>().map(|asked| asked.free_build) {
        world.resource_mut::<Options>().free_build = free_build;
    }
    let kept = world
        .get_resource::<Root>()
        .map_or_else(Vec::new, |root| worlds::list(&root.0));
    let mut menu = Menu::new(kept);
    // The dice roll on from where the screen left them, so the next world is another land.
    menu.dice = world
        .get_resource::<Title>()
        .map_or_else(|| worlds::Dice::from_clock().0, |title| title.0.dice);
    menu.notice = notice;
    if let Some(profile) = world.get_resource::<super::profile::Profile>() {
        (menu.player, menu.address) = (profile.name.clone(), profile.address.clone());
    }
    world.insert_resource(Title(menu));
    world.resource_mut::<Chosen>().0 = None;
    world.insert_resource(Then(Screen::Title));
    // The land again, as the first load made it: every placed thing of the world left goes.
    // Under the loading screen from this frame, so nothing of play runs on a world half gone.
    world
        .resource_mut::<NextState<Screen>>()
        .set(Screen::Loading);
    // And the clock held, as the title screen holds it: a tick's systems read the world's
    // own resources, which are gone until the land is made again.
    world.resource_mut::<Time<Virtual>>().pause();
    let spec = world.resource::<WorldSpec>().clone();
    info!("Leaving the world for the title screen.");
    Command::apply(Cross(spec), world);
}
