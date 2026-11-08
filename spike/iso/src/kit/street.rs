//! The kit's street (layout `Street`, D634): a road running east and west through the origin
//! between two kerbs, a pavement each side, and building fronts beyond them, all as the
//! biome's `street` says. The road and the kerbs are laid floors (kit/laid.rs: a scan at its
//! true size, weathered per pixel), the pavements are the biome's `ground.laid`; the fronts
//! are blocks of a scan with storeys of windows, some lit; street lamps hang sodium spots
//! over the road; signs are emissive cards, each with a coloured light so it lights the
//! ground; props stand by the kerb, the wall or out on the road. Nothing here is a genre's:
//! a neon city at night and a dusty frontier town are the same fields.
//!
//! North is -z, so the camera (south of the street, looking north) sees the north fronts'
//! faces and the south fronts' roofs. The paving carries three maps for the laid floors
//! (`SITES`): `gutter` (by the kerb's face, where the road's camber runs the rain),
//! `puddled` (low patches where water stands) and `walls` (where a wall or kerb rises).
//! The biome's `water` stands in the `puddled` patches as the kit's water (kit/water.rs),
//! its `level` over the road and the pavements: the mirror and the lights' streaks.
//!
//! Here the layout and its maps; the build in `paving` (camera, light, paving, puddles,
//! props), the fronts, lamps and signs in `dress`, the scatter in `strew`.

mod dress;
mod paving;
mod strew;

use super::biome::Laid;
use super::light::Colour;
use crate::terrain::{fbm, smooth};
use bevy::prelude::*;
use serde::Deserialize;

/// The street's maps for the laid floors, in the second UV's x and y and the colour's alpha.
pub const SITES: [&str; 3] = ["gutter", "puddled", "walls"];
/// The paving's mesh spacing, metres.
const CELL: f32 = 0.25;

/// A street: the road's width between the kerbs' faces, each pavement's, the kerb's width
/// and rise, and how long it all runs (centred on the origin, along x).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Street {
    pub road: f32,
    pub walk: f32,
    pub kerb: (f32, f32),
    pub length: f32,
    /// The road's surface (asphalt: a laid scan with no joints).
    pub asphalt: Laid,
    /// The kerbstones' surface.
    pub kerbs: Laid,
    /// Where the camera looks, (x, z): the fight's middle.
    #[serde(default)]
    pub focus: (f32, f32),
    /// After rain, how deep (m) a film of the biome's water lies over all the paving between
    /// the puddles (0: only the puddles), mirroring at the shore's fade of that depth.
    #[serde(default)]
    pub film: f32,
    /// Where water stands: the patches' noise (0 to 1) over which a puddle lies, so lower
    /// is more puddles (0.64 about a tenth of the street; iso-kit j46).
    #[serde(default = "puddle_level")]
    pub puddles: f32,
    /// Wires strung across and along the street and pipes down its fronts (kit/cables.rs).
    #[serde(default)]
    pub cables: Vec<super::cables::Cable>,
    #[serde(default)]
    pub fronts: Vec<Front>,
    #[serde(default)]
    pub lamps: Option<Lamps>,
    #[serde(default)]
    pub signs: Vec<Sign>,
    #[serde(default)]
    pub props: Vec<Prop>,
}

/// Which side of the road: north (-z, its fronts face the camera) or south.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    North,
    South,
}

impl Side {
    /// Its sign along z.
    pub fn z(self) -> f32 {
        match self {
            Side::North => -1.0,
            Side::South => 1.0,
        }
    }
    /// The turn about y that faces a thing on this side toward the road (models face +z).
    pub(crate) fn facing(self) -> f32 {
        match self {
            Side::North => 0.0,
            Side::South => std::f32::consts::PI,
        }
    }
}

/// A block of building along one side, from x `along.0` to `along.1`, `height` tall and
/// `depth` deep, faced in a scan of `assets/materials/` at its true `metres`; its roof a scan
/// too (the front's own when `None`). Its face to the road is either the scan with `windows`
/// or a facade kit's `bays` (kit/facade.rs), the block's face then set back dark behind them.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Front {
    pub side: Side,
    pub along: (f32, f32),
    pub height: f32,
    #[serde(default = "deep")]
    pub depth: f32,
    pub material: String,
    pub metres: f32,
    #[serde(default = "white")]
    pub tint: (f32, f32, f32),
    #[serde(default)]
    pub roof: Option<String>,
    #[serde(default)]
    pub windows: Option<Windows>,
    #[serde(default)]
    pub bays: Option<super::facade::Bays>,
}

/// A front's windows: a storey's height, the first sill's height, one every `every` metres
/// along, each `size` (wide, tall); `lit` of them glowing in one of `colours` at `nits`, the
/// rest dark glass.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Windows {
    pub storey: f32,
    pub from: f32,
    pub every: f32,
    pub size: (f32, f32),
    pub lit: f32,
    pub colours: Vec<Colour>,
    pub nits: f32,
}

