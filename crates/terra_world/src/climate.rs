//! The climate where things are (docs/climate.md, D312 to D315): what the air, the weather and
//! the days before have left at a place, for the things that care and for the shaders.
//!
//! Nothing here asks what season it is. It asks `terra_climate` for the quantities at a place
//! and a moment, from the world's climate file, its seed and its clock (`daylight::Clock`):
//!
//! - **Samples.** A thing that cares carries a [`ClimateSample`], kept up to date by distance
//!   from the nearest player: every half second near one, every few seconds farther off,
//!   rarely beyond, so the cost follows what is near players, not the world's size (D127).
//!   Local things bend it, as components the layers above give: a fire's [`Heat`], a
//!   shelter's walls and roof ([`Sheltered`]), and the canopy's shade, from the sky occlusion
//!   volume (D270). What the sample says is the same on every machine (D312), but for the
//!   canopy, which is each machine's own drawing of the trees.
//! - **Maps.** Small pictures round the camera ([`ClimateMaps`]), as the cover map is painted
//!   round it: what lies on the ground (snow, wetness, rain now, frost), and each kind of
//!   plant's leaves (`assets/data/plant_years.ron`), how many it has and how far they have
//!   turned, early and late in its kind's year, for each plant to take its own place between
//!   by its seed. They are painted on another thread, again when the clock or the camera has
//!   moved on, and the shaders read them.
//!
//! The past at a place is kept per 32 m cell (`terra_climate::PastCache`), taken at the
//! cell's middle so every machine asks the same; it goes from one day's start to the next
//! through the day, so snow builds and melts by the hour rather than at midnight.

use crate::daylight::{Clock, LATITUDE, WorldClimate};
use crate::lod::Viewpoint;
use crate::planet::{MainCamera, Planet};
use crate::sky_occlusion::SkySeen;
use crate::start::Start;
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use std::sync::Arc;
use std::time::Instant;
use terra_climate::{Cell, Fields, LeafYear, Past, PastCache, Place, WorldTime};
use terra_land::frame::Frame;

/// The kinds' years, a layer of the leaves map each, in this order.
pub const YEARS_FILE: &str = "plant_years.ron";

/// The world's climate fields, from its climate file and seed, and where its places are.
#[derive(Resource, Clone)]
pub struct ClimateFields {
    pub fields: Arc<Fields>,
    /// The kinds' years, as `plant_years.ron` lists them.
    pub years: Arc<Vec<LeafYear>>,
    sea_radius_m: f32,
    /// The world's frame: where each place's latitude comes from.
    pub frame: Frame,
}

/// The world's frame, turned as the sky is over the planet's own start (`daylight`).
pub fn world_frame(planet: &Planet, climate: &terra_climate::Climate) -> Frame {
    let year = climate.year;
    let declination = terra_climate::declination(year.tilt_deg.to_radians(), year.phase(0));
    Frame::turned_for(planet.start(), LATITUDE, declination).0
}

impl ClimateFields {
    /// A point in the world as the climate takes it, on level ground: how high over the sea,
    /// and its own latitude, from the world's frame. On the small planet that is the start's,
    /// near enough, everywhere; on the 30 km world a degree is 524 m walked north.
    pub fn place(&self, at: Vec3) -> Place {
        self.place_facing(at, at)
    }

    /// A point as the climate takes it, on ground facing `normal`, which leans it toward the
    /// sun or away (D323).
    pub fn place_facing(&self, at: Vec3, normal: Vec3) -> Place {
        Place {
            at,
            height_m: at.length() - self.sea_radius_m,
            latitude: self.frame.latitude(at),
            sunward: self.frame.sunward(at, normal),
        }
    }

    /// The place a cell's past is taken at: its middle.
    pub fn cell_place(&self, cell: Cell) -> Place {
        self.place(cell.middle())
    }

    /// The past at a point at a moment, between the start of its day and the next's, from a
    /// cache of the cells asked about.
    pub fn past_at(&self, cache: &mut PastCache, at: Vec3, time: WorldTime) -> Past {
        let cell = Cell::containing(at);
        let place = self.cell_place(cell);
        let day = time.day();
        let today = cache.past(&self.fields, cell, place, day);
        let next = terra_climate::step(&self.fields, place, today, day);
        today.between(&next, (time.hour() / terra_climate::HOURS) as f32)
    }

