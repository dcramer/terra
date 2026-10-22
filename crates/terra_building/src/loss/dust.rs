//! Dust (D305): a puff where something breaks or lands hard, and a billow as big as a chunk
//! where one lands, soft lit sprites facing the view that billow out, rise and thin away.

use super::*;
use bevy::render::render_resource::AsBindGroup;

/// A puff of dust: how long it has been up, how wide it grows, and its own material, whose
/// alpha fades.
#[derive(Component)]
pub(super) struct Puff {
    age: f32,
    /// How long it lasts, in seconds.
    life: f32,
    wide: f32,
    rise: Vec3,
    material: Handle<DustMaterial>,
    alpha: f32,
    /// Whether it is the puff's first sprite, which fades the material they share.
    owner: bool,
    /// How far it is turned about the view, so the sprites of a billow are not one stamp,
    /// and how fast it turns, so it churns as dust rolls rather than hanging as a stamp.
    roll: f32,
    spin: f32,
    /// How fast it comes to rise as it ages, in metres a second: a collapse's cloud is
    /// warm and stirred, and lifts off the ground after it has rolled out.
    lift: f32,
    /// Where it lies in its billow, from the billow's middle, once grown: it goes out there
    /// as it grows ([`Sprite::lay`]).
    lay: Vec3,
}

/// The billow a chunk raises where it lands, as big as it: none for a piece under
/// [`BILLOW_FROM_M`] (its own small puff), else wider than it, lasting seconds, rolling out
/// low along the ground the way it was going (`along`) and every way round, rising slowly,
/// as a collapse's cloud does: the air a falling mass pushes out carries its dust as a
/// current heavier than the air round it, out along the ground at a few metres a second
/// and only later up (films of falling towers and demolitions; Battlefield's collapses).
pub(super) fn landing(size: f32, along: Vec3, up: Vec3) -> Option<Billow> {
    if size < BILLOW_FROM_M {
        return None;
    }
    let size = size.min(6.0);
    let flat = along - up * along.dot(up);
    Some(Billow {
        alpha: 1.0,
        // A big chunk's is wider than tall at its foot and climbs: as wide as 3.5 times
        // it, a section landing on the keep's roof raised a 20 m ball (castle film round 6).
        wide: 2.0 + size * 2.5,
        // A demolition's cloud hangs for many seconds as it settles.
        life: 6.0 + size * 2.0,
        drift: flat.normalize_or_zero() * (ROLLS_OUT + 0.4 * size).min(3.5)
            + up * (0.12 + 0.06 * size),
        stir: 1.0 + 0.3 * size,
        spread: size * 0.8,
        tall: (size - 1.5).max(0.0) * 2.0,
    })
}

/// From how many cubes a fall raises a surge ([`surge`]), how many billows a ring of it has
/// at most, and how many more it may have up beside the rest.
const SURGE_FROM: usize = 200;
const SURGE_RING: usize = 18;
/// Room for three big falls' surges up at once: with room for 90, the castle's two towers'
/// surges, hanging 15-25 s, left its keep's little (castle film round 5).
pub(super) const SURGES: usize = 160;
/// How far out past its foot's edge a surge's rings come up, in metres: from the middle of
/// the fall, a keep's surge came up inside its own standing ground storey and the castle's
/// walls, where nothing outside them saw it (castle film round 5).
const SURGE_OUT_M: f32 = 1.0;

/// The colour of the dust a big fall drives out along the ground: its stone's, warmed by the
/// earth and lime mortar it beats out of the ground and the heap, a buff tan as demolition
/// footage shows (castle film round 7: the keep's surge, in its stone's greyed colour and
/// in the keep's shadow, read as grey fire smoke).
pub(super) fn surge_tint(stone: Color) -> Color {
    let stone = stone.to_srgba();
    let [r, g, b] = SURGE_EARTH;
    let mix = |s: f32, e: f32| s * (1.0 - SURGE_EARTH_SHARE) + e * SURGE_EARTH_SHARE;
    Color::srgb(mix(stone.red, r), mix(stone.green, g), mix(stone.blue, b))
}

/// The earth's buff tan in a surge, and how much of its colour it is.
const SURGE_EARTH: [f32; 3] = [0.78, 0.66, 0.5];
const SURGE_EARTH_SHARE: f32 = 0.5;

