//! Hosting and joining from the title screen (v0's Front; docs/netcode.md, D34, D638). Host
//! plays a world of the player's and opens it to others once it is played. Join asks the
//! host at an address; the host's welcome names its land (its seed and size), which this
//! game then makes and plays, its link kept all the while (`terra_walker::net::link::wait`).
//! Whatever ends a session not by the player's choice (nobody answering, another version, a
//! full world, the host leaving, the connection lost) brings the title screen back saying
//! so in the host's or the carrier's own words (D556).

use super::leaving::Leaving;
use super::profile::Profile;
use super::{Title, go};
use crate::shell::Options;
use bevy::prelude::*;
use terra_walker::net::link::{Ended, Link};
use terra_walker::net::session;
use terra_world::loading::Screen;

/// The world chosen is to be hosted once it is played.
#[derive(Resource)]
pub(super) struct Hosting;

/// A session begun from the title screen is on: its end is the title screen's to tell. One
/// begun by a script (`net.host`, `net.join`) is the script's to read (`net.state`).
#[derive(Resource)]
pub(super) struct Session;

pub fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(Screen::Playing),
        host.run_if(resource_exists::<Hosting>),
    )
    .add_systems(
        Update,
        (
            welcomed
                .run_if(in_state(Screen::Title))
                .run_if(resource_exists::<Link>),
            ended
                .run_if(resource_exists::<Ended>)
                .run_if(resource_exists::<Session>),
        ),
    );
}

/// What the player typed on the screen is their profile from now on.
pub(super) fn remember(world: &mut World) {
    let Some(title) = world.get_resource::<Title>() else {
        return;
    };
    let (name, address) = (
        title.0.player.trim().to_string(),
        title.0.address.trim().to_string(),
    );
    if let Some(mut profile) = world.get_resource_mut::<Profile>()
        && (profile.name != name || profile.address != address)
    {
        (profile.name, profile.address) = (name, address);
        profile.save();
    }
}

/// The world now played is opened to others. A port that cannot be opened ends the play:
/// the player asked to host, and is told why they are not.
fn host(world: &mut World) {
    world.remove_resource::<Hosting>();
    let name = world.resource::<Profile>().name.clone();
    match session::open(world, session::PORT, &name) {
        Ok(address) => {
            world.insert_resource(Session);
            info!("Hosting as {name} on {address}.");
        }
        Err(why) => world.insert_resource(Leaving(Some(why))),
    }
}

/// Join was chosen: the host at the address typed is asked.
pub(super) fn begin(world: &mut World, address: &str) {
    remember(world);
    let profile = world.resource::<Profile>().clone();
    let asked = session::address(address).and_then(|server| {
        // Whichever land the host welcomes this game to is made: no realm is asked for.
        session::join(
            world,
            server,
            &profile.identity(),
            &profile.name,
            Vec::new(),
        )
        .map(|()| server)
    });
    let menu = &mut world.resource_mut::<Title>().0;
    match asked {
        Ok(server) => {
            menu.joining = Some(format!("Asking the host at {server}. Back gives up."));
            world.insert_resource(Session);
        }
        Err(why) => menu.notice = Some(why),
    }
}

/// Back was chosen while the host had yet to answer.
pub(super) fn cancel(world: &mut World) {
    session::close(world);
    world.remove_resource::<Session>();
}

/// The host welcomed this game: its land is made, if it is not the land behind the screen,
/// and played, unsaved, the world being the host's to keep.
fn welcomed(world: &mut World) {
    if world.resource::<Title>().0.joining.is_none() {
        return;
    }
    let Some(realm) = world.resource::<Link>().realm().map(<[u8]>::to_vec) else {
        return;
    };
    world.resource_mut::<Title>().0.joining = None;
    let Some((seed, radius_m)) = session::realm_from(&realm) else {
        cancel(world);
        world.resource_mut::<Title>().0.notice =
            Some("The host's world could not be read: it runs another version of the game.".into());
        return;
    };
    info!(
        "Welcomed to a world of seed {seed}, {} km round.",
        radius_m / 1000.0
    );
    let mut options = world.resource_mut::<Options>();
    (options.seed, options.radius_m) = (Some(seed), radius_m);
    go(world, seed);
}

/// The session ended, and not by this player's choice: on the title screen it says why; a
/// world being played is left for the title screen, which says why. While a land is loading
/// it waits for one or the other.
fn ended(world: &mut World) {
    let why = match world.resource::<State<Screen>>().get() {
        Screen::Loading => return,
        Screen::Title | Screen::Playing => world.remove_resource::<Ended>().map(|ended| ended.0),
    };
    world.remove_resource::<Session>();
    if *world.resource::<State<Screen>>().get() == Screen::Playing {
        world.insert_resource(Leaving(why));
    } else {
        let menu = &mut world.resource_mut::<Title>().0;
        (menu.joining, menu.notice) = (None, why);
    }
}
