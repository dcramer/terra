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

mod reaching;
pub use reaching::*;
mod laying;
pub use laying::*;
mod parts;
pub use parts::*;
mod threads;
pub use threads::*;
mod details;
use details::*;
mod blocks;
use blocks::*;
mod ghosts;
use ghosts::*;
mod far;
pub use far::*;
mod plain;
use plain::*;

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
    /// Its damaged cubes and a cube round it, each with how many quarters of its health it
    /// has lost (`loss`), which its surfaces are worn by as they are drawn.
    pub worn: Vec<([i32; 3], u8)>,
    /// Its cubes a fire has blackened and a cube round it (D377, `Base::scorch`), which its
    /// textured surfaces are charred or sooted by as they are drawn.
    pub scorched: Vec<([i32; 3], crate::Scorch)>,
}

impl Laid {
    /// A number for what it holds, so a section whose pieces have not changed is not
    /// merged again.
    pub fn key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.pieces.hash(&mut hasher);
        self.edge.hash(&mut hasher);
        self.worn.hash(&mut hasher);
        self.scorched.hash(&mut hasher);
        hasher.finish()
    }
}

/// A piece as a laying files it: the kit it is of, by its place in the kits, where it
/// comes in the order its kit's pieces go in, and the piece.
type Filed = (usize, usize, Piece);

/// A base laid whole, every kit's pieces, filed by the section that owns each: what its
/// sections are cut from ([`lay`]). A change lays the base once, on a worker, rather than
/// once for every section it reaches, since what a section holds depends on its whole
/// building: a roof's lattice and creases, a row of logs' joins, a deck's boards. Laid
/// once, a roof is cut into its sections as a shipped game's building is chunked.
#[derive(Clone, Debug, Default)]
pub struct Whole {
    /// Each section's pieces: the kit each is of, by its place in the kits, where it comes
    /// in the order its kit's pieces go in, and the piece. Shared with the laying it was
    /// kept from where an edit did not reach it.
    by_section: BTreeMap<[i32; 3], std::sync::Arc<Vec<Filed>>>,
    /// How many pieces each kit laid.
    counts: Vec<usize>,
    /// The kinds of the kits it was laid by, in their order: a piece's kit by its place in
    /// them. Kits read again while the game runs (a material added, taken out or edited)
    /// may be others or in another order.
    laid_by: Vec<Kind>,
    /// What stands of a base with a plan in it, laid apart (D358): what is in is drawn from
    /// what is in, as finished, and a plan's ghosts from the plan. None where every cube
    /// is built, so the two are one.
    standing: Option<Standing>,
}

/// What stands of a base (`Base::what_stands`) and its pieces, filed as [`Whole`] files
/// them.
#[derive(Clone, Debug)]
struct Standing {
    base: std::sync::Arc<Base>,
    by_section: BTreeMap<[i32; 3], std::sync::Arc<Vec<Filed>>>,
    /// The sections laid again in it by the edit it was laid again after, if any.
    reached: BTreeSet<[i32; 3]>,
}

