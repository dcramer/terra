//! Plants make way (D271, D274), as shipped open worlds have them: shrubs, ferns and grass
//! lean away from whoever walks through them (foliage interaction; Ghost of Tsushima's grass
//! takes characters' displacement), and a shrub or young tree between a third-person camera
//! and the one it follows, or round the lens, fades out as a whole ([`ObstructionFade`], a
//! camera-obstruction fade as Unreal games have it), as do leaves right at the lens, so the
//! camera never looks through a bush it has backed into and the figure is not lost in one.
//! Shadows keep everything. Notes: research/foliage-near-camera.md.
//!
//! The foliage and grass shaders read one small buffer that every one of their materials
//! shares ([`Parting`]): the main camera's eye, the point it follows ([`CameraSubject`]),
//! and the bodies nearest it that part plants ([`PartsPlants`]), each where it stands this
//! frame and stood the last, so motion vectors follow a plant pushed aside; and the world's
//! wind (`crate::wind`, D255) and where the climate's maps lie (`crate::climate`, D327), as
//! games keep such globals in one buffer every plant's shader reads (Unreal's material
//! parameter collections, D374). It is written in place each frame; no material changes. A
//! changed material has every plant drawn with it specialized again, some 20 ms of the render
//! thread over two frames each time the wind turned or the maps moved on.

use crate::planet::MainCamera;
use bevy::{
    camera::{primitives::Aabb, visibility::VisibilitySystems},
    math::Affine3A,
    mesh::MeshTag,
    platform::collections::HashMap,
    prelude::*,
    render::storage::ShaderBuffer,
};

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
/// and head now, its feet and head a frame ago; the feet's w is its radius), then the wind,
/// then where the climate's maps lie.
const fn layout_len() -> usize {
    CLIMATE + 4
}

/// Where the wind is in the buffer: which way it blows, and in w how hard as a share of the
/// climate's mean (`WorldWind::strength`), or -1 until the world has one, when each
/// material's own way is kept.
const WIND: usize = 2 + 4 * PUSHERS;

/// Where the climate's maps lie (`ClimateMaps::from_world`, D327), a column at a time: it
/// moves with the camera, so it is kept here rather than in each material that reads them.
const CLIMATE: usize = WIND + 1;

/// The wind as the buffer holds it.
fn wind_entry(wind: Option<&crate::wind::WorldWind>) -> [f32; 4] {
    wind.map_or([0.0, 0.0, 0.0, -1.0], |wind| {
        wind.toward.extend(wind.strength).into()
    })
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        fade_obstructions
            .after(TransformSystems::Propagate)
            .after(VisibilitySystems::CalculateBounds),
    );
    // Without rendering (as in tests) there is no buffer to write.
    if app.world().contains_resource::<Assets<ShaderBuffer>>() {
        app.init_resource::<Parting>()
            .add_systems(PostUpdate, write.after(TransformSystems::Propagate));
    }
}

/// A plant that fades as a whole while it stands between the main camera and the one it
/// follows, or round the lens, as Unreal games fade a camera's obstructions and The Witcher 3
/// the bushes that hide Geralt: all its parts together, dithered out over a few tenths of a
/// second, rather than a hole cut through it (D274). Put on the plant's own
/// entity; its mesh, and its children's, are what fade. Shrubs and low plants, which the
/// camera passes through; not trees, whose trunks the camera's boom stops at.
#[derive(Component, Clone, Debug, Default)]
pub struct ObstructionFade {
    /// How much of it is gone, 0 to [`FADED`].
    gone: f32,
    /// Its parts' box, once their meshes are loaded.
    bounds: Option<Bounds>,
}

/// A plant's box, in its own frame, and round it in the world.
#[derive(Clone, Debug)]
struct Bounds {
    to_plant: Affine3A,
    middle: Vec3,
    half: Vec3,
    /// Its middle in the world, and how far round it the box reaches.
    centre: Vec3,
    reach: f32,
}

/// How much of a plant in the way is gone: all of it, its shadow kept. A sparse dither left
/// of it (a fifth, a twelfth) stippled half the screen when the camera stood in a hazel, and
/// TAA smeared the dots as it moved (D274).
pub const FADED: f32 = 1.0;
/// How long a plant takes to fade out, or back in, in seconds.
const FADE_S: f32 = 0.3;
/// The share of a plant's box its leaves fill, for the line to the figure to pass through:
/// a box holds a bush's corners too, which its leaves do not.
const LEAF_MASS: f32 = 0.8;

/// Whether the line from `from` to `to` passes through a box round the origin of `half`
/// size: a slab test.
fn crosses(from: Vec3, to: Vec3, half: Vec3) -> bool {
    let along = to - from;
    let (mut low, mut high) = (0.0f32, 1.0f32);
    for axis in 0..3 {
        let (start, step, size) = (from[axis], along[axis], half[axis]);
        if step.abs() < 1e-6 {
            if start.abs() > size {
                return false;
            }
            continue;
        }
        let (a, b) = ((-size - start) / step, (size - start) / step);
        low = low.max(a.min(b));
        high = high.min(a.max(b));
        if low > high {
            return false;
        }
    }
    true
}

