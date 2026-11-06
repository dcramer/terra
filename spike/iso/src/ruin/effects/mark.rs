//! Marks on the ground where a blow or a bolt lands, as the ARPGs leave theirs (Diablo IV's
//! scorch and frost decals under a hit, Path of Exile's ground "burn" under a spell's
//! impact): a forward decal (Bevy's `ForwardDecal`, which needs the depth prepass SSAO
//! already brings) lying on the floor, its rim glowing the effect's colour the moment it
//! lands, cooling to a dark residue and gone over its life. The owner makes one a hit at
//! startup and sets its age; nothing is spawned at run time.

use super::kit::{Noise, image, smooth};
use bevy::pbr::decal::{ForwardDecal, ForwardDecalMaterial, ForwardDecalMaterialExt};
use bevy::prelude::*;

pub type Decal =ForwardDecalMaterial<StandardMaterial>;

/// A mark: how long since its hit (negative: not yet), its life (s), the rim's glow at
/// once and the residue's colour (linear), and its own material to fade.
#[derive(Component)]
pub struct Mark {
    pub age: f32,
    life: f32,
    glow: LinearRgba,
    residue: LinearRgba,
    material: Handle<Decal>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, fade);
}

/// Lays a mark `size` across on the floor at `at`, hidden until its owner sets its age.
#[allow(clippy::too_many_arguments)]
pub fn mark(
    commands: &mut Commands,
    decals: &mut Assets<Decal>,
    texture: &Handle<Image>,
    at: Vec3,
    size: f32,
    life: f32,
    glow: LinearRgba,
    residue: LinearRgba,
) -> Entity {
    let material = decals.add(Decal {
        base: StandardMaterial {
            base_color: residue.into(),
            base_color_texture: Some(texture.clone()),
            emissive_texture: Some(texture.clone()),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.9,
            ..default()
        },
        extension: ForwardDecalMaterialExt {
            depth_fade_factor: 0.6,
        },
    });
    commands
        .spawn((
            ForwardDecal,
            MeshMaterial3d(material.clone()),
            Transform::from_translation(at).with_scale(Vec3::splat(size)),
            Visibility::Hidden,
            Mark {
                age: -1.0,
                life,
                glow,
                residue,
                material,
            },
        ))
        .id()
}

/// The mark's texture: alpha its cover (whole in the middle, a ragged edge), the colour its
/// heat (a bright broken ring where the impact threw out, flecks, a dimmer middle).
pub fn mark_image() -> Image {
    let noise = Noise::new(8, 0xa11c);
    let n = 128;
    image(n, n, |x, y| {
        let p = (Vec2::new(x as f32, y as f32) + 0.5) / n as f32 * 2.0 - 1.0;
        let r = p.length();
        let a = p.y.atan2(p.x) / std::f32::consts::TAU + 0.5;
        let ragged = noise.at(Vec2::new(a * 8.0, r * 3.0));
        let cover = 1.0 - smooth(0.45, 0.92, r + (ragged - 0.5) * 0.35);
        let ring = (-((r - 0.55 - (ragged - 0.5) * 0.2) / 0.09).powi(2)).exp();
        let flecks = smooth(0.72, 0.85, noise.at(p * 6.0 + 3.0)) * cover;
        let heat = (0.3 + 0.7 * ring.max(flecks)).min(1.0);
        [heat, heat, heat, cover * (0.55 + 0.45 * ring.max(0.5))]
    })
}

/// The heat of a mark `k` through its life: full at once, mostly gone in its first fifth.
fn heat(k: f32) -> f32 {
    (-k / 0.12).exp()
}

/// The cover of a mark `k` through its life: whole, then fading out over its last half.
fn cover(k: f32) -> f32 {
    1.0 - smooth(0.5, 1.0, k)
}

fn fade(mut marks: Query<(&Mark, &mut Visibility)>, mut decals: ResMut<Assets<Decal>>) {
    for (m, mut vis) in &mut marks {
        let k = m.age / m.life;
        let on = (0.0..1.0).contains(&k);
        let want = if on { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        if !on {
            continue;
        }
        if let Some(mut d) = decals.get_mut(&m.material) {
            let h = heat(k);
            d.base.base_color = m.residue.with_alpha(m.residue.alpha * cover(k)).into();
            d.base.emissive = LinearRgba::rgb(m.glow.red * h, m.glow.green * h, m.glow.blue * h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mark glows at once and cools long before it is gone; it is whole for its first half.
    #[test]
    fn a_mark_cools_then_fades() {
        assert!(heat(0.0) > 0.99 && heat(0.3) < 0.1);
        assert!(cover(0.4) > 0.99 && cover(0.99) < 0.01);
    }

    /// The mark's middle is covered and its corners are bare.
    #[test]
    fn the_mark_is_a_ragged_disc() {
        let img = mark_image();
        let d = img.data.as_ref().unwrap();
        let alpha = |x: usize, y: usize| d[(y * 128 + x) * 4 + 3];
        assert!(alpha(64, 64) > 100, "middle {}", alpha(64, 64));
        assert!(alpha(2, 2) == 0 && alpha(125, 3) == 0);
    }
}
