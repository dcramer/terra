//! Plants make way (D271), as shipped open worlds have them: shrubs, ferns and grass bend
//! away from whoever walks through them (foliage interaction; Ghost of Tsushima's grass
//! takes characters' displacement), and foliage between a third-person camera and the one
//! it follows dithers away, as does foliage near the lens (a camera-obstruction fade, as
//! Unreal games dither theirs), so the camera never looks through a bush it has backed into
//! and the figure is not lost in one. Shadows keep everything. Notes:
//! research/foliage-near-camera.md.
//!
//! The foliage and grass shaders read one small buffer that every one of their materials
//! shares ([`Parting`]): the main camera's eye, the point it follows ([`CameraSubject`]),
//! and the bodies nearest it that part plants ([`PartsPlants`]), each where it stands this
//! frame and stood the last, so motion vectors follow a plant pushed aside. It is written
//! in place each frame; no material changes.

use crate::planet::MainCamera;
use bevy::{platform::collections::HashMap, prelude::*, render::storage::ShaderBuffer};

/// At most this many bodies part plants at once: those nearest the camera. Must match
/// `PUSHERS` in foliage.wesl and grass.wesl.
pub const PUSHERS: usize = 8;
/// Bodies farther than this from the camera are not looked at.
const REACH_M: f32 = 60.0;

/// Something that pushes plants aside as it moves through them: a body standing at its
/// translation, so tall and so wide round. The layers above give it to whoever walks.
#[derive(Component, Clone, Copy, Debug)]
pub struct PartsPlants {
    pub radius: f32,
    pub height: f32,
}

impl Default for PartsPlants {
    fn default() -> Self {
        Self {
            radius: 0.4,
            height: 1.8,
        }
    }
}

/// The point the main camera keeps in view from behind, if it follows one: foliage between
/// it and the camera dithers away. Set by whatever steers the camera.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CameraSubject(pub Option<Vec3>);

/// The buffer the foliage and grass materials read, at binding 101 (D271).
#[derive(Resource, Clone)]
pub struct Parting(pub Handle<ShaderBuffer>);

impl FromWorld for Parting {
    fn from_world(world: &mut World) -> Self {
        let mut buffers = world.resource_mut::<Assets<ShaderBuffer>>();
        Self(buffers.add(ShaderBuffer::new(
            vec![[0.0f32; 4]; layout_len()],
            default(),
        )))
    }
}

/// The buffer's length in vec4s: the eye, the subject, then four for each body (its feet
/// and head now, its feet and head a frame ago; the feet's w is its radius).
const fn layout_len() -> usize {
    2 + 4 * PUSHERS
}

pub fn plugin(app: &mut App) {
    // Without rendering (as in tests) there is no buffer to write.
    if app.world().contains_resource::<Assets<ShaderBuffer>>() {
        app.init_resource::<Parting>()
            .add_systems(PostUpdate, write.after(TransformSystems::Propagate));
    }
}

/// Where a body stands, for the shaders: its feet with its radius, and its head.
fn ends(at: Vec3, body: &PartsPlants) -> [Vec4; 2] {
    let up = at.normalize_or(Vec3::Y);
    [at.extend(body.radius), (at + up * body.height).extend(0.0)]
}

/// The buffer's contents: the eye (w 1 if it has a subject), the subject (w how many
/// bodies), and the bodies nearest the eye, now and a frame ago. A body not seen last frame
/// stood where it stands now.
fn contents(
    eye: Vec3,
    subject: Option<Vec3>,
    bodies: impl Iterator<Item = (Entity, Vec3, PartsPlants)>,
    last: &mut HashMap<Entity, [Vec4; 2]>,
) -> Vec<[f32; 4]> {
    let mut near: Vec<_> = bodies
        .map(|(entity, at, body)| (at.distance_squared(eye), entity, ends(at, &body)))
        .filter(|(far, ..)| *far < REACH_M * REACH_M)
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    near.truncate(PUSHERS);
    let mut out = vec![[0.0; 4]; layout_len()];
    out[0] = eye.extend(f32::from(subject.is_some())).into();
    out[1] = subject.unwrap_or(eye).extend(near.len() as f32).into();
    let mut seen = HashMap::default();
    for (i, (_, entity, now)) in near.into_iter().enumerate() {
        let before = last.get(&entity).copied().unwrap_or(now);
        let at = 2 + 4 * i;
        out[at] = now[0].into();
        out[at + 1] = now[1].into();
        out[at + 2] = before[0].into();
        out[at + 3] = before[1].into();
        seen.insert(entity, now);
    }
    *last = seen;
    out
}

fn write(
    parting: Res<Parting>,
    camera: Option<Single<(&GlobalTransform, Option<&CameraSubject>), With<MainCamera>>>,
    bodies: Query<(Entity, &GlobalTransform, &PartsPlants)>,
    mut last: Local<HashMap<Entity, [Vec4; 2]>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
    let Some(camera) = camera else { return };
    let (eye, subject) = *camera;
    let data = contents(
        eye.translation(),
        subject.and_then(|subject| subject.0),
        bodies
            .iter()
            .map(|(entity, at, body)| (entity, at.translation(), *body)),
        &mut last,
    );
    if let Some(mut buffer) = buffers.get_mut(&parting.0) {
        *buffer = ShaderBuffer::new(data, default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_bodies_are_written_with_where_they_stood() {
        let mut world = World::new();
        let ids: Vec<Entity> = (0..PUSHERS + 2).map(|_| world.spawn_empty().id()).collect();
        let eye = Vec3::new(0.0, 1000.0, 0.0);
        let body = PartsPlants::default();
        // One more body than fits, each a metre farther off than the last, and one beyond reach.
        let mut bodies: Vec<_> = (0..=PUSHERS)
            .map(|i| (ids[i], eye + Vec3::X * (i as f32 + 1.0), body))
            .collect();
        bodies.push((ids[PUSHERS + 1], eye + Vec3::X * (REACH_M + 1.0), body));
        let mut last = HashMap::default();
        let first = contents(eye, Some(eye + Vec3::X), bodies.iter().copied(), &mut last);
        assert_eq!(first.len(), layout_len());
        assert_eq!(first[0][3], 1.0, "has a subject");
        assert_eq!(first[1][3], PUSHERS as f32, "the farthest are left out");
        // Not seen before: it stood where it stands.
        assert_eq!(first[2], first[4]);
        assert_eq!(first[2][3], body.radius);
        // Its head is above its feet, away from the planet's middle.
        assert!((Vec4::from(first[3]) - Vec4::from(first[2])).length() > body.height - 0.01);

        // The nearest moves; a frame later it is written where it was too.
        bodies[0].1 += Vec3::Z * 0.5;
        let second = contents(eye, None, bodies.iter().copied(), &mut last);
        assert_eq!(second[0][3], 0.0, "no subject");
        assert_eq!(Vec4::from(second[4]), Vec4::from(first[2]));
        assert!(Vec4::from(second[2]).distance(Vec4::from(first[2])) > 0.49);
    }
}
