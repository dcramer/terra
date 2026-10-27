//! Moves the sky's clouds to the world's clock (`clouds`, docs/sky.md stage 1): their shapes by
//! the wind aloft, every frame, and their weather map a step of the clock at a time, built ahead
//! on another thread so a step's map is ready when the clock reaches it.

use super::clouds::{self, CloudWinds};
use crate::climate::ClimateFields;
use crate::daylight::Clock;
use crate::planet::MainCamera;
use crate::start::Start;
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use terra_climate::WorldTime;

/// The clouds' place at the clock: what the sky's material is told.
#[derive(Resource, Clone, PartialEq)]
pub(super) struct Clouds {
    pub offsets: [Vec4; 12],
    /// From the world to metres east, up and south of the weather map's middle.
    pub to_map: Mat4,
    /// The weather map's middle, metres east and south of the start: the offsets' anchor.
    pub middle: Vec2,
    /// The maps of this step of the clock and the next, and how far through the step it is.
    pub maps: Option<[Handle<Image>; 2]>,
    pub blend: f32,
}

impl Default for Clouds {
    fn default() -> Self {
        Self {
            offsets: clouds::offsets(&CloudWinds::default(), 0.0, Vec2::ZERO),
            to_map: Mat4::IDENTITY,
            middle: Vec2::ZERO,
            maps: None,
            blend: 0.0,
        }
    }
}

/// A weather map: its middle, metres east and south of the start, and its step of the clock.
type MapKey = (IVec2, i64);

/// The weather maps built, the one being built on another thread, and what the images hold.
#[derive(Resource, Default)]
pub(super) struct Mapping {
    images: Option<[Handle<Image>; 2]>,
    shown: [Option<MapKey>; 2],
    built: Vec<(MapKey, Vec<u8>)>,
    task: Option<(MapKey, Task<Vec<u8>>)>,
}

impl Mapping {
    fn has(&self, key: MapKey) -> bool {
        self.built.iter().any(|(k, _)| *k == key)
    }

    /// Builds a map on another thread, a row after another, so it takes one core.
    fn spawn(&mut self, key: MapKey, fields: &ClimateFields, start: &Start) {
        let (fields, start) = (fields.clone(), *start);
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let (middle, time) = (key.0.as_vec2(), clouds::step_time(key.1));
            (0..clouds::MAP_TEXELS)
                .flat_map(|row| clouds::map_row(&fields, &start, middle, time, row))
                .collect()
        });
        self.task = Some((key, task));
    }

    /// The map for a key, waited for if it is being built and built here, on every core, if it
    /// is not: at the start, and when the clock jumps.
    fn ensure(&mut self, key: MapKey, fields: &ClimateFields, start: &Start) {
        if self.has(key) {
            return;
        }
        let texels = match self.task.take() {
            Some((k, task)) if k == key => block_on(task),
            other => {
                self.task = other;
                clouds::build_map(fields, start, key.0.as_vec2(), clouds::step_time(key.1))
            }
        };
        self.built.push((key, texels));
    }
}

fn map_image() -> Image {
    let mut image = Image::new_fill(
        Extent3d {
            width: clouds::MAP_TEXELS,
            height: clouds::MAP_TEXELS,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        // Clear, heaped, dry.
        &[0, 0, 255, 0],
        // Amounts, not colours.
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    // Mirrored past its edges: the sky reaches the air's horizon, some 140 km off, and the map
    // 32 km, so clamped its edge texels would run on to the horizon in stripes.
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::MirrorRepeat,
        address_mode_v: ImageAddressMode::MirrorRepeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

/// Moves the clouds to the clock. Without a climate they stand still and the sky draws its own
/// cloudiness everywhere.
pub(super) fn move_clouds(
    (clock, fields, start): (
        Option<Res<Clock>>,
        Option<Res<ClimateFields>>,
        Option<Res<Start>>,
    ),
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    mut clouds: ResMut<Clouds>,
    mut mapping: ResMut<Mapping>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(fields), Some(start)) = (fields, start) else {
        return;
    };
    let time = clock.map_or(WorldTime(0.0), |clock| clock.time());
    let eye = start.local(camera.map_or(start.translation, |c| c.translation()));
    let wanted = clouds::middle_for(Vec2::new(eye.x, eye.z)).as_ivec2();
    let (step, blend) = clouds::step_of(time);
    if let Some((key, task)) = &mut mapping.task
        && let Some(texels) = block_on(poll_once(task))
    {
        let key = *key;
        mapping.task = None;
        mapping.built.push((key, texels));
    }
    // The eye has moved on to another middle within a step whose maps are drawn: they reach
    // well past the layer's horizon, so they are drawn on until the new middle's are built on
    // another thread.
    let middle = match mapping.shown {
        [Some((m, s)), Some((_, n))]
            if m != wanted
                && s == step
                && n == step + 1
                && !(mapping.has((wanted, step)) && mapping.has((wanted, step + 1))) =>
        {
            if mapping.task.is_none() {
                let key = if mapping.has((wanted, step)) {
                    (wanted, step + 1)
                } else {
                    (wanted, step)
                };
                mapping.spawn(key, &fields, &start);
            }
            m
        }
        _ => wanted,
    };
    mapping.ensure((middle, step), &fields, &start);
    mapping.ensure((middle, step + 1), &fields, &start);
    if mapping.task.is_none() && !mapping.has((middle, step + 2)) {
        mapping.spawn((middle, step + 2), &fields, &start);
    }
    mapping
        .built
        .retain(|((m, s), _)| *s >= step && (*m == middle || *m == wanted));
    let handles = mapping
        .images
        .get_or_insert_with(|| [images.add(map_image()), images.add(map_image())])
        .clone();
    for (slot, key) in [(middle, step), (middle, step + 1)].into_iter().enumerate() {
        if mapping.shown[slot] == Some(key) {
            continue;
        }
        let texels = mapping
            .built
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, texels)| texels.clone());
        if let (Some(texels), Some(mut image)) = (texels, images.get_mut(&handles[slot])) {
            image.data = Some(texels);
            mapping.shown[slot] = Some(key);
        }
    }
    let to_middle = Mat4::from_translation(-Vec3::new(middle.x as f32, 0.0, middle.y as f32));
    // Set only when it has changed, so a still clock leaves the material alone.
    clouds.set_if_neq(Clouds {
        offsets: clouds::offsets(
            &CloudWinds::of_world(&fields, &start),
            clouds::seconds(time),
            middle.as_vec2(),
        ),
        to_map: to_middle * Mat4::from(start.compute_affine().inverse()),
        middle: middle.as_vec2(),
        maps: Some(handles),
        blend,
    });
}