/// How much darker a dust sprite is at its thickest than at its edge, where it shades itself:
/// at 0.42 a cloud's core was a mid grey whatever its colour, a fire's smoke beside the
/// black (castle film round 7); at 0.15, stacked up a rising column, each sprite's core
/// still read as a dark eye in a lighter ring (castle film r17). Its lighting shades it.
const SHADES: f32 = 0.0;
/// How much of a dust sprite, out from its middle to its edge, is its soft rim.
const RIM: f32 = 0.6;
/// How far a sprite's normals lean out to its edge, as a dome's, and up, to the sky: leant
/// out 0.7, the middle of every sprite faced the eye and so away from a sun behind the cloud,
/// a dark eye in each lit ring, a column of them a chain of beads (castle film r17). Real
/// dust is lit through, the sky's light from above on all of it; six-way lit sprites
/// (Unity's VFX Graph, Unreal's Niagara smoke) shade a cloud as a whole, not each puff round.
const DOME: f32 = 0.25;
const SKYWARD: f32 = 0.35;
/// How deep the fine noise eats into a sprite's rim, so its edge is torn and wispy rather
/// than a lumpy disc's (castle film r17: round dark-cored puffs).
const TORN: f32 = 0.7;

/// The surge a big fall raises as it lands: the air its mass drives out carries its dust
/// out along the ground every way round in a rolling wall, thick, several footprints wide,
/// which then rises over the heap and hangs there, lit by the sun, settling over many
/// seconds (films of demolitions; a falling tower's surge cloud). `cubes` is how much came
/// down, `wide` how wide its foot is, in metres, and `drop_s` how long its top takes to
/// land. Each billow with when it comes up, in seconds from now, and where, from the fall's
/// foot, on the level of the base (x and z) and up it (y).
pub(super) fn surge(cubes: usize, wide: f32, drop_s: f32) -> Vec<(f32, Vec3, Billow)> {
    if cubes < SURGE_FROM {
        return Vec::new();
    }
    // How big it is, 1 for a keep's few thousand cubes.
    let big = (cubes as f32 / 4000.0).sqrt().clamp(0.25, 1.2);
    let ring = ((SURGE_RING as f32 * big).round() as usize).clamp(6, SURGE_RING);
    let mut out = Vec::new();
    let half = wide * 0.5;
    // The first ring as the lowest of it lands, rolling out fast and low; a second, wider
    // and slower, as the top comes down onto the heap.
    for (wave, (when, out_m_s, grows, life)) in
        [(0.25, 3.2, 1.0, 14.0), (drop_s.max(0.6), 2.0, 1.35, 18.0)]
            .into_iter()
            .enumerate()
    {
        for k in 0..ring {
            let turn = (k as f32 + 0.5 * wave as f32) / ring as f32 * core::f32::consts::TAU;
            let way = Vec3::new(libm::cosf(turn), 0.0, libm::sinf(turn));
            out.push((
                when + 0.04 * k as f32 / ring as f32,
                way * (half + SURGE_OUT_M) + Vec3::Y * (0.6 + 0.4 * wave as f32),
                Billow {
                    alpha: 1.0,
                    wide: (4.0 + 7.0 * big) * grows,
                    life: life * (0.8 + 0.4 * big),
                    drift: way * out_m_s * (0.7 + 0.5 * big) + Vec3::Y * 0.25,
                    stir: 0.8,
                    spread: 1.5 * big,
                    tall: 0.0,
                },
            ));
        }
    }
    // Over the heap, thin columns rising fast from where its sections land, lit, leaning
    // with the air, the last to settle: one fat column as wide as the heap read as a
    // cotton ball (castle film round 6), where a demolition's dust goes up in plumes a few
    // metres across, taller than wide, and the surge rolls out under them.
    let columns = 3 + (4.0 * big) as usize;
    for k in 0..columns {
        let turn = (k as f32 * 2.4 + 0.7) % core::f32::consts::TAU;
        let off = Vec3::new(libm::cosf(turn), 0.0, libm::sinf(turn))
            * half
            * (0.25 + 0.5 * ((k * 7 % 5) as f32 / 4.0));
        out.push((
            drop_s * (0.5 + 0.15 * k as f32),
            off + Vec3::Y * 1.5,
            Billow {
                alpha: 1.0,
                wide: COLUMN_M * (0.8 + 0.6 * big),
                life: 16.0 * (0.8 + 0.3 * big),
                drift: off.normalize_or_zero() * 0.3 + Vec3::Y * (1.4 + 0.3 * (k % 3) as f32),
                stir: 0.25,
                spread: 0.6,
                tall: COLUMN_TALL_M * (0.7 + 0.5 * big),
            },
        ));
    }
    out
}

