use super::*;

fn defs() -> Vec<ButterflyDef> {
    terra_world::data::read(FILE)
}

const JULY_NOON: Now = Now {
    air_c: 21.0,
    hour: 13.0,
    rain_mm_h: 0.0,
};

fn meadow() -> Round {
    Round {
        open: 1.0,
        season_c: 13.0,
        ..Round::default()
    }
}

/// Every row's numbers make sense, and a row's model, where it has been made, is a
/// butterfly as the shader takes it: textured, its wings spread flat in the level, wider
/// than it is long and longer than it is thick. A row whose model is not made yet is
/// skipped in play with a line, so it is not a fault here.
#[test]
fn every_breed_has_sane_numbers_and_a_flat_model_if_it_has_one() {
    let defs = defs();
    assert!(defs.len() >= 5, "five breeds for the first realm");
    for def in &defs {
        let name = &def.name;
        for (least, most) in [def.span_m, def.over_m, def.season_c] {
            assert!(0.0 < least && least <= most, "{name}: {least} to {most}");
        }
        assert!(def.span_m.1 < 0.12, "{name} is {} m across", def.span_m.1);
        assert!(def.group.0 >= 1 && def.group.0 <= def.group.1, "{name}");
        assert!(
            def.group.1 <= 6,
            "{name}: a small group, never a cloud (D655)"
        );
        assert!(def.spread_m >= 1.0 && def.roams_m >= 1.0, "{name}");
        // A group keeps to about its own cell and its neighbours'.
        assert!(def.spread_m + def.roams_m <= CELL_M, "{name}");
        assert!(def.bout_s.0 > 0.2 && def.bout_s.1 > 0.1, "{name}");
        // Its beat is seen, not a blur or a slow flap; the shader's bout fades in and out
        // over 0.08 of it at each end.
        assert!((5.0..=20.0).contains(&def.beat_hz), "{name}");
        assert!(flight::beats(def.bout_s) > 0.2, "{name}");
        assert!(def.speed > 0.8 && def.speed < 3.0, "{name}");
        assert!(def.seen_m > 10.0 && def.a_ha > 0.0, "{name}");
        assert!(!def.hours.is_empty() && !def.realms.is_empty(), "{name}");
        assert!(def.woods.max(def.open).max(def.water) > 0.5, "{name}");
        let Ok(parts) = terra_world::models::read_model(&def.model) else {
            continue;
        };
        let part = (parts.iter().find(|part| part.textures.is_some()))
            .unwrap_or_else(|| panic!("{name}'s model has no textured part"));
        let (low, high) = part
            .positions
            .iter()
            .fold((Vec3::MAX, Vec3::MIN), |(low, high), at| {
                (low.min(*at), high.max(*at))
            });
        let size = high - low;
        assert!(
            size.x > size.z && size.z > size.y * 2.0,
            "{name}'s model is {size}: wings spread flat, the span along x (tools/props)"
        );
        // The body's axis, which the wings turn about: down the middle of the span, where
        // the model's weight is too (a butterfly is the same on both sides), and at a
        // height within the model.
        let axis = look::axis(&part.positions).unwrap();
        let weight =
            part.positions.iter().map(|at| at.x).sum::<f32>() / part.positions.len() as f32;
        assert!(
            (weight - axis.across).abs() < size.x * 0.05,
            "{name}'s body is {} m off the middle of its span",
            weight - axis.across
        );
        assert!((low.y..=high.y).contains(&axis.up), "{name}: {axis:?}");
    }
}

/// The axis of a butterfly made here, whose answer is known: two flat wings 6 cm across at
/// 4 mm up, either side of a body 4 mm wide that stands on the ground, set 1 cm off to one
/// side. The wings turn about the middle of the span at the wings' height, not the origin.
#[test]
fn the_wings_turn_about_the_bodys_axis_at_their_roots_height() {
    let mut points = Vec::new();
    for side in [-1.0, 1.0] {
        for out in [0.002, 0.004, 0.008, 0.02, 0.03] {
            for along in [-0.01, 0.0, 0.01] {
                points.push(Vec3::new(0.01 + side * out, 0.004, along));
            }
        }
        points.push(Vec3::new(0.01 + side * 0.002, 0.0, 0.0));
    }
    let axis = look::axis(&points).unwrap();
    assert!((axis.span - 0.06).abs() < 1e-6 && (axis.across - 0.01).abs() < 1e-6);
    assert!((axis.hinge - 0.003).abs() < 1e-6);
    assert!((axis.up - 0.004).abs() < 1e-6, "{axis:?}");
    assert_eq!(look::axis(&[]), None);
    assert_eq!(look::axis(&[Vec3::ZERO, Vec3::Y]), None);
}

