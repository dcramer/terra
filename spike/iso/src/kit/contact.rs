//! Contact shadows and grime under the models that stand on a floor (iso-kit j40): a soft dark
//! blob where each meets the ground (its occlusion, of the sky and every light alike) and a
//! ragged stain a little wider, as the top-down ARPGs ground their clutter with a blob decal
//! under each prop. The blob is sized from the model's own bounds once its scene is up, so a
//! new prop needs no number (D136). An unlit black quad blended over the floor multiplies it
//! by one less its alpha; fogged, it still darkens only the floor's own share.

use crate::ruin::effects::kit::{Noise, image, smooth};
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::light::NotShadowCaster;
use bevy::math::Affine3A;
use bevy::prelude::*;

/// A model standing on the floor at height `floor` (m): once its scene's meshes have their
/// bounds, a blob is laid under it and the marker taken off. One whose foot is more than
/// `STANDS` over the floor (a wall lamp, a sign) gets none.
#[derive(Component)]
pub struct Grounded {
    pub floor: f32,
}

/// A posed body standing on the floor at height `floor` (m): its blob lies under its feet
/// (the joints named `foot_*` and `ball_*`, Tripo's and Unreal's), not its bounds, which
/// are its bind pose's (iso-kit j42), and follows them as its pose moves them.
#[derive(Component)]
pub struct Feet {
    pub floor: f32,
}

/// A posed body's blob and the footprint it was made for (m).
#[derive(Component)]
struct Under {
    blob: Entity,
    size: Vec2,
}

/// A foot this far over the floor (m) is in the air: its stride's swing casts no contact.
const LIFTED: f32 = 0.22;
/// A sole's half-width about its joints (m).
const SOLE: f32 = 0.08;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (lay, under_feet));
}

/// The footprint of a body's feet in its own turn: the joints near the floor (all of them if
/// every one is lifted), widened by a sole, as (middle, half-size) in metres; none with no
/// feet.
pub fn footprint(joints: &[Vec3], floor: f32) -> Option<(Vec2, Vec2)> {
    let down: Vec<Vec3> = joints
        .iter()
        .copied()
        .filter(|p| p.y < floor + LIFTED)
        .collect();
    let feet = if down.is_empty() { joints } else { &down };
    let (lo, hi) = feet.iter().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), p| {
        (lo.min(p.xz()), hi.max(p.xz()))
    });
    (!feet.is_empty()).then(|| ((lo + hi) / 2.0, (hi - lo) / 2.0 + Vec2::splat(SOLE)))
}

