//! The ground under the cover: a picture of what grows round the camera, which the ground
//! shader draws from (`planet::GroundCoverMap`), as Horizon Zero Dawn's placement paints its
//! terrain. Where grass gives out under the trees the ground is leaf litter, and round
//! their feet moss; the grass's edge and its fade with distance land on ground of its own
//! colour, so neither shows.
//!
//! It is two pictures 128 m across round the camera, half a metre a texel: the first grass,
//! how lush, leaf litter and moss, the second needles, duff and roots, and how much canopy
//! stands over. They are painted on another thread from the same
//! [`CoverSource`](super::CoverSource) the blades ask, and painted again when the camera has
//! moved a quarter of the way to their edge.

use super::CoverRules;
use crate::planet::{GroundCoverMap, MainCamera, Planet};
use crate::start::Start;
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};

/// Texels to a side, and how wide each is.
const TEXELS: u32 = 256;
const TEXEL_M: f32 = 0.5;
/// The map is painted again once the camera is this far from its middle.
const REPAINT_M: f32 = 32.0;

#[derive(Resource)]
struct Floor {
    image: Handle<Image>,
    more: Handle<Image>,
    /// The middle of the map now drawn, east and south of the start.
    middle: Option<Vec2>,
    painting: Option<(Vec2, Task<[Vec<u8>; 2]>)>,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, paint.run_if(resource_exists::<CoverRules>));
}

/// Where a map with this middle has its corner, east and south of the start.
fn corner(middle: Vec2) -> Vec2 {
    middle - Vec2::splat(TEXELS as f32 * TEXEL_M / 2.0)
}

/// From a point in the world to its place on a map with this middle: x and y from 0 to 1.
fn from_world(start: &Start, middle: Vec2) -> Mat4 {
    let size = TEXELS as f32 * TEXEL_M;
    let corner = corner(middle);
    let to_map = Mat4::from_cols(
        Vec4::new(1.0 / size, 0.0, 0.0, 0.0),
        Vec4::ZERO,
        Vec4::new(0.0, 1.0 / size, 0.0, 0.0),
        Vec4::new(-corner.x / size, -corner.y / size, 0.0, 1.0),
    );
    to_map * Mat4::from(start.compute_affine().inverse())
}

fn paint(
    mut commands: Commands,
    rules: Res<CoverRules>,
    floor: Option<ResMut<Floor>>,
    planet: Res<Planet>,
    start: Res<Start>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut images: ResMut<Assets<Image>>,
    mut map: ResMut<GroundCoverMap>,
    mut changed: Local<bool>,
) {
    let Some(camera) = camera.iter().next() else {
        return;
    };
    let Some(mut floor) = floor else {
        let mut picture = |fill: [u8; 4]| {
            let mut image = Image::new_fill(
                Extent3d {
                    width: TEXELS,
                    height: TEXELS,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &fill,
                // Amounts, not colours.
                TextureFormat::Rgba8Unorm,
                RenderAssetUsages::default(),
            );
            image.sampler = ImageSampler::linear();
            images.add(image)
        };
        commands.insert_resource(Floor {
            image: picture([255, 128, 0, 0]),
            more: picture([0, 0, 0, 0]),
            middle: None,
            painting: None,
        });
        return;
    };
    let local = start.local(camera.translation());
    let here = Vec2::new(local.x, local.z);

    if let Some((middle, task)) = &mut floor.painting
        && let Some(pixels) = block_on(poll_once(task))
    {
        let middle = *middle;
        floor.painting = None;
        floor.middle = Some(middle);
        let [first, second] = pixels;
        if let Some(mut image) = images.get_mut(&floor.image) {
            image.data = Some(first);
        }
        if let Some(mut image) = images.get_mut(&floor.more) {
            image.data = Some(second);
        }
        *map = GroundCoverMap {
            image: Some(floor.image.clone()),
            more: Some(floor.more.clone()),
            from_world: from_world(&start, middle),
        };
    }
    // Painted again when the rules change, as when a town is laid round the eye, not only
    // once the eye has gone far enough off.
    if rules.is_changed() {
        *changed = true;
    }
    let stale = *changed
        || floor
            .middle
            .is_none_or(|middle| middle.distance(here) > REPAINT_M);
    if floor.painting.is_none() && stale {
        // On a grid, so the same place is painted the same each time.
        let middle = (here / 8.0).round() * 8.0;
        let (source, planet, start) = (rules.source.clone(), planet.clone(), *start);
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let corner = corner(middle);
            let mut pixels = Vec::with_capacity((TEXELS * TEXELS * 4) as usize);
            let mut more = Vec::with_capacity((TEXELS * TEXELS * 4) as usize);
            for b in 0..TEXELS {
                for a in 0..TEXELS {
                    let at = corner + (Vec2::new(a as f32, b as f32) + 0.5) * TEXEL_M;
                    let growing = source.growing(&planet, start.at(at.x, at.y), at);
                    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                    pixels.extend([
                        byte(growing.grass),
                        byte(growing.lush),
                        byte(growing.litter),
                        byte(growing.moss),
                    ]);
                    more.extend([
                        byte(growing.needles),
                        byte(growing.roots),
                        byte(growing.shade),
                        byte(growing.trodden),
                    ]);
                }
            }
            [pixels, more]
        });
        floor.painting = Some((middle, task));
        *changed = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_maps_to_where_it_is_painted() {
        let start = Start(Transform::from_xyz(3.0, 100.0, -2.0).looking_to(Vec3::X, Vec3::Y));
        let middle = Vec2::new(40.0, -24.0);
        let to_map = from_world(&start, middle);
        let at = |east: f32, south: f32| to_map.transform_point3(start.at(east, south)).truncate();
        assert!(
            at(40.0, -24.0).distance(Vec2::splat(0.5)) < 1e-4,
            "the middle is the middle"
        );
        let size = TEXELS as f32 * TEXEL_M;
        let corner = corner(middle);
        assert!(
            at(corner.x, corner.y).length() < 1e-4,
            "the corner is the corner"
        );
        assert!(
            at(corner.x + size, corner.y).distance(Vec2::X) < 1e-4,
            "east is across"
        );
        assert!(
            at(corner.x, corner.y + size).distance(Vec2::Y) < 1e-4,
            "south is down"
        );
    }
}
