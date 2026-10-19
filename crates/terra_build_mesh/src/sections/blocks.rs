//! A block kind's cubes drawn as their surface (D197), with the holes damage makes in it
//! and the beds pieces sit in, in stages: its cubes sorted by how far in they are and how
//! they are cut ([`Sorted`]), the parts drawn this time ([`Wanted`]), how each cube is cut
//! ([`Cutting`]), the beds pieces sit in ([`Beds`]), the holes damage makes ([`Holes`]),
//! then each set's surface ([`Blocks::draw_set`]) and a framed kind's cores
//! ([`Blocks::draw_cores`]).

use super::*;

/// How many steps across a cube's side whether a stone has gone from it is read at: every
/// 6 cm, less than any hole a stone and its mortar leave.
pub(super) const HOLE_LOOKS: usize = 8;

/// How much more may have been lost at a stone's middle than at a point of the stone, the
/// most a stone's width can carry it at a hit's steepest: below the first stones' fall by
/// more, nothing there falls.
pub(super) const STONE_REACH_LOST: f32 = 0.45;

/// The cells' side, in texels, as a number to scale by.
pub(super) const CELLS_F: f32 = crate::cells::CELLS as f32;

/// A cube and those two round it: as far as a cube's loss reaches (`loss::struck`), and a
/// stone of the next cube's middle past that.
pub(super) fn round_it(cube: IVec3) -> impl Iterator<Item = IVec3> {
    (-2..=2).flat_map(move |x| {
        (-2..=2).flat_map(move |y| (-2..=2).map(move |z| cube + IVec3::new(x, y, z)))
    })
}

/// How far round a bed a surface looks for it, in metres: past the furthest any asks
/// (`surface::Heights::bed`), half a cube and a bed's lap.
pub(super) const BED_REACH_M: f32 = 0.55;

/// One of a kit's block surfaces' caching, each under a number of its own.
pub(super) fn cached<'a>(
    caching: &'a mut Option<BlockCaching>,
    surface: u32,
) -> Option<crate::surface::Caching<'a>> {
    caching.as_mut().map(|caching| crate::surface::Caching {
        call: caching.call << 4 | surface,
        old: caching.old,
        new: &mut *caching.new,
    })
}

