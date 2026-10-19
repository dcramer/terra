use super::*;
use terra_build::kit::Kits;
use terra_build::layout::lay_among;
use terra_build::roofer::{self, Style};
use terra_build::{BuildOp, Cube, Panel, STOREY_CUBES};

/// The kits the game ships, every one, with their dressed pieces, as it reads them.
/// The shipped kits these tests read, in this order.
const SHIPPED: &str = "logs hewn stone earth wattle shakes slate thatch greystone";

fn shipped() -> Kits {
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let read = |path: &str| {
        std::fs::read_to_string(assets.join(path)).map_err(|why| format!("{path}: {why}"))
    };
    let mut kits = Vec::new();
    for name in SHIPPED.split(' ') {
        let kit = Kit::read(&format!("data/kits/{name}.ron"), &read)
            .unwrap_or_else(|why| panic!("{name}: {why}"));
        kits.push(kit);
    }
    Kits(kits)
}

/// A kit's kind by its file's name: earth, wattle and daub.
fn named(kits: &Kits, id_of: &str) -> Kind {
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let read = |path: &str| {
        std::fs::read_to_string(assets.join(path)).map_err(|why| format!("{path}: {why}"))
    };
    let kit = Kit::read(&format!("data/kits/{id_of}.ron"), &read).unwrap();
    kits.of(kit.kind()).unwrap().kind()
}

fn kind(kits: &Kits, pattern: Pattern, profile: Profile) -> Kind {
    kits.0
        .iter()
        .find(|kit| {
            kit.pattern == pattern && (pattern == Pattern::Masonry || kit.timber.profile == profile)
        })
        .unwrap()
        .kind()
}

/// A roof of cubes put up over the columns from one to another, on the ground storey, in
/// the roof kind of the walls under it.
fn roof(base: &mut Base, kits: &Kits, min: [i32; 2], max: [i32; 2], style: Style, along_x: bool) {
    let footprint: BTreeSet<[i32; 2]> = (min[0]..=max[0])
        .flat_map(|x| (min[1]..=max[1]).map(move |z| [x, z]))
        .collect();
    let walls = base
        .cube(IVec3::new(min[0], 5, min[1]))
        .map_or(Kind::PLAIN, |cube| cube.kind);
    let kind = kits.roof_for(walls).unwrap();
    let height = roofer::field(&footprint, style, Some(along_x));
    for op in roofer::put_up(base, &footprint, 0, kind, &height) {
        base.apply(op);
    }
}

/// A room from one tile to another under a roof, with a door and a window, its corner at
/// `at`: placed so it crosses sections' edges.
fn house(base: &mut Base, kits: &Kits, at: [i32; 2], size: [i32; 2], walls: Kind, style: Style) {
    let [x, z] = at;
    let (min, max) = ([x, 0, z], [x + size[0], 0, z + size[1]]);
    let mut ops = vec![BuildOp::AddRoom {
        min,
        max,
        kind: walls,
    }];
    ops.extend((x + 3..=x + 4).map(|x| BuildOp::SetWall {
        tile: [x, 0, z + size[1]],
        panel: Some(Panel::Door),
        kind: walls,
    }));
    ops.extend((z + 2..=z + 3).map(|z| BuildOp::SetWall {
        tile: [x, 0, z],
        panel: Some(Panel::WINDOW),
        kind: walls,
    }));
    for op in ops {
        base.apply(op);
    }
    roof(base, kits, [x, z], [x + size[0], z + size[1]], style, true);
}

/// Bases of every kind and roof, and mixed, each across sections' edges, some on the
/// negative side of the grid, with footings, a carved wall and a long wall.
fn bases(kits: &Kits) -> Vec<Base> {
    let logs = kind(kits, Pattern::Logs, Profile::Round);
    let hewn = kind(kits, Pattern::Logs, Profile::Hewn);
    let stone = kind(kits, Pattern::Masonry, Profile::Round);
    let mut out = Vec::new();
    for (walls, style, at) in [
        (logs, Style::Gable, [10, 12]),
        (stone, Style::Hip, [-6, 9]),
        (hewn, Style::LeanTo, [12, -4]),
        (logs, Style::Hip, [-20, -20]),
    ] {
        let mut base = Base::default();
        house(&mut base, kits, at, [11, 9], walls, style);
        base.lay_footings(stone, |_| -40);
        out.push(base);
    }
    // Logs on a stone plinth, an L with a wing in stone, and a wall carved through.
    let mut mixed = Base::default();
    house(&mut mixed, kits, [9, 9], [9, 7], logs, Style::Gable);
    mixed.apply(BuildOp::Fill {
        min: [9, 0, 9],
        max: [18, 1, 9],
        cube: Some(Cube::new(stone, Fill::Solid)),
    });
    mixed.apply(BuildOp::AddRoom {
        min: [9, 0, 16],
        max: [15, 0, 26],
        kind: stone,
    });
    roof(&mut mixed, kits, [9, 17], [15, 26], Style::Gable, false);
    for x in 0..40 {
        mixed.apply(BuildOp::SetWall {
            tile: [x - 20, 0, -3],
            panel: Some(Panel::Solid),
            kind: logs,
        });
    }
    mixed.apply(BuildOp::Fill {
        min: [-2, 2, -3],
        max: [3, 3, -3],
        cube: Some(Cube::new(logs, Fill::Open)),
    });
    mixed.lay_footings(stone, |column| -(column[0].rem_euclid(5) as i16) * 20);
    out.push(mixed);
    // The example base: rooms of two storeys.
    let mut example = Base::default();
    for op in terra_build::example(logs) {
        example.apply(op);
    }
    out.push(example);
    // Earth under a hip and wattle and daub under a gable (D236), each across a corner of
    // four sections; and an L of stone under hips, its valley across a section's edge.
    for (walls, style, at) in [
        (named(kits, "earth"), Style::Hip, [10, 10]),
        (named(kits, "wattle"), Style::Gable, [-5, -4]),
    ] {
        let mut base = Base::default();
        house(&mut base, kits, at, [11, 9], walls, style);
        base.lay_footings(stone, |_| -40);
        out.push(base);
    }
    let mut ell = Base::default();
    ell.apply(BuildOp::AddRoom {
        min: [8, 0, 8],
        max: [22, 0, 14],
        kind: stone,
    });
    ell.apply(BuildOp::AddRoom {
        min: [8, 0, 15],
        max: [14, 0, 26],
        kind: stone,
    });
    let footprint: BTreeSet<[i32; 2]> = (8..=22)
        .flat_map(|x| (8..=14).map(move |z| [x, z]))
        .chain((8..=14).flat_map(|x| (15..=26).map(move |z| [x, z])))
        .collect();
    let height = roofer::field(&footprint, Style::Hip, None);
    let kind = kits.roof_for(stone).unwrap();
    for op in roofer::put_up(&ell, &footprint, 0, kind, &height) {
        ell.apply(op);
    }
    ell.lay_footings(stone, |column| -(column[1].rem_euclid(4) as i16) * 15);
    out.push(ell);
    out
}

/// A section's pieces, cut from its base laid whole.
fn laid(section: &Section) -> Laid {
    lay(section, &Whole::lay(section.base, section.kits))
}

#[test]
fn a_section_cut_from_its_base_laid_whole_is_the_section_laid_alone() {
    let kits = shipped();
    let maps = Maps::default();
    for (index, base) in bases(&kits).iter().enumerate() {
        let whole = Whole::lay(base, &kits);
        let mut cut = 0;
        for at in drawable(base) {
            let section = Section {
                base,
                kits: &kits,
                maps: &maps,
                at: IVec3::from_array(at),
                progress: &Progress::Whole,
            };
            let laid = lay(&section, &whole);
            // Laid alone, kit by kit, from only what reaches the section.
            let (mut own, mut edge) = (Vec::new(), Vec::new());
            for kit in &kits.0 {
                let (alone, beyond) =
                    layout::lay_section_and_edge(base, kit, &kits, IVec3::from_array(at));
                own.extend(alone.pieces);
                edge.extend(
                    beyond
                        .into_iter()
                        .filter(|p| matches!(p, Piece::Block { .. } | Piece::Gable { .. })),
                );
            }
            cut += laid.pieces.len();
            assert_eq!(
                sorted(laid.pieces.iter().map(|p| p.1.clone())),
                sorted(own),
                "base {index}, section {at:?}"
            );
            assert_eq!(
                // Blocks: the lintels and sills a step further are there for their beds.
                sorted(
                    laid.edge
                        .iter()
                        .map(|p| p.1.clone())
                        .filter(|p| matches!(p, Piece::Block { .. } | Piece::Gable { .. }))
                ),
                sorted(edge),
                "base {index}, section {at:?}, a cube beyond it"
            );
        }
        assert!(cut > 0, "base {index} has pieces");
    }
}

fn sorted(pieces: impl IntoIterator<Item = Piece>) -> Vec<String> {
    let mut out: Vec<String> = pieces.into_iter().map(|p| format!("{p:?}")).collect();
    out.sort();
    out
}

#[test]
fn every_piece_belongs_to_one_section_and_laying_a_section_lays_exactly_its_own() {
    let kits = shipped();
    for (index, base) in bases(&kits).iter().enumerate() {
        for kit in &kits.0 {
            let all = lay_among(base, kit, &kits).pieces;
            let mut by_section: BTreeMap<[i32; 3], Vec<Piece>> = BTreeMap::new();
            for piece in &all {
                by_section
                    .entry(piece.section().to_array())
                    .or_default()
                    .push(piece.clone());
            }
            // Every section a piece is in is one the base may draw in.
            let drawable = drawable(base);
            for section in by_section.keys() {
                assert!(drawable.contains(section), "base {index}: {section:?}");
            }
            let mut sections: BTreeSet<[i32; 3]> = by_section.keys().copied().collect();
            sections.extend(base.store().sections().map(|s| s.to_array()));
            let mut laid = 0;
            for section in sections {
                let own = layout::lay_section(base, kit, &kits, IVec3::from_array(section));
                laid += own.pieces.len();
                assert_eq!(
                    sorted(own.pieces),
                    sorted(by_section.remove(&section).unwrap_or_default()),
                    "base {index}, kind {:?}, section {section:?}",
                    kit.kind()
                );
            }
            assert_eq!(laid, all.len(), "base {index}: each piece once");
        }
    }
}

#[test]
fn a_piece_is_owned_by_the_cube_its_middle_is_in_exactly() {
    let log = |from: [i32; 3], to: [i32; 3]| Piece::Log {
        from: layout::Place(from),
        to: layout::Place(to),
        split: layout::Split::Whole,
        seed: 0,
        chinking: None,
        lean: [0, 0],
        lift: [0, 0],
        role: None,
        outside: None,
    };
    let t = layout::TICKS;
    // A log from the middle of cube 2 to the middle of cube 8 has its middle in 5; one
    // from cube 12 to 20, in 16, the next section.
    let along = log([2 * t + t / 2, 64, 128], [8 * t + t / 2, 64, 128]);
    assert_eq!(along.anchor(), IVec3::new(5, 0, 0));
    assert_eq!(along.section(), IVec3::ZERO);
    let across = log([12 * t + t / 2, 64, 128], [20 * t + t / 2, 64, 128]);
    assert_eq!(across.section(), IVec3::new(1, 0, 0));
    // On the line between two sections, the one after it; below the origin, the one below.
    assert_eq!(
        log([15 * t, 0, 0], [17 * t, 0, 0]).section(),
        IVec3::new(1, 0, 0)
    );
    assert_eq!(
        log([-3 * t, 0, 0], [-t, 0, 0]).anchor(),
        IVec3::new(-2, 0, 0)
    );
    assert_eq!(log([-t, 0, 0], [0, 0, 0]).section(), IVec3::new(-1, 0, 0));
}