/// Sets the surge of what `fell` round `middle` in its base, at `frame`, to come up as it
/// lands ([`surge`]): from its foot, as wide as it.
pub(super) fn plan_surge(
    shards: &mut Shards,
    colour: Color,
    frame: &Transform,
    middle: Vec3,
    ground: f32,
    fell: &[(IVec3, Cube)],
) {
    let (low, high) = fell.iter().fold(
        (IVec3::splat(i32::MAX), IVec3::splat(i32::MIN)),
        |(low, high), (at, _)| (low.min(*at), high.max(*at)),
    );
    let span = (high - low + IVec3::ONE).as_vec3() * CELL_M;
    // On the ground it rolls out over, not at the lowest course that fell: the keep's top
    // came down off walls left standing, and its surge rose at their height as a ball over
    // the castle (castle film round 3).
    let foot = Vec3::new(middle.x, ground.min(low.y as f32 * CELL_M), middle.z);
    let drop_s = libm::sqrtf(2.0 * ((high.y + 1) as f32 * CELL_M - foot.y) / fragments::GRAVITY);
    let now = shards.clock_s;
    for (after, from, mut billow) in surge(fell.len(), span.x.max(span.z), drop_s) {
        let at = frame.transform_point(foot + from);
        billow.drift = frame.rotation * billow.drift;
        let up = at.normalize_or(Vec3::Y);
        shards
            .surges
            .push((now + after, at, surge_tint(colour), up, billow));
    }
}

/// Raises the surges' billows whose time has come, or all of them once nothing falls to
/// keep the fragments' clock, within `most` puffs up at once; says how many it refused.
pub(super) fn raise_surges(
    commands: &mut Commands,
    shards: &mut Shards,
    (meshes, materials, images): (
        &mut Assets<Mesh>,
        &mut Assets<DustMaterial>,
        &mut Assets<Image>,
    ),
    most: usize,
) {
    if shards.surges.is_empty() {
        return;
    }
    let now = shards.clock_s;
    let idle = shards.live.is_empty();
    let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut shards.surges)
        .into_iter()
        .partition(|(when, ..)| idle || *when <= now);
    shards.surges = later;
    let (due_count, mut refused) = (due.len(), 0);
    for (_, at, colour, up, billow) in due {
        let assets = (&mut *meshes, &mut *materials, &mut *images);
        refused += usize::from(!billow_at(
            commands, shards, assets, most, at, colour, up, billow,
        ));
    }
    if refused > 0 {
        info!("Build: dust: {refused} of {due_count} surge billows refused, the budget full");
    }
}

/// How fast, in metres a second, a billow rolls out along the ground at least.
const ROLLS_OUT: f32 = 1.8;

/// How long, in seconds, a billow takes in the chunks landing near it, before it has
/// drifted and thinned enough that a new one comes up there.
const JOINS_S: f32 = 1.5;

/// The billows raised lately: where, how wide they grow, in metres, when, and how many
/// chunks have landed in each since. A chunk landing within half a young billow's width
/// joins it rather than raising its own, as a collapse's dust is a few big clouds, not one
/// per stone (the emitter per area of shipped games' destruction, not one per piece); but
/// every [`FEEDS`] chunks landing in one raise another there, so the cloud grows with the
/// mass coming down, as a demolition's does, rather than one billow for a whole keep.
#[derive(Default)]
pub(super) struct Raised(Vec<(Vec3, f32, f32, u32)>);

/// How many chunks landing in a young billow feed it before the next raises another.
pub(super) const FEEDS: u32 = 4;

