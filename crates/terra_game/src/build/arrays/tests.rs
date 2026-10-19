use super::*;

#[test]
fn a_set_not_yet_resident_or_written_never_reads_another_slot_nor_the_damage_mask() {
    // Not resident, resident but not yet written, and ready.
    let reading = Slot {
        slot: 3,
        ready_in: 1,
        last_used: 0.0,
    };
    let ready = Slot {
        slot: 3,
        ready_in: 0,
        last_used: 0.0,
    };
    for drawn in [slot_of(None), slot_of(Some(&reading))] {
        assert!(
            drawn < 0.0,
            "drawn plain until written, not from slot {drawn}"
        );
        assert_ne!(drawn, DAMAGE_SLOT as f32);
    }
    assert_eq!(slot_of(Some(&ready)), 3.0);
}

#[test]
fn kits_naming_the_same_set_share_its_layer_and_pieces_have_none() {
    let texture = |path: &str| terra_build::kit::Texture {
        path: path.into(),
        metres: [1.0, 1.0],
        depth: 0.0,
        reflectance: 0.35,
        strips: None,
        cells: 1,
        worn: None,
    };
    let kit = |id: u16, masonry: &str| terra_build::kit::Kit {
        id,
        textures: [
            (Material::Masonry, texture(masonry)),
            (Material::Dressed, texture("materials/dressed")),
            (Material::Piece, texture("materials/atlas")),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let layers = Layers::of(&Kits(vec![
        kit(2, "materials/stone"),
        kit(7, "materials/lime"),
    ]));
    let stone = layers.get(Kind(2), Material::Masonry).unwrap();
    let lime = layers.get(Kind(7), Material::Masonry).unwrap();
    assert_ne!(stone, lime);
    assert_eq!(
        layers.get(Kind(2), Material::Dressed),
        layers.get(Kind(7), Material::Dressed)
    );
    assert_eq!(layers.get(Kind(2), Material::Piece), None);
    assert!(stone > 0 && lime > 0);
    assert_eq!(layers.sets.len(), 3);
}

#[test]
fn a_materials_detail_is_a_layer_of_its_own_at_its_scale() {
    let texture = |path: &str, metres: [f32; 2]| terra_build::kit::Texture {
        path: path.into(),
        metres,
        depth: 0.0,
        reflectance: 0.35,
        strips: None,
        cells: 1,
        worn: None,
    };
    let detail = terra_build::kit::Detail {
        path: "materials/grain".into(),
        metres: 0.5,
        strength: 0.8,
    };
    let kit = terra_build::kit::Kit {
        id: 3,
        textures: [
            (Material::Masonry, texture("materials/stone", [2.0, 2.0])),
            (Material::Floor, texture("materials/flags", [1.0, 1.5])),
        ]
        .into_iter()
        .collect(),
        details: [(Material::Masonry, detail)].into_iter().collect(),
        ..Default::default()
    };
    let layers = Layers::of(&Kits(vec![kit]));
    let wall = layers.get(Kind(3), Material::Masonry).unwrap();
    let floor = layers.get(Kind(3), Material::Floor).unwrap();
    // Made resident with it, but not what it waits for.
    assert_eq!(layers.with_worn(&[wall]), vec![wall]);
    let grain = *layers.details_of(&[wall]).first().expect("its detail");
    assert_eq!(layers.set(grain).unwrap().path, "materials/grain");
    // A 2 m copy of the wall holds four of the half-metre detail each way.
    let (_, scale, strength) = layers.set(wall).unwrap().detail.clone().unwrap();
    assert_eq!((scale, strength), ([4.0, 4.0], 0.8));
    // The floor has none.
    assert!(layers.details_of(&[floor]).is_empty());
    assert!(layers.set(floor).unwrap().detail.is_none());
}

#[test]
fn a_picture_twice_the_budget_is_averaged_down_and_tiles() {
    let side = SIDE as usize * 2;
    let data: Vec<u8> = (0..side * side)
        .flat_map(|i| [(i % 2 * 200) as u8, 50, 50, 255])
        .collect();
    let out = to_side(&data, side as u32, side as u32, 4, SIDE);
    assert_eq!(out.len(), SIDE as usize * SIDE as usize * 4);
    assert!(out.chunks_exact(4).all(|p| p[0] == 100 && p[1] == 50));
}
