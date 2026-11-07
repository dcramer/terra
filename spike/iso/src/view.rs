//! The asset view (`--asset NAME --shot dir/NAME.png`): one asset alone on neutral grey
//! ground at the ARPG camera's angle and distance (ruin/camera.rs's lens, pitch, yaw and 19 m),
//! lit by a neutral white key from the camera's left and a white sky fill, so it is judged at
//! the size it plays at and in no scene's colour (the skill's asset rubric). The frame is the
//! game's 1080p frame cropped to the asset: the lens narrowed so a metre covers the same
//! pixels, the crop fitted to the asset's measured box (its skeleton's joints for a skinned
//! body, its meshes' boxes otherwise; a body with clips gets room to fall its height in any
//! direction). Writes `NAME.png`, the still in its idle's first frame (or its first clip's),
//! then a strip of `STRIP` frames through every clip, `NAME-<clip>-<k>.jpg`, and `NAME.json`
//! (the clips, their seconds and files, the crop and the box) for tools/iso-art's sheet.
//!
//! NAME is a path under assets/ (`models/creatures/Fox.glb`), or a bare name for
//! `models/iso/NAME.glb`. The knight holds his sword, as every fight gives it him (fight.rs).

use crate::Args;
use crate::camera::Offscreen;
use crate::ruin::camera::{DISTANCE, FOV, PITCH, YAW, arpg};
use bevy::camera::primitives::Aabb;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use std::fmt::Write as _;

/// Frames a clip's strip shows.
pub const STRIP: usize = 6;
/// Frames each pose is held before its shot, so the temporal AA and shadows settle.
const SETTLE: u32 = 24;
/// The neutral light: the forest by day's sun, sky and exposure (wood/mod.rs), all white.
const SUN_LUX: f32 = 3_200.0;
const SKY_NITS: f32 = 330.0;
const EV100: f32 = 8.0;
/// The ground's grey, a mid ground's value (sRGB).
const GROUND: f32 = 0.32;
/// The game's frame height the crop keeps the scale of.
const FRAME_H: f32 = 1080.0;
/// Clips shown first, in this order (by name, any case); the rest follow by name.
const ORDER: [&str; 5] = ["idle", "walk", "run", "attack", "death"];
/// A clip whose name has one of these loops: its strip stops short of its end, which is its start.
const LOOPS: [&str; 7] = ["idle", "walk", "run", "trot", "gallop", "fly", "swim"];

/// The asset's path under assets/, from `--asset`.
pub fn path(name: &str) -> String {
    if name.ends_with(".glb") || name.ends_with(".gltf") {
        name.to_string()
    } else {
        format!("models/iso/{name}.glb")
    }
}

pub fn plugin(app: &mut App) {
    let args = app.world().resource::<Args>().clone();
    let name = args.asset.clone().unwrap_or_default();
    // The fight's plugin hangs the sword on the body standing at its stage's knight place.
    let knight = if name == "knight" {
        Vec2::ZERO
    } else {
        Vec2::splat(1e4)
    };
    app.insert_resource(crate::ruin::effects::Stage {
        knight,
        ..default()
    })
    .add_plugins(crate::ruin::fight::plugin)
    .add_systems(Startup, (ground, light, spawn))
    .add_systems(Update, step);
}

#[derive(Component)]
struct Subject;

/// What the box is measured from: each part's place, a mesh's box, a skin's joints.
type Parts<'w, 's> = Query<
    'w,
    's,
    (
        &'static GlobalTransform,
        Option<&'static Aabb>,
        Option<&'static SkinnedMesh>,
    ),
>;

/// One shot: a clip (none for an asset without clips) at a time, saved to a file.
struct Shot {
    clip: Option<usize>,
    at: f32,
    file: String,
}

#[derive(Default)]
struct State {
    gltf: Option<Handle<Gltf>>,
    /// Frames since the asset was in, its graph built and its first pose set.
    since: u32,
    clips: Vec<(String, f32, AnimationNodeIndex)>,
    player: Option<Entity>,
    shots: Vec<Shot>,
    next: usize,
    held: u32,
    json: String,
}

fn ground(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(60.0, 60.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(GROUND, GROUND, GROUND),
            perceptual_roughness: 0.9,
            ..default()
        })),
    ));
}

