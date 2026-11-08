//! The ruin's fires: two iron braziers flanking the fight, their light the scene's key, and
//! torches on the back wall's face and a pillar, each a pool of warm light in the moonlit
//! dark. The flames are the glade's (motes.rs): tongue cards licking up and falling back, a
//! halo of lit air, embers rising; each light wavers as scene.rs's fire does.

use super::walls::{BACK_Z, PILLARS, THICK};
use crate::forest::Rng;
use crate::motes::{Halo, Tongue, soft_image, tongue_image};
use crate::scene::model;
use bevy::prelude::*;

/// The braziers' bowls, where their flames stand: each about 2 m from a fighter, so the
/// fight stands in their pools, the knight's lit from before him on the left, the ogre's
/// from behind on the right (a warm edge on it against the dark beyond).
pub const BRAZIERS: [Vec2; 2] = [Vec2::new(-1.7, 0.9), Vec2::new(3.4, -2.9)];
/// A brazier's light, and a torch's, in lumens.
pub const BRAZIER_LM: f32 = 140_000.0;
pub const TORCH_LM: f32 = 25_000.0;
const BOWL: f32 = 0.9;

/// The braziers' and torches' sparks and smoke (flame.rs draws them): a few embers a second
/// off each, 7 cm hot streaks (under 7 cm a mote does not show at 960 px, scene.md) thrown
/// 1.5 to 3 m up and off on the night's wind; and a thin column of smoke, 25 cm across at
/// the flame's tip and a metre and a half at its top 3 m up, warm where the fire lights it
/// from below, leaning on the wind. A column darker than the floor did not show (ruin-r16:
/// it rises off the pool over the night's black flags), so it is a dim grey over the floor.
pub fn plume() -> crate::ruin::effects::Plume {
    let wind = super::ambient::WIND * 0.36;
    crate::ruin::effects::Plume {
        embers: 18.0,
        spark: 0.07,
        streak: (0.07, 0.24),
        trail: 0.12,
        rise: (1.2, 2.0),
        life: (1.8, 3.2),
        puffs: 26.0,
        smoke_life: (4.5, 6.0),
        wide: (0.36, 1.8),
        climb: 0.55,
        alpha: 0.48,
        lit: Vec3::new(0.42, 0.17, 0.05),
        dark: Vec3::new(0.05, 0.045, 0.045),
        cool: 0.55,
        wind,
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (crate::motes::lick, crate::motes::face_halo, rise, flicker),
    );
}

#[derive(Component)]
struct Flicker {
    lumens: f32,
    seed: f32,
}

/// An ember rising off a fire from `base`.
#[derive(Component)]
struct Ember {
    base: Vec3,
    born: f32,
    life: f32,
    drift: Vec2,
    seed: f32,
}

pub fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut rng = Rng::new(0xb2a21e2);
    let lit = |c: LinearRgba| StandardMaterial {
        base_color: c.into(),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        ..default()
    };
    let parts = Parts {
        card: meshes.add(Rectangle::new(1.0, 1.0)),
        speck: meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
        flame: materials.add(StandardMaterial {
            base_color_texture: Some(images.add(tongue_image())),
            alpha_mode: AlphaMode::Blend,
            ..lit(LinearRgba::rgb(1.8, 1.8, 1.8))
        }),
        ember: materials.add(lit(LinearRgba::rgb(4.0, 1.1, 0.08))),
        halo: {
            let soft = images.add(soft_image());
            materials.add(StandardMaterial {
                base_color_texture: Some(soft),
                ..lit(LinearRgba::rgb(0.16, 0.06, 0.012))
            })
        },
    };
    // The braziers: a dark iron bowl on three splayed legs, coals in it, a tall fire.
    let iron = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.055, 0.05),
        perceptual_roughness: 0.55,
        metallic: 0.9,
        ..default()
    });
    let bowl = meshes.add(ConicalFrustum {
        radius_top: 0.42,
        radius_bottom: 0.2,
        height: 0.28,
    });
    let leg = meshes.add(Cylinder::new(0.025, 1.0));
    let bed = Coals::new(&mut meshes, &mut materials, &mut images);
    for p in BRAZIERS {
        let at = Vec3::new(p.x, 0.0, p.y);
        bed.spawn(&mut commands, &mut rng, at + Vec3::Y * BOWL);
        commands.spawn((
            Mesh3d(bowl.clone()),
            MeshMaterial3d(iron.clone()),
            Transform::from_translation(at + Vec3::Y * (BOWL - 0.14)),
            // The light stands over the bowl: its rim would throw a black ring round the foot.
            bevy::light::NotShadowCaster,
        ));
        for i in 0..3 {
            let a = i as f32 * std::f32::consts::TAU / 3.0 + 0.4;
            let out = Vec3::new(libm::cosf(a), 0.0, libm::sinf(a));
            // From under the bowl's rim out and down to the floor.
            let top = at + out * 0.22 + Vec3::Y * (BOWL - 0.2);
            let foot = at + out * 0.42;
            let mid = (top + foot) * 0.5;
            let len = top.distance(foot);
            commands.spawn((
                Mesh3d(leg.clone()),
                MeshMaterial3d(iron.clone()),
                Transform::from_translation(mid)
                    .with_rotation(Quat::from_rotation_arc(Vec3::Y, (top - foot).normalize()))
                    .with_scale(Vec3::new(1.0, len, 1.0)),
            ));
        }
        fire(
            &mut commands,
            &parts,
            &mut rng,
            at + Vec3::Y * BOWL,
            0.28,
            9,
            (0.45, 0.7),
        );
        // One light casting soft shadows, cut off at 4 m: a tight hot pool about 3 m across
        // with the dark between the fires, as the ARPGs' night frames have (at 8 m the two
        // braziers spread "one broad warm wash over the middle third", judge 261107-0507).
        light(
            &mut commands,
            &mut rng,
            at + Vec3::Y * (BOWL + 0.45),
            BRAZIER_LM,
            4.0,
            true,
            0.5,
        );
    }
    // The torches: on the back wall's face either side of the breach, and on the tall pillar.
    let face = BACK_Z + THICK * 0.5 + 0.08;
    let torches = [
        (Vec3::new(-6.2, 1.5, face), 0.0),
        (Vec3::new(1.2, 1.5, face), 0.0),
        (
            Vec3::new(PILLARS[0].x - 0.48, 1.5, PILLARS[0].y),
            -std::f32::consts::FRAC_PI_2,
        ),
    ];
    for (at, yaw) in torches {
        // The torch leans out from its bracket on the wall.
        let lean = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(0.35);
        let t = model(&mut commands, &asset, "models/iso/torch.glb", at, 0.0, 1.0);
        commands
            .entity(t)
            .insert(Transform::from_translation(at).with_rotation(lean));
        let head = at + lean * Vec3::Y * 0.6;
        fire(&mut commands, &parts, &mut rng, head, 0.05, 4, (0.32, 0.45));
        light(
            &mut commands,
            &mut rng,
            head + Vec3::Y * 0.25,
            TORCH_LM,
            4.0,
            false,
            0.1,
        );
    }
}

