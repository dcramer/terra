//! Loose things swing (D189): what hangs from a strap or a belt, such as a quiver, swings
//! about where it hangs as the body moves, falls back under gravity, and is kept off the
//! thighs and seat. It is the spring-bone model VRM avatars use (`VRMC_springBone`, MIT), as
//! Source's jiggle bones: a tail point carries its own momentum, less a drag, is pulled toward
//! where it rests and down by gravity, and is held at its length from the pivot and outside
//! the body's capsules. The look turns to follow its tail.
//!
//! The part of a look that swings says so in its glTF extras: `{"swing": {"tail": [x, y, z],
//! "stiffness": 0.8, "drag": 0.4, "gravity": 0.6}}`, its tail where it rests in its own
//! space (glTF's axes), and swings about its own origin, where it hangs from (a quiver's
//! mouth), within the look hung on its socket. The capsules are
//! data too, `models/character/colliders.json`. Nothing here names an item.
//!
//! It runs on last frame's places, so it lags its socket by a frame, which a swing hides.

use bevy::prelude::*;
use serde::Deserialize;

use crate::character::Body;

/// How a loose look swings, from its glTF extras. Reflected, so it can be read and set live
/// over the remote protocol while tuning (`tools/sandbox`).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Reflect)]
pub struct Springiness {
    /// Where its tail rests, from its pivot, in its own space (glTF's axes).
    pub tail: [f32; 3],
    /// How strongly it is pulled back toward where it rests.
    pub stiffness: f32,
    /// How much of its motion it loses each step, 0 to 1.
    pub drag: f32,
    /// How strongly it falls.
    pub gravity: f32,
    /// How far round its tail it keeps from the body.
    #[serde(default = "Springiness::default_hit")]
    pub hit_radius: f32,
}

impl Springiness {
    fn default_hit() -> f32 {
        0.03
    }

    /// From a look's glTF extras, the value under `"swing"`.
    pub fn from_extras(value: serde_json::Value) -> Option<Springiness> {
        serde_json::from_value(value).ok()
    }
}

/// A part of a look that swings, about its own origin, within the look on its socket.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct Swing {
    pub spring: Springiness,
    /// How far it now hangs from where it rests, in degrees: what tuning watches.
    pub swung: f32,
    /// Its tail now and a step before, in world space, once it has been placed.
    #[reflect(ignore)]
    tail: Option<(Vec3, Vec3)>,
    /// Its turn at rest in its parent's space.
    #[reflect(ignore)]
    rest: Option<Quat>,
    /// The body's capsules, as joints and a radius, once found.
    #[reflect(ignore)]
    capsules: Option<Vec<(Entity, Entity, f32)>>,
}

impl Swing {
    pub fn new(spring: Springiness) -> Self {
        Swing {
            spring,
            swung: 0.0,
            tail: None,
            rest: None,
            capsules: None,
        }
    }
}

#[derive(Deserialize)]
struct Capsule {
    from: String,
    to: String,
    radius: f32,
}

#[derive(Deserialize)]
struct Colliders {
    capsules: Vec<Capsule>,
}

/// The capsules round every body that loose looks swing against (`colliders.json`).
#[derive(Resource)]
struct BodyCapsules(Vec<Capsule>);

pub fn plugin(app: &mut App) {
    let colliders: Colliders = serde_json::from_str(include_str!(
        "../../../assets/models/character/colliders.json"
    ))
    .unwrap_or_else(|error| {
        error!("models/character/colliders.json: {error}");
        Colliders { capsules: vec![] }
    });
    app.insert_resource(BodyCapsules(colliders.capsules))
        .register_type::<Swing>()
        .add_systems(Update, swing);
}

