//! Play (`--play`, and the default when a window opens on a biome): the hero is the player.
//! WASD moves him relative to the screen (W up it, not world north: the ARPG camera is
//! turned `ruin::camera::YAW`), running, Shift walking, facing his way; a left click on the
//! ground turns him to the cursor's ground point and swings his attack there, moving held
//! for the swing, and the creatures in its arc within reach fall. The camera follows him at
//! its fixed angle and distance (D629). A `--shot`/`--film` run without `--play` never adds
//! this, so the loop's stills and films are as before; with it, `--press` and `--click`
//! script the input (main.rs's Args).
//!
//! Where he may stand is each layout's (`Ground`): the floor's height and what is open. The
//! staged fight's effects stay held back (walk.rs's `Walking`, as while he walks in).

use crate::Args;
use crate::camera::MainCamera;
use crate::kit::biome::{Biome, Layout};
use crate::ruin::effects::slash::{Blade, Swinging};
use crate::ruin::fight::{Pose, Posed};
use bevy::input::InputSystems;
use bevy::prelude::*;
use std::time::Duration;

// The hero (crate::hero) walks round the same footprints and strikes as this does.
pub(crate) mod feel;
pub(crate) mod footprint;
pub(crate) mod knock;
pub(crate) mod strike;

/// How far the swing reaches (m, from his feet) and half its arc (degrees): the staged
/// fight's ogre stands 2.4 m off.
pub const REACH: f32 = 3.0;
pub const ARC: f32 = 60.0;
/// The attack clip's part a swing plays (s; the blow lands at 1.25, effects::hit's `BLOW`)
/// and how fast.
const SWING: (f32, f32) = (0.7, 1.75);
const BLOW: f32 = 1.25;
const SWING_SPEED: f32 = 1.5;
/// The hit-stop when the blade bites (s), as the staged blow's (ruin/effects/hit.rs).
const STOP: f32 = 0.08;
/// Ground speeds (m/s) of the knight's run and walk clips (art/iso/knight/asset.json).
const RUN: f32 = 3.87;
const WALK: f32 = 1.15;
const BLEND: Duration = Duration::from_millis(150);
/// How fast he turns, and the camera catches up (per second).
const TURN: f32 = 14.0;
const FOLLOW: f32 = 6.0;

/// The player's body.
#[derive(Component)]
pub struct Hero;

/// A body felled by its last blow (strike.rs).
#[derive(Component)]
pub(crate) struct Fallen;

/// The hero's clips on his player, and what he is doing.
#[derive(Component)]
struct Rig {
    player: Entity,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    run: AnimationNodeIndex,
    attack: AnimationNodeIndex,
    playing: Option<AnimationNodeIndex>,
    /// A swing under way: the way he faces, and whether the blow has landed.
    swing: Option<(Vec2, bool)>,
    /// The hit-stop's end (s): the swing held where the blade bit.
    halt: Option<f32>,
}

/// Where the hero may stand in this layout: the floor's height under a point, and whether
/// it is open (inside the biome, off walls and deep water).
#[derive(Resource)]
pub struct Ground {
    pub floor: Box<dyn Fn(Vec2) -> f32 + Send + Sync>,
    pub open: Box<dyn Fn(Vec2) -> bool + Send + Sync>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, (ground, street_hero))
        .add_systems(PreUpdate, script.after(InputSystems))
        .add_systems(Update, (adopt, rig, control, follow).chain())
        .add_systems(Update, footprint::take.after(adopt).before(control))
        // The staged Tripo knight, once adopted, gives way to the game's body (hero/, D628).
        .add_systems(Update, crate::hero::swap.after(adopt).before(rig))
        .add_plugins((strike::plugin, feel::plugin, crate::hero::plugin));
    // The street stages no fight, so nothing else hangs the sword on its hero (held.rs) or
    // draws his blows: its stage (the effects aim at it from Startup) and play's effects.
    let layout = app.world().get_resource::<Args>().and_then(|a| a.layout);
    let stage = app
        .world()
        .get_resource::<Biome>()
        .and_then(|b| Some(street_stage(b.street.as_ref()?, &b.foes)));
    if layout == Some(Layout::Street)
        && let Some(stage) = stage
    {
        app.insert_resource(stage)
            .add_plugins((crate::ruin::held::plugin, crate::ruin::effects::played));
    }
}

