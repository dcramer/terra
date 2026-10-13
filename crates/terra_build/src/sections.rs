//! A base drawn a section at a time (docs/building-plan.md, drawing; D211). Each section of
//! cubes, 16 a side, is laid alone (`layout::lay_section`): the pieces whose middle is in
//! it, every long member belonging to one section only. Its built pieces are merged into a
//! surface per kind and material, its long members with their corners shared; the next
//! piece to go in is drawn as it will look, and the rest of a plan as ghosts, a box or a
//! prism a piece for the game to draw instanced. Far off it is drawn coarser, its textures
//! baked into the colours of its corners, one surface; further still, coarser again.
//!
//! Pure data, so it runs on any thread: the game lays and merges sections on its workers.

use crate::kit::{Kit, Kits, Pattern, Profile};
use crate::layout::ground::Flight;
use crate::layout::roofs::RoofPart;
use crate::layout::{self, Piece};
use crate::mesh::{Material, Surface, Surfaces};
use crate::relief::Maps;
use crate::surface::{Heights, Look};
use crate::{Base, CELL_M, Fill, Kind};
use glam::{IVec3, Mat3, Quat, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

/// How far a piece is in, as it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    In,
    /// What goes in next, lit.
    Next,
    /// Not yet in: the ghost.
    Ghost,
    /// Not drawn: a door or window, which is a prop fitted into its opening.
    Left,
}

/// How far a base's pieces are in.
#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
    /// All of it, its doors and windows drawn by its kit: a village's house.
    Whole,
    /// As far as each cube is filled, these cubes lit as what goes in next; its doors and
    /// windows are props fitted apart: the player's base.
    Cubes { next: BTreeSet<[i32; 3]> },
    /// So much of it, in the order its pieces go in, doors and windows not yet: the kit
    /// sheet's going-up and plan columns.
    Fraction(f32),
}

/// A section to lay and draw: where it is, in sections, and what it is drawn from.
pub struct Section<'a> {
    pub base: &'a Base,
    pub kits: &'a Kits,
    pub maps: &'a Maps,
    pub at: IVec3,
    pub progress: &'a Progress,
    /// The base's door steps, worked out where the ground is known.
    pub flights: &'a [Flight],
}

/// A piece laid, the kit it is of, by its place in the kits, and how far in it is.
pub type Placed = (usize, Piece, Stage);

/// A section's pieces as laid: its own, those a cube beyond its edge, which its block
/// kinds' surface needs, and its door steps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Laid {
    pub pieces: Vec<Placed>,
    pub edge: Vec<Placed>,
    pub steps: Vec<(Vec3, Vec3)>,
}

impl Laid {
    /// A number for what it holds, so a section whose pieces have not changed is not
    /// merged again.
    pub fn key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.pieces.hash(&mut hasher);
        self.edge.hash(&mut hasher);
        for (a, b) in &self.steps {
            for v in [a, b] {
                v.to_array().map(f32::to_bits).hash(&mut hasher);
            }
        }
        hasher.finish()
    }
}

/// Every section a base may draw something in: those with cubes; those beside them that
/// their cubes come within a few of, which overhangs, eaves and steps reach into; and
/// those its roofs rise through.
pub fn drawable(base: &Base) -> BTreeSet<[i32; 3]> {
    let side = crate::cubes::SECTION;
    let mut out = BTreeSet::new();
    let mut reach = |low: IVec3, high: IVec3| {
        let (from, to) = (
            crate::cubes::section_of(low),
            crate::cubes::section_of(high),
        );
        for x in from.x..=to.x {
            for y in from.y..=to.y {
                for z in from.z..=to.z {
                    out.insert([x, y, z]);
                }
            }
        }
    };
    for section in base.store().sections() {
        let origin = section * side;
        let cubes = base.store().within(origin, origin + IVec3::splat(side - 1));
        let Some(low) = cubes.iter().map(|(at, _)| *at).reduce(IVec3::min) else {
            continue;
        };
        let high = cubes.iter().map(|(at, _)| *at).fold(low, IVec3::max);
        reach(low - IVec3::splat(REACH), high + IVec3::splat(REACH));
    }
    for (_, roof) in base.roofs() {
        let section = roof.section;
        let (min, max) = (
            IVec3::from_array(section.min),
            IVec3::from_array(section.max),
        );
        let span = (max - min).max_element() + 1;
        let top = crate::wall_layer(section.storey()) + crate::STOREY_CUBES;
        let rise = libm::ceilf(section.pitch * span as f32) as i32 + REACH;
        reach(
            IVec3::new(min.x - REACH, top - REACH, min.z - REACH),
            IVec3::new(max.x + REACH, top + rise, max.z + REACH),
        );
    }
    out
}

