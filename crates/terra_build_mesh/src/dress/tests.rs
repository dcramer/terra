use super::corners::STONE_SHARE;
use super::masks::{Noise, Rained, masked};
use super::profiles::Sweep;
use super::*;

#[test]
fn a_corner_stone_is_cut_from_inside_one_of_the_walls_own_stones_at_its_size() {
    use terra_build::kit::Dress;
    // A wall's height map of square stones 64 texels across in joints 8 wide.
    const SIDE: usize = 512;
    let heights = (0..SIDE * SIDE)
        .map(|i| {
            if (i % SIDE) % 64 < 8 || (i / SIDE) % 64 < 8 {
                20
            } else {
                220
            }
        })
        .collect();
    let wall = crate::relief::Relief::new(SIDE, SIDE, heights).unwrap();
    let mut kit = Kit::default();
    kit.textures.insert(
        Material::Masonry,
        terra_build::kit::Texture {
            path: "wall".into(),
            metres: [2.0, 2.0],
            depth: 0.02,
            reflectance: terra_build::kit::matte(),
            strips: None,
            cells: 1,
            worn: None,
        },
    );
    let (proud, joint, long) = (0.018, 0.007, [0.42, 0.23]);
    kit.dress.roles.insert(
        layout::Role::Arris,
        Dress::Corners {
            long: vec![long],
            short: Vec::new(),
            proud,
            bevel: 0.016,
            joint,
        },
    );
    let piece = Piece::Corner {
        cube: IVec3::ZERO,
        role: layout::Role::Arris,
        faces: [Dir::North, Dir::West],
        courses: [true, false],
        shared: [false, false],
        seed: 7,
    };
    let mut surfaces = Surfaces::default();
    draw(&piece, &kit, Some(&wall), &mut surfaces);
    let stone = &surfaces.0[&Material::Masonry];
    assert!(!stone.positions.is_empty(), "in the wall's own material");
    assert!(
        stone.triangle_count() <= crate::checker::CORNER_STONE,
        "{}",
        stone.triangle_count()
    );
    // Its size the pattern's numbers: so long along each face less its joint, out by
    // its proud, a course high less its joints.
    let (low, high) = stone.positions.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(l, h), p| (l.min(*p), h.max(*p)),
    );
    let size = high - low;
    let along: Vec<f32> = [size.x, size.z].to_vec();
    let wanted = [long[0] - joint + proud, long[1] - joint + proud];
    assert!(
        (along.iter().any(|a| (a - wanted[0]).abs() < 1e-3))
            && along.iter().any(|a| (a - wanted[1]).abs() < 1e-3),
        "{size}"
    );
    assert!((size.y - (COURSE_M - 2.0 * joint)).abs() < 1e-3, "{size}");
    assert!(
        (low.x + proud).abs() < 1e-3 && (low.z + proud).abs() < 1e-3,
        "proud of both faces: {low}"
    );
    // Every corner of it on one stone of the wall's picture, well inside its edges: no
    // joint shows on a block.
    let cells = wall.cells(STONE_SHARE);
    for uv in &stone.uvs {
        let inside = crate::cells::standing(&cells, *uv, &|_| 0.0, 2.0, 0.0);
        assert!(inside > 1.0, "{uv:?} is {inside} texels inside a stone");
    }
    // And no face drawn larger than the wall draws its texture, two metres a copy: each
    // triangle's metres along its texture's two ways no more than two a copy.
    let all = stone.indexed();
    for t in all.indices.chunks_exact(3) {
        let [p, q, r] = [0, 1, 2].map(|k| all.positions[t[k] as usize]);
        let [a, b, c] = [0, 1, 2].map(|k| all.uvs[t[k] as usize]);
        let (d1, d2) = ([b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]]);
        let det = d1[0] * d2[1] - d1[1] * d2[0];
        // Its faces; a bevel's slant is its arris's, a centimetre or two across.
        let n = (q - p).cross(r - p);
        if det.abs() < 1e-9 || n.length() < 1e-7 || n.normalize().abs().max_element() < 0.99 {
            continue;
        }
        let ju = ((q - p) * d2[1] - (r - p) * d1[1]) / det;
        let jv = ((r - p) * d1[0] - (q - p) * d2[0]) / det;
        for way in [ju, jv] {
            assert!(
                way.length() <= 2.0 * 1.02,
                "drawn {} metres a copy",
                way.length()
            );
        }
    }
}

