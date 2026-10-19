//! Building's checks over the registry's and the fuzz pass's scenes and the roof-to-wall rows:
//! the layout's and the drawing's own tests cannot lay these fixtures, which stand above
//! them (D407), so they are here.

use glam::Vec3;
use terra_build::kit::{Kit, Kits};
use terra_build::layout::Piece;
use terra_build::layout::roofs::Roofs;
use terra_build::roofer::Style;
use terra_build::testing;
use terra_build::{Base, CELL_M};
use terra_build_mesh::relief::Maps;

/// The shipped kits the drawing's tests lay, by their files' names.
const SHIPPED: &str = "logs hewn stone earth wattle shakes slate thatch greystone";

/// The shipped kits, dressed, as the drawing's tests read them.
fn shipped() -> Kits {
    let assets = testing::assets();
    let read = |path: &str| {
        std::fs::read_to_string(assets.join(path)).map_err(|why| format!("{path}: {why}"))
    };
    let kits = SHIPPED.split(' ').map(|name| {
        Kit::read(&format!("data/kits/{name}.ron"), &read)
            .unwrap_or_else(|why| panic!("{name}: {why}"))
    });
    Kits(kits.collect())
}

/// The shipped coursed materials and the roof over them, as the game reads them.
fn coursed() -> Kits {
    Kits(
        ["logs.ron", "hewn.ron", "shakes.ron"]
            .iter()
            .map(|name| testing::shipped(name))
            .collect(),
    )
}
// ---- from crates/terra_build/src/sections/tests.rs

