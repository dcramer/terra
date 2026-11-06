//! Mist over the marsh's water: long low sheets lying along the channels and on the mere, a
//! hand to a knee over the face, each drifting slowly along its water and fading in and out,
//! thinned to nothing near the fight so no sheet lies over the fighters. Lit (the moon blue,
//! the lamps warm) and dark, since under a 1,100 lux moon anything brighter is a grey smudge.
//!
//! Bevy 0.20 sorts transparent things by their mesh's centre plus `depth_bias`, larger drawn
//! later (bevy_core_pipeline core_3d/mod.rs): the water's centre lies mid-frame and it is
//! near opaque, so a sheet behind that centre would draw first and the water hide it; the
//! sheets' bias draws them after it.

use super::dress::{FIRE, LANTERN};
use super::land::{self, MERE};
use super::{FOE, KNIGHT, LEVEL, TARGET, WIND};
use crate::forest::Rng;
use crate::ruin::effects::kit::{Clock, Noise, image, smooth};
use bevy::prelude::*;

/// How thick a sheet's streaks are at their thickest, its colour (linear), how far it drifts
/// before it fades, and the reach round each fighter it thins away over (m).
/// At 0.25 and (0.18, 0.24, 0.36) the sheets matched the mirrored sky's blue and did not
/// show (the frame's mean 0.0504 to 0.0507): paler and greyer than the water, as the refs'
/// veils (vr-fog-lightning). Sheets 2.4 to 3.2 m wide at 0.45, a soft veil, spilled over the
/// dark banks: "a shapeless milky haze" lifting p5 0.0063 to 0.0093 (blind judge, marsh-r12).
/// So narrow wisps wholly over the water, streaked and mostly clear.
const THICK: f32 = 0.55;
const COLOUR: Vec3 = Vec3::new(0.21, 0.25, 0.3);
const SPAN: f32 = 2.5;
const CLEAR: (f32, f32) = (2.4, 3.8);
const BIAS: f32 = 40.0;
/// Away from the lamps a wisp keeps this share of its thickness; within `LAMP.1` m of a
/// lamp it is whole, out to `LAMP.0` it thins: the refs' fog shows where a light catches it.
const UNLIT: f32 = 0.45;
const LAMP: (f32, f32) = (5.5, 1.5);
/// The tatter texture's size: long along the sheet, narrow across it.
const TEX: (u32, u32) = (256, 64);

/// Which water a sheet lies on, and where along it (x for a channel, an offset for the mere).
#[derive(Clone, Copy, Debug)]
enum On {
    Far(f32),
    Near(f32),
    Mere(Vec2),
}

const SHEETS: [On; 10] = [
    On::Far(-10.0),
    On::Far(-7.0),
    On::Far(1.5),
    On::Far(4.5),
    On::Far(7.5),
    On::Near(-8.0),
    On::Near(-6.0),
    On::Near(-4.0),
    On::Near(-2.0),
    // Its span centred on the mere's middle: from (0.5, 0.6) it drifted onto the shore.
    On::Mere(Vec2::ZERO),
];

impl On {
    /// The sheet's middle at the start of its span, and the way it lies and drifts.
    fn place(self) -> (Vec2, Vec2) {
        let along = |f: fn(f32) -> f32, x: f32| {
            let way = Vec2::new(0.2, f(x + 0.1) - f(x - 0.1)).normalize();
            (Vec2::new(x, f(x)) - way * SPAN * 0.5, way)
        };
        match self {
            On::Far(x) => along(land::far, x),
            On::Near(x) => along(land::near, x),
            On::Mere(o) => {
                let way = WIND.normalize();
                (MERE + o - way * self.span() * 0.5, way)
            }
        }
    }

    /// How far a sheet drifts, and its size against a channel's: the mere is round and
    /// small, so its wisp is shorter and drifts less (at a channel's size, 4.4 to 2.5 m out
    /// from its middle, it lay over the shore).
    fn span(self) -> f32 {
        match self {
            On::Mere(_) => 0.8,
            _ => SPAN,
        }
    }

    fn size(self) -> Vec2 {
        match self {
            On::Mere(_) => Vec2::new(0.5, 0.7),
            _ => Vec2::ONE,
        }
    }
}

#[derive(Component)]
struct Sheet {
    from: Vec2,
    way: Vec2,
    span: f32,
    y: f32,
    long: f32,
    wide: f32,
    speed: f32,
    phase: f32,
}

impl Sheet {
    /// Where the sheet's middle is `k` of the way along its span.
    fn at(&self, k: f32) -> Vec2 {
        self.from + self.way * self.span * k
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, drift);
}

fn sheets() -> Vec<Sheet> {
    let mut rng = Rng::new(0x415);
    SHEETS
        .iter()
        .map(|on| {
            let (from, way) = on.place();
            Sheet {
                from,
                way,
                span: on.span(),
                y: LEVEL + rng.range(0.08, 0.22),
                long: rng.range(3.5, 5.0) * on.size().x,
                wide: rng.range(1.0, 1.5) * on.size().y,
                speed: rng.range(0.06, 0.1),
                phase: rng.f(),
            }
        })
        .collect()
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let card = meshes.add(Rectangle::new(1.0, 1.0));
    let tatters: Vec<_> = (0..3).map(|i| images.add(tatter(0x3a0 + i))).collect();
    for (i, s) in sheets().into_iter().enumerate() {
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::NONE,
                base_color_texture: Some(tatters[i % 3].clone()),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 1.0,
                reflectance: 0.0,
                depth_bias: BIAS,
                ..default()
            })),
            Transform::default(),
            bevy::light::NotShadowCaster,
            s,
        ));
    }
}

