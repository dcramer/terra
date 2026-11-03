use super::*;
use std::path::Path;

/// Every subset of `items`.
fn states(items: &[String]) -> Vec<Vec<String>> {
    (0..1u32 << items.len())
        .map(|mask| {
            (items.iter().enumerate())
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, item)| item.clone())
                .collect()
        })
        .collect()
}

/// What is wrong with a figure of `shown` on `body`: a slot with no piece or two, a piece
/// shown twice, a region drawn twice or not at all.
fn faults(w: &Wardrobe, body: &str, shown: &[&str]) -> Vec<String> {
    let mut faults = Vec::new();
    for slot in &w.slots {
        let n = (shown.iter())
            .filter(|p| w.pieces[**p].slot.as_ref() == Some(slot))
            .count();
        if n != 1 {
            faults.push(format!("slot {slot} has {n} pieces"));
        }
    }
    let mut drawn: HashMap<&str, usize> = HashMap::new();
    for p in shown {
        assert_eq!(w.pieces[*p].body, body, "{p} is made on another body");
        for r in &w.pieces[*p].regions {
            *drawn.entry(r.as_str()).or_default() += 1;
        }
    }
    for r in &w.regions {
        match drawn.get(r.as_str()).copied().unwrap_or(0) {
            1 => {}
            n => faults.push(format!("region {r} drawn {n} times")),
        }
    }
    let unique: BTreeSet<&&str> = shown.iter().collect();
    if unique.len() != shown.len() {
        faults.push("a piece shown twice".to_string());
    }
    faults
}

#[test]
fn every_look_in_every_state_of_undress_fills_each_slot_and_draws_each_region_once() {
    let w = wardrobe();
    assert!(!w.looks.is_empty(), "the catalogue has no looks");
    let mut checked = 0;
    for (name, look) in &w.looks {
        let mut items: Vec<String> = (look.pieces.values())
            .filter_map(|p| w.pieces[p].item.clone())
            .collect();
        items.sort();
        items.dedup();
        let whole = w.compose(&look.body, name, None);
        assert!(faults(w, &look.body, &whole).is_empty(), "{name} whole");
        for worn in states(&items) {
            let shown = w.compose(&look.body, name, Some(&worn));
            let wrong = faults(w, &look.body, &shown);
            assert!(
                wrong.is_empty(),
                "{name} wearing {worn:?}: {shown:?}: {wrong:?}"
            );
            // Each slot shows the look's piece exactly when its item is worn.
            for (slot, piece) in &look.pieces {
                let on = worn.contains(w.pieces[piece].item.as_ref().unwrap());
                assert_eq!(
                    shown.contains(&piece.as_str()),
                    on,
                    "{name} {slot} wearing {worn:?}"
                );
            }
            checked += 1;
        }
    }
    assert!(checked >= 8, "only {checked} states checked");
}

#[test]
fn the_starting_kit_is_the_ranger_and_taking_its_coat_off_shows_his_bare_torso_and_arms() {
    let w = wardrobe();
    let kit: Vec<String> = ["leather_coat", "trousers", "boots", "small_pack"]
        .map(String::from)
        .to_vec();
    let dressed = w.compose("male", "ranger", Some(&kit));
    assert_eq!(
        dressed,
        [
            "male_ranger_body",
            "male_ranger_arms",
            "male_ranger_legs",
            "male_ranger_boots",
            "male_head"
        ]
    );
    // Their UpperBody, the bare arms, carries the head, so the head piece goes with it.
    let no_coat = w.compose("male", "ranger", Some(&kit[1..]));
    assert_eq!(
        no_coat,
        [
            "male_bare_torso",
            "male_bare_arms",
            "male_ranger_legs",
            "male_ranger_boots"
        ]
    );
    let no_trousers: Vec<String> = vec![kit[0].clone(), kit[2].clone()];
    assert!(
        w.compose("male", "ranger", Some(&no_trousers))
            .contains(&"male_bare_legs")
    );
    assert_eq!(w.compose("male", "ranger", None), dressed);
}

#[test]
fn a_worn_item_the_look_lacks_shows_another_piece_of_the_body_for_it() {
    let w = Wardrobe::parse(
        r#"{
        "slots": ["body"], "regions": ["head", "torso"],
        "bodies": {"b": {"model": "m.glb", "pieces": "p", "head": "head", "bare": {"body": "skin"}, "look": "plain"}},
        "pieces": {
            "head": {"body": "b", "regions": ["head"]},
            "skin": {"body": "b", "slot": "body", "regions": ["torso"]},
            "coat": {"body": "b", "slot": "body", "item": "coat", "regions": ["torso"]},
            "smock": {"body": "b", "slot": "body", "item": "smock", "regions": ["torso"]}
        },
        "looks": {"plain": {"body": "b", "pieces": {"body": "coat"}}}
    }"#,
    )
    .unwrap();
    assert_eq!(
        w.compose("b", "plain", Some(&["smock".into()])),
        ["smock", "head"]
    );
    assert_eq!(w.compose("b", "plain", Some(&[])), ["skin", "head"]);
    assert_eq!(w.file("smock").as_deref(), Some("p/smock.gltf"));
    assert_eq!(w.body_for_model("m.glb#Scene0").map(|(n, _)| n), Some("b"));
}

