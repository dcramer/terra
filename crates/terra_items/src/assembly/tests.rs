//! The assembler on made-up parts, the numbers of tools/items/fixtures.ts: a box head with a
//! round eye 3.5 cm across and 6 cm tall, a collar under it, a code-made ash haft; the real
//! types and the real axe kind. Each answer is worked from the convention (D566) and D459's
//! rule by hand, not read back from the assembler.

use std::f32::consts::PI;

use super::*;

const TYPES: &str = include_str!("../../../../assets/data/parts/types.json");
const AXE: &str = include_str!("../../../../assets/data/kinds/axe.json");
const SHIELD: &str = include_str!("../../../../assets/data/kinds/round_shield.json");
// Made-up densities, so the answers do not move when the real ones are refined.
const MATERIALS: &str = r#"{"materials": {
  "iron": {"density_kg_m3": {"value": 7800, "source": "made up for the assembler's tests"}},
  "stone": {"density_kg_m3": {"value": 2600, "source": "made up for the assembler's tests"}}}}"#;

const EYE: f32 = 0.035;
const HEAD_H: f32 = 0.06;
const HEAD_M3: f32 = 0.9 / 7800.0; // 0.9 kg in iron
const HEAD_KG: f32 = 0.9;
const COLLAR_KG: f32 = 0.01;
const LEN: f32 = 0.68;
const GRIP: f32 = 0.08;
const ASH: f32 = 670.0;
const PROUD: f32 = 0.004; // D566

fn head(upper_x: f32) -> String {
    format!(
        r#"{{"part": "test_box_head", "type": "axe_head", "volume_m3": {{"value": {HEAD_M3}}}, "centre_m": [0, 0.03, 0], "bounds_m": [[-0.03, 0, -0.03], [0.03, {HEAD_H}, 0.03]],
        "points": [
          {{"name": "eye.lower", "type": "eye", "at": [0, 0, 0], "dir": [0, 1, 0], "size": {EYE}}},
          {{"name": "eye.upper", "type": "eye", "at": [{upper_x}, {HEAD_H}, 0], "dir": [0, 1, 0], "size": {EYE}}},
          {{"name": "edge.lower", "type": "edge", "at": [0, 0.01, -0.03], "dir": [0, 0, -1]}},
          {{"name": "edge.upper", "type": "edge", "at": [0, 0.05, -0.03], "dir": [0, 0, -1]}}]}}"#
    )
}

fn haft(id: &str, len: f32) -> String {
    format!(
        r#"{{"part": "{id}", "type": "haft", "length_m": {{"value": {len}}}, "grip_from_end_m": {{"value": {GRIP}}}, "grip_diameter_m": {{"value": {EYE}}}, "density_kg_m3": {{"value": {ASH}}}}}"#
    )
}

const COLLAR: &str = r#"{"part": "test_collar", "type": "collar", "mass_kg": {"value": 0.01}, "centre_m": [0, -0.01, 0], "bounds_m": [[-0.022, -0.02, -0.022], [0.022, 0, 0.022]], "cover_m": [-0.02, 0],
  "points": [{"name": "bore", "type": "bore", "at": [0, 0, 0], "dir": [0, 1, 0], "size": 0.035}]}"#;

fn lib(head: &str) -> Library {
    let long = haft("test_long_haft", 0.8);
    Library::from_json(
        TYPES,
        MATERIALS,
        &[AXE],
        &[head, &haft("test_haft", LEN), &long, COLLAR],
    )
    .expect("the made-up parts read")
}

fn recipe(parts: &[&str], seed: u64) -> Recipe {
    in_material(parts, "iron", seed)
}

fn in_material(parts: &[&str], material: &str, seed: u64) -> Recipe {
    Recipe {
        kind: "axe".into(),
        parts: parts.iter().map(|p| p.to_string()).collect(),
        material: material.into(),
        seed,
    }
}

const GOOD: [&str; 3] = ["test_box_head", "test_haft", "test_collar"];

#[test]
fn a_good_axe_is_placed_by_the_convention_with_its_grip_at_the_origin() {
    let a = assemble(&lib(&head(0.0)), &recipe(&GOOD, 1)).expect("a good axe assembles");
    // The haft's top stands 4 mm proud of the eye's upper opening; the collar's top meets the eye's lower.
    let head_y = LEN - PROUD - HEAD_H;
    let at = |id: &str| a.placed.iter().find(|p| p.part == id).expect("placed").at;
    assert_eq!(at("test_haft"), [0.0, -GRIP, 0.0]);
    assert!((at("test_box_head")[1] - (head_y - GRIP)).abs() < 1e-6);
    assert!((at("test_collar")[1] - (head_y - GRIP)).abs() < 1e-6);
    assert!(
        a.placed.iter().all(|p| p.turn == [0.0, 0.0, 0.0, 1.0]),
        "nothing on the line turns"
    );
    // The haft is one diameter all along, the eye's, so its section is that everywhere.
    assert!(a.section.iter().all(|s| (s[1] - EYE).abs() < 1e-6));
    assert!((a.section.last().unwrap()[0] - (LEN - GRIP)).abs() < 1e-6);
    // The edge is the head's bit span carried with it.
    let edge = a.edge.expect("an axe has an edge");
    assert!((edge.from[1] - (head_y - GRIP + 0.01)).abs() < 1e-6 && edge.side == [0.0, 0.0, -1.0]);
    assert_eq!(a.second, None, "a one-handed axe has no second grip");
}