// Slow, about a minute: a sweep.
#[test]
fn a_rectangle_is_drawn_closed_in_every_state_it_is_drawn_in() {
    // The rectangle under a gable (the registry's states, docs/building-voxels.md, gap 8)
    // in the baseline's logs, hewn timber and stone, and each two mixed: as a plan, its
    // walls going up, its roof going on, damaged, and after a wall's lowest course is
    // knocked out. What is in is closed on its own: a gable's courses go in with the roof's
    // first stage, a door's posts with their first cube, a lintel left over air stops at its
    // cube, and nothing the mesh checker finds.
    use crate::registry::{Family, Materials, registry};
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let materials = Materials::read(&assets, true).unwrap();
    let kits = &materials.kits;
    let maps = Maps::default();
    let mut found = Vec::new();
    let mut states = 0;
    for scene in registry(&materials) {
        if scene.family != Family::States {
            continue;
        }
        let (name, base) = (&scene.name, scene.base());
        states += 1;
        let report = terra_build_mesh::inspect::inspect(&base, kits, &maps);
        found.extend(
            report
                .findings
                .iter()
                .map(|f| format!("{name}: {:?} {:?} {}", f.fault, f.detail, f.what)),
        );
    }
    assert_eq!(states, 6 * 5, "every state of each of six rectangles");
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn verges_meeting_at_an_apex_take_no_cap_there() {
    // Every roof kind over a gable: each verge's top meets the other verge and the ridge at
    // the apex, under the ridge's own cap; capped there too, the two verges' caps stood out
    // past the apex through each other. Their feet at the eaves stop in the open, capped.
    use crate::registry::{Family, Materials, registry};
    let materials = Materials {
        kits: shipped(),
        names: Default::default(),
        village: None,
    };
    let kits = &materials.kits;
    for scene in registry(&materials) {
        if scene.family != Family::Plans || !scene.name.ends_with(": rectangle, gable roof") {
            continue;
        }
        let (name, base) = (&scene.name, scene.base());
        let top =
            base.cubes().map(|(at, _)| at.y + 1).max().unwrap_or(0) as f32 * terra_build::CELL_M;
        let mut verges = 0;
        for kit in &kits.0 {
            for piece in terra_build::layout::lay_among(&base, kit, kits).pieces {
                let Piece::Roof {
                    part: terra_build::layout::roofs::RoofPart::Verge,
                    from,
                    to,
                    capped,
                    ..
                } = piece
                else {
                    continue;
                };
                verges += 1;
                for (end, capped) in [from, to].into_iter().zip(capped) {
                    let at_apex = end.metres().y > top - 0.6;
                    assert_eq!(
                        capped,
                        !at_apex,
                        "{name}: a verge's end at {}",
                        end.metres()
                    );
                }
            }
        }
        assert!(verges >= 4, "{name}: {verges} verges");
    }
}

#[test]
fn a_roof_falling_to_a_coursed_wall_is_closed_to_it_at_the_gables() {
    // A lean-to falling to a taller wall (`roof_walls`, a roof falling to a wall), in each
    // shipped coursed material: at each gable's end under the roof's lowest cube, where
    // the gable's course is left out under the roof and meets the blocking along the
    // eave, lines in through the gable a centimetre apart, from the wall's top to the
    // roof's underside, all meet something. Hewn beams' course stopped 4 cm short of the
    // eave's blocking there (docs/building-voxels.md, a roof falling to a wall).
    let kits = coursed();
    let shakes = kits.0[2].kind();
    for wall in [kits.0[0].kind(), kits.0[1].kind()] {
        let (_, ops) = crate::roof_walls::rows(wall, shakes, None)
            .into_iter()
            .find(|(row, _)| row == "a roof falling to a wall")
            .expect("the row");
        let mut base = Base::default();
        for op in ops {
            base.apply(op);
        }
        let lines = terra_build_mesh::inspect::Lines::of(&base, &kits);
        let roofs = Roofs::whole(&base, &kits, terra_build::layout::Scope::ALL);
        let eave = terra_build::STOREY_CUBES as f32 * CELL_M;
        for (x, out) in [(0, -1.0f32), (9, 1.0)] {
            let face = (x as f32 + 0.5 + out * 0.5) * CELL_M;
            let middle = (x as f32 + 0.5) * CELL_M;
            for k in 0..50 {
                let z = 5.0 * CELL_M + 0.005 + k as f32 * 0.01;
                let under = roofs.under_over(middle, z, eave).expect("the roof");
                let mut y = eave + 0.01;
                while y < under - 0.03 {
                    let from = Vec3::new(face + out * 0.3, y, z);
                    assert!(
                        lines.meets(from, Vec3::X * -out, 0.3 + CELL_M + 0.02),
                        "kind {}: open under the roof at ({face}, {y:.3}, {z:.3})",
                        wall.0
                    );
                    y += 0.01;
                }
            }
        }
    }
}

#[test] // Slow: minutes.
fn a_roof_over_a_rooms_flat_top_is_closed_to_its_walls_whichever_way_it_rises() {
    // Found by `cargo xtask fuzz` (seeds 28, 110; docs/building-voxels.md, gap 12): a log
    // room laid with its flat top, roofed over it by a lean-to rising north to south and
    // a hip whose ridge runs east and west. The walls along the north and south under the
    // eaves were left to a crossing row's blocking where the room's flat top stood beside
    // them, which is a floor and lays none, and sky showed under the roof.
    use crate::fuzz::{Element::*, build};
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let materials = crate::registry::Materials::read(&assets, true).expect("the baseline");
    let kind = |name: &str| {
        materials
            .names
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(&k, _)| k)
            .expect(name)
    };
    let maps = terra_build_mesh::relief::Maps::worst(&materials.kits, false);
    for elements in [
        vec![
            Room {
                min: [1, 4],
                max: [8, 7],
                storey: 0,
                kind: kind("logs"),
            },
            Roof {
                tile: [4, 5],
                storey: 0,
                style: Style::LeanTo,
                along_x: Some(false),
                kind: kind("shakes"),
            },
        ],
        vec![
            Room {
                min: [1, 1],
                max: [7, 7],
                storey: 0,
                kind: kind("logs"),
            },
            Roof {
                tile: [4, 4],
                storey: 0,
                style: Style::Hip,
                along_x: Some(true),
                kind: kind("shakes"),
            },
        ],
    ] {
        let base = build(&materials.kits, &elements);
        let report = terra_build_mesh::inspect::inspect(&base, &materials.kits, &maps);
        for fault in [
            terra_build_mesh::inspect::Fault::SeeThrough,
            terra_build_mesh::inspect::Fault::UnderRoof,
        ] {
            assert_eq!(
                report.count(fault),
                0,
                "{elements:?}: {:?}",
                report.findings
            );
        }
    }
}
// ---- from crates/terra_build/src/layout/roofs/tests.rs

#[test]
fn a_lone_corner_square_carries_no_crease_out_past_its_edges() {
    // Found by `cargo xtask fuzz` (seeds 20, 21, 42, 95, 118, 159; docs/building-voxels.md,
    // gap 11): a corner cube of a roof kind laid by hand where no roof is. Round its corners
    // its two ramps ran on past it, an inner corner's valley up and out past its high corner
    // and down past its low one, an outer corner's hip up past its apex, and the beam and
    // cap under them stood out past the roof's edge.
    use crate::fuzz::{Element::Blocks, build};
    use terra_build::{Facing, Shape};
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let materials = crate::registry::Materials::read(&assets, true).expect("the baseline");
    let kind = |name: &str| {
        materials
            .names
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(&k, _)| k)
            .expect(name)
    };
    let maps = terra_build_mesh::relief::Maps::worst(&materials.kits, false);
    for (at, shape, roof) in [
        ([4, 0, 8], Shape::InnerCorner(Facing::North), "shakes"),
        ([-1, 0, 11], Shape::InnerCorner(Facing::West), "slate"),
        ([2, -1, 11], Shape::OuterCorner(Facing::South), "shakes"),
        ([5, 0, 7], Shape::OuterCorner(Facing::East), "slate"),
    ] {
        let elements = [Blocks {
            at,
            size: 1,
            shape,
            kind: kind(roof),
        }];
        let base = build(&materials.kits, &elements);
        let report = terra_build_mesh::inspect::inspect(&base, &materials.kits, &maps);
        assert!(
            report.findings.is_empty(),
            "{shape:?} in {roof}: {:?}",
            report.findings
        );
    }
}

