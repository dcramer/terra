use super::standing::deck;
use super::*;
use crate::kit::Pattern;
use crate::roofer::{self, Style};
use crate::{BuildOp, STOREY_M};

const ROOF: Kind = Kind::FIRST_ROOF;

#[test]
fn a_gable_cut_follows_a_valley_across_its_cube_and_never_stands_over_it() {
    // An inner corner's underside over a cube: level at 3.5 m along its north and west,
    // falling to 3 m at its south-east corner, the valley on the diagonal between.
    let m = |metres: f32| super::super::ticks(metres);
    let under = [
        [m(3.5), m(3.5), m(3.5)],
        [m(3.5), m(3.25), m(3.25)],
        [m(3.5), m(3.25), m(3.0)],
    ];
    // The roof itself: the higher of the two slopes meeting in the valley.
    let roof = |x: f32, z: f32| 3.5 - 0.5 * x.min(z);
    for i in 0..=20 {
        for k in 0..=20 {
            let (x, z) = (i as f32 / 20.0, k as f32 / 20.0);
            let cut = gable_cut(IVec3::ZERO, &under, Vec3::new(x * CELL_M, 0.0, z * CELL_M));
            assert!(
                (cut - roof(x, z)).abs() < 2e-3,
                "{x} {z}: {cut} for {}",
                roof(x, z)
            );
        }
    }
}

fn roof_kit() -> Kit {
    Kit {
        id: ROOF.0,
        pattern: Pattern::Roof,
        ..Kit::default()
    }
}

/// A room of plain walls over a footprint of rectangles, roofed by the roofer in a
/// style.
fn roofed(rooms: &[([i32; 3], [i32; 3])], style: Style) -> Base {
    let mut base = Base::default();
    let mut footprint = BTreeSet::new();
    for &(min, max) in rooms {
        base.apply(BuildOp::AddRoom {
            min,
            max,
            kind: Kind::PLAIN,
        });
        for x in min[0]..=max[0] {
            for z in min[2]..=max[2] {
                footprint.insert([x, z]);
            }
        }
    }
    // The flat roof the room tool lays, off, the walls' top layer left under the roof,
    // as "add a roof" does.
    for &[x, z] in &footprint {
        if base.filling(IVec3::new(x, 0, z)).is_none() {
            base.apply(BuildOp::Fill {
                min: [x, 5, z],
                max: [x, 5, z],
                cube: None,
            });
        }
    }
    let plan = roofer::over(&footprint, 6, style, None);
    for (at, shape) in plan.roof {
        base.apply(BuildOp::Fill {
            min: at.to_array(),
            max: at.to_array(),
            cube: Some(Cube {
                shape,
                ..Cube::new(ROOF, Fill::Solid)
            }),
        });
    }
    for at in plan.gables {
        base.apply(BuildOp::Fill {
            min: at.to_array(),
            max: at.to_array(),
            cube: Some(Cube::new(Kind::PLAIN, Fill::Solid)),
        });
    }
    base
}

fn laid(base: &Base) -> Vec<Piece> {
    let kit = roof_kit();
    let mut pieces = Vec::new();
    lay(
        base,
        &kit,
        &Kits(vec![kit.clone()]),
        Scope::ALL,
        &mut pieces,
    );
    pieces
}

fn parts(pieces: &[Piece], part: RoofPart) -> Vec<(Vec3, Vec3)> {
    pieces
        .iter()
        .filter_map(|p| match *p {
            Piece::Roof {
                part: this,
                from,
                to,
                ..
            } if this == part => Some((from.metres(), to.metres())),
            _ => None,
        })
        .collect()
}

fn close(a: Vec3, b: Vec3) -> bool {
    a.distance(b) < 0.01
}

