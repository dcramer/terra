//! A torn banner on the back wall, right of the breach's torch: faded crimson cloth with a
//! tarnished gold border and a sun worn to a ghost, its foot cut in a swallowtail, ragged
//! and holed, hung from a black pole. A grid of cloth on grass.rs's wind material, its
//! second uv how far down the cloth, so its free end ripples on the wind and its top stays.

use super::super::walls::{BACK_Z, THICK};
use super::grass::{WindMaterial, material};
use crate::ruin::effects::kit::{Noise, image, smooth};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// Where its pole hangs on the wall's face, how wide and long the cloth is.
const AT: Vec3 = Vec3::new(2.9, 2.8, BACK_Z + THICK * 0.5 + 0.07);
const WIDE: f32 = 0.9;
const LONG: f32 = 1.9;
/// How far its free end moves in a full gust (m).
const FLUTTER: f32 = 0.14;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn);
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut winds: ResMut<Assets<WindMaterial>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let cloth = material(
        StandardMaterial {
            base_color_texture: Some(images.add(weave())),
            alpha_mode: AlphaMode::Mask(0.5),
            perceptual_roughness: 0.95,
            reflectance: 0.2,
            cull_mode: None,
            double_sided: true,
            ..default()
        },
        FLUTTER,
    );
    commands.spawn((
        Mesh3d(meshes.add(grid(12, 24))),
        MeshMaterial3d(winds.add(cloth)),
        Transform::from_translation(AT),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.022, WIDE + 0.16))),
        MeshMaterial3d(standard.add(StandardMaterial {
            base_color: Color::srgb(0.06, 0.045, 0.035),
            perceptual_roughness: 0.6,
            ..default()
        })),
        Transform::from_translation(AT + Vec3::new(0.0, 0.01, 0.02))
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
    ));
}

/// The cloth: `w` by `h` cells, hanging down from the pole, facing out from the wall.
fn grid(w: u32, h: u32) -> Mesh {
    let mut pos = Vec::new();
    let mut uv = Vec::new();
    let mut down = Vec::new();
    for j in 0..=h {
        for i in 0..=w {
            let (u, v) = (i as f32 / w as f32, j as f32 / h as f32);
            // A cloth sags a little between its corners.
            let sag = 0.03 * (std::f32::consts::PI * u).sin() * v;
            pos.push([(u - 0.5) * WIDE, -v * LONG - sag, 0.0]);
            uv.push([u, v]);
            down.push([v, 1.0]);
        }
    }
    let mut idx = Vec::new();
    for j in 0..h {
        for i in 0..w {
            let a = j * (w + 1) + i;
            let b = a + w + 1;
            idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    let n = pos.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; n])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, down)
    .with_inserted_indices(Indices::U32(idx))
}

/// Whether the cloth is there at (u, v): cut to a swallowtail at the foot, frayed at the
/// edges, holed where the noise says. `n` is a fine noise there, 0 to 1.
fn cloth_at(u: f32, v: f32, n: f32) -> bool {
    let tail = 0.8 + 0.18 * (2.0 * u - 1.0).abs() + 0.05 * (n - 0.5);
    let fray = (u.min(1.0 - u) < 0.03 + 0.04 * n) && n > 0.55;
    let hole = n > 0.82 && v > 0.35;
    v < tail && !fray && !hole
}

fn weave() -> Image {
    let fine = Noise::new(16, 0xba11);
    let big = Noise::new(4, 0xba12);
    let (w, h) = (128, 256);
    image(w, h, |x, y| {
        let (u, v) = ((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
        let n = fine.at(Vec2::new(u * 16.0, v * 32.0));
        if !cloth_at(u, v, n) {
            return [0.0; 4];
        }
        // Faded crimson, darker in streaks of weave and dirt rising from the foot.
        let streak = 0.85 + 0.15 * fine.at(Vec2::new(u * 48.0, v * 2.0));
        let fade = 0.75 + 0.25 * big.at(Vec2::new(u * 4.0, v * 4.0));
        let dirt = 1.0 - 0.45 * smooth(0.4, 0.95, v);
        let mut c = Vec3::new(0.36, 0.06, 0.05) * streak * fade * dirt;
        // A tarnished gold border, and a sun worn to a ghost in the middle.
        let edge = u.min(1.0 - u).min(v);
        let border = smooth(0.07, 0.055, edge) * smooth(0.025, 0.04, edge);
        let r = Vec2::new((u - 0.5) * 1.0, (v - 0.36) * 2.0).length();
        let ray = (std::f32::consts::TAU * 6.0 * (v - 0.36).atan2(u - 0.5) / std::f32::consts::TAU).sin();
        let sun = smooth(0.17, 0.15, r) + smooth(0.3, 0.2, r) * smooth(0.13, 0.17, r) * smooth(0.4, 0.8, ray);
        let gold = Vec3::new(0.42, 0.3, 0.12) * dirt * (0.7 + 0.3 * n);
        c = c.lerp(gold, (border * 0.85 + sun * 0.5).min(1.0));
        [c.x, c.y, c.z, 1.0]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The foot is a swallowtail: the middle shorter than the corners; the top always whole.
    #[test]
    fn swallowtail_foot() {
        assert!(!cloth_at(0.5, 0.85, 0.5) && cloth_at(0.1, 0.85, 0.5));
        assert!((0..20).all(|i| cloth_at(0.1 + i as f32 * 0.04, 0.05, 0.5)));
    }
}
