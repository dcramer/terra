//! World instances (D468 to D471, [docs/instances.md](../../../docs/instances.md)): the
//! world a run is in, how what belongs to it is told apart from what outlives it, and
//! leaving one world for another without anything of the first staying behind.
//!
//! A Bevy `World` holds at most one instance (D468). What belongs to it is found by rule
//! (D469), as Unreal's seamless travel keeps a short list of what persists and lets the
//! rest go with the old world:
//!
//! - **Entities** are the world's by default: [`WorldScoped`] is a required component of
//!   `Transform`, so every placed thing is the world's at spawn. [`Global`] opts out, on the
//!   few roots that outlive a world (the camera, the player's body). Leaving despawns every
//!   scoped root with its descendants, as Bevy's `DespawnOnExit` does; a child of a global
//!   root (the figure's bones) is not a root and stays. A world entity with no `Transform`
//!   adds [`WorldScoped`] itself; the [`Census`] catches one that forgets.
//! - **Resources** a world owns are declared once, `app.world_resource::<R>()`, built on
//!   entering from [`WorldSpec`] in `OnEnter(InWorld)`, and removed on leaving.
//! - **Components a world puts on a global entity** (the sky, haze and grading on the
//!   camera) are declared with `app.world_component::<C>()` and removed on leaving.
//!
//! Crossing is one path for every world, the first of a run included: queue [`Cross`] with
//! the spec of the world to go to. From [`Travel::Away`] it enters; from a world it leaves
//! first, and both happen in one frame, before its `Update` ([`Crossing`]). A world is built
//! in `OnEnter(InWorld)`, in the order of [`Building`]. Entering puts the loading screen up
//! again (D135), with the game clock held, and [`Travel::Playing`] begins when it comes down.

use crate::loading::Screen;
use bevy::{
    app::MainScheduleOrder,
    ecs::{
        entity_disabling::Disabled, lifecycle::HookContext, query::Allow, schedule::ScheduleLabel,
        world::DeferredWorld,
    },
    prelude::*,
    state::state::FreelyMutableState,
    transform::{
        StaticTransformOptimizations, TransformSystems,
        systems::{mark_dirty_trees, propagate_parent_transforms, sync_simple_transforms},
    },
};

pub mod census;
#[cfg(test)]
mod tests;

pub use census::Census;

/// Which world this is: everything a module needs to build its part of it (D468).
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct WorldSpec {
    /// Which instance this is, told apart from every other the server runs.
    pub key: InstanceKey,
    /// The realm's kind, as its theme names it: `forest`, `tundra`.
    pub kind: String,
    /// What the realm's generation starts from.
    pub seed: u64,
}

/// One running copy of a realm, as a portal names where it leads (D470).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstanceKey(pub u64);

/// Where the run is between worlds (D469). The first world of a run is entered as any other
/// is, after start-up: Bevy runs a state's first `OnEnter` before `PreStartup`, before
/// cameras and settings exist, so a run starts [`Travel::Away`] and enters with [`Cross`].
#[derive(States, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Travel {
    /// In no world: before the first, or a run, such as the workbench's, that has none.
    #[default]
    Away,
    /// Modules build the world's resources and spawn its content; the loading screen is up.
    Entering,
    Playing,
    /// The world's content is despawned and its resources removed; the next is entered in
    /// the same frame ([`Crossing`]).
    Leaving,
}

/// On while a world is being entered or played, so content spawned while entering is
/// scoped too: a module builds its part in `OnEnter(InWorld)` and keeps its record in
/// `OnExit(InWorld)`, before its entities and resources go.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InWorld;

impl ComputedStates for InWorld {
    type SourceStates = Travel;
    /// Bevy's default runs `OnExit` and `OnEnter` again when `Travel` changes and this does
    /// not, as from entering to playing, which would build the world a second time.
    const ALLOW_SAME_STATE_TRANSITIONS: bool = false;

    fn compute(travel: Travel) -> Option<Self> {
        matches!(travel, Travel::Entering | Travel::Playing).then_some(InWorld)
    }
}

/// Belongs to the world the run is in, and goes when it is left (D469). Required by
/// `Transform`, so a placed thing never needs to say so; an unplaced world entity (a sound
/// bed, a weather emitter) adds it itself.
#[derive(Component, Default, Clone, Copy, Debug)]
#[component(on_insert = WorldScoped::kept_off_globals)]
pub struct WorldScoped;

