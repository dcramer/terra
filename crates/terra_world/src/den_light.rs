//! The sky's light in a den (D591, D643, docs/caves.md step 3): underground the sky is shut
//! out, so a cave is lit by what of it comes in at the mouth and no more. As the forest's
//! shade is (`sky_occlusion`, D270), by an irradiance volume: a grid of cells over the den,
//! each given how much of the sky each of its six sides sees, here by looking out through the
//! ground itself along a few ways a side, once, on another thread; lit from the open sky as
//! it is painted, each side by its share. What no sky reaches keeps a trace of light
//! ([`DARK`]), enough to tell a wall from the dark; a torch does the rest.
//!
//! Baked from the ground as it was generated: a shaft dug into a den lets no light in yet.

use crate::planet::Planet;
use crate::sky_occlusion::{rgb9e5, world_sides};
use crate::skylight::OpenSky;
use bevy::light::IrradianceVolume;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future};
use terra_core::Ground;
use terra_voxel::caves::Part;

/// The side of a cell, in metres: half a passage's width.
const CELL_M: f32 = 3.0;
/// The volume reaches this far past the den's parts, so the light eases in outside its mouth.
const MARGIN_M: f32 = 6.0;
/// How far a look for the sky goes before what it has not met is taken for sky.
const LOOK_M: f32 = 70.0;
/// The share of the open sky's light where none of it reaches: the eye's dark adaptation
/// stood in for, so the dark has shapes (Valheim's and Skyrim's caves are never black).
const DARK: f32 = 0.03;
/// Light that came in is thrown on by the rock: a cell has this share of the light of the
/// brightest cell beside it, so a passage dims by half every two cells from where the sky
/// was last seen, not at once (looked at: with the sky's own share alone, eighteen metres
/// in was black). And how much of the sky's light the rock throws back.
const THROWN_ON: f32 = 0.7;
const ROCK_THROWS: f32 = 0.35;
/// The ways a side looks, as so far out from its axis along each of the two beside it:
/// straight out, four a little off and four well off.
const WAYS: [(f32, f32); 9] = [
    (0.0, 0.0),
    (0.45, 0.0),
    (-0.45, 0.0),
    (0.0, 0.45),
    (0.0, -0.45),
    (1.1, 1.1),
    (-1.1, 1.1),
    (1.1, -1.1),
    (-1.1, -1.1),
];

/// A den's cells and how much of the sky each side of each sees.
struct Seen {
    cells: UVec3,
    sides: Vec<[f32; 6]>,
    /// How much light is about in each cell, of the open sky's: what it sees of the sky,
    /// or what the cells beside it throw on, whichever is more.
    about: Vec<f32>,
    /// The box the cells fill: its middle, and its axes each as long as the box.
    middle: Vec3,
    axes: Mat3,
}

/// Made afresh in each world: its den's light, being worked out or lit.
#[derive(Resource, Default)]
enum Light {
    #[default]
    None,
    Baking(Task<Seen>),
    Lit {
        seen: std::sync::Arc<Seen>,
        image: Handle<Image>,
        entity: Entity,
        from: Option<Box<OpenSky>>,
    },
    /// A world with no den.
    Nothing,
}

pub fn plugin(app: &mut App) {
    use crate::instance::InstanceApp;
    app.init_world_resource::<Light>()
        .add_systems(Update, light.run_if(crate::instance::in_world))
        .add_systems(
            PostUpdate,
            shut_in
                .before(crate::indoor::Shuts)
                .run_if(crate::instance::in_world),
        );
}

impl Seen {
    /// How much light is about at a point, of the open sky's, if it is in the den's box.
    fn about_at(&self, point: Vec3) -> Option<f32> {
        let axis = |axis: Vec3| (point - self.middle).dot(axis) / axis.length_squared() + 0.5;
        let at = Vec3::new(
            axis(self.axes.x_axis),
            axis(self.axes.y_axis),
            axis(self.axes.z_axis),
        );
        if at.cmplt(Vec3::ZERO).any() || at.cmpge(Vec3::ONE).any() {
            return None;
        }
        let cell = (at * self.cells.as_vec3()).as_uvec3().min(self.cells - 1);
        let index = (cell.z * self.cells.y + cell.y) * self.cells.x + cell.x;
        self.about.get(index as usize).copied()
    }
}

