//! The surface hidden while the eye is in the dungeon. The stair's top is the dungeon's one
//! portal to the outside (`terra_dungeon::stamp::sight`): while a camera's eye is under the
//! ground and no line through the cubes' air from it reaches an opening point in its view, it
//! draws nothing of the surface, as engines cull an exterior from an interior (Godot's rooms
//! and portals, Source's areaportals). The forest is 6,067 meshes and 21 M triangles a frame
//! drawn through the ground, and the crypt is CPU-bound (perf round 1).
//!
//! The switch is the camera's, not the forest's: every mesh clear of the dungeon goes on a
//! surface layer once, as it is spawned or moves, and a camera inside drops that layer, so
//! hiding the forest touches a few entities, not thousands. The sun drops it too (its cascades
//! would draw the forest's shadows into nothing), and the sun-only stand-ins plants cast from
//! go on a surface sun layer for the same. Point and spot lights keep the surface layer, so a
//! campfire still shadows the trees round it. What is near the dungeon (its column of ground,
//! what stands over it, whoever walks in it) stays on the world's layer.

use bevy::camera::primitives::{Aabb, Frustum, Sphere};
use bevy::camera::visibility::{RenderLayers, VisibilitySystems};
use bevy::prelude::*;
use terra_dungeon::stamp::Stamp;
use terra_dungeon::stamp::sight::Sight;

/// The surface's meshes, which a camera in the dungeon does not draw.
pub const SURFACE: usize = 5;
/// The surface's sun-only stand-ins (`terra_world::grown::SUN_ONLY`), which only the sun draws
/// and only while the surface is drawn.
pub const SURFACE_SUN: usize = 6;
const WORLD: usize = 0;
const SUN_ONLY: usize = terra_world::grown::SUN_ONLY;
/// A mesh is kept on the world's layer if it comes within this of a dig, or of the column of
/// ground over it so high: the ground over the dungeon is its roof and shades its stair.
const NEAR_M: f32 = 1.0;
const ROOF_M: f32 = 4.0;
/// An opening point is in a camera's view if a sphere so wide round it is.
const POINT_M: f32 = 0.5;

/// The dungeon's sight and where it lies.
#[derive(Resource)]
pub struct Interior {
    sight: Sight,
    /// World from the dungeon's frame, and back.
    frame: Transform,
    to_frame: bevy::math::Affine3A,
    /// The digs' boxes in the frame, grown by `NEAR_M` and up by `ROOF_M`.
    near: Vec<(Vec3, Vec3)>,
    /// Whether any camera draws the surface now.
    shown: bool,
    /// The ground's level in the frame: the stair's top (`Stamp::start`).
    ground: f32,
}

impl Interior {
    pub fn new(stamp: &Stamp, frame: Transform) -> Interior {
        let near = stamp
            .digs
            .iter()
            .map(|d| {
                let (lo, hi) = (d.centre - d.half, d.centre + d.half);
                (
                    lo - Vec3::splat(NEAR_M),
                    hi + Vec3::new(NEAR_M, NEAR_M + ROOF_M, NEAR_M),
                )
            })
            .collect();
        Interior {
            sight: Sight::new(stamp),
            frame,
            to_frame: frame.compute_affine().inverse(),
            near,
            shown: true,
            ground: stamp.start.y,
        }
    }

    /// Whether an eye in the world is under the ground in the dungeon's dug space: down its
    /// stair as well as in its rooms, whose boxes alone said indoors (`terra_bases::light`),
    /// so the eye held to the outdoor band all the way down (mine round 11).
    pub(super) fn underground(&self, eye: Vec3) -> bool {
        let p = self.to_frame.transform_point3(eye);
        p.y < self.ground && self.sight.inside(p)
    }

    /// Whether a sphere in the world comes near the dungeon.
    fn near(&self, centre: Vec3, radius: f32) -> bool {
        let p = self.to_frame.transform_point3(centre);
        self.near
            .iter()
            .any(|(lo, hi)| p.clamp(*lo, *hi).distance_squared(p) <= radius * radius)
    }

    /// Whether a camera at `eye` with `frustum` may see the surface.
    fn sees_out(&self, eye: Vec3, frustum: &Frustum) -> bool {
        self.sight
            .sees_out(self.to_frame.transform_point3(eye), |p| {
                let sphere = Sphere {
                    center: self.frame.transform_point(p).into(),
                    radius: POINT_M,
                };
                frustum.intersects_sphere(&sphere, true)
            })
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        (layer_meshes, layer_lights, switch, underground)
            .run_if(resource_exists::<Interior>)
            .after(VisibilitySystems::CalculateBounds)
            .after(VisibilitySystems::UpdateFrusta)
            .before(VisibilitySystems::CheckVisibility),
    );
}