/// Every triangle of a set of surfaces, each as its corners' bits, sorted, with their
/// normals in that order.
fn triangles<'a>(surfaces: impl IntoIterator<Item = &'a Surface>) -> Vec<([u32; 9], [Vec3; 3])> {
    let mut out = Vec::new();
    for surface in surfaces {
        let all = surface.indexed();
        for t in all.indices.chunks_exact(3) {
            let mut corners: Vec<([u32; 3], Vec3)> = t
                .iter()
                .map(|&i| {
                    let (p, n) = (all.positions[i as usize], all.normals[i as usize]);
                    ([p.x, p.y, p.z].map(f32::to_bits), n)
                })
                .collect();
            corners.sort_by_key(|c| c.0);
            let at: Vec<u32> = corners.iter().flat_map(|c| c.0).collect();
            out.push((
                at.try_into().unwrap(),
                [corners[0].1, corners[1].1, corners[2].1],
            ));
        }
    }
    // Back to back, the same corners face both ways: told apart by which way, roughly.
    let way =
        |t: &([u32; 9], [Vec3; 3])| t.1.map(|n| (n * 4.0).round().to_array().map(|c| c as i32));
    out.sort_by(|a: &([u32; 9], [Vec3; 3]), b| a.0.cmp(&b.0).then_with(|| way(a).cmp(&way(b))));
    out
}

#[test] // Slow: minutes.
fn the_sections_merged_are_the_sum_of_the_pieces() {
    let kits = shipped();
    let maps = Maps::default();
    let bases = bases(&kits);
    for base in &bases {
        // Each section drawn, near.
        let mut sections: BTreeSet<[i32; 3]> = BTreeSet::new();
        for kit in &kits.0 {
            sections.extend(
                lay_among(base, kit, &kits)
                    .pieces
                    .iter()
                    .map(|p| p.section().to_array()),
            );
        }
        let mut merged: Vec<Surface> = Vec::new();
        for section in &sections {
            let section = Section {
                base,
                kits: &kits,
                maps: &maps,
                at: IVec3::from_array(*section),
                progress: &Progress::Whole,
            };
            let drawn = draw(&section, &laid(&section), Reach::Near);
            merged.extend(drawn.near.into_values());
        }
        // The pieces drawn one by one, and each block kind's cubes as one surface.
        let mut whole: Vec<Surface> = Vec::new();
        for kit in &kits.0 {
            let pieces = lay_among(base, kit, &kits).pieces;
            let mut each = Surfaces::default();
            for piece in &pieces {
                crate::draw::draw(piece, kit, None, &mut each);
            }
            whole.extend(each.0.into_values());
            if kit.pattern == Pattern::Masonry {
                let cubes: BTreeSet<[i32; 3]> = pieces
                    .iter()
                    .filter_map(|p| match p {
                        Piece::Block { cube } | Piece::Gable { cube, .. } => Some(cube.to_array()),
                        _ => None,
                    })
                    .collect();
                let gables: BTreeMap<[i32; 3], ([[i32; 3]; 3], layout::Carried)> = pieces
                    .iter()
                    .filter_map(|p| match p {
                        Piece::Gable {
                            cube,
                            under,
                            carried,
                        } => Some((cube.to_array(), (*under, *carried))),
                        _ => None,
                    })
                    .collect();
                let cut = |cube: IVec3, at: Vec3| {
                    gables
                        .get(&cube.to_array())
                        .map_or([f32::MIN, f32::MAX], |(under, carried)| {
                            layout::roofs::carried_cut(*carried, cube, under, at)
                        })
                };
                let hides = |at: IVec3| {
                    base.cube(at).is_some_and(|cube| {
                        cube.fill != Fill::Open
                            && cube.kind != kit.kind()
                            && !cube.kind.is_roof()
                            && (!base.in_wall(at) || cube.kind == Kind::PLAIN)
                    })
                };
                let flat = |_: [f32; 2]| 0.5;
                // Bedded behind its lintels and sills, as a section's surface is.
                let beds: Vec<crate::dress::Bed> = pieces
                    .iter()
                    .filter_map(|p| crate::dress::bed(p, kit))
                    .collect();
                let bed = |p: Vec3| {
                    beds.iter()
                        .filter_map(|b| b.at(p))
                        .max_by(|a, b| a.0.total_cmp(&b.0))
                };
                let heights = Heights {
                    sides: &flat,
                    top: &flat,
                    bed: (!beds.is_empty()).then_some(&bed as crate::surface::BedAt),
                    bed_key: None,
                    hole: None,
                    holed: None,
                };
                let (mut faces, mut tops) = (Surface::default(), Surface::default());
                crate::surface::draw_cut(
                    &cubes,
                    &hides,
                    &block_look(kit, SURFACE_STEPS),
                    &heights,
                    &cut,
                    [1.0; 3],
                    &mut faces,
                    &mut tops,
                );
                whole.extend([faces, tops]);
            }
        }
        let (a, b) = (triangles(&merged), triangles(&whole));
        assert!(a.len() > 1000);
        assert_eq!(
            a.len(),
            b.len(),
            "as many triangles merged as the pieces have"
        );
        // The same triangles, corner for corner; their normals the same but where a cut's
        // edge meets a section's (D284): there each is summed from the triangles round it,
        // which a neighbour a step past the section's edge splits in another order, so they
        // agree to within a degree.
        assert!(
            a.iter().map(|t| t.0).eq(b.iter().map(|t| t.0)),
            "the same triangles and corners"
        );
        for (x, y) in a.iter().zip(&b) {
            for (m, n) in x.1.iter().zip(&y.1) {
                assert!(m.dot(*n) > 0.9998, "{m} against {n}");
            }
        }
    }
}

#[test]
fn far_off_a_section_is_far_fewer_triangles_and_one_surface() {
    let kits = shipped();
    let maps = Maps::default();
    let base = &bases(&kits)[0];
    let section = Section {
        base,
        kits: &kits,
        maps: &maps,
        at: IVec3::new(0, 0, 0),
        progress: &Progress::Whole,
    };
    let laid = laid(&section);
    let drawn = draw(&section, &laid, Reach::Near);
    let near: usize = drawn.near.values().map(Surface::triangle_count).sum();
    let far_off = draw(&section, &laid, Reach::Far);
    let (far, proxy) = (far_off.far.triangle_count(), far_off.proxy.triangle_count());
    assert!(near > 0 && far > 0 && proxy > 0);
    assert!(far * 5 < near, "far {far} against near {near}");
    // Past the far detail its hull, a fraction of the far detail's triangles.
    assert!(proxy * 5 < far, "proxy {proxy} against far {far}");
    // Far off, nothing finer is drawn; near, nothing coarser than it fades into.
    assert!(far_off.near.is_empty() && far_off.mid.is_empty());
    assert!(drawn.far.triangle_count() == 0 && drawn.proxy.triangle_count() == 0);
    let mid_off = draw(&section, &laid, Reach::Mid);
    assert!(mid_off.near.is_empty() && !mid_off.mid.is_empty());
    assert!(mid_off.far.triangle_count() > 0 && mid_off.proxy.triangle_count() == 0);
    // Close by, the near detail alone, the same as drawn near with its middle detail.
    let close = draw(&section, &laid, Reach::Close);
    assert!(close.mid.is_empty() && close.far.triangle_count() == 0);
    let close_near: usize = close.near.values().map(Surface::triangle_count).sum();
    assert_eq!(close_near, near);
}

#[test]
fn each_detail_drawn_alone_is_that_detail_as_drawn_with_the_rest() {
    let kits = shipped();
    let maps = Maps::default();
    let base = &bases(&kits)[0];
    let section = Section {
        base,
        kits: &kits,
        maps: &maps,
        at: IVec3::new(0, 0, 0),
        progress: &Progress::Whole,
    };
    let laid = laid(&section);
    let near_off = draw(&section, &laid, Reach::Near);
    let mid_off = draw(&section, &laid, Reach::Mid);
    let far_off = draw(&section, &laid, Reach::Far);
    let alone = |detail| {
        let mut drawn = draw_detail(&section, &laid, detail);
        for by in [
            std::mem::take(&mut drawn.near_by),
            std::mem::take(&mut drawn.mid_by),
        ] {
            for part in by {
                for (key, surface) in part {
                    let into = if detail == Detail::Near {
                        &mut drawn.near
                    } else {
                        &mut drawn.mid
                    };
                    into.entry(key).or_default().append(surface);
                }
            }
        }
        drawn
    };
    let (near, mid, far, proxy) = (
        alone(Detail::Near),
        alone(Detail::Mid),
        alone(Detail::Far),
        alone(Detail::Proxy),
    );
    assert!(!near.near.is_empty() && !mid.mid.is_empty());
    assert_eq!(
        triangles(near.near.values()),
        triangles(near_off.near.values())
    );
    assert_eq!(
        triangles(mid.mid.values()),
        triangles(near_off.mid.values())
    );
    assert_eq!(triangles(mid.mid.values()), triangles(mid_off.mid.values()));
    assert_eq!(triangles([&far.far]), triangles([&mid_off.far]));
    assert_eq!(triangles([&proxy.proxy]), triangles([&far_off.proxy]));
    // Each holds its own detail and nothing of another.
    assert!(near.mid.is_empty() && near.far.triangle_count() == 0);
    assert!(mid.near.is_empty() && mid.far.triangle_count() == 0);
    assert!(far.near.is_empty() && far.mid.is_empty() && far.proxy.triangle_count() == 0);
    assert!(proxy.far.triangle_count() == 0 && proxy.near.is_empty());
}

#[test]
fn a_plan_is_drawn_as_ghosts_a_piece_each_and_the_next_lit() {
    let kits = shipped();
    let maps = Maps::default();
    let base = &bases(&kits)[0];
    let all: usize = kits
        .0
        .iter()
        .map(|kit| lay_among(base, kit, &kits).pieces.len())
        .sum();
    let mut ghosts = 0;
    let mut lit = 0;
    let sections: BTreeSet<[i32; 3]> = kits
        .0
        .iter()
        .flat_map(|kit| lay_among(base, kit, &kits).pieces)
        .map(|p| p.section().to_array())
        .collect();
    for at in sections {
        let section = Section {
            base,
            kits: &kits,
            maps: &maps,
            at: IVec3::from_array(at),
            progress: &Progress::Fraction(0.0),
        };
        let drawn = draw(&section, &laid(&section), Reach::Near);
        assert!(drawn.near.values().all(|s| s.triangle_count() == 0) || drawn.near.is_empty());
        ghosts += drawn.ghosts.len();
        lit += drawn
            .next
            .0
            .values()
            .map(Surface::triangle_count)
            .sum::<usize>();
        for ghost in &drawn.ghosts {
            assert!(ghost.size.min_element() > 0.0 && ghost.size.max_element() < 12.0);
        }
    }
    // Every piece but blocks, gables and doors is a ghost, and the first is lit.
    assert!(ghosts > all / 2, "{ghosts} of {all}");
    assert!(lit > 0);
}

#[test]
fn changing_a_cube_marks_its_section_and_its_neighbours_on_an_edge() {
    let mut base = Base::default();
    base.take_dirty();
    let stone = Cube::new(Kind(2), Fill::Solid);
    base.apply(BuildOp::Fill {
        min: [5, 5, 5],
        max: [5, 5, 5],
        cube: Some(stone),
    });
    assert_eq!(base.take_dirty(), BTreeSet::from([[0, 0, 0]]));
    base.apply(BuildOp::Fill {
        min: [16, 0, 15],
        max: [16, 0, 15],
        cube: Some(stone),
    });
    assert_eq!(
        base.take_dirty(),
        BTreeSet::from([[1, 0, 0], [0, 0, 0], [1, -1, 0], [1, 0, 1]])
    );
}