/// How far a plant fades toward `toward` in `dt` seconds.
fn faded(gone: f32, toward: f32, dt: f32) -> f32 {
    let step = FADED / FADE_S * dt;
    gone + (toward - gone).clamp(-step, step)
}

/// The value the foliage shaders read from a mesh's tag: how much of it is gone, of 255.
fn tag_of(gone: f32) -> u32 {
    (gone.clamp(0.0, 1.0) * 255.0).round() as u32
}

fn fade_obstructions(
    mut commands: Commands,
    time: Res<Time>,
    camera: Option<Single<(&GlobalTransform, Option<&CameraSubject>), With<MainCamera>>>,
    mut plants: Query<(
        Entity,
        &mut ObstructionFade,
        Ref<GlobalTransform>,
        Option<&Children>,
    )>,
    parts: Query<(&GlobalTransform, Option<&Aabb>), With<Mesh3d>>,
    mut tags: Query<&mut MeshTag>,
) {
    let line = camera.and_then(|camera| {
        let (eye, subject) = *camera;
        subject
            .and_then(|subject| subject.0)
            .map(|subject| (eye.translation(), subject))
    });
    let dt = time.delta_secs();
    for (plant, mut fade, place, children) in &mut plants {
        let meshes = || std::iter::once(plant).chain(children.into_iter().flatten().copied());
        if fade.bounds.is_none() || place.is_changed() {
            let to_plant = place.affine().inverse();
            let mut corners = Vec::new();
            let mut loaded = true;
            for (at, aabb) in meshes().filter_map(|part| parts.get(part).ok()) {
                let Some(aabb) = aabb else {
                    loaded = false;
                    break;
                };
                let (middle, half) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
                for k in 0..8 {
                    let sign = Vec3::new(
                        if k & 1 == 0 { -1.0 } else { 1.0 },
                        if k & 2 == 0 { -1.0 } else { 1.0 },
                        if k & 4 == 0 { -1.0 } else { 1.0 },
                    );
                    let world = at.transform_point(middle + half * sign);
                    corners.push(to_plant.transform_point3(world));
                }
            }
            if !loaded || corners.is_empty() {
                continue;
            }
            let (low, high) = corners
                .iter()
                .fold((Vec3::MAX, Vec3::MIN), |(l, h), p| (l.min(*p), h.max(*p)));
            if fade.bounds.is_none() {
                for part in meshes().filter(|part| parts.contains(*part)) {
                    commands.entity(part).insert(MeshTag::new(0));
                }
            }
            let (middle, half) = ((low + high) * 0.5, (high - low) * 0.5);
            let m = place.affine().matrix3;
            let scale = m
                .x_axis
                .length()
                .max(m.y_axis.length())
                .max(m.z_axis.length());
            fade.bounds = Some(Bounds {
                to_plant,
                middle,
                half,
                centre: place.transform_point(middle),
                reach: half.length() * scale,
            });
        }
        let Some(bounds) = &fade.bounds else {
            continue;
        };
        let in_way = line.is_some_and(|(eye, subject)| {
            // Most plants are nowhere near the line: one test, before the rest.
            let near = eye
                .distance(bounds.centre)
                .min(subject.distance(bounds.centre));
            if near > eye.distance(subject) + bounds.reach {
                return false;
            }
            let from = bounds.to_plant.transform_point3(eye) - bounds.middle;
            let to = bounds.to_plant.transform_point3(subject) - bounds.middle;
            crosses(from, to, bounds.half * LEAF_MASS)
        });
        let toward = if in_way { FADED } else { 0.0 };
        if fade.gone == toward {
            continue;
        }
        fade.gone = faded(fade.gone, toward, dt);
        let tag = tag_of(fade.gone);
        for part in meshes() {
            if let Ok(mut mesh_tag) = tags.get_mut(part) {
                mesh_tag.set_if_neq(MeshTag::new(tag));
            }
        }
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
    wind: Option<Res<crate::wind::WorldWind>>,
    maps: Option<Res<crate::climate::ClimateMaps>>,
) {
    let Some(camera) = camera else { return };
    let (eye, subject) = *camera;
    let mut data = contents(
        eye.translation(),
        subject.and_then(|subject| subject.0),
        bodies
            .iter()
            .map(|(entity, at, body)| (entity, at.translation(), *body)),
        &mut last,
    );
    data[WIND] = wind_entry(wind.as_deref());
    let from_world = maps.map_or(Mat4::IDENTITY, |maps| maps.from_world);
    data[CLIMATE..CLIMATE + 4].copy_from_slice(&from_world.to_cols_array_2d());
    if let Some(mut buffer) = buffers.get_mut(&parting.0) {
        *buffer = ShaderBuffer::new(data, default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_crosses_a_box_only_where_it_passes_through_it() {
        let half = Vec3::new(1.0, 2.0, 1.0);
        // Through it, from one side to the other; from inside it; ending inside it.
        assert!(crosses(
            Vec3::new(-3.0, 0.0, 0.0),
            Vec3::new(3.0, 0.5, 0.0),
            half
        ));
        assert!(crosses(Vec3::ZERO, Vec3::new(5.0, 5.0, 5.0), half));
        assert!(crosses(
            Vec3::new(-3.0, 1.0, 0.5),
            Vec3::new(0.5, 1.0, 0.5),
            half
        ));
        // Over it, beside it, and stopping short of it.
        assert!(!crosses(
            Vec3::new(-3.0, 2.5, 0.0),
            Vec3::new(3.0, 2.5, 0.0),
            half
        ));
        assert!(!crosses(
            Vec3::new(-3.0, 0.0, 1.2),
            Vec3::new(3.0, 0.0, 1.2),
            half
        ));
        assert!(!crosses(
            Vec3::new(-3.0, 0.0, 0.0),
            Vec3::new(-1.2, 0.0, 0.0),
            half
        ));
        // Past a corner, though both ends' coordinates each lie within its span somewhere.
        assert!(!crosses(
            Vec3::new(-2.0, 0.0, 0.5),
            Vec3::new(0.5, 0.0, 3.0),
            half
        ));
    }

    /// A camera following a point, and a bush, a box of mesh 2 m across round its middle
    /// a metre up, standing at `bush`, its parts drawn by it and a child.
    fn bush_scene(bush: Vec3) -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, fade_obstructions);
        app.world_mut().spawn((
            MainCamera,
            GlobalTransform::from_translation(Vec3::new(0.0, 1.8, 5.0)),
            CameraSubject(Some(Vec3::new(0.0, 1.2, 0.0))),
        ));
        let aabb = Aabb::from_min_max(Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 2.0, 1.0));
        let at = GlobalTransform::from_translation(bush);
        let plant = app
            .world_mut()
            .spawn((ObstructionFade::default(), at, Mesh3d::default(), aabb))
            .id();
        let part = app
            .world_mut()
            .spawn((at, Mesh3d::default(), aabb, ChildOf(plant)))
            .id();
        (app, plant, part)
    }

    /// Runs `frames` frames of a sixtieth of a second each.
    fn run(app: &mut App, frames: usize) {
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(1.0 / 60.0),
        ));
        for _ in 0..frames {
            app.update();
        }
    }

    fn tag(app: &App, entity: Entity) -> u32 {
        app.world()
            .get::<MeshTag>(entity)
            .map_or(0, |tag| tag.value)
    }

    #[test]
    fn a_bush_between_the_camera_and_the_figure_fades_as_a_whole_over_time_and_back() {
        // Halfway along the line from the camera to the figure.
        let (mut app, plant, part) = bush_scene(Vec3::new(0.3, 0.0, 2.5));
        run(&mut app, 3);
        let early = tag(&app, plant);
        assert!(early > 0, "it starts to fade");
        assert!(early < tag_of(FADED) / 2, "not at once: {early}");
        run(&mut app, 30);
        assert_eq!(tag(&app, plant), tag_of(FADED));
        assert_eq!(tag(&app, part), tag(&app, plant), "all its parts together");
        // The figure steps out from behind it: it comes back, as gradually.
        app.world_mut()
            .query::<&mut CameraSubject>()
            .single_mut(app.world_mut())
            .unwrap()
            .0 = Some(Vec3::new(4.0, 1.2, 0.0));
        run(&mut app, 3);
        let back = tag(&app, plant);
        assert!(back > 0 && back < tag_of(FADED), "{back}");
        run(&mut app, 30);
        assert_eq!(tag(&app, plant), 0);
    }

    #[test]
    fn a_bush_beside_the_line_stays() {
        let (mut app, plant, _) = bush_scene(Vec3::new(2.5, 0.0, 2.5));
        run(&mut app, 30);
        assert_eq!(tag(&app, plant), 0);
    }

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

    #[test]
    fn the_wind_is_written_after_the_bodies_or_marked_absent() {
        // The bodies' last entry, the nearest's head a frame ago, comes before the wind.
        assert_eq!(WIND, 2 + 4 * (PUSHERS - 1) + 3 + 1);
        assert_eq!(CLIMATE, WIND + 1);
        assert_eq!(layout_len(), CLIMATE + 4);
        assert!(
            wind_entry(None)[3] < 0.0,
            "no wind yet: the materials' own way"
        );
        let wind = crate::wind::WorldWind {
            toward: Vec3::Z,
            strength: 1.7,
            speed_m_s: 9.0,
        };
        assert_eq!(wind_entry(Some(&wind)), [0.0, 0.0, 1.0, 1.7]);
    }
}
