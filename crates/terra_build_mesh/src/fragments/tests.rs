//! The fragments' tests.

use super::*;
use terra_build::{Base, Fill, Shape as CubeShape};

pub(super) fn cube(shape: CubeShape) -> Cube {
    Cube {
        shape,
        ..Cube::new(Kind(2), Fill::Solid)
    }
}

pub(super) fn chunks(_: Kind) -> Breaks {
    Breaks::Chunks
}

fn flat(_: Vec3) -> f32 {
    0.0
}

fn open(_: Vec3) -> bool {
    false
}

#[test]
fn what_broke_bursts_and_what_hung_falls_whole_later_the_higher_it_was() {
    let wall: Vec<(IVec3, Cube)> = (0..4)
        .flat_map(|y| (0..3).map(move |x| (IVec3::new(x, y, 0), cube(CubeShape::Whole))))
        .collect();
    let foot = |at: IVec3| at.y == 0;
    let parts = burst(&wall, &foot, &chunks, 1.0, 2000);
    let (broken, whole): (Vec<&Fragment>, Vec<&Fragment>) =
        parts.iter().partition(|part| part.shape != Shape::Whole);
    assert_eq!(broken.len(), 3 * Breaks::Chunks.count() as usize);
    assert_eq!(whole.len(), 9);
    assert!(broken.iter().all(|part| part.wait == 0.0));
    let wait = |y: f32| {
        whole
            .iter()
            .filter(|part| (part.at.y - y).abs() < 0.01)
            .map(|part| part.wait)
            .fold(0.0, f32::max)
    };
    assert!(wait(1.75) > wait(0.75), "the top falls after what gave way");
    assert_eq!(
        parts,
        burst(&wall, &foot, &chunks, 1.0, 2000),
        "the same every time"
    );
}

#[test]
fn a_tower_coming_down_stays_within_the_budget_and_falls_solid() {
    let tower: Vec<(IVec3, Cube)> = (0..20)
        .flat_map(|y| (0..15).flat_map(move |x| (0..15).map(move |z| IVec3::new(x, y, z))))
        .map(|at| (at, cube(CubeShape::Whole)))
        .collect();
    let ring = |at: IVec3| at.y == 0;
    let parts = burst(&tower, &ring, &chunks, 1.0, 2000);
    assert!(parts.len() <= 2000, "{}", parts.len());
    let high = parts
        .iter()
        .map(|part| part.at.y + part.size.y * 0.5)
        .fold(0.0, f32::max);
    assert!(high > 9.0, "its top comes down too: {high}");
    // What hung comes down in blocks that between them fill all of it, not as a
    // lattice of every so many cubes.
    let whole: f32 = parts
        .iter()
        .filter(|part| part.shape == Shape::Whole)
        .map(|part| part.size.x * part.size.y * part.size.z)
        .sum();
    let hung = (19 * 15 * 15) as f32 * CELL_M * CELL_M * CELL_M;
    assert!(whole > hung * 0.88, "{whole} of {hung}");
}

#[test]
fn a_block_landing_cracks_into_a_few_uneven_chunks_that_break_again_down_to_pieces() {
    // A block two metres a side, four cubes, falling as one.
    let block = Fragment {
        kind: cube(CubeShape::Whole).kind,
        shape: Shape::Whole,
        at: Vec3::new(1.0, 7.0, 1.0),
        velocity: Vec3::new(0.0, -6.0, 0.0),
        turn: Quat::IDENTITY,
        spin: Vec3::ZERO,
        size: Vec3::splat(4.0 * CELL_M * super::SHRINK),
        wait: 0.0,
        still: false,
        fill: 1.0,
        seed: 7,
        hinge: None,
        body: None,
        sawn: [true; 2],
        laid: None,
    };
    let burst = block.shatter(Breaks::Chunks, 1.0);
    // It cracks into a few smaller chunks of uneven size where it first lands, not into
    // its cubes in a grid, with pieces from the cracks.
    let whole: Vec<_> = burst
        .iter()
        .filter(|part| part.shape == Shape::Whole)
        .collect();
    let longest = |part: &Fragment| part.size.max_element();
    assert!(whole.iter().all(|part| longest(part) < longest(&block)));
    assert!(
        whole.iter().any(|part| longest(part) > CELL_M * 1.5),
        "none bigger than a cube"
    );
    let sizes: std::collections::BTreeSet<[u32; 3]> = whole
        .iter()
        .map(|part| (part.size / CELL_M).round().as_uvec3().to_array())
        .collect();
    assert!(sizes.len() >= 2, "{sizes:?}");
    assert!(
        whole.len() >= 3 && whole.len() < 40,
        "{} chunks of 64 cubes",
        whole.len()
    );
    assert!(burst.len() > whole.len() * 2, "few pieces: {}", burst.len());
    // Broken again and again where they land, it comes down to pieces in the end.
    let mut left: Vec<Fragment> = burst;
    for _ in 0..8 {
        left = left
            .into_iter()
            .flat_map(|part| {
                if part.shape == Shape::Whole {
                    part.shatter(Breaks::Chunks, 1.0)
                } else {
                    vec![part]
                }
            })
            .collect();
    }
    assert!(left.iter().all(|part| part.shape != Shape::Whole));
}