/// How far past its cubes a base's pieces may reach, in cubes.
const REACH: i32 = 3;

/// The sections and those round them.
fn round(sections: impl IntoIterator<Item = IVec3>) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    for section in sections {
        for offset in NEIGHBOURS.iter().chain([&IVec3::ZERO]) {
            out.insert((section + *offset).to_array());
        }
    }
    out
}

/// The 26 sections round one.
const NEIGHBOURS: [IVec3; 26] = {
    let mut out = [IVec3::ZERO; 26];
    let mut i = 0;
    let mut n = 0;
    while n < 27 {
        let offset = IVec3::new(n / 9 - 1, (n / 3) % 3 - 1, n % 3 - 1);
        if n != 13 {
            out[i] = offset;
            i += 1;
        }
        n += 1;
    }
    out
};

/// The sections to lay again after a change that marked these (`Base::take_dirty`): every
/// section of the buildings the marks are in or touch, and those round them. A layout
/// reaches across a whole building, as a log's joins along its wall, a roof's kind from
/// the walls under it and a deck's boards along its longer side do, but never to another
/// building with empty sections between them. Laid again, a section whose pieces are the
/// same ([`Laid::key`]) need not be merged again.
pub fn affected(base: &Base, marked: &BTreeSet<[i32; 3]>) -> BTreeSet<[i32; 3]> {
    let occupied: BTreeSet<[i32; 3]> = base.store().sections().map(|s| s.to_array()).collect();
    let mut found: BTreeSet<[i32; 3]> = marked.clone();
    let mut frontier: Vec<IVec3> = found.iter().map(|&at| IVec3::from_array(at)).collect();
    while let Some(at) = frontier.pop() {
        for offset in NEIGHBOURS {
            let next = at + offset;
            if occupied.contains(&next.to_array()) && found.insert(next.to_array()) {
                frontier.push(next);
            }
        }
    }
    // Those found, and those round them that anything may be drawn in.
    let near = round(found.iter().copied().map(IVec3::from_array));
    let mut out: BTreeSet<[i32; 3]> = drawable(base).intersection(&near).copied().collect();
    out.extend(found);
    out
}

/// How far a section's cubes are searched for the kinds whose kits may lay pieces in it.
const KINDS_REACH: i32 = 4;

/// Lays a section: every kit's pieces in it, with how far each is in.
pub fn lay(section: &Section) -> Laid {
    let (base, kits) = (section.base, section.kits);
    let origin = section.at * crate::cubes::SECTION;
    // Only the kits whose kinds are there, in its cubes or round them, or roof it.
    let near = layout::Scope {
        low: origin - IVec3::splat(KINDS_REACH),
        high: origin + IVec3::splat(crate::cubes::SECTION - 1 + KINDS_REACH),
    };
    let mut kinds: BTreeSet<Kind> = near
        .cubes(base, None)
        .into_iter()
        .map(|(_, cube)| cube.kind)
        .collect();
    kinds.extend(base.roofs().filter_map(|(_, roof)| base.roof_kind(roof)));
    let mut out = Laid::default();
    let low = origin - IVec3::ONE;
    let high = origin + IVec3::splat(crate::cubes::SECTION);
    for (index, kit) in kits.0.iter().enumerate() {
        if !kinds.contains(&kit.kind()) {
            continue;
        }
        let (own, edge) = match section.progress {
            Progress::Fraction(fraction) => {
                // In the order the whole base goes in, so all of it.
                let all = layout::lay_among(base, kit, kits).pieces;
                let done = done(all.len(), *fraction);
                let (mut own, mut edge) = (Vec::new(), Vec::new());
                for (order, piece) in all.into_iter().enumerate() {
                    let stage = match order.cmp(&done) {
                        _ if matches!(piece, Piece::Opening { .. }) => Stage::Left,
                        std::cmp::Ordering::Less => Stage::In,
                        std::cmp::Ordering::Equal => Stage::Next,
                        std::cmp::Ordering::Greater => Stage::Ghost,
                    };
                    let at = piece.anchor();
                    if crate::cubes::section_of(at) == section.at {
                        own.push((index, piece, stage));
                    } else if at.cmpge(low).all() && at.cmple(high).all() {
                        edge.push((index, piece, stage));
                    }
                }
                (own, edge)
            }
            progress => {
                let (own, edge) = layout::lay_section_and_edge(base, kit, kits, section.at);
                let stage = |piece: &Piece| match progress {
                    Progress::Cubes { next } => stage_by_cubes(base, piece, next),
                    _ => Stage::In,
                };
                (
                    own.pieces
                        .into_iter()
                        .map(|piece| (index, stage(&piece), piece))
                        .map(|(index, stage, piece)| (index, piece, stage))
                        .collect(),
                    edge.into_iter()
                        .filter(|piece| matches!(piece, Piece::Block { .. } | Piece::Gable { .. }))
                        .map(|piece| (index, stage(&piece), piece))
                        .map(|(index, stage, piece)| (index, piece, stage))
                        .collect(),
                )
            }
        };
        out.pieces.extend(own);
        out.edge.extend(edge);
    }
    for flight in section.flights {
        for &(min, max) in &flight.steps {
            let middle = ((min + max) * 0.5 / CELL_M).floor().as_ivec3();
            if crate::cubes::section_of(middle) == section.at {
                out.steps.push((min, max));
            }
        }
    }
    out
}