impl WorldScoped {
    /// A global entity given a `Transform` after it was spawned stays global.
    fn kept_off_globals(mut world: DeferredWorld, context: HookContext) {
        if world.entity(context.entity).contains::<Global>() {
            world
                .commands()
                .entity(context.entity)
                .try_remove::<WorldScoped>();
        }
    }
}

/// Outlives every world: the camera, the player's body (D469). Its children (the figure's
/// bones, what the hands hold) stay with it. What a world put on it is declared with
/// [`InstanceApp::world_component`] and taken off on leaving.
#[derive(Component, Default, Clone, Copy, Debug)]
#[component(on_insert = Global::not_the_worlds)]
pub struct Global;

impl Global {
    fn not_the_worlds(mut world: DeferredWorld, context: HookContext) {
        if world.entity(context.entity).contains::<WorldScoped>() {
            world
                .commands()
                .entity(context.entity)
                .try_remove::<WorldScoped>();
        }
    }
}

/// The order a world is built in on entering, in `OnEnter(InWorld)`, as the start-up
/// schedules ordered it before worlds came and went: what a module did in `Startup` goes in
/// [`Building::Place`], in `PostStartup` in [`Building::View`], after the body and camera
/// stand at the arrival and their places are propagated.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Building {
    /// The world's resources made afresh, and the land from its spec: the planet, the start.
    Land,
    /// What stands in the world from the first: the sea, the sun, the sky, the lakes.
    Place,
    /// This machine's body and the camera stood at the arrival.
    Arrive,
    /// What is made round where the viewer stands: the first view of the ground, the woods.
    View,
}

/// Where a crossing asked for is carried out at once (D469): right after `Startup`, so the
/// first world is built once the camera and settings exist and before anything after
/// start-up looks at it, and right after each frame's state transitions, so a world is left
/// and the next entered in the one frame. No world's system ever runs a frame without its
/// resources: `Res<Planet>` alone is read by 87 systems.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Crossing;

/// Carries the crossing through: leaving, entering and the loading screen put up are a
/// state transition each.
fn carry_out(world: &mut World) {
    fn pending<S: FreelyMutableState>(world: &World) -> bool {
        world
            .get_resource::<NextState<S>>()
            .is_some_and(|next| !matches!(next, NextState::Unchanged))
    }
    let mut crossing = false;
    for _ in 0..4 {
        if !(pending::<Travel>(world) || crossing && pending::<Screen>(world)) {
            return;
        }
        crossing = true;
        world.run_schedule(StateTransition);
    }
}

/// The world to go to once the one the run is in has been left.
#[derive(Resource)]
struct Bound(WorldSpec);

/// How many worlds the run has entered, the first included.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entered(pub u32);

