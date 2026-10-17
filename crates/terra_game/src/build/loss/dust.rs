//! Dust (D305): a puff where something breaks or lands hard, and a cloud as big as a big
//! collapse, soft lit sprites facing the view that billow out, rise and thin away.

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
}

/// The cloud of dust a big collapse raises, as a tower's fall does in films and in
/// Battlefield's: rolling out from its foot and up for seconds, its puffs over the ground it
/// covered, low, each about as wide as the mass that fell is across; what it is of, and each
/// puff where it starts in the base and how it billows, its drift in the base's space.
/// Nothing for a fall of fewer than [`CLOUD_CUBES`].
pub(super) fn cloud(fell: &[(IVec3, Cube)]) -> Option<(Kind, Vec<(Vec3, Billow)>)> {
    if fell.len() < CLOUD_CUBES {
        return None;
    }
    let (low, high) = fell.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(low, high), (at, _)| {
            let at = at.as_vec3() * CELL_M;
            (low.min(at), high.max(at + CELL_M))
        },
    );
    let across = (fell.len() as f32 * CELL_M * CELL_M * CELL_M).cbrt();
    let count = (fell.len() / CLOUD_EVERY).clamp(4, CLOUD_MOST);
    let foot = Vec3::new((low.x + high.x) * 0.5, low.y, (low.z + high.z) * 0.5);
    let puffs = (0..count)
        .map(|i| {
            let r = |salt: u32| {
                let h = terra_gen::hash::prospector32_round(
                    (i as u32).wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA77),
                );
                ((h >> 8) & 0xFFFF) as f32 / 65535.0
            };
            let local = Vec3::new(
                low.x + (high.x - low.x) * r(1),
                low.y + (high.y - low.y) * 0.35 * r(2) * r(2),
                low.z + (high.z - low.z) * r(3),
            );
            let out = (local - foot).with_y(0.0).normalize_or(Vec3::X);
            let billow = Billow {
                alpha: 0.95,
                wide: across * (1.3 + 1.0 * r(4)),
                life: CLOUD_S * (0.75 + 0.5 * r(5)),
                drift: out * across * 0.45 + Vec3::Y * (1.0 + 1.4 * r(6)),
                stir: 2.0,
            };
            (local, billow)
        })
        .collect();
    Some((fell[0].1.kind, puffs))
}

/// A soft round of dust, its alpha falling off from its middle, a little uneven: what each
/// sprite of a puff is drawn with.
fn dust_image() -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    const SIDE: u32 = 64;
    let mut data = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (u, v) = (
                (x as f32 + 0.5) / SIDE as f32 * 2.0 - 1.0,
                (y as f32 + 0.5) / SIDE as f32 * 2.0 - 1.0,
            );
            let r = (u * u + v * v).sqrt();
            let lump = 0.85 + 0.15 * libm::sinf(u * 9.0 + libm::cosf(v * 7.0) * 2.0);
            let alpha = ((1.0 - r).max(0.0)).powi(2) * lump;
            data.extend([255, 255, 255, (alpha.clamp(0.0, 1.0) * 255.0) as u8]);
        }
    }
    Image::new(
        Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// The normals of a round billow on each sprite, in its own space: facing out of the sprite
/// in its middle and curving round to its edge, a little lumpy, as [`dust_image`] is.
fn dust_normals() -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    const SIDE: u32 = 64;
    let mut data = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (u, v) = (
                (x as f32 + 0.5) / SIDE as f32 * 2.0 - 1.0,
                (y as f32 + 0.5) / SIDE as f32 * 2.0 - 1.0,
            );
            let lump = 0.18 * libm::sinf(u * 9.0 + libm::cosf(v * 7.0) * 2.0);
            // Rows run down the image; its up is up the sprite.
            let side = Vec2::new(u + lump * 0.5, -v + lump * 0.3) * 0.85;
            let n = side
                .extend((1.0 - side.length_squared()).max(0.05).sqrt())
                .normalize();
            let byte = |c: f32| ((c * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
            data.extend([byte(n.x), byte(n.y), byte(n.z), 255]);
        }
    }
    Image::new(
        Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
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
    let image = shards
        .dust
        .get_or_insert_with(|| images.add(dust_image()))
        .clone();
    let normals = shards
        .dust_normals
        .get_or_insert_with(|| images.add(dust_normals()))
        .clone();
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
            },
            Mesh3d(quad.clone()),
            MeshMaterial3d(material.clone()),
            NotShadowCaster,
            Transform::from_translation(at + drift * 0.2).with_scale(Vec3::splat(PUFF_M[0] * 0.5)),
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
        let grow = 1.0 - (1.0 - through) * (1.0 - through);
        transform.scale = Vec3::splat(PUFF_M[0] * 0.5 + (puff.wide - PUFF_M[0] * 0.5) * grow);
        transform.translation += puff.rise * time.delta_secs() * (1.0 - through);
        if let Some(eye) = eye {
            let at = transform.translation;
            let up = at.normalize_or(Vec3::Y);
            transform.look_at(at * 2.0 - eye, up);
        }
        if puff.owner
            && let Some(mut material) = materials.get_mut(&puff.material)
        {
            // Thick at once, staying thick a while, then thinning away.
            let rise = (through / 0.12).min(1.0);
            let left = 1.0 - through;
            let alpha = puff.alpha * rise * left * (0.35 + 0.65 * left);
            material.base_color = material.base_color.with_alpha(alpha);
        }
    }
}