/// How many of so many pieces are in, so far through.
pub fn done(count: usize, fraction: f32) -> usize {
    ((count as f32 * fraction.clamp(0.0, 1.0)) as usize).min(count)
}

/// What a walker meets of a base shown so far through: the cubes of the pieces in.
pub fn standing(base: &Base, kits: &Kits, fraction: f32) -> Base {
    let laid: Vec<Vec<Piece>> = kits
        .0
        .iter()
        .map(|kit| layout::lay_among(base, kit, kits).pieces)
        .collect();
    base.standing(
        laid.iter()
            .flat_map(|pieces| pieces[..done(pieces.len(), fraction)].iter()),
        false,
    )
}

/// How far a piece is in on a base built by plans: by the cubes it stands in, or its roof
/// cell's stage; lit where it stands in a cube in `next`. Doors and windows are left to
/// the props fitted into them.
pub fn stage_by_cubes(base: &Base, piece: &Piece, next: &BTreeSet<[i32; 3]>) -> Stage {
    if matches!(piece, Piece::Opening { .. }) {
        return Stage::Left;
    }
    // A piece of roof is up with its cell's stage; the stage after is next once nothing
    // of the walls is.
    if let Some(cell) = piece.roof_cell() {
        let stage = base
            .roof(cell.roof)
            .and_then(|roof| roof.stage(cell.cell))
            .unwrap_or(0);
        return if stage >= cell.stage {
            Stage::In
        } else if stage + 1 == cell.stage && next.is_empty() {
            Stage::Next
        } else {
            Stage::Ghost
        };
    }
    let (is_in, cubes) = piece_in(base, piece);
    if is_in {
        Stage::In
    } else if cubes.iter().any(|at| next.contains(at)) {
        Stage::Next
    } else {
        Stage::Ghost
    }
}

/// How far into a cube's height a piece must reach for it to count as standing in it.
const SLIVER_M: f32 = 0.05;

/// Whether a piece is in, by the cubes it stands in, and those cubes: a cube must be
/// filled up to the middle of the part of the piece within it, so a log is in once the
/// fill reaches its middle.
fn piece_in(base: &Base, piece: &Piece) -> (bool, Vec<[i32; 3]>) {
    let point_cube = |point: Vec3| (point / CELL_M).floor().as_ivec3();
    let Some((min, max)) = piece.bounds() else {
        return (true, Vec::new());
    };
    let low = ((min + SLIVER_M) / CELL_M).floor().as_ivec3();
    let high = ((max - SLIVER_M) / CELL_M).floor().as_ivec3();
    let mut cubes = Vec::new();
    let mut is_in = true;
    for x in low.x..=high.x {
        for y in low.y..=high.y {
            for z in low.z..=high.z {
                let at = IVec3::new(x, y, z);
                let bottom = y as f32 * CELL_M;
                let (from, to) = (min.y.max(bottom), max.y.min(bottom + CELL_M));
                let middle = ((from + to) * 0.5 - bottom) / CELL_M;
                let needed = (middle * 16.0).ceil().clamp(1.0, 16.0) as u8;
                // Only the cubes it stands in count: not air its rough edge reaches
                // into, nor an opening it frames, nor one being carved.
                let Some(cube) = base
                    .cube(at)
                    .filter(|cube| layout::drawn_fill(cube) != Fill::Open)
                else {
                    continue;
                };
                is_in &= cube.level() >= needed;
                cubes.push(at.to_array());
            }
        }
    }
    if cubes.is_empty() {
        // Flat, as a floor's boards on its top, or over a wall's top, as a gable is: in
        // with the first cube under it.
        let mut at = point_cube((min + max) * 0.5 - Vec3::Y * 0.01);
        for _ in 0..crate::STOREY_CUBES * 2 {
            if base.cube(at).is_some() {
                break;
            }
            at -= IVec3::Y;
        }
        let is_in = base.cube(at).is_none_or(|cube| cube.is_in());
        return (is_in, vec![at.to_array()]);
    }
    (is_in, cubes)
}

