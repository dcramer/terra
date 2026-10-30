//! The bone audit (D584; `--bone-audit <file.jsonl>`, run by `tools/controller/bones.ts`):
//! how far each layer that poses a body after its clips turns and moves each bone of the
//! player's body, every counted frame, a line of JSON each. Between consecutive pose writers
//! it takes a snapshot of every bone's own transform, the writers held in the one order their
//! constraints allow, so each difference is one layer's alone. A line holds the frame, the
//! body's speed over the ground and its distance from the planet's centre (the ground's rise),
//! and each layer's bones it moved: degrees of their turn and centimetres of their travel.
//! Off unless the option names a file, when it adds nothing to the schedule.

use bevy::prelude::*;
use serde_json::{Map, Value, json};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use terra_world::shell::{Options, ScriptFrame};

/// The layers in the order they pose a body (each after the one before it), as the record
/// names them: the arm held clear for a carry, the fit to the body (its joints held, its lift),
/// the carried tool tipped at the wrist, the fists closed, the feet kept, the arms kept off
/// the hips, the shield arm fitted, the head turned toward the aim.
pub const LAYERS: [&str; 8] = [
    "carry_arm",
    "fitted",
    "carry_hand",
    "grip",
    "feet",
    "arms",
    "shield_arm",
    "gaze",
];
/// The least turn or travel a line keeps for a bone, so it stays short: under it, still.
const LEAST_DEG: f32 = 0.05;
const LEAST_CM: f32 = 0.05;

/// The body the audit reads: the local player's.
#[derive(Component)]
pub struct Audited;

#[derive(Resource)]
struct Audit {
    file: BufWriter<File>,
    /// Each stage's bones, by name, as they stood before the layer after it.
    snaps: [Vec<(Name, Transform)>; LAYERS.len() + 1],
}

pub fn plugin(app: &mut App) {
    let Some(path) = app
        .world()
        .get_resource::<Options>()
        .and_then(|options| options.bone_audit.clone())
    else {
        return;
    };
    add(app, &path);
    app.add_systems(Update, mark);
}

/// The audit written to `path`, its snapshots between the pose writers.
pub fn add(app: &mut App, path: &Path) {
    let file = File::create(path).unwrap_or_else(|problem| {
        panic!(
            "--bone-audit: could not write {} ({problem}); give a file in a folder that exists",
            path.display()
        )
    });
    app.insert_resource(Audit {
        file: BufWriter::new(file),
        snaps: Default::default(),
    });
    use bevy::app::AnimationSystems;
    use bevy::transform::TransformSystems;
    app.add_systems(
        PostUpdate,
        (
            snap::<0>
                .after(AnimationSystems)
                .before(crate::carry_arm::hold_in_body),
            snap::<1>
                .after(crate::carry_arm::hold_in_body)
                .before(crate::Fitted),
            snap::<2>
                .after(crate::Fitted)
                .before(crate::carry_hand::tip_carried),
            snap::<3>
                .after(crate::carry_hand::tip_carried)
                .before(crate::grip::close_hands),
            snap::<4>
                .after(crate::grip::close_hands)
                .before(crate::feet::keep),
            snap::<5>
                .after(crate::feet::keep)
                .before(crate::arms::keep_off_hips),
            snap::<6>
                .after(crate::arms::keep_off_hips)
                .before(crate::shield_arm::fit_shield_arm),
            snap::<7>
                .after(crate::shield_arm::fit_shield_arm)
                .before(crate::gaze::keep_on_aim),
            (snap::<8>, write)
                .chain()
                .after(crate::gaze::keep_on_aim)
                .before(TransformSystems::Propagate),
        ),
    );
}

/// The local player's body, once it is in, is the one audited.
fn mark(
    bodies: Query<(Entity, &ChildOf), (Added<crate::character::Body>, Without<Audited>)>,
    players: Query<(), With<crate::LocalPlayer>>,
    mut commands: Commands,
) {
    for (body, parent) in &bodies {
        if players.contains(parent.parent()) {
            commands.entity(body).insert(Audited);
        }
    }
}

