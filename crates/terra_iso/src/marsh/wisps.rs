//! Will-o'-wisps and midges: cold orbs drifting low over the channels, each the `wisp`
//! preset's glow (kit/effects.rs: a hot core in a soft halo facing the camera, no disc's
//! edge) and its light, mirrored in the water (`kit::water::Glint`),
//! swelling and fading on its own slow beat with a flicker through it; and clouds of midges
//! dancing over the reeds, catching the light.

use super::LEVEL;
use super::land::{far, near};
use crate::forest::Rng;
use crate::kit::effects::{Glow, Look, Preset};
use crate::kit::water::Glint;
use crate::motes::soft_image;
use crate::ruin::effects::kit::{Cam, Clock, glow};
use bevy::prelude::*;

/// The wisps' homes (x, z), each over a water's margin so its light pools on the mud and
/// reeds as well as glinting, their height over the water and how far they drift.
const HOMES: [Vec2; 4] = [
    Vec2::new(-5.6, 1.4),
    Vec2::new(1.0, 2.7),
    Vec2::new(-4.6, -3.6),
    Vec2::new(4.0, -3.5),
];
const HIGH: f32 = 0.5;
const DRIFT: f32 = 0.6;
/// A wisp's beat (s); its look and light (low and bright enough to lay a faint pool on the
/// bank and the reeds under it) are the `wisp` preset's.
const BEAT: f32 = 6.0;
/// Midge clouds, the specks in each, and their size (m: under 0.05 they vanish at 960 px).
const CLOUDS: usize = 5;
const MIDGES: usize = 26;
const MIDGE: f32 = 0.05;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn)
        .add_systems(Update, (float, dance));
}

/// One wisp's parts: the glow card and the light, moved with the core.
#[derive(Component)]
struct Wisp {
    home: Vec3,
    phase: f32,
    part: Part,
}

enum Part {
    /// The glow's material (its strength follows the beat) and the quad's size (m).
    Glow(Handle<Glow>, f32),
    /// The light's lumens at full beat.
    Light(f32),
}

#[derive(Component)]
struct Midge {
    home: Vec3,
    phase: Vec3,
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut glows: ResMut<Assets<Glow>>,
) {
    let soft = images.add(soft_image());
    let card = meshes.add(Rectangle::new(1.0, 1.0));
    // A bare core is a dot at 960 px (iso-art j19), a 1.3 m orb 70 px of lens bokeh, not a
    // light (blind-marsh3), a soft card over a hot sphere a flat disc (every marsh judge,
    // 2026-11-06): the kit's glow, a narrow core in a Gaussian halo windowed to nothing.
    let preset = Preset::load("wisp").unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    let (Some(flash), Some(lamp)) = (preset.flash, preset.lamps.first()) else {
        eprintln!(
            "marsh: the `wisp` preset (crates/terra_iso/biomes/effects/wisp.ron) needs a `flash` and a \
             lamp: wisps.rs draws the one and lights the other (D634)"
        );
        std::process::exit(2)
    };
    for (i, h) in HOMES.iter().enumerate() {
        let home = h.extend(LEVEL + HIGH).xzy();
        let phase = i as f32 * 1.7;
        let look = glows.add(Glow {
            look: Look::flash(&flash, 1.0),
        });
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(look.clone()),
            Transform::from_translation(home),
            bevy::light::NotShadowCaster,
            Wisp {
                home,
                phase,
                part: Part::Glow(look, flash.size),
            },
        ));
        commands.spawn((
            PointLight {
                color: lamp.colour.color(),
                intensity: lamp.lm,
                range: lamp.range,
                radius: 0.05,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(home),
            Wisp {
                home,
                phase,
                part: Part::Light(lamp.lm),
            },
            Glint,
        ));
    }
    let mut rng = Rng::new(0x31d9e);
    let speck = materials.add(glow(LinearRgba::rgb(0.5, 0.48, 0.38), Some(soft)));
    for c in 0..CLOUDS {
        let x = -7.0 + 3.4 * c as f32;
        // Over the water's edges, a cloud to each channel by turns.
        let z = if c % 2 == 0 {
            near(x) - 1.4
        } else {
            far(x) + 1.3
        };
        for _ in 0..MIDGES {
            let home = Vec3::new(
                x + rng.range(-0.5, 0.5),
                LEVEL + rng.range(0.5, 1.4),
                z + rng.range(-0.5, 0.5),
            );
            commands.spawn((
                Mesh3d(card.clone()),
                MeshMaterial3d(speck.clone()),
                Transform::from_translation(home).with_scale(Vec3::splat(MIDGE)),
                bevy::light::NotShadowCaster,
                Midge {
                    home,
                    phase: Vec3::new(
                        rng.range(0.0, 6.3),
                        rng.range(0.0, 6.3),
                        rng.range(0.0, 6.3),
                    ),
                },
            ));
        }
    }
}

