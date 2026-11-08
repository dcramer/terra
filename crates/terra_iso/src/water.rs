//! The pond: water that is dark where it is deep and clear over its silty shallows, fading
//! into the bank rather than ending in an edge, its face rippled (the wind's ripples moving
//! across it) so the sky and sun break on it, and reeds along its margin.

use crate::forest::Rng;
use crate::terrain::{self, Land, POND, WATER};
use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use std::f32::consts::{FRAC_PI_2, PI};

pub type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterExt>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct WaterExt {
    /// Deep water's colour (rgb) and the depth in metres at which it is reached (a).
    #[uniform(100)]
    pub deep: LinearRgba,
    /// The shallows' colour over the silt (rgb) and how strongly the ripples bend (a).
    #[uniform(101)]
    pub shallow: LinearRgba,
}

impl MaterialExtension for WaterExt {
    fn fragment_shader() -> ShaderRef {
        "embedded://iso/shaders/water.wesl".into()
    }
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "shaders/water.wesl");
    app.add_plugins(MaterialPlugin::<WaterMaterial>::default())
        .add_systems(Startup, spawn);
}

/// The pond's face: a grid over the pond at the water's level, each vertex carrying the
/// depth of the water above the land there (its second uv), only where there is water or
/// nearly so.
fn surface(land: &Land) -> Mesh {
    let step = 0.25;
    let half = Vec2::new(9.5, 8.0);
    let lo = POND - half;
    let (nx, nz) = (
        (half.x * 2.0 / step) as usize + 1,
        (half.y * 2.0 / step) as usize + 1,
    );
    let mut pos = Vec::with_capacity(nx * nz);
    let mut uv = Vec::with_capacity(nx * nz);
    let mut uvb = Vec::with_capacity(nx * nz);
    let mut depth = Vec::with_capacity(nx * nz);
    for j in 0..nz {
        for i in 0..nx {
            let p = lo + Vec2::new(i as f32, j as f32) * step;
            let d = WATER - land.height(p);
            pos.push([p.x, WATER, p.y]);
            uv.push([p.x, p.y]);
            uvb.push([d, 0.0]);
            depth.push(d);
        }
    }
    let mut idx: Vec<u32> = Vec::new();
    for j in 0..nz - 1 {
        for i in 0..nx - 1 {
            let a = j * nx + i;
            let (b, c, d) = (a + 1, a + nx, a + nx + 1);
            if [a, b, c, d].iter().all(|&k| depth[k] < -0.05) {
                continue;
            }
            let [a, b, c, d] = [a, b, c, d].map(|k| k as u32);
            idx.extend_from_slice(&[a, d, b, a, c, d]);
        }
    }
    let n = pos.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, uvb)
    .with_inserted_indices(Indices::U32(idx))
}

/// Reeds along the margin, in clumps, where the water is shallow or the bank wet: tall
/// thin blades, olive below and straw at their tips, some with a brown head.
fn reeds(land: &Land, rng: &mut Rng) -> Mesh {
    let mut pos = Vec::new();
    let mut nor = Vec::new();
    let mut col = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let mut clumps = 0;
    for _ in 0..900 {
        let c = POND + Vec2::new(rng.range(-9.0, 9.0), rng.range(-7.5, 7.5));
        let d = WATER - land.height(c);
        // From a hand's depth in the water to a little up the bank, and only in clumps.
        if !(-0.12..0.22).contains(&d) || terrain::fbm(c * 0.3, 91) < 0.48 {
            continue;
        }
        clumps += 1;
        let blades = 14 + (rng.f() * 18.0) as usize;
        for _ in 0..blades {
            let p = c + Vec2::new(rng.f() - 0.5, rng.f() - 0.5) * 0.7;
            let h0 = land.height(p).min(WATER + 0.3);
            let tall = rng.range(0.8, 1.6);
            let width = rng.range(0.012, 0.022);
            let yaw = rng.f() * std::f32::consts::TAU;
            let side = Vec3::new(libm::cosf(yaw), 0.0, libm::sinf(yaw)) * width;
            let lean = Vec3::new(rng.f() - 0.5, 0.0, rng.f() - 0.5) * tall * 0.35;
            let dry = rng.f() * 0.5;
            let base = pos.len() as u32;
            let segs = 4;
            for k in 0..=segs {
                let t = k as f32 / segs as f32;
                let centre = Vec3::new(p.x, h0, p.y) + Vec3::Y * tall * t + lean * t * t;
                let w = 1.0 - t * 0.85;
                pos.push((centre - side * w).to_array());
                pos.push((centre + side * w).to_array());
                let n = (Vec3::Y * 0.6 + side.cross(Vec3::Y).normalize() * 0.4).normalize();
                nor.push(n.to_array());
                nor.push(n.to_array());
                let g = 0.3 + 0.7 * t;
                let tip = terrain::smooth(0.6, 1.0, t) * (0.4 + dry);
                let c = [
                    g * (0.16 + 0.32 * tip),
                    g * (0.22 + 0.12 * tip),
                    g * (0.06 + 0.08 * tip),
                    1.0,
                ];
                col.push(c);
                col.push(c);
            }
            for k in 0..segs as u32 {
                let a = base + k * 2;
                idx.extend_from_slice(&[a, a + 1, a + 3, a, a + 3, a + 2]);
            }
            // A bulrush's head on some: a short brown box near the top.
            if rng.f() < 0.18 {
                let top = Vec3::new(p.x, h0, p.y) + Vec3::Y * tall * 0.86 + lean * 0.74;
                let (r, l) = (0.025, 0.14);
                let b = pos.len() as u32;
                for (k, a) in [0.0f32, FRAC_PI_2, PI, 3.0 * FRAC_PI_2].iter().enumerate() {
                    let o = Vec3::new(libm::cosf(*a), 0.0, libm::sinf(*a)) * r;
                    for y in [0.0, l] {
                        pos.push((top + o + Vec3::Y * y).to_array());
                        nor.push((o.normalize() * 0.7 + Vec3::Y * 0.3).normalize().to_array());
                        col.push([0.16, 0.08, 0.035, 1.0]);
                    }
                    let n = 4u32;
                    let (a0, a1) = (b + k as u32 * 2, b + ((k as u32 + 1) % n) * 2);
                    idx.extend_from_slice(&[a0, a1, a1 + 1, a0, a1 + 1, a0 + 1]);
                }
            }
        }
    }
    debug!("reeds: {clumps} clumps");
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
    .with_inserted_indices(Indices::U32(idx))
}

fn spawn(
    mut commands: Commands,
    land: Res<Land>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut waters: ResMut<Assets<WaterMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(surface(&land))),
        MeshMaterial3d(waters.add(ExtendedMaterial {
            base: StandardMaterial {
                perceptual_roughness: 0.04,
                reflectance: 0.25,
                alpha_mode: AlphaMode::Blend,
                ..default()
            },
            extension: WaterExt {
                deep: LinearRgba::new(0.003, 0.008, 0.009, 0.35),
                shallow: LinearRgba::new(0.022, 0.026, 0.014, 0.2),
            },
        })),
        bevy::light::NotShadowCaster,
    ));
    let mut rng = Rng::new(77);
    commands.spawn((
        Mesh3d(meshes.add(reeds(&land, &mut rng))),
        MeshMaterial3d(materials.add(StandardMaterial {
            cull_mode: None,
            perceptual_roughness: 0.75,
            reflectance: 0.3,
            ..default()
        })),
    ));
}