/// In a den the eye is shut off from the sky as it is in a closed room (`indoor`, D485): no
/// more of the sky's light reaches the air round the camera than is about it there, so the
/// eye opens to the room's key, its dark has the cool fill a dungeon's has, and a torch
/// burns as by night. Said after whatever else says how much sky reaches the eye (a
/// building's rooms) and before it is eased, so leaving the den eases out as leaving a room.
fn shut_in(
    state: Res<Light>,
    camera: Query<&GlobalTransform, With<crate::planet::MainCamera>>,
    air: Option<ResMut<crate::scene::IndoorSky>>,
) {
    let (Light::Lit { seen, .. }, Some(camera), Some(mut air)) =
        (&*state, camera.iter().next(), air)
    else {
        return;
    };
    if let Some(about) = seen.about_at(camera.translation())
        && about < air.0
    {
        air.0 = about;
    }
}

/// The box that holds a den's parts and a margin, upright as the planet is at its middle.
fn bounds(parts: &[Part]) -> (Vec3, Mat3, Vec3) {
    let balls: Vec<(Vec3, f32)> = parts
        .iter()
        .flat_map(|part| match *part {
            Part::Passage { from, to, radius_m } => vec![(from, radius_m), (to, radius_m)],
            Part::Chamber {
                middle,
                across_m,
                high_m,
                ..
            } => {
                vec![(middle, across_m.max(high_m) * 0.5)]
            }
        })
        .collect();
    let about = balls.iter().map(|(at, _)| *at).sum::<Vec3>() / balls.len().max(1) as f32;
    let up = about.normalize_or(Vec3::Y);
    let east = up.any_orthonormal_vector();
    let frame = Mat3::from_cols(east, up, east.cross(up));
    let (mut low, mut high) = (Vec3::MAX, Vec3::MIN);
    for (at, radius) in balls {
        let local = frame.transpose() * (at - about);
        low = low.min(local - radius - MARGIN_M);
        high = high.max(local + radius + MARGIN_M);
    }
    (about + frame * ((low + high) * 0.5), frame, high - low)
}

/// How much of the sky each side of each cell of a den's box sees through the ground.
fn seen(ground: &(impl Fn(Vec3) -> f32 + Sync), parts: &[Part]) -> Seen {
    let (middle, frame, size) = bounds(parts);
    let cells = (size / CELL_M).ceil().as_uvec3().max(UVec3::ONE);
    let step = size / cells.as_vec3();
    let axes = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
    // Whether a look from a place a way meets no ground before it is far off.
    let open = |from: Vec3, way: Vec3| {
        let mut gone = 0.6;
        while gone < LOOK_M {
            let away = ground(from + way * gone);
            if away < 0.0 {
                return false;
            }
            gone += away.max(0.5);
        }
        true
    };
    let slabs: Vec<u32> = (0..cells.z).collect();
    let sides = terra_gen::par::map(&slabs, |&z| {
        let mut slab = Vec::with_capacity((cells.x * cells.y) as usize);
        for y in 0..cells.y {
            for x in 0..cells.x {
                let local = (UVec3::new(x, y, z).as_vec3() + 0.5) * step - size * 0.5;
                let at = middle + frame * local;
                // In the rock nothing is drawn; a cell at a wall is asked from in the open.
                if ground(at) < -CELL_M {
                    slab.push([0.0; 6]);
                    continue;
                }
                slab.push(axes.map(|axis| {
                    let (a, b) = (
                        axis.any_orthonormal_vector(),
                        axis.cross(axis.any_orthonormal_vector()),
                    );
                    let reached = WAYS
                        .iter()
                        .filter(|(u, v)| open(at, (axis + a * *u + b * *v).normalize()))
                        .count();
                    reached as f32 / WAYS.len() as f32
                }));
            }
        }
        slab
    });
    let sides: Vec<[f32; 6]> = sides.into_iter().flatten().collect();
    let about = thrown_on(cells, &sides);
    Seen {
        cells,
        sides,
        about,
        middle,
        axes: Mat3::from_cols(
            frame.x_axis * size.x,
            frame.y_axis * size.y,
            frame.z_axis * size.z,
        ),
    }
}

/// The light about in each cell: its own share of the sky, or [`THROWN_ON`] of the brightest
/// cell beside it, passed on from cell to cell until nothing changes much.
fn thrown_on(cells: UVec3, sides: &[[f32; 6]]) -> Vec<f32> {
    let mut about: Vec<f32> = sides
        .iter()
        .map(|side| side.iter().sum::<f32>() / 6.0)
        .collect();
    let (nx, ny, nz) = (cells.x as usize, cells.y as usize, cells.z as usize);
    let at = |x: usize, y: usize, z: usize| (z * ny + y) * nx + x;
    for _ in 0..16 {
        let before = about.clone();
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    let beside = [
                        (x > 0).then(|| at(x - 1, y, z)),
                        (x + 1 < nx).then(|| at(x + 1, y, z)),
                        (y > 0).then(|| at(x, y - 1, z)),
                        (y + 1 < ny).then(|| at(x, y + 1, z)),
                        (z > 0).then(|| at(x, y, z - 1)),
                        (z + 1 < nz).then(|| at(x, y, z + 1)),
                    ];
                    let most = beside
                        .into_iter()
                        .flatten()
                        .map(|k| before[k])
                        .fold(0.0, f32::max);
                    let here = at(x, y, z);
                    about[here] = before[here].max(most * THROWN_ON);
                }
            }
        }
    }
    about
}

