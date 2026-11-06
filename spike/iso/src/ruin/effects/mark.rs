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

/// The mark's texture: alpha its cover, the colour its heat. A splat as the ARPGs' ground
/// decals are painted (Diablo IV's scorches, PoE 2's burns): a radial noise mask, its edge
/// soft and wandering in and out by two scales of noise round it, lobes thrown out along
/// streaks; the heat in streaks radiating from the middle, a broken arc of rim (never a
/// whole ring: "the rings look like UI circles", the lead's judges) and flecks. A window
/// takes everything to nothing well inside the card, so its square never shows.
pub fn mark_image() -> Image {
    let (broad, fine, grit) = (Noise::new(8, 0xa11c), Noise::new(16, 0xa11d), Noise::new(32, 0xa11e));
    let n = 128;
    image(n, n, |x, y| {
        let p = (Vec2::new(x as f32, y as f32) + 0.5) / n as f32 * 2.0 - 1.0;
        let r = p.length();
        // Round the mark: 0 to 1, so the noise read along it tiles where it meets itself.
        let a = p.y.atan2(p.x) / std::f32::consts::TAU + 0.5;
        let lobes = broad.at(Vec2::new(a * 8.0, 0.5));
        let streaks = fine.at(Vec2::new(a * 16.0, r * 1.5));
        let edge = 0.5 + 0.24 * (lobes - 0.5) * 2.0 + 0.12 * (streaks - 0.5) * 2.0;
        let rough = grit.at(p * 9.0 + 5.0);
        let cover = (1.0 - smooth(edge - 0.22, edge + 0.04, r + (rough - 0.5) * 0.12)) * (1.0 - smooth(0.78, 0.92, r));
        let rim = (-((r - edge * 0.82) / 0.05).powi(2)).exp() * smooth(0.45, 0.7, broad.at(Vec2::new(a * 8.0, 3.5)));
        let rays = smooth(0.55, 0.85, streaks) * (1.0 - smooth(0.1, edge, r));
        let flecks = smooth(0.7, 0.85, rough) * cover;
        let heat = (0.15 + 0.85 * rim.max(rays).max(flecks)).min(1.0) * cover;
        [heat, heat, heat, cover * (0.5 + 0.5 * rim.max(rays))]
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

    /// Nothing reaches the card's border, so its square edge never shows on the floor, and
    /// the edge wanders: the cover's reach differs round the mark by a fifth of its size.
    #[test]
    fn the_mark_has_no_square_and_a_wandering_edge() {
        let img = mark_image();
        let d = img.data.as_ref().unwrap();
        let px = |x: usize, y: usize| &d[(y * 128 + x) * 4..(y * 128 + x) * 4 + 4];
        for k in 0..128 {
            for (x, y) in [(k, 0), (k, 127), (0, k), (127, k), (k, 4), (4, k)] {
                assert_eq!(px(x, y)[3], 0, "alpha at ({x}, {y})");
            }
        }
        let reach: Vec<f32> = (0..64)
            .map(|j| {
                let a = j as f32 / 64.0 * std::f32::consts::TAU;
                (0..64)
                    .map(|s| s as f32 / 64.0)
                    .find(|&r| {
                        let (x, y) = (64.0 + a.cos() * r * 64.0, 64.0 + a.sin() * r * 64.0);
                        px(x as usize, y as usize)[3] < 40
                    })
                    .unwrap_or(1.0)
            })
            .collect();
        let (lo, hi) = reach.iter().fold((1.0f32, 0.0f32), |(l, h), &r| (l.min(r), h.max(r)));
        assert!(hi - lo > 0.2, "the edge reaches {lo} to {hi} of the half-size");
    }
}
