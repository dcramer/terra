//! The ruin's view, an action RPG's: perspective with a narrow lens, high above and behind the
//! hero, looking down 55 degrees, so a 1.85 m hero stands about an eighth of the frame's height
//! (Diablo IV and Path of Exile 2 measure 0.07 to 0.20, research/ iso-art j1). Fixed: a still
//! is judged from the game's own pose.

use crate::Args;
use crate::camera::{MainCamera, Offscreen};
use bevy::anti_alias::contrast_adaptive_sharpening::ContrastAdaptiveSharpening;
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::camera::{Exposure, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::ShadowFilteringMethod;
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// The vertical field of view, how far the camera looks down, how far it is turned from due
/// south, and how far it stands from the hero's chest.
pub const FOV: f32 = 30.0;
pub const PITCH: f32 = 55.0;
pub const YAW: f32 = 20.0;
/// 19 m (was 23.5: the knight, a 1.85 m model that stands lower in his clips, measured
/// 0.07 to 0.08 of the frame in judge r3's still against the references' 0.11 to 0.13).
pub const DISTANCE: f32 = 19.0;
/// What the camera looks at: the middle of the fight, between the knight and the ogre.
pub const FOCUS: Vec3 = Vec3::new(0.7, 1.2, -0.6);

pub fn offset() -> Vec3 {
    let (p, y) = (PITCH.to_radians(), YAW.to_radians());
    Vec3::new(p.cos() * y.sin(), p.sin(), p.cos() * y.cos()) * DISTANCE
}

/// The ruin's camera, its exposure and tone curve the biome's light preset's.
pub fn spawn(
    mut commands: Commands,
    args: Res<Args>,
    light: Res<crate::kit::light::Light>,
    mut images: ResMut<Assets<Image>>,
) {
    let cam = arpg(&mut commands, &args, &mut images, FOCUS, light.ev100);
    light.camera(&mut commands, cam, DISTANCE);
}

/// The ARPG's camera on `focus`, at exposure `ev100`, as the ruin's; the forest's too.
pub fn arpg(
    commands: &mut Commands,
    args: &Args,
    images: &mut Assets<Image>,
    focus: Vec3,
    ev100: f32,
) -> Entity {
    let mut camera = commands.spawn((
        Camera3d::default(),
        Projection::from(PerspectiveProjection {
            fov: FOV.to_radians(),
            near: 1.0,
            far: 120.0,
            ..default()
        }),
        Transform::from_translation(focus + offset()).looking_at(focus, Vec3::Y),
        bevy::camera::Hdr,
        Msaa::Off,
        TemporalAntiAliasing::default(),
        // TAA softens fine stone; a light sharpening gives it back (the spike's first gap).
        ContrastAdaptiveSharpening {
            enabled: true,
            sharpening_strength: 0.6,
            ..default()
        },
        ScreenSpaceAmbientOcclusion::default(),
        ShadowFilteringMethod::Temporal,
        Bloom::NATURAL,
        Tonemapping::TonyMcMapface,
        Exposure { ev100 },
        MainCamera,
    ));
    let id = camera.id();
    if args.shot.is_some() {
        let (w, h) = args.size;
        let image = Image::new_target_texture(
            w,
            h,
            TextureFormat::Rgba8Unorm,
            Some(TextureFormat::Rgba8UnormSrgb),
        );
        let image = images.add(image);
        camera.insert(RenderTarget::Image(image.clone().into()));
        commands.insert_resource(Offscreen(image));
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hero stands an eighth of the frame's height, inside the ARPGs' measured 0.07 to
    /// 0.20: the hero's height seen across the line of sight over the frame's height at them.
    #[test]
    fn hero_is_an_eighth_of_the_frame() {
        let seen = 1.85 * (90.0 - PITCH).to_radians().cos();
        let frame = 2.0 * DISTANCE * (FOV / 2.0).to_radians().tan();
        let share = seen / frame;
        assert!(
            (0.10..0.15).contains(&share),
            "the hero is {share} of the frame"
        );
    }
}
