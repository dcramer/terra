//! The ruin's standing water (pool.rs): a rain pool where the flags have sunk and broken by
//! the west brazier, near black at night with the fire's narrow streak across it, and
//! shallow puddles in the paving's dips that catch the fires the same way (Diablo IV's wet
//! crypt floors: what reads as wet stone is the light's mirror, not a sheen).

use crate::pool::{self, Edge, Look};
use crate::terrain::fbm;
use bevy::prelude::*;

/// The pool: its middle, its half-size (m) before its ragged bank, and its depth (m). It
/// lies 1 m toward the camera from the west brazier (fire.rs), so the brazier's mirror
/// (about 0.7 x the light's 1.35 m height toward the eye from its foot) falls on it.
pub const POOL: (Vec2, Vec2, f32) = (Vec2::new(-1.35, 2.05), Vec2::new(1.45, 0.95), 0.45);
/// The puddles: middle, half-size, depth: films in the flags' dips, each where a fire's
/// mirror falls (0.7 x the light's height toward the camera from its foot: the east
/// brazier, the back wall's torches, the pillar's), since a puddle no light glints in
/// reads as a hole in the floor.
pub const PUDDLES: [(Vec2, Vec2, f32); 4] = [
    (Vec2::new(3.75, -1.95), Vec2::new(0.9, 0.55), 0.12),
    (Vec2::new(1.8, -4.4), Vec2::new(0.8, 0.5), 0.1),
    (Vec2::new(-5.6, -4.5), Vec2::new(0.8, 0.5), 0.1),
    (Vec2::new(6.7, -1.6), Vec2::new(0.7, 0.45), 0.1),
];
/// The pool's face, a little under the flags' lowest (their settling is 1.5 cm either way),
/// so the broken flags at its bank overlap its edge; a puddle's face just over their highest.
const POOL_LEVEL: f32 = super::ground::TOP - 0.02;
const PUDDLE_LEVEL: f32 = super::ground::TOP + 0.017;

/// How wet `p` is under a blob of `half` size at `at`, 0 (dry) to 1 (its deep middle): an
/// ellipse with a ragged noisy bank, steep at the water line so the edge reads crisp (a
/// slow ramp drew the pool as a soft stain).
pub fn blob(p: Vec2, (at, half, _): (Vec2, Vec2, f32), seed: u32) -> f32 {
    let q = (p - at) / half;
    let ragged = (fbm(p * 1.3, seed) - 0.5) * 0.7 + (fbm(p * 4.0, seed + 1) - 0.5) * 0.2;
    super::ground::smooth(1.0, 0.8, q.length() + ragged)
}

/// How much of the pool is at `p` (0..1): the paving gives way to it (ground.rs).
pub fn pool(p: Vec2) -> f32 {
    blob(p, POOL, 331)
}

/// The water at night: near black, a faint moonlit band, the bank's dark mirror, still air.
pub fn look() -> Look {
    Look {
        deep: Vec4::new(0.003, 0.004, 0.005, 0.3),
        shallow: Vec4::new(0.02, 0.018, 0.015, 0.55),
        sky: Vec4::new(4.0, 7.0, 18.0, 20.0),
        horizon: Vec4::new(5.0, 4.0, 3.5, 0.12),
        // A breath of air: ripples just steep enough to draw a light into a streak.
        wave: Vec4::new(0.6, 0.8, 0.06, 0.0),
        look: Vec4::new(0.0, 1500.0, 2.0, 0.25),
        lights: default(),
    }
}

/// A stone bank, harder than the marsh's mud: a short fade, a little broken sky, a skin of
/// dark algae at the margin.
pub fn edge() -> Edge {
    Edge {
        fade: Vec4::new(0.04, 0.5, 4.0, 0.0),
        weed: Vec4::new(0.35, 0.05, 0.7, 0.0),
        green: Vec4::new(0.03, 0.045, 0.015, 0.0),
    }
}

pub fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut pools: ResMut<Assets<pool::PoolMaterial>>,
) {
    let water = pools.add(pool::with_edge(look(), edge()));
    // A puddle is a few centimetres deep: dark wet stone over its middle, not a clear film.
    let mut thin = look();
    thin.deep.w = 0.12;
    thin.shallow.w = 0.45;
    // The moonlit sky's sheen, stronger than the pool's, so a puddle off a fire's line
    // reads as wet stone and not as a hole.
    thin.sky.w = 30.0;
    // Its bank's dark mirror reaches only its rim: a puddle is all margin, and at the
    // pool's reach it mirrored only the bank, black.
    thin.horizon.w = 0.02;
    let film = pools.add(pool::with_edge(thin, edge()));
    let mut lay =
        |blob_at: (Vec2, Vec2, f32), level: f32, seed: u32, water: &Handle<pool::PoolMaterial>| {
            let (at, half, depth) = blob_at;
            let reach = half * 1.6;
            let face = pool::surface(at - reach, at + reach, 0.08, level, |p| {
                // Dry ground stands over the face, so only the blob is wet.
                level - depth * blob(p, blob_at, seed) + 0.06 * (1.0 - blob(p, blob_at, seed))
            });
            commands.spawn((
                Mesh3d(meshes.add(face)),
                MeshMaterial3d(water.clone()),
                bevy::light::NotShadowCaster,
            ));
        };
    lay(POOL, POOL_LEVEL, 331, &water);
    for (k, p) in PUDDLES.iter().enumerate() {
        lay(*p, PUDDLE_LEVEL, 341 + 2 * k as u32, &film);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The brazier's mirror lands on the pool: its light's image falls toward the camera
    /// from its foot, so the water's middle lies that way, and the brazier stands dry.
    #[test]
    fn the_brazier_glints_on_the_pool() {
        let brazier = super::super::fire::BRAZIERS[0];
        let yaw = super::super::camera::YAW.to_radians();
        let toward = Vec2::new(yaw.sin(), yaw.cos());
        let image = brazier + toward * 0.7 * 1.35;
        assert!(
            pool(image) > 0.5,
            "the brazier's glint at {image} misses the pool"
        );
        assert!(
            pool(brazier) < 0.5,
            "the brazier at {brazier} stands in the pool"
        );
    }

    /// The fight stands on dry flags.
    #[test]
    fn the_fight_is_dry() {
        for p in [Vec2::ZERO, Vec2::new(2.0, -1.4), Vec2::new(-2.8, -3.4)] {
            assert!(pool(p) == 0.0, "the pool reaches the fight at {p}");
            for (k, d) in PUDDLES.iter().enumerate() {
                assert!(blob(p, *d, 341 + 2 * k as u32) == 0.0, "puddle {k} at {p}");
            }
        }
    }
}
