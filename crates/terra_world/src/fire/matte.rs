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
//!
//! The matte is the flame's share alone (D472): the motion gate cuts its flame where the matte
//! says flame, so what is drawn over a flame must not move it, or a change to the sparks or the
//! smoke fails "flame motion" with the flame untouched (fire round g66: sharper sparks inside
//! the tips were tracked as short-lived tongues, life 0.10 -> 0.05, and the smoke over the tips
//! took their share back). With the matte asked for, smoke leaves the alpha as it found it, and
//! a spark takes the share from under it by how much it shows, its light added whole as before.

use bevy::prelude::*;
use bevy::render::render_resource::{BlendComponent, BlendFactor, RenderPipelineDescriptor};
use bevy::render::view::screenshot::{ScreenshotCaptured, save_to_disk};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether this run writes the matte: set once as the app is built, before any fire pipeline is
/// specialized, where no option can be read.
static ON: AtomicBool = AtomicBool::new(false);

/// Whether the run writes the flame matte (`--flame-matte`).
pub(super) fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// A spark's pipeline with the matte asked for: its light added whole, as premultiplied with
/// no cover it was, and its alpha (`FLAME_MATTE` in fire.wesl, how much of it shows) taking the
/// flame's share from under it, so a spark's pixel is not counted flame.
pub(super) fn keep_sparks_out(descriptor: &mut RenderPipelineDescriptor) {
    let Some(fragment) = descriptor.fragment.as_mut() else {
        return;
    };
    fragment.shader_defs.push("FLAME_MATTE".into());
    for blend in fragment
        .targets
        .iter_mut()
        .flatten()
        .filter_map(|t| t.blend.as_mut())
    {
        blend.color = BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::One,
            ..default()
        };
    }
}

/// The smoke's pipeline with the matte asked for: blended over the picture as before, the
/// alpha, and with it the flame's share under the smoke, kept.
pub(super) fn keep_smoke_out(descriptor: &mut RenderPipelineDescriptor) {
    let Some(fragment) = descriptor.fragment.as_mut() else {
        return;
    };
    for blend in fragment
        .targets
        .iter_mut()
        .flatten()
        .filter_map(|t| t.blend.as_mut())
    {
        blend.alpha = BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::One,
            ..default()
        };
    }
}

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
    ON.store(true, Ordering::Relaxed);
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

    use bevy::render::render_resource::{
        BlendOperation, BlendState, ColorTargetState, ColorWrites, FragmentState, TextureFormat,
    };

    /// What a blend leaves in a channel: `src` drawn over `dst`, `a` the source's alpha.
    fn blended(c: BlendComponent, src: f32, dst: f32, a: f32) -> f32 {
        assert_eq!(c.operation, BlendOperation::Add);
        let f = |factor| match factor {
            BlendFactor::Zero => 0.0,
            BlendFactor::One => 1.0,
            BlendFactor::SrcAlpha => a,
            BlendFactor::OneMinusSrcAlpha => 1.0 - a,
            other => panic!("{other:?} is not modelled"),
        };
        src * f(c.src_factor) + dst * f(c.dst_factor)
    }

    fn drawn(blend: BlendState) -> RenderPipelineDescriptor {
        let target = ColorTargetState {
            format: TextureFormat::Rgba16Float,
            blend: Some(blend),
            write_mask: ColorWrites::ALL,
        };
        RenderPipelineDescriptor {
            fragment: Some(FragmentState {
                targets: vec![Some(target)],
                ..default()
            }),
            ..default()
        }
    }

    fn blend_of(d: &RenderPipelineDescriptor) -> BlendState {
        d.fragment.as_ref().unwrap().targets[0]
            .as_ref()
            .unwrap()
            .blend
            .unwrap()
    }

    /// A pixel a flame's mask gave a share of 0.6 keeps it under a smoke covering 0.7, which
    /// took most of it back before; a spark that shows whole leaves none of it, one that does
    /// not show leaves it all, and the picture's colour is what it was without the matte.
    #[test]
    fn sparks_and_smoke_drawn_over_a_flame_leave_the_matte_its_share() {
        let (flame, colour) = (1.6, 0.8);
        let share = |alpha: f32| (alpha - 1.0).clamp(0.0, 1.0);

        let mut spark = drawn(BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        keep_sparks_out(&mut spark);
        let b = blend_of(&spark);
        let premultiplied = BlendState::PREMULTIPLIED_ALPHA_BLENDING.color;
        assert_eq!(
            blended(b.color, 2.0, colour, 1.0),
            blended(premultiplied, 2.0, colour, 0.0)
        );
        assert_eq!(
            share(blended(b.alpha, 1.0, flame, 1.0)),
            0.0,
            "a spark's pixel not flame"
        );
        assert!((share(blended(b.alpha, 0.0, flame, 0.0)) - 0.6).abs() < 1e-6);
        let defs = &spark.fragment.as_ref().unwrap().shader_defs;
        let flame_matte = bevy::shader::ShaderDefVal::Bool("FLAME_MATTE".into(), true);
        assert!(defs.contains(&flame_matte));

        let was = BlendState::ALPHA_BLENDING;
        assert!(
            share(blended(was.alpha, 0.7, flame, 0.7)) < 0.3,
            "the smoke took it back"
        );
        let mut smoke = drawn(was);
        keep_smoke_out(&mut smoke);
        let b = blend_of(&smoke);
        assert_eq!(
            b.color, was.color,
            "the smoke drawn over the picture as before"
        );
        assert!((share(blended(b.alpha, 0.7, flame, 0.7)) - 0.6).abs() < 1e-6);

        let shader = include_str!("../../../../assets/shaders/fire.wesl");
        let sparks = &shader[shader.find("} else if kind == 1 {").unwrap()..];
        let sparks = &sparks[..sparks.find("} else if kind == 4 {").unwrap()];
        assert!(
            sparks.contains("@if(FLAME_MATTE)"),
            "a spark's alpha says how much it shows"
        );
    }
}