/// How near a section is drawn for: the finest detail it is given, and all coarser.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reach {
    Far,
    Mid,
    Near,
}

/// How finely a section is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    /// Every piece as it is made, textured: near.
    Near,
    /// Textured still, members with fewer sides and rings further apart, the stone's
    /// surface in fewer steps and less displaced, no battens: a few metres off.
    Mid,
    /// Coarser members, the stone's surface in fewer steps, roofs as a plain covering, no
    /// battens or rafters under it; its textures baked into its corners' colours.
    Far,
    /// Coarser again, only what shows from far off: walls, floors and roofs.
    Proxy,
}

impl Detail {
    /// A kit as it is drawn at this detail: fewer sides round a log, rings further apart.
    fn kit(self, kit: &Kit) -> Kit {
        let mut kit = kit.clone();
        match self {
            Detail::Near => {}
            Detail::Mid => {
                kit.timber.sides = kit.timber.sides.min(8);
                kit.timber.step = kit.timber.step.max(0.4);
            }
            // A log far off a few pixels across: six sides, a ring at each end.
            Detail::Far => {
                kit.timber.sides = kit.timber.sides.min(6);
                kit.timber.step = kit.timber.step.max(8.0);
            }
            Detail::Proxy => {
                kit.timber.sides = kit.timber.sides.min(4);
                kit.timber.step = kit.timber.step.max(8.0);
            }
        }
        kit
    }

    /// How finely a block kind's cube faces are divided.
    fn steps(self) -> u32 {
        match self {
            Detail::Near => SURFACE_STEPS,
            Detail::Mid => MID_STEPS,
            Detail::Far => 2,
            Detail::Proxy => 1,
        }
    }

    /// Whether a piece shows at this detail.
    fn shows(self, piece: &Piece) -> bool {
        match (self, piece) {
            (Detail::Near, _) => true,
            (
                Detail::Mid,
                Piece::Roof {
                    part: RoofPart::Batten,
                    ..
                },
            ) => false,
            (Detail::Mid, _) => true,
            (
                Detail::Far,
                Piece::Roof {
                    part: RoofPart::Batten | RoofPart::Rafter,
                    ..
                },
            ) => false,
            (Detail::Far, _) => true,
            (Detail::Proxy, piece) => matches!(
                piece,
                Piece::Log { .. }
                    | Piece::Block { .. }
                    | Piece::Gable { .. }
                    | Piece::Covering { .. }
                    | Piece::Plank { .. }
            ),
        }
    }
}

/// How finely a block kind's cube faces are divided near: a step every 5 cm or so; and a
/// few metres off, every 12.5 cm.
pub const SURFACE_STEPS: u32 = 10;
const MID_STEPS: u32 = 4;

/// How far a block kind's floors' flagstones stand out, as a fraction of its walls' stone:
/// flatter, as they are walked on.
const FLOOR_RELIEF: f32 = 0.3;

/// A block kind's look as its surface is drawn (D197): from its kit, its tops in its
/// `Floor` material where it has one, its faces divided in so many steps.
pub fn block_look(kit: &Kit, steps: u32) -> Look {
    let metres = |material: Material| kit.textures.get(&material).map(|t| t.metres);
    // Displaced as finely as its steps can show.
    let relief = if steps >= SURFACE_STEPS {
        kit.relief
    } else if steps >= MID_STEPS {
        kit.relief * 0.5
    } else {
        0.0
    };
    Look {
        bevel: kit.bevel,
        relief,
        steps,
        metres: metres(Material::Masonry).unwrap_or([2.0, 2.0]),
        top: metres(Material::Floor).map(|metres| (metres, relief * FLOOR_RELIEF)),
    }
}

/// What a ghost is drawn as: a box, or an eight-sided prism for a round log.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    Box,
    Round,
}

/// A piece not yet in, as the game draws it: a shape of unit size, turned and stretched to
/// it, in the base's own space; `shade` every other one, so they can be counted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ghost {
    pub shape: Shape,
    pub centre: Vec3,
    pub rotation: Quat,
    pub size: Vec3,
    pub shade: bool,
}

