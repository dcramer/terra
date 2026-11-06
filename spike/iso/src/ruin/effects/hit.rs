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
/// The star flash at the bite: its size (m) and life (s). A metre of white star hid the
/// blow's bite in all three biomes (the lead's judges, 2026-11-06); the slash (slash.rs)
/// carries the blow now, the star a small hot glint at the bite.
const STAR: f32 = 0.5;
const STAR_LIFE: f32 = 0.12;
/// The light at the bite (lumens) and how fast it dies (s): 25k over 0.06 s was spent by
/// the still's 0.05 s and lit no ground (the judges: "the sword's sparks don't light the
/// ground"). A second light half a metre over the floor under the bite pools on the
/// ground, as the ARPGs fake an impact's light (spell.rs's pool, traps.md).
const BITE_LM: f32 = 70_000.0;
const BITE_FALL: f32 = 0.14;
const POOL_LM: f32 = 45_000.0;
const POOL_UP: f32 = 0.5;
const POOL_FALL: f32 = 0.2;
/// The warm scuff the blow leaves on the floor under the bite: its size (m) and life (s).
const MARK: f32 = 1.4;
const MARK_LIFE: f32 = 0.7;

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
    Pool,
    Burst,
    Mark,
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
    mut decals: ResMut<Assets<super::mark::Decal>>,
    land: Option<Res<crate::terrain::Land>>,
    stage: Res<Stage>,
) {
    let strike = stage.strike();
    let ground = stage.ground(land.as_deref(), strike.xz());
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.62, 0.32),
            intensity: 0.0,
            range: 3.5,
            radius: 0.1,
            ..default()
        },
        Transform::from_translation(ground + Vec3::Y * POOL_UP),
        Part::Pool,
    ));
    let scuff = images.add(super::mark::mark_image());
    let mark = super::mark::mark(
        &mut commands,
        &mut decals,
        &scuff,
        ground,
        MARK,
        MARK_LIFE,
        LinearRgba::rgb(4.0, 1.3, 0.25),
        LinearRgba::new(0.04, 0.03, 0.025, 0.55),
    );
    commands.entity(mark).insert(Part::Mark);
    let mut rng = Rng::new(0x41771);
    let star = images.add(super::kit::star_image());
    let flash = materials.add(glow(LinearRgba::rgb(9.0, 6.5, 4.0), Some(star)));
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
    // 30 streaks 1.8 cm thick read as a tiny spark star (the judges): more, thicker.
    sparks(&mut commands, burst, &streak, &spark, &mut rng, 44, out, 0.8, (4.0, 11.0), 0.032);
}

/// Finds the knight and the ogre by where they stand, and the ogre's materials once its
/// model is in.
fn find(
    mut hit: ResMut<Hit>,
    bodies: Query<(Entity, &Transform), (With<WorldAssetRoot>, Without<ChildOf>)>,
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
    mut parts: Query<(
        &Part,
        Option<&mut Transform>,
        Option<&mut PointLight>,
        Option<&mut Burst>,
        Option<&mut super::mark::Mark>,
    )>,
    mut bodies: Query<&mut Transform, (With<WorldAssetRoot>, Without<Part>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stage: Res<Stage>,
) {
    let a = if clock.still { STILL_AGE } else { clock.t - hit.at };
    for (part, tr, light, burst, mark) in &mut parts {
        match part {
            Part::Flash(m) => {
                // A hard star where the blade bites (kit::star), full at once and gone in
                // a seventh of a second, not a swelling soft disc.
                let k = (a / STAR_LIFE).clamp(0.0, 1.0);
                let mut tr = tr.unwrap();
                tr.translation = stage.strike();
                tr.rotation = cam.rot * Quat::from_rotation_z(0.2);
                tr.scale = Vec3::splat(if (0.0..STAR_LIFE).contains(&a) { STAR * (1.0 - 0.4 * k) } else { 0.0 });
                if let Some(mut m) = materials.get_mut(m) {
                    let f = (1.0 - k).powf(1.5);
                    m.base_color = LinearRgba::rgb(9.0 * f, 6.5 * f, 4.0 * f).into();
                }
            }
            Part::Light => {
                light.unwrap().intensity = if a >= 0.0 { BITE_LM * (-a / BITE_FALL).exp() } else { 0.0 };
            }
            Part::Pool => {
                light.unwrap().intensity = if a >= 0.0 { POOL_LM * (-a / POOL_FALL).exp() } else { 0.0 };
            }
            Part::Burst => burst.unwrap().age = a,
            Part::Mark => mark.unwrap().age = a,
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