#[test]
fn a_timber_fragment_shows_end_grain_across_its_grain_bark_on_its_sides_and_fresh_wood_where_split()
{
    let mut kit = Kit::default();
    for (material, path) in [(Material::Log, "log"), (Material::LogEnd, "ends")] {
        kit.textures.insert(
            material,
            terra_build::kit::Texture {
                path: path.into(),
                metres: [1.0, 1.0],
                depth: 0.0,
                reflectance: terra_build::kit::matte(),
                strips: None,
                cells: 1,
                worn: Some("wood".into()),
            },
        );
    }
    let made = surfaces(
        &kit,
        &Cut::whole(Vec3::splat(CELL_M), [true; 2], None, [1; 3]),
    );
    let ends = &made.0[&Material::LogEnd];
    // Its ends, across its grain, its first axis: the sawn end, all of its picture.
    assert!(ends.normals.iter().all(|n| n.x.abs() > 0.9));
    assert_eq!(ends.normals.iter().filter(|n| n.x > 0.0).count(), 6);
    let (low, high) = ends
        .uvs
        .iter()
        .fold(([1.0f32; 2], [0.0f32; 2]), |(l, h), uv| {
            (
                [l[0].min(uv[0]), l[1].min(uv[1])],
                [h[0].max(uv[0]), h[1].max(uv[1])],
            )
        });
    assert!(
        low[0] > 0.0 && high[0] < 1.0 && high[0] - low[0] > 0.8,
        "{low:?} {high:?}"
    );
    // Along it, its texture runs with the grain: its second texture axis along the first.
    let face = &made.0[&Material::Log];
    for (corners, uvs) in face.positions.chunks(3).zip(face.uvs.chunks(3)) {
        for (p, uv) in corners.iter().zip(uvs) {
            assert!((uv[1] - p.x * CELL_M).abs() < 1e-4, "{p} at {uv:?}");
        }
    }
    // Its sides bark, a little weathered; its top and bottom, split, bare fresh wood.
    for ((n, wear), colour) in face.normals.iter().zip(&face.wear).zip(&face.colours) {
        if n.z.abs() > 0.9 {
            assert!(*wear < 0.5);
        } else if n.y.abs() > 0.9 {
            assert!(*wear > 0.9 && colour[0] > TIMBER_SHADE);
        }
    }
}

#[test]
fn a_chip_landing_on_a_roof_slides_down_it_and_off_the_eave() {
    // A roof pitched at 35 degrees down to its eave at x = 2 m, 1.6 m up; the ground
    // past it.
    let roof = |p: Vec3| if p.x < 2.0 { 3.0 - p.x * 0.7 } else { 0.0 };
    let fell = [(IVec3::new(1, 8, 0), cube(CubeShape::Whole))];
    let mut chip = burst(&fell, &|_| true, &chunks, 1.0, 100)[0];
    chip.at = Vec3::new(0.5, 4.0, 0.25);
    chip.velocity = Vec3::ZERO;
    for _ in 0..1200 {
        chip.step(1.0 / 60.0, &roof, &open);
    }
    assert!(chip.still, "it comes to rest");
    assert!(
        chip.at.x > 2.0 && chip.at.y < 0.4,
        "on the ground past the eave: {}",
        chip.at
    );
    // On level ground it lies where it fell.
    let mut lying = burst(&fell, &|_| true, &chunks, 1.0, 100)[0];
    lying.at = Vec3::new(0.5, 1.0, 0.25);
    lying.velocity = Vec3::ZERO;
    for _ in 0..600 {
        lying.step(1.0 / 60.0, &flat, &open);
    }
    assert!(
        lying.still && (lying.at.x - 0.5).abs() < 0.3,
        "{}",
        lying.at
    );
}

