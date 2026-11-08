//! Nothing hides the hero (iso-forest-demo): in play every cuttable crown and bough opens in
//! a dithered hole about him from a hand's breadth before him, and fades near the lens, so
//! no bough sweeps over the camera as he runs under the oaks. Diablo IV and Path of Exile 2
//! do the same: occluders between the camera and the player dither out in a circle about him
//! (Diablo IV's "fade cylinder"), and canopy near the camera is not drawn.
//!
//! The check: each frame rays go from the camera to his body from knee to crown (`body`);
//! a surface met before him that the cut does not drop (`foliage::fade`, the shader's twin)
//! hides him, and is logged as `hero hidden at frame N by <name>`, counted in `Hidden`.

use super::silhouette::{GAP, Silhouette};
use crate::camera::MainCamera;
use crate::foliage::{self, FoliageMaterial, Near, Seen};
use crate::play::footprint::Footprint;
use crate::play::{Fallen, Hero};
use crate::rim::RimMaterial;
use bevy::camera::primitives::Aabb;
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
        .add_systems(
            PostUpdate,
            (check, gate).chain().after(TransformSystems::Propagate),
        );
}

/// The run's gate on its last frame (D632): nothing hid him, a body before him included,
/// since his silhouette is drawn through it; its exit an error on a fail. Proven in game
/// (iso-forest-demo j13): the route with the silhouette plugin left out fails it, hidden 125
/// frames (218-242, 644-707, 1003-1030 and more), the 125 the route with it was drawn through.
fn gate(
    args: Res<crate::Args>,
    frame: Res<bevy::diagnostic::FrameCount>,
    seen: Res<Hidden>,
    mut exit: MessageWriter<AppExit>,
) {
    if !args.last_frame().is_some_and(|l| frame.0 + 1 == l) {
        return;
    }
    match verdict(&seen) {
        Ok(line) => info!("✓ {line}"),
        Err(e) => {
            error!("✗ {e}");
            exit.write(AppExit::from_code(1));
        }
    }
}

/// The bar a run's frames meet: no frame hid him.
pub fn verdict(seen: &Hidden) -> Result<String, String> {
    if seen.frames == 0 {
        Ok(format!(
            "the hero: nothing hid him ({} frames drawn through a body)",
            seen.through
        ))
    } else {
        Err(format!(
            "the hero: hidden {} frames (each logged 'hero hidden at frame N by ...'): cut the \
             surface named there in play (foliage.rs's hole) or draw his silhouette through it \
             (silhouette.rs); do not drop it from the check",
            seen.frames
        ))
    }
}

/// The frames something hid him in, and those a body stood before him and his silhouette
/// was drawn through it.
#[derive(Resource, Default)]
pub struct Hidden {
    pub frames: u32,
    pub through: u32,
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
    /// A body (a foe's), which his silhouette is drawn through (silhouette.rs).
    pub body: bool,
}

/// How deep a point of `body` lies inside his outline (m): his axis is about this far behind
/// the surface the camera sees of his chest, so a body must stand `GAP` and this before a
/// point for his silhouette to show there.
const SKIN: f32 = 0.15;

/// A point of his body a surface stands before: the point, the surface (if known), where it
/// was met, and whether his silhouette is drawn through it (a body, not the wood).
pub type Before = (Vec3, Option<Entity>, Vec3, bool);

/// The points of `points` a surface stands before, each with the first that does: `cast`
/// gives what a ray from the eye meets, nearest first; a surface more than a hand's breadth
/// before a point stands before it unless it is cuttable and the cut about `middle` (with
/// `eye`'s near fade) leaves it at least `OPEN` open. A body more than `GAP` and `SKIN` before
/// it is drawn through when `through` (his silhouette is drawn), and so does not hide him.
pub fn hidden(
    eye: Vec4,
    axes: [Vec3; 3],
    middle: Vec3,
    points: &[Vec3],
    through: bool,
    mut cast: impl FnMut(Ray3d) -> Vec<Hit>,
) -> Vec<Before> {
    let from = eye.xyz();
    points
        .iter()
        .filter_map(|p| {
            let dir = Dir3::new(*p - from).ok()?;
            let reach = from.distance(*p) - 0.1;
            cast(Ray3d::new(from, dir))
                .into_iter()
                .take_while(|h| h.distance < reach)
                .find(|h| h.body || !h.cut || foliage::fade(h.at, &[middle], axes, eye) < OPEN)
                .map(|h| {
                    let drawn = h.body && through && reach + 0.1 - h.distance > GAP + SKIN;
                    (*p, h.what, h.at, drawn)
                })
        })
        .collect()
}