#[test] // Slow: minutes.
fn whatever_changes_a_section_is_in_what_a_change_redraws() {
    let kits = shipped();
    let logs = kind(&kits, Pattern::Logs, Profile::Round);
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    let maps = Maps::default();
    let laid = |base: &Base, at: IVec3| {
        let section = Section {
            base,
            kits: &kits,
            maps: &maps,
            at,
            progress: &Progress::Whole,
        };
        laid(&section).key()
    };
    // A long building, then changes far from parts of it: a cube at one end, a window,
    // a wall's kind, a roof.
    let mut base = Base::default();
    house(&mut base, &kits, [2, 2], [40, 7], logs, Style::Gable);
    // And a second building apart from it.
    house(&mut base, &kits, [2, 40], [7, 7], logs, Style::Hip);
    base.lay_footings(stone, |_| -40);
    base.take_dirty();
    // A roof's cube at its far end taken off, and put back as a corner.
    let corner = base.cube(IVec3::new(42, 6, 2)).unwrap();
    let changes = [
        BuildOp::Fill {
            min: [42, 3, 2],
            max: [42, 3, 2],
            cube: None,
        },
        BuildOp::SetWall {
            tile: [30, 0, 2],
            panel: Some(Panel::WINDOW),
            kind: logs,
        },
        BuildOp::Fill {
            min: [2, 1, 2],
            max: [2, 5, 9],
            cube: Some(Cube::new(stone, Fill::Solid)),
        },
        BuildOp::Fill {
            min: [42, 6, 2],
            max: [42, 6, 2],
            cube: Some(Cube {
                shape: terra_build::Shape::OuterCorner(terra_build::Facing::South),
                ..corner
            }),
        },
    ];
    for op in changes {
        let before = base.clone();
        base.apply(op);
        let marked = base.take_dirty();
        let redrawn = affected(&base, &marked);
        let all = drawable(&before)
            .union(&drawable(&base))
            .copied()
            .collect::<BTreeSet<_>>();
        let mut changed = 0;
        for at in all {
            if laid(&before, IVec3::from_array(at)) != laid(&base, IVec3::from_array(at)) {
                changed += 1;
                assert!(
                    redrawn.contains(&at),
                    "{op:?} changed {at:?} but did not redraw it"
                );
            }
        }
        assert!(changed > 0, "{op:?} changed nothing");
        // The other building is not redrawn.
        assert!(!redrawn.contains(&[0, 0, 2]), "{op:?}");
    }
}

#[test]
fn far_off_the_textures_are_baked_into_the_colours() {
    let kits = shipped();
    let base = &bases(&kits)[1];
    let mut maps = Maps::default();
    let stone = kits
        .0
        .iter()
        .find(|k| k.pattern == Pattern::Masonry)
        .unwrap();
    let path = stone.textures[&Material::Masonry].path.clone();
    let red = crate::relief::Swatch::new(1, 1, vec![[1.0, 0.0, 0.0]]).unwrap();
    maps.colours.insert(path, std::sync::Arc::new(red));
    let section = Section {
        base,
        kits: &kits,
        maps: &maps,
        at: IVec3::new(-1, 0, 0),
        progress: &Progress::Whole,
    };
    let drawn = draw(&section, &laid(&section), Reach::Far);
    assert!(drawn.far.triangle_count() > 0);
    assert!(
        drawn
            .far
            .indexed()
            .colours
            .iter()
            .any(|c| c[0] > 0.5 && c[1] == 0.0 && c[2] == 0.0),
        "the stone's red baked in"
    );
    let _ = STOREY_CUBES;
}

#[test]
fn every_shape_is_drawn_in_every_kit_within_its_cut() {
    use terra_build::{Facing, Shape};
    let kits = shipped();
    let maps = Maps::default();
    let shapes = [
        Shape::UpperHalf,
        Shape::InvertedStair(Facing::East),
        Shape::InvertedSlope(Facing::South),
        Shape::OuterCorner(Facing::North),
        Shape::InnerCorner(Facing::West),
    ];
    for kit in kits.0.iter().filter(|kit| kit.pattern != Pattern::Roof) {
        for shape in shapes {
            // The shape on a cube of its kind, nothing round them.
            let mut base = Base::default();
            base.apply(BuildOp::Fill {
                min: [3, 0, 3],
                max: [3, 0, 3],
                cube: Some(Cube::new(kit.kind(), Fill::Solid)),
            });
            base.apply(BuildOp::Fill {
                min: [3, 1, 3],
                max: [3, 1, 3],
                cube: Some(Cube {
                    shape,
                    ..Cube::new(kit.kind(), Fill::Solid)
                }),
            });
            let section = Section {
                base: &base,
                kits: &kits,
                maps: &maps,
                at: IVec3::ZERO,
                progress: &Progress::Whole,
            };
            let laid = laid(&section);
            let low = IVec3::new(3, 1, 3).as_vec3() * CELL_M;
            // Within its cut, but for relief, bevels and trims proud of it.
            let within = |p: Vec3| {
                let u = ((p.x - low.x) / CELL_M).clamp(0.0, 1.0);
                let v = ((p.z - low.z) / CELL_M).clamp(0.0, 1.0);
                let slack = (CELL_M / 8.0 + 0.02) / CELL_M;
                let h = (p.y - low.y) / CELL_M;
                h >= shape.bottom(u, v) - slack && h <= shape.top(u, v) + slack
            };
            if kit.pattern == Pattern::Masonry {
                let drawn = draw(&section, &laid, Reach::Near);
                let surface = &drawn.near[&(kit.kind(), crate::mesh::Material::Masonry)];
                let mine: Vec<Vec3> = surface
                    .indexed()
                    .positions
                    .into_iter()
                    .filter(|p| p.y > low.y + 0.01)
                    .collect();
                assert!(!mine.is_empty(), "{shape:?} drew nothing");
                for p in mine {
                    assert!(within(p), "{shape:?}: {p} outside its cut");
                }
            } else {
                let planks: Vec<(Vec3, Vec3)> = laid
                    .pieces
                    .iter()
                    .filter_map(|(_, piece, _)| match piece {
                        Piece::Plank { from, to, .. } => Some((from.metres(), to.metres())),
                        _ => None,
                    })
                    .filter(|(a, b)| a.y > low.y - 0.01 && b.y > low.y - 0.01)
                    .collect();
                assert!(planks.len() >= 8, "{shape:?} in {}: {planks:?}", kit.id);
                for (a, b) in planks {
                    assert!(within(a) && within(b), "{shape:?}: a board {a} to {b}");
                }
            }
        }
    }
}

mod keys;

#[test] // Slow: minutes.
fn a_section_drawn_again_from_how_it_was_drawn_is_drawn_as_afresh() {
    let kits = shipped();
    let maps = Maps::default();
    let bases = bases(&kits);
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    let (mut taken, mut faces) = (0, 0);
    // What is changed: a wall's cube, the ground, or a roof's cube.
    #[derive(PartialEq)]
    enum Edit {
        Wall,
        Ground,
        Roof,
    }
    for (base, edit) in [
        (&bases[1], Edit::Wall),
        (&bases[4], Edit::Wall),
        (&bases[1], Edit::Ground),
        (&bases[4], Edit::Ground),
        (&bases[1], Edit::Roof),
        (&bases[4], Edit::Roof),
    ] {
        // A wall's cube a metre up taken out; the ground under the base changed, which
        // every piece's dirt rises from; or a roof's lowest cube taken out, which changes
        // where on the roof every point of it is, its masks' heights.
        let ground = edit == Edit::Ground;
        let wall = base
            .cubes()
            .map(|(at, _)| at)
            .find(|&at| at.y == 2 && base.in_wall(at) && base.cube(at).unwrap().kind == stone)
            .unwrap();
        let mut edited = base.clone();
        if ground {
            let ring = edited.ground_ring();
            edited.sample_ground(ring.into_iter().map(|column| terra_build::ground::Sample {
                column,
                height_cm: 30,
            }));
        } else {
            let at = if edit == Edit::Roof {
                base.cubes()
                    .filter(|(_, cube)| cube.kind.is_roof())
                    .min_by_key(|(at, _)| (at.y, at.x, at.z))
                    .map_or(wall, |(at, _)| at)
            } else {
                wall
            };
            edited.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: None,
            });
        }
        let sections: BTreeSet<[i32; 3]> =
            drawable(base).union(&drawable(&edited)).copied().collect();
        for at in sections {
            for reach in [Reach::Near, Reach::Mid] {
                let section = |base| Section {
                    base,
                    kits: &kits,
                    maps: &maps,
                    at: IVec3::from_array(at),
                    progress: &Progress::Whole,
                };
                let before = section(base);
                let mut last = Drawing::default();
                draw_again(&before, &laid(&before), reach, None, Some(&mut last));
                let after = section(&edited);
                let laid_after = laid(&after);
                let mut now = Drawing::default();
                let again = draw_again(&after, &laid_after, reach, Some(&last), Some(&mut now));
                let fresh = draw(&after, &laid_after, reach);
                assert!(again.near == fresh.near, "{at:?} {reach:?}: near differs");
                assert!(again.mid == fresh.mid, "{at:?} {reach:?}: mid differs");
                if edit == Edit::Wall {
                    taken += now.faces_taken();
                    faces += now.faces.len();
                }
            }
        }
    }
    // Most faces are far from the cube taken out, and kept.
    assert!(
        faces > 1000 && taken * 10 > faces * 8,
        "{taken} of {faces} kept"
    );
}

#[test]
fn a_base_laid_again_after_an_edit_cuts_into_the_sections_laid_whole() {
    let kits = shipped();
    let maps = Maps::default();
    let logs = kind(&kits, Pattern::Logs, Profile::Round);
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    // A claim of two buildings sections apart, as a player's may hold many.
    let mut claim = Base::default();
    house(&mut claim, &kits, [10, 12], [11, 9], logs, Style::Gable);
    house(&mut claim, &kits, [70, 12], [9, 9], stone, Style::Hip);
    claim.lay_footings(stone, |_| -40);
    claim.take_dirty();
    let walls: Vec<IVec3> = claim
        .cubes()
        .map(|(at, _)| at)
        .filter(|&at| at.y == 2 && claim.in_wall(at))
        .collect();
    let edits = [
        // A log wall's cube taken out, a stone one, and a cube built on the ground
        // between the two, which is a building of its own.
        BuildOp::Fill {
            min: walls[0].to_array(),
            max: walls[0].to_array(),
            cube: None,
        },
        BuildOp::Fill {
            min: walls[walls.len() - 1].to_array(),
            max: walls[walls.len() - 1].to_array(),
            cube: None,
        },
        BuildOp::Fill {
            min: [44, 0, 14],
            max: [45, 1, 14],
            cube: Some(Cube::new(stone, Fill::Solid)),
        },
    ];
    let mut before = claim.clone();
    let mut whole = Whole::lay(&before, &kits);
    for edit in edits {
        let mut after = before.clone();
        after.apply(edit);
        let marked = after.take_dirty();
        let again = whole.lay_again(&before, &after, &kits, &marked);
        let afresh = Whole::lay(&after, &kits);
        let mut cut = 0;
        for at in drawable(&after).union(&drawable(&before)) {
            let section = Section {
                base: &after,
                kits: &kits,
                maps: &maps,
                at: IVec3::from_array(*at),
                progress: &Progress::Whole,
            };
            let (a, b) = (lay(&section, &again), lay(&section, &afresh));
            cut += b.pieces.len();
            assert!(a == b, "{edit:?}: section {at:?} is not as laid whole");
        }
        assert!(cut > 1000);
        (before, whole) = (after, again);
    }
}

