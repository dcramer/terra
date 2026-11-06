//! The marsh's land and water: low hummocks of moss and grass, mud banks shelving into
//! three open waters (a channel winding across the front of the fight, another behind it,
//! a mere to the right), drawn in the glade's ground layers by what lies where, and the
//! water's face over them (pool.rs).

use super::{KNIGHT, LEVEL};
use crate::pool::{self, Edge, Look, PoolMaterial, Sheen};
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

/// The trodden path: from the far channel's bank past the treant, through the fight and on
/// to the mere's bank under the lantern (the refs run a path through every fight).
const PATH: [Vec2; 5] = [
    Vec2::new(-6.5, -3.4),
    Vec2::new(-3.0, -1.6),
    KNIGHT,
    Vec2::new(2.6, -0.5),
    Vec2::new(4.4, -0.9),
];

/// How far a point lies from the path's middle line (m).
pub fn from_path(p: Vec2) -> f32 {
    PATH.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
            p.distance(a + (b - a) * t)
        })
        .fold(f32::MAX, f32::min)
}

/// How much of the path a point lies on: 1 down its middle, 0 off it, its edge wandering
/// between about 0.4 and 1 m out.
pub fn path(p: Vec2) -> f32 {
    let half = 0.7 + 0.6 * (fbm(p * 0.5, 41) - 0.5);
    smooth(half, half * 0.45, from_path(p))
}

/// Puddles in the path's ruts, sunk under the water's face (so pool.rs draws them and the
/// lights glint in them), none where the cast stands.
fn puddle(p: Vec2) -> f32 {
    let feet = [KNIGHT, super::FOE, super::TARGET]
        .iter()
        .map(|c| p.distance(*c))
        .fold(f32::MAX, f32::min);
    path(p) * smooth(0.58, 0.7, fbm(p * 0.9, 43)) * smooth(1.2, 1.8, feet) * 0.34
}

/// The land's height: hummocks on the banks, shelving into the waters, the path's puddles.
pub fn shape(p: Vec2) -> f32 {
    let bank = BANK + 0.18 * (fbm(p * 0.3, 11) - 0.5) + 0.08 * (fbm(p * 1.1, 13) - 0.5);
    let s = smooth(0.0, 0.6, wet(p));
    bank * (1.0 - s) + DEEP * s - puddle(p)
}

/// Each vertex's layers, in broad zones a few metres across so the ground is not one even
/// tone: a band of wet black mud at the water's line, mud flats in the hollows and trodden
/// about the fight, cushions of bright moss, grass on the hummocks, drifts of dead leaves,
/// roots breaking through; the path wet mud down its middle and packed earth at its edges,
/// and patches a couple of metres across (moss cushions, leaf litter) so no hummock is one
/// even carpet of grass (blind-marsh3).
fn cover(p: Vec2, _n: Vec3, h: f32) -> ([f32; 4], [f32; 2], [f32; 2]) {
    let margin = smooth(0.22, 0.02, h - LEVEL);
    let zone = fbm(p * 0.16, 31);
    let patch = fbm(p * 0.45, 47);
    let detail = fbm(p * 0.7, 21);
    let flats = smooth(0.42, 0.32, zone) * smooth(0.26, 0.08, h - LEVEL);
    let trod = smooth(3.2, 1.2, p.distance(KNIGHT));
    let path = path(p);
    let mud = margin.max(flats).max(trod * 0.75).max(path * 0.9);
    let moss = (1.0 - mud) * smooth(0.46, 0.58, zone * 0.5 + detail * 0.2 + patch * 0.3);
    let litter = smooth(0.55, 0.68, fbm(p * 0.33, 23)).max(smooth(0.4, 0.3, patch));
    let leaves = (1.0 - mud) * (1.0 - moss) * litter * 0.8;
    let grass = (1.0 - mud) * (1.0 - moss) * (1.0 - leaves * 0.7);
    let roots = (1.0 - margin) * smooth(0.66, 0.78, fbm(p * 0.9, 29)) * 0.5;
    let bare = (trod * 0.6).max(smooth(0.0, 0.5, path) * 0.8);
    ([grass, leaves, moss, 0.0], [mud, roots], [1.0, bare])
}

/// The water's look by moonrise: a saturated deep blue sky in it (vr-pond-slash's water is
/// sRGB 1, 41, 86: no red, a violet sky read as lilac paint, iso-art j24), darker toward the
/// banks over half a metre of depth, the moon's sheen (`sheen`) and the fires' and wisps'
/// streaks.
pub fn look() -> Look {
    Look {
        deep: Vec4::new(0.002, 0.005, 0.009, 0.35),
        shallow: Vec4::new(0.015, 0.016, 0.012, 0.45),
        sky: Vec4::new(0.3, 5.0, 26.0, 25.0),
        horizon: Vec4::new(5.0, 11.0, 22.0, 0.5),
        wave: Vec4::new(super::WIND.x, super::WIND.y, 0.05, 0.0),
        look: Vec4::new(0.0, 600.0, 1.0, 0.3),
        lights: default(),
    }
}

/// The margin: the water fading in over 30 cm of depth onto the wet mud, the sky broken
/// into blots about 6 m across, duckweed in dark clumps (not a band: an even mat along the
/// water line read as a pale green rim, iso-art j24) and nearly a lily pad a m² of open
/// shallows.
pub fn edge() -> Edge {
    Edge {
        fade: Vec4::new(0.3, 0.8, 6.0, 0.0),
        weed: Vec4::new(0.6, 0.2, 2.4, 0.9),
        green: Vec4::new(0.035, 0.07, 0.018, 0.24),
    }
}

/// The moon in the water: a broad sheen of its cold light about its mirror on the far
/// channel, and its lane down the water toward the eye, three fifths of it glitter in 4 cm
/// facets; the fire, the lantern and the wisps streak the same way (vr-pond-slash's pond).
pub fn sheen() -> Sheen {
    use super::light::{MOON, MOON_COLOUR, MOON_LUX};
    Sheen {
        toward: MOON.extend(8.0),
        colour: Vec4::new(6.0, 33.0, 80.0, 0.3),
        lane: Vec4::new(0.08, 0.03, 0.04, 0.6),
        moon: (MOON_COLOUR * MOON_LUX).extend(1.0),
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
        MeshMaterial3d(pools.add(pool::with_sheen(look(), edge(), sheen()))),
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

    /// The path runs unbroken from the far channel's bank through the fight to the mere's,
    /// mostly dry underfoot, with a few puddles standing in it.
    #[test]
    fn the_path_runs_from_the_fight_to_the_lantern_with_puddles() {
        let (mut on, mut wet_, mut dry) = (0, 0, 0);
        for w in PATH.windows(2) {
            for k in 0..40 {
                let p = w[0].lerp(w[1], k as f32 / 40.0);
                assert!(path(p) > 0.9, "the path breaks at {p}: {}", path(p));
                on += 1;
                if shape(p) < LEVEL - 0.02 {
                    wet_ += 1;
                } else {
                    dry += 1;
                }
            }
        }
        assert!(wet_ >= 3, "{wet_} of {on} steps stand in a puddle");
        assert!(dry > on * 2 / 3, "only {dry} of {on} steps are dry");
        assert!(path(Vec2::new(0.0, 3.0)) == 0.0, "the path spreads to the near channel");
    }
}
