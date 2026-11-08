//! Nothing hides the hero (iso-forest-demo): in play every cuttable crown and bough opens in
//! a dithered hole about him from a hand's breadth before him, and fades near the lens, so
//! no bough sweeps over the camera as he runs under the oaks. Diablo IV and Path of Exile 2
//! do the same: occluders between the camera and the player dither out in a circle about him
//! (Diablo IV's "fade cylinder"), and canopy near the camera is not drawn.
//!
//! The check: each frame rays go from the camera to his body from knee to crown (`body`);
//! a surface met before him that the cut does not drop (`foliage::fade`, the shader's twin)
//! hides him, and is logged as `hero hidden at frame N by <name>`, counted in `Hidden`.

use crate::camera::MainCamera;
use crate::foliage::{self, FoliageMaterial, Near, Seen};
use crate::play::Hero;
use crate::rim::RimMaterial;
use bevy::picking::mesh_picking::ray_cast::{
    MeshRayCast, MeshRayCastSettings, RayCastBackfaces, RayCastVisibility,
};
use bevy::prelude::*;

/// Play's near fade (m from the camera): the camera stands about 17 m over the hero and the
/// oaks are 12 to 16 m tall, so their crowns came within a few metres of the lens.
pub const NEAR: f32 = 10.0;
/// How open a cut surface over him must be to count as not hiding him (the dither's share).
const OPEN: f32 = 0.75;
/// His middle, where the hole is cut about (foliage.rs `breathe`: his place and 0.9 m up).
pub const MIDDLE: f32 = 0.9;

pub fn plugin(app: &mut App) {
    app.insert_resource(Near(NEAR))
        .init_resource::<Hidden>()
        .add_systems(Update, (seen, backfaces))
        .add_systems(PostUpdate, check.after(TransformSystems::Propagate));
}

/// The frames something hid him in.
#[derive(Resource, Default)]
pub struct Hidden {
    pub frames: u32,
}

/// The wood opens about the hero (foliage.rs's party holes).
fn seen(mut commands: Commands, heroes: Query<Entity, (Added<Hero>, Without<Seen>)>) {
    for h in &heroes {
        commands.entity(h).insert(Seen);
    }
}

/// Leaf cards are seen from both sides, so the check's rays meet their backs too.
fn backfaces(mut commands: Commands, added: Query<Entity, Added<Mesh3d>>) {
    for e in &added {
        commands.entity(e).insert(RayCastBackfaces);
    }
}

/// The points of his body a frame must show: on his axis from knee to crown, and his flanks
/// at hip and chest (`right` the camera's right). Below the knee the ground's cover may lap
/// him, as grass laps the ARPGs' heroes.
pub fn body(feet: Vec3, right: Vec3) -> Vec<Vec3> {
    let up = |h: f32| feet + Vec3::Y * h;
    let mut p: Vec<Vec3> = [0.5, 0.9, 1.3, 1.7].into_iter().map(up).collect();
    for h in [0.9, 1.3] {
        p.push(up(h) + right * 0.25);
        p.push(up(h) - right * 0.25);
    }
    p
}

/// A surface a ray from the camera met: what (if known), where, how far along, and whether
/// play's cut drops it.
pub struct Hit {
    pub what: Option<Entity>,
    pub at: Vec3,
    pub distance: f32,
    pub cut: bool,
}

/// The points of `points` hidden, each with the surface that hides it (`(point, what, at)`):
/// `cast` gives what a ray from the eye meets, nearest first; a surface more than a hand's
/// breadth before a point hides it unless it is cuttable and the cut about `middle` (with
/// `eye`'s near fade) leaves it at least `OPEN` open.
pub fn hidden(
    eye: Vec4,
    axes: [Vec3; 3],
    middle: Vec3,
    points: &[Vec3],
    mut cast: impl FnMut(Ray3d) -> Vec<Hit>,
) -> Vec<(Vec3, Option<Entity>, Vec3)> {
    let from = eye.xyz();
    points
        .iter()
        .filter_map(|p| {
            let dir = Dir3::new(*p - from).ok()?;
            let reach = from.distance(*p) - 0.1;
            cast(Ray3d::new(from, dir))
                .into_iter()
                .take_while(|h| h.distance < reach)
                .find(|h| !h.cut || foliage::fade(h.at, &[middle], axes, eye) < OPEN)
                .map(|h| (*p, h.what, h.at))
        })
        .collect()
}

