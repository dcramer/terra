//! Terra's patch (D614): the transmittance and multiscattering LUTs are drawn again only when
//! what they are made of changes. Bevy 0.20 redraws all four of the atmosphere's LUTs every
//! frame, but these two read only the planet's radii and ground albedo, the medium's density
//! LUT and the LUT sizes and sample counts, never the view or the sun (Hillaire, "A Scalable
//! and Production Ready Sky and Atmosphere Rendering Technique", EGSR 2020, section 5: the
//! transmittance and multiple-scattering LUTs are "only updated when the atmosphere
//! properties change"; Unreal's SkyAtmosphere caches them the same way). A view whose inputs
//! and LUT textures are those its LUTs were last drawn with keeps them; the sky-view and
//! aerial-view LUTs, which follow the camera and the sun, are drawn every frame as before.
//! [`KEEP_UNCHANGED_LUTS`] off draws them every frame, as Bevy did.

use core::sync::atomic::{AtomicBool, Ordering};

use bevy_ecs::{
    component::Component,
    entity::{Entity, EntityHashMap, EntityHashSet},
    system::{Commands, Local, Query, Res},
};
use bevy_render::{
    render_asset::RenderAssets,
    render_resource::{PipelineCache, TextureViewId},
};

use super::{
    ExtractedAtmosphere, GpuAtmosphereSettings,
    resources::{AtmosphereLutPipelines, AtmosphereTextures},
};
use crate::GpuScatteringMedium;

/// Whether a view's transmittance and multiscattering LUTs are kept while their inputs hold
/// (true, Terra's default) or drawn every frame (false, Bevy 0.20's way). Terra's
/// `render.kept_atmosphere_luts` tunable sets it.
pub static KEEP_UNCHANGED_LUTS: AtomicBool = AtomicBool::new(true);

/// On a view whose transmittance and multiscattering LUTs already hold what this frame's
/// would: the LUT pass draws only the sky-view and aerial-view LUTs.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct KeptAtmosphereLuts(pub bool);

/// Everything the two LUTs are made of, and the textures they were drawn into.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct LutInputs {
    words: [u32; 12],
    views: [TextureViewId; 4],
}

/// What the transmittance and multiscattering shaders read: the radii, the ground albedo,
/// the sizes and sample counts of the two LUTs (`transmittance_lut.wesl`,
/// `multiscattering_lut.wesl`; the sun's gain and the other LUTs' settings are not read).
pub(crate) fn lut_words(
    atmosphere: &ExtractedAtmosphere,
    settings: &GpuAtmosphereSettings,
) -> [u32; 12] {
    [
        atmosphere.inner_radius.to_bits(),
        atmosphere.outer_radius.to_bits(),
        atmosphere.ground_albedo.x.to_bits(),
        atmosphere.ground_albedo.y.to_bits(),
        atmosphere.ground_albedo.z.to_bits(),
        settings.transmittance_lut_size.x,
        settings.transmittance_lut_size.y,
        settings.transmittance_lut_samples,
        settings.multiscattering_lut_size.x,
        settings.multiscattering_lut_size.y,
        settings.multiscattering_lut_dirs,
        settings.multiscattering_lut_samples,
    ]
}

/// Whether a view's LUTs are kept this frame, given what they were last drawn with, and what
/// they are then drawn with (`None` until the LUT pass can dispatch).
pub(crate) fn keep(
    last: Option<&LutInputs>,
    now: LutInputs,
    can_draw: bool,
    enabled: bool,
) -> (bool, Option<LutInputs>) {
    if !can_draw {
        // The pass returns before it dispatches: nothing is drawn, so nothing is known.
        return (false, None);
    }
    let kept = enabled && last == Some(&now);
    (kept, Some(now))
}