/// A section drawn: what is in, near, by kind and material (`Kind::PLAIN` for what no one
/// kit draws, in the first kit's materials); what is drawn plain by colour, and glass; the
/// next piece, lit; the ghosts, and the shell of a plan's blocks and plain cubes; far off
/// and further, one surface each; and its doors and windows, for the game to fit.
#[derive(Clone, Debug, Default)]
pub struct Drawn {
    pub near: BTreeMap<(Kind, Material), Surface>,
    /// What is in, a few metres off.
    pub mid: BTreeMap<(Kind, Material), Surface>,
    pub plain: Surface,
    pub glass: Surface,
    pub next: Surfaces,
    pub ghosts: Vec<Ghost>,
    pub shell: Surface,
    pub far: Surface,
    pub proxy: Surface,
    pub openings: Vec<(Piece, Stage)>,
}

/// Draws a laid section, as finely as `reach` asks and coarser: the game draws finer
/// detail only for the sections near the camera. A plan's ghosts, the next part and the
/// doors and windows come with the finest textured detail drawn.
pub fn draw(section: &Section, laid: &Laid, reach: Reach) -> Drawn {
    let mut drawn = Drawn::default();
    let kits = section.kits;
    let details: &[Detail] = match reach {
        Reach::Near => &[Detail::Near, Detail::Mid, Detail::Far, Detail::Proxy],
        Reach::Mid => &[Detail::Mid, Detail::Far, Detail::Proxy],
        Reach::Far => &[Detail::Far, Detail::Proxy],
    };
    let finest = details[0];
    for &detail in details {
        let first = detail == finest && detail != Detail::Far;
        let mut built: BTreeMap<(Kind, Material), Surface> = BTreeMap::new();
        let mut next = Surfaces::sharing();
        for (index, kit) in kits.0.iter().enumerate() {
            let own: Vec<&Placed> = laid.pieces.iter().filter(|p| p.0 == index).collect();
            let edge: Vec<&Placed> = laid.edge.iter().filter(|p| p.0 == index).collect();
            if own.is_empty() {
                continue;
            }
            let coarse = detail.kit(kit);
            let face = if detail == Detail::Near {
                section.maps.shape(kit)
            } else {
                None
            };
            let mut mine = Surfaces::sharing();
            for (count, (_, piece, stage)) in own.iter().enumerate() {
                if !detail.shows(piece) {
                    continue;
                }
                match stage {
                    Stage::In => crate::draw::draw_as(
                        piece,
                        &coarse,
                        face,
                        matches!(detail, Detail::Far | Detail::Proxy),
                        &mut mine,
                    ),
                    Stage::Next if first => {
                        crate::draw::draw_as(piece, &coarse, face, false, &mut next)
                    }
                    // Far off, what goes in next is drawn as though it were in.
                    Stage::Next => crate::draw::draw_as(piece, &coarse, None, true, &mut mine),
                    Stage::Ghost if first => {
                        drawn.ghosts.extend(ghost(piece, kit, count % 2 == 1));
                    }
                    Stage::Ghost | Stage::Left => {}
                }
                if first && matches!(piece, Piece::Opening { .. }) && *stage != Stage::Left {
                    drawn.openings.push((piece.clone(), *stage));
                }
            }
            if kit.pattern == Pattern::Masonry {
                let blocks = draw_blocks(section, kit, &own, &edge, detail, first);
                for (set, surfaces) in blocks.into_iter().enumerate() {
                    match set {
                        0 => join(&mut mine, surfaces),
                        1 if first => {
                            for (_, surface) in surfaces.0 {
                                drawn.shell.append(surface);
                            }
                        }
                        2 if first => join(&mut next, surfaces),
                        2 => join(&mut mine, surfaces),
                        _ => {}
                    }
                }
            }
            for (material, surface) in mine.0 {
                built
                    .entry((kit.kind(), material))
                    .or_default()
                    .append(surface);
            }
        }
        // Door steps, in the footings' stone.
        if !laid.steps.is_empty()
            && let Some(stone) = kits.0.iter().find(|kit| kit.pattern == Pattern::Masonry)
        {
            let look = block_look(stone, detail.steps());
            let (sides, top) = (section.maps.shape(stone), section.maps.top(stone));
            let sides = |uv: [f32; 2]| sides.map_or(0.5, |relief| relief.at(uv));
            let top = |uv: [f32; 2]| top.map_or(0.5, |relief| relief.at(uv));
            let heights = Heights {
                sides: &sides,
                top: &top,
            };
            let shared = Surface {
                sharing: true,
                ..Surface::default()
            };
            let (mut faces, mut tops) = (shared.clone(), shared);
            for &(min, max) in &laid.steps {
                crate::surface::slab(min, max, &look, &heights, [1.0; 3], &mut faces, &mut tops);
            }
            built
                .entry((stone.kind(), Material::Masonry))
                .or_default()
                .append(faces);
            built
                .entry((stone.kind(), Material::Floor))
                .or_default()
                .append(tops);
        }
        if first {
            drawn.next = next;
        }
        match detail {
            Detail::Near => drawn.near = built,
            Detail::Mid => drawn.mid = built,
            Detail::Far => drawn.far = bake(section, built),
            Detail::Proxy => drawn.proxy = bake(section, built),
        }
    }
    plain(section, laid, &mut drawn);
    drawn
}

