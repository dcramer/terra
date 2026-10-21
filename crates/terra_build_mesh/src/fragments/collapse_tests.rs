//! How what hung comes down: the rules of [`super`], each failing when a fall goes over as
//! one rigid body, flies off where it should sink, or never heaps.

use super::super::tests::{chunks, cube};
use super::super::{Rubble, burst};
use super::*;
use terra_build::Shape as CubeShape;

/// A box of whole cubes, from `low` to `high` inclusive.
fn solid(low: IVec3, high: IVec3) -> Vec<(IVec3, Cube)> {
    (low.y..=high.y)
        .flat_map(|y| {
            (low.x..=high.x).flat_map(move |x| (low.z..=high.z).map(move |z| IVec3::new(x, y, z)))
        })
        .map(|at| (at, cube(CubeShape::Whole)))
        .collect()
}

/// What a fall does over `seconds`, as the game steps it: on flat ground and its own heap,
/// whole blocks breaking where they land hard and laying their rubble. Returns what is
/// left, the most any hinge went over before it let go, and the highest anything came to
/// rest.
fn come_down(mut parts: Vec<Fragment>, seconds: f32) -> (Vec<Fragment>, f32, f32) {
    let mut rubble = Rubble::default();
    let mut most_over = 0.0f32;
    let dt = 1.0 / 60.0;
    let solid = |_: Vec3| false;
    for step in 0..(seconds / dt) as usize {
        let now = step as f32 * dt;
        let mut born = Vec::new();
        let mut laid = Vec::new();
        for part in &mut parts {
            if let Some(hinge) = part.hinge {
                most_over = most_over.max(hinge.angle);
            }
            let floor = |at: Vec3| rubble.top(at, now).unwrap_or(0.0).max(0.0);
            let landing = part.step(dt, &floor, &solid);
            if landing.breaks {
                laid.push((*part, floor(part.at)));
                born.extend(part.shatter(Breaks::Chunks, 0.3));
                part.still = true;
                part.size = Vec3::ZERO;
            }
        }
        for (part, floor) in laid {
            rubble.lay(&part, floor, now);
        }
        parts.retain(|part| part.size != Vec3::ZERO);
        parts.extend(born);
    }
    let highest = parts
        .iter()
        .filter(|p| p.still)
        .map(|p| p.at.y)
        .fold(0.0f32, f32::max);
    (parts, most_over, highest)
}

#[test]
fn a_tower_whose_foot_gave_way_evenly_crumbles_where_it_stood_onto_its_heap() {
    // A tower 8 cubes a side (4 m) and 30 high, its whole foot destroyed.
    let tower = solid(IVec3::ZERO, IVec3::new(7, 29, 7));
    let foot = |at: IVec3| at.y == 0;
    assert_eq!(falls(&tower, &foot), Falls::InPlace);
    let mut parts = burst(&tower, &foot, &chunks, 1.0, 5000);
    collapse(&mut parts, &tower, &foot, &chunks);
    assert!(parts.iter().all(|p| p.hinge.is_none()), "it tipped");
    let (parts, _, highest) = come_down(parts, 8.0);
    // It came down where it stood: most of it within a tower's width of its foot, and
    // nothing as far out as it was tall, as a tower gone over would be.
    let middle = Vec3::new(2.0, 0.0, 2.0);
    let out = |p: &Fragment| (p.at - middle).with_y(0.0).length();
    let near = parts.iter().filter(|p| out(p) < 6.0).count();
    assert!(
        near * 10 > parts.len() * 7,
        "{near} of {} near its foot",
        parts.len()
    );
    let farthest = parts.iter().map(out).fold(0.0f32, f32::max);
    assert!(farthest < 12.0, "a piece went {farthest} m out");
    // And onto its own heap: what came last rests well over the ground.
    assert!(highest > 1.0, "the highest piece rests {highest} m up");
    // Its shell peeled out every way, not one.
    let quarter = |p: &&Fragment| (p.at.x > 2.0, p.at.z > 2.0);
    let mut sides = std::collections::BTreeSet::new();
    for p in parts.iter().filter(|p| out(p) > 3.0) {
        sides.insert(quarter(&p));
    }
    assert_eq!(sides.len(), 4, "it spread only {sides:?}");
}

