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

mod strike;

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
struct Fallen;

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
        .add_plugins(strike::plugin);
    // The street stages no fight, so nothing else hangs the sword on its hero (held.rs) or
    // draws his blows: its stage (the effects aim at it from Startup) and play's effects.
    let layout = app.world().get_resource::<Args>().and_then(|a| a.layout);
    let street = app
        .world()
        .get_resource::<Biome>()
        .and_then(|b| b.street.clone());
    if layout == Some(Layout::Street)
        && let Some(st) = street
    {
        app.insert_resource(street_stage(&st))
            .add_plugins((crate::ruin::held::plugin, crate::ruin::effects::played));
    }
}

/// The street's foes in play (it stages none), from its hero: the ruin's ogre where the
/// ruin's stands from its knight (so the ruin's scripted click strikes it here too), and a
/// second behind him on the road.
const STREET_FOES: [Vec2; 2] = [Vec2::new(2.0, -1.4), Vec2::new(-2.2, 1.8)];

/// The street's floor at `p`: the pavements the kerb's rise up, the road at 0.
fn street_floor(st: &crate::kit::street::Street, p: Vec2) -> f32 {
    if p.y.abs() > st.road / 2.0 {
        st.kerb.1
    } else {
        0.0
    }
}

/// The street's stage: its hero at the focus, his first foe the one his blow aims at.
fn street_stage(st: &crate::kit::street::Street) -> crate::ruin::effects::Stage {
    let at = Vec2::new(st.focus.0, st.focus.1);
    crate::ruin::effects::Stage {
        knight: at,
        foe: at + STREET_FOES[0],
        target: at,
        floor: street_floor(st, at),
    }
}

/// A ground direction (x, z) for screen input (x right, y up the screen) under a camera
/// turned `yaw` degrees from due south (it stands at +z turned toward +x, looking back).
pub fn screen_move(input: Vec2, yaw: f32) -> Vec2 {
    let (s, c) = yaw.to_radians().sin_cos();
    let up = Vec2::new(-s, -c);
    let right = Vec2::new(c, -s);
    (right * input.x + up * input.y).normalize_or_zero()
}

/// The turn that faces a body (the models face +z) along `d`.
pub fn facing(d: Vec2) -> Quat {
    Quat::from_rotation_y(d.x.atan2(d.y))
}

/// Whether `p` is in a swing from `at` facing `face`: within `REACH` and `ARC` of its way.
pub fn in_arc(at: Vec2, face: Vec2, p: Vec2) -> bool {
    let d = p - at;
    let n = d.length();
    n <= REACH && (n < 0.3 || d.dot(face) / n >= ARC.to_radians().cos())
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
            let k = crate::wood::KNIGHT;
            let inside = rect(k - 14.0, k + 14.0);
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
            Ground {
                floor: Box::new(move |p| if p.y.abs() > half { rise } else { 0.0 }),
                open: Box::new(inside),
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
    let Some(st) = biome.and_then(|b| b.street.clone()) else {
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
    let ogre = "models/iso/ogre.glb";
    for off in STREET_FOES {
        let p = at + off;
        let yaw = (-off).x.atan2((-off).y);
        let floor = street_floor(&st, p);
        let foe = crate::scene::model(&mut commands, &asset, ogre, p.extend(floor).xzy(), yaw, 1.0);
        // Darker than the ground about it as the ruin's (fight.rs `value`), a faint rim as
        // the street's people, its contact shadow under its feet (kit/contact.rs).
        commands.entity(foe).insert((
            crate::rim::Fighter::valued(0.5, 0.35),
            crate::kit::contact::Feet { floor },
            Pose {
                gltf: asset.load(ogre),
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
    foes: Query<(Entity, &Transform), (With<Pose>, Without<Hero>, Without<Fallen>)>,
    mut players: Query<&mut AnimationPlayer>,
    mut transitions: Query<&mut AnimationTransitions>,
) {
    // The staged fight's blow, slash and bolts held back while he plays (walk.rs).
    if let Some(mut w) = walking {
        w.now = true;
    }
    let (Some(ground), Ok((body, mut tr, mut rig))) = (ground, hero.single_mut()) else {
        return;
    };
    let dt = time.delta_secs();
    let here = tr.translation.xz();
    let (Ok(mut p), Ok(mut tx)) = (players.get_mut(rig.player), transitions.get_mut(rig.player))
    else {
        return;
    };
    // A click: the window's cursor, or the script's at this frame.
    let click = if mouse.just_pressed(MouseButton::Left) {
        windows.single().ok().and_then(|w| w.cursor_position())
    } else {
        args.clicks.iter().find(|c| c.1 == frame.0).map(|c| c.0)
    };
    if rig.swing.is_none()
        && let Some(px) = click
        && let Ok((cam, cam_tr)) = camera.single()
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
                    commands.entity(foe).insert(strike::Blow { from: here, t });
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
    let open = |q: Vec2| (ground.open)(q) && nav.as_ref().is_none_or(|n| n.open_at(q));
    let step = dir * if walk { WALK } else { RUN } * dt;
    // Slide along what blocks him: the whole step, else along x or z alone.
    if let Some(to) = [step, Vec2::new(step.x, 0.0), Vec2::new(0.0, step.y)]
        .into_iter()
        .map(|s| here + s)
        .find(|q| open(*q))
    {
        tr.translation = to.extend((ground.floor)(to)).xzy();
    }
    let k = 1.0 - (-TURN * dt).exp();
    tr.rotation = tr.rotation.slerp(facing(dir), k);
}

/// The camera on the hero's chest at its fixed angle and distance, catching up smoothly.
fn follow(
    time: Res<Time>,
    hero: Query<&Transform, (With<Hero>, With<Rig>)>,
    mut camera: Query<&mut Transform, (With<MainCamera>, Without<Hero>)>,
    mut on: Local<bool>,
) {
    let (Ok(h), Ok(mut c)) = (hero.single(), camera.single_mut()) else {
        return;
    };
    let to = h.translation + Vec3::Y * 1.2 + crate::ruin::camera::offset();
    // On him at once the first time, not easing in from the staged view through the scene.
    let k = if *on {
        1.0 - (-FOLLOW * time.delta_secs()).exp()
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

    /// The street's foes stand in reach of the hero, each on its own floor (the road or the
    /// pavement), and its stage aims his blow at the first.
    #[test]
    fn the_streets_foes_stand_in_reach() {
        let st = crate::kit::biome::Biome::load("neon-street")
            .unwrap()
            .street
            .unwrap();
        let stage = street_stage(&st);
        assert_eq!(stage.knight, Vec2::new(st.focus.0, st.focus.1));
        assert_eq!(stage.floor, street_floor(&st, stage.knight));
        for off in STREET_FOES {
            let foe = stage.knight + off;
            assert!(in_arc(stage.knight, off.normalize(), foe), "{off}");
            assert!(off.length() > 1.5, "{off}: too close to swing at");
        }
        assert_eq!(stage.foe, stage.knight + STREET_FOES[0]);
        assert_eq!(street_floor(&st, Vec2::ZERO), 0.0, "the road");
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