#[test]
fn a_gable_of_cubes_has_one_ridge_two_eaves_and_four_verges() {
    // Five metres by four, the ridge along X two metres in, rising two metres.
    let base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
    let pieces = laid(&base);
    let ridges = parts(&pieces, RoofPart::Ridge);
    assert_eq!(ridges.len(), 1, "{ridges:?}");
    let (a, b) = ridges[0];
    let top = STOREY_M + 2.0;
    // From verge to verge, over the gables' overhang.
    assert!(
        close(a, Vec3::new(-0.5, top, 2.0)) && close(b, Vec3::new(5.5, top, 2.0)),
        "{a} {b}"
    );
    assert_eq!(parts(&pieces, RoofPart::Eave).len(), 2);
    assert_eq!(parts(&pieces, RoofPart::Verge).len(), 4);
    assert!(parts(&pieces, RoofPart::Hip).is_empty());
    assert!(parts(&pieces, RoofPart::Valley).is_empty());
    // The eaves overhang by half a metre, carried on down the slope.
    for (a, b) in parts(&pieces, RoofPart::Eave) {
        assert!(
            (a.y - (STOREY_M - 0.5)).abs() < 0.01 && a.y == b.y,
            "{a} {b}"
        );
        assert!((a.z + 0.5).abs() < 0.01 || (a.z - 4.5).abs() < 0.01, "{a}");
    }
    // A covering over every square, eaves and all, each with its stage.
    let squares = pieces
        .iter()
        .filter(|p| matches!(p, Piece::Covering { .. }))
        .count();
    assert_eq!(squares, 24 * 20);
    for piece in &pieces {
        let (stage, want) = match piece {
            Piece::Covering { cell, .. } => (cell.stage, COVERING),
            Piece::Roof { part, cell, .. } => (cell.stage, part.stage()),
            _ => continue,
        };
        assert_eq!(stage, want, "{piece:?}");
        // Every piece waits on a roof cube.
        let cube = piece.roof_cell().unwrap().cube;
        assert!(base.cube(cube).is_some_and(|c| c.kind == ROOF), "{piece:?}");
    }
    // Rafters along every cube's edge up each slope, nine a side from one gable's inner
    // face to the other's, none on a gable or out under a verge, and a beam under the ridge.
    assert_eq!(parts(&pieces, RoofPart::Rafter).len(), 2 * 9 + 1);
}

#[test]
fn battens_either_side_of_a_hip_stop_short_of_one_another() {
    // A tower's pyramid: four faces, four hips, each face's battens level with the next
    // face's. Laid square across its face, each stops in from the hip by its width, so
    // its upper edge does not reach over the hip into the next face's at its height.
    let base = roofed(&[([0, 0, 0], [7, 0, 7])], Style::Hip);
    let pieces = laid(&base);
    let wide = roof_kit().roof.batten.0;
    let battens = parts(&pieces, RoofPart::Batten);
    assert!(battens.len() > 8);
    // The hips run from the corners to the apex at the middle, 45° in plan: a point is
    // on a face, not past its hips, while it is no nearer the middle across the face than
    // along the hip.
    let middle = Vec3::new(2.0, 0.0, 2.0);
    for (from, to) in &battens {
        let along = (*to - *from).normalize();
        let across = Vec3::Y.cross(along).normalize();
        for end in [*from, *to] {
            // Its corner up the slope from the end, toward the middle.
            let toward = (middle - end).with_y(0.0);
            let up = if across.dot(toward) > 0.0 {
                across
            } else {
                -across
            };
            let corner = end + up * wide;
            let d = (corner - middle).with_y(0.0);
            let (a, c) = (d.dot(along).abs(), d.dot(up).abs());
            assert!(
                a <= c + 1e-3,
                "{from}..{to}: its corner {corner} past a hip"
            );
        }
    }
}

#[test]
fn a_batten_jointed_to_the_next_of_its_line_takes_no_cap_there() {
    // Twenty metres long: the battens jointed at sections' edges, each two at a joint
    // running on as one, so neither is capped there; capped at the gables.
    let base = roofed(&[([0, 0, 0], [39, 0, 7])], Style::Gable);
    let pieces = laid(&base);
    let battens: Vec<(Place, Place, [bool; 2])> = pieces
        .iter()
        .filter_map(|p| match *p {
            Piece::Roof {
                part: RoofPart::Batten,
                from,
                to,
                capped,
                ..
            } => Some((from, to, capped)),
            _ => None,
        })
        .collect();
    let mut joints = 0;
    for &(from, to, capped) in &battens {
        for (end, at) in [from, to].into_iter().enumerate() {
            let joined = battens
                .iter()
                .any(|&(f, t, _)| (f, t) != (from, to) && (f == at || t == at));
            assert_eq!(capped[end], !joined, "{from:?}..{to:?} at {at:?}");
            joints += usize::from(joined);
        }
    }
    assert!(joints >= 4, "{joints}");
}

