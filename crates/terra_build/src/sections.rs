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
use crate::layout::roofs::RoofPart;
use crate::layout::{self, Piece};
use crate::mesh::{Material, Surface, Surfaces};
use crate::relief::Maps;
use crate::surface::{Heights, Look, Packed};
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
}

/// How far a base's pieces are in.
#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
    /// All of it: a village's house.
    Whole,
    /// As far as each cube is filled, these cubes lit as what goes in next: the player's
    /// base.
    Cubes { next: BTreeSet<[i32; 3]> },
    /// So much of it, in the order its pieces go in: the kit sheet's going-up and plan
    /// columns.
    Fraction(f32),
}

/// A section to lay and draw: where it is, in sections, and what it is drawn from.
pub struct Section<'a> {
    pub base: &'a Base,
    pub kits: &'a Kits,
    pub maps: &'a Maps,
    pub at: IVec3,
    pub progress: &'a Progress,
}

/// A piece laid, the kit it is of, by its place in the kits, and how far in it is.
pub type Placed = (usize, Piece, Stage);

/// A section's pieces as laid: its own, and those a cube beyond its edge, which its block
/// kinds' surface needs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Laid {
    pub pieces: Vec<Placed>,
    pub edge: Vec<Placed>,
}

impl Laid {
    /// A number for what it holds, so a section whose pieces have not changed is not
    /// merged again.
    pub fn key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.pieces.hash(&mut hasher);
        self.edge.hash(&mut hasher);
        hasher.finish()
    }
}

/// Every section a base may draw something in: those with cubes, and those beside them that
/// their cubes come within a few of, which overhangs, eaves and steps reach into.
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

/// A base laid whole, every kit's pieces, filed by the section that owns each: what its
/// sections are cut from ([`lay`]). A change lays the base once, on a worker, rather than
/// once for every section it reaches, since what a section holds depends on its whole
/// building: a roof's lattice and creases, a row of logs' joins, a deck's boards. Laid
/// once, a roof is cut into its sections as a shipped game's building is chunked.
#[derive(Clone, Debug, Default)]
pub struct Whole {
    /// Each section's pieces: the kit each is of, by its place in the kits, where it comes
    /// in the order its kit's pieces go in, and the piece.
    by_section: BTreeMap<[i32; 3], Vec<(usize, usize, Piece)>>,
    /// How many pieces each kit laid.
    counts: Vec<usize>,
}

impl Whole {
    /// About how many bytes its pieces take in memory.
    pub fn bytes(&self) -> usize {
        let pieces: usize = self.by_section.values().map(Vec::len).sum();
        pieces * size_of::<(usize, usize, Piece)>() + self.by_section.len() * 64
    }

    /// Lays every kit's pieces of a base.
    pub fn lay(base: &Base, kits: &Kits) -> Whole {
        let mut whole = Whole::default();
        for (index, kit) in kits.0.iter().enumerate() {
            let pieces = layout::lay_among(base, kit, kits).pieces;
            whole.counts.push(pieces.len());
            for (order, piece) in pieces.into_iter().enumerate() {
                whole
                    .by_section
                    .entry(piece.section().to_array())
                    .or_default()
                    .push((index, order, piece));
            }
        }
        whole
    }

    /// This base laid again after edits that marked some sections (`Base::take_dirty`),
    /// from `before` as it was laid: only the buildings the marks are in, before the edits
    /// and after (an edit may join two or split one), are laid again, and every other
    /// building's sections kept as they were, as a claim may hold many. What a section is
    /// cut from it is what laying the base whole gives. Where its kits' pieces come in
    /// the order the whole base goes in is not kept, so a plan shown so far through
    /// (`Progress::Fraction`) is laid whole.
    pub fn lay_again(
        &self,
        before: &Base,
        base: &Base,
        kits: &Kits,
        marked: &BTreeSet<[i32; 3]>,
    ) -> Whole {
        let mut again = affected(before, marked);
        again.extend(affected(base, marked));
        let mut whole = Whole {
            by_section: self
                .by_section
                .iter()
                .filter(|(at, _)| !again.contains(*at))
                .map(|(at, pieces)| (*at, pieces.clone()))
                .collect(),
            counts: self.counts.clone(),
        };
        for (index, kit) in kits.0.iter().enumerate() {
            let pieces = layout::lay_sections(base, kit, kits, &again).pieces;
            for (order, piece) in pieces.into_iter().enumerate() {
                whole
                    .by_section
                    .entry(piece.section().to_array())
                    .or_default()
                    .push((index, order, piece));
            }
        }
        whole
    }
}