    /// A kind's layer in the leaves map.
    pub fn layer(&self, kind: &str) -> Option<usize> {
        self.years.iter().position(|year| year.kind == kind)
    }
}

/// What the climate is where something is, as last taken: the air, what falls, what lies on
/// the ground, bent by what is near it. Add one to anything that cares; it is kept up to
/// date by distance from the nearest player.
#[derive(Component, Reflect, Clone, Copy, Debug, Default, PartialEq)]
#[reflect(Component)]
pub struct ClimateSample {
    /// The air's temperature, in degrees Celsius.
    pub air_c: f32,
    /// The wind, in metres a second, less what shelters it.
    pub wind_m_s: f32,
    /// The sun's light on level ground, after what shades it, in watts a square metre.
    pub sun_w_m2: f32,
    /// Rain and snow falling on it, in millimetres of water an hour.
    pub rain_mm_h: f32,
    pub snow_mm_h: f32,
    /// The air's relative humidity, 0 to 1.
    pub humidity: f32,
    /// How wet the ground is, 0 dry to 1 as wet as it holds.
    pub ground_wetness: f32,
    /// How deep the snow lies, in metres.
    pub snow_depth_m: f32,
    /// How thick ice on still water is, in metres.
    pub ice_m: f32,
    /// Fires' heat on the side facing them, in watts a square metre.
    pub fire_w_m2: f32,
    /// How much of the sky is open over it, 0 under a closed canopy to 1 in the open.
    pub open: f32,
    /// The clock's moment it was taken at, in days.
    pub taken: f64,
    /// When it is next due, in seconds of play.
    #[reflect(ignore)]
    due_s: f64,
}

/// A fire's heat: how much it gives off as radiant heat, in watts. Gameplay gives it to a
/// burning fire.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Heat {
    pub watts: f32,
}

/// How sheltered something is by walls and a roof, 0 in the open to 1 in a closed room:
/// out of the wind, the rain and the sun, and a fire's warmth held in. Gameplay keeps it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Sheltered(pub f32);

/// How often a sample is taken again, in seconds of play, by how far it is from the nearest
/// player: near, farther and far.
const REFRESH_S: [(f32, f64); 3] = [(48.0, 0.5), (200.0, 3.0), (f32::INFINITY, 20.0)];
/// At most this long a frame goes on taking samples; the rest wait for the next frame.
const SAMPLE_BUDGET_MS: f32 = 0.6;
/// Fires warm what is this near them.
const HEAT_REACH_M: f32 = 12.0;
/// Nothing is nearer a fire than this, for its heat: its flames are not a point.
const HEAT_NEAREST_M: f32 = 0.7;
/// Under the canopy, the share of the rain and wind that still gets through where the sky is
/// closed: leaves catch some of it, never all.
const UNDER_LEAVES: f32 = 0.4;
/// The cells the past is kept for, near things that care and round the camera.
const SAMPLE_CELLS: usize = 4096;
const MAP_CELLS: usize = 4096;

/// What the shaders read of the climate round the camera.
#[derive(Resource, Clone)]
pub struct ClimateMaps {
    /// What lies on and falls on the ground: snow depth in red, a texel's full brightness
    /// [`SNOW_FULL_M`]; the ground's wetness in green; rain falling now in blue; frost on
    /// what is out in the cold in alpha.
    pub ground: Option<Handle<Image>>,
    /// Each kind's leaves, a layer a kind in the years file's order: how many it has early
    /// and late in its kind's year in red and green, how far they have turned in blue and
    /// alpha (`terra_climate::SPREAD`).
    pub leaves: Option<Handle<Image>>,
    /// From a point in the world to its place on the maps, x and y from 0 to 1.
    pub from_world: Mat4,
    /// How many times the maps have been painted: a material that reads them is prepared
    /// again when it changes, so it draws from the new pictures.
    pub painting: u32,
    /// The leaves map's layer of the broadleaves whose fallen leaves lie on the forest floor
    /// (`FALLEN_KIND`), and of grass (`GRASS_KIND`), or -1 if there is none.
    pub fallen_layer: i32,
    pub grass_layer: i32,
    /// Each layer's kind's autumn colour, linear, in the years file's order.
    pub autumn: Vec<Vec3>,
}

impl Default for ClimateMaps {
    fn default() -> Self {
        Self {
            ground: None,
            leaves: None,
            from_world: Mat4::IDENTITY,
            painting: 0,
            fallen_layer: -1,
            grass_layer: -1,
            autumn: Vec::new(),
        }
    }
}