#[test]
fn a_crumbling_towers_shell_leans_out_in_slabs_rather_than_spraying_its_blocks() {
    // The tower of the test above: its blocks as they start to come down.
    let tower = solid(IVec3::ZERO, IVec3::new(7, 29, 7));
    let foot = |at: IVec3| at.y == 0;
    let mut parts = burst(&tower, &foot, &chunks, 1.0, 5000);
    collapse(&mut parts, &tower, &foot, &chunks);
    let middle = Vec3::new(2.0, 0.0, 2.0);
    let whole: Vec<&Fragment> = parts.iter().filter(|p| p.shape == Shape::Whole).collect();
    let mut leaning = 0;
    for p in &whole {
        // Each turns outward about its face, as a slab folding out does: about a level
        // axis along the face, not tumbling every way.
        let way = (p.at - middle).with_y(0.0).normalize_or_zero();
        let spin = p.spin.length();
        assert!(
            p.spin.y.abs() <= 0.01 && p.spin.dot(way).abs() <= 0.01 + 0.05 * spin,
            "a block at {} spins {} where it leans out {way}",
            p.at,
            p.spin
        );
        if p.velocity.length() > 0.05 {
            leaning += 1;
        }
    }
    assert!(
        leaning * 4 > whole.len(),
        "only {leaning} of {} lean out",
        whole.len()
    );
    // And a block goes out no slower than the one under it in its column: the face goes as
    // one, the higher the further, never a spray of blocks each thrown its own way.
    for low in &whole {
        for high in &whole {
            let column = (high.at - low.at).with_y(0.0).length() < 0.05;
            if column && high.at.y > low.at.y + 0.1 {
                let (out_low, out_high) = (low.velocity.length(), high.velocity.length());
                assert!(
                    out_high + 1e-4 >= out_low,
                    "a block at {} goes {out_high} m/s, the one under it at {} {out_low}",
                    high.at,
                    low.at
                );
            }
        }
    }
}

#[test]
fn a_slender_wall_whose_foot_gave_way_on_one_side_tips_that_way_and_cracks_up_within_degrees() {
    // A wall 30 cubes long, 3 thick and 24 high along x, its outer course's foot destroyed.
    let wall = solid(IVec3::ZERO, IVec3::new(29, 23, 2));
    let foot = |at: IVec3| at.y == 0 && at.z == 2;
    let Falls::Tips { way } = falls(&wall, &foot) else {
        panic!("it did not tip");
    };
    assert!(way.z > 0.9, "it tips {way}");
    let mut parts = burst(&wall, &foot, &chunks, 1.0, 5000);
    collapse(&mut parts, &wall, &foot, &chunks);
    assert!(parts.iter().any(|p| p.hinge.is_some()));
    let (parts, most_over, _) = come_down(parts, 6.0);
    // Masonry cracks off what it hinges on within a few degrees (the rigid topple went over
    // as one to 17° at its top and 63° at its foot).
    assert!(
        most_over < 0.3,
        "a block went {most_over} rad over before it let go"
    );
    // Over the way it leaned, a good way out past its foot, and not back.
    let out = parts.iter().map(|p| p.at.z).fold(f32::MIN, f32::max);
    assert!(out > 4.0, "the farthest piece is {out} m out");
    let mean = parts.iter().map(|p| p.at.z).sum::<f32>() / parts.len() as f32;
    assert!(mean > 2.0, "it came down {mean} m out on the whole");
    let back = parts.iter().map(|p| p.at.z).fold(f32::MAX, f32::min);
    assert!(back > -4.5, "a piece went {back} m the wrong way");
    assert!(parts.iter().all(|p| p.hinge.is_none()), "some never let go");
}

#[test]
fn tipping_blocks_part_in_the_air_and_tumble_as_they_were_turning() {
    let wall = solid(IVec3::ZERO, IVec3::new(29, 23, 2));
    let foot = |at: IVec3| at.y == 0 && at.z == 2;
    let mut parts = burst(&wall, &foot, &chunks, 1.0, 5000);
    collapse(&mut parts, &wall, &foot, &chunks);
    let floor = |_: Vec3| 0.0;
    let open = |_: Vec3| false;
    // The two highest blocks side by side: as one body they would keep their distance.
    let mut top: Vec<usize> = (0..parts.len()).collect();
    top.sort_by(|&a, &b| parts[b].at.y.total_cmp(&parts[a].at.y));
    let (a, b) = (
        top[0],
        top[1..]
            .iter()
            .copied()
            .find(|&b| (parts[b].at - parts[top[0]].at).length() < 2.5)
            .expect("a block beside the top"),
    );
    let apart = (parts[a].at - parts[b].at).length();
    let mut changed = 0.0f32;
    for _ in 0..90 {
        for part in &mut parts {
            part.step(1.0 / 60.0, &floor, &open);
        }
        if parts[a].hinge.is_none() && parts[b].hinge.is_none() && parts[a].at.y > 2.0 {
            changed = changed.max(((parts[a].at - parts[b].at).length() - apart).abs());
            // Turning over the way the whole went, as it was.
            assert!(parts[a].spin.dot(Vec3::X).abs() > 0.05, "{}", parts[a].spin);
        }
    }
    assert!(
        changed > 0.1,
        "the top blocks stayed {changed} m from where they were apart"
    );
}

#[test]
fn a_squat_block_or_one_cut_off_above_slumps_where_it_stood() {
    // As wide as it is high, its foot gone on one side: it slumps.
    let block = solid(IVec3::ZERO, IVec3::new(11, 11, 11));
    assert_eq!(
        falls(&block, &|at: IVec3| at.y == 0 && at.z > 8),
        Falls::InPlace
    );
    // A wall whose support went nowhere under it (cut off at its ends): no side to tip to.
    let wall = solid(IVec3::ZERO, IVec3::new(29, 23, 2));
    assert_eq!(falls(&wall, &|at: IVec3| at.x == 0), Falls::InPlace);
    // A small fall never tips.
    let post = solid(IVec3::ZERO, IVec3::new(0, 20, 0));
    assert_eq!(falls(&post, &|at: IVec3| at.y == 0), Falls::InPlace);
}