#[test] // Slow: minutes.
fn a_gable_is_cut_in_a_straight_line_to_the_roofs_deck_with_no_gap_under_it() {
    // Every block kind's gable under every roof kind, drawn as the game draws it.
    let kits = shipped();
    let maps = Maps::default();
    let walls: Vec<Kind> = kits
        .0
        .iter()
        .filter(|kit| kit.pattern == Pattern::Masonry)
        .map(|kit| kit.kind())
        .collect();
    let roofs: Vec<Kind> = kits
        .0
        .iter()
        .filter(|kit| kit.pattern == Pattern::Roof)
        .map(|kit| kit.kind())
        .collect();
    assert!(walls.len() >= 3 && roofs.len() == 3);
    // Five metres by four, the ridge along X two metres in: the deck up the west gable's
    // face is a storey up at the eaves, rising a metre a metre.
    let deck = |z: f32| terra_build::STOREY_M + z.min(4.0 - z);
    for &wall in &walls {
        for &covering in &roofs {
            let mut base = Base::default();
            base.apply(BuildOp::AddRoom {
                min: [0, 0, 0],
                max: [9, 0, 7],
                kind: wall,
            });
            let footprint: BTreeSet<[i32; 2]> =
                (0..10).flat_map(|x| (0..8).map(move |z| [x, z])).collect();
            let height = roofer::field(&footprint, Style::Gable, Some(true));
            for op in roofer::put_up(&base, &footprint, 0, covering, &height) {
                base.apply(op);
            }
            let mut sections: BTreeSet<[i32; 3]> = BTreeSet::new();
            for kit in &kits.0 {
                sections.extend(
                    lay_among(&base, kit, &kits)
                        .pieces
                        .iter()
                        .map(|p| p.section().to_array()),
                );
            }
            // The wall's surface on the west gable's face, a little either side of it: its
            // triangles there, and their corners.
            let mut face: Vec<Vec3> = Vec::new();
            let mut triangles: Vec<[Vec3; 3]> = Vec::new();
            for at in &sections {
                let section = Section {
                    base: &base,
                    kits: &kits,
                    maps: &maps,
                    at: IVec3::from_array(*at),
                    progress: &Progress::Whole,
                };
                let drawn = draw(&section, &laid(&section), Reach::Near);
                if let Some(surface) = drawn.near.get(&(wall, crate::mesh::Material::Masonry)) {
                    let on = |p: &Vec3| {
                        p.x.abs() < 0.1
                            && p.y > terra_build::STOREY_M - 0.25
                            && (0.0..=4.0).contains(&p.z)
                    };
                    let all = surface.indexed();
                    face.extend(all.positions.iter().copied().filter(on));
                    triangles.extend(
                        all.indices
                            .chunks_exact(3)
                            .map(|t| [0, 1, 2].map(|k| all.positions[t[k] as usize]))
                            .filter(|t| t.iter().any(on)),
                    );
                }
            }
            let name = format!("{} under {}", wall.0, covering.0);
            assert!(face.len() > 10, "{name}: {}", face.len());
            // Nothing over the deck, and up the whole gable, from eave to ridge either side,
            // the wall's top on it every few centimetres: no step and no gap under the roof.
            for p in &face {
                assert!(p.y <= deck(p.z) + 0.005, "{name}: {p} over the deck");
            }
            // How high the wall stands at a place along the gable: the highest its triangles'
            // edges reach there, as a face of few triangles follows a straight cut (D284).
            let top = |z: f32| {
                triangles
                    .iter()
                    .flat_map(|t| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])])
                    .filter(|(a, b)| (a.z - z) * (b.z - z) <= 0.0 && (a.z - b.z).abs() > 1e-6)
                    .map(|(a, b)| a.y + (b.y - a.y) * (z - a.z) / (b.z - a.z))
                    .fold(f32::MIN, f32::max)
            };
            let mut z = 0.2;
            while z < 3.8 {
                // At its edges' height there, or a corner a few centimetres either side.
                let corner = face
                    .iter()
                    .filter(|p| (p.z - z).abs() < 0.07)
                    .map(|p| deck(p.z) - p.y)
                    .fold(f32::MAX, f32::min);
                let gap = (deck(z) - top(z)).min(corner);
                assert!(
                    gap < 0.01,
                    "{name}: a gap of {gap} m under the roof at z {z}"
                );
                z += 0.05;
            }
        }
    }
}

#[test]
fn a_lintel_is_set_into_an_earth_wall_its_face_built_up_round_it() {
    // The earth kit with its pieces, as the game reads it.
    let kits = shipped();
    let earth = kits.of(named(&kits, "earth")).unwrap().clone();
    let maps = Maps::default();
    // A wall along X with a window in it.
    let mut base = Base::default();
    for x in 0..10 {
        base.apply(BuildOp::SetWall {
            tile: [x, 0, 0],
            panel: Some(if (4..=5).contains(&x) {
                Panel::WINDOW
            } else {
                Panel::Solid
            }),
            kind: earth.kind(),
        });
    }
    let pieces = lay_among(&base, &earth, &kits).pieces;
    // The lintel over it on the wall's north face.
    let bed = pieces
        .iter()
        .filter_map(|p| crate::dress::bed(p, &earth))
        .find(|b| b.over_opening && b.out.z < 0.0)
        .expect("a lintel");
    assert!(bed.face > 0.0 && bed.face <= CELL_M / 8.0, "{}", bed.face);
    let mut sections: BTreeSet<[i32; 3]> = BTreeSet::new();
    sections.extend(pieces.iter().map(|p| p.section().to_array()));
    let mut face: Vec<Vec3> = Vec::new();
    for at in &sections {
        let section = Section {
            base: &base,
            kits: &kits,
            maps: &maps,
            at: IVec3::from_array(*at),
            progress: &Progress::Whole,
        };
        let drawn = draw(&section, &laid(&section), Reach::Near);
        if let Some(surface) = drawn
            .near
            .get(&(earth.kind(), crate::mesh::Material::Masonry))
        {
            face.extend(
                surface
                    .indexed()
                    .positions
                    .into_iter()
                    .filter(|p| p.z.abs() < 0.1),
            );
        }
    }
    // Behind the lintel the earth is sunk behind its face, so none of it shows through; round
    // its top and ends it is built up proud of it, over its edges.
    let (mut behind, mut over) = (0, 0);
    for p in &face {
        let Some(inside) = bed.inside(p.with_z(0.0)) else {
            continue;
        };
        let out = -p.z;
        if inside > 0.05 {
            // Sunk behind the wall's own face, not only the lintel's.
            assert!(out < 0.0, "earth at {p} not sunk behind the lintel");
            behind += 1;
        } else if inside > -0.01 && out > bed.face {
            over += 1;
        }
    }
    assert!(
        behind > 20 && over > 3,
        "{behind} behind, {over} over its edges"
    );
}

#[test]
fn an_earth_windows_sill_is_drawn_once_where_its_rounding_pinches_it() {
    // An earth wall's window, its arrises rounded broadly: at the sill's ends the rounding
    // pinches the squares of its top into darts, which were split across the outside and
    // laid a triangle over their neighbours.
    let kits = shipped();
    let earth = kits.of(named(&kits, "earth")).unwrap().clone();
    let maps = Maps::default();
    let mut base = Base::default();
    for x in 0..10 {
        base.apply(BuildOp::SetWall {
            tile: [x, 0, 0],
            panel: Some(if (4..=5).contains(&x) {
                Panel::WINDOW
            } else {
                Panel::Solid
            }),
            kind: earth.kind(),
        });
    }
    let pieces = lay_among(&base, &earth, &kits).pieces;
    let mut sections: BTreeSet<[i32; 3]> = BTreeSet::new();
    sections.extend(pieces.iter().map(|p| p.section().to_array()));
    for at in &sections {
        let section = Section {
            base: &base,
            kits: &kits,
            maps: &maps,
            at: IVec3::from_array(*at),
            progress: &Progress::Whole,
        };
        let drawn = draw(&section, &laid(&section), Reach::Near);
        if let Some(surface) = drawn
            .near
            .get(&(earth.kind(), crate::mesh::Material::Floor))
        {
            let over = crate::inspect::self_overlap(surface);
            assert!(over < 1e-4, "{over} m² of the tops drawn twice");
        }
    }
}

#[test]
fn an_openings_head_is_sunk_behind_its_lintels_underside() {
    // A stone wall along X with a window in it, under lintels on both faces whose
    // undersides lie in the plane of the opening's head: the head's face is sunk behind
    // them, so the two are never drawn in one plane.
    let kits = shipped();
    let stone = kits.of(named(&kits, "stone")).unwrap().clone();
    let maps = Maps::default();
    let mut base = Base::default();
    for x in 0..10 {
        base.apply(BuildOp::SetWall {
            tile: [x, 0, 0],
            panel: Some(if (4..=5).contains(&x) {
                Panel::WINDOW
            } else {
                Panel::Solid
            }),
            kind: stone.kind(),
        });
    }
    let pieces = lay_among(&base, &stone, &kits).pieces;
    let bed = pieces
        .iter()
        .filter_map(|p| crate::dress::bed(p, &stone))
        .find(|b| b.over_opening)
        .expect("a lintel");
    let head = bed.from.y;
    let (mut sunk, mut flush) = (0, 0);
    let mut sections: BTreeSet<[i32; 3]> = BTreeSet::new();
    sections.extend(pieces.iter().map(|p| p.section().to_array()));
    for at in &sections {
        let section = Section {
            base: &base,
            kits: &kits,
            maps: &maps,
            at: IVec3::from_array(*at),
            progress: &Progress::Whole,
        };
        let drawn = draw(&section, &laid(&section), Reach::Near);
        let Some(surface) = drawn
            .near
            .get(&(stone.kind(), crate::mesh::Material::Masonry))
        else {
            continue;
        };
        // The head's points over the opening's middle, across the wall's depth.
        for p in surface.indexed().positions {
            if (p.x - 2.5).abs() > 0.3 || (p.y - head).abs() > 0.05 || !(0.1..0.4).contains(&p.z) {
                continue;
            }
            if (p.y - head).abs() < 2e-3 {
                flush += 1;
            } else if p.y > head + 0.005 {
                sunk += 1;
            }
        }
    }
    assert!(
        flush == 0 && sunk > 4,
        "{flush} in the lintels' plane, {sunk} sunk"
    );
}

/// Maps of rough height for every kit, as a real stone's or earth's: what pushes a block
/// surface's points about, so faults that only show with relief show.
fn rough(kits: &Kits) -> Maps {
    relief(kits, 64, |i| {
        (terra_gen::hash::of(&[i as u32, 7]) >> 24) as u8
    })
}

/// Maps a side square of the height each of their texels is given, for every kit.
fn relief(kits: &Kits, side: usize, height: impl Fn(usize) -> u8) -> Maps {
    use crate::relief::{Relief, cored_by, shaped_by, topped_by};
    let heights = (0..side * side).map(height).collect();
    let relief = std::sync::Arc::new(Relief::new(side, side, heights).unwrap());
    let mut maps = Maps::default();
    for kit in &kits.0 {
        for path in [shaped_by(kit), topped_by(kit), cored_by(kit)]
            .into_iter()
            .flatten()
        {
            maps.heights.insert(path.to_string(), relief.clone());
        }
    }
    maps
}