/// A block kind's cubes in a section as its surface (D197): those in, those not yet, and
/// the next, each closed where it meets the others but for what is already built, with
/// the cubes a step beyond the section's edge worked out but not drawn, so its edges meet
/// the next section's as one surface.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_blocks(
    section: &Section,
    kit: &Kit,
    own: &[&Placed],
    edge: &[&Placed],
    detail: Detail,
    first: bool,
    mut caching: Option<BlockCaching>,
    slot_of: &dyn Fn(IVec3) -> usize,
    slots: usize,
    wanted: &dyn Fn(usize) -> bool,
) -> [Vec<Surfaces>; 3] {
    let base = section.base;
    let sorted = Sorted::of(base, own, edge, detail, first);
    let parts = Wanted::of(section, slots, wanted);
    let cutting = Cutting {
        base,
        kit,
        gables: &sorted.gables,
    };
    let cut_at = |cube: IVec3, at: Vec3| cutting.at(cube, at);
    let cut_at_all = |cube: IVec3| cutting.at_all(cube);
    let cut_key = |cube: IVec3| cutting.key(cube);
    let cut = crate::surface::Cuts {
        of: &cut_at_all,
        at: &cut_at,
        key: &cut_key,
    };
    let look = block_look(kit, detail.steps());
    let (relief, top) = if matches!(detail, Detail::Near | Detail::Mid) {
        (section.maps.shape(kit), section.maps.top(kit))
    } else {
        (None, None)
    };
    let sides = |uv: [f32; 2]| relief.map_or(0.5, |relief| relief.at(uv));
    let top = |uv: [f32; 2]| top.map_or(0.5, |relief| relief.at(uv));
    let beds = Beds::of(kit, own, edge);
    let bed = |p: Vec3| beds.at(p);
    let bed_key = |cube: IVec3| beds.key(cube);
    let holes = Holes::of(section, kit, relief, &look);
    let hole = |p: Vec3, normal: Vec3| holes.hole(p, normal);
    let holed = |cube: IVec3| holes.holed(cube);
    let heights = Heights {
        sides: &sides,
        top: &top,
        bed: (!beds.is_empty()).then_some(&bed as crate::surface::BedAt),
        bed_key: (!beds.is_empty()).then_some(&bed_key as &dyn Fn(IVec3) -> u64),
        hole: holes
            .joints
            .is_some()
            .then_some(&hole as &dyn Fn(Vec3, Vec3) -> f32),
        holed: holes
            .joints
            .is_some()
            .then_some(&holed as &dyn Fn(IVec3) -> Option<u64>),
    };
    // Floors, and walls of kinds drawn as boxes, hide a face; walls of other kits do not,
    // being thinner than the cube, nor a roof's cubes, drawn as a roof over them.
    let hides_always = |at: IVec3| {
        base.cube(at).is_some_and(|cube| {
            cube.fill != Fill::Open
                && cube.kind != kit.kind()
                && !cube.kind.is_roof()
                && (!base.in_wall(at) || cube.kind == Kind::PLAIN)
        })
    };
    let keep = |at: IVec3| terra_build::cubes::section_of(at) == section.at;
    // A part's cubes drawn only if the part is wanted: what is in, but not the plan's
    // shell or the next piece, which the section has whole.
    let keep_in = |at: IVec3| {
        let slot = slot_of(at);
        (keep(at) && wanted(slot)).then_some(slot)
    };
    let keep_whole = |at: IVec3| keep(at).then(|| slot_of(at));
    let blocks = Blocks {
        section,
        kit,
        detail,
        first,
        sorted: &sorted,
        parts: &parts,
        cutting: &cutting,
        cut: &cut,
        look: &look,
        heights: &heights,
        hides_always: &hides_always,
        keep_in: &keep_in,
        keep_whole: &keep_whole,
        slot_of,
        slots,
    };
    let mut out: [Vec<Surfaces>; 3] =
        [0, 1, 2].map(|_| (0..slots).map(|_| Surfaces::sharing()).collect());
    for (set, out) in out.iter_mut().enumerate() {
        blocks.draw_set(set, &mut caching, out);
    }
    blocks.draw_cores(&mut caching, &mut out[0]);
    out
}

/// A kit's cubes in a section and a cube round it, by how far each is in (in, a plan's,
/// next): whole, and cut to a shape; a gable's with the roof over each; and a framed
/// kind's whose core is in (D236).
struct Sorted {
    sets: [BTreeSet<[i32; 3]>; 3],
    /// Cubes cut to a shape (D193), drawn apart from the whole ones: halves and slopes as
    /// the surface cut down to their tops, stairs as a stone a tread, posts as drums.
    shaped: [BTreeSet<[i32; 3]>; 3],
    /// A gable's cubes, with the roof's underside over each, which cuts them.
    gables: BTreeMap<[i32; 3], ([[i32; 3]; 3], layout::Carried)>,
    /// A framed kind's cubes whose core is in (D236), where its infill is not in, nor lit
    /// as next: neither over it.
    cores: BTreeSet<[i32; 3]>,
}

impl Sorted {
    fn of(base: &Base, own: &[&Placed], edge: &[&Placed], detail: Detail, first: bool) -> Sorted {
        let mut sorted = Sorted {
            sets: Default::default(),
            shaped: Default::default(),
            gables: BTreeMap::new(),
            cores: BTreeSet::new(),
        };
        for (_, piece, stage) in own.iter().chain(edge).copied() {
            let cube = match piece {
                Piece::Wattle { cube } => {
                    if *stage == Stage::In || (*stage == Stage::Next && !first) {
                        sorted.cores.insert(cube.to_array());
                    }
                    continue;
                }
                Piece::Block { cube } => cube,
                Piece::Gable {
                    cube,
                    under,
                    carried,
                } => {
                    sorted.gables.insert(cube.to_array(), (*under, *carried));
                    cube
                }
                _ => continue,
            };
            let set = match (stage, detail) {
                (Stage::In, _) => 0,
                (Stage::Next, _) if first => 2,
                // Far off, what goes in next is in.
                (Stage::Next, _) => 0,
                (Stage::Ghost, _) => 1,
            };
            // A wall carried up into a roof's cube is the wall's whole cube cut at the roof,
            // whatever the roof's cube is cut to.
            // One carried down into a roof's cube it stands on is cut from below at the deck,
            // as a shape with a planed underside is, and drawn with them.
            let cut = (matches!(piece, Piece::Block { .. })
                && base.cube(*cube).is_some_and(|c| c.shape.is_cut()))
                || matches!(
                    piece,
                    Piece::Gable {
                        carried: layout::Carried::Down,
                        ..
                    }
                );
            if cut {
                sorted.shaped[set].insert(cube.to_array());
            } else {
                sorted.sets[set].insert(cube.to_array());
            }
        }
        let Sorted { sets, cores, .. } = &mut sorted;
        cores.retain(|at| !sets[0].contains(at) && !sets[2].contains(at));
        sets[1].retain(|at| !cores.contains(at));
        sorted
    }
}

