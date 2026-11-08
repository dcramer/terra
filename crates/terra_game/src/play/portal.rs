//! Portals (D470, [docs/instances.md](../../../../docs/instances.md) "The crossing and the
//! way back"): a ring standing in a world that, when its interact key is used on it, crosses
//! to the world it leads to under the short fade (D501), and the return stack, so that the
//! portal back leads to the spot the player left. In the sandbox the run decides the crossing
//! itself; a server will (D51).
//!
//! Where a portal stands and where it leads is the run's to say ([`stand`]): the sandbox's
//! `portal` experiment stands one in the forest and one back in its placeholder realm.

use super::interact::{CanInteract, Interactable, Interacted};
use super::marks::{self, Marks};
use super::told::Told;
use super::{ArriveAt, LocalPlayer};
use bevy::prelude::*;
use terra_world::instance::{Building, InWorld, Travel, WorldSpec};
use terra_world::loading::cross_in_the_dark;
use terra_world::planet::Planet;

pub mod gates;
pub mod shared;
pub mod veil;

/// A way to another world (D470), standing on the ground, the ring's opening facing along
/// its forward.
#[derive(Component, Clone, Debug)]
pub struct Portal {
    pub to: WorldSpec,
    /// Whether it can be stepped through: a world's gate starts shut, until one who bears a
    /// mark that opens gates comes to it (D590); its veil hangs only while it is open.
    pub open: bool,
}

/// A portal's veil, hung while it is open.
#[derive(Component)]
struct Veil;

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
    gates::plugin(app);
    shared::plugin(app);
    app.init_resource::<Returns>()
        .add_message::<StepThrough>()
        .add_systems(
            Update,
            (used, step_through)
                .chain()
                .run_if(in_state(Travel::Playing)),
        )
        .add_systems(Update, hang_veils)
        .add_systems(Update, warm.run_if(in_state(Travel::Playing)))
        .add_systems(OnEnter(InWorld), arrive.in_set(Building::Stage))
        .add_systems(OnEnter(Travel::Playing), crossed);
    terra_world::remote::method(app, "portal.step", step);
    terra_world::remote::method(app, "portal.open", open);
    terra_world::remote::method(app, "portal.state", state);
}

/// `portal.state`: the portal nearest the player, without touching it: how far off it
/// stands, whether it is open, and where it leads.
fn state(
    In(_): In<Option<serde_json::Value>>,
    portals: Query<(&Portal, &Transform)>,
    body: Option<Single<&Transform, With<LocalPlayer>>>,
) -> bevy::remote::BrpResult {
    let refused = terra_world::remote::refused;
    let body = body.ok_or_else(|| refused("There is no player near any portal yet."))?;
    let nearest = portals.iter().min_by(|a, b| {
        let away = |at: &Transform| at.translation.distance(body.translation);
        away(a.1).total_cmp(&away(b.1))
    });
    let (portal, at) = nearest.ok_or_else(|| refused("No portal stands in this world."))?;
    Ok(serde_json::json!({
        "away_m": at.translation.distance(body.translation),
        "open": portal.open,
        "to": { "kind": portal.to.kind, "seed": portal.to.seed.to_string() },
    }))
}

/// `portal.open`: opens the portal nearest the player, as one who bears the mark does, so a
/// scripted run crosses without beating the boss (v0-goals, Proof); says whether it was shut.
fn open(
    In(_): In<Option<serde_json::Value>>,
    mut portals: Query<(&mut Portal, &Transform)>,
    body: Option<Single<&Transform, With<LocalPlayer>>>,
) -> bevy::remote::BrpResult {
    let refused = terra_world::remote::refused;
    let body = body.ok_or_else(|| refused("There is no player near any portal yet."))?;
    let nearest = portals.iter_mut().min_by(|a, b| {
        let away = |at: &Transform| at.translation.distance(body.translation);
        away(a.1).total_cmp(&away(b.1))
    });
    let (mut portal, _) = nearest.ok_or_else(|| refused("No portal stands in this world."))?;
    let was_shut = !portal.open;
    if was_shut {
        portal.open = true;
    }
    Ok(serde_json::json!({ "was_shut": was_shut }))
}