/// How far a section's cubes are searched for the kinds whose kits may lay pieces in it.
const KINDS_REACH: i32 = 4;

/// A section's pieces, cut from its base laid whole, with how far each is in: its own,
/// and a cube beyond its edges, as `layout::lay_section_and_edge` lays them.
pub fn lay(section: &Section, whole: &Whole) -> Laid {
    let (base, kits) = (section.base, section.kits);
    let origin = section.at * crate::cubes::SECTION;
    // Only the kits whose kinds are there, in its cubes or round them.
    let near = layout::Scope {
        low: origin - IVec3::splat(KINDS_REACH),
        high: origin + IVec3::splat(crate::cubes::SECTION - 1 + KINDS_REACH),
    };
    let kinds: BTreeSet<Kind> = near
        .cubes(base, None)
        .into_iter()
        .map(|(_, cube)| cube.kind)
        .collect();
    let present: Vec<bool> = kits
        .0
        .iter()
        .map(|kit| kinds.contains(&kit.kind()))
        .collect();
    let low = origin - IVec3::ONE;
    let high = origin + IVec3::splat(crate::cubes::SECTION);
    let stage = |index: usize, order: usize, piece: &Piece| match section.progress {
        // In the order the whole base goes in.
        Progress::Fraction(fraction) => {
            let done = done(whole.counts[index], *fraction);
            match order.cmp(&done) {
                std::cmp::Ordering::Less => Stage::In,
                std::cmp::Ordering::Equal => Stage::Next,
                std::cmp::Ordering::Greater => Stage::Ghost,
            }
        }
        Progress::Cubes { next } => stage_by_cubes(base, piece, next),
        Progress::Whole => Stage::In,
    };
    // Its own, then those of the sections round it a cube past its edges: a block kind's
    // surface needs them to round its edge as its neighbour does.
    let mut own: Vec<(usize, usize, Placed)> = Vec::new();
    let mut edge: Vec<(usize, usize, Placed)> = Vec::new();
    let fraction = matches!(section.progress, Progress::Fraction(_));
    for offset in NEIGHBOURS.iter().chain([&IVec3::ZERO]) {
        let at = section.at + *offset;
        let Some(pieces) = whole.by_section.get(&at.to_array()) else {
            continue;
        };
        for (index, order, piece) in pieces {
            if !present[*index] {
                continue;
            }
            let placed = |piece: &Piece| {
                (
                    *index,
                    *order,
                    (*index, piece.clone(), stage(*index, *order, piece)),
                )
            };
            if at == section.at {
                own.push(placed(piece));
            } else if (fraction
                // Blocks round the edge, and the lintels and sills bedded in them, whose
                // beds reach a little past their cubes (`dress::Bed`).
                || matches!(piece, Piece::Block { .. } | Piece::Gable { .. })
                || matches!(
                    piece,
                    Piece::Trim {
                        role: layout::Role::Head | layout::Role::Sill,
                        ..
                    }
                ))
                && piece.anchor().cmpge(low).all()
                && piece.anchor().cmple(high).all()
            {
                edge.push(placed(piece));
            }
        }
    }
    // Kit by kit, each in the order its pieces go in.
    own.sort_by_key(|(index, order, _)| (*index, *order));
    edge.sort_by_key(|(index, order, _)| (*index, *order));
    Laid {
        pieces: own.into_iter().map(|(_, _, placed)| placed).collect(),
        edge: edge.into_iter().map(|(_, _, placed)| placed).collect(),
    }
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
    )
}

