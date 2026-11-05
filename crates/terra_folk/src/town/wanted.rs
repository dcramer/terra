//! Where a world wants its town (D620): at the place its land stands one
//! (`terra_world::start::Towns`), the site `--site town` found or the world's first town by
//! the player's start, as the world is entered; laid there by [`super::lay`].

use bevy::prelude::*;
use terra_world::instance::{Building, InWorld, InstanceApp};
use terra_world::planet::Planet;
use terra_world::start::{Start, Towns};

/// How far from the place the land's rules found a town (`Towns`) its square may move, to
/// the most level ground there on the ground as drawn.
pub const AT_THE_SITE_M: f32 = 40.0;

/// A town wanted near where, in metres east and south of the start; its square goes on the
/// most level ground within `within` of there, and it is laid from the realm's seed
/// ([`super::seed_in`]).
#[derive(Resource, Clone, Copy, Debug)]
pub struct Wanted {
    pub near: Vec2,
    pub within: f32,
    /// Whether the player is taken there once it is laid: to its main street, looking
    /// toward the square.
    pub arrive: bool,
    /// Whether its people and animals are out: without them, what they cost is measured.
    pub lived_in: bool,
}

/// A run that takes the player to the town once it is laid (terra_sandbox's `town` and
/// `town-empty` experiments, the demos' walks): and whether its people are out. Without
/// it, the town is laid lived in and the player starts where the world starts.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Visit {
    pub lived_in: bool,
}

pub(super) fn plugin(app: &mut App) {
    // Each world wants its own town, or none (D469).
    app.world_resource::<Wanted>()
        .add_systems(OnEnter(InWorld), wanted_here.in_set(Building::Stage));
}

/// The world's first town wanted where its land stands one, as the world is entered.
fn wanted_here(
    mut commands: Commands,
    towns: Option<Res<Towns>>,
    visit: Option<Res<Visit>>,
    planet: Res<Planet>,
    start: Res<Start>,
) {
    let Some(site) = towns.and_then(|towns| towns.0.first().copied()) else {
        return;
    };
    let at = start.local(planet.surface(site.toward));
    commands.insert_resource(Wanted {
        near: Vec2::new(at.x, at.z),
        within: AT_THE_SITE_M,
        arrive: visit.is_some(),
        lived_in: visit.is_none_or(|visit| visit.lived_in),
    });
    info!(
        "Town wanted at the site the land's rules found: {:.0} m east and {:.0} m south of the start.",
        at.x, at.z
    );
}

/// How far round a square's middle a town reaches, for finding level ground for it.
const REACH_M: f32 = 60.0;

/// The most level ground for a town near a place, in metres east and south of the start:
/// of places on a grid within reach, the one whose ground over a town's reach rises and
/// falls least, the nearer winning a tie. The same on every machine: the planet decides it.
pub(super) fn site_for(planet: &Planet, start: &Start, near: Vec2, within: f32) -> Vec2 {
    let mut best = (f32::MAX, near);
    let steps = (within / 15.0).ceil() as i32;
    for i in -steps..=steps {
        for j in -steps..=steps {
            let at = near + Vec2::new(i as f32, j as f32) * 15.0;
            if at.distance(near) > within {
                continue;
            }
            let middle = planet.ground_at(start.at(at.x, at.y));
            let radius = middle.length();
            let (mut low, mut high) = (f32::MAX, f32::MIN);
            for ring in [0.3, 0.6, 1.0] {
                for k in 0..12 {
                    let angle = k as f32 / 12.0 * std::f32::consts::TAU;
                    let offset = Vec2::new(libm::cosf(angle), libm::sinf(angle)) * REACH_M * ring;
                    let p = at + offset;
                    let ground = planet.ground_at(start.at(p.x, p.y));
                    let height =
                        ground.length() - radius + offset.length_squared() / (2.0 * radius);
                    low = low.min(height);
                    high = high.max(height);
                }
            }
            let score = (high - low) + at.distance(near) * 0.002;
            if score < best.0 {
                best = (score, at);
            }
        }
    }
    best.1
}
