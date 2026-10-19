//! A point or spot light's fall-off on a surface softened by its size. Bevy's point light
//! (`bevy_pbr/src/render/pbr_lighting.wesl`, MIT OR Apache-2.0, by the Bevy contributors;
//! Bevy 0.20) falls off as `1 / max(d², 0.0001)` on a surface however wide its `radius`: the
//! radius shapes only the specular highlight. A fire's light is its flames, not a point: by
//! inverse-square alone the stones a campfire's light hung 0.9 m from took twelve times the
//! light of the ground 3 m out and drew past the tonemapper's white, brighter than the flame
//! itself, where every photograph's flame is the brightest thing and what it lights at a
//! metre is some four stops under it (research/2026-10-19-fire-step-back.md, plan step 1).
//!
//! A disc light's illuminance on its axis is `L π r² / (r² + d²)` (Lagarde and de Rousiers,
//! "Moving Frostbite to PBR", SIGGRAPH 2014, the disk light's illuminance): inverse-square
//! far off, levelling off within its radius. So the light's radius is added to the distance,
//! squared, as the fog's in-scatter already does (`scatter`); a light a few centimetres
//! across is unchanged.

use bevy::prelude::*;

/// Where Bevy keeps its lighting functions.
const LIBRARY: &str = "embedded://bevy_pbr/render/pbr_lighting.wesl";

/// Bevy's fall-off of a point light on a surface.
const POINT: &str = "let rangeAttenuation = getDistanceAttenuation(distance_square, (*light).color_inverse_square_range.w);";

/// The same softened by the light's radius, as a disc light's is.
const DISC: &str = "let rangeAttenuation = getDistanceAttenuation(distance_square + (*light).position_radius.w * (*light).position_radius.w, (*light).color_inverse_square_range.w);";

pub fn plugin(app: &mut App) {
    super::shader_patch::patch_library(
        app,
        LIBRARY,
        patched,
        "Bevy's point-light fall-off was not found: a fire's light falls off from a point, and lights what is by it brighter than its flames",
    );
}

fn patched(source: &str) -> Option<String> {
    super::shader_patch::swap_once(source, POINT, DISC)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_light_falls_off_from_its_radius() {
        let shader = format!("fn f() {{\n    {POINT}\n}}\n");
        let soft = patched(&shader).expect("patched");
        assert!(soft.contains(DISC) && !soft.contains(POINT));
        assert!(
            patched("fn f() {}").is_none(),
            "nothing to patch, nothing changed"
        );
    }
}