/// Where a foe of the biome's roster stands from the hero (its `at`, kit/biome.rs `Foe`).
fn off(foe: &crate::kit::biome::Foe) -> Vec2 {
    Vec2::new(foe.at.0, foe.at.1)
}

/// The street's floor at `p`: the pavements the kerb's rise up, the road at 0.
fn street_floor(st: &crate::kit::street::Street, p: Vec2) -> f32 {
    if p.y.abs() > st.road / 2.0 {
        st.kerb.1
    } else {
        0.0
    }
}

/// The street's stage: its hero at the focus, the roster's first foe the one his blow aims
/// at (with none, the way ahead of him).
fn street_stage(
    st: &crate::kit::street::Street,
    foes: &[crate::kit::biome::Foe],
) -> crate::ruin::effects::Stage {
    let at = Vec2::new(st.focus.0, st.focus.1);
    crate::ruin::effects::Stage {
        knight: at,
        foe: at + foes.first().map_or(Vec2::Y, off),
        target: at,
        floor: street_floor(st, at),
    }
}

/// A ground direction (x, z) for screen input (x right, y up the screen) under a camera
/// turned `yaw` degrees from due south (it stands at +z turned toward +x, looking back).
pub fn screen_move(input: Vec2, yaw: f32) -> Vec2 {
    let (s, c) = libm::sincosf(yaw.to_radians());
    let up = Vec2::new(-s, -c);
    let right = Vec2::new(c, -s);
    (right * input.x + up * input.y).normalize_or_zero()
}

/// The turn that faces a body (the models face +z) along `d`.
pub fn facing(d: Vec2) -> Quat {
    Quat::from_rotation_y(libm::atan2f(d.x, d.y))
}

/// Whether `p` is in a swing from `at` facing `face`: within `REACH` and `ARC` of its way.
pub fn in_arc(at: Vec2, face: Vec2, p: Vec2) -> bool {
    let d = p - at;
    let n = d.length();
    n <= REACH && (n < 0.3 || d.dot(face) / n >= libm::cosf(ARC.to_radians()))
}

/// Each layout's ground; the forest's open cells are its nav grid's (trees, logs, pond).
fn ground(mut commands: Commands, args: Res<Args>, biome: Option<Res<Biome>>) {
    let rect = |lo: Vec2, hi: Vec2| move |p: Vec2| p.cmpge(lo).all() && p.cmple(hi).all();
    let g = match args.layout {
        Some(Layout::Ruin) => {
            use crate::ruin::{ground, walls, water};
            let inside = rect(
                Vec2::new(walls::WEST_X + 0.75, walls::BACK_Z + 0.75),
                Vec2::new(ground::PAVED_X.1 - 0.5, ground::PAVED_Z.1 - 0.5),
            );
            Ground {
                floor: Box::new(|_| ground::TOP),
                open: Box::new(move |p| {
                    inside(p)
                        && walls::PILLARS.iter().all(|c| c.distance(p) > 0.75)
                        && water::depth(p) < 0.4
                }),
            }
        }
        Some(Layout::Marsh) => {
            use crate::marsh::{KNIGHT, LEVEL, fight::footing, land::shape};
            let inside = rect(KNIGHT - 12.0, KNIGHT + 12.0);
            Ground {
                floor: Box::new(|p| footing(p, 0.35)),
                // Water to his knees, no deeper.
                open: Box::new(move |p| inside(p) && LEVEL - shape(p) < 0.5),
            }
        }
        Some(Layout::Forest) => {
            // The whole wood the nav grid covers (nav.rs `HALF`), so a route's packs
            // (demo/mobs.rs) lie on open ground: what blocks him there is the grid's.
            let edge = Vec2::splat(crate::nav::HALF - 1.0);
            let inside = rect(-edge, edge);
            Ground {
                floor: Box::new(crate::terrain::shape),
                open: Box::new(inside),
            }
        }
        Some(Layout::Street) => {
            let st = biome
                .and_then(|b| b.street.clone())
                .expect("a street biome");
            let (half, _, wall) = st.edges();
            let (rise, fx) = (st.kerb.1, st.focus.0);
            let inside = rect(
                Vec2::new(fx - 20.0, 0.45 - wall),
                Vec2::new(fx + 20.0, wall - 0.45),
            );
            // The lamp poles are the kit's own meshes, no model's footprint (footprint.rs).
            let (poles, r) = st.poles();
            let poles: Vec<Vec2> = poles.into_iter().map(|(_, f)| f.xz()).collect();
            let r = r + footprint::BODY;
            Ground {
                floor: Box::new(move |p| if p.y.abs() > half { rise } else { 0.0 }),
                open: Box::new(move |p| inside(p) && poles.iter().all(|c| c.distance(p) > r)),
            }
        }
        None => return,
    };
    commands.insert_resource(g);
}