impl ClimateMaps {
    /// A layer's autumn colour, linear, as a shader takes it; grey where there is none.
    pub fn autumn_of(&self, layer: i32) -> Vec4 {
        usize::try_from(layer)
            .ok()
            .and_then(|layer| self.autumn.get(layer))
            .map_or(Vec4::splat(0.2), |colour| colour.extend(1.0))
    }

    /// A kind's layer in the leaves map, as the shaders take it: -1 if it has none.
    pub fn layer_of(fields: &ClimateFields, kind: &str) -> i32 {
        fields.layer(kind).map_or(-1, |layer| layer as i32)
    }
}

/// The kind whose fallen leaves are the forest floor's litter after the fall, and the kind
/// the ground cover's grass is.
pub const FALLEN_KIND: &str = "oak";
pub const GRASS_KIND: &str = "grass";

/// Snow this deep is the ground map's brightest.
pub const SNOW_FULL_M: f32 = 0.5;
/// Texels to a side of the maps, and how wide each is: a cell of the past each.
const TEXELS: u32 = 32;
const TEXEL_M: f32 = terra_climate::CELL_M;
/// The maps are painted again once the camera is this far from their middle, or the clock
/// has moved on this many hours, or at once when it jumps.
const REPAINT_M: f32 = 128.0;
const REPAINT_HOURS: f64 = 0.25;
/// Rain this heavy, in millimetres an hour, wets every surface through.
const SOAKING_MM_H: f32 = 0.5;
/// Frost lies on what is out in air this cold, from none at the first temperature to all at
/// the second.
const FROST_C: (f32, f32) = (1.0, -3.0);

/// The cache of the past near things that care.
#[derive(Resource)]
struct SampleCache(PastCache);

/// The maps being drawn and painted.
#[derive(Resource)]
struct Painting {
    ground: Handle<Image>,
    leaves: Handle<Image>,
    /// The middle and the moment of the maps now drawn.
    painted: Option<(Vec2, f64)>,
    task: Option<Task<Painted>>,
    /// The cache of the past round the camera, lent to the painting thread.
    cache: Option<PastCache>,
    /// Whether the first painting has been said in the log.
    said: bool,
}

struct Painted {
    took_ms: f32,
    middle: Vec2,
    time: f64,
    ground: Vec<u8>,
    leaves: Vec<u8>,
    cache: PastCache,
}

pub fn plugin(app: &mut App) {
    app.register_type::<ClimateSample>()
        .init_resource::<ClimateMaps>()
        .insert_resource(SampleCache(PastCache::new(SAMPLE_CELLS)))
        .add_systems(Startup, read_fields)
        .add_systems(Update, (sample, paint).after(crate::daylight::Ticking));
}