/// Marks each atmosphere view [`KeptAtmosphereLuts`] for the LUT pass.
pub(crate) fn prepare_kept_atmosphere_luts(
    mut commands: Commands,
    views: Query<(
        Entity,
        &ExtractedAtmosphere,
        &GpuAtmosphereSettings,
        &AtmosphereTextures,
    )>,
    media: Res<RenderAssets<GpuScatteringMedium>>,
    pipelines: Res<AtmosphereLutPipelines>,
    pipeline_cache: Res<PipelineCache>,
    mut drawn: Local<EntityHashMap<LutInputs>>,
    mut seen: Local<EntityHashSet>,
) {
    let can_draw = [
        pipelines.transmittance_lut,
        pipelines.multiscattering_lut,
        pipelines.sky_view_lut,
        pipelines.aerial_view_lut,
    ]
    .into_iter()
    .all(|id| pipeline_cache.get_compute_pipeline(id).is_some());
    let enabled = KEEP_UNCHANGED_LUTS.load(Ordering::Relaxed);
    seen.clear();
    for (entity, atmosphere, settings, textures) in &views {
        seen.insert(entity);
        let Some(medium) = media.get(atmosphere.medium) else {
            // The bind groups fail for this view too, so the pass does not run.
            drawn.remove(&entity);
            continue;
        };
        let now = LutInputs {
            words: lut_words(atmosphere, settings),
            views: [
                medium.density_lut_view.id(),
                medium.scattering_lut_view.id(),
                textures.transmittance_lut.default_view.id(),
                textures.multiscattering_lut.default_view.id(),
            ],
        };
        let (kept, after) = keep(drawn.get(&entity), now, can_draw, enabled);
        match after {
            Some(inputs) => drawn.insert(entity, inputs),
            None => drawn.remove(&entity),
        };
        commands.entity(entity).insert(KeptAtmosphereLuts(kept));
    }
    drawn.retain(|entity, _| seen.contains(entity));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(word: u32) -> LutInputs {
        LutInputs {
            words: [word; 12],
            views: [TextureViewId::new(); 4],
        }
    }

    #[test]
    fn the_luts_are_drawn_the_first_frame_and_kept_while_their_inputs_hold() {
        let first = inputs(1);
        let (kept, last) = keep(None, first.clone(), true, true);
        assert!(!kept, "nothing drawn yet");
        let (kept, _) = keep(last.as_ref(), first, true, true);
        assert!(kept, "the same inputs keep the LUTs");
    }

    #[test]
    fn a_changed_input_or_texture_draws_them_again() {
        let first = inputs(1);
        let (_, last) = keep(None, first.clone(), true, true);
        assert!(
            !keep(last.as_ref(), inputs(2), true, true).0,
            "a changed radius draws them"
        );
        let mut moved = first.clone();
        moved.views[2] = TextureViewId::new();
        assert!(
            !keep(last.as_ref(), moved, true, true).0,
            "a new texture draws them"
        );
    }

    #[test]
    fn nothing_is_kept_before_the_pass_can_draw_or_when_switched_off() {
        let first = inputs(1);
        let (kept, last) = keep(None, first.clone(), false, true);
        assert!(
            !kept && last.is_none(),
            "a pass that cannot dispatch draws nothing"
        );
        let (_, last) = keep(None, first.clone(), true, true);
        assert!(
            !keep(last.as_ref(), first, true, false).0,
            "off draws every frame"
        );
    }

    #[test]
    fn the_sun_gain_is_not_an_input() {
        let atmosphere = ExtractedAtmosphere {
            inner_radius: 6.36e6,
            outer_radius: 6.46e6,
            ground_albedo: bevy_math::Vec3::splat(0.3),
            medium: Default::default(),
            world_to_atmosphere: bevy_math::Mat4::IDENTITY,
        };
        let mut settings = GpuAtmosphereSettings::default();
        let before = lut_words(&atmosphere, &settings);
        settings.sun_gain = 3.0;
        settings.sky_view_lut_samples += 1;
        assert_eq!(before, lut_words(&atmosphere, &settings));
        settings.multiscattering_lut_samples += 1;
        assert_ne!(before, lut_words(&atmosphere, &settings));
    }
}
