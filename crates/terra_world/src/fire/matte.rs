//! The flames' ID matte (`--flame-matte`, D433), as an offline render writes an object's ID or
//! a matte pass (an AOV) beside its beauty pass: what the fire sheet measures a flame's colour
//! on, so the ground and stones it lights are not counted as flame (fire round 39: at dusk and
//! night most of the "flame" in a campfire's crop was its lit ground, which no colour rule
//! tells apart).
//!
//! The tonemapper already knows each pixel's flame share, the alpha past 1 a flame's mask
//! draw writes (`film.rs`, `AFTER_FLAMES`), smoke over it having taken its share back. Bevy's
//! tonemapping pass returns the HDR alpha itself, which an 8-bit target clips at 1 and its
//! screenshot drops; with the matte asked for, the pass returns the share instead, and each
//! shot is saved with its alpha as a grey picture beside it. Only with the option: the scaled
//! picture's presenter (`upscale.rs`) draws the world as a blended sprite, so an alpha of
//! nought would leave it out (at full size nothing reads it).

use bevy::prelude::*;
use bevy::render::view::screenshot::{ScreenshotCaptured, save_to_disk};
use std::path::PathBuf;

/// Bevy's tonemapping pass, whose alpha becomes the matte.
const FRAG: &str = "embedded://bevy_core_pipeline/tonemapping/tonemapping_frag.wesl";

/// Its end, returning the HDR alpha.
const RETURN: &str = "    return vec4<f32>(output_rgb, hdr_color.a);";

/// Its end returning the flame's share instead, as `AFTER_FLAMES` takes it.
const MATTE: &str = "    // The flames' share (crates/terra_world/src/fire/matte.rs).
    return vec4<f32>(output_rgb, clamp(hdr_color.a - 1.0, 0.0, 1.0));";

pub(super) fn plugin(app: &mut App) {
    if !app
        .world()
        .get_resource::<crate::shell::Options>()
        .is_some_and(|options| options.flame_matte)
    {
        return;
    }
    crate::shader_patch::patch_library(
        app,
        FRAG,
        patched,
        "Bevy's tonemapping pass's end was not found: --flame-matte's mattes are empty",
    );
}

/// The tonemapping pass returning the flames' share, if it has its end once.
fn patched(source: &str) -> Option<String> {
    crate::shader_patch::swap_once(&source.replace("\r\n", "\n"), RETURN, MATTE)
}

/// Saves a screenshot at `picture`, as `save_to_disk` does, and its alpha, the flames' share,
/// at `matte` before it, white where a pixel is all flame: a run waiting on its pictures finds
/// the matte there too.
pub fn with_matte(picture: PathBuf, matte: PathBuf) -> impl FnMut(On<ScreenshotCaptured>) {
    let mut save = save_to_disk(picture);
    move |captured: On<ScreenshotCaptured>| {
        match captured.image.clone().try_into_dynamic() {
            Ok(image) => {
                let mut grey = image.to_rgba8();
                for pixel in grey.pixels_mut() {
                    let share = pixel.0[3];
                    pixel.0 = [share, share, share, 255];
                }
                if let Err(e) = grey.save(&matte) {
                    error!("the flame matte {} was not saved: {e}", matte.display());
                }
            }
            Err(e) => error!("the flame matte {} was not made: {e}", matte.display()),
        }
        save(captured);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pass returns the flames' share, `AFTER_FLAMES`'s `flame`, and nothing else changes.
    #[test]
    fn the_tonemapping_pass_returns_the_flames_share() {
        let source = "fn fragment() {\n    let a = 1;\r\n    return vec4<f32>(output_rgb, hdr_color.a);\n}\n";
        let matted = patched(source).expect("its end is found");
        assert!(matted.contains("clamp(hdr_color.a - 1.0, 0.0, 1.0)"));
        assert!(!matted.contains(RETURN));
        assert!(matted.starts_with("fn fragment() {\n    let a = 1;\n"));
        assert!(super::super::film::AFTER_FLAMES.contains("clamp(in.a - 1.0, 0.0, 1.0)"));
        assert_eq!(patched(&matted), None, "patched once");
    }
}