/// The volume's texture, laid out as Bevy's shader reads it (`sky_occlusion::texels` says
/// how): each side of each cell the open sky's light for the share it sees, the ground's
/// bounce for as much of the sky as the cell sees at all, and a trace where it sees none.
fn texels(seen: &Seen, sides: &[(Vec3, Vec3); 6]) -> Vec<u8> {
    let (nx, ny, nz) = (
        seen.cells.x as usize,
        seen.cells.y as usize,
        seen.cells.z as usize,
    );
    let mut data = vec![0u8; nx * ny * 2 * nz * 3 * 4];
    let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
    let trace = Vec3::splat(luma(sides[2].0) * DARK);
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                let cell = (z * ny + y) * nx + x;
                let (shares, about) = (seen.sides[cell], seen.about[cell]);
                for (side, &(sky, ground)) in sides.iter().enumerate() {
                    let thrown = (ground + Vec3::splat(luma(sky) * ROCK_THROWS)) * about;
                    let light = sky * shares[side] + thrown + trace;
                    let t = y + if side % 2 == 1 { ny } else { 0 };
                    let p = z + side / 2 * nz;
                    let at = ((p * ny * 2 + t) * nx + x) * 4;
                    data[at..at + 4].copy_from_slice(&rgb9e5(light).to_le_bytes());
                }
            }
        }
    }
    data
}

