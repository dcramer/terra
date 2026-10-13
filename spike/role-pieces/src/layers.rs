//! Candidate 2, role layers and masks: today's surface, each part of it drawn in a material
//! and a shade chosen by its role, with no new geometry. A stone wall's outside corners
//! show dressed quoins long and short by turns, its foot a dressed plinth course, its top a
//! dressed coping, an overhang's edge a dressed fascia, all in the surface itself and
//! standing a little proud of the rubble; its foot darkens with the ground's dirt, its
//! arrises wear lighter and rain streaks run down from its tops. Timber and roofs take the
//! masks only: dirt at the foot, weathering at the top, moss at the eaves.

use crate::masonry::{self, Here, Layer, Materials, Zones};
use crate::model::Model;
use crate::roles::{Edge, Neighbours, Run, runs};
use crate::shapes::Case;
use crate::today::{self, Laid};
use crate::{Drawn, Reliefs, add, add_surfaces, folder};
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use terra_build::kit::{Kit, Kits, Pattern};
use terra_build::layout::{self, Piece};
use terra_build::{CELL_M, Material, Surfaces};

/// How tall a course of dressed stone is: two to a cube.
pub const COURSE_M: f32 = 0.25;
/// How far quoins reach along each face, long and short by turns; a jamb's less.
const QUOIN_LONG_M: f32 = 0.4;
const QUOIN_SHORT_M: f32 = 0.2;
const JAMB_LONG_M: f32 = 0.3;
const JAMB_SHORT_M: f32 = 0.15;
/// How deep a coping's face is, and a fascia's, and how far either reaches over a top.
pub const COPING_M: f32 = 0.15;
const COPING_REACH_M: f32 = 0.25;
/// How long the dressed stones of a band are.
pub const BAND_STONE_M: f32 = 0.5;

/// What the dressed stone's colour is multiplied by: greyed towards the rubble's, one grade
/// a kit (docs/art-style.md).
pub const DRESSED_TINT: [f32; 3] = [0.84, 0.87, 0.93];

/// A small hash to a number from 0 to 1.
pub fn noise(parts: &[i32]) -> f32 {
    let mut x: u32 = 0x811c_9dc5;
    for &p in parts {
        x ^= p as u32;
        x = x.wrapping_mul(0x0100_0193);
        x ^= x >> 13;
        x = x.wrapping_mul(0x5bd1_e995);
        x ^= x >> 15;
    }
    (x & 0xffff) as f32 / 65535.0
}

/// Smooth value noise over a lattice of the given size, in metres.
pub fn smooth(p: Vec3, size: f32, seed: i32) -> f32 {
    let q = p / size;
    let base = q.floor();
    let f = q - base;
    let f = f * f * (Vec3::splat(3.0) - 2.0 * f);
    let b = base.as_ivec3();
    let at = |dx: i32, dy: i32, dz: i32| noise(&[b.x + dx, b.y + dy, b.z + dz, seed]);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = lerp(at(0, 0, 0), at(1, 0, 0), f.x);
    let x10 = lerp(at(0, 1, 0), at(1, 1, 0), f.x);
    let x01 = lerp(at(0, 0, 1), at(1, 0, 1), f.x);
    let x11 = lerp(at(0, 1, 1), at(1, 1, 1), f.x);
    lerp(lerp(x00, x10, f.y), lerp(x01, x11, f.y), f.z)
}

/// The runs of a kind's edge roles, found near any point.
pub struct Features {
    pub runs: Vec<Run>,
    near: HashMap<IVec3, Vec<usize>>,
}

impl Features {
    pub fn new(runs: Vec<Run>) -> Features {
        let mut near: HashMap<IVec3, Vec<usize>> = HashMap::new();
        for (i, run) in runs.iter().enumerate() {
            for cube in &run.cubes {
                for x in -1..=1 {
                    for y in -1..=1 {
                        for z in -1..=1 {
                            let list = near.entry(*cube + IVec3::new(x, y, z)).or_default();
                            if !list.contains(&i) {
                                list.push(i);
                            }
                        }
                    }
                }
            }
        }
        Features { runs, near }
    }

    pub fn around(&self, p: Vec3) -> impl Iterator<Item = &Run> {
        let cell = (p / CELL_M).floor().as_ivec3();
        self.near
            .get(&cell)
            .into_iter()
            .flatten()
            .map(|&i| &self.runs[i])
    }
}