#[test]
fn a_chip_landing_on_a_built_roof_slides_off_it() {
    // A gable roof of cubes the roofer lays over a footprint eight cubes by six, from
    // the third storey's cubes up, on nothing (what a piece lands on is the roof).
    let footprint: std::collections::BTreeSet<[i32; 2]> =
        (0..8).flat_map(|x| (0..6).map(move |z| [x, z])).collect();
    let plan = terra_build::roofer::over(&footprint, 6, terra_build::roofer::Style::Gable, None);
    let mut base = Base::default();
    for (at, shape) in plan.roof {
        base.apply(terra_build::BuildOp::Fill {
            min: at.to_array(),
            max: at.to_array(),
            cube: Some(Cube {
                shape,
                ..Cube::new(Kind::FIRST_ROOF, Fill::Solid)
            }),
        });
    }
    let floor = |at: Vec3| base.top_under(at).unwrap_or(0.0).max(0.0);
    let top = (0..8)
        .map(|z| floor(Vec3::new(2.0, 5.5, z as f32 * 0.5 + 0.25)))
        .fold(0.0f32, f32::max);
    assert!(top > 3.0, "a roof to land on: {top}");
    let fell = [(IVec3::new(4, 8, 2), cube(CubeShape::Whole))];
    let mut chip = burst(&fell, &|_| true, &chunks, 1.0, 100)[0];
    // Dropped on its ridge, the middle of the footprint's width.
    chip.at = Vec3::new(2.0, top + 0.6, 1.5);
    chip.velocity = Vec3::ZERO;
    for _ in 0..1800 {
        chip.step(1.0 / 60.0, &floor, &open);
    }
    assert!(
        chip.at.y < 0.4,
        "on the ground under the eave, not the roof: {}",
        chip.at
    );
}

#[test]
fn a_whole_cube_breaks_where_it_lands_hard_and_its_pieces_settle() {
    let fell = [(IVec3::new(0, 8, 0), cube(CubeShape::Whole))];
    let mut whole = burst(&fell, &|_| false, &chunks, 1.0, 100)[0];
    let mut broke = None;
    for _ in 0..600 {
        let landing = whole.step(1.0 / 60.0, &flat, &open);
        if landing.breaks {
            broke = Some(whole.shatter(Breaks::Chunks, 1.0));
            break;
        }
    }
    let mut pieces = broke.expect("it breaks where it lands");
    assert_eq!(pieces.len(), Breaks::Chunks.count() as usize);
    for _ in 0..900 {
        for piece in &mut pieces {
            piece.step(1.0 / 60.0, &flat, &open);
            assert!(piece.at.y + 1e-3 >= piece.reach_down(), "through the floor");
        }
    }
    assert!(pieces.iter().all(|piece| piece.still), "they come to rest");
    assert!(pieces.iter().all(|piece| piece.at.y < 0.4), "on the ground");
}

#[test]
fn a_piece_rolls_off_a_ledge_and_glances_off_a_wall() {
    let ledge = |at: Vec3| if at.x < 1.0 { 1.0 } else { 0.0 };
    let mut piece = burst(
        &[(IVec3::new(1, 2, 0), cube(CubeShape::Whole))],
        &|_| true,
        &chunks,
        1.0,
        100,
    )[0];
    piece.at = Vec3::new(0.9, 1.1, 0.25);
    piece.velocity = Vec3::new(1.5, 0.0, 0.0);
    for _ in 0..300 {
        piece.step(1.0 / 60.0, &ledge, &open);
    }
    assert!(
        piece.at.x > 1.0 && piece.at.y < 0.4,
        "off the ledge: {piece:?}"
    );
    let wall = |at: Vec3| at.x > 2.0;
    piece.at = Vec3::new(1.8, 0.5, 0.25);
    piece.velocity = Vec3::new(3.0, 0.5, 0.0);
    piece.still = false;
    for _ in 0..60 {
        piece.step(1.0 / 60.0, &flat, &wall);
    }
    assert!(piece.at.x < 2.0, "it glanced off: {}", piece.at);
}

