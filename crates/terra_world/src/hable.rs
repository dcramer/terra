//! A Hable (Uncharted 2) filmic curve for the camera, which Bevy has not got (docs/fire.md, "The
//! tonemapper"; D415): a candidate of the tonemapper bake-off, not the camera's curve until
//! the developers choose it. Bevy's own Reinhard, which nothing of ours uses, is swapped for it
//! in Bevy's tonemapping library (`shader_patch`), so a camera given `HABLE` is drawn with it.
//!
//! Ported from Godot's `tonemap_filmic` (servers/rendering/renderer_rd/shaders/effects/
//! tonemap.glsl, and its white in servers/rendering/storage/environment_storage.cpp), under
//! its MIT licence:
//!
//! Copyright (c) 2014-present Godot Engine contributors (see AUTHORS.md).
//! Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur.
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy of this
//! software and associated documentation files (the "Software"), to deal in the Software
//! without restriction, including without limitation the rights to use, copy, modify, merge,
//! publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons
//! to whom the Software is furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all copies or
//! substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
//! INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR
//! PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE
//! FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
//! OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//! DEALINGS IN THE SOFTWARE.

use bevy::{core_pipeline::tonemapping::Tonemapping, prelude::*};

/// The camera's tonemapping that draws with the Hable curve.
pub const HABLE: Tonemapping = Tonemapping::Reinhard;

/// Where Bevy keeps the library with its tonemappers.
const LIBRARY: &str = "embedded://bevy_core_pipeline/tonemapping.wesl";

/// Bevy's Reinhard, swapped out.
const REINHARD: &str = "fn tonemapping_reinhard(color: vec3<f32>) -> vec3<f32> {
    return color / (1.0 + color);
}";

/// Godot's constants: Hable's curve with its input doubled (`exposure_bias`), baked in.
const BIAS: f32 = 2.0;
const A: f32 = 0.22 * BIAS * BIAS;
const B: f32 = 0.30 * BIAS;
const C: f32 = 0.10;
const D: f32 = 0.20;
const E: f32 = 0.01;
const F: f32 = 0.30;
/// The linear white the curve reaches 1 at, in Godot's units (before its doubling): Hable's
/// own 11.2 halved. Godot's default of 1 clips everything over the scene's white.
pub const WHITE: f32 = 5.6;

/// The curve before it is scaled to its white.
fn curve(x: f32) -> f32 {
    ((x * (A * x + C * B) + D * E) / (x * (A * x + B) + D * F)) - E / F
}

/// The curve as drawn, on one channel: 0 at 0, 1 at `WHITE`.
pub fn hable(x: f32) -> f32 {
    curve(x.max(0.0)) / curve(WHITE)
}

pub fn plugin(app: &mut App) {
    super::shader_patch::patch_library(
        app,
        LIBRARY,
        patched,
        "Bevy's Reinhard was not found: a camera given the Hable curve draws with Reinhard's",
    );
}

/// The library with Reinhard's body Hable's, if it has Reinhard's.
fn patched(source: &str) -> Option<String> {
    let hable = format!(
        "fn tonemapping_reinhard(color: vec3<f32>) -> vec3<f32> {{
    // Hable's filmic curve, ported from Godot (crates/terra_world/src/hable.rs, MIT).
    let c = max(color, vec3(0.0));
    let mapped = ((c * ({A:?} * c + {CB:?}) + {DE:?}) / (c * ({A:?} * c + {B:?}) + {DF:?})) - {EF:?};
    return mapped / {white:?};
}}",
        CB = C * B,
        DE = D * E,
        DF = D * F,
        EF = E / F,
        white = curve(WHITE),
    );
    super::shader_patch::swap_once(source, REINHARD, &hable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_is_a_film_shoulder() {
        // Black stays black and the white is 1, so nothing past it is lost but clipped.
        assert!(hable(0.0).abs() < 1.0e-6);
        assert!((hable(WHITE) - 1.0).abs() < 1.0e-6);
        // Rising everywhere, a toe and a shoulder: steeper through the middle than at either end.
        let slope = |x: f32| (hable(x + 0.01) - hable(x)) / 0.01;
        assert!((0..500).all(|i| slope(i as f32 * 0.01) > 0.0));
        assert!(slope(0.3) > slope(4.0));
        // Middle grey lands near middle grey (Hable's own: 0.18 shows about 0.2).
        assert!((0.15..0.25).contains(&hable(0.18)));
    }

    #[test]
    fn reinhard_is_swapped_once_for_the_same_curve() {
        let library = format!("fn a() {{}}\n{REINHARD}\nfn b() {{}}\n");
        let out = patched(&library).unwrap();
        assert!(!out.contains("color / (1.0 + color)"));
        assert!(out.contains("fn tonemapping_reinhard(color: vec3<f32>) -> vec3<f32>"));
        assert!(out.contains(&format!("{:?}", curve(WHITE))));
        assert!(patched("fn a() {}").is_none());
        assert!(patched(&format!("{library}{library}")).is_none());
    }
}
