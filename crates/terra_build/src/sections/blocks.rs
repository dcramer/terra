//! A block kind's cubes drawn as their surface (D197), with the holes damage makes in it
//! and the beds pieces sit in.

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

/// A block kind's cubes in a section as its surface (D197): those in, those not yet, and
/// the next, each closed where it meets the others but for what is already built, with
/// the cubes a step beyond the section's edge worked out but not drawn, so its edges meet
/// the next section's as one surface.
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
    let mut sets: [BTreeSet<[i32; 3]>; 3] = Default::default();
    // Cubes cut to a shape (D193), drawn apart from the whole ones: halves and slopes as
    // the surface cut down to their tops, stairs as a stone a tread, posts as drums.
    let mut shaped: [BTreeSet<[i32; 3]>; 3] = Default::default();
    // A gable's cubes, with the roof's underside over each, which cuts them.
    let mut gables: BTreeMap<[i32; 3], ([[i32; 3]; 3], layout::Carried)> = BTreeMap::new();
    // A framed kind's cubes whose core is in (D236).
    let mut cores: BTreeSet<[i32; 3]> = BTreeSet::new();
    for (_, piece, stage) in own.iter().chain(edge).copied() {
        let cube = match piece {
            Piece::Wattle { cube } => {
                if *stage == Stage::In || (*stage == Stage::Next && !first) {
                    cores.insert(cube.to_array());
                }
                continue;
            }
            Piece::Block { cube } => cube,
            Piece::Gable {
                cube,
                under,
                carried,
            } => {
                gables.insert(cube.to_array(), (*under, *carried));
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
        // One carried down into a roof's cube it stands on is cut from below at the deck, as a
        // shape with a planed underside is, and drawn with them.
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
            shaped[set].insert(cube.to_array());
        } else {
            sets[set].insert(cube.to_array());
        }
    }
    // The parts drawn this time, each as a box of cubes grown by as far as a face's shape
    // depends on what is round it and its neighbours' do on theirs, if not all of them.
    let section_low = section.at * crate::cubes::SECTION;
    let wanted_boxes: Vec<(IVec3, IVec3)> = (0..slots)
        .filter(|&slot| wanted(slot))
        .map(|slot| {
            let (size, side) = (CLUSTER, crate::cubes::SECTION / CLUSTER);
            let s = slot as i32;
            let low = section_low + IVec3::new(s % side, s / side % side, s / (side * side)) * size;
            (
                low - IVec3::splat(PARTS_REACH),
                low + IVec3::splat(size - 1 + PARTS_REACH),
            )
        })
        .collect();
    let parts_only = slots > 1 && wanted_boxes.len() < slots;
    let near_wanted = |at: IVec3| {
        wanted_boxes
            .iter()
            .any(|(low, high)| at.cmpge(*low).all() && at.cmple(*high).all())
    };
    // A core shows where its infill is not in, nor lit as next: neither over it.
    cores.retain(|at| !sets[0].contains(at) && !sets[2].contains(at));
    sets[1].retain(|at| !cores.contains(at));
    let shape_of = |cube: IVec3| {
        base.cube(cube)
            .filter(|c| c.kind == kit.kind())
            .map_or(crate::Shape::Whole, |c| c.shape)
    };
    let cut = |cube: IVec3, at: Vec3| {
        let [under_roof, roof] = gables
            .get(&cube.to_array())
            .map_or([f32::MIN, f32::MAX], |(under, carried)| {
                layout::roofs::carried_cut(*carried, cube, under, at)
            });
        let (u, v) = (
            (at.x / CELL_M - cube.x as f32).clamp(0.0, 1.0),
            (at.z / CELL_M - cube.z as f32).clamp(0.0, 1.0),
        );
        let shape = shape_of(cube);
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
    };
    // Most cubes are whole: their cut is never asked.
    let cut_at_all = |cube: IVec3| {
        let shape = shape_of(cube);
        gables.contains_key(&cube.to_array()) || shape.planed_top() || shape.planed_under()
    };
    // What a cube's cut is worked out from: its shape and the roof over a gable.
    let cut_key = |cube: IVec3| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        shape_of(cube).code().hash(&mut hasher);
        gables.get(&cube.to_array()).hash(&mut hasher);
        Some(hasher.finish())
    };
    let cut = crate::surface::Cuts {
        of: &cut_at_all,
        at: &cut,
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
    // The face sunk behind every lintel and sill set into it, so each sits in the wall.
    let bedded: Vec<(&Piece, crate::dress::Bed)> = own
        .iter()
        .chain(edge)
        .filter_map(|(_, piece, _)| Some((piece, crate::dress::bed(piece, kit)?)))
        .collect();
    // The pieces bedded within two cubes of a cube, each its box on the face with its lap.
    let bed_boxes: Vec<(Vec3, Vec3, &Piece)> = bedded
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
    // Each bed filed under every cube within half a metre of it, further than any reach a
    // surface asks of it (`surface::Heights::bed`), so a point looks only at those near.
    let mut beds_at: crate::mesh::QuickMap<IVec3, Vec<usize>> = Default::default();
    for (index, (low, high, _)) in bed_boxes.iter().enumerate() {
        let margin = Vec3::splat(BED_REACH_M);
        let (from, to) = (
            ((*low - margin) / CELL_M).floor().as_ivec3(),
            ((*high + margin) / CELL_M).floor().as_ivec3(),
        );
        for x in from.x..=to.x {
            for y in from.y..=to.y {
                for z in from.z..=to.z {
                    beds_at.entry(IVec3::new(x, y, z)).or_default().push(index);
                }
            }
        }
    }
    let bed = |p: Vec3| {
        beds_at
            .get(&(p / CELL_M).floor().as_ivec3())?
            .iter()
            .filter_map(|&index| beds[index].at(p))
            .max_by(|a, b| a.0.total_cmp(&b.0))
    };
    let bed_key = |cube: IVec3| {
        let reach = Vec3::splat(2.0 * CELL_M + 0.1);
        let low = cube.as_vec3() * CELL_M - reach;
        let high = (cube + IVec3::ONE).as_vec3() * CELL_M + reach;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for (l, h, piece) in &bed_boxes {
            if l.cmple(high).all() && h.cmpge(low).all() {
                piece.hash(&mut hasher);
            }
        }
        hasher.finish()
    };
    // Where damage has taken stones out of its walls (D305), the surface is pushed back a
    // stone's depth over each stone gone, and the mortar round it: by the same cells, and the same loss at a point, as the texture
    // arrays' shader draws them gone with.
    let origin = section.at * crate::cubes::SECTION;
    let worn = worn_within(
        base,
        section.kits,
        origin - IVec3::splat(2),
        origin + IVec3::splat(crate::cubes::SECTION + 1),
    );
    let joints = match kit.damage.get(&Material::Masonry) {
        Some(&crate::kit::Breaks::Joints { share, depth }) => {
            relief.map(|relief| (relief.cells(share), depth))
        }
        _ => None,
    };
    // A cube can lose stones where a cube round it has lost enough for one to go
    // anywhere: a number for that damage, which draws its faces finely.
    let fall_keys: std::cell::RefCell<crate::mesh::QuickMap<IVec3, Option<u64>>> =
        Default::default();
    let fall_key = |cube: IVec3| -> Option<u64> {
        if let Some(&key) = fall_keys.borrow().get(&cube) {
            return key;
        }
        let mut key = 0u64;
        let mut most = 0u8;
        for (index, near) in round_it(cube).enumerate() {
            if let Some(&quarters) = worn.get(&near.to_array()) {
                most = most.max(quarters);
                key = crate::layout::splitmix(key ^ u64::from(quarters) ^ (index as u64) << 8);
            }
        }
        // None where no cube round it has lost enough for a stone to go anywhere.
        let can_fall =
            f32::from(most.min(4)) / 4.0 * crate::loss::PATCH_MOST >= crate::cells::FALLS[0];
        let key = can_fall.then_some(key);
        fall_keys.borrow_mut().insert(cube, key);
        key
    };
    // What was lost at a stone's middle, read there once for each stone of each wall's face
    // (`cells::middle` is the very same from every texel of a stone): a stone is whole or
    // gone, never cut across.
    let at_middles: std::cell::RefCell<crate::mesh::QuickMap<[i32; 5], f32>> = Default::default();
    let hole_anywhere = |p: Vec3, normal: Vec3| -> f32 {
        let Some((cells, depth)) = &joints else {
            return 0.0;
        };
        // Its walls' faces; not its tops, its floors' and copings' flagstones.
        if normal.y.abs() > 0.5 {
            return 0.0;
        }
        // Nothing near it damaged enough: nothing gone.
        let cube = ((p - normal * 0.02) / CELL_M).floor().as_ivec3();
        if fall_key(cube).is_none() {
            return 0.0;
        }
        // Too little lost here for any stone near to have gone, a stone's middle no further
        // than a stone from it.
        if crate::loss::struck(&worn, p, normal) < crate::cells::FALLS[0] - STONE_REACH_LOST {
            return 0.0;
        }
        let uv = crate::mesh::planar(p, normal, look.metres);
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
            if let Some(&lost) = at_middles.borrow().get(&key) {
                return lost;
            }
            let (da, db) = (
                (to[0] - uv[0]) * look.metres[0],
                (to[1] - uv[1]) * look.metres[1],
            );
            let n = normal.abs();
            let q = if n.x >= n.z {
                p + Vec3::new(0.0, -db, da)
            } else {
                p + Vec3::new(da, -db, 0.0)
            };
            let lost = crate::loss::struck(&worn, q, normal);
            at_middles.borrow_mut().insert(key, lost);
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
    };
    // Whether any stone has gone from a cube's sides, read every few centimetres over each:
    // only those are drawn finely, and holed; the rest of a damaged wall as it was.
    let has_holes: std::cell::RefCell<crate::mesh::QuickMap<IVec3, bool>> = Default::default();
    let own_hole = |cube: IVec3| -> bool {
        if let Some(&has) = has_holes.borrow().get(&cube) {
            return has;
        }
        let low = cube.as_vec3() * CELL_M;
        let has = fall_key(cube).is_some()
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
                            hole_anywhere(q, normal) > 0.0
                        })
                    })
                });
        has_holes.borrow_mut().insert(cube, has);
        has
    };
    // And a cube beside one, which the hole's mortar or a stone across their edge may reach
    // into, so a hole is never cut off square at a cube's edge.
    let has_hole = |cube: IVec3| -> bool {
        own_hole(cube)
            || [
                IVec3::X,
                IVec3::NEG_X,
                IVec3::Y,
                IVec3::NEG_Y,
                IVec3::Z,
                IVec3::NEG_Z,
            ]
            .into_iter()
            .any(|step| own_hole(cube + step))
    };
    let hole = |p: Vec3, normal: Vec3| -> f32 {
        if !has_hole(((p - normal * 0.02) / CELL_M).floor().as_ivec3()) {
            return 0.0;
        }
        hole_anywhere(p, normal)
    };
    // A cube is holed where a stone has gone from it, which damage round it can take
    // (`loss::struck`): a number for that damage.
    let holed = |cube: IVec3| -> Option<u64> {
        joints.as_ref()?;
        fall_key(cube).filter(|_| has_hole(cube))
    };
    let heights = Heights {
        sides: &sides,
        top: &top,
        bed: (!beds.is_empty()).then_some(&bed as crate::surface::BedAt),
        bed_key: (!beds.is_empty()).then_some(&bed_key as &dyn Fn(IVec3) -> u64),
        hole: joints
            .is_some()
            .then_some(&hole as &dyn Fn(Vec3, Vec3) -> f32),
        holed: joints
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
    let keep = |at: IVec3| crate::cubes::section_of(at) == section.at;
    // A part's cubes drawn only if the part is wanted: what is in, but not the plan's
    // shell or the next piece, which the section has whole.
    let keep_in = |at: IVec3| {
        let slot = slot_of(at);
        (keep(at) && wanted(slot)).then_some(slot)
    };
    let keep_whole = |at: IVec3| keep(at).then(|| slot_of(at));
    let keep_for = |set: usize| -> &dyn Fn(IVec3) -> Option<usize> {
        if set == 0 || (set == 2 && !first) {
            &keep_in
        } else {
            &keep_whole
        }
    };
    let sharing = || Surface {
        sharing: true,
        ..Surface::default()
    };
    let by_slot = || -> Vec<Surface> { (0..slots).map(|_| sharing()).collect() };
    // A plan's blocks are a plain shell, a step to a face.
    // Its texture a cube across, so a ghost's lines fall on the cubes' edges.
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
    let mut out: [Vec<Surfaces>; 3] =
        [0, 1, 2].map(|_| (0..slots).map(|_| Surfaces::sharing()).collect());
    for set in 0..3 {
        if (sets[set].is_empty() && shaped[set].is_empty()) || (set == 1 && !first) {
            continue;
        }
        // What is built hides what is not; the rest show their own faces.
        let built = &sets[0];
        let hides = |at: IVec3| hides_always(at) || (set != 0 && built.contains(&at.to_array()));
        // Its whole cubes and its cut ones, drawn apart into one surface.
        let kin =
            |at: IVec3| sets[set].contains(&at.to_array()) || shaped[set].contains(&at.to_array());
        let (look, heights) = if set == 1 {
            (&plan_look, &plan_heights)
        } else {
            (&look, &heights)
        };
        // A plan's shell plain; what is built in the kit's tint.
        let colour = if set == 1 { [1.0; 3] } else { kit.tint };
        let (mut masonry, mut tops) = (by_slot(), by_slot());
        // Where only some parts are drawn, only the cubes near enough them to shape their
        // faces are worked on.
        let near_parts: BTreeSet<[i32; 3]>;
        let cubes = if parts_only && (set == 0 || (set == 2 && !first)) {
            near_parts = sets[set]
                .iter()
                .copied()
                .filter(|&at| near_wanted(IVec3::from_array(at)))
                .collect();
            &near_parts
        } else {
            &sets[set]
        };
        crate::surface::draw_slots(
            cubes,
            keep_for(set),
            &kin,
            &hides,
            look,
            heights,
            &cut,
            colour,
            &mut masonry,
            &mut tops,
            cached(&mut caching, set as u32),
        );
        // Halves, slopes, their corners and the same upside down: the surface of each run
        // of them cut down to its tops and up to its undersides (D231), hidden where whole
        // cubes are against them, but for a whole cube on a cut top or under a cut
        // underside, which would hide the cut.
        let ours_whole = |at: IVec3| {
            base.cube(at)
                .is_some_and(|c| c.kind == kit.kind() && c.fill == Fill::Solid && !c.shape.is_cut())
        };
        let planed = |shape: crate::Shape| shape.planed_top() || shape.planed_under();
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
                keep_for(set),
                &kin,
                &hides,
                look,
                heights,
                &cut,
                colour,
                &mut masonry,
                &mut tops,
                None,
            );
        }
        // Stairs either way up, a stone a step, and posts, a drum a cube; a framed kind's
        // posts are its timber's (`layout::frames`).
        let pillar = match kit.dress.roles.get(&layout::Role::Post) {
            _ if kit.framing.is_some() => 0.0,
            Some(crate::kit::Dress::Pillar { width }) => *width,
            _ => CELL_M,
        };
        let mut faces = by_slot();
        for &at in &shaped[set] {
            let cube = IVec3::from_array(at);
            if keep_for(set)(cube).is_none() {
                continue;
            }
            let low = cube.as_vec3() * CELL_M;
            let half = CELL_M * 0.5;
            let boxes: Vec<(Vec3, Vec3)> = match shape_of(cube) {
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
                crate::Shape::Post if pillar <= 0.0 => Vec::new(),
                crate::Shape::Post => {
                    let middle = low + Vec3::new(half, 0.0, half);
                    let reach = Vec3::new(pillar * 0.5, 0.0, pillar * 0.5);
                    vec![(middle - reach, middle + reach + Vec3::Y * CELL_M)]
                }
                _ => Vec::new(),
            };
            let slot = slot_of(cube);
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
        for (slot, ((masonry, faces), tops)) in masonry.into_iter().zip(faces).zip(tops).enumerate()
        {
            let into = &mut out[set][slot];
            into.of(Material::Masonry).append(masonry);
            into.of(Material::Masonry).append(faces);
            into.of(Material::Floor).append(tops);
        }
    }
    // A framed kind's cores where its infill is not in: the wattle, drawn as the infill's
    // surface is, in its own material and relief, set back a little behind where the daub
    // will be, hidden by the infill that is in.
    if let Some((framing, material)) = kit
        .framing
        .as_ref()
        .and_then(|framing| Some((framing, framing.core?)))
        && !cores.is_empty()
    {
        let near = matches!(detail, Detail::Near | Detail::Mid);
        let relief = section.maps.core(kit).filter(|_| near);
        let woven = |uv: [f32; 2]| relief.map_or(0.25, |relief| relief.at(uv) * 0.5);
        let core_look = Look {
            relief: if near { framing.core_relief } else { 0.0 },
            metres: kit.textures.get(&material).map_or([1.0, 1.0], |t| t.metres),
            top: None,
            packed: None,
            ..look
        };
        let core_heights = Heights {
            sides: &woven,
            top: &woven,
            bed: None,
            bed_key: None,
            hole: None,
            holed: None,
        };
        let built = &sets[0];
        let hides = |at: IVec3| hides_always(at) || built.contains(&at.to_array());
        let (mut woven_by, mut tops) = (by_slot(), by_slot());
        crate::surface::draw_slots(
            &cores,
            &keep_in,
            &|at| cores.contains(&at.to_array()),
            &hides,
            &core_look,
            &core_heights,
            &cut,
            [1.0; 3],
            &mut woven_by,
            &mut tops,
            cached(&mut caching, 3),
        );
        for (slot, (woven, tops)) in woven_by.into_iter().zip(tops).enumerate() {
            let into = out[0][slot].of(material);
            into.append(woven);
            into.append(tops);
        }
    }
    out
}