impl Raised {
    /// Whether a chunk landing at `at` at `now` seconds joins a billow raised near it,
    /// feeding it; one that would overfeed it raises its own.
    pub(super) fn joins(&mut self, at: Vec3, now: f32) -> bool {
        self.0.retain(|&(_, _, when, _)| now - when < JOINS_S);
        let Some(near) = self
            .0
            .iter_mut()
            .find(|(there, wide, _, _)| there.distance(at) < *wide * 0.5)
        else {
            return false;
        };
        near.3 += 1;
        if near.3 > FEEDS {
            near.3 = 0;
            return false;
        }
        true
    }

    pub(super) fn add(&mut self, at: Vec3, wide: f32, now: f32) {
        self.0.push((at, wide, now, 0));
    }
}

/// A dust sprite's colour, alpha and normals, made once: a lumpy billow whose edge is
/// uneven (a few octaves of value noise, as smoke and dust sprites are painted), thick and
/// darker in its core where it shades itself, thin and lighter at its edge; its normals
/// those of a round, lumpy billow, so the sun lights its sunward side and its lumps.
pub(super) fn dust_images() -> (Image, Image) {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    const SIDE: u32 = 128;
    let lattice = |x: i32, y: i32, octave: u32| {
        let h = terra_gen::hash::prospector32_round(
            (x as u32).wrapping_mul(0x9E37_79B9)
                ^ (y as u32).wrapping_mul(0x85EB_CA77)
                ^ octave.wrapping_mul(0xC2B2_AE3D),
        );
        ((h >> 8) & 0xFFFF) as f32 / 65535.0
    };
    // Value noise, smooth between its lattice points, a few octaves.
    let noise = |u: f32, v: f32| {
        let mut sum = 0.0;
        let mut weight = 0.0;
        for octave in 0..4u32 {
            let scale = 3i32 << octave;
            let (x, y) = (u * scale as f32, v * scale as f32);
            let (x0, y0) = (x.floor(), y.floor());
            let (fx, fy) = (x - x0, y - y0);
            let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
            let at = |dx: i32, dy: i32| lattice(x0 as i32 + dx, y0 as i32 + dy, octave);
            let top = at(0, 0) + (at(1, 0) - at(0, 0)) * sx;
            let bottom = at(0, 1) + (at(1, 1) - at(0, 1)) * sx;
            let w = 1.0 / (1u32 << octave) as f32;
            sum += (top + (bottom - top) * sy) * w;
            weight += w;
        }
        sum / weight
    };
    let density = |x: f32, y: f32| {
        let (u, v) = (x / SIDE as f32, y / SIDE as f32);
        let (cu, cv) = (u * 2.0 - 1.0, v * 2.0 - 1.0);
        let r = (cu * cu + cv * cv).sqrt();
        let n = noise(u, v);
        // Its edge in and out with the noise: lumps, not a disc.
        let edge = 0.55 + 0.45 * n;
        // Thick over its inner part, soft only over its outer lumpy rim: a sprite thinning all
        // the way from its middle overlapped into a see-through veil (castle film round 7).
        let fall = ((edge - r) / (edge * RIM)).clamp(0.0, 1.0);
        let body = fall * fall * (3.0 - 2.0 * fall);
        // The rim torn by finer noise, deepest at the edge and none in the core.
        let fine = noise(u * 2.3 + 0.37, v * 2.3 + 0.71);
        let torn = (1.0 - body) * TORN * (1.0 - fine);
        ((body - torn) * (0.7 + 0.5 * n)).clamp(0.0, 1.0)
    };
    let mut colour = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    let mut normals = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0) as u8;
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let d = density(fx, fy);
            // Darker where thick: its core shades itself.
            let shade = 1.0 - SHADES * d;
            colour.extend([byte(shade), byte(shade), byte(shade), byte(d)]);
            // A dome over the sprite and the slope of its lumps; rows run down the image,
            // its up is up the sprite.
            let (cu, cv) = (fx / SIDE as f32 * 2.0 - 1.0, fy / SIDE as f32 * 2.0 - 1.0);
            let slope = Vec2::new(
                density(fx + 1.5, fy) - density(fx - 1.5, fy),
                -(density(fx, fy + 1.5) - density(fx, fy - 1.5)),
            ) * -6.0;
            let side = Vec2::new(cu, -cv) * DOME + slope + Vec2::new(0.0, SKYWARD);
            let n = side
                .extend((1.0 - side.length_squared().min(0.95)).sqrt())
                .normalize();
            normals.extend([
                byte(n.x * 0.5 + 0.5),
                byte(n.y * 0.5 + 0.5),
                byte(n.z * 0.5 + 0.5),
                255,
            ]);
        }
    }
    let image = |data: Vec<u8>, format: TextureFormat| {
        Image::new(
            Extent3d {
                width: SIDE,
                height: SIDE,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            format,
            RenderAssetUsages::RENDER_WORLD,
        )
    };
    (
        image(colour, TextureFormat::Rgba8UnormSrgb),
        image(normals, TextureFormat::Rgba8Unorm),
    )
}