#[test]
fn a_wings_ridge_beam_butts_the_main_ones_side() {
    // A T: the wing's ridge meets the main ridge along it, the beams under them at one
    // height, the wing's ending at the main one's side, not in it.
    let (_, ops) = crate::roof_rows(&[Kind::PLAIN], false, &|_| ROOF)
        .into_iter()
        .find(|(row, _)| row == "T, gable roof")
        .expect("a T under a gable");
    let mut base = Base::default();
    for op in ops {
        base.apply(op);
    }
    let pieces = laid(&base);
    let half = roof_kit().roof.rafter * 0.5;
    let beams: Vec<(Vec3, Vec3)> = parts(&pieces, RoofPart::Rafter)
        .into_iter()
        .filter(|(a, b)| (a.y - b.y).abs() < 1e-3)
        .collect();
    let along_x = beams.iter().find(|(a, b)| (a.z - b.z).abs() < 1e-3);
    let along_z = beams.iter().find(|(a, b)| (a.x - b.x).abs() < 1e-3);
    let (Some(main), Some(wing)) = (along_x, along_z) else {
        panic!("a beam under each ridge: {beams:?}");
    };
    for end in [wing.0, wing.1] {
        let off = (end.z - main.0.z).abs();
        assert!(
            off > half,
            "the wing's beam ends at {end}, in the main one at {main:?}"
        );
    }
}

#[test]
fn a_rafter_stops_at_the_ridges_plumb_line_clear_of_its_pair() {
    // Rafters either side of a ridge, along the same cube's edge, lie in one plane: one
    // reaching past the ridge's plumb line would lie in the other.
    let base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
    let pieces = laid(&base);
    let (a, _) = parts(&pieces, RoofPart::Ridge)[0];
    let half = roof_kit().roof.rafter * 0.5;
    let mut looked = 0;
    for (from, to) in parts(&pieces, RoofPart::Rafter) {
        if (from.z - to.z).abs() < 1e-3 {
            continue;
        }
        let (top, foot) = if from.y > to.y {
            (from, to)
        } else {
            (to, from)
        };
        let side = (foot.z - a.z).signum();
        // Its edge under its top end: square to it, down, in its plane.
        let axis = (top - foot).normalize();
        let down = Vec3::X.cross(axis) * -side.signum();
        let down = if down.y > 0.0 { -down } else { down };
        let corner = top + down * half;
        assert!(
            (corner.z - a.z) * side > -1e-3,
            "{from} {to}: its foot's edge at {corner}, past the ridge at {a}"
        );
        looked += 1;
    }
    assert!(looked > 10);
}

#[test]
fn a_hip_of_cubes_has_four_hips_and_a_short_ridge() {
    let base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Hip);
    let pieces = laid(&base);
    let hips = parts(&pieces, RoofPart::Hip);
    assert_eq!(hips.len(), 4, "{hips:?}");
    let ridges = parts(&pieces, RoofPart::Ridge);
    assert_eq!(ridges.len(), 1);
    let (a, b) = ridges[0];
    // A metre long: the five metres less two metres' run at each end.
    assert!((a.distance(b) - 1.0).abs() < 0.01, "{a} {b}");
    assert!(parts(&pieces, RoofPart::Verge).is_empty());
    assert_eq!(parts(&pieces, RoofPart::Eave).len(), 4);
    // Hips run from the eaves' corners, past the walls, up to the ridge's ends.
    for (a, b) in hips {
        let (low, high) = if a.y < b.y { (a, b) } else { (b, a) };
        assert!((low.y - (STOREY_M - 0.5)).abs() < 0.01, "{low}");
        assert!(
            close(high, ridges[0].0) || close(high, ridges[0].1),
            "{high}"
        );
    }
}

#[test]
fn an_l_of_cubes_has_a_valley_where_its_wings_meet_and_eaves_that_meet_in_it() {
    let base = roofed(
        &[([0, 0, 0], [11, 0, 7]), ([0, 0, 7], [7, 0, 15])],
        Style::Hip,
    );
    let pieces = laid(&base);
    let valleys = parts(&pieces, RoofPart::Valley);
    assert_eq!(valleys.len(), 1, "{valleys:?}");
    // From the inside corner of the L's eaves, past the walls, up to the hips.
    let (a, b) = valleys[0];
    let low = if a.y < b.y { a } else { b };
    assert!(close(low, Vec3::new(4.5, STOREY_M - 0.5, 4.5)), "{low}");
    // Every eave is one board, the inside corner's two meeting in the valley.
    assert_eq!(parts(&pieces, RoofPart::Eave).len(), 6);
}

