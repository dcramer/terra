use super::*;
use crate::kit::Kits;
use crate::layout::lay_among;
use crate::{BuildOp, Cube, Panel, RoofSection, RoofStyle, STOREY_CUBES};

/// The kits the game ships, as it reads them.
fn shipped() -> Kits {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/data/kits");
    let mut kits = Vec::new();
    for name in ["logs.ron", "hewn.ron", "stone.ron"] {
        let text = std::fs::read_to_string(folder.join(name)).unwrap();
        kits.push(ron::from_str::<Kit>(&text).unwrap_or_else(|why| panic!("{name}: {why}")));
    }
    Kits(kits)
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

/// A room from one tile to another under a roof, with a door and a window, its corner at
/// `at`: placed so it crosses sections' edges.
fn house(at: [i32; 2], size: [i32; 2], walls: Kind, style: RoofStyle) -> Vec<BuildOp> {
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
    ops.push(BuildOp::AddRoof(RoofSection {
        min,
        max,
        style,
        along_x: true,
        pitch: if style == RoofStyle::LeanTo { 0.4 } else { 0.7 },
    }));
    ops
}

/// Bases of every kind and roof, and mixed, each across sections' edges, some on the
/// negative side of the grid, with footings, a carved wall and a long wall.
fn bases(kits: &Kits) -> Vec<Base> {
    let logs = kind(kits, Pattern::Logs, Profile::Round);
    let hewn = kind(kits, Pattern::Logs, Profile::Hewn);
    let stone = kind(kits, Pattern::Masonry, Profile::Round);
    let mut out = Vec::new();
    for (walls, style, at) in [
        (logs, RoofStyle::Gable, [10, 12]),
        (stone, RoofStyle::Hip, [-6, 9]),
        (hewn, RoofStyle::LeanTo, [12, -4]),
        (logs, RoofStyle::Hip, [-20, -20]),
    ] {
        let mut base = Base::default();
        for op in house(at, [11, 9], walls, style) {
            base.apply(op);
        }
        base.lay_footings(stone, |_| -40);
        out.push(base);
    }
    // Logs on a stone plinth, an L with a wing in stone, and a wall carved through.
    let mut mixed = Base::default();
    for op in house([9, 9], [9, 7], logs, RoofStyle::Gable) {
        mixed.apply(op);
    }
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
    mixed.apply(BuildOp::AddRoof(RoofSection {
        min: [9, 0, 16],
        max: [15, 0, 26],
        style: RoofStyle::Gable,
        along_x: false,
        pitch: 0.7,
    }));
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
    out
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
    for base in [&bases[0], &bases[1], &bases[4]] {
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
                flights: &[],
            };
            let drawn = draw(&section, &lay(&section), Reach::Near);
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
                    gables
                        .get(&cube.to_array())
                        .map_or(f32::MAX, |under| layout::roofs::gable_cut(cube, under, at))
                };
                let hides = |at: IVec3| {
                    base.cube(at).is_some_and(|cube| {
                        cube.fill != Fill::Open
                            && cube.kind != kit.kind()
                            && (!base.in_wall(at) || cube.kind == Kind::PLAIN)
                    })
                };
                let flat = |_: [f32; 2]| 0.5;
                let heights = Heights {
                    sides: &flat,
                    top: &flat,
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
        flights: &[],
    };
    let drawn = draw(&section, &lay(&section), Reach::Near);
    let near: usize = drawn.near.values().map(Surface::triangle_count).sum();
    let (far, proxy) = (drawn.far.triangle_count(), drawn.proxy.triangle_count());
    assert!(near > 0 && far > 0 && proxy > 0);
    assert!(far * 5 < near, "far {far} against near {near}");
    assert!(proxy < far, "proxy {proxy} against far {far}");
    // Far off only, nothing near is drawn.
    assert!(draw(&section, &lay(&section), Reach::Far).near.is_empty());
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
            flights: &[],
        };
        let drawn = draw(&section, &lay(&section), Reach::Near);
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
            flights: &[],
        };
        lay(&section).key()
    };
    // A long building, then changes far from parts of it: a cube at one end, a window,
    // a wall's kind, a roof.
    let mut base = Base::default();
    for op in house([2, 2], [40, 7], logs, RoofStyle::Gable) {
        base.apply(op);
    }
    // And a second building apart from it.
    for op in house([2, 40], [7, 7], logs, RoofStyle::Hip) {
        base.apply(op);
    }
    base.lay_footings(stone, |_| -40);
    base.take_dirty();
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
        BuildOp::AddRoof(RoofSection {
            min: [2, 0, 2],
            max: [42, 0, 9],
            style: RoofStyle::Hip,
            along_x: true,
            pitch: 0.5,
        }),
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
        flights: &[],
    };
    let drawn = draw(&section, &lay(&section), Reach::Far);
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
