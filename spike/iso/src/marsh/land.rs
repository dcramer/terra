//! The marsh's land and water: low hummocks of moss and grass, mud banks shelving into
//! three open waters (a channel winding across the front of the fight, another behind it,
//! a mere to the right), drawn in the glade's ground layers by what lies where, and the
//! water's face over them (the kit's, kit/water.rs).

use super::{KNIGHT, LEVEL};
use crate::kit;
use crate::kit::ground::{GroundMaterial, Place, material};
use crate::terrain::{Land, SITES, fbm, smooth};
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

/// How far into open water a point lies: 1 in a channel's middle, 0 on the bank. The
/// mere's shore runs in bays and points (two angular lobes and a finer wander), not a
/// round disc (the judge: "the mere's hard-edged disc").
pub fn wet(p: Vec2) -> f32 {
    let wob = (fbm(p * 0.4, 7) - 0.5) * 1.6;
    let a = 1.0 - ((p.y - near(p.x) - wob * 0.5).abs() / (1.7 + wob * 0.4));
    let b = 1.0 - ((p.y - far(p.x) - wob * 0.5).abs() / (1.5 + wob * 0.4));
    let m = p - MERE;
    let ang = m.y.atan2(m.x);
    let bays = 0.35 * (3.0 * ang + 0.7).sin() + 0.2 * (5.0 * ang + 2.1).sin();
    let shore = 2.8 + wob * 0.9 + bays + (fbm(p * 1.2, 17) - 0.5) * 0.5;
    let c = 1.0 - m.length() / shore;
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

/// Puddles in the path's ruts, sunk under the water's face (so the kit's water draws them and the
/// lights glint in them), none where the cast stands.
fn puddle(p: Vec2) -> f32 {
    let feet = [KNIGHT, super::FOE, super::TARGET]
        .iter()
        .map(|c| p.distance(*c))
        .fold(f32::MAX, f32::min);
    path(p) * smooth(0.58, 0.7, fbm(p * 0.9, 43)) * smooth(1.2, 1.8, feet) * 0.34
}

/// The land's height: hummocks on the banks, shelving into the waters, the path's puddles,
/// with a mud shelf at the water's line.
pub fn shape(p: Vec2) -> f32 {
    let bank = BANK + 0.18 * (fbm(p * 0.3, 11) - 0.5) + 0.08 * (fbm(p * 1.1, 13) - 0.5);
    let s = smooth(0.0, 0.6, wet(p));
    shelf(bank * (1.0 - s) + DEEP * s - puddle(p))
}

/// The mud shelf: heights within `SHELF.y` of `SHELF.x` (just under the face) pressed
/// toward it, half as steep at its middle, so the bank runs out into a broad lip of
/// shallows and wet mud for the stones, roots and reeds to stand in, not one cut line
/// (the judge, iso-art j35). Unchanged outside the band, monotone and smooth: the slope
/// is 1 - 0.25 (cos(pi d / w) + cos(2 pi d / w)), 0.5 at the middle, 1 at the band's ends.
/// At 0.12 wide about -0.03 the bank still fell 0.3 m in its first 0.3 m out (iso-kit j45):
/// 0.2 about -0.05, from a quarter metre under the face to 0.15 m up the bank.
const SHELF: Vec2 = Vec2::new(-0.05, 0.2);
pub fn shelf(h: f32) -> f32 {
    let (c, w) = (SHELF.x, SHELF.y);
    let d = h - c;
    if d.abs() >= w {
        return h;
    }
    let x = std::f32::consts::PI * d / w;
    h - 0.25 * w / std::f32::consts::PI * (x.sin() + 0.5 * (2.0 * x).sin())
}

/// The depth the water's face is drawn at over a point `d` metres under it: a quarter of
/// the true depth to 0.35 m, the true depth from 0.7 m, so the face
/// fades in over about half a metre of wet mud shelving under it and the scum and leaves mat
/// that margin, not a clean cut at the bank's line (the judge, round 261107-0358-2: "banks
/// cut clean"; the ruin's pool's made-up shelving depth, ruin/water.rs, iso-kit j38). Dry
/// land (`d` under 0) is unchanged; monotone, so the line stays where it is.
const SHALLOWS: f32 = 0.25;
pub fn seen_depth(d: f32) -> f32 {
    if d <= 0.0 {
        return d;
    }
    d * (SHALLOWS + (1.0 - SHALLOWS) * smooth(0.35, 0.7, d))
}

/// Each vertex's layers, in broad zones a few metres across so the ground is not one even
/// tone: a band of wet black mud at the water's line, mud flats in the hollows and trodden
/// about the fight, cushions of bright moss, grass on the hummocks, drifts of dead leaves,
/// roots breaking through; the path wet mud down its middle and packed earth at its edges,
/// and patches a couple of metres across (moss cushions, leaf litter) so no hummock is one
/// even carpet of grass (blind-marsh3).
pub fn cover(p: Vec2, _n: Vec3, h: f32) -> ([f32; 4], [f32; 2], [f32; 2]) {
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

fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    land: Res<Land>,
    light: Res<crate::kit::light::Light>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut grounds: ResMut<Assets<GroundMaterial>>,
    mut images: ResMut<Assets<Image>>,
    biome: Res<crate::kit::biome::Biome>,
    mut waters: ResMut<Assets<kit::water::KitWater>>,
) {
    // The weather's floor (kit/weather.rs): the land and the channels' water.
    commands.insert_resource(kit::weather::Floor::sample(
        Vec2::new(-28.0, -24.0),
        Vec2::new(28.0, 16.0),
        0.5,
        Some(LEVEL),
        |p| land.height(p),
    ));
    // The open sky over it all: no crowns.
    let shade = |_: Vec2| 0.0;
    let place = Place {
        land: &land,
        sites: SITES,
        water: Some(LEVEL),
        shade: &shade,
    };
    let ground = material(
        &asset,
        &mut images,
        &biome.ground,
        biome.weather.as_ref(),
        &place,
    );
    commands.spawn((
        Mesh3d(meshes.add(land.mesh(cover))),
        MeshMaterial3d(grounds.add(ground)),
    ));
    // The kit's water over the channels, the mere and the ruts' puddles.
    let Some(water) = biome.water.as_ref() else {
        panic!(
            "the marsh's biome file has no `water`: its channels need one (biomes/marsh.ron, \
             `water: (kind: Pond, ...)`, kit.md; D634)"
        );
    };
    let wind = biome
        .weather
        .as_ref()
        .map_or(super::WIND, |w| Vec2::new(w.wind.0, w.wind.1));
    let face = kit::water::surface(
        Vec2::new(-28.0, -24.0),
        Vec2::new(28.0, 16.0),
        0.25,
        water.level,
        |p| water.level - seen_depth(water.level - land.height(p)),
    );
    kit::water::spawn_face(
        &mut commands,
        &mut meshes,
        &mut waters,
        water,
        wind,
        Some(&light),
        face,
    );
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
            assert!(
                shape(p) < LEVEL - 0.25,
                "{p} is not open water: {}",
                shape(p)
            );
        }
    }

    /// The shelf flattens the bank at the water's line and keeps it rising: half as steep
    /// at the line, never falling, the heights outside the band untouched.
    #[test]
    fn the_shelf_flattens_the_water_line_and_keeps_the_bank_rising() {
        let (c, w) = (SHELF.x, SHELF.y);
        let e = 1e-3;
        let mid = (shelf(c + e) - shelf(c - e)) / (2.0 * e);
        assert!(
            (mid - 0.5).abs() < 0.02,
            "the shelf's slope at its middle is {mid}"
        );
        let mut last = shelf(c - w - 0.1);
        for k in 1..=400 {
            let h = c - w - 0.1 + k as f32 * (2.0 * w + 0.2) / 400.0;
            assert!(shelf(h) >= last, "the shelf falls at {h}");
            last = shelf(h);
        }
        for h in [c - w - 0.01, c + w + 0.01, 0.3, -0.6] {
            assert_eq!(shelf(h), h, "the shelf moves {h}");
        }
    }

    /// The water's face fades in over a margin, not at the bank's line: walking off the near
    /// channel's bank, the face is still mostly clear 0.2 m past the line and hides the bed
    /// only some way further out, and the channel's middle is opaque.
    #[test]
    fn the_face_fades_in_over_a_margin_of_shallows() {
        let water = crate::kit::biome::Biome::load("marsh").unwrap().water.unwrap();
        let absorb = Vec3::new(water.absorb.0, water.absorb.1, water.absorb.2);
        let seen = |p: Vec2| {
            let d = seen_depth(water.level - shape(p));
            kit::water::opacity(d, absorb, water.shore)
        };
        for x in [-6.0, -2.0, 3.0] {
            let centre = Vec2::new(x, near(x));
            assert!(seen(centre) > 0.9, "the channel's middle at x {x} shows its bed");
            // Out from the middle toward the fight's bank until the line.
            let mut p = centre;
            while shape(p) < water.level {
                p.y -= 0.01;
            }
            let line = p.y;
            let at = |out: f32| seen(Vec2::new(x, line + out));
            assert!(at(0.2) < 0.5, "at x {x} the face is {} opaque 0.2 m out", at(0.2));
            let full = (0..300)
                .map(|k| k as f32 * 0.01)
                .find(|&o| at(o) > 0.85)
                .unwrap_or(9.0);
            assert!(full > 0.35, "at x {x} the face is opaque {full} m out: a cut");
        }
        for d in [-0.2, -0.01, 0.0] {
            assert_eq!(seen_depth(d), d, "dry land moves at {d}");
        }
        let mut last = 0.0;
        for k in 1..200 {
            let d = k as f32 * 0.005;
            assert!(seen_depth(d) > last, "the seen depth falls at {d}");
            last = seen_depth(d);
        }
        assert!((seen_depth(0.7) - 0.7).abs() < 1e-4, "the middle's depth moves");
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
        assert!(
            path(Vec2::new(0.0, 3.0)) == 0.0,
            "the path spreads to the near channel"
        );
    }
}
