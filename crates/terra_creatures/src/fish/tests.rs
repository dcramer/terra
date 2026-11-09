use super::*;

fn defs() -> Vec<FishDef> {
    terra_world::data::read(FILE)
}

/// Every row's numbers make sense and its model is there, textured and long along its z:
/// a row that cannot swim is found here, not as an error in play.
#[test]
fn every_kind_has_a_model_and_sane_numbers() {
    let defs = defs();
    assert!(
        defs.len() >= 3,
        "a small schooling fish, a trout and a pike"
    );
    for def in &defs {
        let parts = terra_world::models::read_model(&def.model).unwrap();
        let part = (parts.iter().find(|part| part.textures.is_some()))
            .unwrap_or_else(|| panic!("{}'s model has no textured part", def.name));
        let (low, high) = part
            .positions
            .iter()
            .fold((Vec3::MAX, Vec3::MIN), |(low, high), at| {
                (low.min(*at), high.max(*at))
            });
        let size = high - low;
        // Upright, its length along z (tools/props' `upright`): longer than tall, taller
        // than thick.
        assert!(size.z > size.y && size.y > size.x, "{}: {size}", def.name);
        // Its head at +Z, which it swims toward (D303): a fish is thickest ahead of its
        // middle, and thin at its tail, which is a fin on edge.
        let thick = |slice: usize| {
            let (mut least, mut most) = (f32::MAX, f32::MIN);
            for at in &part.positions {
                if (((at.z - low.z) / size.z * 10.0) as usize).min(9) == slice {
                    least = least.min(at.x);
                    most = most.max(at.x);
                }
            }
            (most - least).max(0.0)
        };
        let ahead: f32 = (5..10).map(thick).sum();
        let behind: f32 = (0..5).map(thick).sum();
        assert!(
            ahead > behind * 1.15,
            "{} swims tail first: {ahead} thick toward +Z, {behind} toward -Z (tools/props' upright)",
            def.name
        );
        for (least, most) in [def.long_m, def.deep_m, def.under_m, def.path_m] {
            assert!(
                0.0 < least && least <= most,
                "{}: {least} to {most}",
                def.name
            );
        }
        assert!(def.school.0 >= 1 && def.school.0 <= def.school.1);
        assert!(
            def.under_m.0 + OFF_THE_BED_M < def.deep_m.0,
            "{} swims in its bed",
            def.name
        );
        assert!(def.seen_m > 5.0 && def.speed > 0.0 && def.a_ha > 0.0 && def.keeps > 1.0);
    }
}

/// Dry ground and water no kind keeps to hold no school; water that suits holds each kind
/// about as often as its row says, the same every time it is asked.
#[test]
fn a_cells_school_is_by_its_waters_depth_and_the_kinds_shares() {
    let defs = defs();
    let cells = || (0..4000).map(|i| IVec2::new(i % 80 - 40, i / 80 - 25));
    assert!(cells().all(|cell| kind_at(9, cell, &defs, -1.0).is_none()));
    assert!(cells().all(|cell| kind_at(9, cell, &defs, 0.05).is_none()));
    assert!(cells().all(|cell| kind_at(9, cell, &defs, 5000.0).is_none()));
    for depth in [0.4, 1.2, 4.0] {
        let suits: Vec<usize> = (0..defs.len())
            .filter(|&kind| depth >= defs[kind].deep_m.0 && depth <= defs[kind].deep_m.1)
            .collect();
        let mut held = vec![0; defs.len()];
        for cell in cells() {
            let kind = kind_at(9, cell, &defs, depth);
            assert_eq!(kind, kind_at(9, cell, &defs, depth));
            if let Some(kind) = kind {
                assert!(suits.contains(&kind), "{} in {depth} m", defs[kind].name);
                held[kind] += 1;
            }
        }
        for &kind in &suits {
            let wanted = defs[kind].a_ha * CELL_HA * 4000.0;
            let got = held[kind] as f32;
            assert!(
                (got - wanted).abs() < wanted * 0.25 + 12.0,
                "{} in {depth} m: {got} schools, {wanted} by its row",
                defs[kind].name
            );
        }
    }
}