/// Street lamps over the road: one every `every` metres from x `start`, sides taking turns
/// from the north, a pole `height` tall by the kerb with an arm `reach` out over the road,
/// a spot of `colour` and `lumens` down through `angle` degrees, `range` metres.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Lamps {
    pub every: f32,
    pub start: f32,
    pub height: f32,
    pub reach: f32,
    pub colour: Colour,
    pub lumens: f32,
    pub angle: f32,
    pub range: f32,
    #[serde(default)]
    pub shadows: bool,
}

/// A sign: a texture (under `assets/`) on a card `size` metres (wide, tall), its middle at x
/// `at` and `height` up a front of `side`, flat on the wall or a `blade` standing out from
/// it; it glows at `nits` and lights the street around it in `colour` at `lumens`, out to
/// `range` metres (a short one makes a pool that falls off to dark), its light `shadows`
/// cast or not. With a `pool` (cone degrees, metres out from the wall, lumens) its light on
/// the street is a spot from the sign aimed down at the pavement that far out, and `lumens`
/// and `range` only its glow on the wall about it, so the face falls dark above and beside
/// it instead of taking the sign's colour whole (iso-kit j42: "neon floods the face").
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Sign {
    pub texture: String,
    pub side: Side,
    pub at: f32,
    pub height: f32,
    pub size: (f32, f32),
    #[serde(default)]
    pub blade: bool,
    pub colour: Colour,
    pub nits: f32,
    pub lumens: f32,
    #[serde(default = "nine")]
    pub range: f32,
    #[serde(default)]
    pub shadows: bool,
    #[serde(default)]
    pub pool: Option<(f32, f32, f32)>,
}

fn nine() -> f32 {
    9.0
}

fn puddle_level() -> f32 {
    0.64
}

/// Where a prop stands across the street: by the kerb on the pavement, against the wall, or
/// on the road `out` metres from its middle toward its side.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    Kerb,
    Wall,
    Road,
}

/// A model (under `assets/`) at each x of `at` on `side`'s `line`, moved `out` metres away
/// from the road (into it when negative; from the middle on `Road`), turned `turn` degrees
/// from facing the road, scaled `scale`; a rigged one (a person on the pavement) held in its
/// `pose`, a clip of its GLB by name and the time in it (the ruin's `Pose`), and edged by
/// `rim` times the fighters' cold rim band (rim.rs; none at 0), so a dark figure reads off
/// the dark wet street as the ARPGs lift theirs. Its `walk` is a film's (ruin/walk.rs); a
/// still holds it where it stands. One only in a `film` stands in no still and in no play
/// (play stages its own foes, play.rs): the street's fight, which a still has none of.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Prop {
    pub model: String,
    pub side: Side,
    pub line: Line,
    pub at: Vec<f32>,
    #[serde(default)]
    pub out: f32,
    #[serde(default)]
    pub turn: f32,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default)]
    pub pose: Option<(String, f32)>,
    #[serde(default)]
    pub rim: f32,
    #[serde(default)]
    pub walk: Option<Walk>,
    #[serde(default)]
    pub film: bool,
    /// Metres up from its line's floor: a wall-hung thing (an air conditioner under a
    /// window, a payphone, a lamp over a door), with no contact shadow under it.
    #[serde(default)]
    pub up: f32,
}

/// A rigged prop's walk through a film, `from` and `to` (x, z) metres from where it stands:
/// a stroll from one to the other at the walk's pace, turned its way, its pose its walk (the
/// street's people, kept in the frame for the film's 16 s: walking on from where they
/// stand, they left it by 5 s, the desk's film of round 261107-1627); or, to `fight`, in
/// from `from` to its place, turned to where it faces, then its pose (the hero, as the
/// ruin's walks in).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Walk {
    pub from: (f32, f32),
    #[serde(default)]
    pub to: (f32, f32),
    #[serde(default)]
    pub fight: bool,
}

fn deep() -> f32 {
    12.0
}

fn white() -> (f32, f32, f32) {
    (1.0, 1.0, 1.0)
}

fn one() -> f32 {
    1.0
}

impl Street {
    /// Half the road, the kerb's outer edge, and the wall line (the pavements' outer edge).
    pub fn edges(&self) -> (f32, f32, f32) {
        let half = self.road / 2.0;
        (half, half + self.kerb.0, half + self.kerb.0 + self.walk)
    }