#[test]
fn rain_streaks_a_wall_under_its_sill_and_open_top_not_under_a_roof_and_wets_its_foot() {
    use terra_build::{BuildOp, Cube, Kind};
    let kind = Kind(4);
    let mut base = Base::default();
    // A wall eight cubes long and six high, a window two cubes up in it.
    base.apply(BuildOp::Fill {
        min: [0, 0, 0],
        max: [7, 5, 0],
        cube: Some(Cube::new(kind, Fill::Solid)),
    });
    base.apply(BuildOp::Fill {
        min: [2, 2, 0],
        max: [3, 3, 0],
        cube: Some(Cube::new(kind, Fill::Open)),
    });
    // A roof's cube over its east end.
    base.apply(BuildOp::Fill {
        min: [6, 6, 0],
        max: [7, 6, 0],
        cube: Some(Cube::new(Kind::FIRST_ROOF, Fill::Solid)),
    });
    let masks = Masks {
        dirt_tint: [0.5; 3],
        splash: 0.5,
        streaks: 1.0,
        streak_reach: 1.5,
        ..Masks::default()
    };
    let mut rained = Rained {
        base: &base,
        kind,
        feet: BTreeMap::new(),
        sources: BTreeMap::new(),
    };
    let out = Vec3::Z;
    // Somewhere along the face under the sill a streak darkens it; nowhere beside it.
    let darkest = |rained: &mut Rained, x0: f32, x1: f32, y: f32| {
        (0..40)
            .map(|i| {
                let x = x0 + (x1 - x0) * i as f32 / 40.0;
                rained.shade(&masks, Vec3::new(x, y, CELL_M), out, None).x
            })
            .fold(1.0f32, f32::min)
    };
    assert!(darkest(&mut rained, 1.0, 2.0, 0.8) < 0.9);
    assert!((darkest(&mut rained, 2.6, 3.4, 0.8) - 1.0).abs() < 1e-4);
    // Under the open top, streaked; under the roof, dry.
    assert!(darkest(&mut rained, 0.0, 1.0, 2.6) < 0.9);
    assert!((darkest(&mut rained, 3.0, 4.0, 2.6) - 1.0).abs() < 1e-4);
    // Wet over its foot, dry a metre up.
    let foot = rained
        .shade(&masks, Vec3::new(0.25, 0.05, CELL_M), out, None)
        .x;
    assert!(foot < 0.65, "{foot}");
    let up = rained
        .shade(&masks, Vec3::new(0.25, 1.4, CELL_M), out, None)
        .x;
    assert!((up - 1.0).abs() < 1e-4, "{up}");
}

#[test]
fn a_square_ends_cap_lays_nothing_over_the_pieces_own_underside() {
    // A coping's section, its underside three points in a line from its drip back into
    // the wall: at a square end the part set in is cut back off the line, so a cap
    // across those three would lie in the underside's plane, over its own faces.
    let sweep = Sweep {
        from: Vec3::ZERO,
        to: Vec3::new(0.5, 0.0, 0.0),
        side: Vec3::Z,
        up: Vec3::Y,
        ends: [End::Square, End::Square],
        extend: 0.0,
        joints: false,
        lift: 0.0,
        sink: 0.0,
        tint: [1.0; 3],
        grain: false,
        buried: false,
    };
    let mut made = Indexed::default();
    let profile = [
        [-0.05, 0.05],
        [0.06, 0.03],
        [0.06, -0.1],
        [0.004, -0.1],
        [0.0, -0.1],
        [-0.05, -0.1],
    ];
    sweep.draw(&profile, [1.0, 1.0], 1, &mut made);
    let surface = made.loose();
    let folded = crate::inspect::self_overlap(&surface);
    assert!(folded < 1e-7, "{} cm² over itself", folded * 1e4);
}