#[test]
fn a_roof_left_hanging_sheds_its_covering_rather_than_falling_as_cubes() {
    let roof: Vec<(IVec3, Cube)> = (0..3)
        .map(|x| {
            (
                IVec3::new(x, 6 + x, 0),
                Cube::new(Kind::FIRST_ROOF, terra_build::Fill::Solid),
            )
        })
        .collect();
    let parts = burst(&roof, &|_| false, &|_| Breaks::Bits, 1.0, 2000);
    assert!(!parts.is_empty());
    assert!(parts.iter().all(|part| part.shape != Shape::Whole));
    let latest = parts.iter().map(|part| part.wait).fold(0.0, f32::max);
    assert!(latest > 0.0, "the higher a moment later");
}

#[test]
fn each_way_of_breaking_is_its_own_table() {
    let sizes = |breaks: Breaks| breaks.look().pieces[0].most;
    // Logs break into lengths, longer than they are thick; stone into chunks.
    let length = sizes(Breaks::Lengths);
    assert!(length[0] > 2.0 * length[1]);
    let chunk = sizes(Breaks::Chunks);
    assert!(chunk[0] < 2.0 * chunk[1]);
    assert!(Breaks::Crumbs.look().dust > Breaks::Lengths.look().dust);
}

#[test]
fn what_is_built_under_a_point_is_found_by_its_shape() {
    let mut base = Base::default();
    base.apply(terra_build::BuildOp::Fill {
        min: [0, 0, 0],
        max: [0, 2, 0],
        cube: Some(Cube::new(Kind(2), Fill::Solid)),
    });
    base.apply(terra_build::BuildOp::Fill {
        min: [1, 0, 0],
        max: [1, 0, 0],
        cube: Some(cube(CubeShape::Half)),
    });
    assert_eq!(base.top_under(Vec3::new(0.25, 4.0, 0.25)), Some(1.5));
    assert_eq!(base.top_under(Vec3::new(0.75, 1.0, 0.25)), Some(0.25));
    assert_eq!(base.top_under(Vec3::new(3.0, 1.0, 0.25)), None);
    assert!(base.solid_at(Vec3::new(0.25, 0.6, 0.25)));
    assert!(!base.solid_at(Vec3::new(0.75, 0.4, 0.25)));
}

#[test]
fn a_fragment_is_a_closed_bevelled_box_of_its_kinds_material() {
    let stone = Kit {
        id: 2,
        pattern: Pattern::Masonry,
        ..Kit::default()
    };
    for shape in Shape::ALL {
        let cut = if shape == Shape::Whole {
            Cut::whole(Vec3::splat(CELL_M), [true; 2], None, [1; 3])
        } else {
            Cut::piece(shape)
        };
        let drawn = surfaces(&stone, &cut);
        let surface = &drawn.0[&Material::Masonry];
        // Six faces and twelve edges of two triangles, eight corners of one.
        assert_eq!(surface.triangle_count(), 6 * 2 + 12 * 2 + 8);
        for triangle in surface.positions.chunks(3) {
            let normal = (triangle[1] - triangle[0]).cross(triangle[2] - triangle[0]);
            let middle = (triangle[0] + triangle[1] + triangle[2]) / 3.0;
            assert!(normal.dot(middle) > 0.0, "{shape:?} faces in");
            assert!(triangle.iter().all(|p| p.abs().max_element() <= 0.5 + 1e-5));
        }
    }
    assert_eq!(material(&Kit::default()), Material::Log);
}