/// The parts drawn this time, each as a box of cubes grown by as far as a face's shape
/// depends on what is round it and its neighbours' do on theirs, if not all of them.
struct Wanted {
    boxes: Vec<(IVec3, IVec3)>,
    /// Whether only some parts are drawn.
    some: bool,
}

impl Wanted {
    fn of(section: &Section, slots: usize, wanted: &dyn Fn(usize) -> bool) -> Wanted {
        let section_low = section.at * terra_build::cubes::SECTION;
        let boxes: Vec<(IVec3, IVec3)> = (0..slots)
            .filter(|&slot| wanted(slot))
            .map(|slot| {
                let (size, side) = (CLUSTER, terra_build::cubes::SECTION / CLUSTER);
                let s = slot as i32;
                let low =
                    section_low + IVec3::new(s % side, s / side % side, s / (side * side)) * size;
                (
                    low - IVec3::splat(PARTS_REACH),
                    low + IVec3::splat(size - 1 + PARTS_REACH),
                )
            })
            .collect();
        Wanted {
            some: slots > 1 && boxes.len() < slots,
            boxes,
        }
    }

    /// Whether a cube is near enough a part drawn to shape its faces.
    fn near(&self, at: IVec3) -> bool {
        self.boxes
            .iter()
            .any(|(low, high)| at.cmpge(*low).all() && at.cmple(*high).all())
    }
}

/// How a kit's cubes are cut: to their shape, and a gable's to the roof over it.
struct Cutting<'a> {
    base: &'a Base,
    kit: &'a Kit,
    gables: &'a BTreeMap<[i32; 3], ([[i32; 3]; 3], layout::Carried)>,
}

impl Cutting<'_> {
    /// A cube's shape, if it is of the kit's kind.
    fn shape_of(&self, cube: IVec3) -> terra_build::Shape {
        self.base
            .cube(cube)
            .filter(|c| c.kind == self.kit.kind())
            .map_or(terra_build::Shape::Whole, |c| c.shape)
    }

    /// The heights a cube is cut to over a point of its face, lowest and highest.
    fn at(&self, cube: IVec3, at: Vec3) -> [f32; 2] {
        let [under_roof, roof] = self
            .gables
            .get(&cube.to_array())
            .map_or([f32::MIN, f32::MAX], |(under, carried)| {
                layout::roofs::carried_cut(*carried, cube, under, at)
            });
        let (u, v) = (
            (at.x / CELL_M - cube.x as f32).clamp(0.0, 1.0),
            (at.z / CELL_M - cube.z as f32).clamp(0.0, 1.0),
        );
        let shape = self.shape_of(cube);
        let high = if shape.planed_top() {
            (cube.y as f32 + shape.top(u, v)) * CELL_M
        } else {
            f32::MAX
        };
        let low = if shape.planed_under() {
            (cube.y as f32 + shape.bottom(u, v)) * CELL_M
        } else {
            f32::MIN
        };
        [low.max(under_roof), roof.min(high)]
    }

    /// Whether a cube is cut at all: most are whole, and their cut is never asked.
    fn at_all(&self, cube: IVec3) -> bool {
        let shape = self.shape_of(cube);
        self.gables.contains_key(&cube.to_array()) || shape.planed_top() || shape.planed_under()
    }

    /// What a cube's cut is worked out from: its shape and the roof over a gable.
    fn key(&self, cube: IVec3) -> Option<u64> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.shape_of(cube).code().hash(&mut hasher);
        self.gables.get(&cube.to_array()).hash(&mut hasher);
        Some(hasher.finish())
    }
}

