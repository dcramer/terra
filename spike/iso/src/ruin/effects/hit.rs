//! The hero's blow landing on the ogre, as the ARPGs sell a hit when the clip alone cannot
//! (Tripo has no hit preset): sparks thrown off the blade's edge, a flash of light where it
//! bites, the ogre lit white then red for a tenth of a second (the hit flash), the ogre
//! thrown back a few degrees and recovering (an additive flinch of the whole body, about its
//! feet), and both clips held for a hit-stop of 80 ms. The blow lands each time the
//! knight's attack passes `BLOW`; in a still, the moment is `STILL_AGE` after it.

use super::Stage;
use super::kit::{Burst, Cam, Clock, glow, sparks};
use crate::forest::Rng;
use bevy::prelude::*;

/// Where the knight and the ogre stand is the stage's (mod.rs), to find them by.
/// When in the knight's attack the blade meets the ogre: the chop coming down (fight.rs).
const BLOW: f32 = 1.25;
/// The still's moment after the blow (s), and the hit-stop's length.
const STILL_AGE: f32 = 0.05;
const STOP: f32 = 0.08;
/// How far the flinch throws the ogre back at its height (radians), and its rise and fall (s).
const FLINCH: f32 = 0.15;
const RISE: f32 = 0.05;
const FALL: f32 = 0.16;

#[derive(Resource, Default)]
struct Hit {
    knight: Option<Entity>,
    ogre: Option<Entity>,
    base: Quat,
    skin: Vec<Handle<StandardMaterial>>,
    seek: f32,
    at: f32,
    stopped: bool,
}

#[derive(Component)]
enum Part {
    Flash(Handle<StandardMaterial>),
    Light,
    Burst,
}

pub fn plugin(app: &mut App) {
    app.insert_resource(Hit {
        at: -10.0,
        ..default()
    })
    .add_systems(Startup, spawn)
    .add_systems(Update, (find, (blow, show).chain()).chain());
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    stage: Res<Stage>,
) {
    let strike = stage.strike();
    let mut rng = Rng::new(0x41771);
    let soft = images.add(crate::motes::soft_image());
    let flash = materials.add(glow(LinearRgba::rgb(6.0, 4.0, 3.0), Some(soft)));
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
        MeshMaterial3d(flash.clone()),
        Transform::from_scale(Vec3::ZERO),
        bevy::light::NotShadowCaster,
        Part::Flash(flash),
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.82, 0.6),
            intensity: 0.0,
            range: 6.0,
            radius: 0.1,
            ..default()
        },
        Transform::from_translation(strike + Vec3::Y * 0.2),
        Part::Light,
    ));
    let burst = commands
        .spawn((Burst { at: strike, age: -1.0 }, Part::Burst))
        .id();
    let out = (stage.foe - stage.knight).normalize().extend(0.0).xzy() + Vec3::Y * 0.45;
    let streak = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let spark = materials.add(glow(LinearRgba::rgb(14.0, 6.5, 1.6), None));
    sparks(&mut commands, burst, &streak, &spark, &mut rng, 26, out, 0.8, (4.0, 10.0), 0.03);
}

/// Finds the knight and the ogre by where they stand, and the ogre's materials once its
/// model is in.
fn find(
    mut hit: ResMut<Hit>,
    bodies: Query<(Entity, &Transform), With<WorldAssetRoot>>,
    children: Query<&Children>,
    skins: Query<&MeshMaterial3d<StandardMaterial>>,
    stage: Res<Stage>,
) {
    let near = |p: Vec2| {
        bodies
            .iter()
            .find(|(_, t)| t.translation.xz().distance(p) < 0.3)
            .map(|(e, t)| (e, t.rotation))
    };
    if hit.knight.is_none() {
        hit.knight = near(stage.knight).map(|b| b.0);
    }
    if hit.ogre.is_none()
        && let Some((e, r)) = near(stage.foe)
    {
        hit.ogre = Some(e);
        hit.base = r;
    }
    if let Some(ogre) = hit.ogre
        && hit.skin.is_empty()
    {
        let mut skin: Vec<_> = children
            .iter_descendants(ogre)
            .filter_map(|e| skins.get(e).ok().map(|m| m.0.clone()))
            .collect();
        skin.dedup();
        hit.skin = skin;
    }
}