#[test]
fn a_whole_block_wears_the_walls_face_where_it_stood_and_broken_stone_where_it_split() {
    // What stands beside a break is undressed, broken stone (D305); a block that fell from
    // a wall split from the cubes round it along its beds and its ends along the wall, so
    // they are drawn so too, and its two sides across the wall keep the wall's own face,
    // each point of it the texture the standing wall drew there (`planar` in the base's
    // space, `surface`), at its size along each axis: castle film rounds 8-11 read
    // fragments as blank cubes, a slab's 2 m texture squeezed onto its thin sides.
    let mut stone = Kit {
        id: 2,
        pattern: Pattern::Masonry,
        tint: [0.8, 0.7, 0.6],
        ..Kit::default()
    };
    let metres = [2.0, 2.0];
    stone.textures.insert(
        Material::Masonry,
        terra_build::kit::Texture {
            path: "stone".into(),
            metres,
            depth: 0.0,
            reflectance: terra_build::kit::matte(),
            strips: None,
            cells: 1,
            worn: None,
        },
    );
    let repeat = repeat(&stone);
    assert_eq!(repeat, [4; 3], "a 2 m texture repeats every four cubes");
    // Two walls meeting at a corner, one along x and one along z, 10 cubes long, 5 high.
    let mut wall: Vec<(IVec3, Cube)> = Vec::new();
    for y in 3..8 {
        for i in 0..10 {
            wall.push((IVec3::new(i + 3, y, 5), cube(CubeShape::Whole)));
            wall.push((IVec3::new(2, y, i + 6), cube(CubeShape::Whole)));
        }
    }
    let made = burst(&wall, &|_| false, &chunks, 1.0, 5000);
    let wholes: Vec<&Fragment> = made.iter().filter(|f| f.shape == Shape::Whole).collect();
    assert!(wholes.len() > 4);
    let (mut faces, mut splits, mut turned) = (0, 0, 0);
    for fragment in wholes {
        let laid = fragment.laid.expect("a whole block knows where it stood");
        let cut = fragment.mesh(repeat);
        let drawn = surfaces(&stone, &cut);
        let surface = &drawn.0[&Material::Masonry];
        let size = (fragment.size / CELL_M).round() * CELL_M;
        // It stood over its cubes: its middle where theirs was.
        let reach = (laid.turn * size).abs();
        let low = laid.at - reach * 0.5;
        assert!(
            (low / CELL_M - (low / CELL_M).round()).abs().max_element() < 1e-3,
            "it stood on the cubes' grid, low corner {low}"
        );
        turned += usize::from(laid.turn != Quat::IDENTITY);
        for ((triangle, uvs), colours) in surface
            .positions
            .chunks(3)
            .zip(surface.uvs.chunks(3))
            .zip(surface.colours.chunks(3))
        {
            let normal = (triangle[1] - triangle[0])
                .cross(triangle[2] - triangle[0])
                .normalize();
            // Its sides across the wall: the wall's face, its colour and, at each point,
            // the texel the wall wore there, give or take whole copies of it.
            let across = laid.turn * normal;
            let wall_face = normal.z.abs() > 0.99;
            if wall_face {
                faces += 1;
                assert!(
                    (colours[0][0] - 0.8).abs() < 1e-4,
                    "its face {:?}",
                    colours[0]
                );
                for (p, uv) in triangle.iter().zip(uvs) {
                    let stood = laid.at + laid.turn * (*p * size);
                    let wall_uv = crate::mesh::planar(stood, across, metres);
                    for k in 0..2 {
                        let off = uv[k] - wall_uv[k];
                        assert!(
                            (off - off.round()).abs() < 1e-3,
                            "at {stood} it wears {uv:?} where the wall wore {wall_uv:?}"
                        );
                    }
                }
            } else if normal.y.abs() > 0.99 || normal.x.abs() > 0.99 {
                splits += 1;
                assert!(
                    colours[0][0] < 0.8 * 0.8,
                    "a face it split along {:?}",
                    colours[0]
                );
            }
        }
    }
    assert!(
        faces > 0 && splits > 0 && turned > 0,
        "{faces} {splits} {turned}"
    );
}