#[allow(clippy::too_many_arguments)]
fn under_feet(
    mut commands: Commands,
    bodies: Query<(Entity, &Feet, &GlobalTransform, Option<&Under>)>,
    children: Query<&Children>,
    joints: Query<(&Name, &GlobalTransform)>,
    mut placed: Query<&mut Transform>,
    mut images: ResMut<Assets<Image>>,
    mut quads: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (body, feet, at, under) in &bodies {
        let (_, rot, pos) = at.to_scale_rotation_translation();
        let f = rot * Vec3::Z;
        let turn = Quat::from_rotation_y(f.x.atan2(f.z));
        let frame = Affine3A::from_rotation_translation(turn, pos).inverse();
        let points: Vec<Vec3> = children
            .iter_descendants(body)
            .filter_map(|e| joints.get(e).ok())
            .filter(|(n, _)| n.as_str().starts_with("foot_") || n.as_str().starts_with("ball_"))
            .map(|(_, t)| {
                let p = frame.transform_point3(t.translation());
                Vec3::new(p.x, t.translation().y, p.z)
            })
            .collect();
        let Some((mid, half)) = footprint(&points, feet.floor) else {
            continue;
        };
        let size = (half + Vec2::splat(MARGIN)) * 2.0;
        let centre = pos + turn * Vec3::new(mid.x, 0.0, mid.y);
        let place = Transform::from_translation(Vec3::new(centre.x, feet.floor + LIFT, centre.z))
            .with_rotation(turn * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
        match under {
            Some(u) => {
                if let Ok(mut t) = placed.get_mut(u.blob) {
                    *t = place.with_scale((size / u.size).extend(1.0));
                }
            }
            None => {
                let seed = (pos.x * 131.0 + pos.z * 17.0).to_bits() % 9973;
                let material = materials.add(StandardMaterial {
                    base_color: Color::BLACK,
                    base_color_texture: Some(images.add(blob(half, seed))),
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    ..default()
                });
                let blob = commands
                    .spawn((
                        Mesh3d(quads.add(Rectangle::new(size.x, size.y))),
                        MeshMaterial3d(material),
                        place,
                        NotShadowCaster,
                        RenderLayers::layer(super::water::LAYER),
                    ))
                    .id();
                commands.entity(body).insert(Under { blob, size });
            }
        }
    }
}

/// Texels a metre of a blob's image, and its largest side.
const TEXELS: f32 = 48.0;
const LARGEST: u32 = 256;
/// How far past the model's footprint the blob reaches (m).
const MARGIN: f32 = 0.35;
/// How far over the floor it lies (m): over the puddles' faces (4 mm).
const LIFT: f32 = 0.007;
/// The highest a model's foot may be over the floor and still stand on it (m).
const STANDS: f32 = 0.3;

/// The blob's alpha at `d` metres from the footprint's edge (negative inside), `n` the
/// stain's noise there (0 to 1): the occlusion darkest under the model and gone about 0.2 m
/// out, the stain ragged, reaching 0.05 to 0.33 m, both nothing at the quad's edge.
pub fn shade(d: f32, n: f32) -> f32 {
    let ao = 0.8 * (1.0 - smooth(-0.08, 0.22, d));
    let reach = 0.05 + 0.28 * n;
    let stain = 0.45 * (1.0 - smooth(reach * 0.3, reach, d));
    let a = 1.0 - (1.0 - ao) * (1.0 - stain);
    a * (1.0 - smooth(MARGIN - 0.04, MARGIN, d))
}

/// The signed distance (m) from a rounded rectangle of half-size `half` at the origin.
fn edge(p: Vec2, half: Vec2) -> f32 {
    let r = (half.min_element() * 0.5).min(0.2);
    let q = p.abs() - half + Vec2::splat(r);
    q.max(Vec2::ZERO).length() + q.max_element().min(0.0) - r
}

/// The blob's image for a footprint of half-size `half` (m), its stain's noise by `seed`.
fn blob(half: Vec2, seed: u32) -> Image {
    let size = (half + Vec2::splat(MARGIN)) * 2.0;
    let (w, h) = (
        ((size.x * TEXELS).ceil() as u32).clamp(16, LARGEST),
        ((size.y * TEXELS).ceil() as u32).clamp(16, LARGEST),
    );
    let (big, fine) = (Noise::new(16, seed), Noise::new(32, seed * 7 + 3));
    image(w, h, |x, y| {
        let uv = (Vec2::new(x as f32, y as f32) + 0.5) / Vec2::new(w as f32, h as f32);
        let p = (uv - 0.5) * size;
        let n = 0.6 * big.at(p * 3.0) + 0.4 * fine.at(p * 9.0);
        [0.0, 0.0, 0.0, shade(edge(p, half), n)]
    })
}

fn lay(
    mut commands: Commands,
    roots: Query<(Entity, &Grounded, &GlobalTransform)>,
    children: Query<&Children>,
    parts: Query<(Option<&Aabb>, &GlobalTransform), With<Mesh3d>>,
    mut images: ResMut<Assets<Image>>,
    mut quads: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (root, g, at) in &roots {
        let (_, rot, pos) = at.to_scale_rotation_translation();
        let f = rot * Vec3::Z;
        let turn = Quat::from_rotation_y(f.x.atan2(f.z));
        let frame = Affine3A::from_rotation_translation(turn, pos).inverse();
        let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
        let (mut any, mut ready) = (false, true);
        for e in children.iter_descendants(root) {
            let Ok((aabb, t)) = parts.get(e) else {
                continue;
            };
            let Some(aabb) = aabb else {
                ready = false;
                break;
            };
            any = true;
            let (c, h) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
            for k in 0..8 {
                let s = Vec3::new(
                    if k & 1 == 0 { -1.0 } else { 1.0 },
                    if k & 2 == 0 { -1.0 } else { 1.0 },
                    if k & 4 == 0 { -1.0 } else { 1.0 },
                );
                let p = frame.transform_point3(t.affine().transform_point3(c + h * s));
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        if !(any && ready) {
            continue;
        }
        commands.entity(root).remove::<Grounded>();
        if pos.y + lo.y > g.floor + STANDS {
            continue;
        }
        let half = ((hi - lo) / 2.0).xz().max(Vec2::splat(0.05));
        let mid = (hi + lo) / 2.0;
        let size = (half + Vec2::splat(MARGIN)) * 2.0;
        let seed = (pos.x * 131.0 + pos.z * 17.0).to_bits() % 9973;
        let material = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            base_color_texture: Some(images.add(blob(half, seed))),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        let centre = pos + turn * Vec3::new(mid.x, 0.0, mid.z);
        commands.spawn((
            Mesh3d(quads.add(Rectangle::new(size.x, size.y))),
            MeshMaterial3d(material),
            Transform::from_translation(Vec3::new(centre.x, g.floor + LIFT, centre.z))
                .with_rotation(turn * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            NotShadowCaster,
            // The main view's alone: the water's mirror camera does not see layer 9.
            RenderLayers::layer(super::water::LAYER),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The blob is darkest under the model, lighter out from its edge, and nothing at the
    /// quad's edge whatever the stain's noise.
    #[test]
    fn a_blob_is_dark_under_and_gone_at_its_edge() {
        for n in [0.0, 0.5, 1.0] {
            let (under, at, out) = (shade(-0.3, n), shade(0.0, n), shade(0.25, n));
            assert!(under > 0.75, "under {under}");
            assert!(under >= at && at > out, "{under} {at} {out} at noise {n}");
            assert!(shade(MARGIN, n) < 0.001, "edge {}", shade(MARGIN, n));
        }
        // The stain reaches further where its noise is high.
        assert!(shade(0.28, 1.0) > shade(0.28, 0.0) + 0.03);
    }

    /// A walker's footprint spans its planted feet, not a foot swinging in the air, and is
    /// a sole wider; with every foot lifted it spans them all.
    #[test]
    fn a_footprint_is_the_planted_feet() {
        let planted = [Vec3::new(-0.1, 0.1, 0.3), Vec3::new(-0.1, 0.05, 0.45)];
        let swing = Vec3::new(0.12, 0.35, -0.3);
        let (mid, half) = footprint(&[planted[0], planted[1], swing], 0.0).unwrap();
        assert!((mid - Vec2::new(-0.1, 0.375)).length() < 1e-5, "{mid}");
        assert!((half - Vec2::new(SOLE, 0.075 + SOLE)).length() < 1e-5, "{half}");
        let (mid, _) = footprint(&[swing], 0.0).unwrap();
        assert!((mid - swing.xz()).length() < 1e-5);
        assert!(footprint(&[], 0.0).is_none());
    }

    #[test]
    fn the_edge_is_signed_distance() {
        let half = Vec2::new(1.0, 0.5);
        assert!(edge(Vec2::ZERO, half) < -0.4);
        assert!(edge(Vec2::new(1.0, 0.0), half).abs() < 1e-5);
        assert!((edge(Vec2::new(1.3, 0.0), half) - 0.3).abs() < 1e-5);
    }
}