fn light(mut commands: Commands) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: SKY_NITS,
        ..default()
    });
    // From 50 degrees to the camera's left, 50 degrees up: a portrait's key.
    let (az, el) = ((YAW - 50.0).to_radians(), 50.0f32.to_radians());
    let from = Vec3::new(az.sin() * el.cos(), el.sin(), az.cos() * el.cos());
    commands.spawn((
        DirectionalLight {
            illuminance: SUN_LUX,
            shadow_maps_enabled: true,
            soft_shadow_size: Some(0.5),
            ..default()
        },
        bevy::light::CascadeShadowConfigBuilder {
            num_cascades: 1,
            minimum_distance: 1.0,
            maximum_distance: 40.0,
            ..default()
        }
        .build(),
        Transform::from_translation(from).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn spawn(mut commands: Commands, asset: Res<AssetServer>, args: Res<Args>) {
    let path = path(args.asset.as_deref().unwrap_or_default());
    let body = crate::scene::model(&mut commands, &asset, &path, Vec3::ZERO, 0.0, 1.0);
    commands.entity(body).insert(Subject);
}

#[allow(clippy::too_many_arguments)]
fn step(
    mut commands: Commands,
    mut state: Local<State>,
    args: Res<Args>,
    asset: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    clips: Res<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut images: ResMut<Assets<Image>>,
    subject: Query<Entity, With<Subject>>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    mut transitions: Query<&mut AnimationTransitions>,
    parts: Parts,
    offscreen: Option<Res<Offscreen>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(body) = subject.single() else {
        return;
    };
    let name = args.asset.clone().unwrap_or_default();
    // Held, or the file is dropped and loaded again every frame and never comes in.
    let handle = state
        .gltf
        .get_or_insert_with(|| asset.load::<Gltf>(path(&name)))
        .clone();
    let Some(gltf) = gltfs.get(&handle) else {
        return;
    };
    // Once: the clips into a graph, the first pose, and the shots' list.
    if state.since == 0 {
        let has_clips = !gltf.named_animations.is_empty();
        let player = children
            .iter_descendants(body)
            .find(|e| players.contains(*e));
        if has_clips && player.is_none() {
            return;
        }
        let mut named: Vec<(String, Handle<AnimationClip>)> = gltf
            .named_animations
            .iter()
            .map(|(n, h)| (n.to_string(), h.clone()))
            .collect();
        if named.iter().any(|(_, h)| clips.get(h).is_none()) {
            return;
        }
        let rank = |n: &str| {
            let l = n.to_lowercase();
            ORDER
                .iter()
                .position(|o| l.contains(o))
                .unwrap_or(ORDER.len())
        };
        named.sort_by(|a, b| (rank(&a.0), &a.0).cmp(&(rank(&b.0), &b.0)));
        let (graph, nodes) = AnimationGraph::from_clips(named.iter().map(|(_, h)| h.clone()));
        if let Some(p) = player {
            commands
                .entity(p)
                .insert(AnimationGraphHandle(graphs.add(graph)));
        }
        state.player = player;
        state.clips = named
            .iter()
            .zip(nodes)
            .map(|((n, h), node)| (n.clone(), clips.get(h).map_or(0.0, |c| c.duration()), node))
            .collect();
        let shot = args
            .shot
            .clone()
            .expect("--asset needs --shot dir/NAME.png");
        let dir = shot.parent().map(|d| d.to_path_buf()).unwrap_or_default();
        let stem = shot
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("asset")
            .to_string();
        let still = state
            .clips
            .iter()
            .position(|c| c.0.to_lowercase().contains("idle"));
        let still = still.or((!state.clips.is_empty()).then_some(0));
        state.shots.push(Shot {
            clip: still,
            at: 0.0,
            file: shot.to_string_lossy().into_owned(),
        });
        let mut json = format!(
            "{{\n \"asset\": \"{}\",\n \"still\": \"{}\",\n \"clips\": [",
            path(&name),
            shot.file_name().unwrap().to_string_lossy()
        );
        for (i, (clip, seconds, _)) in state.clips.clone().iter().enumerate() {
            let looped = LOOPS.iter().any(|l| clip.to_lowercase().contains(l));
            let mut files = Vec::new();
            for k in 0..STRIP {
                let at = if looped {
                    seconds * k as f32 / STRIP as f32
                } else {
                    seconds * k as f32 / (STRIP - 1) as f32
                };
                let file = format!("{stem}-{}-{k}.jpg", clip.replace(['/', ' ', '|'], "_"));
                files.push(format!("{{\"at\": {at:.3}, \"file\": \"{file}\"}}"));
                state.shots.push(Shot {
                    clip: Some(i),
                    // Just inside a clip's end, which a seek past would wrap.
                    at: at.min(seconds - 1e-3).max(0.0),
                    file: dir.join(file).to_string_lossy().into_owned(),
                });
            }
            let sep = if i == 0 { "" } else { "," };
            let _ = write!(
                json,
                "{sep}\n  {{\"name\": \"{clip}\", \"seconds\": {seconds:.3}, \"loops\": {looped}, \"frames\": [{}]}}",
                files.join(", ")
            );
        }
        json.push_str("\n ]");
        state.json = json;
        pose(&mut state, &mut players);
        state.since = 1;
        return;
    }
    state.since += 1;
    // A few frames for the pose to reach the joints, then the crop from the box, once.
    if offscreen.is_none() {
        if state.since < 4 {
            return;
        }
        let (min, max) = bounds(body, &children, &parts);
        let room = if state.clips.is_empty() {
            0.0
        } else {
            max.y - min.y
        };
        let mid = (min + max) / 2.0;
        let half = Vec3::new(
            ((max.x - min.x) / 2.0).max(room),
            (max.y - min.y) / 2.0,
            ((max.z - min.z) / 2.0).max(room),
        );
        let (size, fov) = crop(mid, half);
        let mut a = args.clone();
        a.size = size;
        let cam = arpg(&mut commands, &a, &mut images, mid, EV100);
        commands
            .entity(cam)
            .insert(Projection::from(PerspectiveProjection {
                fov,
                near: 1.0,
                far: 120.0,
                ..default()
            }));
        let _ = write!(
            state.json,
            ",\n \"crop\": [{}, {}],\n \"box\": [[{:.3}, {:.3}, {:.3}], [{:.3}, {:.3}, {:.3}]],\n \"px_per_m\": {:.1}\n}}\n",
            size.0,
            size.1,
            min.x,
            min.y,
            min.z,
            max.x,
            max.y,
            max.z,
            FRAME_H / 2.0 / (FOV / 2.0).to_radians().tan() / DISTANCE
        );
        let file = args.shot.clone().unwrap().with_extension("json");
        std::fs::write(&file, &state.json).expect("the view's json beside the still");
        info!(
            "asset view {name}: crop {size:?}, box {min} to {max}, {} clips",
            state.clips.len()
        );
        state.held = 0;
        return;
    }
    let Some(image) = offscreen else { return };
    state.held += 1;
    if args.film > 0 {
        let (p, t) = (&mut players, &mut transitions);
        film(&mut state, &args, &image, &mut commands, p, t, &mut exit);
        return;
    }
    if state.next >= state.shots.len() {
        if state.held == 30 {
            exit.write(AppExit::Success);
        }
        return;
    }
    match act(state.held) {
        Act::Shoot => {
            let file = state.shots[state.next].file.clone();
            commands
                .spawn(Screenshot::image(image.0.clone()))
                .observe(save_to_disk(file));
        }
        Act::Pose => {
            state.next += 1;
            state.held = 0;
            if state.next < state.shots.len() {
                pose(&mut state, &mut players);
            }
        }
        Act::Hold => {}
    }
}

/// Frames a shot's pose is kept after its screenshot is asked for. The screenshot is of the
/// frame it is spawned in, whose PostUpdate applies the pose set in its Update: a pose set
/// beside the screenshot is the one it shows, so every strip frame was the next shot's pose
/// (each clip's last frame the next clip's first) until iso-art j31.
const AFTER: u32 = 2;

#[derive(Debug, PartialEq)]
enum Act {
    Hold,
    Shoot,
    Pose,
}

/// What the frame `held` frames into a shot does: settle, shoot, then set the next pose.
fn act(held: u32) -> Act {
    if held == SETTLE {
        Act::Shoot
    } else if held == SETTLE + AFTER {
        Act::Pose
    } else {
        Act::Hold
    }
}

/// The gaits the film plays, by the clips' names.
#[derive(Debug, PartialEq, Clone, Copy)]
enum Gait {
    Walk,
    Run,
}

/// Each gait's loops in a turn of the film: walk three times, then run three times, again.
const LAPS: f32 = 3.0;
/// The game's crossfade between gaits (party.rs `gait`).
const FADE: std::time::Duration = std::time::Duration::from_millis(220);

/// The gait the film switches to at frame `n`, a turn being `walk` frames of walk then `run`
/// of run (`run` 0 when the asset has no run); none between switches.
fn switch(n: u32, walk: u32, run: u32) -> Option<Gait> {
    let turn = walk + run;
    if turn == 0 {
        return (n == 0).then_some(Gait::Walk);
    }
    match n % turn {
        0 => Some(Gait::Walk),
        p if p == walk && run > 0 => Some(Gait::Run),
        _ => None,
    }
}

/// `--film N` with `--asset`: the walk and run played as the game plays them (repeat at speed
/// 1, the game's crossfade at each switch, game time at 30 frames a second), each frame saved
/// as `NAME-f<nnnn>.jpg` beside `--shot`, so a loop's seam and the switch from walk to run
/// can be seen to hold (iso-art j31). The walk's first pose is held while the frame settles.
#[allow(clippy::too_many_arguments)]
fn film(
    state: &mut State,
    args: &Args,
    image: &Offscreen,
    commands: &mut Commands,
    players: &mut Query<&mut AnimationPlayer>,
    transitions: &mut Query<&mut AnimationTransitions>,
    exit: &mut MessageWriter<AppExit>,
) {
    let find = |gait: &str| {
        let clip = state
            .clips
            .iter()
            .find(|c| c.0.to_lowercase().contains(gait));
        clip.map(|c| (c.1, c.2))
    };
    let (Some(walk), run) = (find("walk"), find("run")) else {
        panic!(
            "--film with --asset plays the walk and run, and {:?} has no walk",
            args.asset
        );
    };
    let Some(p) = state.player else { return };
    let frames = |secs: f32| (LAPS * secs * crate::FILM_FPS as f32).round() as u32;
    let (walk_n, run_n) = (frames(walk.0), run.map_or(0, |r| frames(r.0)));
    let (Ok(mut player), Ok(mut fades)) = (players.get_mut(p), transitions.get_mut(p)) else {
        // Its transitions go on once, and are there the next frame.
        commands.entity(p).insert(AnimationTransitions::new());
        return;
    };
    if state.held < SETTLE {
        if fades.get_main_animation().is_none() {
            player.stop_all();
            let zero = std::time::Duration::ZERO;
            fades
                .play(&mut player, walk.1, zero)
                .repeat()
                .set_speed(0.0);
        }
        return;
    }
    let n = state.held - SETTLE;
    match (n, switch(n, walk_n, run_n), run) {
        (0, _, _) => _ = player.animation_mut(walk.1).map(|a| a.set_speed(1.0)),
        (_, Some(Gait::Walk), _) => _ = fades.play(&mut player, walk.1, FADE).repeat(),
        (_, Some(Gait::Run), Some(run)) => _ = fades.play(&mut player, run.1, FADE).repeat(),
        _ => {}
    }
    if n < args.film {
        let shot = args.shot.clone().unwrap();
        let stem = shot.file_stem().and_then(|s| s.to_str()).unwrap_or("asset");
        let file = shot.with_file_name(format!("{stem}-f{n:04}.jpg"));
        commands
            .spawn(Screenshot::image(image.0.clone()))
            .observe(save_to_disk(file));
    } else if n == args.film + 30 {
        exit.write(AppExit::Success);
    }
}

/// Sets the next shot's pose, held still.
fn pose(state: &mut State, players: &mut Query<&mut AnimationPlayer>) {
    let shot = &state.shots[state.next];
    let (Some(p), Some(c)) = (state.player, shot.clip) else {
        return;
    };
    let Ok(mut player) = players.get_mut(p) else {
        return;
    };
    player.stop_all();
    player
        .play(state.clips[c].2)
        .seek_to(shot.at)
        .set_speed(0.0);
}

/// The asset's world box: a skinned mesh by its joints (its mesh box is the bind pose's), any
/// other by its box, with a margin for the flesh over a skeleton.
fn bounds(body: Entity, children: &Query<&Children>, parts: &Parts) -> (Vec3, Vec3) {
    let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    let mut skinned = false;
    for e in children.iter_descendants(body) {
        let Ok((tr, aabb, skin)) = parts.get(e) else {
            continue;
        };
        if let Some(skin) = skin {
            skinned = true;
            for j in &skin.joints {
                if let Ok((jt, _, _)) = parts.get(*j) {
                    min = min.min(jt.translation());
                    max = max.max(jt.translation());
                }
            }
        } else if let Some(b) = aabb {
            let (c, h) = (Vec3::from(b.center), Vec3::from(b.half_extents));
            for i in 0..8 {
                let s = Vec3::new(
                    if i & 1 == 0 { -1.0 } else { 1.0 },
                    if i & 2 == 0 { -1.0 } else { 1.0 },
                    if i & 4 == 0 { -1.0 } else { 1.0 },
                );
                let p = tr.transform_point(c + h * s);
                min = min.min(p);
                max = max.max(p);
            }
        }
    }
    if min.x > max.x {
        return (Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 1.0, 0.5));
    }
    if skinned {
        let m = (max - min).max_element() * 0.15 + 0.1;
        min -= Vec3::new(m, 0.0, m);
        max += Vec3::splat(m);
        min.y = min.y.min(0.0);
    }
    (min, max)
}

/// The crop of the game's 1080p frame that holds a box (centre, half extents) seen from the
/// ARPG camera on its centre, and the narrowed lens that keeps a metre's pixels.
fn crop(mid: Vec3, half: Vec3) -> ((u32, u32), f32) {
    let (p, y) = (PITCH.to_radians(), YAW.to_radians());
    let eye = mid + Vec3::new(p.cos() * y.sin(), p.sin(), p.cos() * y.cos()) * DISTANCE;
    let view = Transform::from_translation(eye).looking_at(mid, Vec3::Y);
    let k = FRAME_H / 2.0 / (FOV / 2.0).to_radians().tan();
    let (mut w, mut h) = (0.0f32, 0.0f32);
    for i in 0..8 {
        let s = Vec3::new(
            if i & 1 == 0 { -1.0 } else { 1.0 },
            if i & 2 == 0 { -1.0 } else { 1.0 },
            if i & 4 == 0 { -1.0 } else { 1.0 },
        );
        let v = view.rotation.inverse() * (mid + half * s - eye);
        w = w.max((k * v.x / -v.z).abs());
        h = h.max((k * v.y / -v.z).abs());
    }
    let even =
        |v: f32, most: f32| (((2.0 * v + 48.0) / 16.0).ceil() * 16.0).clamp(160.0, most) as u32;
    let size = (even(w, 1920.0), even(h, FRAME_H));
    let fov = 2.0 * (size.1 as f32 / FRAME_H * (FOV / 2.0).to_radians().tan()).atan();
    (size, fov)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The crop keeps the game frame's scale: a metre at the focus covers the same pixels in
    /// the crop as in the full 1080p frame, by the lens's own geometry.
    #[test]
    fn the_crop_keeps_a_metre_s_pixels() {
        let (size, fov) = crop(Vec3::new(0.0, 0.9, 0.0), Vec3::new(0.5, 0.9, 0.5));
        let crop_px = size.1 as f32 / 2.0 / (fov / 2.0).tan() / DISTANCE;
        let game_px = FRAME_H / 2.0 / (FOV / 2.0).to_radians().tan() / DISTANCE;
        assert!(
            (crop_px - game_px).abs() < 0.01,
            "{crop_px} against {game_px}"
        );
        // A 1.8 m body seen 35 degrees off its side is about 1.5 m tall on screen.
        let tall = 1.8 * (90.0 - PITCH).to_radians().cos() * game_px;
        assert!(
            size.1 as f32 > tall && (size.1 as f32) < tall + 160.0,
            "{size:?} for {tall} px"
        );
    }

    /// A shot shows its own pose: nothing sets the next pose in the frame its screenshot is
    /// asked for, nor in the frame after; each shot is taken once, then posed past once.
    #[test]
    fn a_shot_is_taken_before_the_next_pose_is_set() {
        let acts: Vec<Act> = (1..=SETTLE + AFTER).map(act).collect();
        let shot = acts.iter().position(|a| *a == Act::Shoot).expect("a shot");
        let posed = acts
            .iter()
            .position(|a| *a == Act::Pose)
            .expect("the next pose");
        assert!(
            posed >= shot + 2,
            "the shot at frame {shot}, the next pose at {posed}"
        );
        assert_eq!(acts.iter().filter(|a| **a != Act::Hold).count(), 2);
        assert_eq!(
            *acts.last().unwrap(),
            Act::Pose,
            "the held count resets on the pose"
        );
    }

    /// The film walks three laps, runs three, then walks again; an asset with no run walks.
    #[test]
    fn the_film_switches_at_each_turn() {
        let (walk, run) = (210, 57);
        let at: Vec<(u32, Gait)> = (0..2 * (walk + run))
            .filter_map(|n| switch(n, walk, run).map(|g| (n, g)))
            .collect();
        let want = [
            (0, Gait::Walk),
            (walk, Gait::Run),
            (walk + run, Gait::Walk),
            (2 * walk + run, Gait::Run),
        ];
        assert_eq!(at, want);
        assert_eq!(
            (0..500).filter_map(|n| switch(n, walk, 0)).count(),
            3,
            "walk again each turn"
        );
    }
}