#[test] // Slow: minutes.
fn an_eave_over_a_wall_running_on_from_the_room_is_flashed_against_it() {
    // Found by `cargo xtask fuzz` (seeds 59, 135; docs/building-voxels.md, gap 18): a hip
    // over a log room, a wall of logs running on from one of its corners. The wall's top is
    // at the eave there, so the eave's edge pieces stood on it, out past where the roof's
    // eaves, stopped by the wall, reach.
    use crate::fuzz::{Element::*, build};
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let materials = crate::registry::Materials::read(&assets, true).expect("the baseline");
    let kind = |name: &str| {
        materials
            .names
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(&k, _)| k)
            .expect(name)
    };
    let maps = terra_build_mesh::relief::Maps::worst(&materials.kits, false);
    let cases = [
        ([2, 4], [7, 9], [7, 9], [11, 9], "slate"),
        ([0, 2], [6, 7], [5, 3], [7, 3], "shakes"),
    ];
    for (min, max, from, to, roof) in cases {
        let elements = [
            Room {
                min,
                max,
                storey: 0,
                kind: kind("logs"),
            },
            Wall {
                from,
                to,
                storey: 0,
                kind: kind("logs"),
            },
            Roof {
                tile: [(min[0] + max[0]) / 2, (min[1] + max[1]) / 2],
                storey: 0,
                style: Style::Hip,
                along_x: None,
                kind: kind(roof),
            },
        ];
        let base = build(&materials.kits, &elements);
        let report = terra_build_mesh::inspect::inspect(&base, &materials.kits, &maps);
        let out = report.count(terra_build_mesh::inspect::Fault::PokesOut);
        assert_eq!(out, 0, "{roof}: {:?}", report.findings);
    }
}

// ---- from terra_town's village tests (it draws, which the town cannot)

// Slow, a minute or two: lays and draws one of each sort of the village's buildings.
#[test]
fn no_two_faces_of_the_villages_buildings_fight_for_the_same_pixels() {
    // One of each sort of the village's buildings, in the settlement's own materials as
    // the game reads them, modelled pieces and all: none of their near meshes' faces lie
    // in one plane over one another (`inspect::fights`, the checker's measure). The
    // farmstead's lintels at a wall's end, the outside stair's stringers and the deck
    // beside it, a doorway's posts on a footing, the lookout's rail, ridge caps cut to
    // length and a verge under a higher roof's eave all did.
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let read = |path: &str| {
        std::fs::read_to_string(assets.join(path)).map_err(|why| format!("{path}: {why}"))
    };
    let mut paths: Vec<_> = std::fs::read_dir(assets.join("data/kits"))
        .expect("the kits")
        .flatten()
        .map(|entry| entry.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    paths.sort();
    let mut names = std::collections::BTreeMap::new();
    let mut kits = Vec::new();
    for path in paths {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let kit = terra_build::kit::Kit::read(
            &format!("{}/{stem}.ron", terra_build::kit::MATERIALS),
            &read,
        )
        .unwrap();
        names.insert(stem, kit.kind());
        kits.push(kit);
    }
    let kits = terra_build::kit::Kits(kits);
    let palette =
        terra_town::village::Palette::read(&read, &|name| names.get(name).copied(), &kits).unwrap();
    let maps = terra_build_mesh::relief::Maps::default();
    let progress = terra_build_mesh::sections::Progress::Whole;
    terra_gen::par::map(
        &terra_town::village::each_sort(&palette),
        |(building, base)| {
            let whole = terra_build_mesh::sections::Whole::lay(base, &kits);
            let mut meshes = Vec::new();
            for at in terra_build_mesh::sections::drawable(base) {
                let section = terra_build_mesh::sections::Section {
                    base,
                    kits: &kits,
                    maps: &maps,
                    at: glam::IVec3::from_array(at),
                    progress: &progress,
                };
                let laid = terra_build_mesh::sections::lay(&section, &whole);
                let near = terra_build_mesh::sections::draw(
                    &section,
                    &laid,
                    terra_build_mesh::sections::Reach::Near,
                );
                for (&(kind, material), surface) in &near.near {
                    meshes.push((
                        format!("{kind:?} {material:?}"),
                        Some((kind, material)),
                        surface.clone(),
                    ));
                }
                meshes.push(("plain".to_string(), None, near.plain));
            }
            let fights = terra_build_mesh::inspect::fights(
                meshes.iter().map(|(n, k, s)| (n.clone(), *k, s)),
            );
            assert!(fights.is_empty(), "{}: {fights:?}", building.name);
        },
    );
}
