//! How much of the sky the crowns of a forest hide, everywhere under and among them, so the
//! shade under a closed canopy is lit by the little sky that comes through it and the green
//! light of the leaves, not by the whole open sky (D270). Without it a sun fleck on the path
//! was only half as bright again as the shade beside it; in a real wood it is many times.
//!
//! Shipped games do this with a volume of precomputed light every surface looks up: Far Cry 4
//! and 5 darken the sky's light under trees from a world ambient occlusion map, and baked
//! irradiance volumes (Unreal's volumetric lightmaps, Unity's light probe volumes) do it in
//! three dimensions. This is Bevy's own irradiance volume, of Half-Life 2's ambient cubes,
//! filled not by a baking tool but from the crowns themselves: the layer above says where
//! its crowns are ([`SkyOccluders`]); each cell of a grid over the forest is given how much
//! of the sky each of its six sides sees through them, once, on another thread; and each time
//! the sky is painted again ([`OpenSky`]), the volume's light is the open sky's for what is
//! seen, the leaves' glow for what is hidden, and the ground's bounce, dimmed by the shade it
//! lies in. Everything drawn with Bevy's lighting picks it up: the ground, grass, ferns,
//! bark, rocks and the player alike. Above the canopy and past the forest's edge it sees the
//! whole sky, as the environment map does there, so there is no seam where it ends.

use crate::shell::Options;
use crate::skylight::OpenSky;
use bevy::{
    asset::RenderAssetUsages,
    light::IrradianceVolume,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future},
};
use std::sync::Arc;

/// A crown, as an ellipsoid of leaves: its middle, its half sizes along east, up and south,
/// and how much of the sky it hides looking straight up through its middle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crown {
    pub middle: Vec3,
    pub half: Vec3,
    pub closes: f32,
}

/// What hides the sky, which the layer above inserts once its trees stand: a square of the
/// world round `middle`, on the ground, `half_m` each way, from `below_m` under it to
/// `above_m` over it, and the crowns in it.
#[derive(Resource, Clone, Debug)]
pub struct SkyOccluders {
    pub middle: Vec3,
    pub up: Vec3,
    pub east: Vec3,
    pub half_m: f32,
    pub below_m: f32,
    pub above_m: f32,
    pub crowns: Vec<Crown>,
}

/// The grid is this fine across and up: light from the sky changes slowly from place to
/// place, and the volume is sampled between cells.
const CELL_M: f32 = 4.0;
const LAYER_M: f32 = 2.5;
/// How far a look toward the sky is followed through the crowns.
const REACH_M: f32 = 90.0;
/// The light a crown's underside gives where it hides the sky, as a share of the sky's it
/// hides: sunlight through the leaves above, green, a quarter as bright as the sky they hide. A
/// twelfth was too little: what is not a leaf, skin, leather and stone, was lit by the gaps'
/// blue sky alone and read lilac (D286).
const LEAF_GLOW: Vec3 = Vec3::new(0.18, 0.27, 0.09);
/// How much of the ground's bounce is left in the deepest shade, where little sun reaches it.
const SHADED_GROUND: f32 = 0.1;

/// Each cell's share of the sky seen from each side, +X, -X, +Y, -Y, +Z, -Z of the world,
/// and straight up, as the grid holds them.
struct Seen {
    cells: UVec3,
    sides: Vec<[f32; 6]>,
    up: Vec<f32>,
}

#[derive(Resource)]
struct Baking(Task<Seen>);

#[derive(Resource)]
struct Baked {
    seen: Arc<Seen>,
    image: Handle<Image>,
    entity: Entity,
    /// The sky the volume was last lit from, and its light for a sky since, being worked out.
    from: Option<OpenSky>,
    lighting: Option<Task<Vec<u8>>>,
}

/// The volume is lit again when any of the sky's colours has moved by this share of itself:
/// filling and sending its texture takes a few milliseconds, so not every time the sky is
/// painted, and its brightness, which changes more, is the volume's own and follows at once.
const RELIT: f32 = 0.03;

/// How much of the sky the main camera sees overhead, from 0 under a closed canopy to 1 in
/// the open, from the same cells; there once the volume is (D273). What depends on whether
/// the eye is under the trees or out of them reads it: the forest's haze (`daylight`).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct OpenOverhead(pub f32);

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (bake, receive, light, overhead).chain());
}

