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
    /// How far it is turned about the view, so the sprites of a billow are not one stamp.
    roll: f32,
}

/// The billow a chunk raises where it lands, as big as it: none for a piece under
/// [`BILLOW_FROM_M`] (its own small puff), else wider than it, lasting seconds, rolling on
/// the way it was going (`along`) and up, as a collapse's cloud comes up from where its
/// masses land (Battlefield's and films' falling towers), in lumps, not all at once.
pub(super) fn landing(size: f32, along: Vec3, up: Vec3) -> Option<Billow> {
    if size < BILLOW_FROM_M {
        return None;
    }
    let size = size.min(6.0);
    let flat = along - up * along.dot(up);
    Some(Billow {
        alpha: 1.0,
        wide: 2.0 + size * 3.5,
        life: 4.5 + size * 1.2,
        drift: flat.clamp_length_max(4.0) * 0.4 + up * (0.6 + 0.35 * size),
        stir: 1.0 + 0.3 * size,
        spread: size * 0.8,
    })
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
) {
    if shards.puffs >= most {
        return;
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
        return;
    };
    let alpha = billow.alpha;
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
    for sprite in 0..5u32 {
        let r = |salt: u32| {
            let h = terra_gen::hash::prospector32_round(
                seed ^ sprite.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA77),
            );
            ((h >> 8) & 0xFFFF) as f32 / 65535.0
        };
        let drift = billow.drift
            + (Vec3::new(r(1) - 0.5, 0.0, r(2) - 0.5) * 1.2 + up * (0.25 + 0.35 * r(3)))
                * billow.stir;
        commands.spawn((
            Name::new("Dust"),
            Puff {
                age: 0.0,
                life: billow.life,
                wide: billow.wide * (0.6 + 0.6 * r(4)),
                rise: drift,
                material: material.clone(),
                alpha,
                owner: sprite == 0,
                roll: r(5) * core::f32::consts::TAU,
            },
            Mesh3d(quad.clone()),
            MeshMaterial3d(material.clone()),
            NotShadowCaster,
            // Spread over the billow in lumps, some higher.
            Transform::from_translation(
                at + drift * 0.2
                    + (Vec3::new(r(6) - 0.5, 0.0, r(7) - 0.5) * 2.0 + up * r(8) * 0.8)
                        * billow.spread,
            )
            .with_scale(Vec3::splat(PUFF_M[0] * 0.5)),
        ));
    }
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
        transform.scale = Vec3::splat(PUFF_M[0] * 0.5 + (puff.wide - PUFF_M[0] * 0.5) * grow);
        transform.translation += puff.rise * time.delta_secs() * (1.0 - through);
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