/// Surfaces added to others.
fn join(into: &mut Surfaces, from: Surfaces) {
    for (material, surface) in from.0 {
        into.of(material).append(surface);
    }
}

/// A block kind's cubes in a section as its surface (D197): those in, those not yet, and
/// the next, each closed where it meets the others but for what is already built, with
/// the cubes a step beyond the section's edge worked out but not drawn, so its edges meet
/// the next section's as one surface.
fn draw_blocks(
    section: &Section,
    kit: &Kit,
    own: &[&Placed],
    edge: &[&Placed],
    detail: Detail,
    first: bool,
) -> [Surfaces; 3] {
    let base = section.base;
    let mut sets: [BTreeSet<[i32; 3]>; 3] = Default::default();
    // A gable's cubes, with the roof's underside over each, which cuts them.
    let mut gables: BTreeMap<[i32; 3], [[i32; 3]; 3]> = BTreeMap::new();
    for (_, piece, stage) in own.iter().chain(edge).copied() {
        let cube = match piece {
            Piece::Block { cube } => cube,
            Piece::Gable { cube, under } => {
                gables.insert(cube.to_array(), *under);
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
            (Stage::Left, _) => continue,
        };
        sets[set].insert(cube.to_array());
    }
    let cut = |cube: IVec3, at: Vec3| {
        gables
            .get(&cube.to_array())
            .map_or(f32::MAX, |under| layout::roofs::gable_cut(cube, under, at))
    };
    let look = block_look(kit, detail.steps());
    let (relief, top) = if matches!(detail, Detail::Near | Detail::Mid) {
        (section.maps.shape(kit), section.maps.top(kit))
    } else {
        (None, None)
    };
    let sides = |uv: [f32; 2]| relief.map_or(0.5, |relief| relief.at(uv));
    let top = |uv: [f32; 2]| top.map_or(0.5, |relief| relief.at(uv));
    let heights = Heights {
        sides: &sides,
        top: &top,
    };
    // Floors, and walls of kinds drawn as boxes, hide a face; walls of other kits do not,
    // being thinner than the cube.
    let hides_always = |at: IVec3| {
        base.cube(at).is_some_and(|cube| {
            cube.fill != Fill::Open
                && cube.kind != kit.kind()
                && (!base.in_wall(at) || cube.kind == Kind::PLAIN)
        })
    };
    let keep = |at: IVec3| crate::cubes::section_of(at) == section.at;
    // A plan's blocks are a plain shell, a step to a face.
    // Its texture a cube across, so a ghost's lines fall on the cubes' edges.
    let plan_look = Look {
        bevel: kit.bevel * 0.5,
        relief: 0.0,
        steps: 1,
        metres: [CELL_M, CELL_M],
        top: None,
    };
    let flat = |_: [f32; 2]| 0.5;
    let plan_heights = Heights {
        sides: &flat,
        top: &flat,
    };
    let mut out: [Surfaces; 3] = [
        Surfaces::sharing(),
        Surfaces::sharing(),
        Surfaces::sharing(),
    ];
    for set in 0..3 {
        if sets[set].is_empty() || (set == 1 && !first) {
            continue;
        }
        // What is built hides what is not; the rest show their own faces.
        let built = &sets[0];
        let hides = |at: IVec3| hides_always(at) || (set != 0 && built.contains(&at.to_array()));
        let (look, heights) = if set == 1 {
            (&plan_look, &plan_heights)
        } else {
            (&look, &heights)
        };
        let mut tops = Surface {
            sharing: true,
            ..Surface::default()
        };
        crate::surface::draw_part(
            &sets[set],
            &keep,
            &hides,
            look,
            heights,
            &cut,
            [1.0; 3],
            out[set].of(Material::Masonry),
            &mut tops,
        );
        out[set].of(Material::Floor).append(tops);
    }
    out
}

/// A plan's piece as a ghost, if it is drawn as one: a box or a prism stretched over it.
fn ghost(piece: &Piece, kit: &Kit, shade: bool) -> Option<Ghost> {
    // A box from one point to another along its middle, so wide and so high, its height
    // along `up` as far as that is square to it.
    let along = |from: Vec3, to: Vec3, up: Vec3, high: f32, wide: f32, shape: Shape| {
        let length = from.distance(to);
        if length < 1e-3 {
            return None;
        }
        let x = (to - from) / length;
        let y = (up - x * up.dot(x)).normalize_or(if x.y.abs() < 0.9 { Vec3::Y } else { Vec3::X });
        let z = x.cross(y);
        Some(Ghost {
            shape,
            centre: (from + to) * 0.5,
            rotation: Quat::from_mat3(&Mat3::from_cols(x, y, z)),
            size: Vec3::new(length, high, wide),
            shade,
        })
    };
    let boxed = |min: Vec3, max: Vec3| Ghost {
        shape: Shape::Box,
        centre: (min + max) * 0.5,
        rotation: Quat::IDENTITY,
        size: (max - min).max(Vec3::splat(1e-3)),
        shade,
    };
    match *piece {
        Piece::Log {
            from, to, split, ..
        } => {
            let (from, to) = (from.metres(), to.metres());
            let d = kit.diameter;
            let round = kit.timber.profile == Profile::Round;
            let (high, lift) = match split {
                layout::Split::Whole => (if round { d } else { kit.course * 0.8 }, 0.0),
                layout::Split::Top => (d * 0.5, d * 0.25),
                layout::Split::Bottom => (d * 0.5, -d * 0.25),
            };
            let shape = if round && split == layout::Split::Whole {
                Shape::Round
            } else {
                Shape::Box
            };
            let lift = Vec3::Y * lift;
            along(from + lift, to + lift, Vec3::Y, high, d * 0.92, shape)
        }
        Piece::Plank {
            from,
            to,
            wide,
            high,
            ..
        } => along(
            from.metres(),
            to.metres(),
            Vec3::Y,
            layout::metres(high),
            layout::metres(wide),
            Shape::Box,
        ),
        Piece::Board { min, max } | Piece::Dressed { min, max, .. } => {
            Some(boxed(min.metres(), max.metres()))
        }
        Piece::Covering {
            corners, middle, ..
        } => {
            let c = corners.map(layout::Place::metres);
            let x = ((c[1] - c[0]) + (c[2] - c[3])) * 0.5;
            let z = ((c[3] - c[0]) + (c[2] - c[1])) * 0.5;
            let y = z.cross(x).normalize_or(Vec3::Y);
            let thick = (kit.roof.thick + kit.roof.butt).max(0.03);
            let (width, depth) = (x.length(), (z - y * z.dot(y)).length());
            let mut tile = along(
                Vec3::ZERO,
                x.normalize_or(Vec3::X),
                y,
                thick,
                depth * 0.9,
                Shape::Box,
            )?;
            tile.centre = middle.metres() + y * (thick * 0.5);
            tile.size.x = width * 0.9;
            Some(tile)
        }
        Piece::Roof {
            part,
            from,
            to,
            sides,
            ..
        } => {
            let (from, to) = (from.metres(), to.metres());
            let up = layout::roofs::planar_normal(from, to, sides[0].metres());
            let r = &kit.roof;
            let (high, wide) = match part {
                RoofPart::Rafter => (r.rafter.1, r.rafter.1 * 0.7),
                RoofPart::Batten => (r.batten.1, r.batten.0),
                RoofPart::Fascia | RoofPart::Verge => (r.board.0, r.board.1),
                RoofPart::Ridge | RoofPart::Hip => (0.08, r.cap.max(0.1) * 2.0),
                RoofPart::Valley => (0.03, 0.3),
                RoofPart::Flashing => (r.fillet.max(0.04), r.fillet.max(0.04)),
            };
            along(from, to, up, high, wide, Shape::Box)
        }
        Piece::Opening { .. } | Piece::Block { .. } | Piece::Gable { .. } => None,
    }
}

/// Surfaces by kind and material as one, each corner's colour multiplied by its texture's
/// where it falls on it, as a far mesh's textures are baked (Unreal's HLOD).
fn bake(section: &Section, built: BTreeMap<(Kind, Material), Surface>) -> Surface {
    let encode = srgb_table();
    let to_srgb = |c: f32| encode[((c.clamp(0.0, 1.0) * (SRGB_STEPS - 1) as f32) + 0.5) as usize];
    let mut out = Surface::default();
    for ((kind, material), surface) in built {
        let kit = section.kits.of(kind).or(section.kits.0.first());
        let swatch = kit.and_then(|kit| section.maps.colour(kit, material));
        // Runs of corners share a colour, a log's or a face's: each turned once.
        let mut last: Option<([f32; 3], [f32; 3])> = None;
        let mut paint = |colour: [f32; 3], uv: [f32; 2]| -> [f32; 3] {
            let linear = match last {
                Some((from, to)) if from == colour => to,
                _ => {
                    let to = colour.map(to_linear);
                    last = Some((colour, to));
                    to
                }
            };
            let texel = swatch.map_or(fallback(material), |swatch| swatch.at(uv));
            [0, 1, 2].map(|i| to_srgb(linear[i] * texel[i]))
        };
        let mut surface = surface;
        let textured = surface.uvs.len() == surface.positions.len();
        for i in 0..surface.colours.len() {
            let uv = if textured { surface.uvs[i] } else { [0.0; 2] };
            surface.colours[i] = paint(surface.colours[i], uv);
        }
        let shared = &mut surface.shared;
        for (colour, uv) in shared.colours.iter_mut().zip(&shared.uvs) {
            *colour = paint(*colour, *uv);
        }
        out.append(surface);
    }
    out
}

/// How finely linear colour is turned back to sRGB when baked: finer than a byte.
const SRGB_STEPS: usize = 4096;

/// sRGB for each step of linear colour from 0 to 1.
fn srgb_table() -> &'static [f32] {
    static TABLE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        (0..SRGB_STEPS)
            .map(|i| {
                let c = i as f32 / (SRGB_STEPS - 1) as f32;
                if c <= 0.003_130_8 {
                    c * 12.92
                } else {
                    1.055 * libm::powf(c, 1.0 / 2.4) - 0.055
                }
            })
            .collect()
    })
}