/// The street stages no fight: the hero stands at its focus, his foes about him.
fn street_hero(
    mut commands: Commands,
    args: Res<Args>,
    biome: Option<Res<Biome>>,
    asset: Res<AssetServer>,
) {
    let Some(biome) = biome else {
        return;
    };
    let Some(st) = biome.street.clone() else {
        return;
    };
    if args.layout != Some(Layout::Street) {
        return;
    }
    let at = Vec2::new(st.focus.0, st.focus.1);
    let path = "models/iso/knight.glb";
    let y = street_floor(&st, at);
    let body = crate::scene::model(&mut commands, &asset, path, at.extend(y).xzy(), 0.0, 1.0);
    commands.entity(body).insert((
        Hero,
        crate::rim::Fighter::valued(1.0, 0.35),
        // His attack first, not his idle: the slash bakes its blade's path from the clip
        // he first plays (ruin/effects/slash.rs), and play's rig then takes him over.
        Pose {
            gltf: asset.load(path),
            clip: "attack",
            at: 0.0,
            once: false,
        },
    ));
    for foe in &biome.foes {
        let (model, off) = (foe.model.as_str(), off(foe));
        let p = at + off;
        let yaw = libm::atan2f((-off).x, (-off).y);
        let floor = street_floor(&st, p);
        let foe = crate::scene::model(
            &mut commands,
            &asset,
            model,
            p.extend(floor).xzy(),
            yaw,
            1.0,
        );
        // Darker than the ground about it as the ruin's (the biome's `bodies`), a faint rim as
        // the street's people, its contact shadow under its feet (kit/contact.rs).
        commands.entity(foe).insert((
            crate::rim::Fighter::valued(0.5, 0.35),
            crate::kit::contact::Feet { floor },
            Pose {
                gltf: asset.load(model.to_owned()),
                clip: "idle",
                at: 0.0,
                once: false,
            },
        ));
    }
}

/// The staged fight's hero (the body that walks in to fight) becomes the player.
fn adopt(
    mut commands: Commands,
    bodies: Query<(Entity, &crate::ruin::walk::WalkIn), Without<Hero>>,
) {
    for (body, w) in &bodies {
        if w.fight {
            commands
                .entity(body)
                .insert(Hero)
                .remove::<crate::ruin::walk::WalkIn>();
        }
    }
}

fn player_of(
    body: Entity,
    children: &Query<&Children>,
    players: &Query<&mut AnimationPlayer>,
) -> Option<Entity> {
    children
        .iter_descendants(body)
        .find(|e| players.contains(*e))
}

/// Once his model is in and posed: his idle, walk, run and attack on his player.
fn rig(
    mut commands: Commands,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    heroes: Query<(Entity, &Pose), (With<Hero>, With<Posed>, Without<Rig>)>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    blade: Option<Res<Blade>>,
) {
    // The slash bakes its blade's path from his staged clip first (ruin/effects/slash.rs).
    if blade.is_some_and(|b| !b.settled()) {
        return;
    }
    for (body, pose) in &heroes {
        let (Some(gltf), Some(player)) =
            (gltfs.get(&pose.gltf), player_of(body, &children, &players))
        else {
            continue;
        };
        let clip = |n: &str| gltf.named_animations.get(n).cloned();
        let (Some(idle), Some(walk), Some(run), Some(attack)) =
            (clip("idle"), clip("walk"), clip("run"), clip("attack"))
        else {
            warn!(
                "the hero {:?} lacks idle, walk, run or attack",
                pose.gltf.path()
            );
            commands.entity(body).remove::<Hero>();
            continue;
        };
        let (graph, n) = AnimationGraph::from_clips([idle, walk, run, attack]);
        let mut p = players.get_mut(player).unwrap();
        p.stop_all();
        let mut transitions = AnimationTransitions::new();
        transitions.play(&mut p, n[0], Duration::ZERO).repeat();
        commands
            .entity(player)
            .insert((AnimationGraphHandle(graphs.add(graph)), transitions));
        commands.entity(body).insert(Rig {
            player,
            idle: n[0],
            walk: n[1],
            run: n[2],
            attack: n[3],
            playing: Some(n[0]),
            swing: None,
            halt: None,
        });
    }
}