#[test]
fn an_odd_span_is_capped_by_one_ridge_down_its_flat_middle() {
    let base = roofed(&[([0, 0, 0], [9, 0, 8])], Style::Gable);
    let pieces = laid(&base);
    let ridges = parts(&pieces, RoofPart::Ridge);
    assert_eq!(ridges.len(), 1, "{ridges:?}");
    assert!((ridges[0].0.z - 2.25).abs() < 0.01, "{ridges:?}");
}

#[test]
fn a_roof_against_a_taller_wall_is_flashed_and_its_cubes_hold_by_their_ramp() {
    // A lean-to rising south to a wall two storeys high.
    let mut base = roofed(&[([0, 0, 0], [9, 0, 5])], Style::LeanTo);
    // Its own gables and high side off, and the tall wall where its high side was.
    base.apply(BuildOp::Fill {
        min: [-2, 0, 6],
        max: [11, 13, 6],
        cube: Some(Cube::new(Kind::PLAIN, Fill::Solid)),
    });
    let pieces = laid(&base);
    let flashing = parts(&pieces, RoofPart::Flashing);
    assert_eq!(flashing.len(), 1, "{flashing:?}");
    let (a, b) = flashing[0];
    assert!(
        (a.z - 3.0).abs() < 0.01 && (b.z - 3.0).abs() < 0.01,
        "{a} {b}"
    );
    assert!(base.floating().is_empty());
}

#[test]
fn a_storey_built_over_a_roof_stops_it_and_a_second_roof_over_it_is_apart() {
    let mut base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
    // A second roof two storeys up over the same footprint, by itself.
    let upper = roofer::over(
        &(0..10).flat_map(|x| (0..8).map(move |z| [x, z])).collect(),
        18,
        Style::Hip,
        None,
    );
    for (at, shape) in upper.roof {
        base.apply(BuildOp::Fill {
            min: at.to_array(),
            max: at.to_array(),
            cube: Some(Cube {
                shape,
                ..Cube::new(ROOF, Fill::Solid)
            }),
        });
    }
    let pieces = laid(&base);
    assert_eq!(parts(&pieces, RoofPart::Hip).len(), 4);
    assert_eq!(parts(&pieces, RoofPart::Ridge).len(), 2);
}

#[test]
fn a_stone_gable_rises_into_the_roof_cubes_over_it() {
    let stone = Kit {
        id: 2,
        pattern: Pattern::Masonry,
        ..Kit::default()
    };
    let mut base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
    // Its walls of stone, the gables too.
    let walls: Vec<IVec3> = base
        .cubes()
        .filter(|(_, c)| c.kind == Kind::PLAIN)
        .map(|(at, _)| at)
        .collect();
    for at in walls {
        base.apply(BuildOp::Fill {
            min: at.to_array(),
            max: at.to_array(),
            cube: Some(Cube::new(stone.kind(), Fill::Solid)),
        });
    }
    let mut pieces = Vec::new();
    gables(&base, &stone, Scope::ALL, &mut pieces);
    // Over the west gable's middle, the roof cube at its top, cut at the underside.
    let tops: Vec<IVec3> = pieces
        .iter()
        .filter_map(|p| match *p {
            Piece::Gable { cube, .. } => Some(cube),
            _ => None,
        })
        .collect();
    assert!(tops.contains(&IVec3::new(0, 9, 3)), "{tops:?}");
    for piece in &pieces {
        if let Piece::Gable { cube, under, .. } = piece {
            assert!(base.cube(*cube).is_some_and(|c| c.kind.is_roof()));
            let highest = super::super::metres(*under.iter().flatten().max().unwrap());
            assert!(
                highest > cube.y as f32 * CELL_M && highest <= (cube.y + 1) as f32 * CELL_M + 1e-3
            );
        }
    }
    // Cut along the roof's deck, the plane its covering lies on, with no gap under it:
    // up the west gable's face the eaves are a storey up, rising a metre a metre to the
    // ridge two metres in.
    let deck = |z: f32| STOREY_M + z.min(4.0 - z);
    let mut tried = 0;
    for piece in &pieces {
        let Piece::Gable { cube, under, .. } = piece else {
            continue;
        };
        if cube.x != 0 {
            continue;
        }
        for k in 0..=10 {
            let z = (cube.z as f32 + k as f32 / 10.0) * CELL_M;
            let want = deck(z);
            // Only where the deck crosses this cube, not the one over or under it.
            if want < cube.y as f32 * CELL_M || want > (cube.y + 1) as f32 * CELL_M {
                continue;
            }
            let cut = gable_cut(*cube, under, Vec3::new(0.0, 0.0, z));
            assert!(
                (cut - want).abs() < 0.005,
                "{cube} at {z}: cut at {cut}, deck {want}"
            );
            tried += 1;
        }
    }
    assert!(tried > 20, "{tried}");
}