/// When the knight's attack passes the blow: the hit lands, and both clips stop for the
/// hit-stop, then go on.
fn blow(
    clock: Res<Clock>,
    mut hit: ResMut<Hit>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
) {
    if clock.still {
        return;
    }
    let (Some(knight), Some(ogre)) = (hit.knight, hit.ogre) else {
        return;
    };
    let player = |body: Entity| children.iter_descendants(body).find(|e| players.contains(*e));
    let (Some(kp), Some(op)) = (player(knight), player(ogre)) else {
        return;
    };
    let Some(seek) = players
        .get(kp)
        .ok()
        .and_then(|p| p.playing_animations().next().map(|(_, a)| a.seek_time()))
    else {
        return;
    };
    let last = hit.seek;
    hit.seek = seek;
    let crossed = (last < BLOW && seek >= BLOW) || (seek < last && (last < BLOW || seek >= BLOW));
    let speed = if crossed && !hit.stopped {
        hit.at = clock.t;
        hit.stopped = true;
        0.0
    } else if hit.stopped && clock.t - hit.at >= STOP {
        hit.stopped = false;
        1.0
    } else {
        return;
    };
    for e in [kp, op] {
        if let Ok(mut p) = players.get_mut(e) {
            for (_, a) in p.playing_animations_mut() {
                a.set_speed(speed);
            }
        }
    }
}

/// The flinch's share at `a` s after the blow: up fast, back slowly.
fn flinch(a: f32) -> f32 {
    if a < 0.0 {
        0.0
    } else if a < RISE {
        a / RISE
    } else {
        (-(a - RISE) / FALL).exp()
    }
}

/// The hit flash's emissive at `a`: white, then red, then gone, in a sixth of a second.
/// Partial, as the ARPGs' hit flash is: it multiplies the creature's own albedo (its
/// emissive texture is its base colour's, `show`) and stays under it, so the hide's
/// pattern and the light's shading still read through it rather than a flat silhouette.
fn flash(a: f32) -> LinearRgba {
    if !(0.0..0.17).contains(&a) {
        return LinearRgba::BLACK;
    }
    let white = Vec3::splat(0.55);
    let red = Vec3::new(0.7, 0.05, 0.03);
    let c = white.lerp(red, super::kit::smooth(0.02, 0.07, a))
        * (1.0 - super::kit::smooth(0.07, 0.17, a));
    LinearRgba::rgb(c.x, c.y, c.z)
}

fn show(
    clock: Res<Clock>,
    cam: Res<Cam>,
    hit: Res<Hit>,
    mut parts: Query<(&Part, Option<&mut Transform>, Option<&mut PointLight>, Option<&mut Burst>)>,
    mut bodies: Query<&mut Transform, (With<WorldAssetRoot>, Without<Part>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stage: Res<Stage>,
) {
    let a = if clock.still { STILL_AGE } else { clock.t - hit.at };
    for (part, tr, light, burst) in &mut parts {
        match part {
            Part::Flash(m) => {
                let k = (a / 0.12).clamp(0.0, 1.0);
                let mut tr = tr.unwrap();
                tr.translation = stage.strike();
                tr.rotation = cam.rot;
                tr.scale = Vec3::splat(if a < 0.12 { 0.5 * (1.0 - k * k) + 0.1 } else { 0.0 });
                if let Some(mut m) = materials.get_mut(m) {
                    let f = 1.0 - k;
                    m.base_color = LinearRgba::rgb(6.0 * f, 4.0 * f, 3.0 * f).into();
                }
            }
            Part::Light => {
                light.unwrap().intensity = if a >= 0.0 { 25_000.0 * (-a / 0.06).exp() } else { 0.0 };
            }
            Part::Burst => burst.unwrap().age = a,
        }
    }
    let Some(ogre) = hit.ogre else { return };
    if let Ok(mut tr) = bodies.get_mut(ogre) {
        let d = (stage.foe - stage.knight).normalize().extend(0.0).xzy();
        let f = flinch(a);
        tr.rotation = Quat::from_axis_angle(Vec3::Y.cross(d).normalize(), FLINCH * f)
            * Quat::from_rotation_y(0.12 * f)
            * hit.base;
    }
    let e = flash(a);
    for h in &hit.skin {
        if materials.get(h).is_some_and(|m| m.emissive != e)
            && let Some(mut m) = materials.get_mut(h)
        {
            m.emissive = e;
            if m.emissive_texture.is_none() {
                m.emissive_texture = m.base_color_texture.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The flinch peaks just after the blow and is gone within half a second.
    #[test]
    fn flinch_rises_and_falls() {
        assert_eq!(flinch(-0.1), 0.0);
        assert!((flinch(RISE) - 1.0).abs() < 1e-5);
        assert!(flinch(0.6) < 0.06);
    }

    /// The flash starts white, turns red, and is gone in under two tenths; it is partial
    /// throughout, never adding more than the albedo it multiplies, so shading survives.
    #[test]
    fn flash_goes_white_red_out() {
        let w = flash(0.01);
        let r = flash(0.09);
        assert!(w.green > 0.3 && w.green > 0.8 * w.red, "{w:?}");
        for i in 0..20 {
            let c = flash(i as f32 * 0.01);
            assert!(c.red < 1.0 && c.green < 1.0 && c.blue < 1.0, "{c:?} at {i}0 ms");
        }
        assert!(r.red > 4.0 * r.green && r.red > 0.2, "{r:?}");
        assert_eq!(flash(0.2), LinearRgba::BLACK);
    }
}