/// The scripted input (`--press KEYS@FROM-TO`, frames): the keys held over those frames.
fn script(
    args: Res<Args>,
    frame: Res<bevy::diagnostic::FrameCount>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
) {
    for (letters, from, to) in &args.press {
        for ch in letters.chars() {
            let key = match ch {
                'w' => KeyCode::KeyW,
                'a' => KeyCode::KeyA,
                's' => KeyCode::KeyS,
                'd' => KeyCode::KeyD,
                _ => continue,
            };
            if (*from..*to).contains(&frame.0) {
                keys.press(key);
            } else if frame.0 == *to {
                keys.release(key);
            }
        }
    }
}

/// Moves, turns and swings the hero, and plays the clip for what he does.
#[allow(clippy::too_many_arguments)]
fn control(
    mut commands: Commands,
    args: Res<Args>,
    time: Res<Time>,
    (frame, keys, mouse, windows): (
        Res<bevy::diagnostic::FrameCount>,
        Res<ButtonInput<KeyCode>>,
        Res<ButtonInput<MouseButton>>,
        Query<&Window>,
    ),
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    (ground, nav, walking): (
        Option<Res<Ground>>,
        Option<Res<crate::nav::Nav>>,
        Option<ResMut<crate::ruin::walk::Walking>>,
    ),
    mut hero: Query<(Entity, &mut Transform, &mut Rig), With<Hero>>,
    (foes, footprints): (
        Query<(Entity, &Transform), (With<Pose>, Without<Hero>, Without<Fallen>)>,
        Query<(&footprint::Footprint, &GlobalTransform), footprint::Blocking>,
    ),
    mut players: Query<&mut AnimationPlayer>,
    mut transitions: Query<&mut AnimationTransitions>,
    mut queued: Local<Option<Vec2>>,
) {
    // The staged fight's blow, slash and bolts held back while he plays (walk.rs).
    if let Some(mut w) = walking {
        w.now = true;
    }
    // A click: the window's cursor, or the script's at this frame. The last one is kept until
    // he can swing (his rig not up yet, or a swing under way), not dropped.
    let click = if mouse.just_pressed(MouseButton::Left) {
        windows.single().ok().and_then(|w| w.cursor_position())
    } else {
        args.clicks.iter().find(|c| c.1 == frame.0).map(|c| c.0)
    };
    *queued = click.or(*queued);
    let (Some(ground), Ok((body, mut tr, mut rig))) = (ground, hero.single_mut()) else {
        return;
    };
    let dt = time.delta_secs();
    let here = tr.translation.xz();
    let (Ok(mut p), Ok(mut tx)) = (players.get_mut(rig.player), transitions.get_mut(rig.player))
    else {
        return;
    };
    if rig.swing.is_none()
        && let Ok((cam, cam_tr)) = camera.single()
        && let Some(px) = queued.take()
        && let Ok(ray) = cam.viewport_to_world(cam_tr, px)
        && let Some(d) = ray.intersect_plane(tr.translation, InfinitePlane3d::new(Vec3::Y))
    {
        let ahead = (tr.rotation * Vec3::Z).xz();
        let face = (ray.get_point(d).xz() - here).normalize_or(ahead);
        tr.rotation = facing(face);
        tx.play(&mut p, rig.attack, BLEND)
            .seek_to(SWING.0)
            .set_speed(SWING_SPEED);
        rig.playing = Some(rig.attack);
        rig.swing = Some((face, false));
        debug!("swing at frame {}", frame.0);
    }
    if let Some((face, landed)) = rig.swing {
        let t = p.animation(rig.attack).map_or(SWING.1, |a| a.seek_time());
        // The slash trail draws from it (ruin/effects/slash.rs).
        commands.entity(body).insert(Swinging(rig.attack));
        let now = time.elapsed_secs();
        if rig.halt.is_some_and(|h| now >= h) {
            rig.halt = None;
            if let Some(a) = p.animation_mut(rig.attack) {
                a.set_speed(SWING_SPEED);
            }
        }
        if !landed && t >= BLOW {
            rig.swing = Some((face, true));
            let mut bit = false;
            for (foe, ftr) in &foes {
                if in_arc(here, face, ftr.translation.xz()) {
                    commands.entity(foe).insert(strike::Blow {
                        from: here,
                        t,
                        weight: 0.0,
                    });
                    bit = true;
                }
            }
            // The hit-stop: the swing held a moment where the blade bit.
            if bit && let Some(a) = p.animation_mut(rig.attack) {
                a.set_speed(0.0);
                rig.halt = Some(now + STOP);
            }
            return;
        }
        if t < SWING.1 {
            return;
        }
        rig.swing = None;
        rig.playing = None;
        commands.entity(body).remove::<Swinging>();
    }
    let Ok(mut p) = players.get_mut(rig.player) else {
        return;
    };
    let input = Vec2::new(
        (keys.pressed(KeyCode::KeyD) as i32 - keys.pressed(KeyCode::KeyA) as i32) as f32,
        (keys.pressed(KeyCode::KeyW) as i32 - keys.pressed(KeyCode::KeyS) as i32) as f32,
    );
    let dir = screen_move(input, crate::ruin::camera::YAW);
    let walk = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let clip = if dir == Vec2::ZERO {
        rig.idle
    } else if walk {
        rig.walk
    } else {
        rig.run
    };
    if rig.playing != Some(clip) {
        tx.play(&mut p, clip, BLEND).repeat();
        rig.playing = Some(clip);
    }
    if dir == Vec2::ZERO {
        return;
    }
    // The props and creatures he walks round (footprint.rs).
    let circles = footprint::world(&footprints);
    let step = dir * if walk { WALK } else { RUN } * dt;
    match step_to(&ground, nav.as_deref(), &circles, here, step) {
        Some(to) => tr.translation = to.extend((ground.floor)(to)).xzy(),
        None => debug!("hero blocked at {here:.2}, frame {}", frame.0),
    }
    if frame.0.is_multiple_of(10) {
        debug!("hero at {:.2}, frame {}", tr.translation.xz(), frame.0);
    }
    let k = 1.0 - libm::expf(-TURN * dt);
    tr.rotation = tr.rotation.slerp(facing(dir), k);
}