fn bones(
    root: Entity,
    children: &Query<&Children>,
    named: &Query<(&Name, &Transform)>,
    out: &mut Vec<(Name, Transform)>,
) {
    if let Ok((name, place)) = named.get(root) {
        out.push((name.clone(), *place));
    }
    for child in children.get(root).into_iter().flatten() {
        bones(*child, children, named, out);
    }
}

fn snap<const K: usize>(
    mut audit: ResMut<Audit>,
    bodies: Query<Entity, With<Audited>>,
    children: Query<&Children>,
    named: Query<(&Name, &Transform)>,
) {
    let mut out = std::mem::take(&mut audit.snaps[K]);
    out.clear();
    if let Some(body) = bodies.iter().next() {
        bones(body, &children, &named, &mut out);
    }
    audit.snaps[K] = out;
}

/// The bones a layer moved between two snapshots: each one's degrees and centimetres.
pub fn moved(before: &[(Name, Transform)], after: &[(Name, Transform)]) -> Map<String, Value> {
    let mut bones = Map::new();
    for ((name, a), (_, b)) in before.iter().zip(after) {
        let deg = turn_deg(a.rotation, b.rotation);
        let cm = a.translation.distance(b.translation) * 100.0;
        if deg >= LEAST_DEG || cm >= LEAST_CM {
            bones.insert(name.to_string(), json!([tenth(deg), tenth(cm)]));
        }
    }
    bones
}

/// The turn from `a` to `b`, degrees: by the half-angle's sine and cosine, which keep it exact
/// near none, where `Quat::angle_between`'s arc cosine of a dot a hair under 1 reads a bone
/// left alone as turned a tenth of a degree.
pub fn turn_deg(a: Quat, b: Quat) -> f32 {
    let d = a.inverse() * b;
    (2.0 * libm::atan2f(d.xyz().length(), d.w.abs())).to_degrees()
}

fn tenth(x: f32) -> f64 {
    (x as f64 * 10.0).round() / 10.0
}

