//! The marsh's land and water: low hummocks of moss and grass, mud banks shelving into
//! three open waters (a channel winding across the front of the fight, another behind it,
//! a mere to the right), drawn in the glade's ground layers by what lies where, and the
//! water's face over them (pool.rs).

use super::{KNIGHT, LEVEL};
use crate::pool::{self, Look, PoolMaterial};
use crate::terrain::{GroundMaterial, Land, fbm, ground_material, smooth};
use bevy::prelude::*;

/// The water's floor in a channel's middle, and the bank's mean height over the water.
const DEEP: f32 = -0.6;
const BANK: f32 = 0.16;
/// The mere's middle and size.
pub const MERE: Vec2 = Vec2::new(6.8, 0.6);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn);
}

/// The near channel's middle line: z at x.
pub fn near(x: f32) -> f32 {
    3.7 + 1.2 * (x * 0.33).sin()
}

/// The far channel's middle line.
pub fn far(x: f32) -> f32 {
    -5.0 - 0.12 * x + 0.9 * (x * 0.25 + 1.0).sin()
}

/// How far into open water a point lies: 1 in a channel's middle, 0 on the bank.
pub fn wet(p: Vec2) -> f32 {
    let wob = (fbm(p * 0.4, 7) - 0.5) * 1.6;
    let a = 1.0 - ((p.y - near(p.x) - wob * 0.5).abs() / (1.7 + wob * 0.4));
    let b = 1.0 - ((p.y - far(p.x) - wob * 0.5).abs() / (1.5 + wob * 0.4));
    let c = 1.0 - (p - MERE).length() / (2.8 + wob * 0.7);
    a.max(b).max(c).clamp(0.0, 1.0)
}

/// The land's height: hummocks on the banks, shelving into the waters.
pub fn shape(p: Vec2) -> f32 {
    let bank = BANK + 0.18 * (fbm(p * 0.3, 11) - 0.5) + 0.08 * (fbm(p * 1.1, 13) - 0.5);
    let s = smooth(0.0, 0.6, wet(p));
    bank * (1.0 - s) + DEEP * s
}

/// Each vertex's layers: mud on the low wet ground and trodden about the fight, grass and
/// moss on the hummocks, dead leaves in drifts.
fn cover(p: Vec2, _n: Vec3, h: f32) -> ([f32; 4], [f32; 2], [f32; 2]) {
    let low = smooth(0.2, 0.04, h);
    let trod = smooth(3.2, 1.2, p.distance(KNIGHT));
    let mud = low.max(trod * 0.75);
    let patch = fbm(p * 0.45, 21);
    let moss = (1.0 - mud) * smooth(0.35, 0.65, patch);
    let grass = (1.0 - mud) * (1.0 - moss) * smooth(0.12, 0.24, h);
    let leaves = (1.0 - mud) * smooth(0.6, 0.8, fbm(p * 0.7, 23)) * 0.6;
    let roots = (1.0 - low) * smooth(0.68, 0.8, fbm(p * 0.9, 29)) * 0.5;
    (
        [grass, leaves, moss, 0.0],
        [mud, roots],
        [1.0, trod * 0.6],
    )
}

/// The water's look at dusk: near black, the violet sky high and the sunset's glow low in
/// it, the fires' and wisps' streaks.
pub fn look() -> Look {
    Look {
        deep: Vec4::new(0.004, 0.006, 0.007, 0.35),
        shallow: Vec4::new(0.02, 0.018, 0.012, 0.45),
        sky: Vec4::new(10.0, 12.0, 26.0, 25.0),
        horizon: Vec4::new(40.0, 18.0, 9.0, 0.25),
        wave: Vec4::new(super::WIND.x, super::WIND.y, 0.03, 0.0),
        look: Vec4::new(0.0, 600.0, 1.0, 0.3),
        lights: default(),
    }
}

fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    land: Res<Land>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut grounds: ResMut<Assets<GroundMaterial>>,
    mut pools: ResMut<Assets<PoolMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(land.mesh(cover))),
        MeshMaterial3d(grounds.add(ground_material(&asset))),
    ));
    let face = pool::surface(
        Vec2::new(-28.0, -24.0),
        Vec2::new(28.0, 16.0),
        0.25,
        LEVEL,
        |p| land.height(p),
    );
    commands.spawn((
        Mesh3d(meshes.add(face)),
        MeshMaterial3d(pools.add(pool::material(look()))),
        bevy::light::NotShadowCaster,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fight stands on a dry bank, the channels and the mere are open water.
    #[test]
    fn the_fight_is_dry_and_the_waters_are_open() {
        for p in [KNIGHT, super::super::FOE, super::super::TARGET] {
            assert!(shape(p) > LEVEL + 0.03, "{p} is under water: {}", shape(p));
        }
        for p in [Vec2::new(0.0, near(0.0)), Vec2::new(-2.0, far(-2.0)), MERE] {
            assert!(shape(p) < LEVEL - 0.25, "{p} is not open water: {}", shape(p));
        }
    }
}
