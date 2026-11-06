//! The marsh's ground dressed (the lead's blind judge, 2026-11-06: "the ground is bare:
//! broad flat green grass with sparse blades and no shrubs, flowers or reed beds; the
//! top-right lawn a big undressed field"): sedge tussocks, ferns and low shrubs in dark
//! masses, thick along the banks and out at the frame's edges, thinning toward the fight,
//! a few marsh marigolds and campions by the water; kept off the water, the path, the cast
//! and the stones. V Rising's marsh is built so: dense dark clumps over moss and mud. The
//! clumps are the forest's (wood/lawn.rs), in the marsh's own tints.

use super::dress::{FIRE, PROPS};
use super::land::{path, shape, wet};
use super::{FOE, KNIGHT, LEVEL, TARGET};
use crate::forest::{BUTTERCUP, CAMPION, Rng};
use crate::terrain::{Land, fbm, smooth};
use crate::wood::lawn::{Clump, Kind, draw};
use bevy::prelude::*;

/// The ground the clumps are tried over (world x, z: the frame and a margin) and how many a
/// square metre are tried.
const LO: Vec2 = Vec2::new(-14.0, -13.0);
const HI: Vec2 = Vec2::new(15.0, 8.0);
const TRIES: f32 = 8.0;
/// How far the clumps keep from each of the cast and the fire (m).
const CAST: f32 = 2.0;
/// The fight's middle, and the radii over which the clumps thin toward it (m).
const MID: Vec2 = Vec2::new(-0.3, -0.95);
const THIN: Vec2 = Vec2::new(2.5, 7.0);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn);
}

/// Whether a clump may grow at `p`: on dry bank above the water, off the path, clear of the
/// cast, the fire and the stones and stumps.
pub fn grows(p: Vec2) -> bool {
    let props = PROPS.iter().all(|&(path, at, _, scale, _)| {
        let keep = if path.contains("rock") {
            scale * 1.2
        } else {
            0.4
        };
        at.distance(p) > keep
    });
    shape(p) - LEVEL > 0.05
        && wet(p) < 0.01
        && path(p) < 0.02
        && [KNIGHT, FOE, TARGET].iter().all(|c| c.distance(p) > CAST)
        && FIRE.distance(p) > 1.2
        && props
}

/// How thick the clumps grow at `p` (0 to 1): thin by the fight, thick at the frame's edges
/// and along the banks (where the bank runs down to the water).
pub fn thick(p: Vec2) -> f32 {
    let far = smooth(THIN.x, THIN.y, p.distance(MID));
    let bank = smooth(0.24, 0.07, shape(p) - LEVEL);
    (0.25 + 0.75 * far).max(bank)
}

/// The marsh's clumps, the same every run.
pub fn clumps() -> Vec<Clump> {
    let mut rng = Rng::new(0x3a_4e7);
    let area = (HI.x - LO.x) * (HI.y - LO.y);
    let mut out = Vec::new();
    for _ in 0..(area * TRIES) as usize {
        let p = LO + Vec2::new(rng.f(), rng.f()) * (HI - LO);
        // In masses with a few gaps, never an even lawn.
        let mass = fbm(p * 0.45, 71);
        let keep = (0.55 + 0.45 * smooth(0.3, 0.6, mass)) * thick(p);
        if rng.f() > keep || !grows(p) {
            continue;
        }
        let bank = smooth(0.24, 0.07, shape(p) - LEVEL);
        let hue = fbm(p * 0.21, 73);
        let dry = smooth(0.45, 0.7, fbm(p * 0.25, 75));
        let pick = rng.f();
        let (kind, tint) = if pick < 0.45 {
            // Sedge tussocks, olive to straw where the hummock dries.
            let green = Vec3::new(0.34, 0.5, 0.14).lerp(Vec3::new(0.28, 0.48, 0.18), hue);
            (
                Kind::Tuft,
                green.lerp(Vec3::new(0.62, 0.47, 0.18), dry * 0.8),
            )
        } else if pick < 0.75 {
            (
                Kind::Fern,
                Vec3::new(0.18, 0.46, 0.22).lerp(Vec3::new(0.3, 0.52, 0.16), hue),
            )
        } else {
            // Low shrubs: bog myrtle's green to a bilberry's red-brown.
            let leaf = Vec3::new(0.24, 0.44, 0.16).lerp(Vec3::new(0.46, 0.26, 0.14), dry);
            (Kind::Leaves, leaf)
        };
        // Darker down the banks, in the wet.
        let tint = tint * rng.range(0.5, 0.95) * (1.0 - 0.25 * bank);
        let flower =
            (bank > 0.3 && rng.f() < 0.05).then(|| if rng.f() < 0.7 { BUTTERCUP } else { CAMPION });
        let size = if kind == Kind::Tuft {
            rng.range(0.24, 0.42)
        } else {
            rng.range(0.28, 0.5)
        };
        out.push(Clump {
            at: p,
            radius: size * (0.8 + 0.4 * mass),
            kind,
            tint,
            flower,
        });
    }
    out
}

fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    land: Res<Land>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<crate::foliage::FoliageMaterial>>,
    mut mats: ResMut<crate::foliage::Mats>,
) {
    let all = clumps();
    draw(
        &mut commands,
        &asset,
        &land,
        &mut meshes,
        &mut materials,
        &mut mats,
        &all,
        0x3a_4e8,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No clump stands in the water, on the path or within reach of the cast; most of the
    /// open bank far from the fight has one near, and the ground by the fight fewer.
    #[test]
    fn the_clumps_cover_the_banks_and_thin_toward_the_fight() {
        let all = clumps();
        for c in &all {
            assert!(
                shape(c.at) - LEVEL > 0.05,
                "a clump at {} is in the water",
                c.at
            );
            assert!(path(c.at) < 0.02, "a clump at {} is on the path", c.at);
            for f in [KNIGHT, FOE, TARGET] {
                assert!(
                    f.distance(c.at) > CAST - 1e-3,
                    "a clump at {} is on a fighter",
                    c.at
                );
            }
        }
        // Open points (where clumps may grow) with a clump within 0.6 m, and the clumps
        // there, near the fight and far from it.
        let (mut near, mut far) = ((0, 0, 0), (0, 0, 0));
        for i in 0..80 {
            for j in 0..60 {
                let p = LO
                    + (HI - LO) * Vec2::new(i as f32 + 0.5, j as f32 + 0.5) / Vec2::new(80.0, 60.0);
                if !grows(p) {
                    continue;
                }
                let count = all.iter().filter(|c| c.at.distance(p) < 0.6).count() as i32;
                let d = p.distance(MID);
                let at = if d < 4.0 {
                    &mut near
                } else if d > 8.0 {
                    &mut far
                } else {
                    continue;
                };
                *at = (at.0 + 1, at.1 + (count > 0) as i32, at.2 + count);
            }
        }
        assert!(
            near.0 > 50 && far.0 > 500,
            "open points: {near:?} near, {far:?} far"
        );
        let cover = far.1 as f32 / far.0 as f32;
        assert!(
            cover > 0.8,
            "the clumps cover {cover} of the open ground far from the fight"
        );
        let (n, f) = (near.2 as f32 / near.0 as f32, far.2 as f32 / far.0 as f32);
        assert!(
            n < f * 0.6,
            "{n} clumps a point by the fight against {f} far from it"
        );
    }
}