fn read_fields(
    mut commands: Commands,
    climate: Res<WorldClimate>,
    planet: Res<Planet>,
    mut images: ResMut<Assets<Image>>,
) {
    let years: Vec<LeafYear> = crate::data::read(YEARS_FILE);
    let kinds = years.len().max(1) as u32;
    commands.insert_resource(ClimateFields {
        fields: Arc::new(Fields::new(climate.0.clone(), planet.seed())),
        years: Arc::new(years),
        sea_radius_m: planet.sea_radius_m(),
        frame: world_frame(&planet, &climate.0),
    });
    let mut picture = |layers: u32, fill: [u8; 4]| {
        let mut image = Image::new_fill(
            Extent3d {
                width: TEXELS,
                height: TEXELS,
                depth_or_array_layers: layers,
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
    commands.insert_resource(Painting {
        ground: picture(1, [0, 0, 0, 0]),
        // In full leaf and green until painted.
        leaves: picture(kinds, [255, 255, 0, 0]),
        painted: None,
        task: None,
        cache: Some(PastCache::new(MAP_CELLS)),
        said: false,
    });
}

/// Where the nearest player is from a point, or the camera if there is none.
fn nearest(at: Vec3, players: &[Vec3]) -> f32 {
    players
        .iter()
        .map(|player| player.distance(at))
        .fold(f32::INFINITY, f32::min)
}

/// How often a sample this far from the nearest player is taken again.
fn refresh_after(distance: f32) -> f64 {
    REFRESH_S
        .iter()
        .find(|(within, _)| distance < *within)
        .map_or(REFRESH_S[2].1, |(_, every)| *every)
}

/// What a fire gives a body so far from it, in watts a square metre: its heat spread over a
/// sphere that far round it.
fn fire_at(watts: f32, distance: f32) -> f32 {
    let d = distance.max(HEAT_NEAREST_M);
    watts / (4.0 * std::f32::consts::PI * d * d)
}

/// Takes again the samples that are due, nearest players first in time, as far as a frame's
/// budget goes.
fn sample(
    fields: Option<Res<ClimateFields>>,
    clock: Res<Clock>,
    time: Res<Time>,
    mut cache: ResMut<SampleCache>,
    sky: SkySeen,
    viewpoints: Query<&GlobalTransform, With<Viewpoint>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    heats: Query<(&GlobalTransform, &Heat)>,
    mut samples: Query<(&GlobalTransform, &mut ClimateSample, Option<&Sheltered>)>,
) {
    let Some(fields) = fields else { return };
    let now_s = time.elapsed_secs_f64();
    let started = Instant::now();
    let mut players: Vec<Vec3> = viewpoints.iter().map(|v| v.translation()).collect();
    if players.is_empty() {
        players.extend(camera.iter().map(|c| c.translation()));
    }
    let world_time = clock.time();
    let heats: Vec<(Vec3, f32)> = heats
        .iter()
        .map(|(at, heat)| (at.translation(), heat.watts))
        .collect();
    for (at, mut sample, sheltered) in &mut samples {
        if sample.due_s > now_s {
            continue;
        }
        if started.elapsed().as_secs_f32() * 1000.0 > SAMPLE_BUDGET_MS {
            break;
        }
        let at = at.translation();
        let up = at.normalize_or(Vec3::Y);
        let open = sky.open_at(at + up * 1.5);
        let shelter = sheltered.map_or(0.0, |s| s.0.clamp(0.0, 1.0));
        let fire: f32 = heats
            .iter()
            .filter(|(heat, _)| heat.distance(at) < HEAT_REACH_M)
            .map(|(heat, watts)| fire_at(*watts, heat.distance(at + up)))
            .sum();
        let taken = take(
            &fields,
            &mut cache.0,
            at,
            world_time,
            Bent {
                open,
                shelter,
                fire,
            },
        );
        *sample = ClimateSample {
            due_s: now_s + refresh_after(nearest(at, &players)),
            ..taken
        };
    }
}

/// What bends the climate where something is: how open the sky is over it, how sheltered it
/// is, and fires' heat on it.
#[derive(Clone, Copy, Debug)]
struct Bent {
    open: f32,
    shelter: f32,
    fire: f32,
}

/// A sample at a point and a moment, bent so.
fn take(
    fields: &ClimateFields,
    cache: &mut PastCache,
    at: Vec3,
    time: WorldTime,
    bent: Bent,
) -> ClimateSample {
    let place = fields.place(at);
    let now = fields.fields.at(place, time);
    let past = fields.past_at(cache, at, time);
    let open = bent.open.clamp(0.0, 1.0);
    let leaves = UNDER_LEAVES + (1.0 - UNDER_LEAVES) * open;
    let out = 1.0 - bent.shelter;
    let falls = now.precipitation_mm_per_hour * leaves * out;
    ClimateSample {
        air_c: now.temperature_c,
        wind_m_s: now.wind.speed_m_s * leaves * (1.0 - 0.9 * bent.shelter),
        sun_w_m2: now.sunlight_w_m2 * open * out,
        rain_mm_h: falls * (1.0 - now.snow_share),
        snow_mm_h: falls * now.snow_share,
        humidity: now.humidity,
        ground_wetness: past.ground_wetness(),
        snow_depth_m: past.snow_depth_m(),
        ice_m: past.ice_m,
        // Walls and a roof keep a fire's warmth in.
        fire_w_m2: bent.fire * (1.0 + bent.shelter),
        open,
        taken: time.0,
        due_s: 0.0,
    }
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

/// Paints the maps round the camera on another thread when the camera or the clock has moved
/// on, and hands them to the shaders when done.
fn paint(
    fields: Option<Res<ClimateFields>>,
    painting: Option<ResMut<Painting>>,
    clock: Res<Clock>,
    planet: Res<Planet>,
    start: Res<Start>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut images: ResMut<Assets<Image>>,
    mut maps: ResMut<ClimateMaps>,
) {
    let (Some(fields), Some(mut painting), Some(camera)) = (fields, painting, camera.iter().next())
    else {
        return;
    };
    if let Some(task) = &mut painting.task
        && let Some(painted) = block_on(poll_once(task))
    {
        painting.task = None;
        painting.cache = Some(painted.cache);
        painting.painted = Some((painted.middle, painted.time));
        if let Some(mut image) = images.get_mut(&painting.ground) {
            image.data = Some(painted.ground);
        }
        if let Some(mut image) = images.get_mut(&painting.leaves) {
            image.data = Some(painted.leaves);
        }
        *maps = ClimateMaps {
            ground: Some(painting.ground.clone()),
            leaves: Some(painting.leaves.clone()),
            from_world: from_world(&start, painted.middle),
            painting: maps.painting.wrapping_add(1),
            fallen_layer: ClimateMaps::layer_of(&fields, FALLEN_KIND),
            grass_layer: ClimateMaps::layer_of(&fields, GRASS_KIND),
            autumn: fields
                .years
                .iter()
                .map(|year| {
                    let (r, g, b) = year.autumn;
                    Vec3::from(Color::srgb(r, g, b).to_linear().to_f32_array_no_alpha())
                })
                .collect(),
        };
        if !painting.said {
            painting.said = true;
            say(&fields, &painting, &images, clock.time(), painted.took_ms);
        }
    }
    let local = start.local(camera.translation());
    let here = Vec2::new(local.x, local.z);
    let now = clock.time().0;
    let stale = painting.painted.is_none_or(|(middle, time)| {
        middle.distance(here) > REPAINT_M || (now - time).abs() * 24.0 > REPAINT_HOURS
    });
    if painting.task.is_some() || !stale {
        return;
    }
    let Some(mut cache) = painting.cache.take() else {
        return;
    };
    // On a grid, so the same place is painted the same each time.
    let middle = (here / REPAINT_M).round() * REPAINT_M;
    let (fields, planet, start) = (fields.clone(), planet.clone(), *start);
    let time = WorldTime(now);
    painting.task = Some(AsyncComputeTaskPool::get().spawn(async move {
        let started = Instant::now();
        let (ground, leaves) = paint_maps(&fields, &mut cache, &planet, &start, middle, time);
        Painted {
            took_ms: started.elapsed().as_secs_f32() * 1000.0,
            middle,
            time: now,
            ground,
            leaves,
            cache,
        }
    }));
}

/// The maps' pixels round a middle at a moment.
fn paint_maps(
    fields: &ClimateFields,
    cache: &mut PastCache,
    planet: &Planet,
    start: &Start,
    middle: Vec2,
    time: WorldTime,
) -> (Vec<u8>, Vec<u8>) {
    let texels = (TEXELS * TEXELS) as usize;
    let kinds = fields.years.len().max(1);
    let mut ground = Vec::with_capacity(texels * 4);
    let mut leaves = vec![0u8; texels * 4 * kinds];
    let corner = corner(middle);
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let latitude = fields
        .frame
        .latitude(planet.surface(start.at(middle.x, middle.y).normalize()));
    let day_hours = fields.fields.daylight(latitude, time.day()).day_hours;
    for b in 0..TEXELS {
        for a in 0..TEXELS {
            let at = corner + (Vec2::new(a as f32, b as f32) + 0.5) * TEXEL_M;
            let point = planet.surface(start.at(at.x, at.y).normalize());
            let past = fields.past_at(cache, point, time);
            let now = fields.fields.at(fields.place(point), time);
            let rain = now.precipitation_mm_per_hour * (1.0 - now.snow_share);
            let frost = ((FROST_C.0 - now.temperature_c) / (FROST_C.0 - FROST_C.1)).clamp(0.0, 1.0);
            ground.extend([
                byte(past.snow_depth_m() / SNOW_FULL_M),
                byte(past.ground_wetness()),
                byte(rain / SOAKING_MM_H),
                byte(frost),
            ]);
            let texel = (b * TEXELS + a) as usize;
            for (layer, year) in fields.years.iter().enumerate() {
                let early = year.leaves(&past, day_hours, 1.0);
                let late = year.leaves(&past, day_hours, -1.0);
                let i = (layer * texels + texel) * 4;
                leaves[i..i + 4].copy_from_slice(&[
                    byte(early.on),
                    byte(late.on),
                    byte(early.turned),
                    byte(late.turned),
                ]);
            }
        }
    }
    (ground, leaves)
}

/// Says in the log what the climate is at the camera once its maps are first painted, so a
/// scripted run says which day of the year it shows.
fn say(
    fields: &ClimateFields,
    painting: &Painting,
    images: &Assets<Image>,
    time: WorldTime,
    took_ms: f32,
) {
    let year = &fields.fields.climate().year;
    let middle = |handle: &Handle<Image>, layer: usize| -> Option<[u8; 4]> {
        let data = images.get(handle)?.data.as_ref()?;
        let texel = ((TEXELS / 2) * TEXELS + TEXELS / 2) as usize;
        let i = (layer * (TEXELS * TEXELS) as usize + texel) * 4;
        data.get(i..i + 4)?.try_into().ok()
    };
    let share = |byte: u8| byte as f32 / 255.0;
    let mut said = format!(
        "climate: day {} of the clock, {:.3} of the year after midwinter, its maps painted in {took_ms:.0} ms",
        time.day(),
        year.phase(time.day())
    );
    if let Some([snow, wet, rain, frost]) = middle(&painting.ground, 0) {
        said += &format!(
            "; round the camera snow {:.2} m, ground wetness {:.2}, rain {:.2}, frost {:.2}",
            share(snow) * SNOW_FULL_M,
            share(wet),
            share(rain),
            share(frost)
        );
    }
    for (layer, year) in fields.years.iter().enumerate() {
        if let Some([on_early, on_late, turned_early, turned_late]) =
            middle(&painting.leaves, layer)
        {
            said += &format!(
                "; {} leaves {:.2}-{:.2} on, {:.2}-{:.2} turned",
                year.kind,
                share(on_early),
                share(on_late),
                share(turned_early),
                share(turned_late)
            );
        }
    }
    info!("{said}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_climate::Climate;

    fn fields() -> ClimateFields {
        let climate: Climate = crate::data::read(crate::daylight::WorldClimate::FILE);
        ClimateFields {
            fields: Arc::new(Fields::new(climate, terra_gen::Seed(1))),
            years: Arc::new(crate::data::read(YEARS_FILE)),
            sea_radius_m: 2000.0,
            frame: Frame::turned_for(Vec3::Y, LATITUDE, 0.17).0,
        }
    }

    const OPEN: Bent = Bent {
        open: 1.0,
        shelter: 0.0,
        fire: 0.0,
    };

    #[test]
    fn a_sample_is_the_climate_where_it_is_taken() {
        let fields = fields();
        let mut cache = PastCache::new(16);
        let at = Vec3::new(30.0, 2040.0, -10.0);
        let time = WorldTime::at(140, 9.0);
        let taken = take(&fields, &mut cache, at, time, OPEN);
        let now = fields.fields.at(fields.place(at), time);
        assert_eq!(taken.air_c, now.temperature_c);
        assert_eq!(taken.wind_m_s, now.wind.speed_m_s);
        assert_eq!(taken.sun_w_m2, now.sunlight_w_m2);
        let past = fields.past_at(&mut PastCache::new(4), at, time);
        assert_eq!(taken.snow_depth_m, past.snow_depth_m());
        // Forty metres higher is colder by the lapse rate, give or take the weather there.
        let high = take(&fields, &mut cache, at * (2080.0 / 2040.0), time, OPEN);
        assert!((taken.air_c - high.air_c - 0.26).abs() < 0.1);
    }

    #[test]
    fn shelter_shade_and_a_fire_bend_a_sample() {
        let fields = fields();
        let mut cache = PastCache::new(16);
        let at = Vec3::new(0.0, 2030.0, 0.0);
        // A wet, windy day at noon, found by looking.
        let time = (0..400)
            .map(|day| WorldTime::at(day, 12.0))
            .find(|time| {
                let now = fields.fields.at(fields.place(at), *time);
                now.precipitation_mm_per_hour > 0.5 && now.sunlight_w_m2 > 50.0
            })
            .expect("a wet noon in a year");
        let open = take(&fields, &mut cache, at, time, OPEN);
        let room = take(
            &fields,
            &mut cache,
            at,
            time,
            Bent {
                shelter: 1.0,
                ..OPEN
            },
        );
        assert_eq!(room.rain_mm_h + room.snow_mm_h, 0.0);
        assert_eq!(room.sun_w_m2, 0.0);
        assert!(room.wind_m_s < open.wind_m_s * 0.2);
        assert_eq!(room.air_c, open.air_c, "walls alone do not warm the air");
        let wood = take(&fields, &mut cache, at, time, Bent { open: 0.0, ..OPEN });
        assert_eq!(wood.sun_w_m2, 0.0);
        let falls = |s: &ClimateSample| s.rain_mm_h + s.snow_mm_h;
        assert!(
            falls(&wood) < falls(&open) && falls(&wood) > 0.0,
            "leaves catch some"
        );
        let by_fire = take(
            &fields,
            &mut cache,
            at,
            time,
            Bent {
                fire: 100.0,
                ..OPEN
            },
        );
        assert_eq!(by_fire.fire_w_m2, 100.0);
        let fire_indoors = take(
            &fields,
            &mut cache,
            at,
            time,
            Bent {
                fire: 100.0,
                shelter: 1.0,
                ..OPEN
            },
        );
        assert!(fire_indoors.fire_w_m2 > by_fire.fire_w_m2);
    }

    #[test]
    fn a_fire_warms_less_the_farther_and_its_heat_is_its_power_spread_round_it() {
        // Counted another way: at 2 m, 1000 W over a sphere of 4 pi 4 square metres.
        assert!((fire_at(1000.0, 2.0) - 1000.0 / 50.265).abs() < 0.01);
        assert!(fire_at(1000.0, 4.0) < fire_at(1000.0, 2.0) / 3.9);
        assert_eq!(fire_at(1000.0, 0.0), fire_at(1000.0, HEAT_NEAREST_M));
    }

    #[test]
    fn near_samples_are_taken_often_and_far_ones_rarely() {
        assert!(refresh_after(5.0) < refresh_after(100.0));
        assert!(refresh_after(100.0) < refresh_after(5000.0));
        assert_eq!(nearest(Vec3::ZERO, &[Vec3::X * 3.0, Vec3::Y * 2.0]), 2.0);
    }

    #[test]
    fn a_place_maps_to_where_it_is_painted() {
        let start = Start(Transform::from_xyz(3.0, 100.0, -2.0).looking_to(Vec3::X, Vec3::Y));
        let middle = Vec2::new(128.0, -256.0);
        let to_map = from_world(&start, middle);
        let at = |east: f32, south: f32| to_map.transform_point3(start.at(east, south)).truncate();
        assert!(at(128.0, -256.0).distance(Vec2::splat(0.5)) < 1e-4);
        let corner = corner(middle);
        assert!(at(corner.x, corner.y).length() < 1e-4);
        let size = TEXELS as f32 * TEXEL_M;
        assert!(at(corner.x + size, corner.y).distance(Vec2::X) < 1e-4);
        assert!(at(corner.x, corner.y + size).distance(Vec2::Y) < 1e-4);
    }

    #[test]
    fn the_maps_say_what_the_kinds_years_say_and_where_snow_lies() {
        let fields = fields();
        let planet = Planet::smooth(terra_core::PlanetSpec {
            seed: terra_gen::Seed(1),
            radius_m: 2000.0,
        });
        let start = Start::choose(&planet, true);
        let oak = fields.layer("oak").expect("an oak in the years file");
        let pine = fields.layer("pine").expect("a pine in the years file");
        let mut cache = PastCache::new(MAP_CELLS);
        let texel = (TEXELS / 2 * TEXELS + TEXELS / 2) as usize * 4;
        let texels = (TEXELS * TEXELS) as usize * 4;
        let at = |cache: &mut PastCache, day: i64| {
            let (ground, leaves) = paint_maps(
                &fields,
                cache,
                &planet,
                &start,
                Vec2::ZERO,
                WorldTime::at(day, 12.0),
            );
            (
                ground[texel],
                leaves[oak * texels + texel],
                leaves[pine * texels + texel],
            )
        };
        // The clock starts in late August, under the summer's leaves.
        let (snow, oak_on, pine_on) = at(&mut cache, 0);
        assert_eq!((snow, oak_on, pine_on), (0, 255, 255));
        // By mid January the oak is bare and the pine is not.
        let (_, oak_on, pine_on) = at(&mut cache, 143);
        assert_eq!(oak_on, 0);
        assert!(pine_on > 200);
        // And somewhere in the winter snow lies.
        let snowy = (100..200).step_by(5).any(|day| at(&mut cache, day).0 > 0);
        assert!(snowy, "no snow all winter");
    }
}