/// A cell's group is the seed's: the same asked twice, other groups for another seed,
/// each breed about as often as its row says, and none of a realm that is not its own.
#[test]
fn a_cells_group_is_from_the_seed_by_the_breeds_shares() {
    let defs = defs();
    let cells = || (0..10_000).map(|i| IVec2::new(i % 100 - 50, i / 100 - 30));
    let mut held = vec![0; defs.len()];
    let mut differ = 0;
    for cell in cells() {
        let plan = plan(9, cell, &defs, "world");
        assert_eq!(plan, super::plan(9, cell, &defs, "world"));
        if plan != super::plan(10, cell, &defs, "world") {
            differ += 1;
        }
        if let Some(plan) = plan {
            let def = &defs[plan.kind];
            held[plan.kind] += 1;
            assert!((def.group.0 as usize..=def.group.1 as usize).contains(&plan.count));
            assert!(plan.at.min_element() >= 0.0 && plan.at.max_element() < 1.0);
        }
    }
    assert!(differ > 3000, "{differ} of 10,000 cells differ by the seed");
    for (def, got) in defs.iter().zip(&held) {
        let wanted = def.a_ha * CELL_HA * 10_000.0;
        let got = *got as f32;
        assert!(
            (got - wanted).abs() < wanted * 0.2 + 12.0,
            "{}: {got} groups, {wanted} by its row",
            def.name
        );
    }
    // Stretches with none: most cells are empty.
    let all: i32 = held.iter().sum();
    assert!(all < 5000, "{all} of 10,000 cells hold a group");
    // The far realm's kind, as data/realms.ron names it.
    assert!(cells().all(|cell| plan(9, cell, &defs, "industrial").is_none()));
}

/// Where a breed is found: by what the place is and how warm its growing season, as its
/// row says. The whites keep to the open; nothing is found where the season is too cold.
#[test]
fn a_place_suits_a_breed_by_what_it_is_and_its_season() {
    let defs = defs();
    let white = defs.iter().find(|def| def.name == "Small white").unwrap();
    let woods = Round {
        woods: 1.0,
        season_c: 13.0,
        ..Round::default()
    };
    assert_eq!(white.suits(&meadow()), white.open);
    assert_eq!(white.suits(&woods), white.woods);
    assert!(white.suits(&meadow()) > white.suits(&woods) * 2.0);
    let tundra = Round {
        season_c: 3.0,
        ..meadow()
    };
    for def in &defs {
        assert!(def.suits(&meadow()) > 0.5, "{} in a meadow", def.name);
        assert_eq!(def.suits(&tundra), 0.0, "{} on tundra", def.name);
    }
}

/// On a July afternoon every breed is out, all of it; none flies in January's air, at
/// night or in rain; and at the end of its hours a breed's groups go in one by one, by
/// each one's luck, not all at a stroke.
#[test]
fn none_fly_in_january_air_at_night_or_in_rain() {
    for def in defs() {
        let name = &def.name;
        assert_eq!(def.out(&JULY_NOON), 1.0, "{name} on a July afternoon");
        let january = Now {
            air_c: -2.0,
            ..JULY_NOON
        };
        assert_eq!(def.out(&january), 0.0, "{name} in January");
        // A mild winter noon is still too cold for any.
        let mild = Now {
            air_c: 9.0,
            ..JULY_NOON
        };
        assert_eq!(def.out(&mild), 0.0, "{name} at 9 °C");
        for hour in [22.0, 0.0, 3.0, 5.5] {
            let night = Now { hour, ..JULY_NOON };
            assert_eq!(def.out(&night), 0.0, "{name} at {hour} h");
        }
        let rain = Now {
            rain_mm_h: 1.0,
            ..JULY_NOON
        };
        assert_eq!(def.out(&rain), 0.0, "{name} in rain");
        let last = def.hours.iter().map(|span| span.1).fold(0.0, f32::max);
        let going_in = def.out(&Now {
            hour: last - 0.25,
            ..JULY_NOON
        });
        assert!((going_in - 0.5).abs() < 0.01, "{name}: {going_in}");
    }
}

/// However many groups the ground would hold, no more than the most stand at once, and a
/// group let go gives its room back.
#[test]
fn no_more_than_the_most_stand_at_once() {
    let defs = defs();
    let mut groups = Groups::default();
    let mut asked = 0;
    for i in 0..10_000 {
        let Some(plan) = plan(4, IVec2::new(i % 100, i / 100), &defs, "world") else {
            continue;
        };
        asked += plan.count;
        let taken = groups.take(plan.count);
        assert!(taken <= plan.count && groups.stand <= MOST);
    }
    assert!(asked > MOST * 10, "{asked} wanted: the cap was not reached");
    assert_eq!(groups.stand, MOST);
    assert_eq!(groups.take(3), 0);
    groups.stand -= 5;
    assert_eq!(groups.take(9), 5);
}

/// The rows' own count: on open ground that suits them all, dozens within sight and never
/// a cloud (D647, D655; the module's header says the numbers).
#[test]
fn a_meadow_holds_dozens_within_sight_not_a_cloud() {
    let defs = defs();
    let a_ha: f32 = defs
        .iter()
        .map(|def| def.a_ha * def.suits(&meadow()) * (def.group.0 + def.group.1) as f32 * 0.5)
        .sum();
    let within_sight = a_ha * std::f32::consts::PI * 45.0 * 45.0 / 10_000.0;
    assert!(
        (24.0..=MOST as f32).contains(&within_sight),
        "{within_sight} within 45 m, {a_ha} to a hectare"
    );
}
