//! Dust (D305): a puff where something breaks or lands hard, and a billow as big as a chunk
//! where one lands, soft lit sprites facing the view that billow out, rise and thin away.

use super::*;

/// A puff of dust: how long it has been up, how wide it grows, and its own material, whose
/// alpha fades.
#[derive(Component)]
pub(super) struct Puff {
    age: f32,
    /// How long it lasts, in seconds.
    life: f32,
    wide: f32,
    rise: Vec3,
    material: Handle<StandardMaterial>,
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
        wide: 2.0 + size * 3.5,
        // A demolition's cloud hangs for many seconds as it settles.
        life: 6.0 + size * 2.0,
        drift: flat.normalize_or_zero() * (ROLLS_OUT + 0.4 * size).min(3.5)
            + up * (0.12 + 0.06 * size),
        stir: 1.0 + 0.3 * size,
        spread: size * 0.8,
    })
}

/// From how many cubes a fall raises a surge ([`surge`]), how many billows a ring of it has
/// at most, and how many more it may have up beside the rest.
const SURGE_FROM: usize = 200;
const SURGE_RING: usize = 18;
pub(super) const SURGES: usize = 90;

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
                way * half * 0.8 + Vec3::Y * (0.6 + 0.4 * wave as f32),
                Billow {
                    alpha: 0.9,
                    wide: (4.0 + 7.0 * big) * grows,
                    life: life * (0.8 + 0.4 * big),
                    drift: way * out_m_s * (0.7 + 0.5 * big) + Vec3::Y * 0.25,
                    stir: 0.8,
                    spread: 1.5 * big,
                },
            ));
        }
    }
    // Over the heap, a column rising slowly and spreading, the last to settle.
    for k in 0..(2 + (4.0 * big) as usize) {
        out.push((
            drop_s * (0.6 + 0.2 * k as f32),
            Vec3::Y * (1.5 + 2.0 * k as f32),
            Billow {
                alpha: 0.85,
                wide: (6.0 + 8.0 * big) * (1.0 + 0.15 * k as f32),
                life: 20.0 * (0.8 + 0.3 * big),
                drift: Vec3::Y * (0.5 + 0.2 * k as f32),
                stir: 0.6,
                spread: 2.0 * big,
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
        shards.surges.push((now + after, at, colour, up, billow));
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
fn dust_images() -> (Image, Image) {
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
        let fall = ((edge - r) / edge).clamp(0.0, 1.0);
        (fall * fall * (3.0 - 2.0 * fall) * (0.45 + 0.75 * n)).clamp(0.0, 1.0)
    };
    let mut colour = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    let mut normals = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0) as u8;
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let d = density(fx, fy);
            // Darker where thick: its core shades itself.
            let shade = 1.0 - 0.42 * d;
            colour.extend([byte(shade), byte(shade), byte(shade), byte(d)]);
            // A dome over the sprite and the slope of its lumps; rows run down the image,
            // its up is up the sprite.
            let (cu, cv) = (fx / SIDE as f32 * 2.0 - 1.0, fy / SIDE as f32 * 2.0 - 1.0);
            let slope = Vec2::new(
                density(fx + 1.5, fy) - density(fx - 1.5, fy),
                -(density(fx, fy + 1.5) - density(fx, fy - 1.5)),
            ) * -6.0;
            let side = Vec2::new(cu, -cv) * 0.7 + slope;
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

/// A puff of dust at a place in the world, of a colour, as big as `amount` says: a few soft
/// sprites facing the view, which billow out, rise a little and thin away.
#[allow(clippy::too_many_arguments)]
pub(super) fn dust_at(
    commands: &mut Commands,
    shards: &mut Shards,
    assets: (
        &mut Assets<Mesh>,
        &mut Assets<StandardMaterial>,
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
}

/// A puff of dust billowing at a place, of a colour: five soft sprites facing the view.
/// Whether it was raised, which it is not once `most` are up.
#[allow(clippy::too_many_arguments)]
pub(super) fn billow_at(
    commands: &mut Commands,
    shards: &mut Shards,
    (meshes, materials, images): (
        &mut Assets<Mesh>,
        &mut Assets<StandardMaterial>,
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
        .get_or_insert_with(|| {
            meshes.add(
                Mesh::from(Rectangle::new(1.0, 1.0))
                    .with_generated_tangents()
                    .expect("a rectangle has normals and uvs"),
            )
        })
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
    let alpha = billow.alpha * if billow.wide > 3.0 { 0.75 } else { 1.0 };
    let linear = colour.to_linear();
    // Lit, as smoke and dust are in shipped games (Unreal's lit translucency, normal-mapped
    // smoke sprites): the sun lights its sunward side, its far side and what stands in the
    // castle's shadow go dark, and it takes the fire's light; drawn unlit it was a pale
    // flat disc that vanished against the walls.
    let material = materials.add(StandardMaterial {
        base_color: Color::LinearRgba(linear).with_alpha(alpha),
        base_color_texture: Some(image),
        normal_map_texture: Some(normals),
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let seed = shards.puffs as u32 ^ (at.x.to_bits() >> 8) ^ (at.z.to_bits() >> 4);
    // Many thin layers for a big billow rather than five thick stamps: five sprites as wide
    // as a surge read as a cotton ball and a flat band (castle film round 4); a dozen of
    // uneven size, thinner each, overlap into a lumpy cloud with depth, as shipped games
    // layer their smoke.
    let sprites = (5.0 + billow.wide * 0.7).min(12.0) as u32;
    let big = billow.wide > 3.0;
    for sprite in 0..sprites {
        let r = |salt: u32| {
            let h = terra_gen::hash::prospector32_round(
                seed ^ sprite.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA77),
            );
            ((h >> 8) & 0xFFFF) as f32 / 65535.0
        };
        let drift = billow.drift
            + (Vec3::new(r(1) - 0.5, 0.0, r(2) - 0.5) * 2.4 + up * (0.08 + 0.15 * r(3)))
                * billow.stir;
        commands.spawn((
            Name::new("Dust"),
            Puff {
                age: 0.0,
                life: billow.life,
                wide: billow.wide
                    * if big {
                        0.4 + 0.65 * r(4)
                    } else {
                        0.6 + 0.6 * r(4)
                    },
                rise: drift,
                material: material.clone(),
                alpha,
                owner: sprite == 0,
                roll: r(5) * core::f32::consts::TAU,
                spin: (r(9) - 0.5) * 0.5,
                lift: if big { 0.25 + 0.5 * r(10) } else { 0.0 },
            },
            Mesh3d(quad.clone()),
            MeshMaterial3d(material.clone()),
            NotShadowCaster,
            // Spread over the billow in lumps, some higher.
            Transform::from_translation(
                at + drift * 0.2
                    + (Vec3::new(r(6) - 0.5, 0.0, r(7) - 0.5) * 2.0
                        + up * r(8) * if big { 0.9 } else { 0.3 })
                        * billow.spread,
            )
            .with_scale(Vec3::splat(PUFF_M[0] * 0.5)),
        ));
    }
    true
}

/// Dust billows out, rises and thins away, each sprite facing the view.
pub(super) fn puff(
    mut commands: Commands,
    time: Res<Time>,
    mut shards: ResMut<Shards>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    mut puffs: Query<(Entity, &mut Puff, &mut Transform)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
        let grow = 1.0 - (1.0 - through).powi(3);
        let before = (puff.age - time.delta_secs()).max(0.0) / puff.life;
        let grew = grow - (1.0 - (1.0 - before.min(1.0)).powi(3));
        transform.scale = Vec3::splat(PUFF_M[0] * 0.5 + (puff.wide - PUFF_M[0] * 0.5) * grow);
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
            material.base_color = material.base_color.with_alpha(alpha);
        }
    }
}