// Slow, about a minute: a sweep.
#[test]
fn every_junction_case_is_drawn_without_a_fault() {
    // The junctions family (`junctions`, docs/building-voxels.md, gaps 3, 7 and 9) in
    // logs, hewn timber and stone, each under its roof, the wing of the L in the other
    // roof kind, on footings over level ground: nothing the mesh checker finds.
    let kits = shipped();
    let (shakes, slate) = (named(&kits, "shakes"), named(&kits, "slate"));
    let maps = Maps::default();
    let mut found = Vec::new();
    for walls in ["logs", "hewn", "stone"] {
        let kind = named(&kits, walls);
        let roof = kits.roof_for(kind).unwrap();
        let other = if roof == shakes { slate } else { shakes };
        for (row, ops) in terra_build::junctions::junctions(kind, roof, other) {
            let mut base = Base::from_shortcuts(&ops);
            if let Some(footing) = kits.footing() {
                base.lay_footings(footing.kind(), |_| 0);
            }
            let report = crate::inspect::inspect(&base, &kits, &maps);
            found.extend(
                report
                    .findings
                    .iter()
                    .map(|f| format!("{walls}, {row}: {:?} {:?} {}", f.fault, f.detail, f.what)),
            );
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}

#[test] // Slow: minutes.
fn cubes_of_a_kind_meeting_only_along_an_edge_each_dress_their_own_side() {
    // Stone cubes corner to corner (docs/building-voxels.md, gap 3), on level ground with
    // no footing, so each takes its own plinth: each turns round its own outside corner at
    // the shared edge and keeps to its own side of the plane halfway between them, never
    // lying in the other's dress there.
    let kits = shipped();
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    type Boxes = Vec<([i32; 3], [i32; 3])>;
    let arrangements: Vec<(&str, Boxes)> = vec![
        (
            "diag pair",
            vec![([0, 0, 0], [0, 0, 0]), ([1, 0, 1], [1, 0, 1])],
        ),
        (
            "diag pair up",
            vec![([0, 0, 0], [0, 0, 0]), ([1, 1, 0], [1, 1, 0])],
        ),
        (
            "diag walls",
            vec![([0, 0, 0], [3, 2, 0]), ([4, 0, 1], [7, 2, 1])],
        ),
        (
            "corbel",
            vec![([0, 0, 0], [5, 1, 0]), ([0, 2, 1], [5, 3, 1])],
        ),
        (
            "step",
            vec![
                ([0, 0, 0], [5, 0, 0]),
                ([0, 1, 1], [5, 1, 1]),
                ([0, 0, 2], [5, 2, 2]),
            ],
        ),
        (
            "checker",
            vec![
                ([0, 0, 0], [0, 0, 0]),
                ([1, 1, 0], [1, 1, 0]),
                ([0, 2, 0], [0, 2, 0]),
                ([1, 0, 1], [1, 0, 1]),
            ],
        ),
        (
            "l in plan",
            vec![([0, 0, 0], [2, 2, 0]), ([3, 0, 1], [3, 2, 3])],
        ),
    ];
    let mut fighting = Vec::new();
    for maps in [rough(&kits), Maps::default()] {
        for (name, boxes) in &arrangements {
            let mut base = Base::default();
            for &(min, max) in boxes {
                base.apply(BuildOp::Fill {
                    min,
                    max,
                    cube: Some(Cube::new(stone, Fill::Solid)),
                });
            }
            let report = crate::inspect::inspect(&base, &kits, &maps);
            fighting.extend(
                report
                    .findings
                    .iter()
                    .map(|f| format!("{name}: {:?} {:?} {}", f.fault, f.detail, f.what)),
            );
        }
    }
    assert!(fighting.is_empty(), "{fighting:#?}");
}

#[test]
fn no_stone_wall_or_its_dress_fights_itself_in_one_plane() {
    // A stone house under a gable, its walls cut at the roof's underside up its gables and
    // under its eaves, on footings; and a free wall a cube thick, quoined up both faces at
    // each end, so its end faces carry two corners' stones.
    let kits = shipped();
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    let mut base = Base::default();
    house(&mut base, &kits, [0, 0], [9, 7], stone, Style::Gable);
    base.apply(BuildOp::Fill {
        min: [16, 0, 2],
        max: [21, 3, 2],
        cube: Some(Cube::new(stone, Fill::Solid)),
    });
    base.lay_footings(stone, |[x, z]| (-x * 4 - z * 2) as i16);
    let maps = rough(&kits);
    for at in drawable(&base) {
        let section = Section {
            base: &base,
            kits: &kits,
            maps: &maps,
            at: IVec3::from_array(at),
            progress: &Progress::Whole,
        };
        let drawn = draw(&section, &laid(&section), Reach::Near);
        for material in [Material::Masonry, Material::Dressed, Material::Floor] {
            let Some(surface) = drawn.near.get(&(stone, material)) else {
                continue;
            };
            // Under a square centimetre, in all: no more than rounding where faces meet.
            let fighting = crate::inspect::self_overlap(surface);
            assert!(
                fighting < 1e-4,
                "section {at:?}, {material:?}: {:.0} cm² in one plane, overlapping",
                fighting * 1e4
            );
        }
    }
}

#[test]
fn plinths_meeting_at_a_passages_corners_keep_out_of_one_another() {
    // A stone wall three cubes thick, a passage a cube wide through it over its footing:
    // the plinths along the passage's sides, on its floor, meet the wall's face's at its
    // mouths, each ended square, and neither may lie in the other inside the wall.
    let kits = shipped();
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    let mut base = Base::default();
    base.apply(BuildOp::Fill {
        min: [0, -2, 0],
        max: [2, 5, 8],
        cube: Some(Cube::new(stone, Fill::Solid)),
    });
    base.apply(BuildOp::Fill {
        min: [0, 0, 4],
        max: [2, 1, 4],
        cube: None,
    });
    let height = |[x, _]: [i32; 2]| (24 - x * 4) as i16;
    let ring = base.ground_ring();
    base.sample_ground(ring.into_iter().map(|column| terra_build::ground::Sample {
        column,
        height_cm: height(column),
    }));
    base.lay_footings(stone, height);
    let maps = rough(&kits);
    for at in drawable(&base) {
        let section = Section {
            base: &base,
            kits: &kits,
            maps: &maps,
            at: IVec3::from_array(at),
            progress: &Progress::Whole,
        };
        let drawn = draw(&section, &laid(&section), Reach::Near);
        let Some(surface) = drawn.near.get(&(stone, Material::Dressed)) else {
            continue;
        };
        let fighting = crate::inspect::self_overlap(surface);
        assert!(
            fighting < 1e-4,
            "section {at:?}: {:.0} cm² of dressed stone in one plane, overlapping",
            fighting * 1e4
        );
    }
}

#[test]
fn a_log_floor_in_a_stone_tower_stops_at_its_walls_faces() {
    // A stone tower, its walls four cubes thick, floored a storey up in logs inside, as the
    // castle's north-west one is (`castle`), where it stands: the boards stop at the
    // stone's faces, and run on under none of its walls as under logs set in from their
    // cubes' faces by their round; and their tops stand clear of the flats of the stone's
    // band lying on them.
    let kits = shipped();
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    let logs = kind(&kits, Pattern::Logs, Profile::Round);
    let mut base = Base::default();
    base.apply(BuildOp::Fill {
        min: [-84, 14, -164],
        max: [-69, 20, -149],
        cube: Some(Cube::new(stone, Fill::Solid)),
    });
    base.apply(BuildOp::Fill {
        min: [-80, 14, -160],
        max: [-73, 20, -153],
        cube: None,
    });
    base.apply(BuildOp::Fill {
        min: [-80, 17, -160],
        max: [-73, 17, -153],
        cube: Some(Cube::new(logs, Fill::Solid)),
    });
    // Relief at its full range, a texel's noise, as the checker draws with
    // (`examples/artifacts`): the band's flats pushed as far down as they go.
    let maps = relief(&kits, 256, |i| ((i * 2_654_435_761) >> 13) as u8);
    let report = crate::inspect::inspect(&base, &kits, &maps);
    let fighting: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.fault == crate::inspect::Fault::Coplanar)
        .map(|f| &f.what)
        .collect();
    assert!(fighting.is_empty(), "{fighting:#?}");
}

#[test]
fn a_border_ending_where_its_floor_does_is_not_in_the_floors_face() {
    // A stone wall with a floor of stone a storey high along its north face, as the castle's
    // wall-walk along its curtain, the floor ending in a face at the west where the wall runs
    // on: the border along the wall's foot is sunk into the floor's top, and its end there
    // is not in the floor's end face.
    let kits = shipped();
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    let mut base = Base::default();
    base.apply(BuildOp::Fill {
        min: [0, 0, 0],
        max: [5, 1, 2],
        cube: Some(Cube::new(stone, Fill::Solid)),
    });
    base.apply(BuildOp::Fill {
        min: [-2, 0, 3],
        max: [5, 3, 3],
        cube: Some(Cube::new(stone, Fill::Solid)),
    });
    let report = crate::inspect::inspect(&base, &kits, &rough(&kits));
    let fighting: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.fault == crate::inspect::Fault::Coplanar)
        .map(|f| format!("{:?} {}", f.detail, f.what))
        .collect();
    assert!(fighting.is_empty(), "{fighting:#?}");
}

#[test]
fn a_stone_stairs_noses_keep_out_of_the_flight_over_them() {
    // A flight of stone stairs two cubes wide rising north, each stair cut to two steps on
    // the whole cubes under it: each step's nose lies on its tread and reaches back no
    // further than the step does, not into the stone of the next stair up.
    let kits = shipped();
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    let mut base = Base::default();
    for step in 0..3 {
        if step > 0 {
            base.apply(BuildOp::Fill {
                min: [0, 0, -step],
                max: [1, step - 1, -step],
                cube: Some(Cube::new(stone, Fill::Solid)),
            });
        }
        base.apply(BuildOp::Fill {
            min: [0, step, -step],
            max: [1, step, -step],
            cube: Some(Cube {
                shape: terra_build::Shape::Stair(terra_build::Facing::North),
                ..Cube::new(stone, Fill::Solid)
            }),
        });
    }
    let report = crate::inspect::inspect(&base, &kits, &rough(&kits));
    let buried: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.fault == crate::inspect::Fault::Buried)
        .map(|f| format!("{:?} {}", f.detail, f.what))
        .collect();
    assert!(buried.is_empty(), "{buried:#?}");
}

#[test] // Slow: minutes.
fn an_edit_laid_again_only_as_far_as_it_reaches_is_as_laid_whole() {
    // Town buildings of every kit and plan, two storeys and decks among them, each edited
    // a cube at a time: one taken out of it, one put on its top, one beside it on the
    // ground. Laid again only as far as `reach` says, every section is as laid whole.
    let kits = shipped();
    let maps = Maps::default();
    let named = |name: &str| {
        let index = SHIPPED.split(' ').position(|n| n == name).unwrap();
        kits.0[index].kind()
    };
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let read = |path: &str| {
        std::fs::read_to_string(assets.join(path)).map_err(|why| format!("{path}: {why}"))
    };
    let kinds =
        terra_build::village::Palette::read(&read, &|name| Some(named(name)), &kits).unwrap();
    // And a long hall of each kind of wall, five sections long, a cube taken out at its
    // end and one added there.
    let mut bases: Vec<(String, Base)> = terra_build::village::town(3, 14, &kinds)
        .into_iter()
        .map(|building| {
            let mut base = Base::default();
            base.apply_shortcuts(&building.ops);
            (building.name, base)
        })
        .collect();
    for walls in ["logs", "hewn", "stone", "earth", "wattle"] {
        let mut base = Base::default();
        house(
            &mut base,
            &kits,
            [-3, 2],
            [72, 9],
            named(walls),
            Style::Gable,
        );
        bases.push((format!("a long hall of {walls}"), base));
    }
    // And a walled yard of stone five sections a side, edited at a corner: the far corner
    // is on neither of the rows through it.
    let mut yard = Base::default();
    yard.apply(BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [70, 0, 70],
        kind: named("stone"),
    });
    bases.push(("a walled yard".to_string(), yard));
    let mut random = terra_build::layout::Random(7);
    let (mut edits, mut kept) = (0, 0);
    for (name, mut before) in bases {
        before.take_dirty();
        let mut whole = Whole::lay(&before, &kits);
        let cubes: Vec<IVec3> = before.cubes().map(|(at, _)| at).collect();
        let top = cubes.iter().copied().max_by_key(|at| at.y).unwrap();
        let low = cubes.iter().copied().reduce(IVec3::min).unwrap();
        let wall = cubes[(random.unit() * cubes.len() as f32) as usize % cubes.len()];
        let kind = before.cube(wall).unwrap().kind;
        for (at, cube) in [
            (wall, None),
            (top + IVec3::Y, Some(Cube::new(kind, Fill::Solid))),
            (
                low - IVec3::new(2, 0, 0),
                Some(Cube::new(kind, Fill::Solid)),
            ),
        ] {
            let mut after = before.clone();
            after.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube,
            });
            let marked = after.take_dirty();
            let again = whole.lay_again(&before, &after, &kits, &marked);
            let afresh = Whole::lay(&after, &kits);
            let reached: usize = reach(&before, &after, &kits, &marked)
                .iter()
                .map(BTreeSet::len)
                .sum();
            let all: BTreeSet<[i32; 3]> = drawable(&after)
                .union(&drawable(&before))
                .copied()
                .collect();
            for at_section in &all {
                let section = Section {
                    base: &after,
                    kits: &kits,
                    maps: &maps,
                    at: IVec3::from_array(*at_section),
                    progress: &Progress::Whole,
                };
                let (x, y) = (lay(&section, &again), lay(&section, &afresh));
                assert!(
                    x == y,
                    "{name}: {at} set to {cube:?}: section {at_section:?} is not as laid whole"
                );
            }
            edits += 1;
            kept += all.len() - reached.min(all.len());
            (before, whole) = (after, again);
        }
    }
    assert_eq!(edits, 60);
    // Not every edit lays the whole building again.
    assert!(kept > 0, "every edit reached every section");
}