#[test]
fn a_bands_ends_at_a_joint_or_a_step_keep_its_part_in_the_wall_off_the_cubes_side() {
    // A band along a cube's face, half of it set into the wall, run on at one end and
    // stepping at the other: the ends of the part set in are cut back off the cube's
    // sides, where a step in the footing under it may have a face of its own; the part
    // standing out still meets the next band's at the line.
    let sweep = Sweep {
        from: Vec3::ZERO,
        to: Vec3::new(0.5, 0.0, 0.0),
        side: Vec3::Z,
        up: Vec3::Y,
        ends: [End::On, End::Step],
        extend: 0.0,
        joints: false,
        lift: 0.0,
        sink: 0.0,
        tint: [1.0; 3],
        grain: false,
        buried: false,
    };
    let mut made = Indexed::default();
    let profile = [[-0.05, 0.0], [0.05, 0.0], [0.05, 0.1], [-0.05, 0.1]];
    sweep.draw(&profile, [1.0, 1.0], 1, &mut made);
    let at_end = |p: &Vec3| p.x < 1e-4 || p.x > 0.5 - 1e-4;
    assert!(made.positions.iter().any(|p| p.z > 0.0 && at_end(p)));
    for p in &made.positions {
        if p.z < -1e-3 {
            assert!(!at_end(p), "{p} set in at the cube's side");
        }
    }
}

#[test]
fn a_face_looking_down_is_shaded_as_its_occlusion_would_be_and_a_side_is_not() {
    let masks = Masks::default();
    let mut noise = Noise::default();
    // In light, as drawn: the colour, as on screen, made linear.
    let mut at = |normal: Vec3| {
        let c = masked(
            &masks,
            (false, false),
            Vec3::new(0.0, 2.0, 0.0),
            normal,
            0.0,
            &mut noise,
        )[1];
        libm::powf((c + 0.055) / 1.055, 2.4)
    };
    let (down, side, up) = (at(-Vec3::Y), at(Vec3::X), at(Vec3::Y));
    assert!(
        (side - 1.0).abs() < 1e-5 && (up - 1.0).abs() < 1e-5,
        "{side} {up}"
    );
    assert!(down < 0.6, "a soffit as lit as a wall: {down}");
    // Two thirds of its light taken at most, as drawn, not nine tenths (D373).
    assert!(down > 0.3, "a soffit crushed to black: {down}");
    // A log's lower flank, half way down, by a fifth at most: it sees the sky too.
    let flank = at(Vec3::new(1.0, -1.0, 0.0).normalize());
    assert!(flank > 0.8, "{flank}");
}

#[test]
fn an_empty_surface_takes_the_masks_without_a_fault() {
    // A section can draw nothing in a material; its masks then have no columns.
    let kit = terra_build::testing::kit();
    let mut surface = Surface::default();
    mask(&mut surface, &kit, Material::Log, &Base::default());
    assert_eq!(surface.triangle_count(), 0);
}