/// The face sunk behind every lintel and sill set into it, so each sits in the wall: each
/// piece bedded, its box on the face with its lap, filed under every cube within half a
/// metre of it, further than any reach a surface asks of it (`surface::Heights::bed`), so a
/// point looks only at those near.
struct Beds<'a> {
    beds: Vec<crate::dress::Bed>,
    boxes: Vec<(Vec3, Vec3, &'a Piece)>,
    at: terra_gen::hash::QuickMap<IVec3, Vec<usize>>,
}

impl<'a> Beds<'a> {
    fn of(kit: &Kit, own: &[&'a Placed], edge: &[&'a Placed]) -> Beds<'a> {
        let bedded: Vec<(&Piece, crate::dress::Bed)> = own
            .iter()
            .chain(edge)
            .filter_map(|(_, piece, _)| Some((piece, crate::dress::bed(piece, kit)?)))
            .collect();
        // The pieces bedded within two cubes of a cube, each its box on the face with its lap.
        let boxes: Vec<(Vec3, Vec3, &Piece)> = bedded
            .iter()
            .map(|(piece, b)| {
                let corners = [b.from, b.to].into_iter().flat_map(|end| {
                    [b.low, b.high]
                        .into_iter()
                        .flat_map(move |up| [0.0, b.face].map(|out| end + b.up * up + b.out * out))
                });
                let (low, high) =
                    corners.fold((Vec3::MAX, Vec3::MIN), |(l, h), c| (l.min(c), h.max(c)));
                (low, high, *piece)
            })
            .collect();
        let beds: Vec<crate::dress::Bed> = bedded.iter().map(|(_, bed)| *bed).collect();
        let mut at: terra_gen::hash::QuickMap<IVec3, Vec<usize>> = Default::default();
        for (index, (low, high, _)) in boxes.iter().enumerate() {
            let margin = Vec3::splat(BED_REACH_M);
            let (from, to) = (
                ((*low - margin) / CELL_M).floor().as_ivec3(),
                ((*high + margin) / CELL_M).floor().as_ivec3(),
            );
            for x in from.x..=to.x {
                for y in from.y..=to.y {
                    for z in from.z..=to.z {
                        at.entry(IVec3::new(x, y, z)).or_default().push(index);
                    }
                }
            }
        }
        Beds { beds, boxes, at }
    }

    fn is_empty(&self) -> bool {
        self.beds.is_empty()
    }

    /// Where a point is against the pieces bedded near it (`surface::Heights::bed`).
    fn at(&self, p: Vec3) -> Option<(f32, f32, bool)> {
        self.at
            .get(&(p / CELL_M).floor().as_ivec3())?
            .iter()
            .filter_map(|&index| self.beds[index].at(p))
            .max_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// A number for the pieces bedded within two cubes of a cube.
    fn key(&self, cube: IVec3) -> u64 {
        let reach = Vec3::splat(2.0 * CELL_M + 0.1);
        let low = cube.as_vec3() * CELL_M - reach;
        let high = (cube + IVec3::ONE).as_vec3() * CELL_M + reach;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for (l, h, piece) in &self.boxes {
            if l.cmple(high).all() && h.cmpge(low).all() {
                piece.hash(&mut hasher);
            }
        }
        hasher.finish()
    }
}

/// Where damage has taken stones out of a kit's walls (D305): the surface is pushed back a
/// stone's depth over each stone gone, and the mortar round it, by the same cells, and the
/// same loss at a point, as the texture arrays' shader draws them gone with.
struct Holes {
    worn: BTreeMap<[i32; 3], u8>,
    /// The kit's masonry's cells and how deep a stone gone leaves its hole, if it breaks
    /// at its joints.
    joints: Option<(std::sync::Arc<Vec<u8>>, f32)>,
    metres: [f32; 2],
    /// A cube's damage round it, by a number, or `None` where none round it can take a
    /// stone anywhere, asked once a cube.
    fall_keys: std::cell::RefCell<terra_gen::hash::QuickMap<IVec3, Option<u64>>>,
    /// What was lost at a stone's middle, read there once for each stone of each wall's
    /// face (`cells::middle` is the very same from every texel of a stone): a stone is
    /// whole or gone, never cut across.
    at_middles: std::cell::RefCell<terra_gen::hash::QuickMap<[i32; 5], f32>>,
    /// Whether any stone has gone from a cube's sides, asked once a cube.
    has_holes: std::cell::RefCell<terra_gen::hash::QuickMap<IVec3, bool>>,
}