/// An sRGB colour's value, linear.
fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        libm::powf((c + 0.055) / 1.055, 2.4)
    }
}

/// A material's colour, linear, before its texture has loaded.
fn fallback(material: Material) -> [f32; 3] {
    match material {
        Material::Masonry | Material::Stone | Material::Dressed | Material::Flashing => {
            [0.25, 0.24, 0.22]
        }
        Material::Covering => [0.16, 0.13, 0.11],
        Material::Plain => [0.6, 0.6, 0.6],
        _ => [0.22, 0.15, 0.1],
    }
}

/// What no kit draws, plain by colour, and glass: kept to the triangles in the section,
/// and the plan's planned plain cubes added to its ghost shell. Only worked out when the
/// section or a roof has something of a kind no kit draws, or glass.
fn plain(section: &Section, laid: &Laid, drawn: &mut Drawn) {
    let (base, kits) = (section.base, section.kits);
    let origin = section.at * crate::cubes::SECTION;
    let scope = layout::Scope {
        low: origin - IVec3::ONE,
        high: origin + IVec3::splat(crate::cubes::SECTION),
    };
    let kitless = |kind: Kind| kits.of(kind).is_none();
    let any = scope
        .cubes(base, None)
        .iter()
        .any(|(_, cube)| kitless(cube.kind) || cube.fill == Fill::Glass)
        || base
            .roofs()
            .any(|(_, roof)| base.roof_kind(roof).is_none_or(kitless));
    let _ = laid;
    if !any {
        return;
    }
    let keep = |surface: &Surface| {
        let mut out = Surface::default();
        for i in (0..surface.positions.len()).step_by(3) {
            let middle =
                (surface.positions[i] + surface.positions[i + 1] + surface.positions[i + 2]) / 3.0;
            if crate::cubes::section_of((middle / CELL_M).floor().as_ivec3()) != section.at {
                continue;
            }
            out.positions.extend(&surface.positions[i..i + 3]);
            out.normals.extend(&surface.normals[i..i + 3]);
            out.colours.extend(&surface.colours[i..i + 3]);
            if surface.uvs.len() == surface.positions.len() {
                out.uvs.extend(&surface.uvs[i..i + 3]);
            }
        }
        out
    };
    let (shown, planned) = match section.progress {
        Progress::Cubes { .. } => (
            base.view(
                |cube| cube.is_in() || cube.fill == Fill::Open,
                |roof| roof.standing(),
            ),
            Some(base.view(
                |cube| !cube.is_in() && cube.fill != Fill::Open,
                |roof| !roof.standing(),
            )),
        ),
        _ => (base.clone(), None),
    };
    let mesh = shown.mesh_for_kits(kits);
    drawn.plain = keep(&mesh.structure);
    drawn.glass = keep(&mesh.glass);
    for (material, surface) in &mesh.textured.0 {
        drawn
            .near
            .entry((Kind::PLAIN, *material))
            .or_default()
            .append(keep(surface));
    }
    if let Some(planned) = planned {
        let mesh = planned.mesh_for_kits(kits);
        drawn.shell.append(keep(&mesh.structure));
        for surface in mesh.textured.0.values() {
            drawn.shell.append(keep(surface));
        }
    }
}

#[cfg(test)]
mod tests;
