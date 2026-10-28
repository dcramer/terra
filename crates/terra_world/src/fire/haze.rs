//! Heat haze over the fires (docs/fire.md, "Heat haze"): the hot air rising over a blaze bends
//! the light through it, so what stands behind shimmers. As Unreal's and Unity HDRP's
//! distortion do it, a pass over the HDR picture before the tonemapper samples the scene
//! colour again a little off, by scrolling noise, only inside the column of hot air over each
//! fire (`assets/shaders/fire_haze.wesl`); the flames are in that picture, so they are bent
//! with the rest and never greyed or drawn over, as the haze card of fire round 8 was.
//!
//! Bevy's `FullscreenMaterial` is the pass (Core3d's post-processing, before the tonemapping,
//! D184); this module gives it, each frame, where the columns stand on the screen.

use super::Blazes;
use bevy::{
    core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin},
    prelude::*,
    render::{RenderApp, extract_component::ExtractComponent, render_resource::ShaderType},
    shader::ShaderRef,
};

/// How many fires shimmer at once, the nearest.
pub const COLUMNS: usize = 4;

/// The hot air fades out from so far to so far off, in metres: past it a shimmer is a pixel.
const FADE: (f32, f32) = (25.0, 40.0);

/// Where the hot air starts and ends over a fire's foot, in its flames' heights (a blaze's
/// `tall` is its cards', the flames seen about half of it): from low in the flames, so their
/// edges waver too, to twice their height, where it has mixed into the air.
const COLUMN: (f32, f32) = (0.15, 2.0);

/// How far the haze bends the light at its strongest, as a share of the column's half width:
/// a few centimetres at a campfire, a few pixels close up, as a filmed campfire's is. At 0.05
/// a blind judge saw no difference; at 0.15 it reads as heat shimmer, 5/10 against 4.5 without
/// it (fire round 23); at 0.25 it read as marbled water.
const BEND: f32 = 0.15;

/// The hot air's two octaves of noise (`fire_haze.wesl`): each its cells a half width and its
/// share of the bend. The haze maps each pixel to where it is seen from; where that map's slope
/// reaches a pixel a pixel it folds, and the picture tears along a straight line, value noise
/// being steepest along the middles of its cells. At 3 and 7 cells a half width it tore the
/// flames' tips and the air over them in straight seams, a box of the picture shifted against
/// the rest (campfire round m3); at 1.5 and 2.5 it never folds
/// (`the_haze_never_folds_the_picture`).
const OCTAVES: [(f32, f32); 2] = [(1.5, 0.7), (2.5, 0.3)];

/// Where the bend fades out across a column, from and to, in its half widths from the middle.
const SIDE: (f32, f32) = (0.35, 1.0);

/// The haze's columns on the screen, as the pass's uniform (the shader's `HeatHaze`).
#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Default, Debug)]
#[extract_app(RenderApp)]
pub struct HeatHaze {
    /// Per column, in the picture's 0 to 1 (y down): the foot's x and y, the top's y, and the
    /// half width at the foot as a share of the picture's height.
    columns: [Vec4; COLUMNS],
    /// Per column: how far it bends, as a share of its half width, and its seed.
    looks: [Vec4; COLUMNS],
    time: f32,
    count: u32,
    aspect: f32,
    pad: f32,
    /// The octaves' cells a half width and shares (`OCTAVES`), and where the side fades (`SIDE`).
    octaves: Vec4,
    side: Vec4,
}

