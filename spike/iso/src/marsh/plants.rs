//! The marsh's plants in the wind: reeds and bulrushes in clumps along every water's edge
//! (from a hand's depth in the water to a little up the bank), and tufts of sedge on the
//! hummocks, kept off the fight's ground. Blades with their colour in the vertices, on the
//! ruin's wind material (ruin/ambient/grass.rs): the reeds bend by their height over 1.4 m,
//! the sedge as the ruin's grass.

use super::land::{from_path, path, shape, wet};
use crate::terrain::smooth;
use super::{FOE, KNIGHT, LEVEL, TARGET};
use crate::forest::Rng;
use crate::ruin::ambient::grass::{WindMaterial, material};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// Reed clumps tried, and sedge tufts tried (most fall where they do not grow).
const CLUMPS: usize = 2800;
const TUFTS: usize = 2600;
/// The span planted (x, z): the frame and a margin.
const LO: Vec2 = Vec2::new(-15.0, -12.0);
const HI: Vec2 = Vec2::new(15.0, 9.0);
/// The reeds' sway: how far a tip moves in a full gust (m), the height for full bend (m).
const REED_BEND: f32 = 0.18;
const REED_TALL: f32 = 1.4;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn);
}

/// Blades of one kind being built: place, normal, colour and indices.
#[derive(Default)]
struct Blades {
    pos: Vec<[f32; 3]>,
    nor: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl Blades {
    /// One blade from `foot`, `tall` high and `width` wide, leaning by `lean` at its tip,
    /// its colour from root (dark) to tip by `tint(t)`.
    fn blade(&mut self, foot: Vec3, tall: f32, width: f32, yaw: f32, lean: Vec3, tint: impl Fn(f32) -> [f32; 4]) {
        let side = Vec3::new(yaw.cos(), 0.0, yaw.sin()) * width;
        let n = (Vec3::Y * 0.6 + side.cross(Vec3::Y).normalize() * 0.4).normalize();
        let base = self.pos.len() as u32;
        let segs = 4;
        for k in 0..=segs {
            let t = k as f32 / segs as f32;
            let centre = foot + Vec3::Y * tall * t + lean * t * t;
            let w = 1.0 - t * 0.85;
            self.pos.push((centre - side * w).to_array());
            self.pos.push((centre + side * w).to_array());
            self.nor.push(n.to_array());
            self.nor.push(n.to_array());
            let c = tint(t);
            self.col.push(c);
            self.col.push(c);
        }
        for k in 0..segs as u32 {
            let a = base + k * 2;
            self.idx.extend_from_slice(&[a, a + 1, a + 3, a, a + 3, a + 2]);
        }
    }

    /// A bulrush's head: a short brown four-sided box at `top`.
    fn head(&mut self, top: Vec3) {
        let (r, l) = (0.03, 0.16);
        let b = self.pos.len() as u32;
        for (k, a) in [0.0f32, 1.5708, 3.1416, 4.7124].iter().enumerate() {
            let o = Vec3::new(a.cos(), 0.0, a.sin()) * r;
            for y in [0.0, l] {
                self.pos.push((top + o + Vec3::Y * y).to_array());
                self.nor.push((o.normalize() * 0.7 + Vec3::Y * 0.3).normalize().to_array());
                self.col.push([0.14, 0.07, 0.03, 1.0]);
            }
            let (a0, a1) = (b + k as u32 * 2, b + ((k as u32 + 1) % 4) * 2);
            self.idx.extend_from_slice(&[a0, a1, a1 + 1, a0, a1 + 1, a0 + 1]);
        }
    }

    fn mesh(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nor)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
            .with_inserted_indices(Indices::U32(self.idx))
    }
}

/// Whether a point is kept clear for the fight (the cast's feet and the space between) and
/// the trodden path.
pub fn clear(p: Vec2) -> bool {
    [KNIGHT, FOE, TARGET, super::dress::FIRE]
        .iter()
        .any(|c| p.distance(*c) < 1.4)
        || p.distance((KNIGHT + FOE) * 0.5) < 1.2
        || path(p) > 0.05
}