/// How much of the sky is open over any point, from 0 under a closed canopy to 1 in the open,
/// from the same cells: what shades a body from the sun and shelters it from the rain and the
/// wind (`climate`). Everywhere open until the volume is worked out, and where there is none.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SkySeen<'w> {
    occluders: Option<Res<'w, SkyOccluders>>,
    baked: Option<Res<'w, Baked>>,
}

impl SkySeen<'_> {
    pub fn open_at(&self, point: Vec3) -> f32 {
        match (&self.occluders, &self.baked) {
            (Some(o), Some(baked)) => open_at(o, &baked.seen, point),
            _ => 1.0,
        }
    }
}

/// Follows how open the sky is over the camera, between the cells round it.
fn overhead(
    mut commands: Commands,
    occluders: Option<Res<SkyOccluders>>,
    baked: Option<Res<Baked>>,
    camera: Query<&GlobalTransform, With<crate::planet::MainCamera>>,
    open: Option<ResMut<OpenOverhead>>,
) {
    let (Some(o), Some(baked), Some(camera)) = (occluders, baked, camera.iter().next()) else {
        return;
    };
    let now = OpenOverhead(open_at(o.as_ref(), &baked.seen, camera.translation()));
    match open {
        Some(mut open) => {
            open.set_if_neq(now);
        }
        None => commands.insert_resource(now),
    }
}

/// How much of the sky is seen overhead at a point, blended between the eight cells round
/// it; outside the volume, all of it, as the volume sees there at its edges.
fn open_at(o: &SkyOccluders, seen: &Seen, point: Vec3) -> f32 {
    let south = o.east.cross(o.up);
    let height = o.above_m + o.below_m;
    let size = Vec3::new(o.half_m * 2.0, height, o.half_m * 2.0);
    let step = size / seen.cells.as_vec3();
    let from = point - o.middle;
    let local = Vec3::new(
        from.dot(o.east) + o.half_m,
        from.dot(o.up) + o.below_m,
        from.dot(south) + o.half_m,
    );
    if local.cmplt(Vec3::ZERO).any() || local.cmpgt(size).any() {
        return 1.0;
    }
    let max = (seen.cells - 1).as_vec3();
    let at = (local / step - 0.5).clamp(Vec3::ZERO, max);
    let low = at.floor().as_uvec3();
    let high = (low + 1).min(seen.cells - 1);
    let t = at - low.as_vec3();
    let cell =
        |x: u32, y: u32, z: u32| seen.up[((z * seen.cells.y + y) * seen.cells.x + x) as usize];
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let row = |y: u32, z: u32| lerp(cell(low.x, y, z), cell(high.x, y, z), t.x);
    let layer = |z: u32| lerp(row(low.y, z), row(high.y, z), t.y);
    lerp(layer(low.z), layer(high.z), t.z)
}

/// Starts working out what each cell sees once there are crowns.
fn bake(mut commands: Commands, occluders: Option<Res<SkyOccluders>>) {
    let Some(occluders) = occluders else { return };
    if !occluders.is_added() {
        return;
    }
    let occluders = occluders.clone();
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let started = std::time::Instant::now();
        let seen = seen(&occluders);
        info!(
            "sky occlusion: {} crowns, {}x{}x{} cells, worked out in {:.2} s",
            occluders.crowns.len(),
            seen.cells.x,
            seen.cells.y,
            seen.cells.z,
            started.elapsed().as_secs_f32()
        );
        seen
    });
    commands.insert_resource(Baking(task));
}