impl FullscreenMaterial for HeatHaze {
    fn fragment_shader() -> ShaderRef {
        "shaders/fire_haze.wesl".into()
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(FullscreenMaterialPlugin::<HeatHaze>::default())
        .add_systems(
            PostUpdate,
            columns.after(bevy::transform::TransformSystems::Propagate),
        );
}

/// What the camera sees by: the world to its clip space, where it stands, its right and the
/// picture's width over its height.
#[derive(Clone, Copy, Debug)]
pub(super) struct Lens {
    pub clip_from_world: Mat4,
    pub eye: Vec3,
    pub right: Vec3,
    pub aspect: f32,
}

impl Lens {
    fn of(camera: &Camera, eye: &GlobalTransform) -> Self {
        Lens {
            clip_from_world: camera.clip_from_view() * eye.affine().inverse(),
            eye: eye.translation(),
            right: *eye.right(),
            aspect: camera
                .logical_viewport_size()
                .map_or(16.0 / 9.0, |v| v.x / v.y.max(1.0)),
        }
    }

    /// A point in the picture's 0 to 1 (y down), or none behind the camera.
    fn uv(&self, p: Vec3) -> Option<Vec2> {
        let clip = self.clip_from_world * p.extend(1.0);
        (clip.w > 1e-4).then(|| {
            let ndc = clip.truncate() / clip.w;
            Vec2::new(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5)
        })
    }
}

/// One fire's column of hot air on the screen, or none when it is behind the camera, off the
/// screen or too far to see: its foot, its top and its half width as [`HeatHaze`] keeps
/// them, and how strongly it bends (its heat, faded with distance).
pub(super) fn column(lens: &Lens, blaze: &super::Blaze) -> Option<(Vec4, f32)> {
    let far = lens.eye.distance(blaze.at);
    let fade = 1.0 - terra_gen::smoothstep(FADE.0, FADE.1, far);
    if fade <= 0.0 || blaze.heat <= 0.0 {
        return None;
    }
    let foot = blaze.at + Vec3::Y * blaze.tall * COLUMN.0;
    let top = blaze.at + Vec3::Y * blaze.tall * COLUMN.1;
    // As wide as the flames, a tall flame's as wide as half its height.
    let half = 0.5 * blaze.wide.max(0.5 * blaze.tall);
    let (f, t, s) = (
        lens.uv(foot)?,
        lens.uv(top)?,
        lens.uv(foot + lens.right * half)?,
    );
    let w = ((s - f) * Vec2::new(lens.aspect, 1.0)).length();
    // Off the screen: the column's box clear of the picture.
    let reach = w * 1.6 / lens.aspect;
    if f.x + reach < 0.0 || f.x - reach > 1.0 || t.y > 1.0 || f.y < 0.0 || w <= 0.0 {
        return None;
    }
    Some((
        Vec4::new(f.x, f.y, t.y, w),
        BEND * blaze.heat.min(1.0) * fade,
    ))
}

/// Each 3D camera's haze, from the nearest fires: given it while a fire shimmers in view,
/// taken away when none does.
fn columns(
    mut commands: Commands,
    time: Res<Time>,
    blazes: Res<Blazes>,
    mut cameras: Query<(Entity, &Camera, &GlobalTransform, Option<&mut HeatHaze>), With<Camera3d>>,
) {
    for (entity, camera, eye, haze) in &mut cameras {
        let lens = Lens::of(camera, eye);
        let mut near: Vec<(f32, &super::Blaze)> = blazes
            .all()
            .map(|b| (lens.eye.distance_squared(b.at), b))
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut next = HeatHaze {
            // The haze over the flames moves with them (`fire::PACE`).
            time: super::flame_time(time.elapsed_secs_wrapped()),
            aspect: lens.aspect,
            octaves: Vec4::new(OCTAVES[0].0, OCTAVES[0].1, OCTAVES[1].0, OCTAVES[1].1),
            side: Vec4::new(SIDE.0, SIDE.1, 0.0, 0.0),
            ..default()
        };
        for (_, blaze) in near {
            if next.count as usize == COLUMNS {
                break;
            }
            if let Some((place, bend)) = column(&lens, blaze) {
                let i = next.count as usize;
                next.columns[i] = place;
                next.looks[i] = Vec4::new(bend, blaze.seed, 0.0, 0.0);
                next.count += 1;
            }
        }
        // No pass at all where no fire shimmers.
        match haze {
            _ if next.count == 0 => {
                commands.entity(entity).remove::<HeatHaze>();
            }
            Some(mut haze) => *haze = next,
            None => {
                commands.entity(entity).insert(next);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fire::Blaze;

    /// A camera at `eye`, looking level along -z, a 16:9 picture.
    fn lens(eye: Vec3) -> Lens {
        let view = Transform::from_translation(eye).looking_to(Vec3::NEG_Z, Vec3::Y);
        let aspect = 16.0 / 9.0;
        // Bevy's own perspective (`PerspectiveProjection`) builds its matrix this way.
        #[allow(deprecated)]
        let projection = Mat4::perspective_infinite_reverse_rh(0.8, aspect, 0.1);
        Lens {
            clip_from_world: projection * view.to_matrix().inverse(),
            eye,
            right: *view.right(),
            aspect,
        }
    }

    fn fire(at: Vec3) -> Blaze {
        Blaze {
            at,
            tall: 1.0,
            heat: 1.0,
            seed: 0.0,
            wide: 0.8,
        }
    }

    #[test]
    fn a_fire_ahead_has_its_column_over_it_in_the_middle() {
        let l = lens(Vec3::new(0.0, 1.7, 0.0));
        let (place, bend) = column(&l, &fire(Vec3::new(0.0, 0.0, -4.0))).unwrap();
        assert!((place.x - 0.5).abs() < 1e-3, "centred: {place}");
        assert!(place.z < place.y, "the top above the foot: {place}");
        assert!(place.w > 0.0 && bend > 0.0);
    }

    #[test]
    fn a_nearer_fire_has_a_wider_column() {
        let l = lens(Vec3::new(0.0, 1.7, 0.0));
        let near = column(&l, &fire(Vec3::new(0.0, 0.0, -3.0))).unwrap().0;
        let far = column(&l, &fire(Vec3::new(0.0, 0.0, -12.0))).unwrap().0;
        assert!(near.w > 3.0 * far.w, "{near} {far}");
    }

    /// Across a column, the haze's map moves a pixel by its noise times the side's fade, and
    /// by the fade's own slope times the noise: together under one pixel a pixel, so it never
    /// folds the picture into a seam. Value noise of lattice values -1 to 1 blended by a
    /// smoothstep is at its steepest 1.5 x 2 a cell; the octaves' shares make the noise at
    /// most 1. Along the rise the bend is 0.6 of this, its ramp longer than the side's.
    #[test]
    fn the_haze_never_folds_the_picture() {
        const NOISE_SLOPE: f32 = 3.0;
        let shares: f32 = OCTAVES.iter().map(|o| o.1).sum();
        assert!((shares - 1.0).abs() < 1e-4, "{shares}");
        let cells: f32 = OCTAVES.iter().map(|(k, s)| k * s).sum();
        let fade = |a: f32| 1.0 - terra_gen::smoothstep(SIDE.0, SIDE.1, a);
        let mut worst = 0.0f32;
        for i in 0..=1400 {
            let a = i as f32 / 1000.0;
            let slope = (fade(a + 1e-3) - fade(a)).abs() / 1e-3;
            worst = worst.max(BEND * (NOISE_SLOPE * cells * fade(a) + slope));
        }
        assert!(worst < 1.0, "the haze folds: {worst} a pixel a pixel");
    }

    #[test]
    fn no_column_behind_off_screen_or_far() {
        let l = lens(Vec3::new(0.0, 1.7, 0.0));
        assert!(
            column(&l, &fire(Vec3::new(0.0, 0.0, 4.0))).is_none(),
            "behind"
        );
        assert!(
            column(&l, &fire(Vec3::new(30.0, 0.0, -4.0))).is_none(),
            "off to the side"
        );
        assert!(
            column(&l, &fire(Vec3::new(0.0, 0.0, -45.0))).is_none(),
            "past the fade"
        );
    }
}