/// Works the den's light out once in each world, then lights it from the sky as it is
/// painted, again whenever the sky has changed. Its volume is in the world once first lit.
fn light(
    mut commands: Commands,
    mut state: ResMut<Light>,
    planet: Res<Planet>,
    open: Option<Res<OpenSky>>,
    mut images: ResMut<Assets<Image>>,
    mut volumes: Query<&mut IrradianceVolume>,
) {
    match &mut *state {
        Light::Nothing => {}
        Light::None => {
            let Some(den) = planet.den() else {
                *state = Light::Nothing;
                return;
            };
            let (ground, parts) = (planet.voxels().clone(), den.parts.clone());
            *state = Light::Baking(
                AsyncComputeTaskPool::get()
                    .spawn(async move { seen(&|at| ground.distance(at), &parts) }),
            );
        }
        Light::Baking(task) => {
            let Some(seen) = block_on(future::poll_once(task)) else {
                return;
            };
            let size = Extent3d {
                width: seen.cells.x,
                height: seen.cells.y * 2,
                depth_or_array_layers: seen.cells.z * 3,
            };
            let image = images.add(Image::new_fill(
                size,
                TextureDimension::D3,
                &[0; 4],
                TextureFormat::Rgb9e5Ufloat,
                bevy::asset::RenderAssetUsages::RENDER_WORLD
                    | bevy::asset::RenderAssetUsages::MAIN_WORLD,
            ));
            let entity = commands
                .spawn((
                    Name::new("Den light"),
                    Transform::from_matrix(Mat4::from_mat3(seen.axes))
                        .with_translation(seen.middle),
                ))
                .id();
            *state = Light::Lit {
                seen: std::sync::Arc::new(seen),
                image,
                entity,
                from: None,
            };
        }
        Light::Lit {
            seen,
            image,
            entity,
            from,
        } => {
            let Some(open) = open else { return };
            if let Ok(mut volume) = volumes.get_mut(*entity)
                && volume.intensity != open.brightness
            {
                volume.intensity = open.brightness;
            }
            if from.as_deref() == Some(&*open) {
                return;
            }
            *from = Some(Box::new(*open));
            if let Some(mut picture) = images.get_mut(&*image) {
                picture.data = Some(texels(seen, &world_sides(&open)));
                // In the world, for a view to gather, only from its first light: a volume
                // with none in it would black out all it holds (D485).
                if !volumes.contains(*entity) {
                    commands.entity(*entity).insert(IrradianceVolume {
                        voxels: image.clone(),
                        intensity: open.brightness,
                        affects_lightmapped_meshes: false,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dens_volume_is_in_the_world_only_once_it_is_lit() {
        // D485: a fragment inside a volume takes its diffuse light from the volume alone, so
        // one gathered before its light is written blacks out the den and the land over it.
        // Worked out but with no sky painted yet, nothing is there to gather; with a sky,
        // a volume with light in it is.
        use crate::sky_occlusion::lit_volumes;
        let seen = Seen {
            cells: UVec3::splat(2),
            sides: vec![[1.0; 6]; 8],
            about: vec![1.0; 8],
            middle: Vec3::Y * 1000.0,
            axes: Mat3::from_diagonal(Vec3::splat(6.0)),
        };
        let pool = AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::new);
        let mut app = App::new();
        app.insert_resource(Planet::smooth(terra_core::PlanetSpec {
            seed: terra_core::Seed(1),
            radius_m: 2000.0,
        }))
        .init_resource::<Assets<Image>>()
        .insert_resource(Light::Baking(pool.spawn(async move { seen })))
        .add_systems(Update, light);
        let mut frames = 0;
        while !matches!(*app.world().resource::<Light>(), Light::Lit { .. }) {
            app.update();
            frames += 1;
            assert!(frames < 20_000, "the den's light was never worked out");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        app.update();
        assert!(lit_volumes(&mut app).is_empty());
        app.insert_resource(crate::skylight::clear_sky(
            Vec3::new(0.3, 0.5, 0.8),
            Quat::IDENTITY,
        ));
        app.update();
        assert_eq!(lit_volumes(&mut app).len(), 1);
    }

    /// A level ground with a chamber under it and a shaft up to the air at one end: sky
    /// reaches down the shaft and a little way in, the chamber's far end sees none, and the
    /// open air over the ground sees all of it upward.
    #[test]
    fn the_sky_reaches_in_at_the_mouth_and_not_the_far_end() {
        let base = Vec3::Y * 1000.0;
        let parts = vec![
            Part::Passage {
                from: base + Vec3::new(0.0, -12.0, 0.0),
                to: base + Vec3::new(40.0, -12.0, 0.0),
                radius_m: 3.0,
            },
            Part::Passage {
                from: base + Vec3::new(0.0, -12.0, 0.0),
                to: base + Vec3::new(0.0, 4.0, 0.0),
                radius_m: 3.0,
            },
        ];
        // The ground: below the plane y = 1000, less the two capsules.
        let capsule = |p: Vec3, a: Vec3, b: Vec3, r: f32| {
            let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
            p.distance(a + (b - a) * t) - r
        };
        let (a, b, c) = (
            base + Vec3::new(0.0, -12.0, 0.0),
            base + Vec3::new(40.0, -12.0, 0.0),
            base + Vec3::new(0.0, 4.0, 0.0),
        );
        let ground = move |p: Vec3| {
            let cave = capsule(p, a, b, 3.0).min(capsule(p, a, c, 3.0));
            (p.y - 1000.0).max(-cave)
        };
        let seen = seen(&ground, &parts);
        let at = |p: Vec3| {
            let local = Mat3::from_cols(
                seen.axes.x_axis / seen.axes.x_axis.length_squared(),
                seen.axes.y_axis / seen.axes.y_axis.length_squared(),
                seen.axes.z_axis / seen.axes.z_axis.length_squared(),
            )
            .transpose()
                * (p - seen.middle)
                + 0.5;
            let cell = (local * seen.cells.as_vec3())
                .floor()
                .as_uvec3()
                .min(seen.cells - 1);
            seen.sides[((cell.z * seen.cells.y + cell.y) * seen.cells.x + cell.x) as usize]
        };
        // +Y is side 2. Under the shaft the sky is seen overhead; at the far end not at all.
        assert!(at(base + Vec3::new(0.0, -12.0, 0.0))[2] > 0.1);
        assert_eq!(at(base + Vec3::new(36.0, -12.0, 0.0)), [0.0; 6]);
        // Over the ground the whole sky is overhead, and none below.
        let over = at(base + Vec3::new(20.0, 3.0, 0.0));
        assert_eq!((over[2], over[3]), (1.0, 0.0));
        // The light about at a point: little at the far end, all of it over the ground, and
        // nothing said of a place outside the den's box.
        assert!(seen.about_at(base + Vec3::new(36.0, -12.0, 0.0)).unwrap() < 0.2);
        assert!(seen.about_at(base + Vec3::new(20.0, 3.0, 0.0)).unwrap() > 0.4);
        assert_eq!(seen.about_at(base + Vec3::new(500.0, 0.0, 0.0)), None);
        // Lit, the far end keeps a trace and the open air is many times brighter.
        let sky = [(Vec3::ONE, Vec3::splat(0.2)); 6];
        let data = texels(&seen, &sky);
        assert!(data.iter().any(|&byte| byte != 0));
    }
}
