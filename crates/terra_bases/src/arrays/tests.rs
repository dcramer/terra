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

/// Arrays as `setup` makes them, with no GPU: nothing drawn yet.
fn arrays_alone(images: &mut Assets<Image>) -> Arrays {
    bevy::tasks::AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::new);
    Arrays::new(
        images,
        &mut Assets::<BuildMaterial>::default(),
        true,
        Quality::default(),
        Arc::new(Layers::default()),
        Arc::new(|_: &str| false),
        Handle::default(),
    )
}

/// The render world drawing each array at its asked side, the first `swapped` of them at
/// `layers`, the rest at `before`.
fn draw(arrays: &Arrays, swapped: usize, layers: u32, before: u32) {
    let mut drawn = arrays.uploads.drawn.lock().unwrap();
    for (i, (id, [side, _])) in arrays.asked().into_iter().enumerate() {
        drawn.insert(id, Some([side, if i < swapped { layers } else { before }]));
    }
}

#[test]
fn grown_arrays_are_bound_again_once_all_are_drawn_and_their_new_slots_wait_for_them() {
    let mut images = Assets::<Image>::default();
    let mut arrays = arrays_alone(&mut images);
    // Nothing drawn yet: the material is made again until it is.
    assert_eq!(arrays.follow_drawn(), 0);
    assert!(arrays.table_dirty);
    draw(&arrays, 5, FIRST_SLOTS, FIRST_SLOTS);
    arrays.follow_drawn();
    arrays.table_dirty = false;
    assert_eq!(arrays.follow_drawn(), FIRST_SLOTS);
    assert!(!arrays.table_dirty, "settled, the material is left alone");

    // Full, the arrays grow, and the next set goes into the first new slot.
    arrays.free.clear();
    assert!(arrays.grow(&mut images));
    let grown = arrays.capacity;
    assert!(grown > FIRST_SLOTS);
    let slot = arrays.free.pop().unwrap();
    assert!(slot >= FIRST_SLOTS);
    arrays.slots.insert(
        "materials/timber".into(),
        Slot {
            slot,
            ready_in: 2,
            last_used: 0.0,
        },
    );
    // The render world swaps the grown copies in one a frame, as the uploads' cap leaves room
    // (D420), and more frames pass than a written set waits.
    for swapped in 0..5 {
        draw(&arrays, swapped, grown, FIRST_SLOTS);
        arrays.table_dirty = false;
        assert_eq!(arrays.follow_drawn(), FIRST_SLOTS);
        assert!(
            arrays.table_dirty,
            "made again while {} of 5 arrays are drawn at their old size",
            5 - swapped
        );
        assert_eq!(
            slot_of(arrays.slots.get("materials/timber")),
            MISSING,
            "a set in slot {slot} is drawn plain while a drawn copy holds {FIRST_SLOTS} slots"
        );
    }
    // All swapped: made again once more, binding every grown copy, and the set ready once
    // its write has passed.
    draw(&arrays, 5, grown, grown);
    let mut frames = 0;
    let mut remade = 0;
    while slot_of(arrays.slots.get("materials/timber")) == MISSING {
        arrays.table_dirty = false;
        assert_eq!(arrays.follow_drawn(), grown);
        remade += usize::from(arrays.table_dirty);
        frames += 1;
        assert!(frames <= 2, "ready two frames after its copy is drawn");
    }
    assert_eq!(slot_of(arrays.slots.get("materials/timber")), slot as f32);
    assert_eq!(
        remade, 2,
        "made again as the copies are drawn and as the set is ready"
    );
    arrays.table_dirty = false;
    arrays.follow_drawn();
    assert!(!arrays.table_dirty);
}
