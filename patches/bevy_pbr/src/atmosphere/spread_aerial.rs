//! Terra's patch (D615): the aerial-view LUT drawn a workgroup a column.
//!
//! Bevy 0.20's `aerial_view_lut.wesl` `main` runs a thread a column of the table (32x32
//! threads, four workgroups of 256), each marching every slice's samples in a row; on a card of
//! a hundred multiprocessors it is latency-bound on each sample's five table reads. Its
//! `main_spread` gives each column a workgroup whose threads work out the samples' extinction,
//! transmittance, in-scattering and analytic segment into workgroup memory together, then one
//! accumulates them in `main`'s order and arithmetic. Hillaire's reference (sebh/UnrealEngineSkyAtmosphere,
//! `RenderCameraVolumePS`) and Unreal's SkyAtmosphere spread the table further, a thread a
//! froxel each marching from the camera to its own slice, but in other samples, so the table
//! would change. [`SPREAD_AERIAL_VIEW_LUT`] off draws it Bevy's way.

use core::sync::atomic::AtomicBool;

use bevy_math::UVec3;

/// Whether the aerial-view LUT is drawn a workgroup a column (true, Terra's default) or a
/// thread a column (false, Bevy 0.20's way). Terra's `render.spread_aerial_view_lut` tunable
/// sets it.
pub static SPREAD_AERIAL_VIEW_LUT: AtomicBool = AtomicBool::new(true);

/// The workgroups `main_spread` is dispatched in for a table of `size`: one a column.
pub(crate) fn spread_workgroups(size: UVec3) -> UVec3 {
    UVec3::new(size.x, size.y, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHADER: &str = include_str!("aerial_view_lut.wesl");

    /// The value of the shader's `const <name>: u32 = <n>u;`.
    fn shader_const(name: &str) -> u32 {
        let line = SHADER
            .lines()
            .find(|l| l.starts_with(&format!("const {name}: u32 = ")))
            .unwrap_or_else(|| panic!("aerial_view_lut.wesl declares no {name}"));
        line.rsplit("= ")
            .next()
            .unwrap()
            .trim_end_matches(';')
            .trim_end_matches('u')
            .parse()
            .unwrap()
    }

    /// Every column of the table, and no more, gets a workgroup of its own, whatever its size
    /// (Terra's low and ultra settings halve and double the default 32x32x32).
    #[test]
    fn a_workgroup_a_column() {
        for size in [UVec3::splat(32), UVec3::splat(64), UVec3::new(16, 8, 4)] {
            let groups = spread_workgroups(size);
            assert_eq!(groups.x * groups.y * groups.z, size.x * size.y, "{size}");
        }
    }

    /// `main_spread`'s arrays of SPREAD_SAMPLES vec3s (16 bytes each in workgroup memory) fit
    /// wgpu's default limit of 16 KiB a workgroup, so its pipeline compiles on every device, and
    /// its workgroup is SPREAD_THREADS threads, which its loop over the samples strides by.
    #[test]
    fn the_spread_fits_a_workgroup() {
        let arrays = SHADER
            .lines()
            .filter(|l| l.starts_with("var<workgroup>"))
            .inspect(|l| assert!(l.ends_with("array<vec3<f32>, SPREAD_SAMPLES>;"), "{l}"))
            .count() as u32;
        let bytes = arrays * shader_const("SPREAD_SAMPLES") * 16;
        assert!(bytes <= 16_384, "{bytes} bytes of workgroup memory");
        let threads = shader_const("SPREAD_THREADS");
        assert!(
            SHADER.contains(&format!("@workgroup_size({threads}, 1, 1)\nfn main_spread")),
            "main_spread's workgroup is not SPREAD_THREADS ({threads}) wide"
        );
    }
}