/// How many corners a dust sprite's cutout has round its rim.
pub(super) const CUTOUT_CORNERS: usize = 16;

/// A dust sprite's mesh: a fan fitted round what of its image shows (alpha over nothing), not
/// the square it is drawn in, after Unreal's particle cutouts (bounding geometry fitted to a
/// sprite's alpha, which cuts translucent overdraw; docs only). Every pixel of a lit sprite
/// pays the whole of the sun's, its shadows' and the sky's light, shown or not, and a
/// castle's fall layers some two thousand of them: its transparent pass climbed to 23 ms of
/// the frame, 4 of them with the dust unlit (castle bench, round 15). The square's corners and
/// the rim past each lump are about half of it.
///
/// Each corner is as far out as the farthest shown texel of the two wedges beside it, over the
/// cosine of half a wedge, so the fan holds every one of them. 1 m across, as the square was,
/// facing +z, its uvs the square's.
pub(super) fn cutout(image: &Image) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    let side = image.width() as usize;
    let data = image.data.as_ref().expect("made on the CPU");
    let wedge = core::f32::consts::TAU / CUTOUT_CORNERS as f32;
    let mut farthest = [0.0f32; CUTOUT_CORNERS];
    for y in 0..side {
        for x in 0..side {
            if data[(y * side + x) * 4 + 3] == 0 {
                continue;
            }
            // Each corner of the texel, as the sprite's x right and y up, -1 to 1.
            for (cx, cy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let u = (x + cx) as f32 / side as f32 * 2.0 - 1.0;
                let v = 1.0 - (y + cy) as f32 / side as f32 * 2.0;
                let a = libm::atan2f(v, u).rem_euclid(core::f32::consts::TAU);
                let w = ((a / wedge) as usize).min(CUTOUT_CORNERS - 1);
                farthest[w] = farthest[w].max((u * u + v * v).sqrt());
            }
        }
    }
    let mut positions = vec![[0.0f32; 3]];
    let mut uvs = vec![[0.5f32, 0.5]];
    for i in 0..CUTOUT_CORNERS {
        let a = i as f32 * wedge;
        let (sin, cos) = (libm::sinf(a), libm::cosf(a));
        let r = farthest[i].max(farthest[(i + CUTOUT_CORNERS - 1) % CUTOUT_CORNERS])
            / libm::cosf(wedge * 0.5);
        // Within the square: nothing shows past its edge.
        let r = r.min(1.0 / cos.abs().max(sin.abs()));
        let (x, y) = (cos * r * 0.5, sin * r * 0.5);
        positions.push([x, y, 0.0]);
        uvs.push([x + 0.5, 0.5 - y]);
    }
    let indices: Vec<u32> = (0..CUTOUT_CORNERS as u32)
        .flat_map(|i| [0, 1 + i, 1 + (i + 1) % CUTOUT_CORNERS as u32])
        .collect();
    let corners = positions.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; corners])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
    .with_generated_tangents()
    .expect("a fan has normals and uvs")
}

/// A puff of dust at a place in the world, of a colour, as big as `amount` says: a few soft
/// sprites facing the view, which billow out, rise a little and thin away.
#[allow(clippy::too_many_arguments)]
pub(super) fn dust_at(
    commands: &mut Commands,
    shards: &mut Shards,
    assets: (
        &mut Assets<Mesh>,
        &mut Assets<DustMaterial>,
        &mut Assets<Image>,
    ),
    most: usize,
    at: Vec3,
    colour: Color,
    amount: f32,
    up: Vec3,
) {
    if amount <= 0.0 {
        return;
    }
    let billow = Billow {
        alpha: (0.6 * amount).clamp(0.3, 0.8),
        wide: PUFF_M[0] + (PUFF_M[1] - PUFF_M[0]) * (amount / 1.4).min(1.0),
        life: PUFF_S,
        drift: Vec3::ZERO,
        stir: 1.0,
        spread: 0.0,
        tall: 0.0,
    };
    billow_at(commands, shards, assets, most, at, colour, up, billow);
}