/// Puts the volume in the world once it is worked out: at once in a scripted run, so every
/// picture is lit the same.
fn receive(
    mut commands: Commands,
    options: Res<Options>,
    occluders: Option<Res<SkyOccluders>>,
    baking: Option<ResMut<Baking>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(mut baking), Some(occluders)) = (baking, occluders) else {
        return;
    };
    let ready = if options.scripted() && !options.bench {
        Some(block_on(&mut baking.0))
    } else {
        block_on(future::poll_once(&mut baking.0))
    };
    let Some(seen) = ready else { return };
    commands.remove_resource::<Baking>();
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
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    ));
    let o = &*occluders;
    let south = o.east.cross(o.up);
    let height = o.above_m + o.below_m;
    let entity = commands
        .spawn((
            Name::new("Sky occlusion"),
            Transform {
                translation: o.middle + o.up * (o.above_m - o.below_m) * 0.5,
                rotation: Quat::from_mat3(&Mat3::from_cols(o.east, o.up, south)),
                scale: Vec3::new(o.half_m * 2.0, height, o.half_m * 2.0),
            },
            IrradianceVolume {
                voxels: image.clone(),
                intensity: 0.0,
                affects_lightmapped_meshes: false,
            },
        ))
        .id();
    commands.insert_resource(Baked {
        seen: Arc::new(seen),
        image,
        entity,
        from: None,
        lighting: None,
    });
}

/// Lights the volume from the sky as it is painted now, on another thread: at once in a
/// scripted run, so every picture is lit the same.
fn light(
    options: Res<Options>,
    open: Res<OpenSky>,
    occluders: Option<Res<SkyOccluders>>,
    baked: Option<ResMut<Baked>>,
    mut images: ResMut<Assets<Image>>,
    mut volumes: Query<&mut IrradianceVolume>,
) {
    let (Some(mut baked), Some(occluders)) = (baked, occluders) else {
        return;
    };
    if let Ok(mut volume) = volumes.get_mut(baked.entity)
        && volume.intensity != open.brightness
    {
        volume.intensity = open.brightness;
    }
    if let Some(task) = baked.lighting.as_mut() {
        let ready = if options.scripted() && !options.bench {
            Some(block_on(task))
        } else {
            block_on(future::poll_once(task))
        };
        if let Some(data) = ready {
            baked.lighting = None;
            if let Some(mut image) = images.get_mut(&baked.image) {
                image.data = Some(data);
            }
        }
        return;
    }
    if baked.from.is_some_and(|from| !moved(&from, &open)) {
        return;
    }
    baked.from = Some(*open);
    let (seen, sides) = (baked.seen.clone(), world_sides(&occluders, &open));
    baked.lighting = Some(AsyncComputeTaskPool::get().spawn(async move { texels(&seen, &sides) }));
}

/// Whether any of the sky's colours has moved by more than [`RELIT`] of itself.
fn moved(from: &OpenSky, to: &OpenSky) -> bool {
    from.sky
        .iter()
        .chain(&from.ground)
        .zip(to.sky.iter().chain(&to.ground))
        .any(|(a, b)| (*a - *b).abs().max_element() > RELIT * a.max_element().max(1e-3))
}

/// The open sky's light toward each side of the world, +X, -X, +Y, -Y, +Z, -Z, from the
/// sky's own frame: what comes from above the horizon, and from the ground.
fn world_sides(o: &SkyOccluders, open: &OpenSky) -> [(Vec3, Vec3); 6] {
    let south = o.east.cross(o.up);
    let axes = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
    axes.map(|axis| {
        // The side, in the sky's frame, as a blend of its six sides by the square of each
        // part, as an ambient cube is read.
        let local = Vec3::new(axis.dot(o.east), axis.dot(o.up), axis.dot(south));
        let mut sky = Vec3::ZERO;
        let mut ground = Vec3::ZERO;
        for (k, part) in [local.x, -local.x, local.y, -local.y, local.z, -local.z]
            .into_iter()
            .enumerate()
        {
            let w = part.max(0.0) * part.max(0.0);
            sky += open.sky[k] * w;
            ground += open.ground[k] * w;
        }
        (sky, ground)
    })
}

/// The volume's texture: for each cell and side, the open sky's light for the share seen,
/// the leaves' glow for the share hidden, and the ground's bounce, dimmed in deep shade. Laid
/// out as Bevy's irradiance volumes are: the negative sides in the first half of each slab,
/// the positive in the second; X, then Y, then Z, one slab after the other.
fn texels(seen: &Seen, sides: &[(Vec3, Vec3); 6]) -> Vec<u8> {
    let UVec3 {
        x: nx,
        y: ny,
        z: nz,
    } = seen.cells;
    let (nx, ny, nz) = (nx as usize, ny as usize, nz as usize);
    let mut data = vec![0u8; nx * ny * 2 * nz * 3 * 4];
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                let cell = (z * ny + y) * nx + x;
                let shares = seen.sides[cell];
                let ground_lit = SHADED_GROUND + (1.0 - SHADED_GROUND) * seen.up[cell];
                for (side, &(sky, ground)) in sides.iter().enumerate() {
                    let share = shares[side];
                    let luma = sky.dot(Vec3::new(0.2126, 0.7152, 0.0722));
                    let light =
                        sky * share + LEAF_GLOW * luma * (1.0 - share) + ground * ground_lit;
                    let axis = side / 2;
                    let negative = side % 2 == 1;
                    let t = y + if negative { 0 } else { ny };
                    let p = z + axis * nz;
                    let at = ((p * ny * 2 + t) * nx + x) * 4;
                    data[at..at + 4].copy_from_slice(&rgb9e5(light).to_le_bytes());
                }
            }
        }
    }
    data
}

