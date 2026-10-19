//! The layout's tests that draw what it lays or look at it drawn: the layout's own tests
//! cannot name the drawing, which stands above it (D407), so they are here.

use glam::Vec3;
use terra_build::kit::{Dress, Kit, Kits, Profile};
use terra_build::layout::stairs::find;
use terra_build::layout::{Dir, Piece, Role, lay};
use terra_build::pieces::Material;
use terra_build::{Base, BuildOp, Cube, Fill, Shape};

// ---- from crates/terra_build/src/layout/stairs.rs

/// A kit dressing a flight's noses and its posts in hewn members.
fn stairs_kit() -> Kit {
    let mut kit = Kit::default();
    let member = |scale: f32| Dress::Member {
        profile: Profile::Hewn,
        scale,
        material: Material::Log,
    };
    kit.dress.roles = [(Role::Nose, member(1.0)), (Role::Jamb, member(0.6))]
        .into_iter()
        .collect();
    kit
}

#[test]
fn a_stringers_foot_is_cut_plumb_at_the_flights_front() {
    // Three stairs two cubes wide rising north, each cut to two steps on the cubes under
    // it: the lowest nose so near the floor the stringer starts at the flight's front,
    // and its lower edge, its depth down the slope from its line, is cut plumb there
    // rather than standing out in front over whatever is below.
    let mut base = Base::default();
    for step in 0..3 {
        if step > 0 {
            base.apply(BuildOp::Fill {
                min: [0, 0, -step],
                max: [1, step - 1, -step],
                cube: Some(Cube::new(stairs_kit().kind(), Fill::Solid)),
            });
        }
        base.apply(BuildOp::Fill {
            min: [0, step, -step],
            max: [1, step, -step],
            cube: Some(Cube {
                shape: terra_build::Shape::Stair(terra_build::Facing::North),
                ..Cube::new(stairs_kit().kind(), Fill::Solid)
            }),
        });
    }
    let layout = lay(&base, &stairs_kit());
    let mut stringers = 0;
    for piece in layout
        .pieces
        .iter()
        .filter(|p| matches!(p, Piece::Plank { from, to, .. } if from.0[1] != to.0[1]))
    {
        stringers += 1;
        let mut surfaces = crate::mesh::Surfaces::default();
        crate::draw::draw(piece, &stairs_kit(), None, &mut surfaces);
        for p in surfaces.0.values().flat_map(|s| &s.positions) {
            assert!(
                p.z <= terra_build::CELL_M + 1e-3,
                "{p} out in front of the flight"
            );
        }
    }
    assert_eq!(stringers, 2);
}