/// How a puff billows: how thick it is at first, how wide it grows, in metres, how long it
/// lasts, in seconds, the way it drifts as a whole and how much each sprite wanders and
/// rises, in metres a second.
pub(super) struct Billow {
    pub(super) alpha: f32,
    pub(super) wide: f32,
    pub(super) life: f32,
    pub(super) drift: Vec3,
    pub(super) stir: f32,
    /// How far apart its sprites start, in metres.
    pub(super) spread: f32,
    /// How far up its sprites are stacked, in metres, those higher rising faster, so it
    /// goes up as a plume taller than wide: 0 for a billow.
    pub(super) tall: f32,
}

/// A rising column's sprites' width and how tall it is stacked, in metres.
const COLUMN_M: f32 = 4.0;
const COLUMN_TALL_M: f32 = 9.0;

/// How long a billow takes to come to most of its width, in seconds, and what share of it
/// that is: a surge rolls out a wall's height in a second or two, then grows slowly as it
/// hangs. Grown by the share of its life alone, a surge lasting 18 s was a tenth of its
/// width after a second and a fifth after two, so the keep's surge never showed over its
/// own ground storey (castle film round 6).
const GROWS_S: f32 = 0.9;
const GROWS_EARLY: f32 = 0.75;

/// How much of its width a billow has grown to, from 0 to 1, `age` seconds into a `life`.
pub(super) fn grown(age: f32, life: f32) -> f32 {
    let through = (age / life.max(0.01)).clamp(0.0, 1.0);
    let by_life = 1.0 - (1.0 - through).powi(3);
    let by_time = GROWS_EARLY * (1.0 - libm::expf(-age.max(0.0) / GROWS_S));
    by_life.max(by_time)
}

/// A dust sprite's look: lit as Bevy's standard material is, and faded out where it comes
/// within [`DustFade::soft`] of what stands behind it (`shaders/dust.wesl`), as shipped games'
/// soft particles are (Unreal's DepthFade, Unity's soft particles): drawn as plain sprites, a
/// breach's dust cut each wall it stood in along a hard straight line, the wall's blocks
/// showing through the slab above it (castle film r17, 24.5-25.5 s).
pub(crate) type DustMaterial = bevy::pbr::ExtendedMaterial<StandardMaterial, DustFade>;

/// What `dust.wesl` is told. Field order must match it.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub(crate) struct DustFade {
    /// How far behind a sprite what is drawn there must stand for the sprite to show
    /// whole, in metres (x); the rest unused.
    #[uniform(100)]
    soft: Vec4,
}

impl bevy::pbr::MaterialExtension for DustFade {
    fn fragment_shader() -> bevy::shader::ShaderRef {
        "shaders/dust.wesl".into()
    }
}

/// How deep a billow `wide` metres across fades into what it meets: about a third of its
/// width, so a puff off a broken stone still shows in front of the wall it broke from and a
/// surge's sprites, metres wide, melt into the walls they stand in.
pub(super) fn soft_m(wide: f32) -> f32 {
    (wide * 0.35).clamp(0.25, 2.0)
}