/// A colour packed as three nine-bit mantissas sharing a five-bit exponent, as the GPU's
/// RGB9E5 format holds it (the OpenGL EXT_texture_shared_exponent rules).
fn rgb9e5(c: Vec3) -> u32 {
    const MAX: f32 = 65408.0;
    let c = c.clamp(Vec3::ZERO, Vec3::splat(MAX));
    let largest = c.max_element();
    if largest < 1e-9 {
        return 0;
    }
    let mut exponent = (libm::floorf(libm::log2f(largest)) as i32).max(-16) + 1 + 15;
    let mut scale = libm::exp2f((exponent - 15 - 9) as f32);
    if libm::floorf(largest / scale + 0.5) as u32 == 512 {
        exponent += 1;
        scale *= 2.0;
    }
    let m = |v: f32| (libm::floorf(v / scale + 0.5) as u32).min(511);
    m(c.x) | (m(c.y) << 9) | (m(c.z) << 18) | ((exponent as u32) << 27)
}

/// Looks toward the sky from every cell, through the crowns' leaves.
fn seen(o: &SkyOccluders) -> Seen {
    let height = o.above_m + o.below_m;
    let across = (o.half_m * 2.0 / CELL_M).ceil().max(2.0) as u32;
    let cells = UVec3::new(across, (height / LAYER_M).ceil().max(2.0) as u32, across);
    let size = Vec3::new(o.half_m * 2.0, height, o.half_m * 2.0);
    let step = size / cells.as_vec3();
    let leaves = Leaves::of(o, cells, step);
    // Directions toward the sky, in the grid's frame, in rings from the horizon up, each
    // standing for an even share of the sky's dome.
    let mut looks: Vec<(Vec3, f32)> = Vec::new();
    for (height, count) in [
        (6.0f32, 16),
        (20.0, 16),
        (35.0, 12),
        (50.0, 10),
        (65.0, 6),
        (80.0, 4),
    ] {
        let up = height.to_radians();
        for k in 0..count {
            // Each ring turned a little from the last, so no two line up.
            let a = (k as f32 + height / 37.0) / count as f32 * std::f32::consts::TAU;
            let d = Vec3::new(
                libm::cosf(up) * libm::cosf(a),
                libm::sinf(up),
                libm::cosf(up) * libm::sinf(a),
            );
            // A ring's share of the dome, split among its looks.
            looks.push((d, libm::cosf(up) / count as f32));
        }
    }
    looks.push((Vec3::Y, 0.05));
    let south = o.east.cross(o.up);
    let axes = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z]
        .map(|axis| Vec3::new(axis.dot(o.east), axis.dot(o.up), axis.dot(south)));
    let count = (cells.x * cells.y * cells.z) as usize;
    let mut sides = vec![[1.0f32; 6]; count];
    let mut up = vec![1.0f32; count];
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let rows = cells.y * cells.z;
    let per = rows.div_ceil(threads as u32) as usize * cells.x as usize;
    std::thread::scope(|scope| {
        for (chunk, (sides, up)) in sides.chunks_mut(per).zip(up.chunks_mut(per)).enumerate() {
            let (leaves, looks, axes) = (&leaves, &looks, &axes);
            scope.spawn(move || {
                for (i, (side, straight)) in sides.iter_mut().zip(up.iter_mut()).enumerate() {
                    let cell = (chunk * per + i) as u32;
                    let x = cell % cells.x;
                    let y = cell / cells.x % cells.y;
                    let z = cell / (cells.x * cells.y);
                    let at = (UVec3::new(x, y, z).as_vec3() + 0.5) * step;
                    let through: Vec<f32> =
                        looks.iter().map(|(d, _)| leaves.through(at, *d)).collect();
                    for (k, axis) in axes.iter().enumerate() {
                        let (mut sum, mut weight) = (0.0, 0.0);
                        for ((d, w), t) in looks.iter().zip(&through) {
                            let lean = d.dot(*axis).max(0.0) * w;
                            sum += lean * t;
                            weight += lean;
                        }
                        side[k] = if weight > 0.0 { sum / weight } else { 1.0 };
                    }
                    *straight = side[2].min(1.0);
                }
            });
        }
    });
    Seen { cells, sides, up }
}

