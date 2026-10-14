use super::*;
use crate::kit::Kits;
use crate::layout::lay_among;
use crate::roofer::{self, Style};
use crate::{BuildOp, Cube, Panel, STOREY_CUBES};

/// The kits the game ships, every one, with their dressed pieces, as it reads them.
fn shipped() -> Kits {
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let read = |path: &str| {
        std::fs::read_to_string(assets.join(path)).map_err(|why| format!("{path}: {why}"))
    };
    let mut kits = Vec::new();
    for name in [
        "logs", "hewn", "stone", "earth", "wattle", "shakes", "slate", "thatch",
    ] {
        let text = read(&format!("data/kits/{name}.ron")).unwrap();
        let mut kit = ron::from_str::<Kit>(&text).unwrap_or_else(|why| panic!("{name}: {why}"));
        kit.dress.read(&read).unwrap();
        kits.push(kit);
    }
    Kits(kits)
}

/// A kit's kind by its file's name: earth, wattle and daub.
fn named(kits: &Kits, id_of: &str) -> Kind {
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let text = std::fs::read_to_string(assets.join(format!("data/kits/{id_of}.ron"))).unwrap();
    let kit = ron::from_str::<Kit>(&text).unwrap();
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
    for op in crate::example(logs) {
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
                sorted(laid.edge.iter().map(|p| p.1.clone())),
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
        role: None,
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

/// Every triangle of a set of surfaces, each as its corners' bits, sorted.
fn triangles<'a>(surfaces: impl IntoIterator<Item = &'a Surface>) -> Vec<[u32; 18]> {
    let mut out = Vec::new();
    for surface in surfaces {
        let all = surface.indexed();
        for t in all.indices.chunks_exact(3) {
            let mut corners: Vec<[u32; 6]> = t
                .iter()
                .map(|&i| {
                    let (p, n) = (all.positions[i as usize], all.normals[i as usize]);
                    [p.x, p.y, p.z, n.x, n.y, n.z].map(f32::to_bits)
                })
                .collect();
            corners.sort();
            out.push(corners.concat().try_into().unwrap());
        }
    }
    out.sort();
    out
}

#[test]
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
                let gables: BTreeMap<[i32; 3], [[i32; 3]; 3]> = pieces
                    .iter()
                    .filter_map(|p| match p {
                        Piece::Gable { cube, under } => Some((cube.to_array(), *under)),
                        _ => None,
                    })
                    .collect();
                let cut = |cube: IVec3, at: Vec3| {
                    let high = gables
                        .get(&cube.to_array())
                        .map_or(f32::MAX, |under| layout::roofs::gable_cut(cube, under, at));
                    [f32::MIN, high]
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
                        .filter_map(|b| b.inside(p).map(|inside| (inside, b.face)))
                        .max_by(|a, b| a.0.total_cmp(&b.0))
                };
                let heights = Heights {
                    sides: &flat,
                    top: &flat,
                    bed: (!beds.is_empty()).then_some(&bed as &dyn Fn(Vec3) -> Option<(f32, f32)>),
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
        assert!(a == b, "the same triangles, corners and normals");
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

#[test]
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
                shape: crate::Shape::OuterCorner(crate::Facing::South),
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
    use crate::{Facing, Shape};
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

#[test]
fn a_section_drawn_again_from_how_it_was_drawn_is_drawn_as_afresh() {
    let kits = shipped();
    let maps = Maps::default();
    let bases = bases(&kits);
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    let (mut taken, mut faces) = (0, 0);
    for (base, ground) in [
        (&bases[1], false),
        (&bases[4], false),
        (&bases[1], true),
        (&bases[4], true),
    ] {
        // A wall's cube a metre up taken out; or the ground under the base changed, which
        // every piece's dirt rises from.
        let wall = base
            .cubes()
            .map(|(at, _)| at)
            .find(|&at| at.y == 2 && base.in_wall(at) && base.cube(at).unwrap().kind == stone)
            .unwrap();
        let mut edited = base.clone();
        if ground {
            let ring = edited.ground_ring();
            edited.sample_ground(ring.into_iter().map(|column| crate::ground::Sample {
                column,
                height_cm: 30,
            }));
        } else {
            edited.apply(BuildOp::Fill {
                min: wall.to_array(),
                max: wall.to_array(),
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
                if !ground {
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

#[test]
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
    let deck = |z: f32| crate::STOREY_M + z.min(4.0 - z);
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
            // The wall's surface on the west gable's face, a little either side of it.
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
                if let Some(surface) = drawn.near.get(&(wall, crate::mesh::Material::Masonry)) {
                    face.extend(surface.indexed().positions.into_iter().filter(|p| {
                        p.x.abs() < 0.1
                            && p.y > crate::STOREY_M - 0.25
                            && (0.0..=4.0).contains(&p.z)
                    }));
                }
            }
            let name = format!("{} under {}", wall.0, covering.0);
            assert!(face.len() > 50, "{name}: {}", face.len());
            // Nothing over the deck, and up the whole gable, from eave to ridge either side,
            // the wall's top on it every few centimetres: no step and no gap under the roof.
            for p in &face {
                assert!(p.y <= deck(p.z) + 0.005, "{name}: {p} over the deck");
            }
            let mut z = 0.2;
            while z < 3.8 {
                let gap = face
                    .iter()
                    .filter(|p| (p.z - z).abs() < 0.07)
                    .map(|p| deck(p.z) - p.y)
                    .fold(f32::MAX, f32::min);
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
            assert!(out < bed.face, "earth at {p} through the lintel");
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

/// Maps of rough height for every kit, as a real stone's or earth's: what pushes a block
/// surface's points about, so faults that only show with relief show.
fn rough(kits: &Kits) -> Maps {
    use crate::relief::{Relief, cored_by, shaped_by, topped_by};
    const SIDE: usize = 64;
    let heights = (0..SIDE * SIDE)
        .map(|i| (crate::layout::hash(&[i as u32, 7]) >> 24) as u8)
        .collect();
    let relief = std::sync::Arc::new(Relief::new(SIDE, SIDE, heights).unwrap());
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
fn an_edit_laid_again_only_as_far_as_it_reaches_is_as_laid_whole() {
    // Town buildings of every kit and plan, two storeys and decks among them, each edited
    // a cube at a time: one taken out of it, one put on its top, one beside it on the
    // ground. Laid again only as far as `reach` says, every section is as laid whole.
    let kits = shipped();
    let maps = Maps::default();
    let named = |name: &str| {
        let index = [
            "logs", "hewn", "stone", "earth", "wattle", "shakes", "slate", "thatch",
        ]
        .iter()
        .position(|n| *n == name)
        .unwrap();
        kits.0[index].kind()
    };
    let kinds = crate::village::Kinds {
        logs: named("logs"),
        stone: named("stone"),
        hewn: named("hewn"),
        earth: named("earth"),
        wattle: named("wattle"),
        shakes: named("shakes"),
        slate: named("slate"),
        thatch: named("thatch"),
    };
    // And a long hall of each kind of wall, five sections long, a cube taken out at its
    // end and one added there.
    let mut bases: Vec<(String, Base)> = crate::village::town(3, 14, kinds)
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
    let mut random = crate::layout::Random(7);
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
fn verges_meeting_at_an_apex_take_no_cap_there() {
    // Every roof kind over a gable: each verge's top meets the other verge and the ridge at
    // the apex, under the ridge's own cap; capped there too, the two verges' caps stood out
    // past the apex through each other. Their feet at the eaves stop in the open, capped.
    let kits = shipped();
    for (name, base) in crate::inspect::cases(&kits, Some("rectangle, gable roof")) {
        if name.contains("with") {
            continue;
        }
        let top = base.cubes().map(|(at, _)| at.y + 1).max().unwrap_or(0) as f32 * crate::CELL_M;
        let mut verges = 0;
        for kit in &kits.0 {
            for piece in crate::layout::lay_among(&base, kit, &kits).pieces {
                let Piece::Roof {
                    part: crate::layout::roofs::RoofPart::Verge,
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