/// A puff of dust billowing at a place, of a colour: five soft sprites facing the view.
/// Whether it was raised, which it is not once `most` are up.
#[allow(clippy::too_many_arguments)]
pub(super) fn billow_at(
    commands: &mut Commands,
    shards: &mut Shards,
    (meshes, materials, images): (
        &mut Assets<Mesh>,
        &mut Assets<DustMaterial>,
        &mut Assets<Image>,
    ),
    most: usize,
    at: Vec3,
    colour: Color,
    up: Vec3,
    billow: Billow,
) -> bool {
    if shards.puffs >= most {
        return false;
    }
    shards.puffs += 1;
    let quad = shards
        .puff
        .get_or_insert_with(|| meshes.add(cutout(&dust_images().0)))
        .clone();
    if shards.dust.is_none() || shards.dust_normals.is_none() {
        let (image, normals) = dust_images();
        shards.dust = Some(images.add(image));
        shards.dust_normals = Some(images.add(normals));
    }
    let (Some(image), Some(normals)) = (shards.dust.clone(), shards.dust_normals.clone()) else {
        return false;
    };
    // Thinner each the more there are, so the cloud is as thick as five were at its core
    // and soft at its edges.
    let alpha = billow.alpha * if billow.wide > 3.0 { 0.9 } else { 1.0 };
    let linear = colour.to_linear();
    // Lit, as smoke and dust are in shipped games (Unreal's lit translucency, normal-mapped
    // smoke sprites): the sun lights its sunward side, its far side and what stands in the
    // castle's shadow go dark, and it takes the fire's light; drawn unlit it was a pale
    // flat disc that vanished against the walls.
    let material = materials.add(DustMaterial {
        base: StandardMaterial {
            base_color: Color::LinearRgba(linear).with_alpha(alpha),
            base_color_texture: Some(image),
            normal_map_texture: Some(normals),
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            // Lit through as well as on its face: a dust cloud with the sun behind it glows at
            // its edges rather than going black (as the impacts' spray is).
            diffuse_transmission: 0.8,
            alpha_mode: AlphaMode::Blend,
            ..default()
        },
        extension: DustFade {
            soft: Vec4::new(soft_m(billow.wide), 0.0, 0.0, 0.0),
        },
    });
    let seed = shards.puffs as u32 ^ (at.x.to_bits() >> 8) ^ (at.z.to_bits() >> 4);
    for (index, sprite) in layout(&billow, up, seed).into_iter().enumerate() {
        commands.spawn((
            Name::new("Dust"),
            Puff {
                age: 0.0,
                life: billow.life,
                wide: sprite.wide,
                rise: sprite.drift,
                material: material.clone(),
                alpha,
                owner: index == 0,
                roll: sprite.roll,
                spin: sprite.spin,
                lift: sprite.lift,
                lay: sprite.lay,
            },
            Mesh3d(quad.clone()),
            MeshMaterial3d(material.clone()),
            NotShadowCaster,
            Transform::from_translation(at + sprite.drift * 0.2)
                .with_scale(Vec3::splat(sprite_wide(sprite.wide, 0.0))),
        ));
    }
    true
}

/// A sprite of a billow as it comes up: where it lies in the billow once grown, from its
/// middle, how it drifts, how wide it grows, how it is turned and turns, and how fast it
/// comes to rise.
pub(super) struct Sprite {
    pub(super) lay: Vec3,
    pub(super) drift: Vec3,
    pub(super) wide: f32,
    roll: f32,
    spin: f32,
    lift: f32,
}

/// How many seconds of its stir a sprite goes out from its billow's middle as it grows.
const STIRRED_S: f32 = 2.0;