impl Holes {
    fn of(
        section: &Section,
        kit: &Kit,
        relief: Option<&crate::relief::Relief>,
        look: &Look,
    ) -> Holes {
        let origin = section.at * terra_build::cubes::SECTION;
        let worn = worn_within(
            section.base,
            section.kits,
            origin - IVec3::splat(2),
            origin + IVec3::splat(terra_build::cubes::SECTION + 1),
        );
        let joints = match kit.damage.get(&Material::Masonry) {
            Some(&terra_build::kit::Breaks::Joints { share, depth }) => {
                relief.map(|relief| (relief.cells(share), depth))
            }
            _ => None,
        };
        Holes {
            worn,
            joints,
            metres: look.metres,
            fall_keys: Default::default(),
            at_middles: Default::default(),
            has_holes: Default::default(),
        }
    }

    /// A cube can lose stones where a cube round it has lost enough for one to go
    /// anywhere: a number for that damage, which draws its faces finely.
    fn fall_key(&self, cube: IVec3) -> Option<u64> {
        if let Some(&key) = self.fall_keys.borrow().get(&cube) {
            return key;
        }
        let mut key = 0u64;
        let mut most = 0u8;
        for (index, near) in round_it(cube).enumerate() {
            if let Some(&quarters) = self.worn.get(&near.to_array()) {
                most = most.max(quarters);
                key = terra_build_layout::layout::splitmix(
                    key ^ u64::from(quarters) ^ (index as u64) << 8,
                );
            }
        }
        // None where no cube round it has lost enough for a stone to go anywhere.
        let can_fall =
            f32::from(most.min(4)) / 4.0 * terra_build::loss::PATCH_MOST >= crate::cells::FALLS[0];
        let key = can_fall.then_some(key);
        self.fall_keys.borrow_mut().insert(cube, key);
        key
    }

    /// How deep a stone's hole is at a point of a wall's face, wherever it is.
    fn anywhere(&self, p: Vec3, normal: Vec3) -> f32 {
        let Some((cells, depth)) = &self.joints else {
            return 0.0;
        };
        // Its walls' faces; not its tops, its floors' and copings' flagstones.
        if normal.y.abs() > 0.5 {
            return 0.0;
        }
        // Nothing near it damaged enough: nothing gone.
        let cube = ((p - normal * 0.02) / CELL_M).floor().as_ivec3();
        if self.fall_key(cube).is_none() {
            return 0.0;
        }
        // Too little lost here for any stone near to have gone, a stone's middle no further
        // than a stone from it.
        if terra_build::loss::struck(&self.worn, p, normal)
            < crate::cells::FALLS[0] - STONE_REACH_LOST
        {
            return 0.0;
        }
        let metres = self.metres;
        let uv = crate::mesh::planar(p, normal, metres);
        let axis = normal.abs().max_position();
        let plane = (p[axis] * 1000.0).round() as i32;
        let side = axis as i32 * 2 + i32::from(normal[axis] > 0.0);
        // What was lost at a place on the texture: at the point of the wall's face there,
        // the texture being mapped flat on it (`mesh::planar`, upright on walls).
        let at_uv = |to: [f32; 2]| {
            let n = CELLS_F * 2.0;
            let key = [
                (to[0] * n).round() as i32,
                (to[1] * n).round() as i32,
                plane,
                side,
                0,
            ];
            if let Some(&lost) = self.at_middles.borrow().get(&key) {
                return lost;
            }
            let (da, db) = ((to[0] - uv[0]) * metres[0], (to[1] - uv[1]) * metres[1]);
            let n = normal.abs();
            let q = if n.x >= n.z {
                p + Vec3::new(0.0, -db, da)
            } else {
                p + Vec3::new(da, -db, 0.0)
            };
            let lost = terra_build::loss::struck(&self.worn, q, normal);
            self.at_middles.borrow_mut().insert(key, lost);
            lost
        };
        // A stone there or gone, read from the cells' distance field so its outline is
        // smooth: the surface (`surface::draw_slots`) puts the points beside a hole on it,
        // so its sides stand square along its stones' edges, and marks what is sunk, which
        // the texture arrays' shader draws as the hollow.
        if crate::cells::fallen(cells, uv, &at_uv) {
            *depth
        } else {
            0.0
        }
    }

