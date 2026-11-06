//! Mist over the marsh's water: long low sheets lying along the channels and on the mere, a
//! hand to a knee over the face, each drifting slowly along its water and fading in and out,
//! thinned to nothing near the fight so no sheet lies over the fighters. Lit (the moon blue,
//! the lamps warm) and dark, since under a 1,100 lux moon anything brighter is a grey smudge.
//!
//! Bevy 0.20 sorts transparent things by their mesh's centre plus `depth_bias`, larger drawn
//! later (bevy_core_pipeline core_3d/mod.rs): the water's centre lies mid-frame and it is
//! near opaque, so a sheet behind that centre would draw first and the water hide it; the
//! sheets' bias draws them after it.

use super::land::{self, MERE};
use super::{FOE, KNIGHT, LEVEL, TARGET, WIND};
use crate::forest::Rng;
use crate::ruin::effects::kit::{Clock, Noise, image, smooth};
use bevy::prelude::*;

/// How thick a sheet is at its thickest, its colour (linear), how far it drifts before it
/// fades, and the reach round each fighter it thins away over (m).
/// At 0.25 and (0.18, 0.24, 0.36) the sheets matched the mirrored sky's blue and did not
/// show (the frame's mean 0.0504 to 0.0507): paler and greyer than the water, as the refs'
/// veils (vr-fog-lightning), and wide enough to spill over the dark banks where they read.
const THICK: f32 = 0.45;
const COLOUR: Vec3 = Vec3::new(0.21, 0.25, 0.3);
const SPAN: f32 = 2.5;
const CLEAR: (f32, f32) = (2.4, 3.8);
const BIAS: f32 = 40.0;

/// Which water a sheet lies on, and where along it (x for a channel, an offset for the mere).
#[derive(Clone, Copy, Debug)]
enum On {
    Far(f32),
    Near(f32),
    Mere(Vec2),
}

const SHEETS: [On; 9] = [
    On::Far(-10.0),
    On::Far(-7.0),
    On::Far(1.5),
    On::Far(4.5),
    On::Far(7.5),
    On::Near(-7.0),
    On::Near(-4.5),
    On::Near(-2.0),
    // Its span centred near the mere's middle: from (0.5, 0.6) it drifted onto the shore.
    On::Mere(Vec2::new(-0.2, 0.4)),
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
            On::Mere(o) => (MERE + o - WIND.normalize() * SPAN * 0.5, WIND.normalize()),
        }
    }
}

#[derive(Component)]
struct Sheet {
    from: Vec2,
    way: Vec2,
    y: f32,
    long: f32,
    wide: f32,
    speed: f32,
    phase: f32,
}

impl Sheet {
    /// Where the sheet's middle is `k` of the way along its span.
    fn at(&self, k: f32) -> Vec2 {
        self.from + self.way * SPAN * k
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
                y: LEVEL + rng.range(0.08, 0.22),
                long: rng.range(4.5, 6.5),
                wide: rng.range(2.4, 3.2),
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

/// A sheet's texture: soft noise drawn out along the card's length and thinned to tatters,
/// fading to nothing well inside its edge (an ellipse, so no corner shows).
fn tatter(seed: u32) -> Image {
    let big = Noise::new(4, seed);
    let fine = Noise::new(12, seed * 5 + 3);
    let (w, h) = (192, 64);
    image(w, h, |x, y| {
        let p = Vec2::new((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
        let r = (p * 2.0 - 1.0).length();
        let n = 0.7 * big.at(Vec2::new(p.x * 4.0, p.y * 2.0))
            + 0.3 * fine.at(Vec2::new(p.x * 12.0, p.y * 4.0));
        let a = smooth(0.98, 0.4, r) * smooth(0.25, 0.7, n);
        [1.0, 1.0, 1.0, a]
    })
}

/// How thick a sheet is `k` of the way along its span: in from nothing, out to nothing.
fn fade(k: f32) -> f32 {
    (std::f32::consts::PI * k).sin().powf(1.5)
}

/// How much of a sheet shows with its middle at `p`: none within `CLEAR.0` of a fighter.
fn clear(p: Vec2) -> f32 {
    let d = [KNIGHT, FOE, TARGET].iter().map(|f| f.distance(p)).fold(f32::MAX, f32::min);
    smooth(CLEAR.0, CLEAR.1, d)
}

fn drift(
    clock: Res<Clock>,
    mut sheets: Query<(&Sheet, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (s, mut tr, mat) in &mut sheets {
        let k = (clock.t * s.speed / SPAN + s.phase).rem_euclid(1.0);
        let p = s.at(k);
        tr.translation = Vec3::new(p.x, s.y, p.y);
        tr.rotation = Quat::from_rotation_y((-s.way.y).atan2(s.way.x))
            * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        tr.scale = Vec3::new(s.long, s.wide, 1.0);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            let a = THICK * fade(k) * clear(p);
            m.base_color = Color::linear_rgba(COLOUR.x, COLOUR.y, COLOUR.z, a);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every sheet lies over open water along its whole span, and none comes within the
    /// clear reach of a fighter, so no sheet of mist lies over the fight.
    #[test]
    fn sheets_lie_on_the_water_clear_of_the_fight() {
        for s in sheets() {
            for i in 0..=10 {
                let p = s.at(i as f32 / 10.0);
                assert!(land::wet(p) > 0.3, "a sheet's middle at {p} is on the bank");
                assert!(clear(p) > 0.0, "a sheet's middle at {p} lies over the fight");
            }
        }
    }

    /// A sheet comes and goes from nothing: none pops in or out at its span's ends.
    #[test]
    fn sheets_fade_at_their_ends() {
        assert!(fade(0.0) < 1e-3 && fade(0.999) < 0.01);
        assert!((fade(0.5) - 1.0).abs() < 1e-5);
    }
}