/// Where a point is against a run: on which of its faces (0, the side; 1, the top or
/// bottom or the other side), how far in from the edge across that face, and how far along.
pub fn against(run: &Run, p: Vec3, normal: Vec3) -> Option<(usize, f32, f32)> {
    let k = (0..2).find(|&k| run.out[k].dot(normal) > 0.99)?;
    let rel = p - run.from;
    if rel.dot(run.out[k]).abs() > 1e-3 {
        return None;
    }
    let length = run.from.distance(run.to);
    let dir = (run.to - run.from) / length;
    let along = rel.dot(dir);
    if along < -1e-3 || along > length + 1e-3 {
        return None;
    }
    let inward = -rel.dot(run.out[1 - k]);
    Some((k, inward, along))
}

/// A stone's number from where it is.
pub fn stone_id(parts: &[i32]) -> u32 {
    (noise(parts) * 1.0e6) as u32 ^ (parts.iter().fold(7u32, |a, &p| a.wrapping_mul(31) ^ p as u32))
}

/// How the stone's zones are drawn: as dressed stone in the surface (candidate 2), or as
/// pieces of their own over the surface sunk back behind them (3), and how far the rubble
/// field itself is set back.
#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Layers,
    Pieces { field_inset: f32 },
}

pub struct StoneZones<'a> {
    pub features: &'a Features,
    pub ground: fn(f32, f32) -> f32,
    pub mode: Mode,
    /// Whether the openings' trims are the kit's own (D210), so holes are left alone.
    pub holes: bool,
}

/// Which band a run makes on a face, and how far from its edge.
fn band(role: Edge) -> Option<(f32, f32)> {
    match role {
        Edge::Crest => Some((COPING_M, COPING_REACH_M)),
        Edge::Sill => Some((0.1, COPING_REACH_M)),
        Edge::Soffit => Some((COPING_M, COPING_REACH_M)),
        Edge::Head => Some((COURSE_M, CELL_M)),
        Edge::Foot => Some((COURSE_M, 0.0)),
        _ => None,
    }
}

impl StoneZones<'_> {
    /// Whether a point is in dressed stone, and which stone and how near a joint.
    pub fn dressed(&self, p: Vec3, normal: Vec3) -> Option<(u32, f32)> {
        let joint = |x: f32, size: f32| {
            let m = x.rem_euclid(size);
            let d = m.min(size - m);
            if d < 0.02 { 1.0 } else { 0.0 }
        };
        let mut best: Option<(u32, f32)> = None;
        for run in self.features.around(p) {
            let hole = matches!(run.role, Edge::Jamb | Edge::Sill | Edge::Head);
            if hole && run.framed && !self.holes {
                continue;
            }
            match run.role {
                Edge::Arris | Edge::Jamb => {
                    let Some((k, inward, _)) = against(run, p, normal) else {
                        continue;
                    };
                    let course = (p.y / COURSE_M).floor() as i32;
                    let long = (course + k as i32).rem_euclid(2) == 0;
                    let width = match (run.role, long) {
                        (Edge::Arris, true) => QUOIN_LONG_M,
                        (Edge::Arris, false) => QUOIN_SHORT_M,
                        (_, true) => JAMB_LONG_M,
                        (_, false) => JAMB_SHORT_M,
                    };
                    if inward <= width + 1e-3 {
                        let e = run.from;
                        let id = stone_id(&[
                            (e.x * 8.0) as i32,
                            (e.z * 8.0) as i32,
                            course,
                            1,
                        ]);
                        best = Some((id, joint(p.y, COURSE_M)));
                    }
                }
                role => {
                    let Some((face, reach)) = band(role) else {
                        continue;
                    };
                    let Some((k, inward, along)) = against(run, p, normal) else {
                        continue;
                    };
                    let size = if k == 0 { face } else { reach };
                    if inward > size + 1e-3 {
                        continue;
                    }
                    // A head's lintel is one stone; the rest in lengths, their joints
                    // falling half a stone from the cubes' lines.
                    let (index, j) = if role == Edge::Head {
                        (0, 0.0)
                    } else {
                        let shifted = along + run.from.x + run.from.z + BAND_STONE_M * 0.5;
                        (
                            (shifted / BAND_STONE_M).floor() as i32,
                            joint(shifted, BAND_STONE_M),
                        )
                    };
                    let id = stone_id(&[
                        (run.from.x * 8.0) as i32,
                        (run.from.y * 8.0) as i32,
                        (run.from.z * 8.0) as i32,
                        index,
                        role as i32,
                    ]);
                    // A band wins over a quoin: copings and plinths run over the corners.
                    return Some((id, j));
                }
            }
        }
        best
    }
}