#[test]
fn a_flight_of_timber_is_laid_as_open_stairs_on_stringers_and_posts() {
    // Four steps two cubes wide, rising north, each a cube higher.
    let mut base = Base::default();
    for step in 0..4 {
        base.apply(BuildOp::Fill {
            min: [0, 0, -step],
            max: [1, step, -step],
            cube: Some(Cube::new(stairs_kit().kind(), Fill::Solid)),
        });
    }
    let kits = Kits(vec![stairs_kit()]);
    let steps = find(&base, &stairs_kit(), &kits, terra_build::layout::Scope::ALL);
    assert_eq!(steps.len(), 8, "{steps:?}");
    assert!(steps.values().all(|step| step.out == Dir::South));
    let layout = lay(&base, &stairs_kit());
    let logs: Vec<&Piece> = layout
        .pieces
        .iter()
        .filter(|p| matches!(p, Piece::Log { .. }))
        .collect();
    // No log lies across the flight: nothing under the treads but posts.
    for piece in &logs {
        let Piece::Log { from, to, role, .. } = piece else {
            unreachable!()
        };
        assert_eq!(*role, Some(Role::Jamb), "{piece:?}");
        assert!(
            from.0[0] == to.0[0] && from.0[2] == to.0[2],
            "{piece:?} is no post"
        );
    }
    // A stringer up each side, rising from the floor at the flight's foot.
    let stringers: Vec<_> = layout
        .pieces
        .iter()
        .filter_map(|p| match p {
            Piece::Plank { from, to, .. } if from.0[1] != to.0[1] => {
                Some((from.metres(), to.metres()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(stringers.len(), 2);
    for (from, to) in &stringers {
        let (low, high) = if from.y < to.y {
            (from, to)
        } else {
            (to, from)
        };
        assert!(low.y < 0.2, "{low}");
        assert!(high.y > 1.5, "{high}");
    }
    // Drawn, nothing of a stringer stands over the top step's cube, as its head is cut
    // level with what it climbs to; nor out of the flight's side further than a trim.
    for piece in layout
        .pieces
        .iter()
        .filter(|p| matches!(p, Piece::Plank { from, to, .. } if from.0[1] != to.0[1]))
    {
        let mut surfaces = crate::mesh::Surfaces::default();
        crate::draw::draw(piece, &stairs_kit(), None, &mut surfaces);
        for surface in surfaces.0.values() {
            for p in &surface.positions {
                assert!(
                    p.y <= 4.0 * terra_build::CELL_M + 1e-3,
                    "{p} over the top step"
                );
                assert!(
                    p.x > -0.0625 && p.x < 2.0 * terra_build::CELL_M + 0.0625,
                    "{p}"
                );
            }
        }
    }
    // Posts under them where the flight stands high: upright, from the floor.
    let posts = logs
        .iter()
        .filter(|p| matches!(p, Piece::Log { from, to, .. } if from.0[0] == to.0[0] && from.0[2] == to.0[2]))
        .count();
    assert!(posts >= 2, "{posts} posts");
    // A tread a step, as wide as the flight.
    let treads: Vec<_> = layout
        .pieces
        .iter()
        .filter_map(|p| match p {
            Piece::Plank { from, to, .. } if from.0[1] == to.0[1] => {
                Some((from.metres(), to.metres()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(treads.len(), 4);
    for (from, to) in treads {
        assert!((from.x.min(to.x) - 0.0).abs() < 1e-3 && (from.x.max(to.x) - 1.0).abs() < 1e-3);
    }
}

// ---- from crates/terra_build/src/layout/floors.rs

#[test]
fn a_lookouts_deck_is_closed_under_its_rail_and_beside_its_corner_posts() {
    // A deck on posts at its corners with a rail of its own beams along one edge, the
    // posts running up into the rail's corners, as the village's lookout: the rail's
    // beams are narrower than its cubes, so the deck's boards run on under them, and the
    // deck's rims, ending at the corner posts, run on into them. Lines up through the
    // deck under the rail, and in through its side beside a corner post, all meet
    // something; slots 4 to 12 cm wide were open there.
    let hewn = terra_build::testing::shipped("hewn.ron");
    let kits = Kits(vec![hewn.clone()]);
    let cube = |shape: Shape| {
        Some(terra_build::Cube {
            shape,
            ..terra_build::Cube::new(hewn.kind(), Fill::Solid)
        })
    };
    let mut base = Base::default();
    let mut fill = |min: [i32; 3], max: [i32; 3], shape: Shape| {
        base.apply(BuildOp::Fill {
            min,
            max,
            cube: cube(shape),
        })
    };
    for [x, z] in [[0, 0], [5, 0], [0, 3], [5, 3]] {
        fill([x, 0, z], [x, 0, z], Shape::Post);
    }
    fill([0, 1, 0], [5, 1, 3], Shape::Whole);
    fill([0, 2, 0], [5, 2, 0], Shape::Whole);
    let lines = crate::inspect::Lines::of(&base, &kits);
    // Up through the deck under the rail, its cubes but the corners'.
    for i in 0..=200 {
        let x = 0.55 + i as f32 * 0.0095;
        for j in 0..=50 {
            let z = 0.0 + j as f32 * 0.01;
            assert!(
                lines.meets(Vec3::new(x, 0.3, z), Vec3::Y, 2.0),
                "open under the rail at ({x:.3}, {z:.3})"
            );
        }
    }
    // In through the deck's side beside the corner post, under its boards, as closed as
    // the rim beside it.
    let heights: Vec<f32> = (0..=45)
        .map(|j| 0.5 + j as f32 * 0.01)
        .filter(|&y| lines.meets(Vec3::new(1.25, y, -0.3), Vec3::Z, 0.6))
        .collect();
    assert!(heights.len() > 5, "the deck's rim");
    for i in 0..=14 {
        let x = 0.36 + i as f32 * 0.01;
        for &y in &heights {
            assert!(
                lines.meets(Vec3::new(x, y, -0.3), Vec3::Z, 0.6),
                "open beside the corner post at ({x:.2}, {y:.2})"
            );
        }
    }
}

// ---- from crates/terra_build/src/layout/runs.rs

/// The shipped coursed materials and the roof over them, as the game reads them.
fn coursed() -> Kits {
    Kits(
        ["logs.ron", "hewn.ron", "shakes.ron"]
            .iter()
            .map(|name| terra_build::testing::shipped(name))
            .collect(),
    )
}

#[test]
fn a_thick_walls_decked_top_is_closed_to_a_pier_rising_from_it() {
    // A wall two cubes thick and four long, its top decked, a pier rising from its west
    // end, standing in the ground, as the QA cabin's garden wall: the pier's members are
    // narrower than its cube, so the deck's rim runs on into it. Lines through the wall's
    // thickness beside the pier, from the course under the deck to its boards, all meet
    // something; a slot 10 to 15 cm wide was open there, the ground showing through.
    let kits = coursed();
    for kit in &kits.0[..2] {
        let mut base = Base::default();
        let cube = Some(Cube::new(kit.kind(), Fill::Solid));
        for (min, max) in [([0, -2, 0], [3, -1, 1]), ([0, 0, 0], [0, 1, 1])] {
            base.apply(BuildOp::Fill { min, max, cube });
        }
        let ring = base.ground_ring();
        base.sample_ground(ring.into_iter().map(|column| terra_build::Sample {
            column,
            height_cm: -60,
        }));
        let lines = crate::inspect::Lines::of(&base, &kits);
        // As closed as the rim along the deck's own cubes is at each height: the seams
        // between its beams are the deck's, not this junction's.
        let heights: Vec<f32> = (0..=36)
            .map(|j| -0.45 + j as f32 * 0.01)
            .filter(|&y| lines.meets(Vec3::new(0.75, y, -0.3), Vec3::Z, 1.6))
            .collect();
        assert!(heights.len() > 30, "the deck's rim");
        for i in 0..=30 {
            let x = 0.25 + i as f32 * 0.01;
            for &y in &heights {
                assert!(
                    lines.meets(Vec3::new(x, y, -0.3), Vec3::Z, 1.6),
                    "kind {}: open beside the pier at ({x:.2}, {y:.2})",
                    kit.id
                );
            }
        }
        // A corner post over the pier's corners, where the kit dresses them, runs down
        // past the deck beside it to the course under the deck, and at its free end, over
        // the courses' ends, on to the ground, not stopping over the air where the ground
        // falls under the cubes it stands in.
        if kit.dress.member(Role::Arris).is_some() {
            let laid = lay(&base, kit);
            let feet: Vec<Vec3> = laid
                .pieces
                .iter()
                .filter_map(|piece| match piece {
                    Piece::Log {
                        from,
                        role: Some(Role::Arris),
                        ..
                    } => Some(from.metres()),
                    _ => None,
                })
                .collect();
            assert_eq!(feet.len(), 4, "the pier's corner posts");
            for foot in feet {
                let ground = if foot.x < 0.25 { -0.6 } else { -0.5 };
                assert!(foot.y <= ground, "a corner post's foot at {foot}");
            }
        }
    }
}