fn write(
    mut audit: ResMut<Audit>,
    frame: Option<Res<ScriptFrame>>,
    bodies: Query<(&ChildOf, &GlobalTransform), With<Audited>>,
    players: Query<(&crate::Motion, &GlobalTransform, Option<&crate::feet::Kept>)>,
) {
    let Some(frame) = frame.filter(|frame| frame.is_changed()) else {
        return;
    };
    let audit = &mut *audit;
    let snaps = &audit.snaps;
    // Every stage the same bones: the body is in and whole.
    if snaps[0].is_empty() || snaps.iter().any(|s| s.len() != snaps[0].len()) {
        return;
    }
    let mut layers = Map::new();
    for (k, layer) in LAYERS.iter().enumerate() {
        let bones = moved(&snaps[k], &snaps[k + 1]);
        if !bones.is_empty() {
            layers.insert(layer.to_string(), Value::Object(bones));
        }
    }
    let player = bodies
        .iter()
        .next()
        .and_then(|(parent, _)| players.get(parent.parent()).ok());
    let (speed, up) = player.map_or((0.0, 0.0), |(motion, place, _)| {
        (motion.velocity.length(), place.translation().length())
    });
    // How the foot lock kept each foot, left then right (`feet::Kept::states`): which of its
    // states bent a leg.
    let kept = player.and_then(|(.., kept)| kept).map(|kept| kept.states());
    let line = json!({
        "frame": frame.0,
        "speed": (speed as f64 * 100.0).round() / 100.0,
        "up": (up as f64 * 1000.0).round() / 1000.0,
        "kept": kept,
        "layers": layers,
    });
    let written = writeln!(audit.file, "{line}").and_then(|()| audit.file.flush());
    if let Err(problem) = written {
        warn!("--bone-audit: could not write a line ({problem})");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::ScheduleRunnerPlugin;

    /// A bone left alone reads no turn, however far from unit length its rotation drifted.
    #[test]
    fn a_bone_left_alone_reads_no_turn() {
        let q = Quat::from_xyzw(0.3, -0.5, 0.2, 0.79) * (1.0 + 3e-7);
        assert_eq!(turn_deg(q, q), 0.0);
        let turned = Quat::from_rotation_y(20f32.to_radians()) * q.normalize();
        assert!((turn_deg(q.normalize(), turned) - 20.0).abs() < 1e-3);
    }

    /// A layer that turns a bone 20 degrees and moves another 3 cm reads so, as that layer's
    /// alone: the layers before and after it read nothing.
    #[test]
    fn a_layer_turning_a_bone_20_degrees_reads_20_as_its_own() {
        let lines = audited(crate::pose::Switch::default(), "on");
        assert_eq!(lines.len(), 2, "{lines:?}");
        for line in &lines {
            let layers = line["layers"].as_object().unwrap();
            assert_eq!(layers.keys().collect::<Vec<_>>(), ["feet"], "{line}");
            let feet = &layers["feet"];
            assert!(
                (feet["calf_l"][0].as_f64().unwrap() - 20.0).abs() < 0.11,
                "{line}"
            );
            assert!(
                (feet["pelvis"][1].as_f64().unwrap() - 3.0).abs() < 0.11,
                "{line}"
            );
            assert!(feet.get("foot_l").is_none(), "{line}");
        }
    }

    /// The same layer turned off by the switch (`--pose-layers off=feet`) leaves the clip's
    /// pose as it was: the audit reads nothing for it, and the other layers off leave it on.
    #[test]
    fn a_layer_switched_off_leaves_the_clips_pose_alone() {
        use crate::pose::{Layer, Switch};
        let lines = audited(Switch::off([Layer::Feet]), "off");
        assert_eq!(lines.len(), 2, "{lines:?}");
        for line in &lines {
            assert!(line["layers"].as_object().unwrap().is_empty(), "{line}");
        }
        let others = Layer::ALL.into_iter().filter(|&l| l != Layer::Feet);
        let lines = audited(Switch::off(others), "others");
        assert!(lines.iter().all(|l| l["layers"].get("feet").is_some()));
    }

    /// The audit's lines over two frames of a stand-in layer in the foot lock's place and
    /// set, under `switch`: turning `calf_l` 20 degrees and moving `pelvis` 3 cm.
    fn audited(switch: crate::pose::Switch, tag: &str) -> Vec<Value> {
        let path =
            std::env::temp_dir().join(format!("bone-audit-{}-{tag}.jsonl", std::process::id()));
        let mut app = App::new();
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()));
        app.init_resource::<ScriptFrame>();
        add(&mut app, &path);
        crate::pose::configure(&mut app, switch);
        app.add_systems(First, |mut frame: ResMut<ScriptFrame>| frame.0 += 1);
        // In the foot lock's place: after the snapshot before it, before the one after it.
        app.add_systems(
            PostUpdate,
            (|mut bones: Query<(&Name, &mut Transform)>| {
                for (name, mut place) in &mut bones {
                    match name.as_str() {
                        "calf_l" => {
                            place.rotation = Quat::from_rotation_x(20f32.to_radians());
                        }
                        "pelvis" => place.translation = Vec3::new(0.0, 0.97, 0.0),
                        _ => {}
                    }
                }
            })
            .in_set(crate::pose::Layer::Feet)
            .after(snap::<4>)
            .before(snap::<5>),
        );
        let body = app.world_mut().spawn((Audited, Transform::default())).id();
        for (name, y) in [("pelvis", 1.0), ("calf_l", 0.5), ("foot_l", 0.1)] {
            let bone = app
                .world_mut()
                .spawn((Name::new(name), Transform::from_xyz(0.0, y, 0.0)))
                .id();
            app.world_mut().entity_mut(body).add_child(bone);
        }
        // The layer's change undone each frame, as the clips would.
        app.add_systems(Update, |mut bones: Query<(&Name, &mut Transform)>| {
            for (name, mut place) in &mut bones {
                place.rotation = Quat::IDENTITY;
                if name.as_str() == "pelvis" {
                    place.translation.y = 1.0;
                }
            }
        });
        app.update();
        app.update();
        drop(app);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).ok();
        text.lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}
