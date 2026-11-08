//! Each world's gate, stood where its seed places it (D641, v0's "the gate placed from the
//! seed, reachable on every seed"): found on entering a world whose land is worked out round
//! its start (`terra_land::gate`), on ground a walk from the start reaches, its carved face
//! toward the start. It leads on to the realm beyond, a world of a seed derived from this
//! one's; in a world come to through a gate, it leads back, and a player crossing on arrives
//! before the gate there, not at that world's start: one steps out of a gate.
//!
//! What the realm beyond is like (its theme) is the next realm's to say; here it is a world
//! of the same kind and another seed.

use super::{Returns, veil};
use bevy::prelude::*;
use terra_core::Seed;
use terra_walker::walker::ArriveAt;
use terra_world::instance::{Building, InWorld, InstanceApp, InstanceKey, WorldSpec};
use terra_world::planet::Planet;
use terra_world::realm::Realms;
use terra_world::start::Start;

/// How far before its gate one who steps out of it stands, in metres.
const BEFORE_M: f32 = 3.0;

/// The world's gate: where it stands on the ground, and the way it faces, toward the start.
/// None of it in a world whose land has no place for one (a yard, a test level).
#[derive(Resource, Clone, Copy, Debug)]
pub struct Gate {
    pub at: Vec3,
    pub facing: Vec3,
    /// Whether it has been opened (D590): shut in a world first come to by its start, open
    /// in one come to through a gate; kept with the world's record.
    pub open: bool,
}

/// The portal that is its world's gate, whose state is the world's ([`Gate`]).
#[derive(Component)]
pub(super) struct WorldGate;

pub(super) fn plugin(app: &mut App) {
    app.world_resource::<Gate>()
        .add_systems(
            OnEnter(InWorld),
            (
                find.in_set(Building::Stage).before(super::arrive),
                stand.in_set(Building::Place),
            ),
        )
        .add_systems(Update, follow.run_if(resource_exists::<Gate>));
}

/// The realm beyond a world's gate: of the kind its realm's data names (`data/realms.ron`),
/// its seed derived from the world's, all 64 bits (D596), its instance keyed by that seed.
pub fn beyond(here: &WorldSpec, realms: &Realms) -> WorldSpec {
    let seed = Seed(here.seed).derive("beyond the gate").0;
    WorldSpec {
        key: InstanceKey(seed),
        kind: realms.beyond(&here.kind),
        seed,
    }
}

/// Finds the world's gate from its land, before anyone arrives.
fn find(mut commands: Commands, planet: Res<Planet>, start: Res<Start>, returns: Res<Returns>) {
    let Some(round) = planet.round() else {
        return;
    };
    let began = std::time::Instant::now();
    let from = start.translation.normalize();
    let Some(toward) = terra_land::gate::find(&round.land, from) else {
        warn!(
            "Gate: this world's land has no place for one within {} m of the start (terra_land::gate); none stands.",
            terra_land::gate::FARTHEST_M
        );
        return;
    };
    let at = planet.surface(toward);
    let up = at.normalize();
    let facing = (start.translation - at)
        .reject_from(up)
        .normalize_or(up.any_orthonormal_vector());
    let off = start.local(at);
    info!(
        "Gate: {:.0} m east and {:.0} m south of the start, found in {:.2} s",
        off.x,
        off.z,
        began.elapsed().as_secs_f32()
    );
    // Come to through a gate, this world's own stands open: it is the way back.
    commands.insert_resource(Gate {
        at,
        facing,
        open: !returns.0.is_empty(),
    });
}

/// Where one who came through a gate stands in the world come to: before its gate, facing
/// out from it, if the crossing named no spot of its own.
pub(super) fn arrival(gate: &Gate) -> ArriveAt {
    let at = gate.at + gate.facing * BEFORE_M;
    ArriveAt {
        at,
        toward: at + gate.facing * 10.0,
    }
}

/// Stands the gate: leading back, in a world come to through one, else on to the realm
/// beyond.
fn stand(
    mut commands: Commands,
    gate: Option<Res<Gate>>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<veil::VeilMaterial>>,
    planet: Res<Planet>,
    spec: Res<WorldSpec>,
    (returns, realms): (Res<Returns>, Res<Realms>),
) {
    let Some(gate) = gate else { return };
    let to = match returns.0.last() {
        Some(back) => back.world.clone(),
        None => beyond(&spec, &realms),
    };
    let portal = super::stand(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        &planet,
        (gate.at, gate.facing),
        (to, gate.open),
    );
    commands.entity(portal).insert(WorldGate);
}

/// The gate's portal and the world's record of it are one state: opened at the portal (by a
/// marked player, or a script), the world keeps it; given back open by a save, the portal
/// opens.
fn follow(mut gate: ResMut<Gate>, mut portals: Query<&mut super::Portal, With<WorldGate>>) {
    for mut portal in &mut portals {
        if portal.open && !gate.open {
            gate.open = true;
        } else if gate.open && !portal.open {
            portal.open = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_realm_beyond_is_another_world_by_the_seed_of_the_kind_the_data_names() {
        let realms = Realms::read();
        let here = WorldSpec {
            key: InstanceKey(0),
            kind: "world".into(),
            seed: 7,
        };
        let there = beyond(&here, &realms);
        assert_ne!(there.seed, here.seed);
        assert_eq!(there.key, InstanceKey(there.seed));
        // The first realm's gate leads to the industrial realm (D642), the same each time.
        assert_eq!(there.kind, "industrial");
        assert_eq!(beyond(&here, &realms), there);
        // Another world's gate leads to another realm of that kind.
        let other = beyond(
            &WorldSpec {
                seed: 8,
                ..here.clone()
            },
            &realms,
        );
        assert_ne!(other.seed, there.seed);
    }

    #[test]
    fn one_who_steps_out_of_a_gate_stands_before_it_facing_out() {
        let gate = Gate {
            at: Vec3::new(0.0, 1000.0, 0.0),
            facing: Vec3::X,
            open: true,
        };
        let arrive = arrival(&gate);
        assert_eq!(arrive.at, Vec3::new(BEFORE_M, 1000.0, 0.0));
        assert!((arrive.toward - arrive.at).normalize().dot(Vec3::X) > 0.999);
    }
}