/// Every point along a piece of a roof's frame, a rafter, a beam or a batten.
fn frame_points(pieces: &[Piece]) -> Vec<(Vec3, bool)> {
    pieces
        .iter()
        .filter_map(|p| match *p {
            Piece::Roof {
                part: RoofPart::Rafter | RoofPart::Batten,
                from,
                to,
                ..
            } => Some((from.metres(), to.metres())),
            _ => None,
        })
        .flat_map(|(a, b)| {
            let level = (a.y - b.y).abs() < 1e-3;
            (0..=20).map(move |i| (a.lerp(b, i as f32 / 20.0), level))
        })
        .collect()
}

#[test]
fn no_piece_of_a_frame_crosses_a_gable_or_ends_in_the_air_past_it() {
    // Five metres by four, the gables' outer faces at x = 0 and 5, their inner faces
    // half a metre in.
    let base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
    let points = frame_points(&laid(&base));
    assert!(points.len() > 100);
    for (p, _) in points {
        assert!(
            (0.5 - 1e-3..=4.5 + 1e-3).contains(&p.x),
            "a piece of frame at {p}, in a gable or out past it"
        );
    }
}

#[test]
fn a_level_piece_of_frame_runs_only_over_the_rooms_of_any_plan() {
    // An L, a T and a U of gables: no batten or ridge beam in a wall, gable or not, nor
    // out past the walls under an eave or a verge.
    for shape in ["L", "T", "U"] {
        let (rooms, _) = crate::roof_plan(shape, Style::Gable, Style::Gable);
        let rooms: Vec<([i32; 3], [i32; 3])> = rooms.to_vec();
        let base = roofed(&rooms, Style::Gable);
        // In the plan, and not in its outer walls: every column round it in the plan too.
        let planned = |x: i32, z: i32| {
            rooms
                .iter()
                .any(|(min, max)| (min[0]..=max[0]).contains(&x) && (min[2]..=max[2]).contains(&z))
        };
        let inside = |x: i32, z: i32| (-1..=1).all(|dx| (-1..=1).all(|dz| planned(x + dx, z + dz)));
        let points = frame_points(&laid(&base));
        assert!(points.iter().any(|(_, level)| *level), "{shape}");
        for (p, level) in points {
            let (x, z) = (
                libm::floorf(p.x / CELL_M) as i32,
                libm::floorf(p.z / CELL_M) as i32,
            );
            // On a cube's edge, either side will do.
            let near = |a: f32| (a / CELL_M - libm::roundf(a / CELL_M)).abs() < 1e-3;
            let xs = if near(p.x) { vec![x - 1, x] } else { vec![x] };
            let zs = if near(p.z) { vec![z - 1, z] } else { vec![z] };
            let over_a_room = xs.iter().any(|&x| zs.iter().any(|&z| inside(x, z)));
            assert!(
                !level || over_a_room,
                "{shape}: a level piece of frame at {p}"
            );
        }
    }
}

#[test]
fn a_point_under_a_roof_finds_its_underside() {
    let base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
    let kit = roof_kit();
    let roofs = Roofs::whole(&base, &Kits(vec![kit]), Scope::ALL);
    // A metre in from the eave, a metre up the slope.
    let top = roofs.top_over(2.0, 1.0, 0.0).unwrap();
    assert!((top - (STOREY_M + 1.0)).abs() < 1e-3, "{top}");
    // Past the eave, the overhang.
    let eave = roofs.top_over(2.0, -0.25, 0.0).unwrap();
    assert!((eave - (STOREY_M - 0.25)).abs() < 1e-3, "{eave}");
    let (up, inside) = roofs.place_of(Vec3::new(2.0, 4.99, 2.0 - 0.01)).unwrap();
    assert!(up > 0.9 && inside > 1.0, "{up} {inside}");
}