/// One step of the spring: where the tail goes next, from where it is (`tail`) and was
/// (`before`), about `pivot`, resting along `rest` (a unit direction, world space), `length`
/// from the pivot, falling along `down`, kept outside `capsules` (from, to, radius).
pub fn step(
    spring: &Springiness,
    tail: Vec3,
    before: Vec3,
    pivot: Vec3,
    rest: Vec3,
    length: f32,
    down: Vec3,
    capsules: &[(Vec3, Vec3, f32)],
    dt: f32,
) -> Vec3 {
    let inertia = (tail - before) * (1.0 - spring.drag);
    let mut next = tail + inertia + rest * spring.stiffness * dt + down * spring.gravity * dt;
    next = pivot + (next - pivot).normalize_or(rest) * length;
    for &(a, b, radius) in capsules {
        let along = b - a;
        let t = ((next - a).dot(along) / along.length_squared().max(1e-6)).clamp(0.0, 1.0);
        let nearest = a + along * t;
        let away = next - nearest;
        let reach = radius + spring.hit_radius;
        if away.length() < reach {
            next = nearest + away.normalize_or(rest) * reach;
            next = pivot + (next - pivot).normalize_or(rest) * length;
        }
    }
    next
}

/// Each loose look's tail one step on, and the look turned to follow it.
fn swing(
    time: Res<Time>,
    capsules: Res<BodyCapsules>,
    mut looks: Query<(Entity, &mut Swing, &mut Transform, &ChildOf)>,
    places: Query<&GlobalTransform>,
    parents: Query<&ChildOf>,
    bodies: Query<(), With<Body>>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    // A long frame (a hitch, a load) is taken as one ordinary step, not a leap.
    let dt = time.delta_secs().min(1.0 / 30.0);
    if dt <= 0.0 {
        return;
    }
    for (look, mut swing, mut transform, socket) in &mut looks {
        // Its parent: the look's root, on the socket.
        let Ok(socket_place) = places.get(socket.parent()) else {
            continue;
        };
        let rest = *swing.rest.get_or_insert(transform.rotation);
        if swing.capsules.is_none() {
            // The body it hangs on, up its parents; its joints by name.
            let body = parents
                .iter_ancestors(look)
                .find(|each| bodies.contains(*each));
            if let Some(body) = body {
                let joint = |name: &str| crate::character::find(body, name, &children, &names);
                swing.capsules = Some(
                    capsules
                        .0
                        .iter()
                        .filter_map(|c| Some((joint(&c.from)?, joint(&c.to)?, c.radius)))
                        .collect(),
                );
            }
        }
        let shape: Vec<(Vec3, Vec3, f32)> = swing
            .capsules
            .iter()
            .flatten()
            .filter_map(|&(a, b, r)| {
                Some((
                    places.get(a).ok()?.translation(),
                    places.get(b).ok()?.translation(),
                    r,
                ))
            })
            .collect();
        let (_, socket_turn, _) = socket_place.to_scale_rotation_translation();
        let tail_local = Vec3::from(swing.spring.tail);
        let length = tail_local.length();
        if length <= 1e-4 {
            continue;
        }
        let rest_dir = (socket_turn * rest * tail_local).normalize();
        let pivot = socket_place.transform_point(transform.translation);
        let down = -pivot.normalize_or(Vec3::Y);
        let (now, before) = match swing.tail {
            // Placed afresh when first seen, or when it jumped (a teleport, a respawn).
            Some((now, before)) if now.distance(pivot) < length * 4.0 => (now, before),
            _ => {
                let at = pivot + rest_dir * length;
                (at, at)
            }
        };
        let next = step(
            &swing.spring,
            now,
            before,
            pivot,
            rest_dir,
            length,
            down,
            &shape,
            dt,
        );
        swing.tail = Some((next, now));
        // Turned in its socket's space so its tail points where the spring put it.
        let toward = socket_turn.inverse() * (next - pivot).normalize_or(rest_dir);
        let resting = socket_turn.inverse() * rest_dir;
        transform.rotation = Quat::from_rotation_arc(resting, toward) * rest;
        swing.swung = resting.angle_between(toward).to_degrees();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPRING: Springiness = Springiness {
        tail: [0.0, -0.3, 0.0],
        stiffness: 0.8,
        drag: 0.4,
        gravity: 0.6,
        hit_radius: 0.03,
    };

    /// Runs the spring for `steps` at 60 a second with the pivot moved by `pivot(k)`, its
    /// tail resting along `rest` and falling along -Y.
    fn run(
        steps: usize,
        pivot: impl Fn(usize) -> Vec3,
        rest: Vec3,
        capsules: &[(Vec3, Vec3, f32)],
    ) -> Vec<Vec3> {
        let length = 0.3;
        let mut now = pivot(0) + rest * length;
        let mut before = now;
        let mut out = vec![];
        for k in 0..steps {
            let at = pivot(k);
            let next = step(
                &SPRING,
                now,
                before,
                at,
                rest,
                length,
                Vec3::NEG_Y,
                capsules,
                1.0 / 60.0,
            );
            before = now;
            now = next;
            out.push(now - at);
        }
        out
    }

    #[test]
    fn a_tail_resting_sideways_sags_until_its_pulls_balance() {
        // Resting along +X and falling along -Y, it settles where the pull back toward its
        // rest and its weight add up to a pull straight along it: it sags, but holds.
        let tails = run(600, |_| Vec3::ZERO, Vec3::X, &[]);
        let last = *tails.last().unwrap();
        let balance = (Vec3::X * SPRING.stiffness + Vec3::NEG_Y * SPRING.gravity).normalize();
        assert!(last.distance(balance * 0.3) < 1e-3, "{last}");
    }

    #[test]
    fn it_trails_when_the_body_moves_off_and_settles_once_it_stops() {
        // Walked forward along +X for a second, then stood still.
        let walk = |k: usize| Vec3::X * (k.min(60) as f32 / 60.0) * 1.5;
        let tails = run(360, walk, Vec3::NEG_Y, &[]);
        assert!(
            tails[30].x < -0.01,
            "it lags behind while moving: {}",
            tails[30]
        );
        let settled = tails.last().unwrap();
        assert!(
            settled.x.abs() < 0.01,
            "and hangs plumb again after: {settled}"
        );
        // Always the same length from where it hangs.
        assert!(tails.iter().all(|t| (t.length() - 0.3).abs() < 1e-4));
    }

    #[test]
    fn it_never_swings_into_a_thigh() {
        // A thigh just in front of where it hangs, and the body moving backwards into it.
        let thigh = (Vec3::new(0.08, 0.0, 0.0), Vec3::new(0.08, -0.5, 0.0), 0.06);
        let back = |k: usize| Vec3::NEG_X * (k.min(30) as f32 / 30.0) * 0.5;
        let tails = run(200, back, Vec3::NEG_Y, &[(thigh.0, thigh.1, thigh.2)]);
        for (k, tail) in tails.iter().enumerate() {
            // The thigh stands still in the world; the tail, kept relative to the pivot, is
            // put back in the world to measure.
            let world = back(k) + *tail;
            let along = thigh.1 - thigh.0;
            let t = ((world - thigh.0).dot(along) / along.length_squared()).clamp(0.0, 1.0);
            let clear = world.distance(thigh.0 + along * t);
            assert!(
                clear >= thigh.2 + SPRING.hit_radius - 1e-4,
                "step {k}: {world}"
            );
        }
    }

    /// Where the parts hang in `hung`: the look's root on its socket, 10 m up, so the world's
    /// down there is -Y.
    const ROOT_AT: Vec3 = Vec3::new(0.0, 10.0, 0.0);

    /// A body, a look's root on it turned `turn` in the world, and on that a part resting
    /// turned `rest` that swings; and the part.
    fn hung(turn: Quat, rest: Quat) -> (World, Entity, Entity) {
        let mut world = World::new();
        world.insert_resource(BodyCapsules(vec![]));
        world.insert_resource(Time::<()>::default());
        let body = world.spawn(Body).id();
        let root = world
            .spawn((
                GlobalTransform::from(Transform::from_translation(ROOT_AT).with_rotation(turn)),
                ChildOf(body),
            ))
            .id();
        let part = world
            .spawn((
                Swing::new(SPRING),
                Transform::from_rotation(rest),
                ChildOf(root),
            ))
            .id();
        (world, root, part)
    }

    fn frame(world: &mut World, seconds: f32) {
        use bevy::ecs::system::RunSystemOnce;
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(seconds));
        world.run_system_once(swing).unwrap();
    }

    /// Which way the part's tail points in the world, as it is drawn.
    fn drawn(world: &World, turn: Quat, part: Entity) -> Vec3 {
        let rotation = world.get::<Transform>(part).unwrap().rotation;
        (turn * rotation * Vec3::from(SPRING.tail)).normalize()
    }

    /// A socket turned a quarter about Z and a part turned 40 degrees about X within it: its
    /// tail, down in its own space, rests level in the world.
    fn turns() -> (Quat, Quat) {
        (
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            Quat::from_rotation_x(40f32.to_radians()),
        )
    }

    #[test]
    fn a_part_turned_on_a_turned_socket_sags_from_where_it_rests_in_the_world() {
        let (turn, rest) = turns();
        let (mut world, _, part) = hung(turn, rest);
        for _ in 0..600 {
            frame(&mut world, 1.0 / 60.0);
        }
        let resting = (turn * rest * Vec3::from(SPRING.tail)).normalize();
        assert!(resting.y.abs() < 1e-5, "the fixture rests level: {resting}");
        // Settled where the pull back toward its rest and its weight balance, as the tail is
        // drawn, not only as the spring has it.
        let balance = (resting * SPRING.stiffness + Vec3::NEG_Y * SPRING.gravity).normalize();
        let tail = drawn(&world, turn, part);
        assert!(tail.distance(balance) < 1e-3, "{tail} against {balance}");
        let swung = world.get::<Swing>(part).unwrap().swung;
        assert!(
            (swung - resting.angle_between(balance).to_degrees()).abs() < 0.1,
            "{swung}"
        );
    }

    #[test]
    fn a_part_whose_body_jumps_is_hung_afresh_where_it_rests() {
        let (turn, rest) = turns();
        let (mut world, root, part) = hung(turn, rest);
        for _ in 0..120 {
            frame(&mut world, 1.0 / 60.0);
        }
        assert!(world.get::<Swing>(part).unwrap().swung > 20.0);
        // Teleported 100 m: not dragged after its old place, but placed at rest and one
        // step on from there.
        world.entity_mut(root).insert(GlobalTransform::from(
            Transform::from_translation(ROOT_AT + Vec3::X * 100.0).with_rotation(turn),
        ));
        frame(&mut world, 1.0 / 60.0);
        let swung = world.get::<Swing>(part).unwrap().swung;
        assert!(swung < 5.0, "{swung}");
    }

    #[test]
    fn a_long_frame_is_taken_as_one_ordinary_step() {
        let (turn, rest) = turns();
        let (mut hitch, _, hitched) = hung(turn, rest);
        frame(&mut hitch, 1.0);
        let (mut ordinary, _, part) = hung(turn, rest);
        frame(&mut ordinary, 1.0 / 30.0);
        let (a, b) = (drawn(&hitch, turn, hitched), drawn(&ordinary, turn, part));
        assert!(ordinary.get::<Swing>(part).unwrap().swung > 1.0);
        assert!(a.distance(b) < 1e-5, "{a} against {b}");
    }

    #[test]
    fn the_shipped_capsules_run_between_joints_the_skeleton_has() {
        // Read without the fallback the plugin has, which would leave a broken file unseen;
        // a joint the body lacks drops its capsule as quietly.
        let colliders: Colliders = serde_json::from_str(include_str!(
            "../../../assets/models/character/colliders.json"
        ))
        .unwrap();
        assert!(!colliders.capsules.is_empty());
        let joints = crate::model_files::joints(&crate::model_files::json(crate::appearance::BASE));
        for capsule in &colliders.capsules {
            for end in [&capsule.from, &capsule.to] {
                assert!(
                    joints.contains(end),
                    "{end} is not a joint of the base body"
                );
            }
            assert!(capsule.radius > 0.0);
        }
    }
}