    /// Whether any stone has gone from a cube's sides, read every few centimetres over
    /// each: only those are drawn finely, and holed; the rest of a damaged wall as it was.
    fn own_hole(&self, cube: IVec3) -> bool {
        if let Some(&has) = self.has_holes.borrow().get(&cube) {
            return has;
        }
        let low = cube.as_vec3() * CELL_M;
        let has = self.fall_key(cube).is_some()
            && [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z]
                .into_iter()
                .any(|normal| {
                    let axis = normal.abs().max_position();
                    let across = if axis == 0 { 2 } else { 0 };
                    let mut p = low;
                    p[axis] += if normal[axis] > 0.0 { CELL_M } else { 0.0 };
                    (0..=HOLE_LOOKS).any(|i| {
                        (0..=HOLE_LOOKS).any(|j| {
                            let mut q = p;
                            q[across] += CELL_M * i as f32 / HOLE_LOOKS as f32;
                            q.y += CELL_M * j as f32 / HOLE_LOOKS as f32;
                            self.anywhere(q, normal) > 0.0
                        })
                    })
                });
        self.has_holes.borrow_mut().insert(cube, has);
        has
    }

    /// And a cube beside one, which the hole's mortar or a stone across their edge may
    /// reach into, so a hole is never cut off square at a cube's edge.
    fn has_hole(&self, cube: IVec3) -> bool {
        self.own_hole(cube)
            || [
                IVec3::X,
                IVec3::NEG_X,
                IVec3::Y,
                IVec3::NEG_Y,
                IVec3::Z,
                IVec3::NEG_Z,
            ]
            .into_iter()
            .any(|step| self.own_hole(cube + step))
    }

    /// How deep a stone's hole is at a point, where a cube near it has one.
    fn hole(&self, p: Vec3, normal: Vec3) -> f32 {
        if !self.has_hole(((p - normal * 0.02) / CELL_M).floor().as_ivec3()) {
            return 0.0;
        }
        self.anywhere(p, normal)
    }

    /// A cube is holed where a stone has gone from it, which damage round it can take
    /// (`loss::struck`): a number for that damage.
    fn holed(&self, cube: IVec3) -> Option<u64> {
        self.joints.as_ref()?;
        self.fall_key(cube).filter(|_| self.has_hole(cube))
    }
}

/// What a kit's sets of cubes in a section are drawn from.
struct Blocks<'a> {
    section: &'a Section<'a>,
    kit: &'a Kit,
    detail: Detail,
    first: bool,
    sorted: &'a Sorted,
    parts: &'a Wanted,
    cutting: &'a Cutting<'a>,
    cut: &'a crate::surface::Cuts<'a>,
    look: &'a Look,
    heights: &'a Heights<'a>,
    hides_always: &'a dyn Fn(IVec3) -> bool,
    keep_in: &'a dyn Fn(IVec3) -> Option<usize>,
    keep_whole: &'a dyn Fn(IVec3) -> Option<usize>,
    slot_of: &'a dyn Fn(IVec3) -> usize,
    slots: usize,
}