/// How far along `ray` it first meets the upright capsule from `a` to `b` of radius `r`, from
/// outside. Ported from Inigo Quilez's capsule intersector (iquilezles.org/articles/
/// intersectors, MIT). The foes' bodies are met as capsules on their footprints, not as
/// meshes: a ray cast on a skinned mesh meets its bind pose, never the body as it stands
/// posed (iso-forest-demo j12: no body counted at the parry, where a treant stood before him).
pub fn capsule(ray: Ray3d, a: Vec3, b: Vec3, r: f32) -> Option<f32> {
    let (ro, rd) = (ray.origin, *ray.direction);
    let (ba, oa) = (b - a, ro - a);
    let (baba, bard, baoa) = (ba.dot(ba), ba.dot(rd), ba.dot(oa));
    let (rdoa, oaoa) = (rd.dot(oa), oa.dot(oa));
    let k2 = baba - bard * bard;
    let k1 = baba * rdoa - baoa * bard;
    let k0 = baba * oaoa - baoa * baoa - r * r * baba;
    let h = k1 * k1 - k2 * k0;
    if h < 0.0 {
        return None;
    }
    // Where the ray meets the infinite cylinder, along the axis (a ray along the axis meets
    // the cap on the origin's side).
    let y = if k2 > 1e-6 {
        let t = (-k1 - h.sqrt()) / k2;
        let y = baoa + t * bard;
        if y > 0.0 && y < baba {
            return (t > 0.0).then_some(t);
        }
        y
    } else {
        baoa
    };
    // A cap: the sphere at the end the side met lies past.
    let oc = if y <= 0.0 { oa } else { ro - b };
    let (b1, c1) = (rd.dot(oc), oc.dot(oc) - r * r);
    let h1 = b1 * b1 - c1;
    let t = -b1 - h1.max(0.0).sqrt();
    (h1 > 0.0 && t > 0.0).then_some(t)
}

/// A foe's body as the check meets it: the capsule over its footprint's circles, from the
/// ground to the top of its bounds (its bind pose's height, near its standing one).
pub type Upright = (Entity, Vec3, Vec3, f32);

/// The standing foes' capsules: each footprint circle stood up from the body's feet to the
/// top of its meshes' bounds less a radius, so the cap's top is the bounds' top.
#[allow(clippy::type_complexity)]
fn uprights(
    standing: &Query<
        (Entity, &Footprint, &GlobalTransform),
        (With<crate::rim::Fighter>, Without<Hero>, Without<Fallen>),
    >,
    kids: &Query<&Children>,
    bounds: &Query<(&Aabb, &GlobalTransform), With<Mesh3d>>,
) -> Vec<Upright> {
    let mut out = Vec::new();
    for (e, foot, g) in standing {
        let feet = g.translation();
        let top = kids
            .iter_descendants(e)
            .filter_map(|k| bounds.get(k).ok())
            .map(|(b, kg)| {
                let (c, h) = (Vec3::from(b.center), Vec3::from(b.half_extents));
                (0..8)
                    .map(|i| {
                        let s = Vec3::new(
                            if i & 1 == 0 { -1.0 } else { 1.0 },
                            if i & 2 == 0 { -1.0 } else { 1.0 },
                            if i & 4 == 0 { -1.0 } else { 1.0 },
                        );
                        kg.transform_point(c + h * s).y
                    })
                    .fold(f32::MIN, f32::max)
            })
            .fold(f32::MIN, f32::max);
        if top <= feet.y {
            continue;
        }
        let s = g.scale().x;
        for &(p, r) in &foot.0 {
            let r = r * s;
            let at = g.transform_point(Vec3::new(p.x, 0.0, p.y));
            let low = at.with_y(feet.y + r);
            let high = at.with_y((top - r).max(low.y));
            out.push((e, low, high, r));
        }
    }
    out
}

