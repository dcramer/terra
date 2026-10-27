use super::*;
use bevy::{
    core_pipeline::core_3d::TransparentSortingInfo3d,
    render::render_phase::{DrawFunctionId, PhaseItemExtraIndex},
    render::render_resource::CachedRenderPipelineId,
};

fn card(entity: Entity, batch: core::ops::Range<u32>) -> Transparent3d {
    Transparent3d {
        sorting_info: TransparentSortingInfo3d::AlwaysOnTop,
        distance: 0.0,
        pipeline: CachedRenderPipelineId::INVALID,
        entity: (entity, MainEntity::from(entity)),
        draw_function: DrawFunctionId(0),
        batch_range: batch,
        extra_index: PhaseItemExtraIndex::None,
        indexed: true,
    }
}

#[test]
fn the_fires_cards_leave_the_transparent_pass_for_their_own_in_their_order() {
    let mut world = World::new();
    let [lake, smoke, soot, flames, window] = [(); 5].map(|_| world.spawn_empty().id());
    let mut phase = SortedRenderPhase::<Transparent3d>::default();
    // The smoke and the soot one batch of one material, as the batching leaves them.
    for (e, batch) in [
        (lake, 0..1),
        (smoke, 1..3),
        (soot, 2..3),
        (flames, 3..4),
        (window, 4..5),
    ] {
        phase.add_transient(card(e, batch));
    }
    let seen: MainEntityHashSet = [smoke, soot, flames]
        .map(MainEntity::from)
        .into_iter()
        .collect();
    let mut late = SortedRenderPhase::default();
    take_late(&mut phase, &mut late, &seen);
    let order: Vec<_> = late
        .items
        .values()
        .map(|i| (i.entity.0, i.batch_range.clone()))
        .collect();
    assert_eq!(order, vec![(smoke, 1..3), (soot, 2..3), (flames, 3..4)]);
    let left: Vec<_> = phase
        .items
        .values()
        .map(|i| (i.entity.0, i.batch_range.clone()))
        .collect();
    assert_eq!(
        left,
        vec![
            (lake, 0..1),
            (smoke, 0..0),
            (soot, 0..0),
            (flames, 0..0),
            (window, 4..5)
        ]
    );
    // Taken again next frame, nothing is drawn twice.
    take_late(&mut phase, &mut late, &seen);
    assert_eq!(late.items.len(), 3);
}

#[test]
fn the_haze_is_told_as_the_fog_pass_is() {
    let volume = FogVolume {
        density_factor: 0.028,
        absorption: 0.05,
        scattering: 0.5,
        scattering_asymmetry: 0.7,
        ..default()
    };
    let box_m = Vec3::new(240.0, 60.0, 240.0);
    let at = GlobalTransform::from(Transform::from_xyz(10.0, 2000.0, -5.0).with_scale(box_m));
    let fog = VolumetricFog::default();
    let told = told(Some((&volume, &at)), Some(&fog));
    let uvw = Mat4::from_cols_array_2d(&[told[0], told[1], told[2], told[3]]);
    // The box's corners are its density picture's, its middle the picture's.
    let low = Vec3::new(10.0, 2000.0, -5.0) - box_m * 0.5;
    assert!(uvw.transform_point3(low).abs_diff_eq(Vec3::ZERO, 1e-4));
    assert!(
        uvw.transform_point3(low + box_m)
            .abs_diff_eq(Vec3::ONE, 1e-4)
    );
    assert_eq!(told[4][3], 0.028);
    assert!((told[6][3] - (box_m * 0.5).length()).abs() < 1e-2);
    assert_eq!(told[7][..3], [0.05, 0.5, 0.7]);
    // No haze, or no fog on the camera: no density, so the cards are left as they are.
    assert_eq!(super::told(None, Some(&fog))[4][3], 0.0);
    assert_eq!(super::told(Some((&volume, &at)), None)[4][3], 0.0);
}

#[test]
fn the_shaders_haze_is_as_long_as_its_told() {
    let shader =
        std::fs::read_to_string(crate::data::assets().join("shaders/fire_fog.wesl")).unwrap();
    let start = shader.find("struct Haze {").unwrap();
    let body = &shader[start..start + shader[start..].find('}').unwrap()];
    let vec4s = body.matches("vec4<f32>,").count() + 4 * body.matches("mat4x4<f32>,").count();
    assert_eq!(vec4s, HAZE_VEC4S);
    for (file, binding) in [
        ("fire.wesl", "@binding(112)"),
        ("fire_smoke.wesl", "@binding(106)"),
    ] {
        let shader =
            std::fs::read_to_string(crate::data::assets().join("shaders").join(file)).unwrap();
        assert!(
            shader.contains(&format!("{binding} var<storage, read> haze: Haze;"))
                || shader.contains(&format!("{binding}\nvar<storage, read> haze: Haze;")),
            "{file}"
        );
    }
}

#[test]
fn each_smoke_is_drawn_by_one_material() {
    // A hearth's smoke (kind 0) is the torches' material's or the other's, never both nor
    // neither, and soot (kind 1) has one: else a torch's smoke is drawn twice, or not at all.
    for (kind, torch) in [(0.0, false), (0.0, true), (1.0, false)] {
        let drawing = SMOKES
            .iter()
            .filter(|s| s.kind == kind && s.torches == torch)
            .count();
        assert_eq!(drawing, 1, "kind {kind}, torch {torch}");
    }
}