/// How many edge pieces lie along each open side of each square of covering.
fn closings(base: &Base) -> Vec<(Vec3, Vec3, usize)> {
    let pieces = laid(base);
    let edges: Vec<(Vec3, Vec3)> = pieces
        .iter()
        .filter_map(|p| match *p {
            Piece::Roof { part, from, to, .. }
                if !matches!(
                    part,
                    RoofPart::Rafter
                        | RoofPart::Batten
                        | RoofPart::Ridge
                        | RoofPart::Hip
                        | RoofPart::Valley
                ) =>
            {
                Some((from.metres(), to.metres()))
            }
            _ => None,
        })
        .collect();
    // Whether a point lies on a piece's line, within a centimetre.
    let on = |p: Vec3, (a, b): (Vec3, Vec3)| {
        let t = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
        (a + (b - a) * t).distance(p) < 0.01
    };
    let mut out = Vec::new();
    for piece in &pieces {
        let Piece::Covering { corners, open, .. } = piece else {
            continue;
        };
        for side in 0..4 {
            if !open[side] {
                continue;
            }
            let (p, r) = (corners[side].metres(), corners[(side + 1) % 4].metres());
            let n = edges
                .iter()
                .filter(|&&line| on(p, line) && on(r, line) && on((p + r) * 0.5, line))
                .count();
            out.push((p, r, n));
        }
    }
    out
}

#[test]
fn every_open_side_of_a_roofs_covering_is_closed_by_one_edge_piece() {
    // docs/building-voxels.md, invariant 3: every side of a square of covering that does
    // not carry on into the next is closed by exactly one edge piece, an eave, a verge or
    // the flashing against what stands there, over every plan, style and edge a roof has.
    let rect = [([0, 0, 0], [9, 0, 7])];
    let l = [([0, 0, 0], [11, 0, 7]), ([0, 0, 7], [7, 0, 15])];
    let t = [([0, 0, 0], [13, 0, 7]), ([3, 0, 7], [10, 0, 15])];
    let mut bases = Vec::new();
    for style in [Style::Gable, Style::Hip, Style::LeanTo] {
        bases.push((format!("rectangle {style:?}"), roofed(&rect, style)));
    }
    for style in [Style::Gable, Style::Hip] {
        bases.push((format!("L {style:?}"), roofed(&l, style)));
        bases.push((format!("T {style:?}"), roofed(&t, style)));
    }
    // A lean-to rising south to a wall two storeys high, and a gable whose verge is
    // against a taller wall beyond its end: their edges against the walls are flashed.
    let tall = |base: &mut Base, min: [i32; 3], max: [i32; 3]| {
        base.apply(BuildOp::Fill {
            min,
            max,
            cube: Some(Cube::new(Kind::PLAIN, Fill::Solid)),
        });
    };
    let mut against = roofed(&[([0, 0, 0], [9, 0, 5])], Style::LeanTo);
    tall(&mut against, [-2, 0, 6], [11, 13, 6]);
    bases.push(("lean-to against a wall".into(), against));
    let mut verge = roofed(&rect, Style::Gable);
    tall(&mut verge, [-1, 0, -1], [-1, 13, 8]);
    bases.push(("verge against a wall".into(), verge));
    for (name, base) in &bases {
        let sides = closings(base);
        assert!(sides.len() > 20, "{name}: {}", sides.len());
        for (p, r, n) in sides {
            assert_eq!(
                n, 1,
                "{name}: the covering's side {p} to {r} closed {n} times"
            );
        }
    }
}
/// The highest roof cube in a column.
fn top_of(base: &Base, x: i32, z: i32) -> IVec3 {
    let y = base
        .column(x, z)
        .filter(|&y| {
            base.cube(IVec3::new(x, y, z))
                .is_some_and(|c| c.kind.is_roof())
        })
        .max()
        .expect("a roof over the column");
    IVec3::new(x, y, z)
}

fn put(base: &mut Base, at: IVec3, cube: Cube) {
    base.apply(BuildOp::Fill {
        min: at.to_array(),
        max: at.to_array(),
        cube: Some(cube),
    });
}

