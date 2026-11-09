//! The instance host (docs/instances.md, D468, D640): each realm a headless world of its
//! own, with its own server ([`Host`]), ticked with no window and nothing drawn; the realms
//! share one carrier through a switch (`terra_net::transport::switch`), a connection routed
//! to one of them. Between the switch's taking in and its sending out the realms that have
//! anyone tick at once, each on a thread of the task pool, none touching another.
//!
//! A realm nobody is in keeps ticking for a grace, so a quick return costs nothing, and is
//! then frozen: not ticked, its server's tick and so its clock stopped (D500), until
//! someone comes to it.
//!
//! What a realm runs is its layers' (D198): the walker's own is [`walker`] (the ground, the
//! mover, the walk and the host's half of `net`); a layer above adds its rules as another.
//! The running game does not host through this yet: a realm here has no trees, props or
//! bases to stop a walker (D640).

use super::{Host, parts};
use bevy::app::{FixedFirst, FixedLast, FixedPostUpdate, FixedPreUpdate};
use bevy::ecs::schedule::{ScheduleLabel, SingleThreadedExecutor};
use bevy::prelude::*;
use bevy::tasks::{ComputeTaskPool, TaskPool};
use std::time::Duration;
use terra_net::server::Config;
use terra_net::transport::switch::{Branch, Switch};
use terra_net::transport::{Peer, ServerTransport};
use terra_world::planet::Planet;
use terra_world::tick::Sim;

/// One tick (D138).
const TICK: Duration = Duration::from_micros(15_625);
/// How long a realm nobody is in still ticks before it is frozen, in ticks: half a minute.
pub const GRACE_TICKS: u32 = 30 * 64;

/// Rules a realm runs, added to its world as a plugin is.
pub type Layer = fn(&mut App);

/// The walker's layer of a realm: the tick's stages, the walk, and hosting.
pub fn walker(app: &mut App) {
    app.configure_sets(
        FixedUpdate,
        (Sim::Act, Sim::World, Sim::Resolve, Sim::End).chain(),
    )
    .init_resource::<terra_world::flora::Obstacles>();
    crate::movement::plugin(app);
    // A realm is one world from its making: what the game makes on entering one is made here.
    app.init_resource::<crate::movement::Movers>();
    super::hosting(app);
}

/// The trees' layer of a realm (D640): what a small world's trees stop a walker by, filed
/// among the realm's obstacles as the game files them for the trees it draws, by the same
/// rule of the world (`terra_world::flora::trees`), so a player a realm walks is stopped by
/// the trunk their own game shows. After [`walker`]. A world with mountains has its trees
/// from the woods (`terra_world::woods`), which no realm holds yet.
pub fn trees(app: &mut App) {
    use terra_world::flora_kinds::{Flora, Season};
    let planet = app.world().resource::<Planet>();
    // The start as the game chooses it for this planet: the rule is laid out from it.
    let start = terra_world::start::Start::choose(planet, false);
    let stood =
        terra_world::flora::trees::stops((planet, &start.0), &Flora::read(), &Season::Unknown);
    let mut obstacles = app
        .world_mut()
        .get_resource_or_init::<terra_world::flora::Obstacles>();
    for (foot, radius) in stood {
        obstacles.add(foot, radius);
    }
}

/// Where a realm is in its life (docs/instances.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Life {
    /// Ticking, with someone in it.
    Active,
    /// Ticking with nobody in it, for so many ticks so far.
    Idle(u32),
    /// In memory, not ticked.
    Frozen,
}

/// One running realm: its world and where it is in its life.
pub struct Realm {
    world: World,
    life: Life,
}