#[test]
fn its_mass_balance_and_inertia_are_its_parts_by_d459s_rule() {
    let a = assemble(&lib(&head(0.0)), &recipe(&GOOD, 1)).unwrap();
    let head_y = LEN - PROUD - HEAD_H;
    let haft_kg = haft_kg();
    let m = HEAD_KG + COLLAR_KG + haft_kg;
    let centre =
        (HEAD_KG * (head_y + 0.03) + COLLAR_KG * (head_y - 0.01) + haft_kg * LEN / 2.0) / m;
    let b = centre - GRIP;
    assert!((a.mass_kg - m).abs() < 1e-4, "{} against {m}", a.mass_kg);
    assert!(
        (a.balance_m - b).abs() < 1e-4,
        "{} against {b}",
        a.balance_m
    );
    assert!((a.length_m - LEN).abs() < 1e-6);
    assert!((a.inertia_kgm2 - m * (b * b + LEN * LEN / 12.0)).abs() < 1e-4);
}

#[test]
fn the_recipes_material_weighs_the_head_by_its_density() {
    let l = lib(&head(0.0));
    let iron = assemble(&l, &recipe(&GOOD, 1)).unwrap();
    // The same head in stone weighs 2600/7800 of its iron self; the haft and collar do not change.
    let stone_kg = HEAD_M3 * 2600.0;
    let e = assemble(&l, &in_material(&GOOD, "stone", 1)).unwrap_err();
    assert!(
        e.contains(&format!(
            "head_mass_kg {stone_kg:.4} is outside its envelope"
        )),
        "a 0.3 kg stone head is under the axe's band: {e}"
    );
    assert!((iron.mass_kg - (HEAD_KG + COLLAR_KG + haft_kg())).abs() < 1e-4);
    let e = assemble(&l, &in_material(&GOOD, "glass", 1)).unwrap_err();
    assert!(
        e.contains("material glass is not in assets/data/parts/materials.json; the materials are iron, stone"),
        "{e}"
    );
}

#[test]
fn every_material_gives_its_density_with_its_source() {
    let text = include_str!("../../../../assets/data/parts/materials.json");
    let l = Library::from_json(TYPES, text, &[], &[]).expect("materials.json reads");
    assert!(l.materials.contains_key("iron") && l.materials.contains_key("stone"));
    let bare = r#"{"materials": {"iron": {"density_kg_m3": {"value": 7870}}}}"#;
    let e = Library::from_json(TYPES, bare, &[], &[]).unwrap_err();
    assert!(e.contains("missing field `source`"), "{e}");
}

fn haft_kg() -> f32 {
    PI * (EYE / 2.0) * (EYE / 2.0) * LEN * ASH
}

#[test]
fn the_handle_meets_an_eye_proud_of_its_top_and_a_trim_under_its_entry() {
    let l = lib(&head(0.0));
    let (h, haft, collar) = (
        &l.parts["test_box_head"],
        &l.parts["test_haft"],
        &l.parts["test_collar"],
    );
    assert_eq!(
        meet(&l, haft, h),
        Some(Meet::HandleTop {
            head: "test_box_head".into(),
            y: HEAD_H + PROUD,
            d: EYE
        })
    );
    // Its cover's top (0) against eye.lower (0): no shift, no turn.
    assert_eq!(
        meet(&l, collar, h),
        Some(Meet::Placed {
            part: "test_collar".into(),
            onto: "test_box_head".into(),
            at: [0.0, 0.0, 0.0],
            turn: ONE
        })
    );
    assert_eq!(meet(&l, h, collar), meet(&l, collar, h), "either order");
    assert_eq!(
        meet(&l, haft, collar),
        None,
        "a trim meets the handle at sizes only"
    );
    let mut names: Vec<_> = pairings(&l, "test_box_head")
        .unwrap()
        .into_iter()
        .map(|p| p.partner)
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["test_collar", "test_haft", "test_long_haft"],
        "every partner its kind allows, once"
    );
}