/// Every flashing piece: what it is and its ends.
fn flashings(pieces: &[Piece]) -> Vec<(RoofPart, Vec3, Vec3)> {
    pieces
        .iter()
        .filter_map(|p| match *p {
            Piece::Roof { part, from, to, .. } if is_flashing(part) => {
                Some((part, from.metres(), to.metres()))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_wall_standing_on_a_slope_leaves_the_roof_whole_under_it_and_is_flashed_round() {
    // D343 as amended: a stack of wall standing on the north slope's middle. The roof
    // is drawn the same under it, covering and frame, and its foot is flashed: an apron
    // below it, where the roof rises to it, steps up its sides, a back gutter above it,
    // where the roof falls to it.
    let mut base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
    let roof = top_of(&base, 4, 1);
    for y in 1..=2 {
        put(
            &mut base,
            roof + IVec3::Y * y,
            Cube::new(Kind::PLAIN, Fill::Solid),
        );
    }
    let pieces = laid(&base);
    let under = pieces
        .iter()
        .filter(|p| matches!(p, Piece::Covering { cell, .. } if cell.cube == roof))
        .count();
    assert_eq!(under, 4, "the covering runs on under the wall");
    let (x0, x1, z0, z1) = (2.0, 2.5, 0.5, 1.0);
    let rafters_under = parts(&pieces, RoofPart::Rafter)
        .iter()
        .filter(|(a, b)| {
            let mid = (*a + *b) * 0.5;
            (x0 - 0.01..=x1 + 0.01).contains(&mid.x) && (0.0..=1.5).contains(&mid.z)
        })
        .count();
    assert!(rafters_under > 0, "the frame carries the wall");
    let flashed = flashings(&pieces);
    let along = |want: RoofPart, on: &dyn Fn(Vec3, Vec3) -> bool| {
        flashed.iter().any(|&(part, a, b)| part == want && on(a, b))
    };
    assert!(
        along(RoofPart::Flashing, &|a, b| (a.z - z0).abs() < 1e-3
            && (b.z - z0).abs() < 1e-3),
        "an apron below: {flashed:?}"
    );
    assert!(
        along(RoofPart::Gutter, &|a, b| (a.z - z1).abs() < 1e-3
            && (b.z - z1).abs() < 1e-3),
        "a back gutter above: {flashed:?}"
    );
    for x in [x0, x1] {
        assert!(
            along(RoofPart::Step, &|a, b| (a.x - x).abs() < 1e-3
                && (b.x - x).abs() < 1e-3),
            "steps up its side at {x}: {flashed:?}"
        );
    }
}

#[test]
fn the_covering_is_tucked_into_a_wall_beside_the_roof() {
    // A lean-to rising south to a wall two storeys high: its covering runs on past its
    // high edge into the wall, under its own boards, so a wall thinner than its cube
    // leaves no slot down past its face.
    let mut base = roofed(&[([0, 0, 0], [9, 0, 5])], Style::LeanTo);
    base.apply(BuildOp::Fill {
        min: [-2, 0, 6],
        max: [11, 13, 6],
        cube: Some(Cube::new(Kind::PLAIN, Fill::Solid)),
    });
    let pieces = laid(&base);
    let face = 3.0;
    let tucked: Vec<[Vec3; 4]> = pieces
        .iter()
        .filter_map(|p| match *p {
            Piece::Covering { corners, open, .. } if open == [false; 4] => {
                Some(corners.map(Place::metres))
            }
            _ => None,
        })
        .filter(|c| c.iter().any(|p| p.z > face + 0.01))
        .collect();
    assert!(!tucked.is_empty(), "no covering tucked into the wall");
    let mut along = 0.0;
    for corners in &tucked {
        let deepest = corners.iter().map(|p| p.z).fold(f32::MIN, f32::max);
        assert!(
            (deepest - (face + TUCK_M)).abs() < 0.01,
            "tucked to {deepest} into the wall at {face}"
        );
        let xs = corners.iter().map(|p| p.x);
        along += xs.clone().fold(f32::MIN, f32::max) - xs.fold(f32::MAX, f32::min);
    }
    // Along the whole edge, from verge to verge.
    assert!(along > 4.4, "{along}");
}

#[test]
fn a_hole_in_a_roof_is_flashed_round_and_takes_no_eaves() {
    // Gap 5 of docs/building-voxels.md: a cube taken out of a slope, as a smoke hole.
    let mut base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
    let hole = top_of(&base, 4, 1);
    base.apply(BuildOp::Fill {
        min: hole.to_array(),
        max: hole.to_array(),
        cube: None,
    });
    let pieces = laid(&base);
    let inside = |p: Vec3| (2.0..=2.5).contains(&p.x) && (0.5..=1.0).contains(&p.z);
    for part in [RoofPart::Eave, RoofPart::Verge] {
        assert!(
            parts(&pieces, part)
                .iter()
                .all(|(a, b)| !inside((*a + *b) * 0.5)),
            "{part:?} round the hole"
        );
    }
    assert!(
        pieces.iter().all(|p| match p {
            Piece::Covering { middle, .. } => !inside(middle.metres()),
            _ => true,
        }),
        "covering carried on into the hole: {:?}",
        pieces
            .iter()
            .filter(|p| match p {
                Piece::Covering { middle, .. } => inside(middle.metres()),
                _ => false,
            })
            .collect::<Vec<_>>()
    );
    let round = flashings(&pieces)
        .iter()
        .filter(|(_, a, b)| inside(*a) && inside(*b))
        .count();
    assert!(round >= 4, "flashed on every side: {round}");
}

#[test]
fn a_wall_standing_on_a_roof_is_carried_down_to_its_deck() {
    // R2: stone standing on a slope is the stone's cube cut at the roof's deck from
    // below; a timber kind's foot a curb of its boards on the deck.
    let stone = Kit {
        id: 2,
        pattern: Pattern::Masonry,
        ..Kit::default()
    };
    let timber = Kit {
        id: 3,
        pattern: Pattern::Logs,
        ..Kit::default()
    };
    for kit in [&stone, &timber] {
        let mut base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
        let roof = top_of(&base, 4, 1);
        put(
            &mut base,
            roof + IVec3::Y,
            Cube::new(kit.kind(), Fill::Solid),
        );
        let cube = base.cube(roof).unwrap();
        let mut pieces = Vec::new();
        if kit.pattern == Pattern::Masonry {
            gables(&base, kit, Scope::ALL, &mut pieces);
            assert!(
                pieces.iter().any(|p| matches!(
                    p,
                    Piece::Gable { cube: at, under, carried: super::super::Carried::Down }
                        if *at == roof && *under == deck(roof, cube)
                )),
                "{pieces:?}"
            );
        } else {
            curbs(&base, kit, Scope::ALL, &mut pieces);
            let Some(Piece::Curb { min, max, under }) = pieces.first() else {
                panic!("no curb: {pieces:?}");
            };
            assert_eq!(*under, deck(roof, cube));
            // Up to the foot of what stands on it.
            assert_eq!(max.0[1], (roof.y + 1) * super::super::TICKS);
            assert!(min.0[1] < max.0[1]);
        }
    }
}

#[test]
fn flashing_in_an_inside_corner_butts_and_does_not_lie_in_the_other() {
    // A lean-to in the corner of two taller walls, south and east: the flashing along
    // the east wall, running north and south, stops short of the corner by the south
    // one's width, so the two boards are not laid one in the other.
    let mut base = roofed(&[([0, 0, 0], [9, 0, 5])], Style::LeanTo);
    base.apply(BuildOp::Fill {
        min: [-2, 0, 6],
        max: [11, 13, 6],
        cube: Some(Cube::new(Kind::PLAIN, Fill::Solid)),
    });
    base.apply(BuildOp::Fill {
        min: [10, 0, -2],
        max: [10, 13, 5],
        cube: Some(Cube::new(Kind::PLAIN, Fill::Solid)),
    });
    let flashed = flashings(&laid(&base));
    let east: Vec<_> = flashed
        .iter()
        .filter(|(_, a, b)| (a.x - 5.0).abs() < 1e-3 && (b.x - 5.0).abs() < 1e-3)
        .collect();
    assert!(!east.is_empty(), "{flashed:?}");
    let reach = east
        .iter()
        .map(|(_, a, b)| a.z.max(b.z))
        .fold(f32::MIN, f32::max);
    assert!(
        (reach - (3.0 - 0.2)).abs() < 0.01,
        "the east flashing reaches {reach}: {east:?}"
    );
}

#[test]
fn a_lone_corner_square_carries_no_crease_out_past_its_edges() {
    // Found by `cargo xtask fuzz` (seeds 20, 21, 42, 95, 118, 159; docs/building-voxels.md,
    // gap 11): a corner cube of a roof kind laid by hand where no roof is. Round its corners
    // its two ramps ran on past it, an inner corner's valley up and out past its high corner
    // and down past its low one, an outer corner's hip up past its apex, and the beam and
    // cap under them stood out past the roof's edge.
    use crate::scenes::fuzz::{Element::Blocks, build};
    use crate::{Facing, Shape};
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let materials = crate::scenes::registry::Materials::read(&assets, true).expect("the baseline");
    let kind = |name: &str| {
        materials
            .names
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(&k, _)| k)
            .expect(name)
    };
    let maps = crate::relief::Maps::worst(&materials.kits, false);
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
        let report = crate::inspect::inspect(&base, &materials.kits, &maps);
        assert!(
            report.findings.is_empty(),
            "{shape:?} in {roof}: {:?}",
            report.findings
        );
    }
}