#[test]
fn a_timber_wall_falls_as_lengths_along_its_grain_sawn_only_where_its_logs_end() {
    // Two log walls meeting at a corner, 16 and 9 cubes long, 6 courses high, one thick.
    let mut wall: Vec<(IVec3, Cube)> = Vec::new();
    for y in 0..6 {
        for x in 0..16 {
            wall.push((IVec3::new(x, y, 0), cube(CubeShape::Whole)));
        }
        for z in 1..9 {
            wall.push((IVec3::new(0, y, z), cube(CubeShape::Whole)));
        }
    }
    let made = burst(&wall, &|_| false, &|_| Breaks::Lengths, 1.0, 5000);
    let wholes: Vec<&Fragment> = made.iter().filter(|f| f.shape == Shape::Whole).collect();
    let cubes: f32 = wholes.iter().map(|f| (f.size.x / CELL_M).round()).sum();
    assert!(
        (cubes - wall.len() as f32).abs() < 1.0,
        "{cubes} of {}",
        wall.len()
    );
    // Each a length of its log, its grain along it, three cubes or more on average, never a
    // grid of one-cube blocks.
    let mean = cubes / wholes.len() as f32;
    assert!(mean >= 3.0, "lengths of {mean} cubes on average");
    assert!(
        wholes
            .iter()
            .all(|f| f.size.x >= f.size.y && f.size.x >= f.size.z)
    );
    // Sawn ends only at each course's log ends; every other end of a length snapped.
    let sawn: usize = wholes
        .iter()
        .map(|f| f.sawn.iter().filter(|&&s| s).count())
        .sum();
    assert!(sawn <= 6 * 4, "{sawn} sawn ends on 6 courses of two logs");
    assert!(
        wholes.len() * 2 - sawn >= wholes.len(),
        "most ends are snaps"
    );
    // Where it snapped it is drawn as split wood, never the log's sawn end.
    let mut kit = Kit {
        pattern: Pattern::Logs,
        ..Kit::default()
    };
    for (material, path) in [(Material::Log, "log"), (Material::LogEnd, "ends")] {
        kit.textures.insert(
            material,
            terra_build::kit::Texture {
                path: path.into(),
                metres: [1.0, 1.0],
                depth: 0.0,
                reflectance: terra_build::kit::matte(),
                strips: None,
                cells: 1,
                worn: Some("wood".into()),
            },
        );
    }
    let one_end = surfaces(
        &kit,
        &Cut::whole(
            Vec3::new(4.0 * CELL_M, CELL_M, CELL_M),
            [false, true],
            None,
            [1; 3],
        ),
    );
    let ends = &one_end.0[&Material::LogEnd];
    assert!(
        ends.normals.iter().all(|n| n.x > 0.9),
        "sawn only at its high end"
    );
    let snapped = surfaces(
        &kit,
        &Cut::whole(
            Vec3::new(4.0 * CELL_M, CELL_M, CELL_M),
            [false; 2],
            None,
            [1; 3],
        ),
    );
    assert!(!snapped.0.contains_key(&Material::LogEnd));
}

#[test]
fn a_whole_block_starts_over_the_cubes_it_was_whichever_way_it_is_turned() {
    // A stone wall along x, one along z, each big enough to fall in blocks of several cubes
    // (`chunks_of`), and timber posts standing up: each block turned
    // its own way (`grain`), all of them starting where their cubes stood.
    let box_of = |high: IVec3| -> Vec<(IVec3, Cube)> {
        (0..=high.y)
            .flat_map(|y| {
                (0..=high.x).flat_map(move |x| (0..=high.z).map(move |z| IVec3::new(x, y, z)))
            })
            .map(|at| (at, cube(CubeShape::Whole)))
            .collect()
    };
    let lengths = |_: Kind| Breaks::Lengths;
    // What fell, its far corner, how it breaks.
    type Case<'a> = (&'a str, IVec3, &'a dyn Fn(Kind) -> Breaks);
    let cases: [Case; 3] = [
        ("a wall along x", IVec3::new(29, 15, 2), &chunks),
        ("a wall along z", IVec3::new(2, 15, 29), &chunks),
        ("timber posts", IVec3::new(1, 9, 0), &lengths),
    ];
    for (name, high, breaks) in cases {
        let fell = box_of(high);
        let made = burst(&fell, &|_| false, breaks, 1.0, 5000);
        // Each block's box in the base's space, from its turned size.
        let boxes: Vec<(Vec3, Vec3)> = made
            .iter()
            .filter(|f| f.shape == Shape::Whole)
            .map(|f| {
                let half = (f.turn * f.size).abs() * 0.5;
                (f.at - half, f.at + half)
            })
            .collect();
        let most = (high + IVec3::ONE).as_vec3() * CELL_M;
        let eps = 1e-4;
        for (low, top) in &boxes {
            assert!(
                low.cmpge(Vec3::splat(-eps)).all() && top.cmple(most + eps).all(),
                "{name}: a block from {low} to {top} is outside the cubes, 0 to {most}"
            );
        }
        // Side by side as their cubes were: no two in each other.
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                let inside = (a.1.min(b.1) - a.0.max(b.0)).min_element();
                assert!(inside < eps, "{name}: two blocks overlap by {inside} m");
            }
        }
        // And they fill the cubes, each a hair smaller ([`SHRINK`]).
        let volume: f32 = boxes.iter().map(|(l, t)| (*t - *l).element_product()).sum();
        let whole = most.element_product() * SHRINK * SHRINK * SHRINK;
        assert!(
            (volume - whole).abs() < whole * 0.01,
            "{name}: blocks of {volume} m³ for cubes of {whole}"
        );
    }
}
