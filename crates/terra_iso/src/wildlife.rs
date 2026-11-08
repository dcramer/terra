//! The wood's beasts and birds, going about their day and minding the party: red deer
//! graze the clearing's edge and bolt when anyone comes near, hares crop the grass and
//! dash off, a boar roots by the trees and trots away only when pressed, finches and
//! sparrows peck in the open and flush into the air, mallards dabble on the pond.

use crate::forest::Rng;
use crate::nav::Nav;
use crate::party::{Actor, Member, creature};
use crate::terrain::{self, Land, POND, WATER};
use bevy::prelude::*;

pub const STAG: &str = "models/creatures/RedDeerStag.glb";
pub const HIND: &str = "models/creatures/RedDeerHind.glb";
pub const CALF: &str = "models/creatures/RedDeerCalf.glb";
pub const HARE: &str = "models/creatures/Hare.glb";
pub const BOAR: &str = "models/creatures/WildBoar.glb";
pub const FOX: &str = "models/creatures/Fox.glb";
pub const CHAFFINCH: &str = "models/creatures/Chaffinch.glb";
pub const SPARROW: &str = "models/creatures/Sparrow.glb";
pub const MALLARD: &str = "models/creatures/Mallard.glb";
pub const SQUIRREL: &str = "models/creatures/Squirrel.glb";
/// The forest scene's (wood/): the iso pipeline's, a walk its only clip.
pub const MOOSE: &str = "models/iso/moose.glb";