/// Where a step `step` from `here` lands, past what blocks it (the layout's ground, the nav
/// grid, the props' and creatures' `circles`, footprint.rs): the whole step, else along the
/// circles he meets, the step's part into each taken out (collide and slide, as Quake's
/// `PM_ClipVelocity`, so he goes round a body rather than stopping at it), else along x or
/// z alone (a grid cell's wall); None where all are blocked.
pub(crate) fn step_to(
    ground: &Ground,
    nav: Option<&crate::nav::Nav>,
    circles: &[(Vec2, f32)],
    here: Vec2,
    step: Vec2,
) -> Option<Vec2> {
    let open = |q: Vec2| {
        (ground.open)(q)
            && nav.is_none_or(|n| n.open_at(q))
            && !footprint::blocked(circles, here, q)
    };
    // Each circle within a step of touching him, the way into it taken out of the step.
    let slid = circles.iter().fold(step, |s, &(c, r)| {
        let n = (here - c).normalize_or_zero();
        let into = s.dot(n);
        let near = here.distance(c) < r + footprint::BODY + step.length();
        if near && into < 0.0 { s - n * into } else { s }
    });
    [step, slid, Vec2::new(step.x, 0.0), Vec2::new(0.0, step.y)]
        .into_iter()
        .filter(|s| *s != Vec2::ZERO)
        .map(|s| here + s)
        .find(|q| open(*q))
}

