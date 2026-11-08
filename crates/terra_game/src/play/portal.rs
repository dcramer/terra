//! Portals (D470, [docs/instances.md](../../../../docs/instances.md) "The crossing and the
//! way back"): a ring standing in a world that, when its interact key is used on it, crosses
//! to the world it leads to under the short fade (D501), and the return stack, so that the
//! portal back leads to the spot the player left. In the sandbox the run decides the crossing
//! itself; a server will (D51).
//!
//! Where a portal stands and where it leads is the run's to say ([`stand`]): the sandbox's
//! `portal` experiment stands one in the forest and one back in its placeholder realm.

use super::interact::{CanInteract, Interactable, Interacted};
use super::{ArriveAt, LocalPlayer};
use bevy::prelude::*;
use terra_world::instance::{Building, InWorld, Travel, WorldSpec};
use terra_world::loading::cross_in_the_dark;
use terra_world::planet::Planet;

pub mod veil;

/// A way to another world (D470), standing on the ground, the ring's opening facing along
/// its forward.
#[derive(Component, Clone, Debug)]
pub struct Portal {
    pub to: WorldSpec,
}

/// Where the player came from, a crossing at a time (D470): the world, the spot and the
/// point faced. Outlives every world: it is the character's, not the world's.
#[derive(Resource, Default, Debug)]
pub struct Returns(pub Vec<Return>);

#[derive(Clone, Debug, PartialEq)]
pub struct Return {
    pub world: WorldSpec,
    pub at: Vec3,
    pub toward: Vec3,
}

/// A crossing under way: where to arrive in the world crossed to, if not at its own arrival.
/// Set when a portal is stepped through, taken on entering the next world.
#[derive(Resource, Debug)]
struct Arriving(Option<ArriveAt>);

/// Step through this portal: the interact key on it, or a scripted run's key.
#[derive(Message, Clone, Copy, Debug)]
pub struct StepThrough(pub Entity);

pub fn plugin(app: &mut App) {
    veil::plugin(app);
    app.init_resource::<Returns>()
        .add_message::<StepThrough>()
        .add_systems(
            Update,
            (used, step_through)
                .chain()
                .run_if(in_state(Travel::Playing)),
        )
        .add_systems(OnEnter(InWorld), arrive.in_set(Building::Stage))
        .add_systems(OnEnter(Travel::Playing), crossed);
}

/// When the portal was stepped through, on the wall clock, until the next world is played:
/// a scripted run's `Time<Real>` steps a frame at a time, and stands still while it loads.
#[derive(Resource)]
struct SteppedAt(std::time::Instant);

/// Says how long the crossing took, from the step through to play in the next world.
fn crossed(mut commands: Commands, stepped: Option<Res<SteppedAt>>, spec: Res<WorldSpec>) {
    if let Some(stepped) = stepped {
        info!(
            "Portal: crossed to {} {} in {:.2} s",
            spec.kind,
            spec.key.0,
            stepped.0.elapsed().as_secs_f64()
        );
        commands.remove_resource::<SteppedAt>();
    }
}

/// The interact key used on a portal steps through it.
fn used(mut used: MessageReader<Interacted>, mut through: MessageWriter<StepThrough>) {
    for portal in used.read().filter_map(|used| used.on(Interactable::Portal)) {
        through.write(StepThrough(portal));
    }
}

/// Crosses to where the portal leads: back to the spot left, when it leads to the world the
/// player came from, otherwise to that world's own arrival, with this spot kept to come back
/// to. Once a crossing: a second press while the window darkens does nothing.
fn step_through(
    mut commands: Commands,
    mut through: MessageReader<StepThrough>,
    portals: Query<&Portal>,
    body: Option<Single<&Transform, With<LocalPlayer>>>,
    spec: Res<WorldSpec>,
    mut returns: ResMut<Returns>,
    arriving: Option<Res<Arriving>>,
) {
    let Some(portal) = through.read().find_map(|step| portals.get(step.0).ok()) else {
        return;
    };
    let Some(body) = body else { return };
    if arriving.is_some() {
        return;
    }
    let arrive = crossing(&mut returns, &spec, &body, &portal.to);
    info!(
        "Portal: from {} {} to {} {}",
        spec.kind, spec.key.0, portal.to.kind, portal.to.key.0
    );
    commands.insert_resource(Arriving(arrive));
    commands.insert_resource(SteppedAt(std::time::Instant::now()));
    cross_in_the_dark(&mut commands, portal.to.clone());
}

/// What a crossing does to the return stack: back to the world on top of it pops it and
/// arrives where it says; to any other pushes where the body stands.
fn crossing(
    returns: &mut Returns,
    here: &WorldSpec,
    body: &Transform,
    to: &WorldSpec,
) -> Option<ArriveAt> {
    if returns
        .0
        .last()
        .is_some_and(|back| back.world.key == to.key)
    {
        return returns.0.pop().map(|back| ArriveAt {
            at: back.at,
            toward: back.toward,
        });
    }
    returns.0.push(Return {
        world: here.clone(),
        at: body.translation,
        toward: body.translation + *body.forward() * 10.0,
    });
    None
}

