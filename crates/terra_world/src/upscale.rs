//! The world drawn at a share of the screen's size and scaled up to it (D239), as consoles
//! and every PC game's lower presets do: an integrated GPU spends most of its frame
//! shading pixels, so three fifths of the width and height is about a third of the work. The main camera
//! draws into a picture that size, with its temporal anti-aliasing, sharpened (AMD's
//! contrast-adaptive sharpening, as FidelityFX's upscaler finishes with; at full size too,
//! after TAA, `sharpen`), and a camera of
//! its own shows it stretched over the screen, with the interface drawn over it at the
//! screen's own size. At full size the main camera draws to the screen as it always has.
//!
//! The picture's scale factor is the screen's times the share, so the camera's view in
//! logical pixels is the screen's and pointing with the mouse is unchanged.
//!
//! A window larger than the preset is made for is drawn at no more pixels than a budget
//! (`PIXEL_BUDGET`, D616), as shipped games cap their render resolution below a large output
//! (Unreal's screen percentage, consoles' fixed internal resolution): shading pixels is most
//! of the frame at the lake, so a window of twice 1080p's pixels costs about twice its frame.

use crate::planet::MainCamera;
use crate::quality::Quality;
use crate::shell::Picture;
use bevy::{
    anti_alias::contrast_adaptive_sharpening::ContrastAdaptiveSharpening,
    camera::{ImageRenderTarget, RenderTarget, visibility::RenderLayers},
    prelude::*,
    render::render_resource::{TextureFormat, TextureUsages},
    window::{PrimaryWindow, WindowRef},
};
use terra_gen::tune::Tunable;

/// The render layer the upscaled picture and the camera showing it are on.
const SHOWN_LAYER: usize = 31;

/// The most pixels the world is drawn at, in millions, before it is scaled up to the screen
/// (D616); 0 draws it at the preset's share of the screen whatever the screen's size.
pub static PIXEL_BUDGET: Tunable = Tunable::new(
    "render.pixel_budget",
    0.0,
    0.0..=40.0,
    "the most pixels the world is drawn at, in millions, scaled up to a larger screen; 0 no limit",
);

/// The share of the screen's width and height the world is drawn at: the preset's, or less
/// where the screen has more pixels than the budget (in millions; 0 none) allows.
pub fn share(preset: f32, screen: UVec2, budget_mpx: f32) -> f32 {
    let pixels = screen.x as f32 * screen.y as f32;
    if budget_mpx <= 0.0 || pixels <= 0.0 {
        return preset;
    }
    let within = libm::sqrtf(budget_mpx * 1.0e6 / pixels);
    // In hundredths, so a window dragged a pixel wider makes no new picture.
    preset.min(libm::floorf(within * 100.0) / 100.0)
}

pub fn plugin(app: &mut App) {
    terra_gen::tune::register(&[&PIXEL_BUDGET]);
    app.add_systems(PostUpdate, (follow, sharpen)).add_systems(
        PostUpdate,
        stand_with_the_camera.before(bevy::transform::TransformSystems::Propagate),
    );
}

/// The world's picture sharpened after its temporal anti-aliasing, as the preset says
/// (`Quality::sharpening`, D362): TAA's blend of jittered frames is a filter about a pixel
/// wide, which took more than half the finest detail out of a log's face a metre off
/// (research/texture-detail-2026-10-15.md), so shipped games finish with a sharpening pass
/// (FidelityFX's RCAS after its upscaler, Unreal's tonemapper sharpen); AMD's
/// contrast-adaptive sharpening, which leaves edges already sharp alone.
fn sharpen(
    quality: Res<Quality>,
    mut commands: Commands,
    camera: Query<
        (Entity, Option<&ContrastAdaptiveSharpening>),
        (With<MainCamera>, Without<Presenter>),
    >,
) {
    let strength = quality.sharpening();
    for (entity, sharpening) in &camera {
        if sharpening.is_some_and(|s| s.enabled && s.sharpening_strength == strength) {
            continue;
        }
        commands.entity(entity).insert(ContrastAdaptiveSharpening {
            enabled: true,
            sharpening_strength: strength,
            ..default()
        });
    }
}

/// The camera showing the picture stands where the main one does: Bevy measures what is
/// drawn at a distance (`VisibilityRange`) from every camera, the shadows' casters too,
/// and one left at the planet's middle would count everything near as far.
fn stand_with_the_camera(
    camera: Query<&Transform, (With<MainCamera>, Without<Presenter>)>,
    mut presenters: Query<&mut Transform, With<Presenter>>,
) {
    let Some(eye) = camera.iter().next().map(|t| t.translation) else {
        return;
    };
    for mut presenter in &mut presenters {
        if presenter.translation != eye {
            presenter.translation = eye;
        }
    }
}

/// The camera that shows the world's picture on the screen, and the picture shown.
#[derive(Component)]
struct Presenter;

#[derive(Component)]
struct Shown;

/// What the world is drawn into while it is scaled, and where it is shown.
struct Scaled {
    size: UVec2,
    shown_on: RenderTarget,
}