#[test]
fn a_catalogue_naming_what_it_lacks_is_refused() {
    let bad = r#"{
        "slots": ["body"], "regions": ["torso"],
        "bodies": {"b": {"model": "m.glb", "pieces": "p", "head": "head", "bare": {}, "look": "plain"}},
        "pieces": {"head": {"body": "b", "regions": ["torso"]}},
        "looks": {"plain": {"body": "b", "pieces": {}}}
    }"#;
    assert!(Wardrobe::parse(bad).unwrap_err().contains("no bare body"));
}

#[test]
fn every_body_and_piece_of_the_catalogue_has_its_game_file() {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let w = wardrobe();
    for body in w.bodies.values() {
        assert!(
            assets.join(&body.model).is_file(),
            "{} is missing",
            body.model
        );
    }
    for piece in w.pieces.keys() {
        let file = w.file(piece).unwrap();
        assert!(assets.join(&file).is_file(), "{file} is missing");
    }
}

#[test]
fn no_look_wearing_any_of_the_catalogues_items_composes_a_refused_pairing() {
    let w = wardrobe();
    let mut items: Vec<String> = w.pieces.values().filter_map(|p| p.item.clone()).collect();
    items.sort();
    items.dedup();
    let mut checked = 0;
    for (name, look) in &w.looks {
        for worn in states(&items) {
            let shown = w.compose(&look.body, name, Some(&worn));
            assert!(
                faults(w, &look.body, &shown).is_empty(),
                "{name} wearing {worn:?}"
            );
            for (i, a) in shown.iter().enumerate() {
                for b in &shown[i + 1..] {
                    assert!(
                        !w.refused(a, b),
                        "{name} wearing {worn:?} shows {a} with {b}"
                    );
                }
            }
            checked += 1;
        }
    }
    assert!(checked >= 32, "only {checked} states checked");
}

#[test]
fn a_refused_pairing_is_never_composed_and_no_look_may_wear_one() {
    let catalogue = |look: &str| {
        format!(
            r#"{{
            "slots": ["body", "legs"], "regions": ["head", "torso", "legs"],
            "bodies": {{"b": {{"model": "m.glb", "pieces": "p", "head": "head", "bare": {{"body": "skin", "legs": "bare_legs"}}, "look": "plain"}}}},
            "pieces": {{
                "head": {{"body": "b", "regions": ["head"]}},
                "skin": {{"body": "b", "slot": "body", "regions": ["torso"]}},
                "bare_legs": {{"body": "b", "slot": "legs", "regions": ["legs"]}},
                "coat": {{"body": "b", "slot": "body", "item": "coat", "regions": ["torso"]}},
                "full_trousers": {{"body": "b", "slot": "legs", "item": "trousers", "regions": ["legs"]}},
                "narrow_trousers": {{"body": "b", "slot": "legs", "item": "trousers", "regions": ["legs"]}},
                "kilt": {{"body": "b", "slot": "legs", "item": "kilt", "regions": ["legs"]}}
            }},
            "looks": {{"plain": {{"body": "b", "pieces": {look}}}}},
            "pairings": [
                {{"pieces": ["coat", "full_trousers"], "verdict": "refused", "worse": 22}},
                {{"pieces": ["coat", "kilt"], "verdict": "refused"}},
                {{"pieces": ["coat", "narrow_trousers"], "verdict": "pass"}}
            ]
        }}"#
        )
    };
    let bad = Wardrobe::parse(&catalogue(r#"{"body": "coat", "legs": "full_trousers"}"#));
    assert!(bad.unwrap_err().contains("refused pairing"));
    let w = Wardrobe::parse(&catalogue(r#"{"legs": "full_trousers"}"#)).unwrap();
    let wearing = |items: &[&str]| -> Vec<String> { items.iter().map(|s| s.to_string()).collect() };
    // The look's own trousers, until the coat is worn: then the trousers that pair with it.
    assert_eq!(
        w.compose("b", "plain", Some(&wearing(&["trousers"]))),
        ["skin", "full_trousers", "head"]
    );
    assert_eq!(
        w.compose("b", "plain", Some(&wearing(&["coat", "trousers"]))),
        ["coat", "narrow_trousers", "head"]
    );
    // Nothing that pairs: the slot shows bare rather than the refused pairing.
    assert_eq!(
        w.compose("b", "plain", Some(&wearing(&["coat", "kilt"]))),
        ["coat", "bare_legs", "head"]
    );
}