/// What a world owns beyond its entities, as modules declared it: each with how to take it
/// away on leaving.
#[derive(Resource, Default)]
struct Owned {
    resources: Vec<(&'static str, fn(&mut World))>,
    components: Vec<(&'static str, fn(&mut World))>,
}

/// Go to another world (D470): from a world, leave it first; from none, enter at once. The
/// first world of a run is entered this way too.
pub struct Cross(pub WorldSpec);

impl Command for Cross {
    type Out = ();

    fn apply(self, world: &mut World) {
        let travel = world
            .get_resource::<State<Travel>>()
            .map(|state| *state.get());
        let next = match travel {
            None | Some(Travel::Away) => {
                world.insert_resource(self.0);
                Travel::Entering
            }
            Some(_) => {
                world.insert_resource(Bound(self.0));
                Travel::Leaving
            }
        };
        world
            .resource_mut::<NextState<Travel>>()
            .set_if_different(next);
    }
}

/// How a module says what of its own belongs to the world (D469).
pub trait InstanceApp {
    /// A resource the world owns: built on entering, from [`WorldSpec`], in
    /// `OnEnter(InWorld)`; removed on leaving, which drops its handles and cancels its tasks.
    fn world_resource<R: Resource>(&mut self) -> &mut Self;
    /// A world resource made afresh on entering each world, as `init_resource` makes one
    /// at the start: the state a module keeps of the world it is in.
    fn init_world_resource<R: Resource + FromWorld>(&mut self) -> &mut Self;
    /// A component the world puts on global entities, such as the sky on the camera;
    /// removed from every entity on leaving.
    fn world_component<C: Component>(&mut self) -> &mut Self;
    /// Asset types the [`Census`] counts beyond those registered for reflection.
    fn census_assets<A: Asset>(&mut self) -> &mut Self;
}

impl InstanceApp for App {
    fn world_resource<R: Resource>(&mut self) -> &mut Self {
        let name = std::any::type_name::<R>();
        let mut owned = self.world_mut().get_resource_or_init::<Owned>();
        if !owned.resources.iter().any(|(each, _)| *each == name) {
            owned.resources.push((name, |world| {
                world.remove_resource::<R>();
            }));
        }
        self
    }

    fn init_world_resource<R: Resource + FromWorld>(&mut self) -> &mut Self {
        self.world_resource::<R>().add_systems(
            OnEnter(InWorld),
            (|world: &mut World| {
                world.init_resource::<R>();
            })
            .in_set(Building::Land),
        )
    }

    fn world_component<C: Component>(&mut self) -> &mut Self {
        let name = std::any::type_name::<C>();
        let mut owned = self.world_mut().get_resource_or_init::<Owned>();
        if !owned.components.iter().any(|(each, _)| *each == name) {
            owned.components.push((name, take_off::<C>));
        }
        self
    }

    fn census_assets<A: Asset>(&mut self) -> &mut Self {
        census::count::<A>(self.world_mut());
        self
    }
}

/// Takes a world's component off every entity that has it, disabled ones included.
fn take_off<C: Component>(world: &mut World) {
    let holders: Vec<Entity> = world
        .query_filtered::<Entity, (With<C>, Allow<Disabled>)>()
        .iter(world)
        .collect();
    for entity in holders {
        world.entity_mut(entity).remove::<C>();
    }
}

/// World instances: the states, the scoping rule and leaving. Added by the shell before any
/// entity with a `Transform` exists: Bevy refuses to make one component required by
/// another once the other is in use (`ArchetypeExists`), a loud failure at start.
pub fn plugin(app: &mut App) {
    app.register_required_components::<Transform, WorldScoped>();
    app.init_schedule(Crossing);
    let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
    order.insert_startup_after(Startup, Crossing);
    order.insert_after(StateTransition, Crossing);
    app.add_systems(Crossing, carry_out)
        .configure_sets(
            OnEnter(InWorld),
            (
                (Building::Land, Building::Place, Building::Arrive)
                    .chain()
                    .before(TransformSystems::Propagate),
                Building::View.after(TransformSystems::Propagate),
            ),
        )
        // The body and camera's places propagated before the view is made round them, as
        // at the start (`PostStartup`).
        .add_systems(
            OnEnter(InWorld),
            (
                mark_dirty_trees,
                propagate_parent_transforms,
                sync_simple_transforms,
            )
                .chain()
                .in_set(TransformSystems::Propagate)
                .run_if(resource_exists::<StaticTransformOptimizations>),
        );
    app.init_state::<Travel>()
        .add_computed_state::<InWorld>()
        .init_resource::<Owned>()
        .init_resource::<Entered>()
        .add_systems(OnEnter(Travel::Leaving), leave)
        .add_systems(OnEnter(Travel::Entering), enter)
        .add_systems(OnExit(Screen::Loading), play)
        .add_systems(
            Update,
            play.run_if(in_state(Travel::Entering))
                .run_if(not(resource_exists::<State<Screen>>)),
        );
    census::plugin(app);
}

/// Entering: the loading screen goes up again for every world after the first (D135), the
/// clock held under it; it is up already for the first.
fn enter(mut entered: ResMut<Entered>, screen: Option<ResMut<NextState<Screen>>>) {
    entered.0 += 1;
    if let Some(mut screen) = screen {
        screen.set_if_different(Screen::Loading);
    }
    info!("Entering world {}", entered.0);
}

/// The world is ready once the loading screen comes down, or at once in a run without one.
fn play(travel: Res<State<Travel>>, mut next: ResMut<NextState<Travel>>) {
    if *travel.get() == Travel::Entering {
        next.set_if_different(Travel::Playing);
    }
}

/// Leaving (D470): every scoped root goes with its descendants, disabled ones included;
/// the world's resources are removed, dropping their handles, and its components taken off
/// global entities. Then the next world is entered, or none.
fn leave(world: &mut World) {
    world.despawn_all::<(
        With<WorldScoped>,
        Without<ChildOf>,
        Without<Global>,
        Allow<Disabled>,
    )>();
    world.resource_scope(|world, owned: Mut<Owned>| {
        for (_, remove) in &owned.resources {
            remove(world);
        }
        for (_, take) in &owned.components {
            take(world);
        }
    });
    world.remove_resource::<WorldSpec>();
    let next = match world.remove_resource::<Bound>() {
        Some(Bound(spec)) => {
            world.insert_resource(spec);
            Travel::Entering
        }
        None => Travel::Away,
    };
    world
        .resource_mut::<NextState<Travel>>()
        .set_if_different(next);
}