#[test]
fn a_wall_in_a_room_dug_deep_under_its_base_is_dirtied_from_its_floor_not_as_buried() {
    use terra_build::{BuildOp, Cube, Kind};
    // A dungeon room's shell as terra_dungeon stamps it, ten metres under its base's level,
    // with no ground sampled (as the dungeon's base): a stone floor, a wall four metres high
    // on its north edge, a ceiling over it; and the same shell standing on the ground.
    let shell = |under: i32| {
        let mut base = Base::default();
        let solid = Some(Cube::new(Kind(4), Fill::Solid));
        for (min, max) in [
            ([0, -1, 0], [7, -1, 7]),
            ([0, 0, 0], [7, 7, 0]),
            ([0, 8, 0], [7, 8, 7]),
        ] {
            let (min, max): ([i32; 3], [i32; 3]) = (min, max);
            base.apply(BuildOp::Fill {
                min: [min[0], min[1] - under, min[2]],
                max: [max[0], max[1] - under, max[2]],
                cube: solid,
            });
        }
        base
    };
    let mut kit = terra_build::testing::kit();
    kit.dress.masks = Masks {
        dirt: 1.4,
        dirt_tint: [0.4, 0.37, 0.32],
        ..Masks::default()
    };
    // The room side of the wall at its foot and 2.5 m up, past the dirt's highest rise, and
    // the ceiling's underside: their shades, dug in and on the ground.
    let shades = |under: i32| {
        let floor = -(under as f32) * CELL_M;
        let mut surface = Surface::default();
        for (y, normal) in [(0.05, Vec3::Z), (2.5, Vec3::Z), (4.0, -Vec3::Y)] {
            let z = if normal == Vec3::Z { CELL_M } else { 2.0 };
            surface.positions.push(Vec3::new(1.8, floor + y, z));
            surface.normals.push(normal);
            surface.colours.push([1.0; 3]);
        }
        mask(&mut surface, &kit, Material::Masonry, &shell(under));
        surface.colours.iter().map(|c| c[0]).collect::<Vec<_>>()
    };
    let (dug, standing) = (shades(20), shades(0));
    for (i, (dug, standing)) in dug.iter().zip(&standing).enumerate() {
        assert!(
            (dug - standing).abs() < 0.08,
            "point {i}: {dug} dug in, {standing} on the ground"
        );
    }
    // Dirty at its foot, clean up the wall, as a wall on the ground.
    assert!(dug[0] < 0.6, "the foot not dirtied: {}", dug[0]);
    assert!(
        (dug[1] - 1.0).abs() < 1e-4,
        "dirtied 2.5 m up its wall: {}",
        dug[1]
    );
}

#[test]
fn a_wall_in_a_dug_room_over_the_level_under_it_is_splashed_from_its_floor() {
    use terra_build::{BuildOp, Cube, Kind};
    // A dungeon's wall as terra_dungeon stamps two levels, twenty metres under its base's
    // level: the wall's kind running on down through the floor slab (another kind) into the
    // wall of the level under it, so the wall's own foot is six metres under the floor.
    let mut kit = terra_build::testing::kit();
    kit.dress.masks = Masks {
        dirt_tint: [0.5; 3],
        splash: 0.35,
        ..Masks::default()
    };
    let (wall, floor) = (kit.kind(), Kind(kit.kind().0 + 1));
    let under = 40;
    let mut base = Base::default();
    for (min, max, kind) in [
        ([0, -13, 0], [7, 7, 0], wall),
        ([0, -1, 1], [7, -1, 7], floor),
    ] {
        base.apply(BuildOp::Fill {
            min: [min[0], min[1] - under, min[2]],
            max: [max[0], max[1] - under, max[2]],
            cube: Some(Cube::new(kind, Fill::Solid)),
        });
    }
    let top = -(under as f32) * CELL_M;
    let mut surface = Surface::default();
    for y in [0.05, 1.4] {
        surface.positions.push(Vec3::new(1.8, top + y, CELL_M));
        surface.normals.push(Vec3::Z);
        surface.colours.push([1.0; 3]);
    }
    mask(&mut surface, &kit, Material::Masonry, &base);
    let [foot, up] = [surface.colours[0][0], surface.colours[1][0]];
    // Wet over the floor it stands on, dry a metre up, as a wall on its own foot.
    assert!(foot < 0.65, "the foot not splashed: {foot}");
    assert!((up - 1.0).abs() < 1e-4, "splashed 1.4 m up: {up}");
}