impl Realm {
    fn new(planet: Planet, start: Vec3, config: Config, wire: Branch, layers: &[Layer]) -> Realm {
        let mut app = App::new();
        let realm = super::realm_of(&planet);
        app.insert_resource(planet)
            .init_resource::<Time>()
            .insert_resource(Host::new(config, parts(), Box::new(wire), start, realm));
        for layer in layers {
            layer(&mut app);
        }
        let mut world = std::mem::take(app.world_mut());
        // Realms tick side by side, each on one thread: the pool is spent across realms, not
        // within one.
        let mut schedules = world.resource_mut::<Schedules>();
        for (_, schedule) in schedules.iter_mut() {
            schedule.set_executor(SingleThreadedExecutor::new());
        }
        Realm {
            world,
            life: Life::Idle(0),
        }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn life(&self) -> Life {
        self.life
    }

    /// The tick its server is on: it stands while the realm is frozen.
    pub fn server_tick(&self) -> u32 {
        self.world.resource::<Host>().server().tick()
    }

    /// One tick of the realm: time passes, and the tick's schedules run in a game's order.
    fn tick(&mut self) {
        self.world.resource_mut::<Time>().advance_by(TICK);
        let order = [
            First.intern(),
            FixedFirst.intern(),
            FixedPreUpdate.intern(),
            FixedUpdate.intern(),
            FixedPostUpdate.intern(),
            FixedLast.intern(),
        ];
        for schedule in order {
            // A schedule no layer put anything in does not exist.
            let _ = self.world.try_run_schedule(schedule);
        }
        self.world.clear_trackers();
    }

    /// After a tick: active with anyone connected, else idle a tick longer, frozen past the
    /// grace.
    fn age(&mut self) {
        let anyone = self.world.resource::<Host>().server().clients() > 0;
        self.life = match (anyone, self.life) {
            (true, _) => Life::Active,
            (false, Life::Idle(ticks)) if ticks >= GRACE_TICKS => Life::Frozen,
            (false, Life::Idle(ticks)) => Life::Idle(ticks + 1),
            (false, _) => Life::Idle(0),
        };
    }
}

/// The instance host: the realms running here and the carrier they share.
pub struct Instances<T> {
    switch: Switch<T>,
    realms: Vec<Realm>,
    config: Config,
    layers: Vec<Layer>,
}

impl<T: ServerTransport> Instances<T> {
    /// A host of realms on a carrier, each running these layers ([`walker`] first).
    pub fn new(carrier: T, config: Config, layers: Vec<Layer>) -> Instances<T> {
        Instances {
            switch: Switch::new(carrier),
            realms: Vec::new(),
            config,
            layers,
        }
    }

    /// Opens a realm on a planet, those who come to it starting at a place; its number. The
    /// first opened is where a new connection goes.
    pub fn open(&mut self, planet: Planet, start: Vec3) -> usize {
        let (number, wire) = self.switch.branch();
        let mut config = self.config.clone();
        config.epoch = u8::try_from(number).expect("a host of at most 256 realms");
        let realm = Realm::new(planet, start, config, wire, &self.layers);
        self.realms.push(realm);
        number
    }

    pub fn realm(&self, number: usize) -> &Realm {
        &self.realms[number]
    }

    pub fn realm_mut(&mut self, number: usize) -> &mut Realm {
        &mut self.realms[number]
    }

    pub fn realms(&self) -> usize {
        self.realms.len()
    }

    /// Names the realm a new connection goes to.
    pub fn home(&mut self, realm: usize) {
        self.switch.home(realm);
    }

    /// The realm a connection is in.
    pub fn realm_of(&self, peer: Peer) -> Option<usize> {
        self.switch.branch_of(peer)
    }

    /// Moves a connection to another realm (a crossing): it leaves the one, and the other
    /// greets it as one arriving.
    pub fn hand(&mut self, peer: Peer, to: usize) {
        self.switch.hand(peer, to);
    }

    /// One tick of every realm that has anyone or is in its grace; a frozen realm someone has
    /// come to thaws. So much time has passed for the carrier.
    pub fn tick(&mut self, elapsed: Duration) {
        self.switch.take_in(elapsed);
        for (number, realm) in self.realms.iter_mut().enumerate() {
            if realm.life == Life::Frozen && self.switch.waiting(number) {
                realm.life = Life::Idle(0);
            }
        }
        let mut ticking: Vec<&mut Realm> = self
            .realms
            .iter_mut()
            .filter(|realm| realm.life != Life::Frozen)
            .collect();
        if let [realm] = &mut ticking[..] {
            realm.tick();
        } else if !ticking.is_empty() {
            ComputeTaskPool::get_or_init(TaskPool::default).scope(|scope| {
                for realm in &mut ticking {
                    scope.spawn(async move { realm.tick() });
                }
            });
        }
        for realm in ticking {
            realm.age();
        }
        self.switch.send_out();
    }

    /// The host is closing: everyone in every realm is told so.
    pub fn close(&mut self) {
        for realm in &mut self.realms {
            realm.world.resource_mut::<Host>().close();
        }
        self.switch.send_out();
    }
}