/// What a ray meets of the standing foes' capsules, as the check's hits.
pub fn met(ray: Ray3d, uprights: &[Upright]) -> Vec<Hit> {
    uprights
        .iter()
        .filter_map(|&(e, a, b, r)| {
            capsule(ray, a, b, r).map(|t| Hit {
                what: Some(e),
                at: ray.get_point(t),
                distance: t,
                cut: false,
                body: true,
            })
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
        Query<(), With<crate::rim::Fighter>>,
        Query<&Mesh3d>,
    ),
    (heroes, silhouettes): (Query<(), With<Hero>>, Query<(), With<Silhouette>>),
    (frame, near, mut seen): (Res<bevy::diagnostic::FrameCount>, Res<Near>, ResMut<Hidden>),
    (standing, kids, bounds): (
        Query<
            (Entity, &Footprint, &GlobalTransform),
            (With<crate::rim::Fighter>, Without<Hero>, Without<Fallen>),
        >,
        Query<&Children>,
        Query<(&Aabb, &GlobalTransform), With<Mesh3d>>,
    ),
) {
    let (Ok((_, h)), Ok(cam)) = (hero.single(), camera.single()) else {
        return;
    };
    // His silhouette is drawn once his meshes have their copies (silhouette.rs).
    let through = !silhouettes.is_empty();
    let axes = [
        cam.right().as_vec3(),
        cam.up().as_vec3(),
        cam.forward().as_vec3(),
    ];
    let eye = cam.translation().extend(near.0);
    let feet = h.translation();
    // His own body is no occluder; the wood, its props and the foes' bodies are.
    let up = |e: Entity| std::iter::once(e).chain(parents.iter_ancestors(e));
    let body = |e: Entity| up(e).any(|a| fighters.contains(a));
    // A foe's meshes are left to its capsule (`capsule`).
    let occluder = |e: Entity| {
        (leaves.contains(e) || shaded.contains(e)) && !up(e).any(|a| heroes.contains(a)) && !body(e)
    };
    let uprights = uprights(&standing, &kids, &bounds);
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
        &self::body(feet, axes[0]),
        through,
        |r| {
            let mut hits: Vec<Hit> = ray
                .cast_ray(r, &settings)
                .iter()
                .map(|(e, hit)| Hit {
                    what: Some(*e),
                    at: hit.point,
                    distance: hit.distance,
                    cut: cut(*e),
                    body: false,
                })
                .collect();
            hits.extend(met(r, &uprights));
            hits.sort_by(|a, b| a.distance.total_cmp(&b.distance));
            hits
        },
    );
    let drawn = points.iter().filter(|p| p.3).count();
    if drawn > 0 {
        seen.through += 1;
        info!(
            "hero drawn through a body at frame {} ({drawn} points; {} frames so far)",
            frame.0, seen.through
        );
    }
    let Some((point, what, at, _)) = points.iter().find(|p| !p.3) else {
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
                .or_else(|| fighters.contains(e).then(|| format!("a foe's body {e}")))
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
        let wood = sphere(c, r, false);
        move |ray: Ray3d| {
            let mut h = wood(ray);
            h.iter_mut().for_each(|h| h.cut = cut);
            h
        }
    }

    /// A body (a foe's trunk) of radius `r` about `c`.
    fn sphere(c: Vec3, r: f32, body: bool) -> impl Fn(Ray3d) -> Vec<Hit> {
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
                cut: false,
                body,
            }]
        }
    }

    /// A foe's body between the camera and him (a treant at the parry, frames 1026 to 1036 of
    /// the route) no longer hides him: his silhouette is drawn through it. Its planted fault,
    /// no silhouette (`through` false), hides him; and a body pressed within `GAP` of his
    /// outline hides the point it stands before even with it, as the depth test does.
    #[test]
    fn a_body_before_him_is_drawn_through() {
        let feet = Vec3::new(3.0, 0.2, 5.0);
        let (eye, axes) = view(feet, NEAR);
        let middle = feet + Vec3::Y * MIDDLE;
        let points = body(feet, axes[0]);
        let chest = feet + Vec3::Y * 1.3;
        let toward = (eye.xyz() - chest).normalize();
        // A treant's trunk, 0.8 m thick, its near side 2.2 m before his chest.
        let treant = sphere(chest + toward * 3.0, 0.8, true);
        let before = hidden(eye, axes, middle, &points, true, &treant);
        assert!(!before.is_empty(), "the treant stands before him");
        assert!(
            before.iter().all(|p| p.3),
            "drawn through: {}",
            before.len()
        );
        let fault = hidden(eye, axes, middle, &points, false, &treant);
        assert!(!fault.is_empty() && fault.iter().all(|p| !p.3));
        // Its near side 0.3 m before his chest's axis: within GAP and SKIN, not drawn.
        let pressed = sphere(chest + toward * 0.6, 0.3, true);
        let close = hidden(eye, axes, middle, &points, true, &pressed);
        assert!(close.iter().any(|p| !p.3), "pressed to him, it hides him");
    }

    /// A ray meets an upright capsule where its side or cap stands, from outside, and not
    /// beside it or behind its origin.
    #[test]
    fn a_ray_meets_a_capsule_on_its_side_and_caps() {
        let (a, b, r) = (Vec3::ZERO, Vec3::Y * 2.0, 0.5);
        let ray = |o: Vec3, d: Vec3| Ray3d::new(o, Dir3::new(d).unwrap());
        let side = capsule(ray(Vec3::new(-5.0, 1.0, 0.0), Vec3::X), a, b, r).unwrap();
        assert!((side - 4.5).abs() < 1e-4, "side at {side}");
        let top = capsule(ray(Vec3::new(0.0, 6.0, 0.0), -Vec3::Y), a, b, r).unwrap();
        assert!((top - 3.5).abs() < 1e-4, "top cap at {top}");
        let low = ray(Vec3::new(-5.0, -0.3, 0.0), Vec3::X);
        let t = capsule(low, a, b, r).unwrap();
        assert!((low.get_point(t).length() - r).abs() < 1e-4, "foot cap");
        assert!(capsule(ray(Vec3::new(-5.0, 1.0, 0.6), Vec3::X), a, b, r).is_none());
        assert!(capsule(ray(Vec3::new(-5.0, 2.6, 0.0), Vec3::X), a, b, r).is_none());
        assert!(capsule(ray(Vec3::new(5.0, 1.0, 0.0), Vec3::X), a, b, r).is_none());
    }

    /// A treant standing before him at the camera, as its footprint's capsule, is a body he
    /// is drawn through (`met`), as the posed mesh the old cast missed never was. Its planted
    /// fault, no silhouette, hides him; a treant beside the line does neither.
    #[test]
    fn a_standing_foe_before_him_is_met_as_its_capsule() {
        let feet = Vec3::new(3.0, 0.2, 5.0);
        let (eye, axes) = view(feet, NEAR);
        let middle = feet + Vec3::Y * MIDDLE;
        let points = body(feet, axes[0]);
        let chest = feet + Vec3::Y * 1.3;
        // A tall treant 0.6 m thick whose axis the line from the eye to his chest crosses
        // 2 m before him.
        let on = chest + (eye.xyz() - chest).normalize() * 2.0;
        let treant = |g: Vec3| {
            let g = g.with_y(feet.y);
            vec![(
                Entity::PLACEHOLDER,
                g + Vec3::Y * 0.6,
                g + Vec3::Y * 6.0,
                0.6,
            )]
        };
        let standing = treant(on);
        let before = hidden(eye, axes, middle, &points, true, |r| met(r, &standing));
        assert!(!before.is_empty(), "the treant stands before him");
        assert!(before.iter().all(|p| p.3), "drawn through");
        let fault = hidden(eye, axes, middle, &points, false, |r| met(r, &standing));
        assert!(
            !fault.is_empty() && fault.iter().all(|p| !p.3),
            "no silhouette hides him"
        );
        let aside = treant(on + axes[0] * 3.0);
        assert!(hidden(eye, axes, middle, &points, false, |r| met(r, &aside)).is_empty());
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
                let n = hidden(eye, axes, middle, &points, true, ball(c, r, true)).len();
                assert_eq!(
                    n, 0,
                    "a bough {along:.2} m before him (r {r}) hides {n} points"
                );
            }
        }
        let c = chest + (eye.xyz() - chest).normalize() * 1.5;
        assert!(!hidden(eye, axes, middle, &points, true, ball(c, 0.6, false)).is_empty());
        let (still, _) = view(feet, 0.0);
        // Its near side 0.65 m before his chest: open in play, two-thirds kept by the stills'.
        let c = chest + (eye.xyz() - chest).normalize() * 0.35;
        assert!(hidden(eye, axes, middle, &points, true, ball(c, 0.3, true)).is_empty());
        assert!(!hidden(still, axes, middle, &points, true, ball(c, 0.3, true)).is_empty());
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