/// How likely a sedge tuft is to grow: in patches over the hummocks, and thick along the
/// path's verges where feet do not reach.
fn tufted(c: Vec2) -> f32 {
    let verge = smooth(1.7, 1.1, from_path(c));
    crate::terrain::fbm(c * 0.5, 17).max(verge * 0.9)
}

/// Where reeds grow: the water's edge, from 0.25 m deep to 0.12 m up the bank.
pub fn edge(p: Vec2) -> bool {
    let d = shape(p) - LEVEL;
    (-0.25..0.12).contains(&d)
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut winds: ResMut<Assets<WindMaterial>>,
) {
    let mut rng = Rng::new(0x3a25);
    let mut reeds = Blades::default();
    for _ in 0..CLUMPS {
        let c = Vec2::new(rng.range(LO.x, HI.x), rng.range(LO.y, HI.y));
        if !edge(c) || clear(c) || crate::terrain::fbm(c * 0.35, 91) < 0.36 {
            continue;
        }
        for _ in 0..(12 + (rng.f() * 22.0) as usize) {
            let p = c + Vec2::new(rng.f() - 0.5, rng.f() - 0.5) * 0.8;
            let foot = p.extend(shape(p).min(LEVEL + 0.1)).xzy();
            let tall = rng.range(0.8, 1.6);
            let lean = Vec3::new(rng.f() - 0.5, 0.0, rng.f() - 0.5) * tall * 0.35;
            let dry = rng.f() * 0.6;
            // Broad enough to read as leaves at the camera (a pixel is about 1 cm): thinner
            // ones alias into dark hair.
            reeds.blade(foot, tall, rng.range(0.016, 0.03), rng.f() * 6.283, lean, |t| {
                let g = 0.4 + 1.0 * t;
                let tip = crate::terrain::smooth(0.5, 1.0, t) * (0.4 + dry);
                [g * (0.15 + 0.34 * tip), g * (0.2 + 0.12 * tip), g * (0.06 + 0.07 * tip), 1.0]
            });
            if rng.f() < 0.15 {
                reeds.head(foot + Vec3::Y * tall * 0.84 + lean * 0.7);
            }
        }
    }
    let mut sedge = Blades::default();
    for _ in 0..TUFTS {
        let c = Vec2::new(rng.range(LO.x, HI.x), rng.range(LO.y, HI.y));
        let h = shape(c) - LEVEL;
        if h < 0.06 || wet(c) > 0.02 || clear(c) || rng.f() > tufted(c) {
            continue;
        }
        let foot = c.extend(shape(c)).xzy();
        for _ in 0..(8 + (rng.f() * 10.0) as usize) {
            let tall = rng.range(0.18, 0.5);
            let yaw = rng.f() * 6.283;
            let lean = Vec3::new(yaw.sin(), 0.0, yaw.cos()) * tall * rng.range(0.2, 0.6);
            let dry = rng.f();
            sedge.blade(foot, tall, rng.range(0.008, 0.016), yaw, lean, |t| {
                let g = 0.3 + 0.7 * t;
                [g * (0.2 + 0.22 * dry), g * (0.22 + 0.08 * dry), g * (0.07 + 0.04 * dry), 1.0]
            });
        }
    }
    let base = StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        reflectance: 0.3,
        cull_mode: None,
        double_sided: true,
        ..default()
    };
    let mut reed = material(base.clone(), 0.0);
    reed.extension.wind.wind.z = REED_BEND;
    reed.extension.wind.cloth.y = REED_TALL;
    // No shadows: under a sun 10 degrees up each blade's would run five or six times its
    // height, streaking the whole frame (iso-art j19's marsh).
    for (mesh, mat) in [(reeds, reed), (sedge, material(base, 0.0))] {
        commands.spawn((
            Mesh3d(meshes.add(mesh.mesh())),
            MeshMaterial3d(winds.add(mat)),
            bevy::light::NotShadowCaster,
        ));
    }
}