/// How thick the leaves are in each cell, per metre of a look through them.
struct Leaves {
    cells: UVec3,
    step: Vec3,
    thickness: Vec<f32>,
}

impl Leaves {
    fn of(o: &SkyOccluders, cells: UVec3, step: Vec3) -> Self {
        let south = o.east.cross(o.up);
        let corner = o.middle - o.east * o.half_m - south * o.half_m - o.up * o.below_m;
        let mut thickness = vec![0.0f32; (cells.x * cells.y * cells.z) as usize];
        for crown in &o.crowns {
            let rel = crown.middle - corner;
            let at = Vec3::new(rel.dot(o.east), rel.dot(o.up), rel.dot(south));
            let half = crown.half.max(Vec3::splat(0.5));
            // Thick enough that a look straight up through the middle is hidden as said.
            let per_m = -libm::logf((1.0 - crown.closes).clamp(0.01, 1.0)) / (2.0 * half.y);
            let low = ((at - half) / step).floor().max(Vec3::ZERO).as_uvec3();
            let high = ((at + half) / step).ceil().as_uvec3().min(cells - 1);
            for z in low.z..=high.z {
                for y in low.y..=high.y {
                    for x in low.x..=high.x {
                        // Sampled at a few points in the cell, so a crown smaller than a
                        // cell still counts for what it covers.
                        let mut inside = 0.0;
                        for corner in 0..8u32 {
                            let offset = Vec3::new(
                                (corner & 1) as f32,
                                (corner >> 1 & 1) as f32,
                                (corner >> 2 & 1) as f32,
                            ) * 0.5
                                + 0.25;
                            let p = (UVec3::new(x, y, z).as_vec3() + offset) * step;
                            let r = ((p - at) / half).length();
                            inside += 1.0 - smoothstep(0.75, 1.0, r);
                        }
                        let cell = ((z * cells.y + y) * cells.x + x) as usize;
                        thickness[cell] += per_m * inside / 8.0;
                    }
                }
            }
        }
        Self {
            cells,
            step,
            thickness,
        }
    }

    /// How much light comes from the sky along a look from `at`, in the grid's frame.
    fn through(&self, at: Vec3, d: Vec3) -> f32 {
        let size = self.cells.as_vec3() * self.step;
        let stride = self.step.min_element() * 0.5;
        let mut depth = 0.0;
        let mut travelled = stride * 0.5;
        while travelled < REACH_M {
            let p = at + d * travelled;
            if p.y >= size.y || p.x < 0.0 || p.z < 0.0 || p.x >= size.x || p.z >= size.z {
                break;
            }
            let c = (p / self.step).as_uvec3().min(self.cells - 1);
            depth +=
                self.thickness[((c.z * self.cells.y + c.y) * self.cells.x + c.x) as usize] * stride;
            if depth > 6.0 {
                return 0.0;
            }
            travelled += stride;
        }
        libm::expf(-depth)
    }
}

fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wood(crowns: Vec<Crown>) -> SkyOccluders {
        SkyOccluders {
            middle: Vec3::ZERO,
            up: Vec3::Y,
            east: Vec3::X,
            half_m: 40.0,
            below_m: 5.0,
            above_m: 35.0,
            crowns,
        }
    }

    fn cell_at(seen: &Seen, o: &SkyOccluders, p: Vec3) -> usize {
        let size = Vec3::new(o.half_m * 2.0, o.above_m + o.below_m, o.half_m * 2.0);
        let rel = p + Vec3::new(o.half_m, o.below_m, o.half_m);
        let c = (rel / (size / seen.cells.as_vec3())).as_uvec3();
        ((c.z * seen.cells.y + c.y) * seen.cells.x + c.x) as usize
    }

    #[test]
    fn open_ground_sees_the_whole_sky() {
        let o = wood(Vec::new());
        let seen = seen(&o);
        let cell = cell_at(&seen, &o, Vec3::new(3.0, 1.0, -7.0));
        assert!(
            seen.sides[cell].iter().all(|s| (s - 1.0).abs() < 1e-4),
            "{:?}",
            seen.sides[cell]
        );
    }

    #[test]
    fn under_a_closed_canopy_little_sky_is_seen_from_below_and_more_from_the_side() {
        // Crowns every ten metres over the middle of the square, each hiding nine tenths of
        // the sky straight up through it, their edges meeting.
        let mut crowns = Vec::new();
        for i in -2..=2 {
            for j in -2..=2 {
                crowns.push(Crown {
                    middle: Vec3::new(i as f32 * 10.0, 15.0, j as f32 * 10.0),
                    half: Vec3::new(7.0, 5.0, 7.0),
                    closes: 0.9,
                });
            }
        }
        let o = wood(crowns);
        let seen = seen(&o);
        let under = cell_at(&seen, &o, Vec3::new(0.0, 1.0, 0.0));
        let up = seen.sides[under][2];
        assert!(up < 0.4, "looking up under it sees {up} of the sky");
        let side = seen.sides[under][0];
        assert!(
            side > up,
            "a side sees the low sky under the crown's edge: {side} against {up}"
        );
        let above = cell_at(&seen, &o, Vec3::new(0.0, 30.0, 0.0));
        assert!(
            seen.sides[above][2] > 0.99,
            "above the crown the sky is open"
        );
        let away = cell_at(&seen, &o, Vec3::new(35.0, 1.0, 35.0));
        assert!(seen.sides[away][2] > seen.sides[under][2] + 0.3);
    }

    #[test]
    fn the_sky_overhead_is_closed_under_a_crown_and_open_above_it_and_outside() {
        // A canopy over the middle of the square, as in the test above.
        let mut crowns = Vec::new();
        for i in -2..=2 {
            for j in -2..=2 {
                crowns.push(Crown {
                    middle: Vec3::new(i as f32 * 10.0, 15.0, j as f32 * 10.0),
                    half: Vec3::new(7.0, 5.0, 7.0),
                    closes: 0.9,
                });
            }
        }
        let o = wood(crowns);
        let seen = seen(&o);
        let under = open_at(&o, &seen, Vec3::new(0.5, 1.7, -0.5));
        let above = open_at(&o, &seen, Vec3::new(0.5, 30.0, -0.5));
        let beside = open_at(&o, &seen, Vec3::new(36.0, 1.7, 36.0));
        assert!(under < 0.5, "under the crown {under}");
        assert!(above > 0.99, "above it {above}");
        assert!(beside > under + 0.3, "beside it {beside}");
        assert_eq!(open_at(&o, &seen, Vec3::new(90.0, 1.7, 0.0)), 1.0);
        // Between cells it changes smoothly, not in steps.
        let a = open_at(&o, &seen, Vec3::new(10.0, 1.7, 0.0));
        let b = open_at(&o, &seen, Vec3::new(10.5, 1.7, 0.0));
        assert!((a - b).abs() < 0.1, "{a} then {b}");
    }

    #[test]
    fn colours_pack_as_the_gpu_reads_them() {
        let unpack = |v: u32| {
            let e = (v >> 27) as i32 - 15 - 9;
            let s = libm::exp2f(e as f32);
            Vec3::new(
                (v & 511) as f32 * s,
                (v >> 9 & 511) as f32 * s,
                (v >> 18 & 511) as f32 * s,
            )
        };
        for c in [
            Vec3::new(0.5, 0.25, 0.125),
            Vec3::new(3.0, 0.01, 1.7),
            Vec3::new(0.0009, 0.0004, 0.0),
        ] {
            let back = unpack(rgb9e5(c));
            assert!(
                (back - c).abs().max_element() <= c.max_element() / 256.0,
                "{c} came back {back}"
            );
        }
        assert_eq!(rgb9e5(Vec3::ZERO), 0);
    }
}
