//! Weathering one chosen building, the spike's renderer side (research/2026-10-25-wfc-weathering.md):
//! `assets/data/weathering.ron` names a build by its name and how to weather it (none, the
//! masks with noise motifs, or the masks with motifs solved by wave function collapse, at an
//! age); once it is laid, `terra_weathering` makes its per-face maps and the building
//! material (`shaders/build.wesl`) blends them over every surface inside that build's box.
//! One build at a time: the maps and the build's place ride on the one shared material.

use crate::Built;
use crate::arrays::{Arrays, BuildMaterial};
use bevy::asset::RenderAssetUsages;
use bevy::diagnostic::FrameCount;
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
use terra_build::kit::Pattern;
use terra_build::{CELL_M, Fill};
use terra_weathering::{Mode, Settings, Stuff};

/// The data file: which build, and how.
const FILE: &str = "weathering.ron";
/// Frames to wait before weathering, so the build is laid and placed.
const AFTER_FRAMES: u32 = 20;

#[derive(serde::Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Way {
    /// Not weathered: the standard textures alone.
    #[default]
    Off,
    /// The masks, their motif layers cut by noise.
    Masks,
    /// The masks, their motif layers solved by wave function collapse.
    Wfc,
}

/// What `weathering.ron` says.
#[derive(serde::Deserialize, Clone, Debug, Default)]
pub struct WeatheringFile {
    pub way: Way,
    /// Years the building has stood.
    pub age: f32,
    pub seed: u32,
    /// A part of the build's name, as `Village: masonry and timber: L, gable`.
    pub build: String,
}

#[derive(Resource, Default)]
struct Weathering {
    file: WeatheringFile,
    done: bool,
}

/// The material's placeholders: nothing weathered.
pub fn placeholders(images: &mut Assets<Image>) -> (Handle<Image>, Handle<Image>) {
    let lookup = Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0; 4],
        TextureFormat::R32Uint,
        RenderAssetUsages::RENDER_WORLD,
    );
    (images.add(lookup), images.add(atlas(1, vec![0; 4])))
}

/// An RGBA8 array, `side` square, a layer for each `side * side * 4` bytes of `data`.
fn atlas(side: u32, data: Vec<u8>) -> Image {
    let layers = (data.len() / (side as usize * side as usize * 4)) as u32;
    let mut image = Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: layers,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    image
}

fn read(mut commands: Commands) {
    let file = if terra_world::data::path(FILE).exists() {
        terra_world::data::read::<WeatheringFile>(FILE)
    } else {
        WeatheringFile::default()
    };
    commands.insert_resource(Weathering { file, done: false });
}

#[allow(clippy::too_many_arguments)]
fn weather(
    mut state: ResMut<Weathering>,
    frames: Res<FrameCount>,
    builds: Query<(&Name, &Built)>,
    roots: Query<&GlobalTransform>,
    arrays: Option<Res<Arrays>>,
    mut materials: ResMut<Assets<BuildMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if state.done || frames.0 < AFTER_FRAMES {
        return;
    }
    let Some(arrays) = arrays else { return };
    let file = state.file.clone();
    let mode = match file.way {
        Way::Off => {
            state.done = true;
            return;
        }
        Way::Masks => Mode::Masks,
        Way::Wfc => Mode::Wfc,
    };
    let Some((name, built)) = builds
        .iter()
        .find(|(name, _)| name.as_str().contains(&file.build))
    else {
        return;
    };
    let Ok(root) = roots.get(built.root) else {
        return;
    };
    let base = &built.base;
    let Some(kits) = built.kits.as_ref() else {
        return;
    };
    let cubes: Vec<IVec3> = base.cubes().map(|(at, _)| at).collect();
    let (Some(low), Some(high)) = (
        cubes.iter().copied().reduce(IVec3::min),
        cubes.iter().copied().reduce(IVec3::max),
    ) else {
        return;
    };
    state.done = true;
    let stuff = |at: IVec3| match base.cube(at) {
        None => Stuff::Air,
        Some(cube) if cube.fill == Fill::Open => Stuff::Air,
        Some(cube) if cube.kind.is_roof() => Stuff::Roof,
        Some(cube) => match kits.of(cube.kind).map(|kit| kit.pattern) {
            Some(Pattern::Masonry) => Stuff::Stone,
            Some(Pattern::Roof) => Stuff::Roof,
            _ => Stuff::Timber,
        },
    };
    let settings = Settings {
        mode,
        age: file.age,
        seed: file.seed,
        tries: 10,
    };
    let start = std::time::Instant::now();
    let weathered = terra_weathering::weather(&stuff, low, high, &settings);
    let took = start.elapsed().as_secs_f32() * 1000.0;
    let s = weathered.stats;
    info!(
        "Weathering: {name} ({:?}, {} years): {} faces in {} planes, {:.1} ms; {}/{} solves done, \
         {} at the first try, {} tries, {} cells; {} KB",
        file.way,
        file.age,
        s.faces,
        s.planes,
        took,
        s.solved,
        s.solves,
        s.first_try,
        s.tries,
        s.cells,
        weathered.bytes() / 1024
    );
    let c = weathered.compression();
    let way = |w: terra_weathering::compress::Way| {
        format!(
            "{} KB, rms {:.2}, max {:.0}, worst {} {:.2}",
            w.bytes / 1024,
            w.rms,
            w.max,
            w.worst,
            w.worst_rms
        )
    };
    info!(
        "Weathering, stored smaller: raw {} KB, {} blank faces; bc4 {}; half {}; half+bc4 {}; \
         mixed {}",
        c.raw / 1024,
        c.blank,
        way(c.bc4),
        way(c.half),
        way(c.half_bc4),
        way(c.mixed)
    );
    let dims = weathered.dims;
    let lookup = Image::new(
        Extent3d {
            width: (dims.x * 6) as u32,
            height: (dims.y * dims.z) as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        weathered
            .lookup
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect(),
        TextureFormat::R32Uint,
        RenderAssetUsages::RENDER_WORLD,
    );
    let side = weathered.side() as u32;
    let atlas = atlas(side, weathered.atlas.concat());
    let to_local = root.affine().inverse();
    let (m, t) = (to_local.matrix3, to_local.translation);
    // Rows of the world-to-build affine, in cubes rather than metres.
    let row = |i: usize| Vec4::new(m.x_axis[i], m.y_axis[i], m.z_axis[i], t[i]) / CELL_M;
    let Some(mut material) = materials.get_mut(&arrays.material) else {
        return;
    };
    let w = &mut material.extension;
    w.weather = [
        row(0),
        row(1),
        row(2),
        weathered.low.as_vec3().extend(1.0),
        dims.as_vec3().extend(weathered.per_row as f32),
        Vec4::new(mode as u8 as f32, file.age, 0.0, 0.0),
    ];
    w.weather_lookup = images.add(lookup);
    w.weather_atlas = images.add(atlas);
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, read)
        .add_systems(Update, weather.run_if(resource_exists::<Weathering>));
}