    /// The lamp poles' feet (lamps() and play's ground): from `start` every `every` along
    /// the street, north and south by turns, just out from the kerb; and their radius.
    pub fn poles(&self) -> (Vec<(Side, Vec3)>, f32) {
        let Some(lp) = &self.lamps else {
            return (Vec::new(), 0.0);
        };
        let (_, kerb, _) = self.edges();
        let mut out = Vec::new();
        let mut x = lp.start;
        while x <= self.length / 2.0 {
            let side = [Side::North, Side::South][out.len() % 2];
            out.push((side, Vec3::new(x, self.kerb.1, side.z() * (kerb + 0.35))));
            x += lp.every;
        }
        (out, POLE)
    }

    /// Where a prop's foot stands on `side`'s `line`, `out` metres out.
    pub fn spot(&self, side: Side, line: Line, x: f32, out: f32) -> Vec3 {
        let (_, kerb, wall) = self.edges();
        let (across, y) = match line {
            Line::Kerb => (kerb + 0.4 + out, self.kerb.1),
            Line::Wall => (wall - 0.35 - out, self.kerb.1),
            Line::Road => (out, 0.0),
        };
        Vec3::new(x, y, side.z() * across)
    }

    /// The street as laid out, or what is wrong and the fix (D556): every sign on a front of
    /// its side and inside it, every prop inside the street's length and off the road unless
    /// it stands on it.
    pub fn check(&self) -> Result<(), String> {
        let ends = self.length / 2.0;
        for f in &self.fronts {
            let Some(b) = &f.bays else { continue };
            if f.windows.is_some() {
                return Err(format!(
                    "the front from x {} to {} has both `windows` and `bays`: its face is one \
                     or the other. Fix: drop its `windows` (the bays' `lit` and `colours` light \
                     its rooms; kit/street.rs, D634)",
                    f.along.0, f.along.1
                ));
            }
            b.check(f.along, f.height)?;
        }
        for s in &self.signs {
            let (w, h) = (s.size.0 / 2.0, s.size.1 / 2.0);
            let on = self.fronts.iter().any(|f| {
                f.side == s.side
                    && f.along.0 <= s.at - w.min(0.3)
                    && s.at + w.min(0.3) <= f.along.1
                    && s.height + h <= f.height
                    && s.height - h >= 0.0
            });
            if !on {
                return Err(format!(
                    "sign {:?} at x {} and {} m up on the {:?} side hangs on no front: each sign \
                     goes on a front of its side, inside its span and under its height. Fix: \
                     move its `at` or `height`, or add a front there (kit/street.rs, D634)",
                    s.texture, s.at, s.height, s.side
                ));
            }
        }
        for p in &self.props {
            for x in &p.at {
                let far = p.line == Line::Road && p.out.abs() > self.road / 2.0;
                if x.abs() > ends || far {
                    return Err(format!(
                        "prop {:?} at x {x} (out {}) lies off the street ({} m long, the road \
                         {} m wide). Fix: an x within ±{ends} and a road prop's out within \
                         ±{} (kit/street.rs, D634)",
                        p.model,
                        p.out,
                        self.length,
                        self.road,
                        self.road / 2.0
                    ));
                }
            }
        }
        for c in &self.cables {
            c.check()?;
        }
        Ok(())
    }

    /// How near the kerb's face a point of the road is: 1 at the face, gone 0.9 m out, its
    /// edge ragged; 0 off the road.
    pub fn gutter(&self, p: Vec2) -> f32 {
        let (half, _, _) = self.edges();
        if p.y.abs() > half {
            return 0.0;
        }
        let d = half - p.y.abs() + (fbm(p * 0.7, 401) - 0.5) * 0.5;
        smooth(0.9, 0.05, d)
    }

    /// Where water stands: low patches over the street, more of them in the gutters.
    pub fn puddled(&self, p: Vec2) -> f32 {
        let patch = fbm(p * 0.32 + Vec2::splat(17.0), 402);
        let lower = 0.08 * self.gutter(p);
        smooth(self.puddles, self.puddles + 0.06, patch + lower)
    }

    /// How deep water stands at a point (m), as the water's face reads it: up to 6 cm in a
    /// puddle's middle, 0 at its edge (`puddled` a half), the `film` between them; with no
    /// film under the face's cut (5 cm) on the dry, so no face is drawn there.
    pub fn standing(&self, p: Vec2) -> f32 {
        let puddle = (self.puddled(p) - 0.5) * 0.12;
        if self.film > 0.0 {
            puddle.max(self.film)
        } else {
            puddle
        }
    }