#[test]
fn a_big_fall_of_stone_comes_down_in_bigger_blocks_than_one_of_plaster() {
    let wall = solid(IVec3::ZERO, IVec3::new(29, 15, 2));
    let mean = |breaks: &dyn Fn(terra_build::cubes::Kind) -> Breaks| {
        let parts = burst(&wall, &|_| false, breaks, 1.0, 5000);
        let whole: Vec<_> = parts.iter().filter(|p| p.shape == Shape::Whole).collect();
        whole
            .iter()
            .map(|p| p.size.x * p.size.y * p.size.z)
            .sum::<f32>()
            / whole.len() as f32
    };
    let stone = mean(&chunks);
    let plaster = mean(&|_| Breaks::Bits);
    assert!(
        stone > plaster * 2.0,
        "stone {stone} m³ a block, plaster {plaster}"
    );
}

#[test]
fn a_block_landing_on_its_edge_tumbles_over_it_and_one_landing_flat_does_not() {
    let wall = solid(IVec3::ZERO, IVec3::new(1, 1, 1));
    let block = burst(&wall, &|_| false, &chunks, 1.0, 10)[0];
    let land = |turn: Quat| {
        let mut block = Fragment {
            at: Vec3::Y * 3.0,
            turn,
            velocity: Vec3::ZERO,
            spin: Vec3::ZERO,
            wait: 0.0,
            ..block
        };
        while block.step(1.0 / 60.0, &|_| 0.0, &|_| false).hit.is_none() {}
        block.spin
    };
    let flat = land(Quat::IDENTITY);
    let edge = land(Quat::from_rotation_z(0.5));
    assert!(flat.length() < 0.5, "flat it spins at {flat}");
    // Over the edge it hit, about the edge's own axis.
    assert!(edge.z.abs() > 2.0, "on its edge it spins at {edge}");
}

#[test]
fn a_crumbling_wall_comes_down_in_sections_that_hold_together_until_they_land() {
    // A wall 24 cubes long (12 m), 3 thick and 30 high, its foot destroyed.
    let wall = solid(IVec3::ZERO, IVec3::new(23, 29, 2));
    let foot = |at: IVec3| at.y == 0;
    let mut parts = burst(&wall, &foot, &chunks, 1.0, 5000);
    collapse(&mut parts, &wall, &foot, &chunks);
    let whole: Vec<usize> = (0..parts.len())
        .filter(|&i| parts[i].shape == Shape::Whole)
        .collect();
    // Most of its blocks go in sections of several, a few sections in all.
    let riding = whole.iter().filter(|&&i| parts[i].body.is_some()).count();
    assert!(
        riding * 10 >= whole.len() * 8,
        "{riding} of {} in sections",
        whole.len()
    );
    let mut sections: Vec<Vec3> = Vec::new();
    for &i in &whole {
        if let Some(body) = parts[i].body
            && !sections.contains(&body.middle)
        {
            sections.push(body.middle);
        }
    }
    assert!(
        sections.len() >= 3 && sections.len() * 4 <= riding,
        "{} sections of {riding} blocks",
        sections.len()
    );
    // Falling, two blocks of a section keep their places to one another, as one body.
    let same =
        |a: usize, b: usize| parts[a].body.map(|s| s.middle) == parts[b].body.map(|s| s.middle);
    let pairs: Vec<(usize, usize, f32)> = whole
        .iter()
        .flat_map(|&a| whole.iter().map(move |&b| (a, b)))
        .filter(|&(a, b)| a < b && parts[a].body.is_some() && same(a, b))
        .map(|(a, b)| (a, b, parts[a].at.distance(parts[b].at)))
        .collect();
    assert!(!pairs.is_empty());
    let dt = 1.0 / 60.0;
    let open = |_: Vec3| false;
    for _ in 0..30 {
        for part in &mut parts {
            part.step(dt, &|_| 0.0, &open);
        }
    }
    let kept: Vec<_> = pairs
        .iter()
        .filter(|&&(a, b, _)| parts[a].body.is_some() && parts[b].body.is_some())
        .collect();
    assert!(kept.len() * 2 > pairs.len(), "sections parted in the air");
    for &&(a, b, was) in &kept {
        let now = parts[a].at.distance(parts[b].at);
        assert!(
            (now - was).abs() < 0.01,
            "a section's blocks moved {was} -> {now} m apart"
        );
    }
    // Each lets go as it lands: by the time the top would have reached the ground, none
    // goes with its section any more, and most broke where they landed.
    let mut broke = 0;
    for _ in 0..(3.0 / dt) as usize {
        for part in &mut parts {
            if part.step(dt, &|_| 0.0, &open).breaks {
                broke += 1;
            }
        }
    }
    assert!(
        parts.iter().all(|p| p.body.is_none()),
        "a block still rides its section"
    );
    assert!(
        broke * 2 > whole.len(),
        "{broke} of {} blocks broke landing",
        whole.len()
    );
}