impl Blocks<'_> {
    /// Which part each cube of a set is drawn into, if it is drawn: a part's cubes only if
    /// it is wanted for what is in, the whole section for the plan's shell and the next.
    fn keep_for(&self, set: usize) -> &dyn Fn(IVec3) -> Option<usize> {
        if set == 0 || (set == 2 && !self.first) {
            self.keep_in
        } else {
            self.keep_whole
        }
    }

    fn by_slot(&self) -> Vec<Surface> {
        (0..self.slots)
            .map(|_| Surface {
                sharing: true,
                ..Surface::default()
            })
            .collect()
    }

    /// A set's cubes, whole and cut, and its stairs and posts, into each part's surfaces:
    /// what is in (0), a plan's shell (1) or what goes in next (2).
    fn draw_set(&self, set: usize, caching: &mut Option<BlockCaching>, out: &mut [Surfaces]) {
        let Sorted {
            sets,
            shaped,
            gables,
            ..
        } = self.sorted;
        let first = self.first;
        if (sets[set].is_empty() && shaped[set].is_empty()) || (set == 1 && !first) {
            return;
        }
        let (base, kit) = (self.section.base, self.kit);
        let hides_always = self.hides_always;
        // What is built hides what is not; the rest show their own faces.
        let built = &sets[0];
        let hides = |at: IVec3| hides_always(at) || (set != 0 && built.contains(&at.to_array()));
        // Its whole cubes and its cut ones, drawn apart into one surface.
        let kin =
            |at: IVec3| sets[set].contains(&at.to_array()) || shaped[set].contains(&at.to_array());
        // A plan's blocks are a plain shell, a step to a face, its texture a cube across,
        // so a ghost's lines fall on the cubes' edges.
        let plan_look = Look {
            bevel: kit.bevel * 0.5,
            relief: 0.0,
            steps: 1,
            metres: [CELL_M, CELL_M],
            top: None,
            packed: None,
        };
        let flat = |_: [f32; 2]| 0.5;
        let plan_heights = Heights {
            sides: &flat,
            top: &flat,
            bed: None,
            bed_key: None,
            hole: None,
            holed: None,
        };
        let (look, heights) = if set == 1 {
            (&plan_look, &plan_heights)
        } else {
            (self.look, self.heights)
        };
        // A plan's shell plain; what is built in the kit's tint.
        let colour = if set == 1 { [1.0; 3] } else { kit.tint };
        let (mut masonry, mut tops) = (self.by_slot(), self.by_slot());
        // Where only some parts are drawn, only the cubes near enough them to shape their
        // faces are worked on.
        let near_parts: BTreeSet<[i32; 3]>;
        let cubes = if self.parts.some && (set == 0 || (set == 2 && !first)) {
            near_parts = sets[set]
                .iter()
                .copied()
                .filter(|&at| self.parts.near(IVec3::from_array(at)))
                .collect();
            &near_parts
        } else {
            &sets[set]
        };
        crate::surface::draw_slots(
            cubes,
            self.keep_for(set),
            &kin,
            &hides,
            look,
            heights,
            self.cut,
            colour,
            &mut masonry,
            &mut tops,
            cached(caching, set as u32),
        );
        // Halves, slopes, their corners and the same upside down: the surface of each run
        // of them cut down to its tops and up to its undersides (D231), hidden where whole
        // cubes are against them, but for a whole cube on a cut top or under a cut
        // underside, which would hide the cut.
        let shape_of = |cube: IVec3| self.cutting.shape_of(cube);
        let ours_whole = |at: IVec3| {
            base.cube(at)
                .is_some_and(|c| c.kind == kit.kind() && c.fill == Fill::Solid && !c.shape.is_cut())
        };
        let planed = |shape: terra_build::Shape| shape.planed_top() || shape.planed_under();
        let cut_down: BTreeSet<[i32; 3]> = shaped[set]
            .iter()
            .copied()
            .filter(|&at| {
                planed(shape_of(IVec3::from_array(at)))
                    || matches!(gables.get(&at), Some((_, layout::Carried::Down)))
            })
            .collect();
        let over_a_cut = |at: IVec3| {
            let (under, over) = (at - IVec3::Y, at + IVec3::Y);
            (cut_down.contains(&under.to_array()) && shape_of(under).planed_top())
                || (cut_down.contains(&over.to_array()) && shape_of(over).planed_under())
        };
        if !cut_down.is_empty() {
            let hides = |at: IVec3| hides(at) || (ours_whole(at) && !over_a_cut(at));
            crate::surface::draw_slots(
                &cut_down,
                self.keep_for(set),
                &kin,
                &hides,
                look,
                heights,
                self.cut,
                colour,
                &mut masonry,
                &mut tops,
                None,
            );
        }
        let mut faces = self.by_slot();
        self.stairs_and_posts(set, look, heights, colour, &mut faces, &mut tops);
        for (slot, ((masonry, faces), tops)) in masonry.into_iter().zip(faces).zip(tops).enumerate()
        {
            let into = &mut out[slot];
            into.of(Material::Masonry).append(masonry);
            into.of(Material::Masonry).append(faces);
            into.of(Material::Floor).append(tops);
        }
    }

    /// Stairs either way up, a stone a step, and posts, a drum a cube; a framed kind's
    /// posts are its timber's (`layout::frames`).
    fn stairs_and_posts(
        &self,
        set: usize,
        look: &Look,
        heights: &Heights,
        colour: [f32; 3],
        faces: &mut [Surface],
        tops: &mut [Surface],
    ) {
        let kit = self.kit;
        let pillar = match kit.dress.roles.get(&layout::Role::Post) {
            _ if kit.framing.is_some() => 0.0,
            Some(terra_build::kit::Dress::Pillar { width }) => *width,
            _ => CELL_M,
        };
        for &at in &self.sorted.shaped[set] {
            let cube = IVec3::from_array(at);
            if self.keep_for(set)(cube).is_none() {
                continue;
            }
            let low = cube.as_vec3() * CELL_M;
            let half = CELL_M * 0.5;
            let boxes: Vec<(Vec3, Vec3)> = match self.cutting.shape_of(cube) {
                shape if shape.stepped() => shape
                    .steps()
                    .into_iter()
                    .map(|(a, b)| {
                        (
                            low + Vec3::from_array(a) * CELL_M,
                            low + Vec3::from_array(b) * CELL_M,
                        )
                    })
                    .collect(),
                terra_build::Shape::Post if pillar <= 0.0 => Vec::new(),
                terra_build::Shape::Post => {
                    let middle = low + Vec3::new(half, 0.0, half);
                    let reach = Vec3::new(pillar * 0.5, 0.0, pillar * 0.5);
                    vec![(middle - reach, middle + reach + Vec3::Y * CELL_M)]
                }
                _ => Vec::new(),
            };
            let slot = (self.slot_of)(cube);
            for (min, max) in boxes {
                crate::surface::slab(
                    min,
                    max,
                    look,
                    heights,
                    colour,
                    &mut faces[slot],
                    &mut tops[slot],
                );
            }
        }
    }

    /// A framed kind's cores where its infill is not in: the wattle, drawn as the infill's
    /// surface is, in its own material and relief, set back a little behind where the daub
    /// will be, hidden by the infill that is in.
    fn draw_cores(&self, caching: &mut Option<BlockCaching>, out: &mut [Surfaces]) {
        let kit = self.kit;
        let cores = &self.sorted.cores;
        let Some((framing, material)) = kit
            .framing
            .as_ref()
            .and_then(|framing| Some((framing, framing.core?)))
        else {
            return;
        };
        if cores.is_empty() {
            return;
        }
        let near = matches!(self.detail, Detail::Near | Detail::Mid);
        let relief = self.section.maps.core(kit).filter(|_| near);
        let woven = |uv: [f32; 2]| relief.map_or(0.25, |relief| relief.at(uv) * 0.5);
        let core_look = Look {
            relief: if near { framing.core_relief } else { 0.0 },
            metres: kit.textures.get(&material).map_or([1.0, 1.0], |t| t.metres),
            top: None,
            packed: None,
            ..*self.look
        };
        let core_heights = Heights {
            sides: &woven,
            top: &woven,
            bed: None,
            bed_key: None,
            hole: None,
            holed: None,
        };
        let built = &self.sorted.sets[0];
        let hides_always = self.hides_always;
        let hides = |at: IVec3| hides_always(at) || built.contains(&at.to_array());
        let (mut woven_by, mut tops) = (self.by_slot(), self.by_slot());
        crate::surface::draw_slots(
            cores,
            self.keep_in,
            &|at| cores.contains(&at.to_array()),
            &hides,
            &core_look,
            &core_heights,
            self.cut,
            [1.0; 3],
            &mut woven_by,
            &mut tops,
            cached(caching, 3),
        );
        for (slot, (woven, tops)) in woven_by.into_iter().zip(tops).enumerate() {
            let into = out[slot].of(material);
            into.append(woven);
            into.append(tops);
        }
    }
}