fn follow(
    quality: Res<Quality>,
    mut scaled: Local<Option<Scaled>>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut camera: Query<(&mut RenderTarget, &mut Projection), (With<MainCamera>, Without<Presenter>)>,
    window: Query<&Window, With<PrimaryWindow>>,
    picture: Option<Res<Picture>>,
    presenters: Query<Entity, With<Presenter>>,
    mut shown: Query<&mut Sprite, With<Shown>>,
) {
    let Some((mut target, mut projection)) = camera.iter_mut().next() else {
        return;
    };
    // Where the world is shown: where the main camera drew before it was scaled.
    let shown_on = scaled
        .as_ref()
        .map_or_else(|| target.clone(), |scaled| scaled.shown_on.clone());
    // Offscreen, the screen is the picture, once the camera has been pointed at it.
    if picture.is_some() && matches!(shown_on, RenderTarget::Window(_)) {
        return;
    }
    // The screen's size in pixels, and how many pixels a logical one is.
    let (size, factor) = match &shown_on {
        RenderTarget::Window(WindowRef::Primary) => match window.single() {
            Ok(window) => (window.physical_size(), window.scale_factor()),
            Err(_) => return,
        },
        RenderTarget::Image(image) => match images.get(&image.handle) {
            Some(found) => (found.size(), image.scale_factor),
            None => return,
        },
        _ => return,
    };
    let share = share(quality.render_scale(), size, PIXEL_BUDGET.get());
    if share >= 1.0 || size.x == 0 || size.y == 0 {
        // Full size: drawn straight to the screen.
        if let Some(scaled) = scaled.take() {
            *target = scaled.shown_on;
            // Bevy 0.20's `camera_system` measures a camera's target again only when the
            // camera is added, its projection changes or the target's window or image says it
            // changed, never when the `RenderTarget` itself is swapped; back on a picture
            // that already existed it kept the scaled size and drew into a corner of it.
            projection.set_changed();
            for presenter in &presenters {
                commands.entity(presenter).despawn();
            }
            info!(
                "Graphics: the world drawn at the screen's {}x{}.",
                size.x, size.y
            );
        }
        return;
    }
    // A picture drawn offscreen is the screen; one drawn for anything else is left alone.
    if let RenderTarget::Image(image) = &shown_on
        && picture
            .as_ref()
            .is_none_or(|picture| picture.0 != image.handle)
    {
        return;
    }
    let wanted = (size.as_vec2() * share).round().as_uvec2().max(UVec2::ONE);
    let logical = size.as_vec2() / factor;
    if let Some(scaled) = scaled.as_ref()
        && scaled.size == wanted
    {
        for mut sprite in &mut shown {
            if sprite.custom_size != Some(logical) {
                sprite.custom_size = Some(logical);
            }
        }
        return;
    }
    let mut image = Image::new_target_texture(
        wanted.x,
        wanted.y,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    // Drawn into, so nothing to upload or to wait under the upload cap for (D420; the
    // offscreen picture's comment, `shell::picture`).
    image.data = None;
    let image = images.add(image);
    *target = RenderTarget::Image(ImageRenderTarget {
        handle: image.clone(),
        scale_factor: factor * share,
    });
    for presenter in &presenters {
        commands.entity(presenter).despawn();
    }
    // Stretching softens, so the picture is sharpened first, at its own size, as
    // FidelityFX's upscaler finishes by sharpening (`sharpen`).
    commands.spawn((
        Name::new("Upscaled view"),
        Presenter,
        Camera2d,
        Camera {
            order: 1,
            ..default()
        },
        // One picture stretched over the screen has no edges to smooth.
        Msaa::Off,
        // On a layer of its own, so no light treats it as a view of the world to cast
        // shadows for, and nothing of the world is drawn into it.
        RenderLayers::layer(SHOWN_LAYER),
        shown_on.clone(),
        children![(
            Shown,
            RenderLayers::layer(SHOWN_LAYER),
            Sprite {
                image,
                custom_size: Some(logical),
                ..default()
            },
        )],
    ));
    info!(
        "Graphics: the world drawn at {}x{} and scaled up to {}x{}.",
        wanted.x, wanted.y, size.x, size.y
    );
    *scaled = Some(Scaled {
        size: wanted,
        shown_on,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A screen over the budget is drawn at about the budget's pixels, never more; one within
    /// it, or with no budget, at the preset's share.
    #[test]
    fn a_screen_over_the_budget_is_drawn_at_no_more_pixels_than_it() {
        let wide = UVec2::new(3816, 1550);
        let s = share(1.0, wide, 2.0736);
        let drawn = (wide.as_vec2() * s).round();
        let pixels = drawn.x * drawn.y;
        assert!(
            pixels <= 2.0736e6,
            "{pixels} pixels drawn over a 2.07 M budget"
        );
        assert!(
            pixels > 0.95 * 2.0736e6,
            "{pixels} pixels drawn, far under the budget"
        );
        assert_eq!(share(1.0, UVec2::new(1920, 1080), 2.0736), 1.0);
        assert_eq!(share(1.0, wide, 0.0), 1.0);
        assert_eq!(share(0.6, wide, 0.0), 0.6);
        // The low preset's share is a ceiling the budget only lowers.
        assert_eq!(share(0.6, UVec2::new(1920, 1080), 2.0736), 0.6);
        assert!(share(0.6, UVec2::new(7680, 4320), 2.0736) < 0.6);
    }
}
