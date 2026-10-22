//! The land's horizon round the camera as a picture (D416): `terra_land::horizon`'s map, baked
//! on a worker, for the ground, the far forest and the lakes to shade what the sun is under
//! the horizon from past the shadow map's 300 m, and the lakes to light the slopes they mirror
//! off screen (`assets/shaders/horizon.wesl`). Baked round the land's middle first, then again
//! round the camera once it is a kilometre from the map's; the sun's moving needs nothing.

use crate::planet::{MainCamera, Planet};
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use std::time::Instant;
use terra_land::horizon::{HorizonMap, WAYS};

/// The camera this far from the map's middle bakes it again round the camera.
const REBAKE_M: f32 = 1000.0;

/// The horizon's picture and where it lies, as the shaders read it: the middle's direction and
/// the world's radius; east and the map's width in metres; north and whether there is a map.
#[derive(Resource, Default)]
pub struct HorizonPicture {
    pub image: Option<Handle<Image>>,
    pub middle: Vec4,
    pub east: Vec4,
    pub north: Vec4,
    baking: Option<Task<(HorizonMap, f32)>>,
}

impl HorizonPicture {
    /// The uniforms a material is told, and the picture: none until there is one, so Bevy
    /// binds its fallback of the right shape (a blank 2D picture is not an array).
    pub fn uniforms(&self) -> (Vec4, Vec4, Vec4, Option<Handle<Image>>) {
        (self.middle, self.east, self.north, self.image.clone())
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<HorizonPicture>()
        .add_systems(Update, (bake, baked).chain());
}

/// Starts a bake where there is none yet, or the camera has left the map's middle.
fn bake(
    planet: Res<Planet>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut picture: ResMut<HorizonPicture>,
) {
    let Some(round) = planet.round() else {
        return;
    };
    if picture.baking.is_some() {
        return;
    }
    let around = match (picture.image.is_some(), camera.iter().next()) {
        (false, _) => round.middle,
        (true, Some(camera)) => {
            let at = camera.translation().normalize_or_zero();
            let off = at.angle_between(picture.middle.truncate()) * planet.radius_m();
            if off < REBAKE_M {
                return;
            }
            at
        }
        (true, None) => return,
    };
    let round = round.clone();
    picture.baking = Some(AsyncComputeTaskPool::get().spawn(async move {
        let started = Instant::now();
        let map = round.horizons(around);
        (map, started.elapsed().as_secs_f32())
    }));
}

/// Takes a finished bake into the picture.
fn baked(mut picture: ResMut<HorizonPicture>, mut images: ResMut<Assets<Image>>) {
    // Only looked at: a `ResMut` touched marks the materials' maps changed.
    if picture.baking.is_none() {
        return;
    }
    let Some(task) = picture.bypass_change_detection().baking.as_mut() else {
        return;
    };
    let Some((map, took_s)) = block_on(poll_once(task)) else {
        return;
    };
    picture.baking = None;
    let n = map.cells as u32;
    let mut image = Image::new(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: (WAYS / 4) as u32,
        },
        TextureDimension::D2,
        map.bytes(),
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    let frame = map.frame;
    info!(
        "horizon map: {n}x{n} of {} m baked in {took_s:.2} s",
        map.cell_m
    );
    // The old picture goes once no material holds it.
    picture.image = Some(images.add(image));
    picture.middle = frame.middle.extend(frame.radius_m);
    picture.east = frame.east.extend(map.size_m());
    picture.north = frame.north.extend(1.0);
}