/// A brazier's bed of coals, seen from above as the ARPGs' braziers are: a heap of dark
/// lumps, the ones deep in the fire glowing orange to yellow and the crusted ones at the
/// rim dull red, over a soft glow that fills the bowl, so the bowl reads as burning even
/// where the flame cards are thin.
struct Coals {
    lump: Handle<Mesh>,
    glow: Handle<Mesh>,
    /// From the crust's dull red to the heart's yellow.
    heat: [Handle<StandardMaterial>; 4],
    haze: Handle<StandardMaterial>,
    soot: Handle<StandardMaterial>,
}

impl Coals {
    fn new(
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
    ) -> Self {
        let hot = |e: LinearRgba| StandardMaterial {
            base_color: Color::srgb(0.03, 0.025, 0.02),
            perceptual_roughness: 0.9,
            emissive: e,
            ..default()
        };
        let soft = images.add(soft_image());
        Coals {
            soot: materials.add(StandardMaterial {
                base_color: Color::srgba(0.012, 0.01, 0.009, 0.8),
                base_color_texture: Some(soft.clone()),
                perceptual_roughness: 1.0,
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
            lump: meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
            glow: meshes.add(Circle::new(1.0)),
            heat: [
                LinearRgba::rgb(0.12, 0.015, 0.0),
                LinearRgba::rgb(0.6, 0.09, 0.005),
                LinearRgba::rgb(2.0, 0.45, 0.04),
                LinearRgba::rgb(4.5, 1.8, 0.35),
            ]
            .map(|e| materials.add(hot(e))),
            haze: materials.add(StandardMaterial {
                base_color: LinearRgba::rgb(1.4, 0.36, 0.04).into(),
                base_color_texture: Some(soft),
                unlit: true,
                alpha_mode: AlphaMode::Add,
                ..default()
            }),
        }
    }

    /// The bed in a bowl whose rim is at `top`, 0.42 m across the rim.
    fn spawn(&self, commands: &mut Commands, rng: &mut Rng, top: Vec3) {
        // Soot on the flags about the brazier's foot, years of fallen ash and smoke.
        commands.spawn((
            Mesh3d(self.glow.clone()),
            MeshMaterial3d(self.soot.clone()),
            Transform::from_translation(top.with_y(super::ground::TOP + 0.004))
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::new(1.25, 1.05, 1.0)),
            bevy::light::NotShadowCaster,
        ));
        commands.spawn((
            Mesh3d(self.glow.clone()),
            MeshMaterial3d(self.haze.clone()),
            Transform::from_translation(top + Vec3::Y * 0.012)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::splat(0.5)),
            bevy::light::NotShadowCaster,
        ));
        for _ in 0..34 {
            // Heaped to the middle: the heart higher and hotter, the rim's lumps crusted.
            let r = 0.37 * rng.f().sqrt();
            let a = rng.f() * std::f32::consts::TAU;
            let k = 1.0 - r / 0.37;
            let heat = ((k * 2.6 + rng.range(-0.6, 1.0)).round() as usize).min(3);
            let s = rng.range(0.035, 0.07);
            commands.spawn((
                Mesh3d(self.lump.clone()),
                MeshMaterial3d(self.heat[heat].clone()),
                Transform::from_translation(
                    top + Vec3::new(libm::cosf(a) * r, 0.01 + 0.06 * k, libm::sinf(a) * r),
                )
                .with_rotation(
                    Quat::from_rotation_y(rng.f() * 6.0) * Quat::from_rotation_x(rng.f()),
                )
                .with_scale(Vec3::new(s * rng.range(0.9, 1.4), s * 0.6, s)),
                bevy::light::NotShadowCaster,
            ));
        }
    }
}