#[test]
fn a_seated_trim_is_turned_so_its_seat_lies_on_its_boards_point() {
    // A boss whose seat faces -Z, rolled +Y, 1 cm behind its origin, onto a board's boss point
    // facing +X, rolled +Y, at (0.1, 0.2, 0): the boss turns a quarter about +Y.
    let board = r#"{"part": "test_board", "type": "round_board", "points": [
      {"name": "boss", "type": "seat", "at": [0.1, 0.2, 0], "dir": [1, 0, 0], "up": [0, 1, 0]}]}"#;
    let boss = r#"{"part": "test_boss", "type": "boss", "points": [
      {"name": "seat", "type": "seat", "at": [0, 0, 0.01], "dir": [0, 0, -1], "up": [0, 1, 0]}]}"#;
    let l = Library::from_json(TYPES, MATERIALS, &[SHIELD], &[board, boss]).unwrap();
    let Some(Meet::Placed { at, turn, .. }) =
        meet(&l, &l.parts["test_boss"], &l.parts["test_board"])
    else {
        panic!("a boss sits on its board");
    };
    let q = glam::Quat::from_array(turn);
    let moved = |p: [f32; 3]| q * glam::Vec3::from(p) + glam::Vec3::from(at);
    // The seat's point lands on the board's, its direction on the board's, its roll kept.
    assert!(moved([0.0, 0.0, 0.01]).distance(glam::Vec3::new(0.1, 0.2, 0.0)) < 1e-6);
    assert!((q * glam::Vec3::NEG_Z).distance(glam::Vec3::X) < 1e-6);
    assert!((q * glam::Vec3::Y).distance(glam::Vec3::Y) < 1e-6);
}

#[test]
fn the_same_recipe_gives_the_same_bytes() {
    let l = lib(&head(0.0));
    let first = assemble(&l, &recipe(&GOOD, 7)).unwrap().to_bytes();
    let again = assemble(&lib(&head(0.0)), &recipe(&GOOD, 7))
        .unwrap()
        .to_bytes();
    assert_eq!(first, again);
    assert_ne!(
        first,
        assemble(&l, &recipe(&GOOD, 8)).unwrap().to_bytes(),
        "the seed is part of what is made"
    );
}

#[test]
fn parts_stay_separate_and_share_their_mesh_across_items() {
    let l = lib(&head(0.0));
    let one = assemble(&l, &recipe(&GOOD, 1)).unwrap();
    let two = assemble(&l, &recipe(&["test_box_head", "test_haft"], 2)).unwrap();
    assert_eq!(one.placed.len(), 3, "one placement a part, never merged");
    let heads = |a: &Assembly| {
        a.placed
            .iter()
            .filter(|p| p.part == "test_box_head")
            .count()
    };
    assert_eq!(
        (heads(&one), heads(&two)),
        (1, 1),
        "both items name the one head part, so they draw its one mesh"
    );
}

#[test]
fn an_item_outside_its_kinds_envelope_is_refused_with_the_number_and_the_limit() {
    let e = assemble(
        &lib(&head(0.0)),
        &recipe(&["test_box_head", "test_long_haft"], 1),
    )
    .unwrap_err();
    assert!(
        e.contains("the axe's length_m 0.8000 is outside its envelope [0.612, 0.748]"),
        "{e}"
    );
}

#[test]
fn a_point_off_the_line_is_refused() {
    let e = assemble(&lib(&head(0.005)), &recipe(&GOOD, 1)).unwrap_err();
    assert!(e.contains("eye.upper") && e.contains("off the line"), "{e}");
}

#[test]
fn a_part_with_no_slot_or_a_missing_slot_is_refused() {
    let l = lib(&head(0.0));
    let e = assemble(&l, &recipe(&["test_box_head", "test_haft", "test_haft"], 1)).unwrap_err();
    assert!(e.contains("fits no free slot of the axe"), "{e}");
    let e = assemble(&l, &recipe(&["test_haft"], 1)).unwrap_err();
    assert!(e.contains("head slot is empty"), "{e}");
    let e = assemble(&l, &recipe(&["test_box_head", "nothing"], 1)).unwrap_err();
    assert!(
        e.contains("part nothing is not in assets/data/parts"),
        "{e}"
    );
}

#[test]
fn the_library_on_disk_reads() {
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let l = Library::read(&assets).expect("every kind, material and part reads");
    let kinds = std::fs::read_dir(assets.join("data/kinds"))
        .expect("the kinds' folder")
        .filter(|f| {
            f.as_ref().is_ok_and(|f| {
                let n = f.file_name();
                n.to_string_lossy().ends_with(".json") && n != "kind.schema.json"
            })
        })
        .count();
    assert_eq!(l.kinds.len(), kinds);
    assert!(l.kinds.contains_key("axe") && l.materials.contains_key("iron"));
}

#[test]
fn a_part_is_made_for_the_kinds_it_names_or_else_every_kind_with_a_slot_for_its_type() {
    let l = Library::from_json(TYPES, MATERIALS, &[AXE, SHIELD], &[]).unwrap();
    let names = |ks: Vec<&Kind>| ks.into_iter().map(|k| k.kind.clone()).collect::<Vec<_>>();
    // axe.json's slots take a haft and a collar; round_shield.json's a grip bar and a boss.
    assert_eq!(names(kinds_taking(&l, "haft")), ["axe"]);
    assert_eq!(names(kinds_taking(&l, "boss")), ["round_shield"]);
    assert!(kinds_taking(&l, "blade").is_empty());
    assert_eq!(names(kinds_for(&l, "haft", None)), ["axe"]);
    let named = ["round_shield".to_string(), "no_such_kind".to_string()];
    assert_eq!(names(kinds_for(&l, "haft", Some(&named))), ["round_shield"]);
}