/// A billow's sprites, `up` the way up where it is, `seed` its luck. Each comes up at the
/// billow's middle and goes out to its place in it ([`Sprite::lay`]) as it grows, so the
/// billow is one cloud from its first frame: laid out at once, a dozen sprites a third of a
/// metre wide, a column's strung ten metres up, came up as rows of dark round discs over the
/// keep's breach until they grew into one (castle film r16, 24-26.4 s), as shipped games'
/// dust and smoke sprites spawn at their emitter and spread with the cloud.
pub(super) fn layout(billow: &Billow, up: Vec3, seed: u32) -> Vec<Sprite> {
    // Many thin layers for a big billow rather than five thick stamps: five sprites as wide
    // as a surge read as a cotton ball and a flat band (castle film round 4); a dozen of
    // uneven size, thinner each, overlap into a lumpy cloud with depth, as shipped games
    // layer their smoke.
    // A plume's are more, one for every metre and a bit up it, so its stack stays one column.
    let sprites = (5.0 + billow.wide * 0.7).min(12.0) as u32 + (billow.tall * 0.8) as u32;
    let big = billow.wide > 3.0;
    (0..sprites)
        .map(|sprite| {
            let r = |salt: u32| {
                let h = terra_gen::hash::prospector32_round(
                    seed ^ sprite.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA77),
                );
                ((h >> 8) & 0xFFFF) as f32 / 65535.0
            };
            // A plume's sprites stacked up it, each the faster rising the higher, so it
            // climbs and stretches rather than swelling as a ball.
            let stacked = billow.tall * (sprite as f32 + r(8)) / sprites as f32;
            // A billow's sprites stirred out from its middle as far as their stir carries
            // them in [`STIRRED_S`], going out there as they grow: thrown out at that speed
            // from the first frame, a ring's sprites outran their own growth and stood alone
            // after two seconds.
            let stirred = Vec3::new(r(1) - 0.5, 0.0, r(2) - 0.5) * 2.4 * billow.stir * STIRRED_S;
            let drift =
                billow.drift + up * (0.08 + 0.15 * r(3)) * billow.stir + up * stacked * 0.15;
            let size = if big {
                0.4 + 0.65 * r(4)
            } else {
                0.6 + 0.6 * r(4)
            };
            // The smaller lumps lie in its core and the bigger out to its edge, as far out
            // as they are big: a small one stirred as far as a big one stood alone on the sky.
            let out = size / if big { 1.05 } else { 1.2 };
            Sprite {
                // Spread over the billow in lumps, some higher.
                lay: ((Vec3::new(r(6) - 0.5, 0.0, r(7) - 0.5) * 2.0
                    + up * r(8) * if big { 0.9 } else { 0.3 })
                    * billow.spread
                    + stirred)
                    * out
                    + up * stacked,
                drift,
                wide: billow.wide * size,
                roll: r(5) * core::f32::consts::TAU,
                spin: (r(9) - 0.5) * 0.5,
                lift: if big { 0.25 + 0.5 * r(10) } else { 0.0 },
            }
        })
        .collect()
}

/// How wide a sprite of a billow is drawn, `grow` of the way to its full `wide`: from a
/// quarter of it (a small puff's from a third of a metre), as a big chunk's dust bursts
/// out a good share of its size at once.
pub(super) fn sprite_wide(wide: f32, grow: f32) -> f32 {
    let first = (wide * 0.25).max(PUFF_M[0] * 0.5).min(wide);
    first + (wide - first) * grow
}

/// Dust billows out, rises and thins away, each sprite facing the view.
pub(super) fn puff(
    mut commands: Commands,
    time: Res<Time>,
    mut shards: ResMut<Shards>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    mut puffs: Query<(Entity, &mut Puff, &mut Transform)>,
    mut materials: ResMut<Assets<DustMaterial>>,
) {
    let eye = camera.map(|camera| camera.translation());
    for (entity, mut puff, mut transform) in &mut puffs {
        puff.age += time.delta_secs();
        let through = (puff.age / puff.life).min(1.0);
        if through >= 1.0 {
            commands.entity(entity).despawn();
            if puff.owner {
                materials.remove(&puff.material);
                shards.puffs = shards.puffs.saturating_sub(1);
            }
            continue;
        }
        // Out fast as it is thrown up, then slowly.
        let grow = grown(puff.age, puff.life);
        let grew = grow - grown(puff.age - time.delta_secs(), puff.life);
        transform.scale = Vec3::splat(sprite_wide(puff.wide, grow));
        // Out to its place in the billow as it grows.
        transform.translation += puff.lay * grew;
        let up = transform.translation.normalize_or(Vec3::Y);
        // Its middle rises as it grows, so it billows up off the ground rather than being
        // cut flat along it, and a big one lifts more as it ages.
        transform.translation += puff.rise * time.delta_secs() * (1.0 - through)
            + up * (grew * puff.wide * 0.3 + puff.lift * through * time.delta_secs());
        puff.roll += puff.spin * time.delta_secs();
        if let Some(eye) = eye {
            let at = transform.translation;
            let up = at.normalize_or(Vec3::Y);
            transform.look_at(at * 2.0 - eye, up);
            transform.rotate_local_z(puff.roll);
        }
        if puff.owner
            && let Some(mut material) = materials.get_mut(&puff.material)
        {
            // Thick at once, staying thick most of its life, then thinning away.
            let rise = (puff.age / 0.3).min(1.0);
            let left = 1.0 - through * through;
            let alpha = puff.alpha * rise * left;
            material.base.base_color = material.base.base_color.with_alpha(alpha);
        }
    }
}