/// How far a piece is in on a base built by plans: by the cubes it stands in, or its roof
/// cube's stage; lit where it stands in a cube in `next`, or is its roof cube's next stage.
pub fn stage_by_cubes(base: &Base, piece: &Piece, next: &BTreeSet<[i32; 3]>) -> Stage {
    if let Some(cell) = piece.roof_cell() {
        let stage = base
            .cube(cell.cube)
            .map_or(0, |cube| layout::roofs::stage_of(&cube));
        return if stage >= cell.stage {
            Stage::In
        } else if stage + 1 == cell.stage && next.contains(&cell.cube.to_array()) {
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
    // A framed kind's core and frame go in with a cube's first step, before its infill is
    // finished (D236): the wattle woven on the frame, then daubed.
    if let Piece::Wattle { cube } = *piece {
        let is_in = base
            .cube(cube)
            .is_some_and(|c| c.level() > 0 && c.fill == Fill::Solid);
        return (is_in, vec![cube.to_array()]);
    }
    let first_step = matches!(piece, Piece::Beam { .. });
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
                let needed = if first_step {
                    1
                } else {
                    (middle * 16.0).ceil().clamp(1.0, 16.0) as u8
                };
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

/// How near a section is drawn for: the finest detail it is given, and the next coarser,
/// which it fades into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reach {
    Far,
    Mid,
    Near,
}

/// How finely a section is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Detail {
    /// Every piece as it is made, textured: near.
    Near,
    /// Textured still, members with fewer sides and rings further apart, the stone's
    /// surface in fewer steps and less displaced, no battens: a few metres off.
    Mid,
    /// Coarser members, the stone's surface in fewer steps, roofs as a plain covering, no
    /// battens or rafters under it; its textures baked into its corners' colours.
    Far,
    /// Past the far detail: the hull of its cubes ([`crate::hull`]), not its pieces.
    Proxy,
}

impl Detail {
    /// A kit as it is drawn at this detail: fewer sides round a log, rings further apart.
    fn kit(self, kit: &Kit) -> Kit {
        let mut kit = kit.clone();
        match self {
            Detail::Near | Detail::Proxy => {}
            Detail::Mid => {
                kit.timber.sides = kit.timber.sides.min(8);
                kit.timber.step = kit.timber.step.max(0.4);
                kit.dress.far = true;
            }
            // A log far off a few pixels across: six sides, a ring at each end; a band of
            // dress its outline's three furthest corners, which keep its silhouette.
            Detail::Far => {
                kit.dress.far = true;
                kit.timber.sides = kit.timber.sides.min(6);
                kit.timber.step = kit.timber.step.max(8.0);
                if let Some(made) = &kit.dress.made {
                    let mut coarse = (**made).clone();
                    for profile in coarse.profiles.values_mut() {
                        *profile = outline(profile);
                    }
                    kit.dress.made = Some(std::sync::Arc::new(coarse));
                }
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
            // Far off a kit's dress keeps its bands, which shape the silhouette, and drops
            // its corner stones (D215).
            (Detail::Far, Piece::Corner { .. }) => false,
            (Detail::Far, _) => true,
            (Detail::Proxy, _) => false,
        }
    }
}

/// A profile cut to its ends and the point that stands furthest from the line between them.
fn outline(profile: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let (Some(&first), Some(&last)) = (profile.first(), profile.last()) else {
        return profile.to_vec();
    };
    let (a, b) = (glam::Vec2::from(first), glam::Vec2::from(last));
    let along = (b - a).normalize_or(glam::Vec2::X);
    let off = |p: &[f32; 2]| (glam::Vec2::from(*p) - a).perp_dot(along).abs();
    match profile.iter().max_by(|p, q| off(p).total_cmp(&off(q))) {
        Some(&far) if profile.len() > 3 && far != first && far != last => vec![first, far, last],
        _ => profile.to_vec(),
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
        // Its packing's lumps as far as its steps can show them, its lifts' ridges near.
        packed: kit
            .packed
            .filter(|_| steps >= MID_STEPS)
            .map(|packing| Packed {
                packing: crate::kit::Packing {
                    ridge: if steps >= SURFACE_STEPS {
                        packing.ridge
                    } else {
                        0.0
                    },
                    ..packing
                },
                lift: kit.dress.masks.lift,
            }),
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
/// kit draws, in the first kit's materials); what is drawn plain by colour; the
/// next piece, lit; the ghosts, and the shell of a plan's blocks and plain cubes; far off
/// and further, one surface each; and its doors and windows, for the game to fit.
#[derive(Clone, Debug, Default)]
pub struct Drawn {
    pub near: BTreeMap<(Kind, Material), Surface>,
    /// What is in, a few metres off.
    pub mid: BTreeMap<(Kind, Material), Surface>,
    pub plain: Surface,
    pub next: Surfaces,
    pub ghosts: Vec<Ghost>,
    pub shell: Surface,
    pub far: Surface,
    pub proxy: Surface,
}

/// Draws a laid section, as finely as `reach` asks and coarser: the game draws finer
/// detail only for the sections near the camera. A plan's ghosts, the next part and the
/// doors and windows come with the finest textured detail drawn.
pub fn draw(section: &Section, laid: &Laid, reach: Reach) -> Drawn {
    draw_again(section, laid, reach, None, None)
}

/// What a section's finer details were drawn as, piece by piece and face by face, kept so
/// drawing it again after an edit works out only what the edit reached (`draw_again`): a
/// piece drawn the same as before, and a block surface's faces whose cubes round them are
/// as they were, are taken as they were drawn, as a voxel game remeshes only round an edit.
#[derive(Clone, Debug, Default)]
pub struct Drawing {
    faces: crate::surface::FaceCache,
    pieces: crate::mesh::QuickMap<(usize, Detail, Piece), std::sync::Arc<Surfaces>>,
    /// The ground under the base when it was drawn, which the pieces' masks were made on.
    ground: Option<crate::ground::Ground>,
}

impl Drawing {
    /// How many faces and pieces it holds.
    pub fn len(&self) -> usize {
        self.faces.len() + self.pieces.len()
    }

    /// How many of its block surfaces' faces were taken from the last drawing.
    pub fn faces_taken(&self) -> usize {
        self.faces.taken()
    }

    /// How many block surfaces' faces it holds.
    pub fn faces(&self) -> usize {
        self.faces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// As [`draw`], taking what is unchanged from how the section was drawn last time, if
/// that is given, and keeping how it is drawn now in `keep`, if that is: the same surfaces
/// as drawing it afresh, to the bit.
pub fn draw_again(
    section: &Section,
    laid: &Laid,
    reach: Reach,
    last: Option<&Drawing>,
    keep: Option<&mut Drawing>,
) -> Drawn {
    // Only the details that can be seen while it is drawn for this reach: what it fades
    // into further off is made when the camera has gone far enough for it to be laid
    // again coarser, before that detail is seen. Past the far detail the game draws a
    // base's sections' proxies merged a cell at a time ([`proxy`]).
    let details: &[Detail] = match reach {
        Reach::Near => &[Detail::Near, Detail::Mid],
        Reach::Mid => &[Detail::Mid, Detail::Far],
        Reach::Far => &[Detail::Far],
    };
    let mut drawn = draw_details(section, laid, details, last, keep);
    if reach == Reach::Far {
        drawn.proxy = proxy(section);
    }
    drawn
}

/// A section far off: the hull of its cubes ([`crate::hull`]), a few hundred triangles of
/// its materials' colours, what its pieces fade into past the far detail, and what the game
/// merges with its neighbours' into one mesh a cell of its base further off still, as
/// Unreal's HLOD merges a cell's meshes into an approximated one.
pub fn proxy(section: &Section) -> Surface {
    let origin = section.at * crate::cubes::SECTION;
    crate::hull::hull(
        section.base,
        section.kits,
        section.maps,
        origin,
        origin + IVec3::splat(crate::cubes::SECTION - 1),
    )
}

/// Draws a laid section at these details, finest first, as [`draw_again`] says.
fn draw_details(
    section: &Section,
    laid: &Laid,
    details: &[Detail],
    last: Option<&Drawing>,
    keep: Option<&mut Drawing>,
) -> Drawn {
    let mut keep = keep;
    if let Some(keep) = keep.as_deref_mut() {
        keep.ground = Some(section.base.ground().clone());
    }
    let mut drawn = Drawn::default();
    let kits = section.kits;
    // The pieces drawn last time, kept masked, so to be taken only while the ground their
    // dirt rises from is as it was.
    let pieces_kept = last
        .filter(|last| last.ground.as_ref() == Some(section.base.ground()))
        .map(|last| &last.pieces);
    let finest = details[0];
    // Where each point of its roofs is on them, for their masks, worked out once.
    let roofs = std::cell::OnceCell::new();
    let origin = section.at * crate::cubes::SECTION;
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
            // A roof's covering keeps its relief a few metres off too, where its swell is
            // what shows of it; walls' and logs' only near.
            let face = if detail == Detail::Near
                || (detail == Detail::Mid && kit.pattern == Pattern::Roof)
            {
                section.maps.shape(kit)
            } else {
                None
            };
            // What is drawn in, masked (dirt from the ground, worn edges; D215) as it is
            // drawn, so a piece kept is kept masked; and what is drawn afresh, masked
            // together once all of it is.
            let mut mine = Surfaces::sharing();
            let mut fresh = Surfaces::sharing();
            let masked = |mut surfaces: Surfaces, stone: bool| {
                for (material, surface) in surfaces.0.iter_mut() {
                    if stone || *material != Material::Masonry {
                        crate::dress::mask(surface, kit, *material, section.base);
                    }
                }
                surfaces
            };
            // Where pieces are made finely each is drawn and masked on its own, to be kept
            // if asked: the same whether kept or not, so drawing again draws the same.
            let kept = matches!(detail, Detail::Near | Detail::Mid);
            for (count, (_, piece, stage)) in own.iter().enumerate() {
                if !detail.shows(piece) {
                    continue;
                }
                match stage {
                    Stage::In if far_box(piece, &coarse, detail, &mut fresh) => {}
                    Stage::In if kept => {
                        let key = (index, detail, piece.clone());
                        // Kept masked, so only while the ground its dirt rises from is as
                        // it was; but for stone, whose rain streaks hang from whatever is
                        // built above, which is masked afresh each time.
                        let made = match pieces_kept.and_then(|pieces| pieces.get(&key)) {
                            Some(made) => made.clone(),
                            None => {
                                let mut made = Surfaces::sharing();
                                crate::draw::draw_as(piece, &coarse, face, false, &mut made);
                                std::sync::Arc::new(masked(made, false))
                            }
                        };
                        for (&material, surface) in &made.0 {
                            let into = if material == Material::Masonry {
                                &mut fresh
                            } else {
                                &mut mine
                            };
                            into.of(material).extend_from(surface);
                        }
                        if let Some(keep) = keep.as_deref_mut() {
                            keep.pieces.insert(key, made);
                        }
                    }
                    Stage::In => crate::draw::draw_as(
                        piece,
                        &coarse,
                        face,
                        detail == Detail::Far,
                        &mut fresh,
                    ),
                    Stage::Next if first => {
                        crate::draw::draw_as(piece, &coarse, face, false, &mut next)
                    }
                    // Far off, what goes in next is drawn as though it were in.
                    Stage::Next => crate::draw::draw_as(piece, &coarse, None, true, &mut fresh),
                    Stage::Ghost if first => {
                        drawn.ghosts.extend(ghost(piece, kit, count % 2 == 1));
                    }
                    Stage::Ghost => {}
                }
            }
            if kit.pattern == Pattern::Masonry {
                let caching = keep.as_deref_mut().map(|keep| BlockCaching {
                    call: (index as u32) << 8 | detail as u32,
                    old: last.map(|last| &last.faces),
                    new: &mut keep.faces,
                });
                let blocks = draw_blocks(section, kit, &own, &edge, detail, first, caching);
                for (set, surfaces) in blocks.into_iter().enumerate() {
                    match set {
                        0 => join(&mut fresh, surfaces),
                        1 if first => {
                            for (_, surface) in surfaces.0 {
                                drawn.shell.append(surface);
                            }
                        }
                        2 if first => join(&mut next, surfaces),
                        2 => join(&mut fresh, surfaces),
                        _ => {}
                    }
                }
            }
            let fresh = masked(fresh, true);
            join(&mut mine, fresh);
            for (material, surface) in mine.0 {
                built
                    .entry((kit.kind(), material))
                    .or_default()
                    .append(surface);
            }
        }
        if first {
            drawn.next = next;
        }
        // A roof's own masks, by where on the roof each point is, over all of it at once.
        for ((kind, material), surface) in built.iter_mut() {
            if let Some(kit) = kits.of(*kind)
                && matches!(material, Material::Covering | Material::Flashing)
            {
                let roofs = roofs.get_or_init(|| {
                    layout::roofs::Roofs::whole(
                        section.base,
                        kits,
                        layout::Scope {
                            low: origin - IVec3::ONE,
                            high: origin + IVec3::splat(crate::cubes::SECTION),
                        },
                    )
                });
                crate::dress::roof_mask(surface, kit, roofs);
            }
        }
        match detail {
            Detail::Near => drawn.near = built,
            Detail::Mid => drawn.mid = built,
            Detail::Far => drawn.far = bake(section, built),
            Detail::Proxy => drawn.proxy = proxy(section),
        }
    }
    plain(section, laid, &mut drawn);
    drawn
}

/// Draws a long member far off as a plain box, if it is one that is (`draw::member_box`):
/// a deck's plank past the middle detail, and an undressed log at the proxy's distance,
/// each its own shade as near. Their rounded and chamfered sections and their rings are
/// most of a far building's triangles and cannot be seen there.
fn far_box(piece: &Piece, kit: &Kit, detail: Detail, surfaces: &mut Surfaces) -> bool {
    let shade = |seed: u32| {
        let mut random = layout::Random(seed);
        [1.0 - kit.timber.variation.shade * 0.8 * random.unit(); 3]
    };
    let metres = |material: Material| kit.textures.get(&material).map_or([2.0, 2.0], |t| t.metres);
    match (detail, piece) {
        (
            Detail::Far,
            Piece::Plank {
                from,
                to,
                wide,
                high,
                seed,
            },
        ) => {
            let half = [layout::metres(*wide) * 0.5, layout::metres(*high) * 0.5];
            crate::draw::member_box(
                surfaces.of(Material::Floor),
                from.metres(),
                to.metres(),
                half,
                metres(Material::Floor),
                shade(*seed),
            );
            true
        }
        (
            Detail::Far,
            Piece::Beam {
                from,
                to,
                wide,
                deep,
                seed,
                ..
            },
        ) => {
            // A box its depth across and its width up, a few pixels across, which way it
            // turns unseen.
            let half = [layout::metres(*deep) * 0.5, layout::metres(*wide) * 0.5];
            let material = kit.framing.as_ref().map_or(Material::Hewn, |f| f.material);
            crate::draw::member_box(
                surfaces.of(material),
                from.metres(),
                to.metres(),
                half,
                metres(material),
                shade(*seed),
            );
            true
        }
        _ => false,
    }
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
/// A kit's block surfaces drawn at a detail, under a number, as they were and as kept.
struct BlockCaching<'a> {
    call: u32,
    old: Option<&'a crate::surface::FaceCache>,
    new: &'a mut crate::surface::FaceCache,
}

/// One of a kit's block surfaces' caching, each under a number of its own.
fn cached<'a>(
    caching: &'a mut Option<BlockCaching>,
    surface: u32,
) -> Option<crate::surface::Caching<'a>> {
    caching.as_mut().map(|caching| crate::surface::Caching {
        call: caching.call << 4 | surface,
        old: caching.old,
        new: &mut *caching.new,
    })
}

fn draw_blocks(
    section: &Section,
    kit: &Kit,
    own: &[&Placed],
    edge: &[&Placed],
    detail: Detail,
    first: bool,
    mut caching: Option<BlockCaching>,
) -> [Surfaces; 3] {
    let base = section.base;
    let mut sets: [BTreeSet<[i32; 3]>; 3] = Default::default();
    // Cubes cut to a shape (D193), drawn apart from the whole ones: halves and slopes as
    // the surface cut down to their tops, stairs as a stone a tread, posts as drums.
    let mut shaped: [BTreeSet<[i32; 3]>; 3] = Default::default();
    // A gable's cubes, with the roof's underside over each, which cuts them.
    let mut gables: BTreeMap<[i32; 3], [[i32; 3]; 3]> = BTreeMap::new();
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
        };
        // A wall carried up into a roof's cube is the wall's whole cube cut at the roof,
        // whatever the roof's cube is cut to.
        let cut = matches!(piece, Piece::Block { .. })
            && base.cube(*cube).is_some_and(|c| c.shape.is_cut());
        if cut {
            shaped[set].insert(cube.to_array());
        } else {
            sets[set].insert(cube.to_array());
        }
    }
    // A core shows where its infill is not in, nor lit as next: neither over it.
    cores.retain(|at| !sets[0].contains(at) && !sets[2].contains(at));
    sets[1].retain(|at| !cores.contains(at));
    let shape_of = |cube: IVec3| {
        base.cube(cube)
            .filter(|c| c.kind == kit.kind())
            .map_or(crate::Shape::Whole, |c| c.shape)
    };
    let cut = |cube: IVec3, at: Vec3| {
        let roof = gables
            .get(&cube.to_array())
            .map_or(f32::MAX, |under| layout::roofs::gable_cut(cube, under, at));
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
        [low, roof.min(high)]
    };
    // Most cubes are whole: their cut is never asked.
    let cut_at_all = |cube: IVec3| {
        let shape = shape_of(cube);
        gables.contains_key(&cube.to_array()) || shape.planed_top() || shape.planed_under()
    };
    let cut = crate::surface::Cuts {
        of: &cut_at_all,
        at: &cut,
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
    let beds: Vec<crate::dress::Bed> = own
        .iter()
        .chain(edge)
        .filter_map(|(_, piece, _)| crate::dress::bed(piece, kit))
        .collect();
    let bed = |p: Vec3| {
        beds.iter()
            .filter_map(|b| b.inside(p).map(|inside| (inside, b.face)))
            .max_by(|a, b| a.0.total_cmp(&b.0))
    };
    let heights = Heights {
        sides: &sides,
        top: &top,
        bed: (!beds.is_empty()).then_some(&bed as &dyn Fn(Vec3) -> Option<(f32, f32)>),
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
    };
    let mut out: [Surfaces; 3] = [
        Surfaces::sharing(),
        Surfaces::sharing(),
        Surfaces::sharing(),
    ];
    for set in 0..3 {
        if (sets[set].is_empty() && shaped[set].is_empty()) || (set == 1 && !first) {
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
        // A plan's shell plain; what is built in the kit's tint.
        let colour = if set == 1 { [1.0; 3] } else { kit.tint };
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
            colour,
            out[set].of(Material::Masonry),
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
            .filter(|&at| planed(shape_of(IVec3::from_array(at))))
            .collect();
        let over_a_cut = |at: IVec3| {
            let (under, over) = (at - IVec3::Y, at + IVec3::Y);
            (cut_down.contains(&under.to_array()) && shape_of(under).planed_top())
                || (cut_down.contains(&over.to_array()) && shape_of(over).planed_under())
        };
        if !cut_down.is_empty() {
            let hides = |at: IVec3| hides(at) || (ours_whole(at) && !over_a_cut(at));
            crate::surface::draw_part(
                &cut_down,
                &keep,
                &hides,
                look,
                heights,
                &cut,
                colour,
                out[set].of(Material::Masonry),
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
        let mut faces = Surface {
            sharing: true,
            ..Surface::default()
        };
        for &at in &shaped[set] {
            let cube = IVec3::from_array(at);
            if !keep(cube) {
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
            for (min, max) in boxes {
                crate::surface::slab(min, max, look, heights, colour, &mut faces, &mut tops);
            }
        }
        out[set].of(Material::Masonry).append(faces);
        out[set].of(Material::Floor).append(tops);
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
        };
        let built = &sets[0];
        let hides = |at: IVec3| hides_always(at) || built.contains(&at.to_array());
        let mut tops = Surface {
            sharing: true,
            ..Surface::default()
        };
        crate::surface::draw_part(
            &cores,
            &keep,
            &hides,
            &core_look,
            &core_heights,
            &cut,
            [1.0; 3],
            out[0].of(material),
            &mut tops,
            cached(&mut caching, 3),
        );
        out[0].of(material).append(tops);
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
        Piece::Board { min, max } => Some(boxed(min.metres(), max.metres())),
        Piece::Beam {
            from,
            to,
            face,
            wide,
            deep,
            ..
        } => along(
            from.metres(),
            to.metres(),
            face.step().as_vec3(),
            layout::metres(deep),
            layout::metres(wide),
            Shape::Box,
        ),
        // A plan's trims as a band along their edge, standing out of its face.
        Piece::Trim {
            cube,
            role,
            side,
            other,
            ..
        } => {
            let (from, to) = layout::roles::line(cube, layout::roles::Edge { role, side, other });
            let at = |p: [i32; 3]| {
                glam::IVec3::from_array(p).as_vec3() * (CELL_M / layout::TICKS as f32)
            };
            let (from, to) = (at(from), at(to));
            let out = side.step().as_vec3();
            let up = if other == layout::Dir::Up { -0.1 } else { 0.25 };
            let (a, b) = (from - out * 0.05, to + out * 0.06 + Vec3::Y * up);
            Some(boxed(a.min(b), a.max(b)))
        }
        Piece::Corner { .. } => None,
        // A cut cube's band along its line, out of its face and up from it.
        Piece::Band { from, to, side, .. } => {
            let (from, to) = (from.metres(), to.metres());
            let out = side.step().as_vec3();
            let up = (to - from).normalize_or(Vec3::X).cross(out);
            let up = up * up.y.signum();
            along(
                from + out * 0.01,
                to + out * 0.01,
                up,
                0.1,
                0.12,
                Shape::Box,
            )
        }
        Piece::Covering {
            corners, middle, ..
        } => {
            let c = corners.map(layout::Place::metres);
            let x = ((c[1] - c[0]) + (c[2] - c[3])) * 0.5;
            let z = ((c[3] - c[0]) + (c[2] - c[1])) * 0.5;
            let y = z.cross(x).normalize_or(Vec3::Y);
            let thick = (kit.roof.thick + kit.roof.relief).max(0.03);
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
            // As big as the module laid for it, up and across.
            let module = kit.roof.parts.get(&part).and_then(|p| {
                kit.dress
                    .made
                    .as_deref()
                    .and_then(|pieces| pieces.modules.get(&p.module))
            });
            let [high, wide] = module.map_or([0.08, 0.2], |m| m.size().map(|s| s.max(0.02)));
            along(from, to, up, high, wide, Shape::Box)
        }
        Piece::Block { .. } | Piece::Gable { .. } | Piece::Wattle { .. } => None,
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

/// An sRGB colour's value, linear, from a table finer than a byte.
fn to_linear(c: f32) -> f32 {
    static TABLE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        (0..SRGB_STEPS)
            .map(|i| {
                let c = i as f32 / (SRGB_STEPS - 1) as f32;
                if c <= 0.04045 {
                    c / 12.92
                } else {
                    libm::powf((c + 0.055) / 1.055, 2.4)
                }
            })
            .collect()
    });
    table[((c.clamp(0.0, 1.0) * (SRGB_STEPS - 1) as f32) + 0.5) as usize]
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

/// What no kit draws, plain by colour: kept to the triangles in the section, and the
/// plan's planned plain cubes added to its ghost shell. Only worked out when the section
/// has something of a kind no kit draws.
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
        .any(|(_, cube)| kitless(cube.kind));
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
            base.view(|cube| cube.is_in() || cube.fill == Fill::Open),
            Some(base.view(|cube| !cube.is_in() && cube.fill != Fill::Open)),
        ),
        _ => (base.clone(), None),
    };
    let mesh = shown.mesh_for_kits(kits);
    drawn.plain = keep(&mesh.structure);
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