/// A sheet's texture: white, its alpha from `tatters`.
fn tatter(seed: u32) -> Image {
    let a = tatters(seed);
    image(TEX.0, TEX.1, |x, y| {
        [1.0, 1.0, 1.0, a[(y * TEX.0 + x) as usize]]
    })
}

/// A sheet's alpha, row by row: noise drawn far out along the card's length into streaks,
/// cut hard so most of the card is clear, broken along its length into tatters, and fading
/// to nothing well inside its edge (an ellipse, so no corner shows).
fn tatters(seed: u32) -> Vec<f32> {
    let streak = Noise::new(32, seed);
    let fine = Noise::new(32, seed * 5 + 3);
    let gaps = Noise::new(32, seed * 7 + 11);
    let (w, h) = TEX;
    (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let p = Vec2::new((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
            let r = (p * 2.0 - 1.0).length();
            let n = 0.6 * streak.at(Vec2::new(p.x * 3.0, p.y * 12.0))
                + 0.4 * fine.at(Vec2::new(p.x * 7.0, p.y * 26.0));
            let torn = smooth(0.3, 0.6, gaps.at(Vec2::new(p.x * 5.0, p.y * 3.0)));
            smooth(0.95, 0.35, r) * smooth(0.55, 0.78, n) * torn
        })
        .collect()
}

/// How lit a wisp with its middle at `p` is by the lamps: 1 within `LAMP.1` of one.
fn lamp(p: Vec2) -> f32 {
    let d = [FIRE, LANTERN]
        .iter()
        .map(|l| l.distance(p))
        .fold(f32::MAX, f32::min);
    smooth(LAMP.0, LAMP.1, d)
}

/// How thick a sheet is `k` of the way along its span: in from nothing, out to nothing.
fn fade(k: f32) -> f32 {
    (std::f32::consts::PI * k).sin().powf(1.5)
}

/// How much of a sheet shows with its middle at `p`: none within `CLEAR.0` of a fighter.
fn clear(p: Vec2) -> f32 {
    let d = [KNIGHT, FOE, TARGET]
        .iter()
        .map(|f| f.distance(p))
        .fold(f32::MAX, f32::min);
    smooth(CLEAR.0, CLEAR.1, d)
}

fn drift(
    clock: Res<Clock>,
    mut sheets: Query<(&Sheet, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (s, mut tr, mat) in &mut sheets {
        let k = (clock.t * s.speed / s.span + s.phase).rem_euclid(1.0);
        let p = s.at(k);
        tr.translation = Vec3::new(p.x, s.y, p.y);
        tr.rotation = Quat::from_rotation_y((-s.way.y).atan2(s.way.x))
            * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        tr.scale = Vec3::new(s.long, s.wide, 1.0);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            let a = THICK * fade(k) * clear(p) * (UNLIT + (1.0 - UNLIT) * lamp(p));
            m.base_color = Color::linear_rgba(COLOUR.x, COLOUR.y, COLOUR.z, a);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every sheet lies wholly over open water along its whole span (its middle and its
    /// texture's ellipse at 0.8 of the way out, where the streaks still show), so no mist
    /// lies over the dark banks and lifts the blacks; and none comes within the clear reach
    /// of a fighter, so no sheet lies over the fight.
    #[test]
    fn sheets_lie_on_the_water_clear_of_the_fight() {
        let mut bank = Vec::new();
        for (n, s) in sheets().iter().enumerate() {
            let across = s.way.perp();
            for i in 0..=10 {
                let p = s.at(i as f32 / 10.0);
                assert!(
                    clear(p) > 0.0,
                    "sheet {n}'s middle at {p} lies over the fight"
                );
                let rim = (0..12).map(|j| {
                    let t = j as f32 / 12.0 * std::f32::consts::TAU;
                    p + 0.4 * (s.way * s.long * t.cos() + across * s.wide * t.sin())
                });
                for q in std::iter::once(p).chain(rim) {
                    if land::wet(q) <= 0.3 {
                        bank.push(format!("sheet {n} ({:?}) at {q}", SHEETS[n]));
                    }
                }
            }
        }
        assert!(bank.is_empty(), "mist over the bank: {}", bank.join("; "));
    }

    /// A sheet is mostly clear, in streaks drawn out along its length: its alpha changes
    /// far less over 8 px along the card than over 8 px across it, and under a third of it
    /// shows at all.
    #[test]
    fn tatters_are_sparse_streaks() {
        let (w, h) = TEX;
        for seed in 0x3a0..0x3a3 {
            let a = tatters(seed);
            let at = |x: u32, y: u32| a[(y * w + x) as usize];
            let shown = a.iter().filter(|&&v| v > 0.05).count() as f32 / a.len() as f32;
            assert!(
                (0.03..0.33).contains(&shown),
                "seed {seed}: {shown} of it shows"
            );
            let (mut along, mut over) = (0.0, 0.0);
            for y in 0..h - 8 {
                for x in 0..w - 8 {
                    along += (at(x + 8, y) - at(x, y)).abs();
                    over += (at(x, y + 8) - at(x, y)).abs();
                }
            }
            assert!(
                along < 0.5 * over,
                "seed {seed}: along {along}, across {over}"
            );
        }
    }

    /// A wisp by a lamp is whole; far from both it keeps only `UNLIT`.
    #[test]
    fn wisps_thin_away_from_the_lamps() {
        assert!((lamp(FIRE + Vec2::X) - 1.0).abs() < 1e-5);
        assert!(lamp(Vec2::new(-10.0, -5.0)) < 1e-5);
    }

    /// A sheet comes and goes from nothing: none pops in or out at its span's ends.
    #[test]
    fn sheets_fade_at_their_ends() {
        assert!(fade(0.0) < 1e-3 && fade(0.999) < 0.01);
        assert!((fade(0.5) - 1.0).abs() < 1e-5);
    }
}