/// A mesh's layers as this module reads them: the world's or the sun-only stand-ins', each on
/// the surface or not; any other (an icon's, the upscaler's) is left alone.
fn layers_of(layers: Option<&RenderLayers>) -> Option<(bool, bool)> {
    let Some(layers) = layers else {
        return Some((false, false));
    };
    [
        (WORLD, false, false),
        (SURFACE, false, true),
        (SUN_ONLY, true, false),
        (SURFACE_SUN, true, true),
    ]
    .into_iter()
    .find(|&(layer, _, _)| *layers == RenderLayers::layer(layer))
    .map(|(_, sun, surface)| (sun, surface))
}

/// Each mesh as it is spawned or moves: on the surface's layer when clear of the dungeon.
fn layer_meshes(
    mut commands: Commands,
    interior: Res<Interior>,
    meshes: Query<
        (Entity, &Aabb, &GlobalTransform, Option<&RenderLayers>),
        (
            With<Mesh3d>,
            Or<(
                Changed<GlobalTransform>,
                Changed<Aabb>,
                Changed<RenderLayers>,
            )>,
        ),
    >,
) {
    for (entity, aabb, at, layers) in &meshes {
        let Some((sun, surface)) = layers_of(layers) else {
            continue;
        };
        let centre = at.transform_point(aabb.center.into());
        let radius = (at.affine().matrix3 * aabb.half_extents).length();
        let away = !interior.near(centre, radius);
        if away != surface {
            let layer = match (sun, away) {
                (false, false) => WORLD,
                (false, true) => SURFACE,
                (true, false) => SUN_ONLY,
                (true, true) => SURFACE_SUN,
            };
            commands.entity(entity).insert(RenderLayers::layer(layer));
        }
    }
}

/// Point and spot lights cast the surface's shadows as well as the world's.
fn layer_lights(
    mut commands: Commands,
    lights: Query<
        (Entity, Option<&RenderLayers>),
        (
            Or<(With<PointLight>, With<SpotLight>)>,
            Or<(Added<PointLight>, Added<SpotLight>, Changed<RenderLayers>)>,
        ),
    >,
) {
    for (entity, layers) in &lights {
        let layers = layers.cloned().unwrap_or_default();
        if layers.intersects(&RenderLayers::layer(WORLD))
            && !layers.intersects(&RenderLayers::layer(SURFACE))
        {
            commands.entity(entity).insert(layers.with(SURFACE));
        }
    }
}

/// Tells the scene whether the eye is under the ground in the dungeon, so it opens there as
/// in a room (`terra_world::scene::Underground`).
fn underground(
    interior: Res<Interior>,
    camera: Query<&GlobalTransform, With<terra_world::planet::MainCamera>>,
    under: Option<ResMut<terra_world::scene::Underground>>,
) {
    let Some(mut under) = under else {
        return;
    };
    let Some(eye) = camera.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    under.set_if_neq(terra_world::scene::Underground(interior.underground(eye)));
}

/// Each camera of the world drops the surface while its eye is in the dungeon and sees no
/// opening; the sun draws the surface while any camera does.
fn switch(
    mut commands: Commands,
    mut interior: ResMut<Interior>,
    cameras: Query<(Entity, &GlobalTransform, &Frustum, Option<&RenderLayers>), With<Camera3d>>,
    suns: Query<(Entity, Option<&RenderLayers>), With<DirectionalLight>>,
) {
    let mut shown = false;
    for (entity, at, frustum, layers) in &cameras {
        let layers = layers.cloned().unwrap_or_default();
        if !layers.intersects(&RenderLayers::layer(WORLD)) {
            continue;
        }
        let sees = interior.sees_out(at.translation(), frustum);
        shown |= sees;
        let want = if sees {
            layers.clone().with(SURFACE)
        } else {
            layers.clone().without(SURFACE)
        };
        if want != layers {
            commands.entity(entity).insert(want);
        }
    }
    if shown != interior.shown {
        info!(
            "Dungeon: the surface {}",
            if shown { "shown" } else { "hidden" }
        );
        interior.shown = shown;
    }
    for (entity, layers) in &suns {
        let layers = layers.cloned().unwrap_or_default();
        let want = if shown {
            layers.clone().with(SURFACE).with(SURFACE_SUN)
        } else {
            layers.clone().without(SURFACE).without(SURFACE_SUN)
        };
        if want != layers {
            commands.entity(entity).insert(want);
        }
    }
}