/// Each frame of play: rays to his body; what hides him logged by name.
#[allow(clippy::too_many_arguments)]
fn check(
    mut ray: MeshRayCast,
    hero: Query<(Entity, &GlobalTransform), With<Hero>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    leaves: Query<&MeshMaterial3d<FoliageMaterial>>,
    shaded: Query<
        (),
        Or<(
            With<MeshMaterial3d<RimMaterial>>,
            With<MeshMaterial3d<StandardMaterial>>,
        )>,
    >,
    (mats, parents, names, fighters, meshes): (
        Res<Assets<FoliageMaterial>>,
        Query<&ChildOf>,
        Query<&Name>,
        Query<(), Or<(With<Hero>, With<crate::rim::Fighter>)>>,
        Query<&Mesh3d>,
    ),
    (frame, near, mut seen): (Res<bevy::diagnostic::FrameCount>, Res<Near>, ResMut<Hidden>),
) {
    let (Ok((_, h)), Ok(cam)) = (hero.single(), camera.single()) else {
        return;
    };
    let axes = [
        cam.right().as_vec3(),
        cam.up().as_vec3(),
        cam.forward().as_vec3(),
    ];
    let eye = cam.translation().extend(near.0);
    let feet = h.translation();
    // A body (his, a foe's) is no occluder; only the wood and its props are.
    let occluder = |e: Entity| {
        (leaves.contains(e) || shaded.contains(e))
            && !std::iter::once(e)
                .chain(parents.iter_ancestors(e))
                .any(|a| fighters.contains(a))
    };
    let cut = |e: Entity| {
        leaves
            .get(e)
            .ok()
            .and_then(|m| mats.get(&m.0))
            .is_some_and(|m| m.extension.foliage.leaf.y > 0.5)
    };
    let settings = MeshRayCastSettings::default()
        .with_visibility(RayCastVisibility::VisibleInView)
        .with_filter(&occluder)
        .never_early_exit();
    let points = hidden(
        eye,
        axes,
        feet + Vec3::Y * MIDDLE,
        &body(feet, axes[0]),
        |r| {
            ray.cast_ray(r, &settings)
                .iter()
                .map(|(e, hit)| Hit {
                    what: Some(*e),
                    at: hit.point,
                    distance: hit.distance,
                    cut: cut(*e),
                })
                .collect()
        },
    );
    let Some((point, what, at)) = points.first() else {
        return;
    };
    seen.frames += 1;
    // The surface by its own or an ancestor's name, else its mesh's file and its material's
    // kind (a scattered card or rock has no name).
    let name = what
        .and_then(|e| {
            std::iter::once(e)
                .chain(parents.iter_ancestors(e))
                .find_map(|a| names.get(a).ok())
                .map(|n| n.to_string())
                .or_else(|| {
                    let mesh = meshes.get(e).ok().and_then(|m| m.0.path());
                    let kind = if leaves.contains(e) {
                        "foliage"
                    } else {
                        "rim or standard"
                    };
                    Some(format!(
                        "{kind} mesh {}",
                        mesh.map_or("(made in code)".into(), |p| p.to_string())
                    ))
                })
        })
        .unwrap_or_else(|| "?".into());
    info!(
        "hero hidden at frame {} by {name} ({} points; the first {:.1} m up, met at {:.1},{:.1},{:.1} {:.1} m before it; {} frames so far)",
        frame.0,
        points.len(),
        point.y - feet.y,
        at.x,
        at.y,
        at.z,
        at.distance(*point),
        seen.frames
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ruin::camera::offset;

    /// The play camera's eye and axes over a hero standing at `feet`.
    fn view(feet: Vec3, near: f32) -> (Vec4, [Vec3; 3]) {
        let eye = feet + Vec3::Y * 1.2 + offset();
        let t = Transform::from_translation(eye).looking_at(feet + Vec3::Y * 1.2, Vec3::Y);
        (
            eye.extend(near),
            [t.right().as_vec3(), t.up().as_vec3(), t.forward().as_vec3()],
        )
    }

    /// A ball of leaves or bark of radius `r` about `c`, as a ray meets it.
    fn ball(c: Vec3, r: f32, cut: bool) -> impl Fn(Ray3d) -> Vec<Hit> {
        move |ray: Ray3d| {
            let o = ray.origin - c;
            let b = o.dot(*ray.direction);
            let disc = b * b - (o.length_squared() - r * r);
            if disc < 0.0 {
                return vec![];
            }
            let t = -b - disc.sqrt();
            vec![Hit {
                what: None,
                at: ray.get_point(t),
                distance: t,
                cut,
            }]
        }
    }

    /// A bough anywhere on the line from the camera to his chest, from his shoulder to the
    /// lens, is cut away: none of his body is hidden. Its planted fault, a bough the cut does
    /// not reach (not cuttable), hides him; with the stills' hole (no play cut), a bough at
    /// his shoulder hides him too, as the developer saw.
    #[test]
    fn nothing_between_the_camera_and_the_hero_hides_him() {
        let feet = Vec3::new(3.0, 0.2, 5.0);
        let (eye, axes) = view(feet, NEAR);
        let middle = feet + Vec3::Y * MIDDLE;
        let points = body(feet, axes[0]);
        let chest = feet + Vec3::Y * 1.3;
        for k in 1..40 {
            // From 0.5 m before his chest up to the lens.
            let along = 0.5 + k as f32 * 0.45;
            let c = chest + (eye.xyz() - chest).normalize() * along;
            for r in [0.2, 0.6, 1.2] {
                if along < r + 0.3 {
                    continue;
                }
                let n = hidden(eye, axes, middle, &points, ball(c, r, true)).len();
                assert_eq!(
                    n, 0,
                    "a bough {along:.2} m before him (r {r}) hides {n} points"
                );
            }
        }
        let c = chest + (eye.xyz() - chest).normalize() * 1.5;
        assert!(!hidden(eye, axes, middle, &points, ball(c, 0.6, false)).is_empty());
        let (still, _) = view(feet, 0.0);
        // Its near side 0.65 m before his chest: open in play, two-thirds kept by the stills'.
        let c = chest + (eye.xyz() - chest).normalize() * 0.35;
        assert!(hidden(eye, axes, middle, &points, ball(c, 0.3, true)).is_empty());
        assert!(!hidden(still, axes, middle, &points, ball(c, 0.3, true)).is_empty());
    }

    /// Near the lens every cuttable surface fades, wherever it is on the screen: a bough
    /// 5 m from the camera at the frame's top is gone; one 15 m off and away from him stays.
    #[test]
    fn boughs_near_the_lens_fade_and_far_ones_stay() {
        let feet = Vec3::ZERO;
        let (eye, axes) = view(feet, NEAR);
        let middle = feet + Vec3::Y * MIDDLE;
        let top = eye.xyz() + axes[2] * 5.0 + axes[1] * 1.0;
        assert!(foliage::fade(top, &[middle], axes, eye) > 0.99);
        let far = feet + Vec3::new(6.0, 4.0, -4.0);
        assert!(eye.xyz().distance(far) > NEAR);
        assert!(foliage::fade(far, &[middle], axes, eye) < 0.01);
    }
}
