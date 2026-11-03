use super::*;

#[test]
fn a_body_is_posed_less_often_the_further_it_is() {
    assert_eq!(every(5.0), 1);
    assert!(every(40.0) > every(20.0));
    assert!(every(70.0) >= every(40.0));
    assert_eq!(every(500.0), FAR_EVERY);
}

#[test]
fn of_a_group_one_is_worn_and_each_by_its_chance() {
    let garment = |item: &str, chance: f32, group: Option<&str>| Garment {
        item: item.into(),
        dye: "Cloth".into(),
        colours: vec![(0.5, 0.5, 0.5)],
        chance,
        group: group.map(Into::into),
    };
    let list = [
        garment("shirt", 1.0, Some("top")),
        garment("leather_coat", 1.0, Some("top")),
        garment("trousers", 1.0, None),
        garment("cap", 0.0, None),
    ];
    let mut coats = 0;
    for seed in 0..200 {
        let items: Vec<&str> = worn(&list, seed)
            .iter()
            .map(|(_, g)| g.item.as_str())
            .collect();
        let tops = items
            .iter()
            .filter(|i| ["shirt", "leather_coat"].contains(i))
            .count();
        assert_eq!(tops, 1, "{items:?}");
        assert!(
            items.contains(&"trousers") && !items.contains(&"cap"),
            "{items:?}"
        );
        coats += items.contains(&"leather_coat") as usize;
    }
    assert!((60..140).contains(&coats), "{coats} of 200 in a coat");
}

#[test]
fn every_role_in_the_plan_has_a_look_of_garments_there_are() {
    let looks: Looks = ron::from_str(
        &std::fs::read_to_string(terra_walker::model_files::asset("data/townsfolk.ron")).unwrap(),
    )
    .unwrap();
    let plan = terra_town::Plan::parse(
        &std::fs::read_to_string(terra_walker::model_files::asset("data/life.ron")).unwrap(),
    )
    .unwrap();
    for (activity, carried) in &looks.holds {
        assert!(
            plan.activities.contains_key(activity),
            "holds: no activity {activity}"
        );
        let file = character::held_look(&carried.item);
        assert!(
            terra_walker::model_files::asset(&file).exists(),
            "holds: no {file}"
        );
        // A hung look (a basket) is carried by its handle in the off hand (D567), as its
        // kind's lead says (tools/characters/attach_kinds.json); anything else in the main.
        let hung =
            terra_walker::model_files::extra(&terra_walker::model_files::json(&file), "hold")
                .is_some_and(|hold| hold == "hung");
        let hand = if hung {
            terra_motion::hold::Hand::Off
        } else {
            terra_motion::hold::Hand::Main
        };
        assert_eq!(
            carried.hand, hand,
            "holds: {activity}'s {} is carried in the {hand:?} hand (D567: a hung look in the off hand, the rest in the main); set its hand in data/townsfolk.ron",
            carried.item
        );
    }
    for role in &plan.roles {
        let look = looks.roles.get(&role.name).expect("a look for every role");
        for body in &look.bodies {
            assert!(appearance::Appearance::template(body).is_some(), "{body}");
        }
        for garment in look.wears.iter().chain(&look.women) {
            let file = character::look(&garment.item);
            assert!(
                terra_walker::model_files::asset(&file).exists(),
                "{}: no {file}",
                role.name
            );
            let materials =
                terra_walker::model_files::materials(&terra_walker::model_files::json(&file));
            assert!(
                materials
                    .iter()
                    .any(|m| m.split('.').next() == Some(garment.dye.as_str())),
                "{}: {} has no {} to dye",
                role.name,
                garment.item,
                garment.dye
            );
        }
    }
}

/// A townsperson going from a walk into a jog or back goes on at the same step, on the
/// body file's own clips (D585's one stride clock): the foot just planted in the one
/// giving way is planted in the one taking over, as far on to the next plant. Started at
/// a drawn point, as before, the other foot came down and the planted one slid. On every
/// body the town's people stand on.
#[test]
fn a_walk_into_a_jog_and_back_keeps_the_planted_foots_phase() {
    let bodies = appearance::template_bodies();
    assert!(bodies.len() >= 2, "a man's body and a woman's");
    for body in bodies {
        keeps_the_planted_foots_phase(body.clips());
    }
}

fn keeps_the_planted_foots_phase(file: &str) {
    let json = terra_walker::model_files::json(file);
    let mut markers: std::collections::HashMap<String, Markers> =
        serde_json::from_value(terra_walker::model_files::extra(&json, "markers").unwrap())
            .unwrap();
    let (idle, walk, jog) = (
        AnimationNodeIndex::new(1),
        AnimationNodeIndex::new(2),
        AnimationNodeIndex::new(3),
    );
    let clips = Clips {
        named: HashMap::default(),
        idle,
        walk,
        jog,
        walk_pace: 1.0,
        jog_pace: 1.0,
        strides: vec![
            (walk, markers.remove("Walk_Loop").unwrap()),
            (jog, markers.remove("Jog_Fwd_Loop").unwrap()),
        ],
    };
    let stride = |node| &clips.strides.iter().find(|(n, _)| *n == node).unwrap().1;
    let mut checked = 0;
    for (from, to) in [(walk, jog), (jog, walk)] {
        let (was, now) = (stride(from), stride(to));
        for (side, spans) in [&was.foot_down_l, &was.foot_down_r].into_iter().enumerate() {
            // Each plant just after it comes down, not one still down over the seam.
            let wraps = spans.iter().any(|span| span[1] < span[0]);
            for span in spans.iter().filter(|span| !(wraps && span[0] <= 1e-4)) {
                let t = span[0] + 0.02;
                for seed in 0..8 {
                    let at = clips.start_at(Some((from, t)), to, seed);
                    assert!(
                        now.lock(side, at) >= terra_motion::feet::FULL,
                        "{file}: {from:?} {t:.3} -> {to:?} {at:.3}: foot {side} is not planted"
                    );
                    let (sa, pa) = was.phase_in_loop(t).unwrap();
                    let (sb, pb) = now.phase_in_loop(at).unwrap();
                    assert!(
                        sa == sb && (pa - pb).abs() < 1e-3,
                        "{file}: {from:?} -> {to:?}"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(
        checked >= 16,
        "{file}: the walk and jog have too few plants: {checked}"
    );
    // From an idle, or into one, each starts at its own drawn point.
    assert_eq!(clips.start_at(Some((idle, 0.3)), jog, 5), draw(5, 99) * 2.0);
    assert_eq!(clips.start_at(None, walk, 5), draw(5, 99) * 2.0);
}
