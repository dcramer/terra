//! The view shaking: a blow landing or taken (`hits`), a collapse nearby (`terra_building::loss`),
//! anything that shakes the view kicks [`Shake`], and the camera is moved a little off where
//! it was put until the shake runs down. A blow landing in a fight punches it instead
//! ([`Shake::punch`], D456): the view tipped a little at once and eased back, as Source's view
//! punch kicks the camera. The walker's, since the view is: what kicks it names it from above.

use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.init_resource::<Shake>()
        .add_systems(Update, run_down)
        .add_systems(
            PostUpdate,
            shake
                .after(crate::camera::follow)
                .before(TransformSystems::Propagate)
                .before(terra_world::remote::CameraHold),
        );
}

/// The view shaking: how much longer, how far at the start, and for how long in all; and the
/// punch showing. Whatever shakes the view kicks it ([`Shake::kick`]) or punches it
/// ([`Shake::punch`]).
#[derive(Resource, Default)]
pub struct Shake {
    left: f32,
    reach: f32,
    length: f32,
    punch: Punch,
}

/// A punch of the view: how far it tipped the view, how long it takes to settle, and how long
/// ago it landed.
#[derive(Default, Clone, Copy, Debug, PartialEq)]
struct Punch {
    deg: f32,
    length: f32,
    age: f32,
}

impl Shake {
    /// Shakes the view so far, in metres, for so long, unless it is shaking harder.
    pub fn kick(&mut self, reach: f32, seconds: f32) {
        let now = if self.length > 0.0 {
            self.reach * self.left / self.length
        } else {
            0.0
        };
        if reach > now {
            self.left = seconds;
            self.reach = reach;
            self.length = seconds;
        }
    }

    /// Punches the view: tipped down `deg` at once, easing back to rest over `seconds`
    /// ([`punched`]), unless a harder punch is still showing.
    pub fn punch(&mut self, deg: f32, seconds: f32) {
        if deg >= self.punched_deg() {
            self.punch = Punch {
                deg,
                length: seconds,
                age: 0.0,
            };
        }
    }

    /// How far the punch showing tips the view now, degrees.
    pub fn punched_deg(&self) -> f32 {
        punched(self.punch.deg, self.punch.length, self.punch.age)
    }
}

/// How far a punch of `deg` that settles over `length` seconds tips the view `age` seconds
/// after it landed: all of it at once (the kick), then easing out to rest as the square of
/// the time left, so it settles without a bump.
pub fn punched(deg: f32, length: f32, age: f32) -> f32 {
    if length <= 0.0 || !(0.0..length).contains(&age) {
        return 0.0;
    }
    let left = 1.0 - age / length;
    deg * left * left
}

/// The shake runs down; it is not touched once it has, so what draws it is not woken for
/// nothing. What kicks it in a frame runs before this (`hits` orders its own).
pub fn run_down(time: Res<Time>, mut shake: ResMut<Shake>) {
    if shake.left > 0.0 {
        shake.left = (shake.left - time.delta_secs()).max(0.0);
    }
}

/// Moves the camera a little off where it was put, while shaking, and tips it while punched;
/// a punch ages once it has been shown, so its first frame shows all of it.
fn shake(
    time: Res<Time>,
    mut shaking: ResMut<Shake>,
    mut camera: Single<&mut Transform, With<Camera3d>>,
) {
    if shaking.left > 0.0 {
        let fading = shaking.left / shaking.length.max(1e-3);
        let t = time.elapsed_secs();
        let right = camera.right();
        let up = camera.up();
        let off = (right * libm::sinf(t * 53.0) + up * libm::sinf(t * 71.0 + 1.3))
            * shaking.reach
            * fading;
        camera.translation += off;
    }
    let punch = shaking.punch;
    if punch.length > 0.0 && punch.age < punch.length {
        camera.rotate_local_x(-shaking.punched_deg().to_radians());
        shaking.punch.age += time.delta_secs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use std::time::Duration;

    #[test]
    fn a_kick_shakes_unless_harder_already_and_runs_down_to_rest() {
        let mut world = World::new();
        world.init_resource::<Time>();
        world.init_resource::<Shake>();
        world.resource_mut::<Shake>().kick(0.09, 0.3);
        // A lighter kick while the harder one shakes leaves it be.
        world.resource_mut::<Shake>().kick(0.02, 1.0);
        assert_eq!(world.resource::<Shake>().length, 0.3);
        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.2));
        world.run_system_once(run_down).unwrap();
        assert!((world.resource::<Shake>().left - 0.1).abs() < 1e-5);
        world.run_system_once(run_down).unwrap();
        assert_eq!(world.resource::<Shake>().left, 0.0);
    }

    /// A punch tips the view down all of its size on the frame it lands, less each frame
    /// after, and the camera is where it was put again once it has settled; a lighter punch
    /// while a harder one shows leaves it be.
    #[test]
    fn a_punch_tips_the_view_down_at_once_and_settles_back_where_it_was_put() {
        let mut world = World::new();
        world.init_resource::<Time>();
        world.init_resource::<Shake>();
        let put = Transform::from_xyz(0.0, 2.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y);
        let camera = world.spawn((Camera3d::default(), put)).id();
        world.resource_mut::<Shake>().punch(1.0, 0.1);
        world.resource_mut::<Shake>().punch(0.3, 1.0);
        let mut tipped = Vec::new();
        for _ in 0..12 {
            world
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(1.0 / 60.0));
            // The camera follows the player first each frame, putting it afresh.
            *world.get_mut::<Transform>(camera).unwrap() = put;
            world.run_system_once(shake).unwrap();
            let now = world.get::<Transform>(camera).unwrap();
            let down = put.forward().angle_between(*now.forward()).to_degrees();
            // Tipped down, about the camera's own right: its forward drops, its right stays.
            assert!(down < 1e-3 || now.forward().y < put.forward().y, "{down}");
            assert!(now.right().angle_between(*put.right()) < 1e-4);
            tipped.push(down);
        }
        assert!(
            (tipped[0] - 1.0).abs() < 0.01,
            "the whole punch at once: {tipped:?}"
        );
        assert!(tipped.windows(2).all(|w| w[1] <= w[0] + 1e-4), "{tipped:?}");
        assert!(
            tipped[6..].iter().all(|&deg| deg < 1e-3),
            "settled: {tipped:?}"
        );
    }
}