pub fn plugin(app: &mut App) {
    // After Startup, so the nav knows the trees, rocks and water it keeps off.
    app.add_systems(PostStartup, spawn)
        .add_systems(Update, live);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Habit {
    /// Walks, grazes, looks about, and runs from the party.
    Grazer,
    /// Hops and pecks, and flies off a way when the party comes near.
    Bird,
    /// Swims about the pond and dabbles, and swims off from the bank's walkers.
    Duck,
}

/// A beast's mind: where it keeps to, how near the party may come before it goes, how
/// far it goes, and what it is doing for how long.
#[derive(Component)]
pub struct Beast {
    habit: Habit,
    home: Vec2,
    reach: f32,
    wary: f32,
    flee: f32,
    timer: f32,
    fleeing: bool,
    /// A bird's flight: from, to, and how high it climbs.
    flight: Option<(Vec2, Vec2, f32)>,
    rng: Rng,
}

impl Beast {
    fn new(habit: Habit, home: Vec2, reach: f32, wary: f32, flee: f32, seed: u32) -> Self {
        let mut rng = Rng::new(seed * 7919 + 13);
        Beast {
            habit,
            home,
            reach,
            wary,
            flee,
            timer: rng.range(0.0, 3.0),
            fleeing: false,
            flight: None,
            rng,
        }
    }
}

fn spawn(mut commands: Commands, asset: Res<AssetServer>, nav: Res<Nav>, land: Res<Land>) {
    let count = std::cell::Cell::new(1u32);
    let next = || {
        count.set(count.get() + 1);
        count.get()
    };
    let put = |commands: &mut Commands, actor: Actor, at: Vec2, scale: f32, mut mind: Beast| {
        let yaw = mind.rng.f() * std::f32::consts::TAU;
        // The red deer's Tripo coats came out a pale fawn; a summer red deer is russet.
        let tint = match actor.model {
            STAG | HIND | CALF => Vec3::new(0.95, 0.62, 0.45),
            _ => Vec3::ONE,
        };
        let e = creature(commands, &asset, actor, at, yaw, scale);
        commands.entity(e).insert((mind, crate::rim::Tint(tint)));
    };
    let open = |p: Vec2| nav.nearest_open(p).unwrap_or(p);

    // A hind with her calf and a second hind, the stag a little apart and the first to go.
    let herd = Vec2::new(4.5, -3.0);
    for (model, off, scale, wary) in [
        (HIND, Vec2::new(0.0, 0.0), 1.0, 7.0),
        (CALF, Vec2::new(1.1, 0.6), 1.0, 7.0),
        (HIND, Vec2::new(-1.6, 1.3), 0.95, 7.0),
        (STAG, Vec2::new(2.6, -1.4), 1.0, 9.0),
    ] {
        let at = open(herd + off);
        let s = next();
        put(
            &mut commands,
            Actor::new(model, 1.2, 7.5, 1.6, 0.5),
            at,
            scale,
            Beast::new(Habit::Grazer, at, 3.0, wary, 16.0, s),
        );
    }
    // Hares out in the grass.
    for at in [
        Vec2::new(-6.5, -4.0),
        Vec2::new(3.5, 7.5),
        Vec2::new(-8.0, 3.5),
    ] {
        let at = open(at);
        let s = next();
        put(
            &mut commands,
            Actor::new(HARE, 0.7, 6.5, 0.5, 0.2),
            at,
            1.0,
            Beast::new(Habit::Grazer, at, 2.5, 5.5, 10.0, s),
        );
    }
    // A boar rooting at the wood's edge; he lets the party nearer and trots, not runs.
    let at = open(Vec2::new(-6.0, 6.5));
    let s = next();
    put(
        &mut commands,
        Actor::new(BOAR, 0.8, 2.8, 0.9, 0.45),
        at,
        1.0,
        Beast::new(Habit::Grazer, at, 2.0, 4.0, 7.0, s),
    );
    // A fox nosing along the ridge's foot.
    let at = open(Vec2::new(-3.0, -8.0));
    let s = next();
    put(
        &mut commands,
        Actor::new(FOX, 0.9, 6.0, 0.6, 0.25),
        at,
        1.0,
        Beast::new(Habit::Grazer, at, 4.0, 8.0, 12.0, s),
    );
    // Finches and sparrows pecking in the open, in two little flocks.
    for (centre, n) in [(Vec2::new(-4.0, -3.5), 6), (Vec2::new(2.5, 0.5), 4)] {
        for i in 0..n {
            let mut r = Rng::new(900 + i * 17 + centre.x as u32);
            let at = open(centre + Vec2::new(r.range(-1.2, 1.2), r.range(-1.2, 1.2)));
            let model = if i % 2 == 0 { CHAFFINCH } else { SPARROW };
            let s = next();
            put(
                &mut commands,
                Actor::new(model, 0.35, 5.5, 0.15, 0.08),
                at,
                // Larger than life, as the view is far.
                1.7,
                Beast::new(Habit::Bird, at, 1.0, 4.5, 14.0, s),
            );
        }
    }
    // Mallards on the pond.
    for off in [
        Vec2::new(-1.5, 0.5),
        Vec2::new(0.8, -1.0),
        Vec2::new(1.6, 1.2),
    ] {
        let at = POND + off;
        let s = next();
        let mut duck = Actor::new(MALLARD, 0.35, 1.2, 0.5, 0.2);
        duck.swims = true;
        put(
            &mut commands,
            duck,
            at,
            // True to life: at 1.5 a flying drake spanned about 1.3 m beside the 1.75 m
            // knight (judge 261107-0507); a mallard's is 0.8 to 1 m, its body 0.6 m.
            1.0,
            Beast::new(Habit::Duck, POND, 3.5, 4.0, 3.0, s),
        );
    }
    let _ = land;
}

/// Whether a beast may stand at a point: on the land, dry, not too steep, open.
fn fit(nav: &Nav, land: &Land, p: Vec2) -> bool {
    p.abs().max_element() < crate::nav::HALF - 1.0
        && land.height(p) > WATER + 0.03
        && terrain::pond(p) < -0.1
        && land.slope(p) < 0.5
        && nav.nearest_open(p).is_some_and(|q| q.distance(p) < 0.3)
}

fn live(
    time: Res<Time>,
    nav: Res<Nav>,
    land: Res<Land>,
    party: Query<&Transform, (With<Member>, Without<Beast>)>,
    mut beasts: Query<(&mut Actor, &mut Beast, &Transform)>,
) {
    let dt = time.delta_secs();
    if dt == 0.0 {
        return;
    }
    for (mut actor, mut mind, tr) in &mut beasts {
        let here = tr.translation.xz();
        let near = party
            .iter()
            .map(|m| m.translation.xz())
            .min_by(|a, b| a.distance(here).total_cmp(&b.distance(here)));
        let threat = near.filter(|p| p.distance(here) < mind.wary);
        match mind.habit {
            Habit::Grazer => graze(&mut actor, &mut mind, here, threat, dt, &nav, &land),
            Habit::Bird => peck(&mut actor, &mut mind, here, threat, dt, &nav, &land),
            Habit::Duck => dabble(&mut actor, &mut mind, here, threat, dt),
        }
    }
}

/// Away from a threat, a little to one side, as far as `flee`.
fn away(mind: &mut Beast, here: Vec2, from: Vec2) -> Vec2 {
    let dir = (here - from).try_normalize().unwrap_or(Vec2::X);
    let turn = mind.rng.range(-0.6, 0.6);
    here + Vec2::from_angle(turn).rotate(dir) * mind.flee * mind.rng.range(0.8, 1.2)
}

#[allow(clippy::too_many_arguments)]
fn graze(
    actor: &mut Actor,
    mind: &mut Beast,
    here: Vec2,
    threat: Option<Vec2>,
    dt: f32,
    nav: &Nav,
    land: &Land,
) {
    if let Some(from) = threat
        && !mind.fleeing
    {
        // Bolt: try a few ways out, away from whoever came near.
        for _ in 0..6 {
            let to = away(mind, here, from);
            if fit(nav, land, to)
                && let Some(path) = nav.path(here, to)
            {
                actor.path = path.into();
                actor.running = true;
                actor.busy = false;
                mind.fleeing = true;
                mind.home = to;
                return;
            }
        }
    }
    if !actor.path.is_empty() {
        return;
    }
    if mind.fleeing {
        // Stopped: stand alert a while, then go back to grazing here.
        mind.fleeing = false;
        actor.running = false;
        mind.timer = mind.rng.range(2.5, 5.0);
        actor.busy = false;
        return;
    }
    mind.timer -= dt;
    if mind.timer > 0.0 {
        return;
    }
    let roll = mind.rng.f();
    if roll < 0.55 {
        actor.busy = true;
        mind.timer = mind.rng.range(4.0, 10.0);
    } else if roll < 0.8 {
        actor.busy = false;
        mind.timer = mind.rng.range(1.5, 4.0);
    } else {
        actor.busy = false;
        let a = mind.rng.f() * std::f32::consts::TAU;
        let to = mind.home + Vec2::from_angle(a) * mind.reach * mind.rng.f().sqrt();
        if fit(nav, land, to)
            && let Some(path) = nav.path(here, to)
        {
            actor.path = path.into();
        }
        mind.timer = mind.rng.range(0.5, 2.0);
    }
}

#[allow(clippy::too_many_arguments)]
fn peck(
    actor: &mut Actor,
    mind: &mut Beast,
    here: Vec2,
    threat: Option<Vec2>,
    dt: f32,
    nav: &Nav,
    land: &Land,
) {
    if let Some((from, to, high)) = mind.flight {
        // In the air: climb and come down along the flight, landing at its end.
        let whole = from.distance(to).max(0.1);
        let done = (1.0 - here.distance(to) / whole).clamp(0.0, 1.0);
        actor.lift = high * libm::powf(libm::sinf(done * std::f32::consts::PI), 0.6);
        if actor.path.is_empty() {
            actor.lift = 0.0;
            actor.running = false;
            mind.flight = None;
            mind.home = to;
            mind.timer = mind.rng.range(0.5, 2.0);
        }
        return;
    }
    if let Some(from) = threat {
        for _ in 0..6 {
            let to = away(mind, here, from);
            if fit(nav, land, to) {
                actor.path = [to].into();
                actor.running = true;
                actor.busy = false;
                mind.flight = Some((here, to, mind.rng.range(2.5, 4.5)));
                return;
            }
        }
    }
    if !actor.path.is_empty() {
        return;
    }
    mind.timer -= dt;
    if mind.timer > 0.0 {
        return;
    }
    let roll = mind.rng.f();
    if roll < 0.6 {
        actor.busy = true;
        mind.timer = mind.rng.range(0.6, 2.0);
    } else if roll < 0.75 {
        actor.busy = false;
        mind.timer = mind.rng.range(0.4, 1.2);
    } else {
        // A few hops.
        actor.busy = false;
        let a = mind.rng.f() * std::f32::consts::TAU;
        let to = mind.home + Vec2::from_angle(a) * mind.reach * mind.rng.f();
        if fit(nav, land, to) {
            actor.path = [to].into();
        }
        mind.timer = mind.rng.range(0.2, 0.8);
    }
}

fn dabble(actor: &mut Actor, mind: &mut Beast, here: Vec2, threat: Option<Vec2>, dt: f32) {
    let wet = |p: Vec2| terrain::pond(p) > 0.35;
    if let Some(from) = threat
        && !mind.fleeing
    {
        for _ in 0..6 {
            let to = away(mind, here, from);
            if wet(to) {
                actor.path = [to].into();
                actor.busy = false;
                mind.fleeing = true;
                return;
            }
        }
    }
    if !actor.path.is_empty() {
        return;
    }
    mind.fleeing = false;
    mind.timer -= dt;
    if mind.timer > 0.0 {
        return;
    }
    let roll = mind.rng.f();
    if roll < 0.3 {
        actor.busy = true;
        mind.timer = mind.rng.range(2.0, 5.0);
    } else if roll < 0.5 {
        actor.busy = false;
        mind.timer = mind.rng.range(2.0, 4.0);
    } else {
        actor.busy = false;
        let a = mind.rng.f() * std::f32::consts::TAU;
        let to = mind.home + Vec2::from_angle(a) * mind.reach * mind.rng.f().sqrt();
        if wet(to) {
            actor.path = [to].into();
        }
        mind.timer = mind.rng.range(0.5, 1.5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beast(habit: Habit, at: Vec2) -> (Actor, Beast) {
        (
            Actor::new(HIND, 1.2, 7.5, 1.6, 0.5),
            Beast::new(habit, at, 3.0, 7.0, 16.0, 3),
        )
    }

    #[test]
    fn a_deer_runs_away_from_whoever_comes_near_and_settles_again() {
        let (land, nav) = (Land::new(), Nav::default());
        let at = Vec2::new(4.5, -3.0);
        let (mut deer, mut mind) = beast(Habit::Grazer, at);
        let walker = at + Vec2::new(-4.0, 0.0);
        graze(&mut deer, &mut mind, at, Some(walker), 0.016, &nav, &land);
        assert!(deer.running, "it runs");
        let end = *deer.path.back().expect("it has somewhere to go");
        assert!(
            end.distance(walker) > at.distance(walker) + 8.0,
            "away from the walker: {end}"
        );
        // Arrived, it stands alert, then grazes or wanders on, no longer running.
        deer.path.clear();
        graze(&mut deer, &mut mind, end, None, 0.016, &nav, &land);
        assert!(!deer.running && !mind.fleeing);
    }

    #[test]
    fn a_bird_flushes_into_the_air_and_lands_away() {
        let (land, nav) = (Land::new(), Nav::default());
        let at = Vec2::new(-4.0, -3.5);
        let (mut bird, mut mind) = beast(Habit::Bird, at);
        mind.flee = 14.0;
        peck(
            &mut bird,
            &mut mind,
            at,
            Some(at + Vec2::X),
            0.016,
            &nav,
            &land,
        );
        let to = *bird.path.back().expect("it flies somewhere");
        // Halfway, it is up in the air; at the end, down.
        peck(
            &mut bird,
            &mut mind,
            at.lerp(to, 0.5),
            None,
            0.016,
            &nav,
            &land,
        );
        assert!(bird.lift > 2.0, "lift {}", bird.lift);
        bird.path.clear();
        peck(&mut bird, &mut mind, to, None, 0.016, &nav, &land);
        assert_eq!(bird.lift, 0.0);
        assert!(mind.flight.is_none());
    }
}