/// The camera on the hero's chest at its fixed angle and distance, catching up smoothly.
fn follow(
    time: Res<Time>,
    hero: Query<&Transform, With<Hero>>,
    mut camera: Query<&mut Transform, (With<MainCamera>, Without<Hero>)>,
    mut on: Local<bool>,
) {
    let (Ok(h), Ok(mut c)) = (hero.single(), camera.single_mut()) else {
        return;
    };
    let to = h.translation + Vec3::Y * 1.2 + crate::ruin::camera::offset();
    // On him at once the first time, not easing in from the staged view through the scene.
    let k = if *on {
        1.0 - libm::expf(-FOLLOW * time.delta_secs())
    } else {
        1.0
    };
    *on = true;
    c.translation = c.translation.lerp(to, k);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ruin::camera::{YAW, offset};

    /// W goes up the screen, D to its right: the camera's own forward and right laid on the
    /// ground, not world north and east.
    #[test]
    fn keys_move_relative_to_the_screen() {
        let cam = Transform::from_translation(offset()).looking_at(Vec3::ZERO, Vec3::Y);
        let flat = |v: Vec3| v.xz().normalize();
        let w = screen_move(Vec2::Y, YAW);
        let d = screen_move(Vec2::X, YAW);
        assert!(w.distance(flat(*cam.forward())) < 1e-4, "W {w}");
        assert!(d.distance(flat(*cam.right())) < 1e-4, "D {d}");
        assert!(screen_move(-Vec2::Y, YAW).distance(-w) < 1e-4);
        // Not world north: the camera is turned.
        assert!(w.distance(Vec2::NEG_Y) > 0.1);
        // Two keys at once go no faster.
        assert!((screen_move(Vec2::ONE, YAW).length() - 1.0).abs() < 1e-4);
        assert_eq!(screen_move(Vec2::ZERO, YAW), Vec2::ZERO);
    }

    /// Each street's roster (its file's `foes`) is models that exist, standing about the
    /// hero clear of his body and the first within his blow's reach, which its stage aims at.
    #[test]
    fn the_streets_foes_stand_in_reach() {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        for name in ["neon-street", "wasteland"] {
            let biome = crate::kit::biome::Biome::load(name).unwrap();
            let st = biome.street.clone().unwrap();
            assert!(!biome.foes.is_empty(), "{name}: play's roster is empty");
            let stage = street_stage(&st, &biome.foes);
            assert_eq!(stage.knight, Vec2::new(st.focus.0, st.focus.1));
            assert_eq!(stage.floor, street_floor(&st, stage.knight));
            for foe in &biome.foes {
                assert!(
                    assets.join(&foe.model).is_file(),
                    "{name}: no {}",
                    foe.model
                );
                assert!(
                    off(foe).length() > 1.5,
                    "{name} {foe:?}: too close to swing at"
                );
            }
            let first = off(&biome.foes[0]);
            assert!(
                first.length() <= REACH,
                "{name}: the first foe out of reach"
            );
            assert_eq!(stage.foe, stage.knight + first);
        }
        let st = crate::kit::biome::Biome::load("neon-street")
            .unwrap()
            .street
            .unwrap();
        assert_eq!(street_floor(&st, Vec2::ZERO), 0.0, "the road");
        assert_eq!(
            street_stage(&st, &[]).foe,
            Vec2::new(st.focus.0, st.focus.1) + Vec2::Y
        );
    }

    /// The swing faces the click: the model's +z turned onto the way to it, and only what
    /// stands in its arc within reach is struck.
    #[test]
    fn the_attack_faces_the_click_and_strikes_its_arc() {
        let at = Vec2::new(1.0, 2.0);
        for click in [
            Vec2::new(4.0, 2.0),
            Vec2::new(1.0, -3.0),
            Vec2::new(-2.0, 5.0),
        ] {
            let face = (click - at).normalize();
            let fwd = (facing(face) * Vec3::Z).xz();
            assert!(fwd.distance(face) < 1e-4, "{click}: faces {fwd}");
        }
        let face = Vec2::X;
        assert!(in_arc(at, face, at + Vec2::new(2.4, 0.5)));
        assert!(!in_arc(at, face, at + Vec2::new(-2.0, 0.0)), "behind");
        assert!(
            !in_arc(at, face, at + Vec2::new(0.5, 2.5)),
            "beside, past the arc"
        );
        assert!(
            !in_arc(at, face, at + Vec2::new(REACH + 0.5, 0.0)),
            "out of reach"
        );
    }
}