/// `portal.step`: the player steps through the portal nearest them, as the interact key on
/// it does, for a scripted run; says how far off it stood and where it leads.
fn step(
    In(_): In<Option<serde_json::Value>>,
    portals: Query<(Entity, &Portal, &Transform)>,
    body: Option<Single<&Transform, With<LocalPlayer>>>,
    mut through: MessageWriter<StepThrough>,
) -> bevy::remote::BrpResult {
    let refused = terra_world::remote::refused;
    let body = body.ok_or_else(|| refused("There is no player to step through a portal yet."))?;
    let nearest = portals.iter().min_by(|a, b| {
        let away = |at: &Transform| at.translation.distance(body.translation);
        away(a.2).total_cmp(&away(b.2))
    });
    let (portal, leads, at) = nearest.ok_or_else(|| refused("No portal stands in this world."))?;
    through.write(StepThrough(portal));
    Ok(serde_json::json!({
        "away_m": at.translation.distance(body.translation),
        "open": leads.open,
        "to": { "kind": leads.to.kind, "seed": leads.to.seed.to_string() },
    }))
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
    mut portals: Query<&mut Portal>,
    body: Option<Single<(Entity, &Transform, &Marks), With<LocalPlayer>>>,
    spec: Res<WorldSpec>,
    mut returns: ResMut<Returns>,
    arriving: Option<Res<Arriving>>,
    (kinds, mut told): (Res<marks::Kinds>, MessageWriter<Told>),
    (hosting, mut link): (
        Option<Res<terra_walker::net::Host>>,
        Option<ResMut<terra_walker::net::link::Link>>,
    ),
) {
    let Some(mut portal) = through
        .read()
        .map(|step| step.0)
        .last()
        .and_then(|portal| portals.get_mut(portal).ok())
    else {
        return;
    };
    let Some((player, body, marks)) = body.map(Single::into_inner) else {
        return;
    };
    if arriving.is_some() {
        return;
    }
    if !portal.open {
        // Shut: opened by one who bears a mark that opens gates, for everyone and for good
        // (D590); to anyone else it stays shut, and says so.
        match (kinds.opens_gates(marks), link.as_mut()) {
            // In another's world the gate is the host's to open: asked of it (`shared`).
            (Some(mark), Some(link)) => {
                link.ask(&shared::ask(marks));
                told.write(Told::new(
                    player,
                    format!("You hold {} to the gate.", mark.name),
                ));
            }
            (Some(mark), None) => {
                portal.open = true;
                told.write(Told::new(
                    player,
                    format!("The gate opens to {}.", mark.name),
                ));
            }
            (None, _) => {
                told.write(Told::new(
                    player,
                    "The gate is shut. It answers a mark you do not bear.",
                ));
            }
        }
        return;
    }
    // A shared world is one world until its realms run as instances (D638, D640).
    if hosting.is_some() || link.is_some() {
        told.write(Told::new(
            player,
            "The gate is open, but nobody can cross while this world is shared yet.",
        ));
        return;
    }
    let arrive = crossing(&mut returns, &spec, body, &portal.to);
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

/// How near an open portal the world beyond begins to be made, in metres: a jog of twenty
/// seconds, about what a full world's land takes (D501's cover is for what is left).
const WARM_M: f32 = 100.0;

/// The world beyond an open portal the player nears is begun ahead of the crossing
/// (`terra_world::planet::entering::generating::Warm`), once for each world neared; not in
/// a shared world, which nobody crosses yet.
fn warm(
    mut commands: Commands,
    portals: Query<(&Portal, &Transform)>,
    body: Option<Single<&Transform, With<LocalPlayer>>>,
    shared: (
        Option<Res<terra_walker::net::Host>>,
        Option<Res<terra_walker::net::link::Link>>,
    ),
    mut warmed: Local<Option<terra_world::instance::InstanceKey>>,
) {
    let Some(body) = body else { return };
    if shared.0.is_some() || shared.1.is_some() {
        return;
    }
    let near = portals
        .iter()
        .find(|(portal, at)| portal.open && at.translation.distance(body.translation) < WARM_M);
    if let Some((portal, _)) = near
        && *warmed != Some(portal.to.key)
    {
        *warmed = Some(portal.to.key);
        commands.queue(terra_world::planet::entering::generating::Warm(
            portal.to.clone(),
        ));
    }
}

/// A portal's veil hangs while it is open, and not while it is shut.
fn hang_veils(
    portals: Query<(&Portal, &Children), Changed<Portal>>,
    mut veils: Query<&mut Visibility, With<Veil>>,
) {
    for (portal, children) in &portals {
        for child in children.iter() {
            if let Ok(mut visibility) = veils.get_mut(child) {
                visibility.set_if_neq(if portal.open {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                });
            }
        }
    }
}

/// On entering the world crossed to, where the crossing says to arrive, or before that
/// world's gate if it says nowhere, before the body stands (`Building::Arrive`).
fn arrive(world: &mut World) {
    match world.remove_resource::<Arriving>() {
        Some(Arriving(Some(at))) => world.insert_resource(at),
        // Crossed on, to a world with a gate of its own: out of that gate.
        Some(Arriving(None)) => {
            if let Some(gate) = world.get_resource::<gates::Gate>().copied() {
                world.insert_resource(gates::arrival(&gate));
            }
        }
        None => {}
    }
}

/// The gate's model (`art/props/realm_gate`, `builds.ron`'s "Realm gate"): a ring of fitted
/// stone on two steps, 3.3 m wide and 3.4 m tall, its carved face its front.
const MODEL: &str = "models/props/RealmGate.glb";
/// The middle of the ring's opening above the ground, and the opening's radius, as the model
/// is made: where the veil hangs.
const MIDDLE_M: f32 = 1.75;
const OPENING_M: f32 = 1.12;
/// How far round its foot the ground is cleared of what grows: a small glade, so its
/// opening is seen from where one comes to it and no trunk stands in the way through.
pub(crate) const CLEAR_M: f32 = 5.0;

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
    (to, open): (WorldSpec, bool),
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
            Portal { to, open },
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
                Veil,
                // Light, not a thing: it throws no shadow.
                bevy::light::NotShadowCaster,
                Mesh3d(disc),
                MeshMaterial3d(veil),
                Transform::from_xyz(0.0, MIDDLE_M, 0.0),
                if open {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
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