impl Zones for StoneZones<'_> {
    fn at(&self, p: Vec3, normal: Vec3) -> Here {
        let top = normal.y > 0.99;
        let plain = Here {
            layer: if top { Layer::Top } else { Layer::Field },
            inset: match self.mode {
                Mode::Layers => 0.0,
                Mode::Pieces { field_inset } => field_inset,
            },
            stone: 0,
            joint: 0.0,
        };
        match (self.dressed(p, normal), self.mode) {
            (None, _) => plain,
            (Some((stone, joint)), Mode::Layers) => Here {
                layer: Layer::Dressed,
                inset: 0.0,
                stone,
                joint,
            },
            // Under a piece: sunk well back, out of sight.
            (Some(_), Mode::Pieces { .. }) => Here {
                inset: 0.05,
                ..plain
            },
        }
    }

    fn tint(&self, p: Vec3, at: Vec3, normal: Vec3, outward: Vec3, round: f32) -> [f32; 3] {
        let m = masks(self.features, self.ground, p, outward, round);
        // Each dressed stone a little lighter or darker than the next, as cut from the bed.
        match (self.mode, self.dressed(at, normal)) {
            (Mode::Layers, Some((stone, _))) => {
                let shade = 0.9 + 0.16 * noise(&[stone as i32, 3]);
                let g = DRESSED_TINT;
                [m[0] * shade * g[0], m[1] * shade * g[1], m[2] * shade * g[2]]
            }
            _ => m,
        }
    }
}

/// The masks: dirt where a wall meets the ground, wear along outside edges, rain streaks
/// down from tops, as a shade per corner.
pub fn masks(
    features: &Features,
    ground: fn(f32, f32) -> f32,
    p: Vec3,
    outward: Vec3,
    round: f32,
) -> [f32; 3] {
    let mut c = Vec3::ONE;
    // Dirt and damp rising from the ground, unevenly.
    let h = p.y - ground(p.x, p.z);
    let rise = 0.35 + 0.25 * smooth(p, 0.4, 3);
    let dirt = libm::powf((1.0 - h / rise).clamp(0.0, 1.0), 1.5) * 0.6;
    c = c.lerp(Vec3::new(0.55, 0.5, 0.42), dirt);
    // Worn lighter along outside edges.
    c *= 1.0 + 0.1 * round;
    // Rain run down from a top: streaks under a crest or sill on the faces below it.
    if outward.y.abs() < 0.5 {
        for run in features.around(p) {
            if !matches!(run.role, Edge::Crest | Edge::Sill) {
                continue;
            }
            let side = run.out[0];
            if (p - run.from).dot(side).abs() > 0.08 || side.dot(outward) < 0.5 {
                continue;
            }
            let length = run.from.distance(run.to);
            let dir = (run.to - run.from) / length;
            let along = (p - run.from).dot(dir);
            if !(0.0..=length).contains(&along) {
                continue;
            }
            let below = run.from.y - p.y;
            let reach = 0.4 + 0.5 * noise(&[(along / 0.07) as i32, 9]);
            if (0.0..reach).contains(&below) {
                let streak = noise(&[(along / 0.035) as i32, 11]);
                let w = (1.0 - below / reach) * (0.12 + 0.18 * streak);
                c *= 1.0 - w;
            }
        }
    }
    c.to_array()
}

/// The stone kit's materials for the masonry mesher.
pub fn materials<'a>(kit: &Kit, reliefs: &'a Reliefs) -> Option<Materials<'a>> {
    let get = |material: Material| {
        let t = kit.textures.get(&material)?;
        let r = reliefs.of(kit, material)?;
        Some((r, t.metres))
    };
    let (field, fm) = get(Material::Masonry)?;
    let (top, tm) = get(Material::Floor)?;
    let (dressed, dm) = get(Material::Dressed)?;
    Some(Materials {
        field: (field, fm, kit.relief),
        top: (top, tm, kit.relief * today::FLOOR_RELIEF),
        dressed: (dressed, dm, 0.012),
        bevel: kit.bevel,
        steps: today::SURFACE_STEPS,
    })
}

/// A block kind's roles, runs and the cubes it draws.
pub struct Blocks {
    pub cubes: BTreeSet<[i32; 3]>,
    pub gables: BTreeMap<[i32; 3], [[i32; 3]; 3]>,
    pub features: Features,
}

pub fn blocks(case: &Case, kit: &Kit, pieces: &[Piece]) -> Blocks {
    let (cubes, gables) = today::blocks_of(pieces);
    let ground = case.ground;
    let n = Neighbours {
        base: &case.base,
        kind: kit.kind(),
        ground: &ground,
    };
    let mut roles = n.all();
    // A gable's cubes are cut by their roof, and what is under one is covered: no tops.
    roles.retain(|at, _| !gables.contains_key(at));
    let mut all = runs(&roles);
    for run in &mut all {
        run.framed = run.cubes.iter().any(|c| {
            run.out.iter().any(|o| {
                case.base
                    .cube(*c + o.as_ivec3())
                    .is_some_and(|cube| cube.fill != terra_build::Fill::Solid)
            })
        });
    }
    let all = all
        .into_iter()
        .filter(|run| {
            !(matches!(run.role, Edge::Crest)
                && run
                    .cubes
                    .iter()
                    .any(|c| gables.contains_key(&(*c + IVec3::Y).to_array())))
        })
        .collect();
    Blocks {
        cubes,
        gables,
        features: Features::new(all),
    }
}

