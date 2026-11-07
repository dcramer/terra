//! The hero's blow landing on the ogre, as the ARPGs sell a hit when the clip alone cannot
//! (Tripo has no hit preset): the ogre lit white then red for a tenth of a second (the hit
//! flash), the ogre thrown back a few degrees and recovering (an additive flinch of the
//! whole body, about its feet), both clips held for a hit-stop of 80 ms, and a scuff on the
//! floor. The blow's flash, sparks, smoke and lights are the biome's effect presets
//! (kit/effects.rs, cued by `Cues::blow`). The blow lands each time the knight's attack
//! passes `BLOW`; in a still, the moment is `STILL_AGE` after it.

use super::Stage;
use super::kit::Clock;
use crate::kit::effects::Cues;
use bevy::prelude::*;

/// Where the knight and the ogre stand is the stage's (mod.rs), to find them by.
/// When in the knight's attack the blade meets the ogre: the chop coming down (fight.rs).
const BLOW: f32 = 1.25;
/// The still's moment after the blow (s), and the hit-stop's length.
const STILL_AGE: f32 = 0.05;
const STOP: f32 = 0.08;
/// How far the flinch throws the ogre back at its height (radians), and its rise and fall (s).
pub(crate) const FLINCH: f32 = 0.15;
const RISE: f32 = 0.05;
const FALL: f32 = 0.16;
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

/// The scuff under the bite.
#[derive(Component)]
struct Scuff;

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
    mut images: ResMut<Assets<Image>>,
    mut decals: ResMut<Assets<super::mark::Decal>>,
    land: Option<Res<crate::terrain::Land>>,
    stage: Res<Stage>,
) {
    let ground = stage.ground(land.as_deref(), stage.strike().xz());
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
    commands.entity(mark).insert(Scuff);
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
    mut cues: ResMut<Cues>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    walking: Option<Res<crate::ruin::walk::Walking>>,
) {
    if clock.still {
        return;
    }
    let (Some(knight), Some(ogre)) = (hit.knight, hit.ogre) else {
        return;
    };
    let player = |body: Entity| {
        children
            .iter_descendants(body)
            .find(|e| players.contains(*e))
    };
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
    // While the hero walks in his clip is the walk's: no blow, and his attack counts from
    // its start once he stands (walk.rs).
    let walks = walking.is_some_and(|w| w.now);
    let last = hit.seek;
    hit.seek = if walks { 0.0 } else { seek };
    let crossed =
        !walks && ((last < BLOW && seek >= BLOW) || (seek < last && (last < BLOW || seek >= BLOW)));
    let speed = if crossed && !hit.stopped {
        hit.at = clock.t;
        cues.blow = clock.t;
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
pub(crate) fn flinch(a: f32) -> f32 {
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
pub(crate) fn flash(a: f32) -> LinearRgba {
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
    args: Res<crate::Args>,
    clock: Res<Clock>,
    hit: Res<Hit>,
    cues: Res<Cues>,
    mut scuffs: Query<
        (&mut super::mark::Mark, &mut Transform),
        (With<Scuff>, Without<WorldAssetRoot>),
    >,
    mut bodies: Query<&mut Transform, With<WorldAssetRoot>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stage: Res<Stage>,
) {
    let a = if clock.still {
        STILL_AGE
    } else {
        clock.t - hit.at
    };
    for (mut mark, mut tr) in &mut scuffs {
        // In play the scuff is the player's blow's, on the floor under where it bit
        // (play/strike.rs's contact), not the staged one's (held back while he plays).
        mark.age = match (args.play, cues.contact) {
            (true, Some(c)) => {
                tr.translation = scuff_at(&c);
                clock.t - cues.blow
            }
            (true, None) => -1.0,
            _ => a,
        };
    }
    // In play the ogre's flinch and flash are play's own blows' (play/strike.rs).
    let Some(ogre) = hit.ogre.filter(|_| !args.play) else {
        return;
    };
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

/// Where a blow's scuff lies: on the floor straight under the bite.
fn scuff_at(c: &crate::kit::effects::Contact) -> Vec3 {
    Vec3::new(c.point.x, c.floor, c.point.z)
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
            assert!(
                c.red < 1.0 && c.green < 1.0 && c.blue < 1.0,
                "{c:?} at {i}0 ms"
            );
        }
        assert!(r.red > 4.0 * r.green && r.red > 0.2, "{r:?}");
        assert_eq!(flash(0.2), LinearRgba::BLACK);
    }
}