/// On entering the world crossed to, where the crossing says to arrive, before the body
/// stands (`Building::Arrive`).
fn arrive(world: &mut World) {
    if let Some(Arriving(Some(at))) = world.remove_resource::<Arriving>() {
        world.insert_resource(at);
    }
}

/// The gate's model (`art/props/realm_gate`, `builds.ron`'s "Realm gate"): a ring of fitted
/// stone on two steps, 3.3 m wide and 3.4 m tall, its carved face its front.
const MODEL: &str = "models/props/RealmGate.glb";
/// The middle of the ring's opening above the ground, and the opening's radius, as the model
/// is made: where the veil hangs.
const MIDDLE_M: f32 = 1.75;
const OPENING_M: f32 = 1.12;
/// How far round its foot the ground is cleared of what grows: its half width and a pace.
const CLEAR_M: f32 = 2.6;

/// Stands a portal on the ground at `at`, its carved face and its opening toward `facing`,
/// leading to `to`: the gate's stone ring round a violet veil, the world's (`WorldScoped`),
/// so it goes with it.
pub fn stand(
    commands: &mut Commands,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<veil::VeilMaterial>,
    planet: &Planet,
    (at, facing): (Vec3, Vec3),
    to: WorldSpec,
) -> Entity {
    let feet = planet.ground_at(at);
    let up = feet.normalize();
    let facing = facing
        .reject_from(up)
        .normalize_or(up.any_orthonormal_vector());
    let veil = veil::material(materials);
    // What grows where it stands, and a pace round it, is cleared, as a settlement clears
    // its ground: nothing stands in the opening or grows through the ring.
    // Kept clear at once, for what is laid after this frame (the forest's floor, trees that
    // stream in), and asked of what already grows.
    commands.queue(move |world: &mut World| {
        if let Some(mut kept) = world.get_resource_mut::<terra_world::flora::KeptClear>() {
            kept.keep(feet, CLEAR_M);
        }
    });
    commands.write_message(terra_world::flora::ClearAround(feet, CLEAR_M));
    let disc = meshes.add(Circle::new(OPENING_M));
    let ring = assets.load(GltfAssetLabel::Scene(0).from_asset(MODEL));
    commands
        .spawn((
            Name::new("Portal"),
            Portal { to },
            CanInteract(Interactable::Portal),
            Transform::from_translation(feet).looking_to(facing, up),
            Visibility::default(),
        ))
        .with_children(|portal| {
            portal.spawn((
                WorldAssetRoot(ring),
                // The models face +Z; things here face -Z.
                Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            ));
            portal.spawn((
                Mesh3d(disc),
                MeshMaterial3d(veil),
                Transform::from_xyz(0.0, MIDDLE_M, 0.0),
            ));
        })
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_world::instance::InstanceKey;

    fn world(key: u64) -> WorldSpec {
        WorldSpec {
            key: InstanceKey(key),
            kind: if key == 0 { "yard" } else { "playground" }.into(),
            seed: key + 1,
        }
    }

    /// Out through a portal keeps the spot left; the portal back pops it and arrives there,
    /// facing as the body faced; the stack is empty again after each round trip.
    #[test]
    fn the_portal_back_leads_to_the_spot_left() {
        let mut returns = Returns::default();
        let left = Transform::from_xyz(3.0, 100.0, -4.0).looking_to(Vec3::X, Vec3::Y);
        for _ in 0..3 {
            let out = crossing(&mut returns, &world(0), &left, &world(7));
            assert!(out.is_none(), "the other world's own arrival");
            assert_eq!(returns.0.len(), 1);
            let somewhere = Transform::from_xyz(50.0, 90.0, 50.0);
            let back = crossing(&mut returns, &world(7), &somewhere, &world(0))
                .expect("back to the spot left");
            assert_eq!(back.at, left.translation);
            assert!((back.toward - left.translation).normalize().dot(Vec3::X) > 0.999);
            assert!(returns.0.is_empty());
        }
    }

    /// A portal on to a third world from the second pushes again: the way back unwinds.
    #[test]
    fn crossings_on_stack_and_unwind() {
        let mut returns = Returns::default();
        let at = |x: f32| Transform::from_xyz(x, 100.0, 0.0);
        assert!(crossing(&mut returns, &world(0), &at(1.0), &world(7)).is_none());
        assert!(crossing(&mut returns, &world(7), &at(2.0), &world(9)).is_none());
        let back = crossing(&mut returns, &world(9), &at(3.0), &world(7)).unwrap();
        assert_eq!(back.at.x, 2.0);
        let home = crossing(&mut returns, &world(7), &at(4.0), &world(0)).unwrap();
        assert_eq!(home.at.x, 1.0);
    }
}