/// How near a section is drawn for: the finest detail it is given, and the next coarser,
/// which it fades into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reach {
    /// Past where its far detail is seen: its hull alone ([`proxy`]), which is all that is
    /// drawn of it there, so nothing finer is held.
    Hull,
    Far,
    Mid,
    Near,
    /// So near that its middle detail is not seen: its near detail alone, as a voxel game
    /// streams only the level of detail a chunk is seen at. The middle detail is made when
    /// the camera has gone far enough for it to be laid again `Near`, before it is seen.
    Close,
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
            // A few metres off: eight sides, a ring a metre, which still follow a log's bow
            // and taper; its knots, a centimetre high, go with their rings.
            Detail::Mid => {
                kit.timber.sides = kit.timber.sides.min(8);
                kit.timber.step = kit.timber.step.max(1.0);
                kit.timber.knots = 0.0;
                kit.dress.far = true;
                // A band of dress its profile within a centimetre, a third of a pixel there.
                if let Some(made) = &kit.dress.made {
                    let mut coarse = (**made).clone();
                    for profile in coarse.profiles.values_mut() {
                        *profile = simplified(profile, MID_PROFILE_M);
                    }
                    kit.dress.made = Some(std::sync::Arc::new(coarse));
                }
            }
            // A log far off a few pixels across: six sides, a ring at each end; a band of
            // dress its outline's three furthest corners, which keep its silhouette.
            Detail::Far => {
                kit.dress.far = true;
                kit.dress.far_off = true;
                kit.timber.sides = kit.timber.sides.min(6);
                kit.timber.step = kit.timber.step.max(8.0);
                kit.timber.knots = 0.0;
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

/// How far from its profile a band of dress may be drawn a few metres off.
const MID_PROFILE_M: f32 = 0.01;

/// A profile with the points dropped that stand no more than `within` from the line through
/// those kept round them (Ramer, Douglas and Peucker).
fn simplified(profile: &[[f32; 2]], within: f32) -> Vec<[f32; 2]> {
    if profile.len() < 3 {
        return profile.to_vec();
    }
    let mut keep = vec![false; profile.len()];
    keep[0] = true;
    keep[profile.len() - 1] = true;
    let mut spans = vec![(0, profile.len() - 1)];
    while let Some((a, b)) = spans.pop() {
        let (pa, pb) = (glam::Vec2::from(profile[a]), glam::Vec2::from(profile[b]));
        let along = (pb - pa).normalize_or(glam::Vec2::ZERO);
        let off = |p: glam::Vec2| {
            if along == glam::Vec2::ZERO {
                p.distance(pa)
            } else {
                (p - pa).perp_dot(along).abs()
            }
        };
        let far = (a + 1..b)
            .map(|i| (i, off(glam::Vec2::from(profile[i]))))
            .max_by(|x, y| x.1.total_cmp(&y.1));
        if let Some((i, by)) = far
            && by > within
        {
            keep[i] = true;
            spans.push((a, i));
            spans.push((i, b));
        }
    }
    profile
        .iter()
        .zip(&keep)
        .filter(|(_, k)| **k)
        .map(|(p, _)| *p)
        .collect()
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

/// The budget for a block kind's finest detail (D284), its surface and its dress together
/// (corner stones, plinths, copings, lintels): triangles a cube of it, and MiB of mesh a
/// building every section of which is drawn near at once, as a player beside it has it.
/// Its surface alone is held to `surface::NEAR_TRIANGLES_A_SQUARE_METRE`. Checked on a
/// stone house by the tests and on the village by `cargo xtask sections`.
pub const NEAR_TRIANGLES_A_CUBE: f32 = 120.0;

pub const NEAR_MIB_A_BASE: f32 = 4.0;

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
    /// The same a part of the section at a time ([`CLUSTER`]), as [`draw_parts`] leaves
    /// them for the game, `near` and `mid` empty.
    pub near_by: Vec<BTreeMap<(Kind, Material), Surface>>,
    pub mid_by: Vec<BTreeMap<(Kind, Material), Surface>>,
    pub plain: Surface,
    pub next: Surfaces,
    pub ghosts: Vec<Ghost>,
    pub shell: Surface,
    pub far: Surface,
    pub proxy: Surface,
    /// Its damaged cubes and a cube round it, with the quarters of health each has lost
    /// (`Laid::worn`): a textured surface near and a few metres off is worn by them as it
    /// is drawn (D290's shared damage mask, in the game), the rest in its colours here.
    pub worn: Vec<([i32; 3], u8)>,
    /// Its cubes a fire has blackened (`Laid::scorched`), drawn by the texture arrays.
    pub scorched: Vec<([i32; 3], crate::Scorch)>,
}

impl Drawn {
    /// Another section's drawing added to this one's, surface by surface, so a group of
    /// sections is drawn as one, as an HLOD cluster is: its parts each into the same part.
    pub fn append(&mut self, other: Drawn) {
        fn into(
            to: &mut BTreeMap<(Kind, Material), Surface>,
            from: BTreeMap<(Kind, Material), Surface>,
        ) {
            for (key, surface) in from {
                to.entry(key).or_default().append(surface);
            }
        }
        into(&mut self.near, other.near);
        into(&mut self.mid, other.mid);
        for (to, from) in [
            (&mut self.near_by, other.near_by),
            (&mut self.mid_by, other.mid_by),
        ] {
            for (part, surfaces) in from.into_iter().enumerate() {
                if to.len() <= part {
                    to.resize_with(part + 1, BTreeMap::new);
                }
                into(&mut to[part], surfaces);
            }
        }
        self.plain.append(other.plain);
        for (material, surface) in other.next.0 {
            self.next.of(material).append(surface);
        }
        self.ghosts.extend(other.ghosts);
        self.shell.append(other.shell);
        self.far.append(other.far);
        self.proxy.append(other.proxy);
    }
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
    /// Its pieces drawn, by detail and part (out of how many parts), each part's kept whole
    /// so a part not drawn again carries on as it was without looking at its pieces.
    pieces: crate::mesh::QuickMap<(Detail, usize, usize), std::sync::Arc<PiecesDrawn>>,
    /// The ground under the base when it was drawn, which the pieces' masks were made on.
    ground: Option<crate::ground::Ground>,
    /// The roofs round it when it was drawn ([`layout::roofs::Roofs::fingerprint`]), which a
    /// roof's pieces' masks were made on.
    roofs: Option<u64>,
}

/// Threads taken from [`SPARE_THREADS`], as many as are free up to those wanted, given back
/// when it goes.
pub struct Spare(usize);

/// [`draw_parts`]'s parts drawn in shares on threads of their own beside this one, as many
/// as are spare ([`Spare`]), and put together: each part's surfaces from the thread that
/// drew it, and the section's own (the plan's ghosts and shell, the next piece, what is
/// drawn plain) from the first, as every thread draws them the same; how each was drawn
/// kept together, a part's own from its thread.
fn draw_side_by_side(
    section: &Section,
    laid: &Laid,
    reach: Reach,
    last: Option<&Drawing>,
    keep: Option<&mut Drawing>,
    only: &[bool],
) -> Drawn {
    let kept = keep.is_some();
    let chosen: Vec<usize> = (0..only.len()).filter(|&c| only[c]).collect();
    let spare = Spare::take(chosen.len() - 1);
    let groups = shares(&chosen, spare.count() + 1);
    let draw = |group: &Vec<usize>| {
        let mut mine = vec![false; only.len()];
        for &c in group {
            mine[c] = true;
        }
        let mut drawing = kept.then(Drawing::default);
        let drawn = draw_together(section, laid, reach, last, drawing.as_mut(), Some(&mine));
        (group.clone(), drawn, drawing)
    };
    let mut each: Vec<(Vec<usize>, Drawn, Option<Drawing>)> = std::thread::scope(|scope| {
        let threads: Vec<_> = groups[1..]
            .iter()
            .map(|group| scope.spawn(move || draw(group)))
            .collect();
        let mut all = vec![draw(&groups[0])];
        all.extend(
            threads
                .into_iter()
                .map(|thread| thread.join().expect("a share of parts is drawn")),
        );
        all
    });
    drop(spare);
    let (first_group, mut drawn, first) = each.remove(0);
    let mut drawings: Vec<(Vec<usize>, Drawing)> = Vec::new();
    if let Some(first) = first {
        drawings.push((first_group, first));
    }
    for (group, mut part, drawing) in each {
        for &c in &group {
            for (to, from) in [
                (&mut drawn.near_by, &mut part.near_by),
                (&mut drawn.mid_by, &mut part.mid_by),
            ] {
                if let Some(surfaces) = from.get_mut(c) {
                    if to.len() <= c {
                        to.resize_with(c + 1, BTreeMap::new);
                    }
                    to[c] = std::mem::take(surfaces);
                }
            }
        }
        if let Some(drawing) = drawing {
            drawings.push((group, drawing));
        }
    }
    if let Some(keep) = keep {
        for (group, drawing) in drawings {
            keep.ground = drawing.ground;
            keep.roofs = drawing.roofs;
            keep.faces.take_from(drawing.faces);
            // Each part's pieces from the thread that drew it; the rest, kept on as they
            // were, from any.
            for (key, part) in drawing.pieces {
                if group.contains(&key.2) || !keep.pieces.contains_key(&key) {
                    keep.pieces.insert(key, part);
                }
            }
        }
    }
    drawn
}

/// [`draw_parts`] on this thread.
fn draw_together(
    section: &Section,
    laid: &Laid,
    reach: Reach,
    last: Option<&Drawing>,
    keep: Option<&mut Drawing>,
    only: Option<&[bool]>,
) -> Drawn {
    // Only the details that can be seen while it is drawn for this reach: what it fades
    // into further off is made when the camera has gone far enough for it to be laid
    // again coarser, before that detail is seen. Past the far detail the game draws a
    // base's sections' proxies merged a cell at a time ([`proxy`]).
    let details: &[Detail] = match reach {
        Reach::Close => &[Detail::Near],
        Reach::Near => &[Detail::Near, Detail::Mid],
        Reach::Mid => &[Detail::Mid, Detail::Far],
        Reach::Far => &[Detail::Far],
        Reach::Hull => &[],
    };
    let mut drawn = if details.is_empty() {
        Drawn::default()
    } else {
        draw_details(section, laid, details, last, keep, only)
    };
    if reach <= Reach::Far {
        drawn.proxy = proxy(section);
    }
    drawn
}

/// A kit's block surfaces drawn at a detail, under a number, as they were and as kept.
struct BlockCaching<'a> {
    call: u32,
    old: Option<&'a crate::surface::FaceCache>,
    new: &'a mut crate::surface::FaceCache,
}

#[cfg(test)]
mod tests;