#[test]
fn a_plan_so_far_through_lights_one_piece_not_one_a_kit() {
    // A log house on stone footings under shakes: three kits, each with a next piece, of
    // which only the lowest is the builder's next; the others lay about the plan, lit.
    let kits = shipped();
    let logs = kind(&kits, Pattern::Logs, Profile::Round);
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    let mut base = Base::default();
    house(&mut base, &kits, [0, 0], [9, 7], logs, Style::Gable);
    base.lay_footings(stone, |[x, z]| (-x * 4 - z * 2) as i16);
    let whole = Whole::lay(&base, &kits);
    for fraction in [0.0, 0.3] {
        let progress = Progress::Fraction(fraction);
        let mut lit = BTreeSet::new();
        for at in drawable(&base) {
            let section = Section {
                base: &base,
                kits: &kits,
                maps: &Maps::default(),
                at: IVec3::from_array(at),
                progress: &progress,
            };
            for (index, _, stage) in lay(&section, &whole).pieces {
                if stage == Stage::Next {
                    lit.insert(index);
                }
            }
        }
        assert_eq!(lit.len(), 1, "{fraction}: kits lit {lit:?}");
    }
}

#[test]
fn a_stone_house_near_keeps_to_the_block_kinds_budget() {
    let kits = shipped();
    let maps = Maps::default();
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    let base = &bases(&kits)[1];
    let (mut surface, mut dress, mut bytes) = (0usize, 0usize, 0usize);
    for at in drawable(base) {
        let section = Section {
            base,
            kits: &kits,
            maps: &maps,
            at: IVec3::from_array(at),
            progress: &Progress::Whole,
        };
        let drawn = draw(&section, &laid(&section), Reach::Near);
        for ((of, material), drawn) in &drawn.near {
            if *of != stone {
                continue;
            }
            let all = drawn.indexed();
            match material {
                Material::Masonry | Material::Floor => surface += all.triangle_count(),
                _ => dress += all.triangle_count(),
            }
            // A corner as the game uploads it, position to texture place, and an index.
            bytes += all.positions.len() * (12 + 12 + 16 + 16 + 8) + all.indices.len() * 4;
        }
    }
    // Its stone, and how much of its surface is open to the air.
    let cubes: Vec<IVec3> = base
        .cubes()
        .filter(|(_, c)| c.kind == stone && c.is_in())
        .map(|(at, _)| at)
        .collect();
    let open = cubes
        .iter()
        .flat_map(|&at| terra_build::Side::ALL.map(|side| at + side.offset()))
        .filter(|&next| base.cube(next).is_none_or(|c| c.kind != stone))
        .count();
    let area = open as f32 * CELL_M * CELL_M;
    let per_metre = surface as f32 / area;
    let per_cube = (surface + dress) as f32 / cubes.len() as f32;
    let mib = bytes as f32 / (1024.0 * 1024.0);
    assert!(
        per_metre <= crate::surface::NEAR_TRIANGLES_A_SQUARE_METRE,
        "{per_metre} triangles a square metre of its surface"
    );
    assert!(
        per_cube <= NEAR_TRIANGLES_A_CUBE,
        "{per_cube} triangles a cube with its dress"
    );
    assert!(mib <= NEAR_MIB_A_BASE, "{mib} MiB");
}

#[test]
fn a_log_house_is_drawn_with_its_logs_worn_out_of_its_room() {
    // The shipped log kit's face names what it wears down to: its logs carry their wear to
    // the drawing, more of it on the walls' outer faces than their inner.
    let kits = shipped();
    let logs = named(&kits, "logs");
    let mut base = Base::default();
    house(&mut base, &kits, [0, 0], [7, 6], logs, Style::Gable);
    assert!(base.room_count() > 0, "no room");
    let maps = Maps::default();
    let (mut outer, mut inner) = ((0.0, 0), (0.0, 0));
    for at in drawable(&base) {
        let section = Section {
            base: &base,
            kits: &kits,
            maps: &maps,
            at: IVec3::from_array(at),
            progress: &Progress::Whole,
        };
        let drawn = draw(&section, &laid(&section), Reach::Near);
        let Some(surface) = drawn.near.get(&(logs, Material::Log)) else {
            continue;
        };
        let face = surface.indexed();
        let middle = Vec3::new(1.75, 0.0, 1.5);
        for (i, (p, n)) in face.positions.iter().zip(&face.normals).enumerate() {
            // Facing level, away from the room's middle or toward it.
            if n.y.abs() > 0.5 {
                continue;
            }
            let wear = face.wear.get(i).copied().unwrap_or(0.0);
            let out = (*p - middle).with_y(0.0);
            let side = if out.dot(n.with_y(0.0)) > 0.0 {
                &mut outer
            } else {
                &mut inner
            };
            side.0 += wear;
            side.1 += 1;
        }
    }
    let (outer, inner) = (
        outer.0 / outer.1.max(1) as f32,
        inner.0 / inner.1.max(1) as f32,
    );
    assert!(outer > 0.1, "outer faces worn {outer}");
    assert!(outer > inner * 1.3, "outer {outer}, inner {inner}");
}

#[test]
fn a_kit_whose_kind_a_base_lacks_lays_nothing_there() {
    // What lets a base be laid by its own kits alone, however many materials there are.
    let kits = shipped();
    for base in bases(&kits) {
        let kinds: BTreeSet<Kind> = base.cubes().map(|(_, cube)| cube.kind).collect();
        for kit in &kits.0 {
            if !kinds.contains(&kit.kind()) {
                let laid = layout::lay_among(&base, kit, &kits).pieces;
                assert!(laid.is_empty(), "{:?}: {} pieces", kit.kind(), laid.len());
            }
        }
    }
}

#[test] // Slow: minutes.
fn kits_read_again_with_one_taken_out_added_or_reordered_lay_and_draw_without_fault() {
    // A material file added, taken out or edited while the game runs reads the kits again,
    // while bases laid by the old ones are still being cut and drawn: each base's sections
    // are cut from its whole laid by the old kits with the new, and laid again from it.
    let kits = shipped();
    let maps = Maps::default();
    let mut fewer = kits.clone();
    let gone = fewer.0.remove(1).kind();
    let mut turned = kits.clone();
    turned.0.reverse();
    let mut more = kits.clone();
    let mut added = kits.0[0].clone();
    added.id = 999;
    let new = added.kind();
    more.0.insert(0, added);
    for base in bases(&kits) {
        let has_gone = base.cubes().any(|(_, cube)| cube.kind == gone);
        let old = Whole::lay(&base, &kits);
        for now in [&fewer, &turned, &more] {
            let fresh = Whole::lay(&base, now);
            let again = old.lay_again(&base, &base, now, &BTreeSet::new());
            assert!(again.laid_by(now) && !old.laid_by(now));
            for at in drawable(&base) {
                let section = Section {
                    base: &base,
                    kits: now,
                    maps: &maps,
                    at: IVec3::from_array(at),
                    progress: &Progress::Whole,
                };
                // Cut from the old whole with the new kits: no piece of a kit taken out,
                // none yet of one added, and, where the kits only moved or the one taken
                // out had no cubes there, what the new kits lay.
                let stale = lay(&section, &old);
                assert!(stale.pieces.iter().all(|p| now.0[p.0].kind() != new));
                if !(std::ptr::eq(now, &fewer) && has_gone) {
                    assert_eq!(stale.pieces, lay(&section, &fresh).pieces, "{at:?}");
                }
                draw(&section, &stale, Reach::Near);
                // Laid again from it, as laid whole by the new kits.
                assert_eq!(lay(&section, &again).pieces, lay(&section, &fresh).pieces);
            }
        }
    }
}

#[test]
fn a_stone_wall_is_holed_where_its_stones_have_fallen_and_flat_where_not_harmed() {
    // A stone wall a cube thick over a grid of stones in mortar, one cube of it all but
    // gone: its face there is pushed back a stone's depth over the stones gone, and the
    // cubes not near it keep their face.
    let kits = shipped();
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    let kit = kits.of(stone).unwrap();
    let Some(terra_build::kit::Breaks::Joints { depth, share }) =
        kit.damage.get(&Material::Masonry).copied()
    else {
        panic!("stone breaks at its joints");
    };
    const SIDE: usize = 256;
    let heights = (0..SIDE * SIDE)
        .map(|i| {
            let (x, y) = (i % SIDE, i / SIDE);
            // Its joints off the cubes' lines, so the hole's outline is not the grid's.
            if (x + 11) % 32 < 4 || (y + 19) % 32 < 4 {
                30
            } else {
                220
            }
        })
        .collect();
    let relief = std::sync::Arc::new(crate::relief::Relief::new(SIDE, SIDE, heights).unwrap());
    let mut maps = Maps::default();
    maps.heights.insert(
        crate::relief::shaped_by(kit).unwrap().to_string(),
        relief.clone(),
    );
    let mut base = Base::default();
    base.apply(BuildOp::Fill {
        min: [0, 0, 0],
        max: [7, 3, 0],
        cube: Some(Cube::new(stone, Fill::Solid)),
    });
    let rules = kits.rules();
    let health = kit.toughness.rules().health;
    base.act(
        &terra_build::ops::Op::Damage {
            cubes: vec![[2, 1, 0]],
            amount: (f32::from(health) * 0.97) as u16,
        },
        &terra_build::ops::Context::free(&rules),
    )
    .unwrap();
    let section = Section {
        base: &base,
        kits: &kits,
        maps: &maps,
        at: IVec3::ZERO,
        progress: &Progress::Whole,
    };
    let drawn = draw(&section, &laid(&section), Reach::Near);
    let face = drawn.near[&(stone, Material::Masonry)].indexed();
    // The south face, z = 0.5 m: over the harmed cube, and over one far from it.
    let behind = |x: [f32; 2]| {
        face.positions
            .iter()
            .filter(|p| p.x > x[0] && p.x < x[1] && p.y > 0.55 && p.y < 0.95 && p.z > 0.3)
            .map(|p| 0.5 - p.z)
            .fold(0.0f32, f32::max)
    };
    let (harmed, whole) = (behind([1.05, 1.45]), behind([3.05, 3.45]));
    assert!(harmed > depth * 0.8, "{harmed}");
    assert!(whole < 0.005, "{whole}");
    // Its hole's rim runs along its stones' edges rather than round the grid's squares: the
    // points left standing at the top of its sides lie where the outline crosses the grid's
    // lines, off the grid's points, which a rim stepped on the grid keeps to.
    let step = terra_build::CELL_M / (crate::surface::HOLED_FINER as u32 * SURFACE_STEPS) as f32;
    let on_grid = |v: f32| ((v / step) - (v / step).round()).abs() < 0.02;
    let standing = |p: Vec3| p.z > 0.5 - 0.002;
    let mut rim = std::collections::BTreeSet::new();
    for t in face.indices.chunks(3) {
        let corners = [0, 1, 2].map(|k| face.positions[t[k] as usize]);
        let middle = (corners[0] + corners[1] + corners[2]) / 3.0;
        if middle.x < 0.95 || middle.x > 1.55 || middle.y < 0.45 || middle.y > 1.05 {
            continue;
        }
        if corners.iter().any(|p| !standing(*p)) {
            for p in corners.iter().filter(|p| standing(**p)) {
                rim.insert([p.x.to_bits(), p.y.to_bits()]);
            }
        }
    }
    let off = rim
        .iter()
        .filter(|[x, y]| !(on_grid(f32::from_bits(*x)) && on_grid(f32::from_bits(*y))))
        .count();
    assert!(rim.len() > 20, "a hole with a rim: {}", rim.len());
    let on: Vec<[f32; 2]> = rim
        .iter()
        .filter(|[x, y]| on_grid(f32::from_bits(*x)) && on_grid(f32::from_bits(*y)))
        .map(|[x, y]| [f32::from_bits(*x), f32::from_bits(*y)])
        .collect();
    assert!(
        off * 10 > rim.len() * 7,
        "{off} of {} off the grid: on {on:?}",
        rim.len()
    );
    // Each stone whole and flat to the face, or gone and sunk, never part of each: every
    // point well inside a stone, by the stone it is in, sunk (`wear`) alike.
    let cells = relief.cells(share);
    let metres = kit.textures[&Material::Masonry].metres;
    let mut stones: BTreeMap<[i32; 2], (usize, usize)> = BTreeMap::new();
    for (index, p) in face.positions.iter().enumerate() {
        if p.z < 0.3 || p.x < 0.6 || p.x > 1.9 || p.y < 0.1 || p.y > 1.4 {
            continue;
        }
        let uv = [p.x / metres[0], -p.y / metres[1]];
        if crate::cells::standing(&cells, uv, &|_| 0.0, 2.0, 0.0) < 3.0 {
            continue;
        }
        let middle = crate::cells::middle(&cells, uv).map(|v| (v * 4096.0).round() as i32);
        let sunk = face.wear.get(index).copied().unwrap_or(0.0) > 0.5;
        let entry = stones.entry(middle).or_default();
        if sunk {
            entry.0 += 1;
        } else {
            entry.1 += 1;
        }
    }
    let gone = stones.values().filter(|(sunk, _)| *sunk > 0).count();
    assert!(
        gone > 0 && gone < stones.len(),
        "{gone} of {} stones gone",
        stones.len()
    );
    for (middle, (sunk, flat)) in &stones {
        assert!(
            *sunk == 0 || *flat == 0,
            "the stone at {middle:?} is {sunk} sunk, {flat} not"
        );
    }
}