/// A block kind's surface drawn by roles.
pub fn draw_blocks(
    case: &Case,
    kit: &Kit,
    blocks: &Blocks,
    reliefs: &Reliefs,
    mode: Mode,
    holes: bool,
    cut_more: &dyn Fn(IVec3, Vec3) -> f32,
    faces: &dyn Fn(IVec3, usize, f32) -> bool,
) -> Surfaces {
    let Some(m) = materials(kit, reliefs) else {
        return Surfaces::default();
    };
    let zones = StoneZones {
        features: &blocks.features,
        ground: case.ground,
        mode,
        holes,
    };
    let cut = |cube: IVec3, at: Vec3| {
        let gable = blocks
            .gables
            .get(&cube.to_array())
            .map_or(f32::MAX, |under| layout::roofs::gable_cut(cube, under, at));
        gable.min(cut_more(cube, at))
    };
    let hides = today::hides_for(&case.base, kit.kind());
    let mut out = masonry::Out::default();
    masonry::draw(&blocks.cubes, &hides, &m, &zones, &cut, faces, &mut out);
    let mut s = Surfaces::default();
    *s.of(Material::Masonry) = out.field;
    *s.of(Material::Floor) = out.top;
    *s.of(Material::Dressed) = out.dressed;
    s
}

/// Timber, roofs and anything else not a block kind's surface: the masks as shades.
pub fn shade_rest(case: &Case, kit: &Kit, material: Material, surfaces: &mut Surfaces) {
    let Some(s) = surfaces.0.get_mut(&material) else {
        return;
    };
    let low_roof = s.positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    for (p, c) in s.positions.iter().zip(s.colours.iter_mut()) {
        let mut k = Vec3::ONE;
        match material {
            Material::Log | Material::LogEnd | Material::Daub | Material::Board => {
                let h = p.y - (case.ground)(p.x, p.z);
                let dirt = libm::powf((1.0 - h / 0.9).clamp(0.0, 1.0), 1.4) * 0.45;
                k = k.lerp(Vec3::new(0.6, 0.55, 0.48), dirt);
                // Grey where the weather reaches the upper logs, unevenly.
                let grey = 0.12 + 0.18 * smooth(*p, 0.7, 5);
                k = k.lerp(Vec3::new(0.82, 0.82, 0.8), grey * (h / 3.0).clamp(0.0, 1.0));
            }
            Material::Covering => {
                // Moss along the eaves, where the water runs off slowest.
                let up = p.y - low_roof;
                let moss = (1.0 - up / 0.6).clamp(0.0, 1.0) * (0.3 + 0.4 * smooth(*p, 0.5, 7));
                k = k.lerp(Vec3::new(0.62, 0.72, 0.45), moss);
            }
            _ => {}
        }
        *c = [c[0] * k.x, c[1] * k.y, c[2] * k.z];
    }
    let _ = kit;
}

pub fn draw(case: &Case, kits: &Kits, reliefs: &Reliefs, door: &Model, window: &Model) -> Drawn {
    let laid: Vec<Laid> = today::lay(case, kits);
    let mut drawn = Drawn::default();
    today::plain(case, kits, &mut drawn);
    for Laid { kit, pieces } in &laid {
        let bark = reliefs.of(kit, Material::Log);
        let mut s = Surfaces::default();
        for piece in pieces {
            if let Piece::Opening { .. } = piece {
                if case.fitted {
                    today::openings(piece, door, window, &mut drawn);
                }
                continue;
            }
            terra_build::draw::draw(piece, kit, bark, &mut s);
        }
        for material in [
            Material::Log,
            Material::LogEnd,
            Material::Daub,
            Material::Board,
            Material::Covering,
        ] {
            shade_rest(case, kit, material, &mut s);
        }
        add_surfaces(&mut drawn, kit, s);
        if kit.pattern == Pattern::Masonry {
            let b = blocks(case, kit, pieces);
            let s = draw_blocks(
                case,
                kit,
                &b,
                reliefs,
                Mode::Layers,
                false,
                &|_, _| f32::MAX,
                &|_, _, _| true,
            );
            add_surfaces(&mut drawn, kit, s);
        }
    }
    let _ = (add, folder);
    drawn
}
