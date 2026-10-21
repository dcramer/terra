//! The view shaking: a blow landing or taken (`hits`), a collapse nearby (`build::loss`),
//! anything that shakes the view kicks [`Shake`], and the camera is moved a little off where
//! it was put until the shake runs down. The walker's, since the view is: what kicks it
//! names it from above.

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

/// The view shaking: how much longer, how far at the start, and for how long in all.
/// Whatever shakes the view kicks it ([`Shake::kick`]).
#[derive(Resource, Default)]
pub struct Shake {
    left: f32,
    reach: f32,
    length: f32,
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
            *self = Shake {
                left: seconds,
                reach,
                length: seconds,
            };
        }
    }
}

/// The shake runs down; it is not touched once it has, so what draws it is not woken for
/// nothing. What kicks it in a frame runs before this (`hits` orders its own).
pub fn run_down(time: Res<Time>, mut shake: ResMut<Shake>) {
    if shake.left > 0.0 {
        shake.left = (shake.left - time.delta_secs()).max(0.0);
    }
}

/// Moves the camera a little off where it was put, while shaking.
fn shake(time: Res<Time>, shaking: Res<Shake>, mut camera: Single<&mut Transform, With<Camera3d>>) {
    if shaking.left <= 0.0 {
        return;
    }
    let fading = shaking.left / shaking.length.max(1e-3);
    let t = time.elapsed_secs();
    let right = camera.right();
    let up = camera.up();
    let off =
        (right * libm::sinf(t * 53.0) + up * libm::sinf(t * 71.0 + 1.3)) * shaking.reach * fading;
    camera.translation += off;
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
}