#[test]
fn a_flight_of_stone_ramps_coping_runs_on_from_one_stone_to_the_next() {
    // Three columns of stone, each a cube taller, each topped by a slope rising east: a ramp
    // up a storey's flight, open both sides.
    let kits = shipped();
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    let mut base = Base::default();
    for x in 0..3 {
        for y in 0..=x {
            let shape = if y == x {
                terra_build::Shape::Slope(terra_build::Facing::East)
            } else {
                terra_build::Shape::Whole
            };
            base.apply(BuildOp::Fill {
                min: [x, y, 0],
                max: [x, y, 0],
                cube: Some(Cube {
                    shape,
                    ..Cube::new(stone, Fill::Solid)
                }),
            });
        }
    }
    let maps = Maps::default();
    let whole = Whole::lay(&base, &kits);
    let mut bands = Vec::new();
    for at in drawable(&base) {
        let section = Section {
            base: &base,
            kits: &kits,
            maps: &maps,
            at: IVec3::from_array(at),
            progress: &Progress::Whole,
        };
        let laid = lay(&section, &whole);
        for placed in &laid.pieces {
            if let Piece::Band {
                role: layout::roles::Role::Crest,
                cube,
                ends,
                ..
            } = &placed.1
            {
                bands.push((cube.x, *ends));
            }
        }
    }
    // The coping up each side runs on from one stone to the next, squared only at the
    // flight's foot and head: squared at every cube, it stepped at every stone.
    use layout::roles::End;
    assert_eq!(bands.len(), 6, "{bands:?}");
    for (x, ends) in &bands {
        let foot = if *x == 0 { End::Square } else { End::On };
        let head = if *x == 2 { End::Square } else { End::On };
        assert_eq!(*ends, [foot, head], "the stone at {x}");
    }
}

#[test]
fn a_ramps_coping_is_mitred_to_the_level_coping_at_its_head_and_its_foot() {
    // A stone ramp rising east, one cube, from a step a cube high to a platform two high:
    // its coping meets a level coping over the bend at its head and under it at its foot,
    // and each level coping meets it the same way.
    let kits = shipped();
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    let mut base = Base::default();
    let put = |base: &mut Base, at: [i32; 3], shape: terra_build::Shape| {
        base.apply(BuildOp::Fill {
            min: at,
            max: at,
            cube: Some(Cube {
                shape,
                ..Cube::new(stone, Fill::Solid)
            }),
        });
    };
    put(&mut base, [0, 0, 0], terra_build::Shape::Whole);
    put(&mut base, [1, 0, 0], terra_build::Shape::Whole);
    put(
        &mut base,
        [1, 1, 0],
        terra_build::Shape::Slope(terra_build::Facing::East),
    );
    put(&mut base, [2, 0, 0], terra_build::Shape::Whole);
    put(&mut base, [2, 1, 0], terra_build::Shape::Whole);
    let maps = Maps::default();
    let whole = Whole::lay(&base, &kits);
    let mut ends = Vec::new();
    for at in drawable(&base) {
        let section = Section {
            base: &base,
            kits: &kits,
            maps: &maps,
            at: IVec3::from_array(at),
            progress: &Progress::Whole,
        };
        for placed in &lay(&section, &whole).pieces {
            match &placed.1 {
                Piece::Band {
                    role: layout::roles::Role::Crest,
                    cube,
                    side,
                    ends: e,
                    ..
                } if side.axis() == 2 => ends.push((*cube, *e)),
                Piece::Trim {
                    role: layout::roles::Role::Crest,
                    cube,
                    side,
                    ends: e,
                    ..
                } if side.axis() == 2 => ends.push((*cube, *e)),
                _ => {}
            }
        }
    }
    use layout::roles::End;
    let at = |x: i32, y: i32| -> Vec<[End; 2]> {
        ends.iter()
            .filter(|(c, _)| c.x == x && c.y == y)
            .map(|(_, e)| *e)
            .collect()
    };
    // The ramp's, up each side: under at its foot, over at its head.
    assert_eq!(at(1, 1), vec![[End::Under, End::Over]; 2], "{ends:?}");
    // The step's top at its east end, and the platform's at its west, meet it so.
    assert!(
        at(0, 0).iter().filter(|e| e[1] == End::Under).count() == 2,
        "{ends:?}"
    );
    assert!(
        at(2, 1).iter().filter(|e| e[0] == End::Over).count() == 2,
        "{ends:?}"
    );
    // And mitred so, nothing is drawn twice in one plane where they meet.
    let report = crate::inspect::inspect(&base, &kits, &maps);
    for fault in [
        crate::inspect::Fault::Coplanar,
        crate::inspect::Fault::Doubled,
    ] {
        let found: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.fault == fault && f.detail == Detail::Near)
            .collect();
        assert!(found.is_empty(), "{found:?}");
    }
}

#[test]
fn a_ramps_coping_runs_down_to_its_foot_over_a_half_beside_it() {
    // A stone ramp two cubes long rising north, its east side over a half at its foot: the
    // coping up that side starts at the ramp's foot, not a cube up, and runs on.
    let kits = shipped();
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    let mut base = Base::default();
    let put = |base: &mut Base, at: [i32; 3], shape: terra_build::Shape| {
        base.apply(BuildOp::Fill {
            min: at,
            max: at,
            cube: Some(Cube {
                shape,
                ..Cube::new(stone, Fill::Solid)
            }),
        });
    };
    put(
        &mut base,
        [0, 0, 2],
        terra_build::Shape::Slope(terra_build::Facing::North),
    );
    put(&mut base, [0, 0, 1], terra_build::Shape::Whole);
    put(
        &mut base,
        [0, 1, 1],
        terra_build::Shape::Slope(terra_build::Facing::North),
    );
    put(&mut base, [1, 0, 2], terra_build::Shape::Half);
    let maps = Maps::default();
    let whole = Whole::lay(&base, &kits);
    let mut east = Vec::new();
    for at in drawable(&base) {
        let section = Section {
            base: &base,
            kits: &kits,
            maps: &maps,
            at: IVec3::from_array(at),
            progress: &Progress::Whole,
        };
        for placed in &lay(&section, &whole).pieces {
            if let Piece::Band {
                role: layout::roles::Role::Crest,
                cube,
                side: layout::roles::Dir::East,
                ends,
                ..
            } = &placed.1
                && cube.x == 0
            {
                east.push((cube.z, *ends));
            }
        }
    }
    use layout::roles::End;
    east.sort_by_key(|(z, _)| -z);
    assert_eq!(
        east,
        vec![(2, [End::Square, End::On]), (1, [End::On, End::Square])],
        "the coping up the ramp's east side"
    );
    let report = crate::inspect::inspect(&base, &kits, &maps);
    for fault in [
        crate::inspect::Fault::Coplanar,
        crate::inspect::Fault::Doubled,
        crate::inspect::Fault::PokesOut,
    ] {
        let found: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.fault == fault && f.detail == Detail::Near)
            .collect();
        assert!(found.is_empty(), "{found:?}");
    }
}

#[test]
fn a_logs_top_under_a_roof_is_shaded_and_one_open_to_the_sky_is_not() {
    // The same room of logs with a roof and without: its logs' upper faces, which see the
    // roof's underside rather than the sky, are darker under the roof; their sides are not.
    let kits = shipped();
    let logs = kind(&kits, Pattern::Logs, Profile::Round);
    let mut open = Base::default();
    open.apply(BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [7, 0, 5],
        kind: logs,
    });
    let mut roofed = open.clone();
    roof(&mut roofed, &kits, [0, 0], [7, 5], Style::Gable, true);
    let maps = Maps::default();
    // The mean lightness of the logs' faces looking up (`up`) or level (not), below the
    // wall's top.
    let lightness = |base: &Base, up: bool| {
        let whole = Whole::lay(base, &kits);
        let (mut sum, mut count) = (0.0, 0usize);
        for at in drawable(base) {
            let section = Section {
                base,
                kits: &kits,
                maps: &maps,
                at: IVec3::from_array(at),
                progress: &Progress::Whole,
            };
            let drawn = draw(&section, &lay(&section, &whole), Reach::Near);
            for ((kind, material), surface) in &drawn.near {
                if *kind != logs || *material != Material::Log {
                    continue;
                }
                let all = surface.indexed();
                for ((p, n), c) in all.positions.iter().zip(&all.normals).zip(&all.colours) {
                    if p.y > 2.5 || (n.y > 0.8) != up || (!up && n.y.abs() > 0.2) {
                        continue;
                    }
                    // In light, as drawn: the colour made linear.
                    let light = |v: f32| libm::powf((v + 0.055) / 1.055, 2.4);
                    sum += (light(c[0]) + light(c[1]) + light(c[2])) / 3.0;
                    count += 1;
                }
            }
        }
        sum / count.max(1) as f32
    };
    let (open_top, roofed_top) = (lightness(&open, true), lightness(&roofed, true));
    let (open_side, roofed_side) = (lightness(&open, false), lightness(&roofed, false));
    assert!(roofed_top < open_top * 0.85, "{roofed_top} {open_top}");
    assert!(
        (roofed_side - open_side).abs() < open_side * 0.03,
        "{roofed_side} {open_side}"
    );
}

/// A wall of a kind planned from one cube to another, the cubes `built` says in as far as
/// it says (a level in sixteenths, none for planned).
fn wall_going_up(kind: Kind, max: [i32; 3], built: impl Fn(IVec3) -> Option<u8>) -> Base {
    let mut base = Base::default();
    for x in 0..=max[0] {
        for y in 0..=max[1] {
            for z in 0..=max[2] {
                let at = IVec3::new(x, y, z);
                let cube = Cube::planned(kind, None).at_level(built(at).unwrap_or(0));
                base.apply(BuildOp::Fill {
                    min: at.to_array(),
                    max: at.to_array(),
                    cube: Some(cube),
                });
            }
        }
    }
    base
}