/// How bright a wisp is at time `t`: swelling and dying on its beat, a flicker through it.
pub fn bright(t: f32, phase: f32) -> f32 {
    let k = libm::sinf((t / BEAT + phase) * std::f32::consts::TAU) * 0.5 + 0.5;
    let flick = 0.75 + 0.25 * libm::sinf(t * 17.0 + phase * 5.0) * libm::sinf(t * 7.3 + phase);
    (0.25 + 0.75 * k * k) * flick
}

fn float(
    clock: Res<Clock>,
    cam: Res<Cam>,
    mut glows: ResMut<Assets<Glow>>,
    mut wisps: Query<(&Wisp, &mut Transform, Option<&mut PointLight>)>,
) {
    let t = clock.t;
    for (w, mut tr, light) in &mut wisps {
        let p = w.phase;
        let at = w.home
            + Vec3::new(
                libm::sinf(t * 0.21 + p) * DRIFT,
                libm::sinf(t * 0.9 + p * 2.0) * 0.12,
                libm::sinf(t * 0.17 + p * 1.3) * DRIFT * 0.6,
            );
        let k = bright(t, p);
        tr.translation = at;
        match &w.part {
            Part::Glow(look, size) => {
                tr.rotation = cam.rot;
                tr.scale = Vec3::splat(size * (0.7 + 0.3 * k));
                if let Some(mut g) = glows.get_mut(look) {
                    g.look.shape.x = k.min(1.0);
                }
            }
            Part::Light(lm) => {
                if let Some(mut l) = light {
                    l.intensity = lm * k;
                }
            }
        }
    }
}

/// The midges' tight dance about their homes, each speck facing the camera.
fn dance(clock: Res<Clock>, cam: Res<Cam>, mut midges: Query<(&Midge, &mut Transform)>) {
    let t = clock.t;
    for (m, mut tr) in &mut midges {
        let p = m.phase;
        tr.translation = m.home
            + Vec3::new(
                libm::sinf(t * 3.1 + p.x) + 0.5 * libm::sinf(t * 7.7 + p.y),
                libm::sinf(t * 2.3 + p.y) * 0.6,
                libm::sinf(t * 2.9 + p.z) + 0.5 * libm::sinf(t * 6.1 + p.x),
            ) * 0.22;
        tr.rotation = cam.rot;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wisp never quite goes out and never passes its full light.
    #[test]
    fn a_wisp_fades_but_never_dies() {
        let ks: Vec<f32> = (0..600).map(|i| bright(i as f32 * 0.05, 0.3)).collect();
        let (lo, hi) = ks
            .iter()
            .fold((9.0f32, 0.0f32), |(a, b), k| (a.min(*k), b.max(*k)));
        assert!(lo > 0.1 && lo < 0.35, "dimmest {lo}");
        assert!(hi > 0.8 && hi <= 1.0, "brightest {hi}");
    }
}