struct Parts {
    card: Handle<Mesh>,
    speck: Handle<Mesh>,
    flame: Handle<StandardMaterial>,
    ember: Handle<StandardMaterial>,
    halo: Handle<StandardMaterial>,
}

/// A fire of `n` tongues over a bed `r` across at `at`, sized in `size`, with its halo and
/// embers.
fn fire(
    commands: &mut Commands,
    parts: &Parts,
    rng: &mut Rng,
    at: Vec3,
    r: f32,
    n: usize,
    size: (f32, f32),
) {
    for i in 0..n {
        let a = i as f32 * 2.4;
        let rr = if i < 2 {
            r * 0.15
        } else {
            rng.range(0.3, 1.0) * r
        };
        let foot = at + Vec3::new(libm::cosf(a) * rr, 0.0, libm::sinf(a) * rr);
        commands.spawn((
            Mesh3d(parts.card.clone()),
            MeshMaterial3d(parts.flame.clone()),
            Transform::from_translation(foot),
            bevy::light::NotShadowCaster,
            Tongue {
                foot,
                seed: rng.f() * 50.0,
                size: if i < 2 {
                    size.1 * 1.3
                } else {
                    rng.range(size.0, size.1)
                },
            },
        ));
    }
    commands.spawn((
        Mesh3d(parts.card.clone()),
        MeshMaterial3d(parts.halo.clone()),
        Transform::from_translation(at + Vec3::Y * size.1 * 0.35)
            .with_scale(Vec3::splat(size.1 * 2.6)),
        bevy::light::NotShadowCaster,
        Halo,
    ));
    for i in 0..(n * 2) {
        commands.spawn((
            Mesh3d(parts.speck.clone()),
            MeshMaterial3d(parts.ember.clone()),
            Transform::from_xyz(0.0, -100.0, 0.0),
            bevy::light::NotShadowCaster,
            Ember {
                base: at + Vec3::Y * 0.1,
                born: -(i as f32) * 0.11,
                life: rng.range(1.2, 2.4),
                drift: Vec2::new(rng.range(-0.2, 0.2), rng.range(-0.2, 0.2)),
                seed: rng.f() * 100.0,
            },
        ));
    }
}

fn light(
    commands: &mut Commands,
    rng: &mut Rng,
    at: Vec3,
    lumens: f32,
    range: f32,
    shadows: bool,
    radius: f32,
) {
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.55, 0.22),
            intensity: lumens,
            range,
            radius,
            shadow_maps_enabled: shadows,
            soft_shadows_enabled: shadows,
            ..default()
        },
        Transform::from_translation(at),
        Flicker {
            lumens,
            seed: rng.f() * 40.0,
        },
        // The pool mirrors every fire as a streak (water.rs, the kit's water).
        crate::kit::water::Glint,
    ));
}

/// Embers rise off their fire, drift on the air, shrink and die, and are born again.
fn rise(time: Res<Time>, mut embers: Query<(&mut Ember, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (mut e, mut tr) in &mut embers {
        let mut age = t - e.born;
        if age > e.life {
            e.born = t;
            e.seed += 1.37;
            age = 0.0;
        }
        if age < 0.0 {
            continue;
        }
        let k = age / e.life;
        let swirl = Vec3::new(
            libm::sinf(age * 4.0 + e.seed),
            0.0,
            libm::cosf(age * 3.3 + e.seed),
        ) * 0.12
            * k;
        let start = Vec3::new(libm::sinf(e.seed * 7.1), 0.0, libm::cosf(e.seed * 3.7)) * 0.15;
        tr.translation = e.base
            + start
            + Vec3::new(e.drift.x + 0.2, 0.0, e.drift.y + 0.1) * age
            + Vec3::Y * (1.0 * age - 0.1 * age * age)
            + swirl;
        tr.scale = Vec3::splat(0.02 * libm::powf(1.0 - k, 0.8));
    }
}

/// Flames waver (scene.rs's pattern).
fn flicker(time: Res<Time>, mut lights: Query<(&Flicker, &mut PointLight)>) {
    let t = time.elapsed_secs();
    for (f, mut light) in &mut lights {
        let w = libm::sinf(t * 13.0 + f.seed) * 0.06
            + libm::sinf(t * 7.3 + f.seed * 2.1) * 0.08
            + libm::sinf(t * 23.0 + f.seed * 0.7) * 0.04;
        light.intensity = f.lumens * (1.0 + w);
    }
}