    /// How near a rising wall a point is: a front's foot on the pavements, the kerb's face on
    /// the road; 1 at the foot, gone a metre out.
    pub fn walls(&self, p: Vec2) -> f32 {
        let (half, kerb, wall) = self.edges();
        let a = p.y.abs();
        let d = if a <= half {
            half - a
        } else if a >= kerb {
            wall - a
        } else {
            9.0
        };
        smooth(1.0, 0.0, d + (fbm(p * 0.9, 403) - 0.5) * 0.4)
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins((
        super::laid::plugin,
        super::water::plugin,
        super::scatter::plugin,
        super::weather::plugin,
        super::facade::plugin,
        super::contact::plugin,
        crate::ruin::walk::plugin,
        // The plant scatter's cards (dry grass in a wasteland's gutters).
        crate::foliage::plugin,
    ))
    .add_systems(Startup, (paving::spawn, strew::strew))
    // Rigged props stand in their clips (the ruin's poser).
    .add_systems(Update, crate::ruin::fight::pose);
}

/// A lamp pole's radius (m).
const POLE: f32 = 0.07;

#[cfg(test)]
mod tests {
    use super::strew::with_ground;
    use super::*;
    use crate::kit::biome::Biome;

    fn neon() -> Street {
        Biome::load("neon-street").unwrap().street.unwrap()
    }

    /// The neon street lays out: its signs hang on fronts, its props lie on the street.
    #[test]
    fn the_neon_street_lays_out() {
        neon().check().unwrap_or_else(|e| panic!("{e}"));
    }

    /// The wasteland's dry grass is put down on its street (uc-wasteland j3: the street
    /// drew no plant set), in the gutters and at the walls' feet, never mid-road.
    #[test]
    fn a_streets_plant_sets_come_up_where_their_maps_lie() {
        let biome = Biome::load("wasteland").unwrap();
        let st = biome.street.as_ref().unwrap();
        st.check().unwrap_or_else(|e| panic!("{e}"));
        let mut budget = super::super::scatter::Budget::default();
        let clumps = with_ground(st, |g| {
            super::super::scatter::plants(&mut budget, &biome.refs, &biome.scatter, g)
        });
        assert!(clumps.len() > 50, "only {} clumps", clumps.len());
        let (half, _, _) = st.edges();
        for c in &clumps {
            assert!(c.at.y.abs() > half - 1.2, "a clump mid-road at {}", c.at);
        }
    }

    /// A sign off every front fails, naming the sign and the fix.
    #[test]
    fn a_sign_off_the_fronts_says_the_fix() {
        let mut st = neon();
        st.signs[0].at = st.length * 4.0;
        let e = st.check().unwrap_err();
        assert!(
            e.contains(&st.signs[0].texture) && e.contains("Fix:"),
            "{e}"
        );
    }

    /// The gutter lies along the kerb's face and nowhere near the road's middle or on the
    /// pavement; the walls' map at the fronts' feet and the kerb's, not mid-pavement.
    #[test]
    fn gutters_and_wall_feet_lie_where_they_rise() {
        let st = neon();
        let (half, kerb, wall) = st.edges();
        for x in [-9.0, -2.5, 0.0, 4.0, 11.0] {
            for s in [-1.0, 1.0] {
                assert!(st.gutter(Vec2::new(x, s * (half - 0.05))) > 0.6);
                assert_eq!(st.gutter(Vec2::new(x, 0.0)), 0.0);
                assert_eq!(st.gutter(Vec2::new(x, s * (kerb + 0.5))), 0.0);
                assert!(st.walls(Vec2::new(x, s * (wall - 0.05))) > 0.6);
                assert!(st.walls(Vec2::new(x, s * (half - 0.05))) > 0.6);
                assert_eq!(st.walls(Vec2::new(x, s * (kerb + st.walk / 2.0))), 0.0);
            }
        }
    }

    /// Water stands in some patches and not most of the street.
    #[test]
    fn puddles_are_patches() {
        let st = neon();
        let mut wet = 0;
        let n = 2000;
        for k in 0..n {
            let p = Vec2::new((k % 50) as f32 * 0.7 - 17.0, (k / 50) as f32 * 0.4 - 8.0);
            if st.puddled(p) > 0.5 {
                wet += 1;
            }
        }
        let share = wet as f32 / n as f32;
        assert!((0.03..0.4).contains(&share), "puddles cover {share}");
    }

    /// A film wets the whole paving at its depth and the puddles stand deeper; with none,
    /// the dry lies under the water face's cut.
    #[test]
    fn a_film_lies_between_the_puddles() {
        let mut st = neon();
        st.film = 0.0;
        let ps: Vec<Vec2> = (0..2000)
            .map(|k| Vec2::new((k % 50) as f32 * 0.7 - 17.0, (k / 50) as f32 * 0.4 - 8.0))
            .collect();
        let dry = ps
            .iter()
            .find(|p| st.puddled(**p) < 0.01)
            .expect("a dry point");
        let wet = ps
            .iter()
            .find(|p| st.puddled(**p) > 0.99)
            .expect("a puddle");
        assert!(st.standing(*dry) < -0.05);
        st.film = 0.008;
        assert_eq!(st.standing(*dry), 0.008);
        assert!(st.standing(*wet) > 0.05);
    }
}