/// Every piece of a base going up as it is drawn, with how far in each is.
fn drawn_as_built(base: &Base, kits: &Kits) -> Vec<(Piece, Stage)> {
    let maps = Maps::default();
    let whole = Whole::lay(base, kits);
    let progress = Progress::Cubes {
        next: BTreeSet::new(),
    };
    drawable(base)
        .into_iter()
        .flat_map(|at| {
            let section = Section {
                base,
                kits,
                maps: &maps,
                at: IVec3::from_array(at),
                progress: &progress,
            };
            lay(&section, &whole).pieces
        })
        .map(|(_, piece, stage)| (piece, stage))
        .collect()
}

type DressedIn = BTreeSet<([i32; 3], layout::Role, [layout::Dir; 2])>;

/// The edges dressed in what is in: each cube's role and the faces it is between, sorted.
fn dressed_in(drawn: &[(Piece, Stage)]) -> DressedIn {
    drawn
        .iter()
        .filter(|(_, stage)| *stage == Stage::In)
        .filter_map(|(piece, _)| {
            let (cube, role, mut faces) = match *piece {
                Piece::Trim {
                    cube,
                    role,
                    side,
                    other,
                    ..
                } => (cube, role, [side, other]),
                Piece::Corner {
                    cube, role, faces, ..
                } => (cube, role, faces),
                _ => return None,
            };
            faces.sort();
            Some((cube.to_array(), role, faces))
        })
        .collect()
}

/// Whether a cube's edge between two faces is dressed in what is in with a role.
fn has_edge(
    dressed: &DressedIn,
    cube: [i32; 3],
    role: layout::Role,
    faces: [layout::Dir; 2],
) -> bool {
    let mut faces = faces;
    faces.sort();
    dressed.contains(&(cube, role, faces))
}

#[test]
fn a_stone_wall_built_halfway_along_has_an_end_and_a_top_where_it_stops() {
    // D358: what is in is drawn from what is in. A wall planned eight cubes long and two
    // high, built four along: where it stops it has quoins up its end and a coping along
    // its top, across the end too, though the plan runs on; the plan beyond is ghosts.
    use layout::{Dir, Role};
    let kits = shipped();
    let stone = named(&kits, "stone");
    let base = wall_going_up(stone, [7, 1, 0], |at| (at.x <= 3).then_some(16));
    let drawn = drawn_as_built(&base, &kits);
    let dressed = dressed_in(&drawn);
    for y in 0..=1 {
        for side in [Dir::South, Dir::North] {
            assert!(
                has_edge(&dressed, [3, y, 0], Role::Arris, [Dir::East, side]),
                "quoins up its end at {y}: {dressed:?}"
            );
        }
    }
    assert!(has_edge(
        &dressed,
        [3, 1, 0],
        Role::Crest,
        [Dir::East, Dir::Up]
    ));
    assert!(has_edge(
        &dressed,
        [2, 1, 0],
        Role::Crest,
        [Dir::North, Dir::Up]
    ));
    // Nothing of what is in past where it stops, and the plan there drawn as ghosts.
    assert!(dressed.iter().all(|(cube, _, _)| cube[0] <= 3));
    assert!(
        drawn
            .iter()
            .any(|(piece, stage)| *stage == Stage::Ghost && piece.anchor().x >= 4)
    );
    // Built on to its end, its quoins and coping's end move there.
    let done = wall_going_up(stone, [7, 1, 0], |_| Some(16));
    let dressed = dressed_in(&drawn_as_built(&done, &kits));
    assert!(!has_edge(
        &dressed,
        [3, 1, 0],
        Role::Arris,
        [Dir::East, Dir::North]
    ));
    assert!(!has_edge(
        &dressed,
        [3, 1, 0],
        Role::Crest,
        [Dir::East, Dir::Up]
    ));
    assert!(has_edge(
        &dressed,
        [7, 1, 0],
        Role::Arris,
        [Dir::East, Dir::North]
    ));
    assert!(has_edge(
        &dressed,
        [7, 1, 0],
        Role::Crest,
        [Dir::East, Dir::Up]
    ));
}

#[test]
fn a_middle_cube_built_alone_has_its_edges_and_finishing_its_neighbour_takes_them_away() {
    // D358: a wall's middle cube built before those either side of it is a pier on its
    // own, dressed all round: quoins up both ends and a coping round its top. Built on
    // beside it, the edge it shares goes, and the coping runs on into its neighbour's.
    use layout::{Dir, Role};
    let kits = shipped();
    let stone = named(&kits, "stone");
    let alone = wall_going_up(stone, [6, 0, 0], |at| (at.x == 3).then_some(16));
    let dressed = dressed_in(&drawn_as_built(&alone, &kits));
    for side in [Dir::East, Dir::West] {
        for other in [Dir::South, Dir::North] {
            assert!(has_edge(&dressed, [3, 0, 0], Role::Arris, [side, other]));
        }
    }
    for side in Dir::LEVEL {
        assert!(
            has_edge(&dressed, [3, 0, 0], Role::Crest, [side, Dir::Up]),
            "a coping on its {side:?} side: {dressed:?}"
        );
    }
    let pair = wall_going_up(stone, [6, 0, 0], |at| (3..=4).contains(&at.x).then_some(16));
    let drawn = drawn_as_built(&pair, &kits);
    let dressed = dressed_in(&drawn);
    for (role, faces) in [
        (Role::Arris, [Dir::East, Dir::North]),
        (Role::Arris, [Dir::East, Dir::South]),
        (Role::Crest, [Dir::East, Dir::Up]),
    ] {
        assert!(!has_edge(&dressed, [3, 0, 0], role, faces), "{role:?}");
        assert!(has_edge(&dressed, [4, 0, 0], role, faces), "{role:?}");
    }
    assert!(!has_edge(
        &dressed,
        [4, 0, 0],
        Role::Crest,
        [Dir::West, Dir::Up]
    ));
    // Its coping along the wall's side runs on into the neighbour's.
    let ends = drawn.iter().find_map(|(piece, stage)| match piece {
        Piece::Trim {
            cube,
            role: Role::Crest,
            side: Dir::North,
            ends,
            ..
        } if *stage == Stage::In && cube.x == 3 => Some(*ends),
        _ => None,
    });
    assert_eq!(ends.map(|ends| ends[1]), Some(layout::End::On));
}

#[test]
fn a_log_wall_going_up_ends_its_courses_where_what_is_in_stops() {
    // D358, in the coursed pattern: a log wall planned eight cubes long, built four along
    // and the fifth half in. Its lower course runs on through the fifth; the one over it,
    // not yet in there, ends at the fourth's face, not at a joint short of it; no log of
    // what is in reaches past the fifth.
    let kits = shipped();
    let logs = named(&kits, "logs");
    let base = wall_going_up(logs, [7, 0, 0], |at| match at.x {
        0..=3 => Some(16),
        4 => Some(8),
        _ => None,
    });
    let logs_in: Vec<(Vec3, Vec3)> = drawn_as_built(&base, &kits)
        .into_iter()
        .filter(|(piece, stage)| *stage == Stage::In && matches!(piece, Piece::Log { .. }))
        .filter_map(|(piece, _)| piece.bounds())
        .collect();
    assert!(!logs_in.is_empty());
    let reach = |low: f32, high: f32| {
        logs_in
            .iter()
            .filter(|(min, max)| (min.y + max.y) * 0.5 >= low && (min.y + max.y) * 0.5 < high)
            .map(|(_, max)| max.x)
            .fold(f32::MIN, f32::max)
    };
    let (fourth, fifth) = (4.0 * CELL_M, 5.0 * CELL_M);
    let lower = reach(0.0, 0.25);
    assert!(
        lower > fourth + 0.2 && lower <= fifth + 0.01,
        "the lower course ends at {lower}"
    );
    let upper = reach(0.25, 0.5);
    assert!(
        upper > fourth - 0.05 && upper <= fourth + 0.01,
        "the upper course ends at {upper}"
    );
}

#[test]
fn a_house_going_up_laid_again_a_fill_at_a_time_is_as_laid_whole() {
    // D358: what stands is laid apart from the plan, and laid again only where an edit
    // changed it. A small house in logs and in stone under its roof, planned whole, then
    // filled a cube at a time from its foot, half in and then built, and a cube of plan
    // added and taken out; and a long log hall built but for its end wall, then that wall,
    // whose first cubes turn the logs along the floor from them: every section, as drawn
    // going up, is as laid whole, and every section drawn otherwise than before is one
    // the edit says it reached.
    let kits = shipped();
    let maps = Maps::default();
    let next = Progress::Cubes {
        next: BTreeSet::new(),
    };
    let going_up = |name: &str, mut before: Base, edits: Vec<(IVec3, Option<Cube>)>| {
        before.take_dirty();
        let mut whole = Whole::lay(&before, &kits);
        for (at, cube) in edits {
            let mut after = before.clone();
            after.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube,
            });
            let marked = after.take_dirty();
            let again = whole.lay_again(&before, &after, &kits, &marked);
            let afresh = Whole::lay(&after, &kits);
            let mut reached: BTreeSet<[i32; 3]> = reach(&before, &after, &kits, &marked)
                .into_iter()
                .flatten()
                .collect();
            reached.extend(again.standing_reached());
            for at_section in drawable(&after).union(&drawable(&before)) {
                let section = |base| Section {
                    base,
                    kits: &kits,
                    maps: &maps,
                    at: IVec3::from_array(*at_section),
                    progress: &next,
                };
                let now = lay(&section(&after), &again);
                assert!(
                    now == lay(&section(&after), &afresh),
                    "{name}: {at} set to {cube:?}: section {at_section:?} is not as laid whole"
                );
                assert!(
                    now == lay(&section(&before), &whole) || reached.contains(at_section),
                    "{name}: {at} set to {cube:?}: section {at_section:?} changed unreached"
                );
            }
            (before, whole) = (after, again);
        }
        before
    };
    let planned = |built: &Base, keep: &dyn Fn(IVec3) -> bool| {
        let mut base = built.clone();
        for (at, cube) in built.cubes().filter(|(at, _)| !keep(*at)) {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(cube.at_level(0)),
            });
        }
        base
    };
    for walls in ["logs", "stone"] {
        let mut built = Base::default();
        house(
            &mut built,
            &kits,
            [14, 2],
            [5, 4],
            named(&kits, walls),
            Style::Gable,
        );
        let mut cubes: Vec<(IVec3, Cube)> = built.cubes().collect();
        cubes.sort_by_key(|(at, _)| (at.y, at.x, at.z));
        let mut edits: Vec<(IVec3, Option<Cube>)> = cubes
            .iter()
            .step_by(7)
            .flat_map(|&(at, cube)| [(at, Some(cube.at_level(8))), (at, Some(cube))])
            .collect();
        let (low, kind) = (cubes[0].0, cubes[0].1.kind);
        edits.push((low - IVec3::X * 2, Some(Cube::planned(kind, None))));
        edits.push((low - IVec3::X * 2, None));
        let after = going_up(walls, planned(&built, &|_| false), edits);
        let built = after.cubes().filter(|(_, c)| c.is_built()).count();
        assert!(
            built > 10 && after.store().planned() > 0,
            "{walls}: {built} built"
        );
    }
    let mut hall = Base::default();
    let logs = named(&kits, "logs");
    house(&mut hall, &kits, [-3, 2], [40, 9], logs, Style::Gable);
    let end = hall.cubes().map(|(at, _)| at.x).max().unwrap_or(0);
    let mut wall: Vec<(IVec3, Cube)> = hall
        .cubes()
        .filter(|(at, cube)| at.x == end && !cube.kind.is_roof())
        .collect();
    wall.sort_by_key(|(at, _)| (at.y, at.z));
    let edits = wall
        .iter()
        .take(12)
        .map(|&(at, cube)| (at, Some(cube)))
        .collect();
    going_up(
        "a long log hall",
        planned(&hall, &|at| {
            at.x != end || hall.cube(at).is_some_and(|c| c.kind.is_roof())
        }),
        edits,
    );
}
