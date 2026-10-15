//! What a renderer would show as junk in what a base's sections draw, found from the meshes
//! themselves, with no GPU (D69): the faults that come out of the data rather than the
//! renderer, checked at every level of detail, so `cargo xtask artifacts` and the tests can
//! hold them to none. A frame shows each only from some views and some distances; the
//! meshes show every one of them everywhere.
//!
//! - **Not finite**: a corner, normal or texture place that is NaN or infinite, or a normal
//!   of no length. A shader's `normalize` of it is NaN, which lights a pixel black, and TAA
//!   then spreads it frame by frame.
//! - **Coplanar**: two triangles facing the same way in one plane, overlapping, so the depth
//!   test picks between them by rounding and the picture flips between them as the camera
//!   moves: z-fighting. Where trims, kinds, floors and footings meet is where it comes from.
//! - **Doubled**: the same triangle drawn twice, as when two sections both draw a piece.
//! - **Under the roof**: a gap between a wall's top and the roof's underside, where the wall
//!   carried up into a roof cube does not reach the roof: a gable stepped with sky under it.
//! - **Pokes out**: a piece past the plane it belongs within: a cap floating over a ridge, a
//!   batten's stub through the covering or past a verge, a gable's step through the roof, a
//!   lintel standing further out of a wall than a trim may (an eighth of a cube, D192).
//! - **See-through**: a line through a solid wall, from one side to the other, that meets no
//!   triangle: a hole, or light let through a joint where the surface mesher and the role
//!   pieces, or two kinds, meet. Cast square to each face of every solid built cube open to
//!   the air, and slanting across it, and down through every roof cube open to the sky.
//!
//! Engines check the same before shipping (Unreal's map check, mesh validation in every
//! content pipeline); here it runs on the village and on every kit meeting every other.

use crate::kit::{Kits, Pattern};
use crate::mesh::{Material, Surface};
use crate::relief::Maps;
use crate::sections::{self, Detail, Progress, Reach, Section};
use crate::{Base, CELL_M, Fill, Kind, Shape};
use glam::{IVec3, Vec2, Vec3};
use std::collections::BTreeMap;

/// A fault, by its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Fault {
    NotFinite,
    /// A mesh whose corners' colours, which multiply its texture, are mostly near black, so
    /// it is drawn black in sun and shade alike.
    Black,
    /// Triangles wound against their own normals, so the GPU culls their outside and draws
    /// their inside, lit from behind: a wall drawn black, or seen through.
    Inward,
    Coplanar,
    Doubled,
    SeeThrough,
    /// A gap between a wall's top and the roof's underside over it: a line in under the
    /// roof, through where the wall is carried up into it, that meets nothing.
    UnderRoof,
    /// A piece out past the outer plane it belongs within: up through a roof's covering or
    /// floating over it, out past a roof's edges, or further out of its cubes than the
    /// eighth of a cube a trim may stand out (D192).
    PokesOut,
    /// A piece sunk under a block kind's surface: the surface's relief stands out over the
    /// piece's face, so it shows in dashes where the relief dips, or not at all, as wattle's
    /// frame did under its daub.
    Buried,
}

/// One fault found: what, at which detail, where in the base's space, and between what.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub fault: Fault,
    pub detail: Detail,
    pub at: Vec3,
    pub what: String,
    /// How far out, in metres, for a piece poking out; nothing for the rest.
    pub by: f32,
}

/// What was looked at and what was found.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub findings: Vec<Finding>,
    /// Triangles looked at, at each detail: near, mid, far, proxy.
    pub triangles: [usize; 4],
    /// Lines cast through walls and roofs.
    pub rays: usize,
}

impl Report {
    /// How many of a fault were found.
    pub fn count(&self, fault: Fault) -> usize {
        self.findings.iter().filter(|f| f.fault == fault).count()
    }
}

/// How far apart two planes may be and still fight for the same pixels, in metres: a depth
/// buffer of 32 bits, reversed, keeps far finer than this at any distance a building is
/// seen from, so this is two surfaces meant to be one.
const COPLANAR_M: f32 = 0.001;
/// How dark a corner's colour (sRGB) is to count as black: nothing a kit tints or masks is
/// meant this dark, the darkest dirt and soot a little over a tenth.
const DARK: f32 = 0.06;
/// How nearly two triangles must face the same way to fight: within about two degrees.
const SAME_FACING: f32 = 0.9994;
/// How much two triangles must overlap to be seen fighting, in square metres: a square
/// centimetre.
const OVERLAP_M2: f32 = 1e-4;
/// How far outside a cube a line starts, past every trim (an eighth of a cube) and eave.
const RAY_START_M: f32 = 0.3;
/// How short of a wall's far side a line may come out and still count as through it: its
/// rounded edges are not holes.
const THROUGH_M: f32 = 0.3;
/// How far a line may run inside solid cubes and still be through a wall rather than along
/// one: a wall three cubes thick, slanting across it.
const ALONG_M: f32 = 3.0 * CELL_M * 1.2;
/// How far a line may run inside solid cubes and come out of a side square to the face it
/// went in by, as through a corner: further, it ran along a wall and out of its side.
const CORNER_M: f32 = 2.0 * CELL_M * 1.2;

/// A triangle drawn, in the base's space, in a mesh by its number.
#[derive(Clone, Copy, Debug)]
struct Tri {
    p: [Vec3; 3],
    n: Vec3,
    d: f32,
    mesh: u32,
}

/// Every triangle drawn at one detail, and each mesh's name.
#[derive(Default)]
struct Drawn {
    tris: Vec<Tri>,
    meshes: Vec<String>,
    /// Each mesh's kind and material, where it has one kind.
    kinds: Vec<Option<(Kind, Material)>>,
}

impl Drawn {
    /// Adds a surface's triangles as one mesh, and its faults of numbers.
    fn add(
        &mut self,
        name: String,
        kind: Option<(Kind, Material)>,
        surface: &Surface,
        findings: &mut Vec<Finding>,
        detail: Detail,
    ) {
        let mesh = self.meshes.len() as u32;
        self.meshes.push(name);
        self.kinds.push(kind);
        let all = surface.indexed();
        let mut bad = 0usize;
        let mut first_bad = None;
        for (i, (p, n)) in all.positions.iter().zip(&all.normals).enumerate() {
            let uv = all.uvs.get(i).copied().unwrap_or([0.0; 2]);
            if !p.is_finite()
                || !n.is_finite()
                || n.length() < 0.5
                || !uv[0].is_finite()
                || !uv[1].is_finite()
            {
                bad += 1;
                first_bad.get_or_insert(*p);
            }
        }
        // Corners whose colour, which multiplies the texture, is near black: drawn black
        // whatever the light, as a mask or a bake gone wrong does.
        if let Some(dark) = black(surface) {
            findings.push(Finding {
                by: 0.0,
                fault: Fault::Black,
                detail,
                at: all.positions.first().copied().unwrap_or(Vec3::ZERO),
                what: format!(
                    "{}: {dark} of {} corners near black",
                    self.meshes[mesh as usize],
                    all.colours.len()
                ),
            });
        }
        if bad > 0 {
            findings.push(Finding {
                by: 0.0,
                fault: Fault::NotFinite,
                detail,
                at: first_bad.unwrap_or(Vec3::ZERO),
                what: format!(
                    "{}: {bad} corners not finite or with no normal",
                    self.meshes[mesh as usize]
                ),
            });
        }
        let (mut inward, mut area_in, mut area_all) = (0usize, 0.0f32, 0.0f32);
        let mut first_in = None;
        for t in all.indices.chunks_exact(3) {
            let p = [0, 1, 2].map(|i| all.positions[t[i] as usize]);
            let cross = (p[1] - p[0]).cross(p[2] - p[0]);
            let area2 = cross.length();
            if !area2.is_finite() || area2 < 1e-9 {
                continue;
            }
            let n = cross / area2;
            // Wound against its corners' normals: culled from outside and drawn from
            // inside, lit from behind.
            let normal: Vec3 = t.iter().map(|&i| all.normals[i as usize]).sum();
            area_all += area2;
            if normal.dot(n) < -0.2 * normal.length() {
                inward += 1;
                area_in += area2;
                first_in.get_or_insert(p[0]);
            }
            self.tris.push(Tri {
                p,
                n,
                d: n.dot(p[0]),
                mesh,
            });
        }
        // A sliver here and there wound back where a surface folds is not seen; a share of
        // its area is.
        if area_in > area_all * 0.01 {
            findings.push(Finding {
                by: 0.0,
                fault: Fault::Inward,
                detail,
                at: first_in.unwrap_or(Vec3::ZERO),
                what: format!(
                    "{}: {inward} triangles, {:.0}% of its area, wound against their normals",
                    self.meshes[mesh as usize],
                    area_in / area_all * 100.0
                ),
            });
        }
    }
}

/// How many of a surface's corners are near black, if over a quarter are ([`Fault::Black`]):
/// the game asks it of every surface it is given, with the real maps the masks read.
pub fn black(surface: &Surface) -> Option<usize> {
    let colours = surface.colours.iter().chain(&surface.shared.colours);
    let (mut dark, mut all) = (0, 0);
    for c in colours {
        all += 1;
        if !c.iter().all(|v| v.is_finite()) || c[0].max(c[1]).max(c[2]) < DARK {
            dark += 1;
        }
    }
    (all > 0 && dark * 4 > all).then_some(dark)
}

/// Inspects a base built whole, at every detail its sections are drawn at.
pub fn inspect(base: &Base, kits: &Kits, maps: &Maps) -> Report {
    let mut report = Report::default();
    let whole = sections::Whole::lay(base, kits);
    let progress = Progress::Whole;
    let mut drawn: [Drawn; 4] = Default::default();
    for at in sections::drawable(base) {
        let section = Section {
            base,
            kits,
            maps,
            at: IVec3::from_array(at),
            progress: &progress,
        };
        let laid = sections::lay(&section, &whole);
        let near = sections::draw(&section, &laid, Reach::Near);
        let far = sections::draw(&section, &laid, Reach::Far);
        let name = |what: &str| format!("section {at:?} {what}");
        let findings = &mut report.findings;
        for ((kind, material), surface) in &near.near {
            let label = name(&label(kits, *kind, *material));
            drawn[0].add(
                label,
                Some((*kind, *material)),
                surface,
                findings,
                Detail::Near,
            );
        }
        drawn[0].add(name("plain"), None, &near.plain, findings, Detail::Near);
        for ((kind, material), surface) in &near.mid {
            let label = name(&label(kits, *kind, *material));
            drawn[1].add(
                label,
                Some((*kind, *material)),
                surface,
                findings,
                Detail::Mid,
            );
        }
        drawn[2].add(name("far"), None, &far.far, findings, Detail::Far);
        drawn[3].add(name("proxy"), None, &far.proxy, findings, Detail::Proxy);
    }
    let details = [Detail::Near, Detail::Mid, Detail::Far, Detail::Proxy];
    let roofs = crate::layout::roofs::Roofs::whole(base, kits, crate::layout::Scope::ALL);
    for (slot, drawn) in drawn.iter().enumerate() {
        if slot < 2 {
            buried(kits, drawn, details[slot], &mut report.findings);
            pokes_out(
                base,
                kits,
                &roofs,
                drawn,
                details[slot],
                &mut report.findings,
            );
        }
        report.rays += under_roof(
            base,
            kits,
            &roofs,
            drawn,
            details[slot],
            &mut report.findings,
        );
        report.triangles[slot] = drawn.tris.len();
        coplanar(drawn, details[slot], &mut report.findings);
        // Far off a wall's lines need only be square to it: slanting ones find the coarser
        // surface's corners rounded off, which the distance hides.
        let slanting = slot < 2;
        report.rays += see_through(
            base,
            kits,
            drawn,
            details[slot],
            slanting,
            &mut report.findings,
        );
    }
    report
}

/// A mesh's name: its kind's and its material.
fn label(kits: &Kits, kind: Kind, material: Material) -> String {
    let pattern = kits.of(kind).map_or("plain".to_string(), |kit| {
        format!("{:?} {}", kit.pattern, kit.id)
    });
    format!("{pattern} {material:?}")
}

/// How much a surface's triangles overlap one another facing the same way in one plane, in
/// square metres: z-fighting within one mesh, as [`Fault::Coplanar`] finds it between meshes.
pub fn self_overlap(surface: &Surface) -> f32 {
    let mut drawn = Drawn::default();
    drawn.add(String::new(), None, surface, &mut Vec::new(), Detail::Near);
    overlaps(&drawn).values().map(|(_, area, _)| area).sum()
}

/// Pairs of triangles in one plane, facing the same way, that overlap.
fn coplanar(drawn: &Drawn, detail: Detail, findings: &mut Vec<Finding>) {
    for ((m, n, same), (at, area, count)) in overlaps(drawn) {
        let (a, b) = (&drawn.meshes[m as usize], &drawn.meshes[n as usize]);
        findings.push(Finding {
            by: 0.0,
            fault: if same {
                Fault::Doubled
            } else {
                Fault::Coplanar
            },
            detail,
            at,
            what: if m == n {
                format!(
                    "{a}: {count} pairs of its own triangles, {:.0} cm²",
                    area * 1e4
                )
            } else {
                format!("{a} and {b}: {count} pairs, {:.0} cm²", area * 1e4)
            },
        });
    }
}

/// The overlaps of triangles in one plane facing the same way, for each pair of meshes and
/// whether they are the same triangle twice: where they first fight, how much, how many.
fn overlaps(drawn: &Drawn) -> BTreeMap<(u32, u32, bool), (Vec3, f32, usize)> {
    // Filed by their plane, coarsely: the normal to a twentieth, the distance to a
    // centimetre; each is looked for in the bins round its own.
    let key = |t: &Tri| {
        let n = (t.n * 20.0).round().as_ivec3();
        [n.x, n.y, n.z, (t.d * 100.0).round() as i32]
    };
    let mut bins: std::collections::HashMap<[i32; 4], Vec<u32>> = std::collections::HashMap::new();
    for (i, t) in drawn.tris.iter().enumerate() {
        bins.entry(key(t)).or_default().push(i as u32);
    }
    // The stone's triangles, a block kind's surface and its dress, filed by cube, made when
    // first asked.
    let stone = std::cell::OnceCell::new();
    let stone = || {
        stone.get_or_init(|| {
            let is_stone = |mesh: u32| {
                drawn.kinds[mesh as usize].is_some_and(|(_, material)| {
                    matches!(
                        material,
                        Material::Masonry | Material::Floor | Material::Dressed
                    )
                })
            };
            Grid::of(drawn, |t| is_stone(t.mesh))
        })
    };
    // One finding for each pair of meshes, where they first fight, and how much.
    let mut pairs: BTreeMap<(u32, u32, bool), (Vec3, f32, usize)> = BTreeMap::new();
    let mut near: Vec<u32> = Vec::new();
    for (i, a) in drawn.tris.iter().enumerate() {
        let k = key(a);
        near.clear();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for dd in -1..=1 {
                        if let Some(bin) = bins.get(&[k[0] + dx, k[1] + dy, k[2] + dz, k[3] + dd]) {
                            near.extend(bin.iter().copied().filter(|&j| j as usize > i));
                        }
                    }
                }
            }
        }
        for &j in &near {
            let b = &drawn.tris[j as usize];
            if a.n.dot(b.n) < SAME_FACING {
                continue;
            }
            if b.p.iter().any(|p| (a.n.dot(*p) - a.d).abs() > COPLANAR_M)
                || a.p.iter().any(|p| (b.n.dot(*p) - b.d).abs() > COPLANAR_M)
            {
                continue;
            }
            let (area, middle) = clipped(a, b);
            if area < OVERLAP_M2 {
                continue;
            }
            let same =
                a.p.iter()
                    .all(|p| b.p.iter().any(|q| p.distance(*q) < 1e-4));
            // Pressed against a face turned to meet them in their plane, as a sill's
            // underside and a split log's flat foot both on a footing's top: never seen,
            // as a viewer would have to be inside what that face closes.
            if !same && pressed(drawn, &bins, Some(stone()), a, middle) {
                continue;
            }
            let entry = pairs
                .entry((a.mesh.min(b.mesh), a.mesh.max(b.mesh), same))
                .or_insert(((a.p[0] + a.p[1] + a.p[2]) / 3.0, 0.0, 0));
            entry.1 += area;
            entry.2 += 1;
        }
    }
    pairs
}

/// How far inside what a face turned to meet it closes a point may be and still be pressed
/// against it: the few millimetres a piece is let into the stone it stands on.
const PRESSED_M: f32 = 0.01;

/// Whether a point of a triangle's plane is on a triangle facing it in that plane, or a
/// little behind one, inside what that face closes.
fn pressed(
    drawn: &Drawn,
    bins: &std::collections::HashMap<[i32; 4], Vec<u32>>,
    stone: Option<&Grid>,
    a: &Tri,
    at: Vec3,
) -> bool {
    let n = (-a.n * 20.0).round().as_ivec3();
    let d = (-a.d * 100.0).round() as i32;
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                for dd in -1..=1 {
                    let Some(bin) = bins.get(&[n.x + dx, n.y + dy, n.z + dz, d + dd]) else {
                        continue;
                    };
                    for &j in bin {
                        let c = &drawn.tris[j as usize];
                        if c.n.dot(-a.n) >= SAME_FACING
                            && (c.n.dot(at) - c.d).abs() <= COPLANAR_M
                            && contains(c, at)
                        {
                            return true;
                        }
                    }
                }
            }
        }
    }
    // Inside a mass of stone (a block kind's surface or its dress): a line back from the
    // point, against the way it faces, comes out through the stone's face within a few
    // millimetres.
    stone.is_some_and(|grid| {
        grid.hit_where(drawn, at, -a.n, 0.0, PRESSED_M, |c| c.n.dot(-a.n) > 0.7)
            .is_some()
    })
}

/// Whether a point in a triangle's plane is within it.
fn contains(t: &Tri, p: Vec3) -> bool {
    let (e1, e2, w) = (t.p[1] - t.p[0], t.p[2] - t.p[0], p - t.p[0]);
    let (d11, d12, d22) = (e1.dot(e1), e1.dot(e2), e2.dot(e2));
    let (dw1, dw2) = (w.dot(e1), w.dot(e2));
    let det = d11 * d22 - d12 * d12;
    if det.abs() < 1e-12 {
        return false;
    }
    let u = (d22 * dw1 - d12 * dw2) / det;
    let v = (d11 * dw2 - d12 * dw1) / det;
    u >= -1e-4 && v >= -1e-4 && u + v <= 1.0 + 1e-4
}

/// How much two triangles in one plane overlap, in square metres: the one clipped by the
/// other's edges (Sutherland and Hodgman), in the plane's own two axes.
#[cfg(test)]
fn overlap(a: &Tri, b: &Tri) -> f32 {
    clipped(a, b).0
}

/// How much two triangles in one plane overlap, in square metres, and the middle of where
/// they do.
fn clipped(a: &Tri, b: &Tri) -> (f32, Vec3) {
    let u = a.n.any_orthonormal_vector();
    let v = a.n.cross(u);
    // Measured from a corner of the first, not the world's origin: tens of metres out, a
    // place in single precision is good to a few micrometres, and an area summed from such
    // places only to a square centimetre, a sliver beside a triangle counted as over it.
    let origin = a.p[0];
    let flat = |p: Vec3| Vec2::new((p - origin).dot(u), (p - origin).dot(v));
    let mut poly: Vec<Vec2> = b.p.iter().map(|p| flat(*p)).collect();
    let clip: Vec<Vec2> = a.p.iter().map(|p| flat(*p)).collect();
    // Wound the same way as the clipping triangle, counter-clockwise.
    let wind = |t: &[Vec2]| (t[1] - t[0]).perp_dot(t[2] - t[0]);
    let clip = if wind(&clip) < 0.0 {
        vec![clip[0], clip[2], clip[1]]
    } else {
        clip
    };
    for e in 0..3 {
        let (c0, c1) = (clip[e], clip[(e + 1) % 3]);
        let inside = |p: Vec2| (c1 - c0).perp_dot(p - c0) >= 0.0;
        let mut out = Vec::with_capacity(poly.len() + 2);
        for i in 0..poly.len() {
            let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
            let (pin, qin) = (inside(p), inside(q));
            if pin {
                out.push(p);
            }
            if pin != qin {
                let (dp, dq) = ((c1 - c0).perp_dot(p - c0), (c1 - c0).perp_dot(q - c0));
                let t = dp / (dp - dq);
                out.push(p + (q - p) * t);
            }
        }
        poly = out;
        if poly.len() < 3 {
            return (0.0, Vec3::ZERO);
        }
    }
    let mut twice = 0.0;
    for i in 0..poly.len() {
        twice += poly[i].perp_dot(poly[(i + 1) % poly.len()]);
    }
    let flat_middle = poly.iter().copied().sum::<Vec2>() / poly.len() as f32;
    let middle = origin + u * flat_middle.x + v * flat_middle.y;
    (twice.abs() * 0.5, middle)
}

/// Triangles filed by the cubes their bounds reach, to cast lines through.
struct Grid {
    cells: std::collections::HashMap<[i32; 3], Vec<u32>>,
}

impl Grid {
    fn new(drawn: &Drawn) -> Grid {
        Grid::of(drawn, |_| true)
    }

    /// Those of the triangles `keep` keeps.
    fn of(drawn: &Drawn, keep: impl Fn(&Tri) -> bool) -> Grid {
        let mut cells: std::collections::HashMap<[i32; 3], Vec<u32>> =
            std::collections::HashMap::new();
        for (i, t) in drawn.tris.iter().enumerate() {
            if !keep(t) {
                continue;
            }
            let low = (t.p[0].min(t.p[1]).min(t.p[2]) / CELL_M).floor().as_ivec3();
            let high = (t.p[0].max(t.p[1]).max(t.p[2]) / CELL_M).floor().as_ivec3();
            for x in low.x..=high.x {
                for y in low.y..=high.y {
                    for z in low.z..=high.z {
                        cells.entry([x, y, z]).or_default().push(i as u32);
                    }
                }
            }
        }
        Grid { cells }
    }

    /// The nearest triangle a line meets between two distances along it, from either side.
    fn hit(&self, drawn: &Drawn, from: Vec3, dir: Vec3, near: f32, far: f32) -> Option<f32> {
        self.hit_where(drawn, from, dir, near, far, |_| true)
    }

    /// The nearest triangle a line meets between two distances along it, of those `ok` keeps.
    fn hit_where(
        &self,
        drawn: &Drawn,
        from: Vec3,
        dir: Vec3,
        near: f32,
        far: f32,
        ok: impl Fn(&Tri) -> bool,
    ) -> Option<f32> {
        let mut best: Option<f32> = None;
        let mut seen = std::collections::HashSet::new();
        for cell in cells_along(from + dir * near, dir, far - near) {
            let Some(list) = self.cells.get(&cell) else {
                continue;
            };
            for &i in list {
                if !seen.insert(i) {
                    continue;
                }
                let t = &drawn.tris[i as usize];
                if !ok(t) {
                    continue;
                }
                if let Some(at) = meets(from, dir, t)
                    && at >= near
                    && at <= far
                    && best.is_none_or(|b| at < b)
                {
                    best = Some(at);
                }
            }
        }
        best
    }
}

/// The cubes a line passes through over a length, in order (Amanatides and Woo).
fn cells_along(from: Vec3, dir: Vec3, length: f32) -> Vec<[i32; 3]> {
    let mut cell = (from / CELL_M).floor().as_ivec3();
    let step = IVec3::new(sign(dir.x), sign(dir.y), sign(dir.z));
    let next = |c: i32, s: i32| (c + i32::from(s > 0)) as f32 * CELL_M;
    let t_of = |o: f32, d: f32, c: i32, s: i32| {
        if s == 0 {
            f32::INFINITY
        } else {
            (next(c, s) - o) / d
        }
    };
    let mut t_max = Vec3::new(
        t_of(from.x, dir.x, cell.x, step.x),
        t_of(from.y, dir.y, cell.y, step.y),
        t_of(from.z, dir.z, cell.z, step.z),
    );
    let delta = Vec3::new(
        if step.x == 0 {
            f32::INFINITY
        } else {
            CELL_M / dir.x.abs()
        },
        if step.y == 0 {
            f32::INFINITY
        } else {
            CELL_M / dir.y.abs()
        },
        if step.z == 0 {
            f32::INFINITY
        } else {
            CELL_M / dir.z.abs()
        },
    );
    let mut out = vec![cell.to_array()];
    loop {
        let axis = if t_max.x <= t_max.y && t_max.x <= t_max.z {
            0
        } else if t_max.y <= t_max.z {
            1
        } else {
            2
        };
        if t_max[axis] > length {
            break;
        }
        cell[axis] += step[axis];
        t_max[axis] += delta[axis];
        out.push(cell.to_array());
        if out.len() > 4096 {
            break;
        }
    }
    out
}

fn sign(x: f32) -> i32 {
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

/// Where a line meets a triangle, from either side (Möller and Trumbore).
fn meets(from: Vec3, dir: Vec3, t: &Tri) -> Option<f32> {
    let (e1, e2) = (t.p[1] - t.p[0], t.p[2] - t.p[0]);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = from - t.p[0];
    let u = s.dot(p) * inv;
    if !(-1e-5..=1.0 + 1e-5).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < -1e-5 || u + v > 1.0 + 1e-5 {
        return None;
    }
    Some(e2.dot(q) * inv)
}

/// Whether a cube is a solid wall's, every part of it there to see: built, whole, a wall
/// kind's, not glass or an opening.
fn solid(base: &Base, kits: &Kits, at: IVec3) -> bool {
    // A post's top in a deck, boarded over as the deck is (`layout::post_in_deck`).
    if crate::layout::post_in_deck(base, kits, at).is_some() {
        return true;
    }
    base.cube(at).is_some_and(|cube| {
        cube.is_built()
            && cube.fill == Fill::Solid
            && cube.shape == Shape::Whole
            && !cube.kind.is_roof()
            && kits
                .of(cube.kind)
                .is_some_and(|kit| kit.pattern != Pattern::Roof)
    })
}

/// Lines cast through solid walls and down through roofs; each that passes through without
/// meeting a triangle is a hole. Returns how many were cast.
fn see_through(
    base: &Base,
    kits: &Kits,
    drawn: &Drawn,
    detail: Detail,
    slanting: bool,
    findings: &mut Vec<Finding>,
) -> usize {
    let grid = Grid::new(drawn);
    let mut cast = 0;
    // One finding a cube and fault, the first line through it.
    let mut found: BTreeMap<[i32; 3], (Vec3, String)> = BTreeMap::new();
    let faces = [
        IVec3::X,
        IVec3::NEG_X,
        IVec3::Y,
        IVec3::NEG_Y,
        IVec3::Z,
        IVec3::NEG_Z,
    ];
    let ground = base.ground();
    // A timber flight's cubes are drawn as steps on stringers, open under them as a stair
    // is: not a wall to look through.
    let open = crate::layout::open_flights(base, kits);
    for (at, cube) in base.cubes() {
        if found.contains_key(&at.to_array()) || open.contains(&at.to_array()) {
            continue;
        }
        // A roof cube open to the sky is covered: every line down through it meets its
        // covering.
        if cube.kind.is_roof() && cube.is_built() {
            if base.cube(at + IVec3::Y).is_some() {
                continue;
            }
            for (u, v) in samples() {
                let top = (at.as_vec3() + Vec3::new(u, 1.0, v)) * CELL_M;
                let from = top + Vec3::Y * RAY_START_M;
                cast += 1;
                if grid
                    .hit(drawn, from, Vec3::NEG_Y, 0.0, RAY_START_M + CELL_M * 1.02)
                    .is_none()
                {
                    found.insert(
                        at.to_array(),
                        (
                            top,
                            format!("roof cube {at} open to the sky at ({u:.1}, {v:.1})"),
                        ),
                    );
                    break;
                }
            }
            continue;
        }
        if !solid(base, kits, at) {
            continue;
        }
        for face in faces {
            let beside = at + face;
            if solid(base, kits, beside) || base.cube(beside).is_some_and(|c| c.kind.is_roof()) {
                continue;
            }
            // A wall's end, the wall running on behind it: a line in through it near its
            // sides runs along the wall in the air its cubes hold beside a timber wall's
            // faces, not through it, so lines go in up its middle, along its core, and
            // beside it only where another wall meets it there.
            // Carried on up into a roof's cube behind, as a gable is, it runs on too.
            let behind = at - face;
            let end = face.y == 0
                && (solid(base, kits, behind)
                    || (base.cube(behind).is_some_and(|c| c.kind.is_roof())
                        && solid(base, kits, behind - IVec3::Y)));
            let across = IVec3::new(face.z, 0, face.x).abs().as_vec3();
            // A face down into the ground, or a footing's sides in it, is not seen.
            let floor_m = at.y as f32 * CELL_M;
            let column_ground = |c: IVec3| f32::from(ground.height_cm([c.x, c.z])) / 100.0;
            if floor_m + CELL_M <= column_ground(beside) + 0.05
                || (face == IVec3::NEG_Y && floor_m <= column_ground(at) + 0.3)
            {
                continue;
            }
            let n = face.as_vec3();
            let (u_axis, v_axis) = (
                n.any_orthonormal_vector().abs(),
                n.cross(n.any_orthonormal_vector()).abs(),
            );
            let middle = (at.as_vec3() + Vec3::splat(0.5)) * CELL_M;
            // Square to the face but for a few degrees off it either way: a line exactly
            // along an axis threads a seam that runs with it, as the groove between two
            // courses along a log wall seen from its end, which no view sees through.
            let mut dirs: Vec<Vec3> = vec![(-n + u_axis * 0.07 + v_axis * 0.04).normalize()];
            if slanting {
                // Each a few degrees off the other way across too, so none runs in the
                // plane of a seam across the face, as two boards laid tight meet in.
                for (t, off) in [
                    (u_axis, v_axis),
                    (-u_axis, v_axis),
                    (v_axis, u_axis),
                    (-v_axis, u_axis),
                ] {
                    dirs.push((-n + t * 0.6 + off * 0.04).normalize());
                }
            }
            'lines: for (u, v) in samples() {
                let mut offset = (u_axis * (u - 0.5) + v_axis * (v - 0.5)) * CELL_M;
                // Up its middle, or toward a wall beside it, where it meets that wall.
                let side = offset.dot(across);
                let toward = (across * side.signum()).round().as_ivec3();
                if end && !solid(base, kits, at + toward) {
                    offset -= across * side;
                }
                let on_face = middle + n * (CELL_M * 0.5) + offset;
                for &dir in &dirs {
                    let from = on_face - dir * RAY_START_M;
                    // How far it runs inside solid cubes, and which way it comes out.
                    let Some((out_t, out_n)) = through_solid(base, kits, from, dir) else {
                        continue;
                    };
                    let inside = out_t - RAY_START_M;
                    // Only a line that comes out on the other side of the wall, having gone
                    // through it, not one clipping a corner, nor one running along a wall's
                    // length, in the air its cubes hold beside a timber wall's faces, nor one
                    // that comes out into the ground, as down through a floor laid on it.
                    let out = from + dir * out_t;
                    let out_cell = (out / CELL_M).floor().as_ivec3();
                    if !(THROUGH_M..=ALONG_M).contains(&inside)
                        || (out_n.dot(n).abs() < 0.3 && inside > CORNER_M)
                        || out_n.dot(n) > 0.3
                        || out.y < column_ground(out_cell) + 0.02
                    {
                        continue;
                    }
                    cast += 1;
                    if grid.hit(drawn, from, dir, 0.0, out_t + 0.02).is_none() {
                        found.insert(
                            at.to_array(),
                            (
                                on_face,
                                format!(
                                    "{} cube {at}: a line in through its {face} face, heading ({:.2}, {:.2}, {:.2}), meets nothing",
                                    kind_name(kits, cube.kind),
                                    dir.x,
                                    dir.y,
                                    dir.z
                                ),
                            ),
                        );
                        break 'lines;
                    }
                }
            }
        }
    }
    for (_, (at, what)) in found {
        findings.push(Finding {
            by: 0.0,
            fault: Fault::SeeThrough,
            detail,
            at,
            what,
        });
    }
    cast
}

fn kind_name(kits: &Kits, kind: Kind) -> String {
    kits.of(kind).map_or(format!("kind {}", kind.0), |kit| {
        format!("{:?} {}", kit.pattern, kit.id)
    })
}

/// Where on a face lines are cast from: a few places across it, clear of its rounded edges.
fn samples() -> impl Iterator<Item = (f32, f32)> {
    const AT: [f32; 3] = [0.25, 0.5, 0.75];
    AT.into_iter()
        .flat_map(|u| AT.into_iter().map(move |v| (u, v)))
}

/// Following a line from a point outside, where it leaves the solid cubes it enters, as a
/// distance along it, and the face it leaves by; none if it never enters one.
fn through_solid(base: &Base, kits: &Kits, from: Vec3, dir: Vec3) -> Option<(f32, Vec3)> {
    let mut entered = false;
    let mut cell = (from / CELL_M).floor().as_ivec3();
    let step = IVec3::new(sign(dir.x), sign(dir.y), sign(dir.z));
    let next = |c: i32, s: i32| (c + i32::from(s > 0)) as f32 * CELL_M;
    let t_of = |o: f32, d: f32, c: i32, s: i32| {
        if s == 0 {
            f32::INFINITY
        } else {
            (next(c, s) - o) / d
        }
    };
    let mut t_max = Vec3::new(
        t_of(from.x, dir.x, cell.x, step.x),
        t_of(from.y, dir.y, cell.y, step.y),
        t_of(from.z, dir.z, cell.z, step.z),
    );
    let delta = Vec3::new(
        if step.x == 0 {
            f32::INFINITY
        } else {
            CELL_M / dir.x.abs()
        },
        if step.y == 0 {
            f32::INFINITY
        } else {
            CELL_M / dir.y.abs()
        },
        if step.z == 0 {
            f32::INFINITY
        } else {
            CELL_M / dir.z.abs()
        },
    );
    for _ in 0..256 {
        let axis = if t_max.x <= t_max.y && t_max.x <= t_max.z {
            0
        } else if t_max.y <= t_max.z {
            1
        } else {
            2
        };
        let t = t_max[axis];
        cell[axis] += step[axis];
        t_max[axis] += delta[axis];
        let now = solid(base, kits, cell);
        if now {
            entered = true;
        } else if entered {
            let mut out = Vec3::ZERO;
            out[axis] = step[axis] as f32;
            return Some((t, out));
        }
    }
    None
}

/// How far over a roof's top its own pieces may stand, past its covering's thickness and
/// the step of its courses: a ridge's or a hip's cap.
const CAP_M: f32 = 0.1;
/// How high over a roof's top, straight up, a roof kind's own pieces may stand: its covering
/// and its tallest piece (a ridge's capping is modelled standing well over the thatch, D258),
/// each measured square to the roof, up a slope of 45 degrees, and a cap's worth more.
fn roof_reach(kit: &crate::kit::Kit) -> f32 {
    let pieces = kit.dress.made.as_deref();
    // Each piece as seated: over its covering where its part lies on it (D290).
    let tallest = kit
        .roof
        .parts
        .iter()
        .filter_map(|(&part, spec)| {
            let module = pieces.and_then(|p| p.modules.get(&spec.module))?;
            Some((crate::roofing::seated(kit, part), module))
        })
        .flat_map(|(lift, module)| {
            module
                .near
                .iter()
                .chain(&module.cap)
                .flat_map(move |mesh| mesh.positions.iter().map(move |p| p[1] + lift))
        })
        .fold(0.0, f32::max);
    (kit.roof.thick + kit.roof.relief).max(tallest) * std::f32::consts::SQRT_2 + CAP_M
}

/// How far out of a roof's edge its boards may hang.
/// How far out over a piece's face a block surface may be and still bury it: past this the
/// surface is another wall, not relief over the piece.
const BURIED_M: f32 = 0.08;
/// A mesh of pieces is buried when this share of its faces is under a surface.
const BURIED_SHARE: f32 = 0.15;

/// Meshes of pieces whose faces lie under a block kind's surface facing the same way: a line
/// out from the face meets the surface within a few centimetres, so the surface's relief
/// covers what should stand proud of it.
fn buried(kits: &Kits, drawn: &Drawn, detail: Detail, findings: &mut Vec<Finding>) {
    let is_surface = |mesh: u32| {
        drawn.kinds[mesh as usize].is_some_and(|(kind, material)| {
            matches!(material, Material::Masonry | Material::Floor)
                && kits
                    .of(kind)
                    .is_some_and(|kit| kit.pattern == Pattern::Masonry)
        })
    };
    let mut cells: std::collections::HashMap<[i32; 3], Vec<u32>> = std::collections::HashMap::new();
    for (i, t) in drawn.tris.iter().enumerate() {
        if !is_surface(t.mesh) {
            continue;
        }
        let low = (t.p[0].min(t.p[1]).min(t.p[2]) / CELL_M).floor().as_ivec3();
        let high = (t.p[0].max(t.p[1]).max(t.p[2]) / CELL_M).floor().as_ivec3();
        for x in low.x..=high.x {
            for y in low.y..=high.y {
                for z in low.z..=high.z {
                    cells.entry([x, y, z]).or_default().push(i as u32);
                }
            }
        }
    }
    if cells.is_empty() {
        return;
    }
    let grid = Grid { cells };
    // Each mesh of pieces: its area, how much of it is buried, and where first.
    let mut meshes: BTreeMap<u32, (f32, f32, Vec3)> = BTreeMap::new();
    for t in &drawn.tris {
        let Some((kind, _)) = drawn.kinds[t.mesh as usize] else {
            continue;
        };
        if is_surface(t.mesh) || kits.of(kind).is_none_or(|kit| kit.pattern == Pattern::Roof) {
            continue;
        }
        let area = (t.p[1] - t.p[0]).cross(t.p[2] - t.p[0]).length() * 0.5;
        let middle = (t.p[0] + t.p[1] + t.p[2]) / 3.0;
        let entry = meshes.entry(t.mesh).or_insert((0.0, 0.0, middle));
        entry.0 += area;
        let over = grid.hit_where(drawn, middle, t.n, 0.001, BURIED_M, |s| s.n.dot(t.n) > 0.7);
        if over.is_some() {
            if entry.1 == 0.0 {
                entry.2 = middle;
            }
            entry.1 += area;
        }
    }
    for (mesh, (area, under, at)) in meshes {
        if under > 1e-3 && under > area * BURIED_SHARE {
            findings.push(Finding {
                fault: Fault::Buried,
                detail,
                at,
                by: 0.0,
                what: format!(
                    "{}: {:.0}% of its faces under a block surface's relief",
                    drawn.meshes[mesh as usize],
                    100.0 * under / area
                ),
            });
        }
    }
}

/// How far out past a roof's edge its pieces may stand, at the least: a board's hang.
const EDGE_M: f32 = 0.08;

/// How far out past a roof's edge, level, a roof kind's own pieces may stand: as far as its
/// verge's and eave's modules are modelled out past the edge they are laid along (a thatch
/// verge's bound bundle wraps the covering's cut edge, D258), and a board's hang at least.
fn edge_reach(kit: &crate::kit::Kit) -> f32 {
    use crate::layout::roofs::RoofPart;
    let pieces = kit.dress.made.as_deref();
    let out = kit
        .roof
        .parts
        .iter()
        .filter(|(part, _)| matches!(part, RoofPart::Verge | RoofPart::Eave))
        .filter_map(|(_, spec)| pieces.and_then(|p| p.modules.get(&spec.module)))
        .flat_map(|module| module.near.iter().chain(&module.cap))
        .flat_map(|mesh| mesh.positions.iter().map(|p| p[2]))
        .fold(0.0, f32::max);
    EDGE_M.max(out + 0.02)
}
/// How far a trim may stand out of its cubes (D192), and a little for its relief.
const TRIM_M: f32 = CELL_M / 8.0 + 0.012;

/// Pieces out past the planes they belong within: over a roof's covering, out past its
/// edges, or out of their cubes further than a trim may stand.
fn pokes_out(
    base: &Base,
    kits: &Kits,
    roofs: &crate::layout::roofs::Roofs,
    drawn: &Drawn,
    detail: Detail,
    findings: &mut Vec<Finding>,
) {
    // The worst of each mesh and how: how far out, and where.
    let mut worst: BTreeMap<(u32, &'static str), (f32, Vec3)> = BTreeMap::new();
    let mut note = |mesh: u32, by: f32, at: Vec3, how: &'static str| {
        let entry = worst.entry((mesh, how)).or_insert((0.0, at));
        if by > entry.0 {
            *entry = (by, at);
        }
    };
    let cube_at = |p: Vec3| base.cube((p / CELL_M).floor().as_ivec3());
    for t in &drawn.tris {
        let Some((kind, _)) = drawn.kinds[t.mesh as usize] else {
            continue;
        };
        let Some(kit) = kits.of(kind) else {
            continue;
        };
        let middle = (t.p[0] + t.p[1] + t.p[2]) / 3.0;
        let is_roof = kit.pattern == Pattern::Roof;
        // Over the roof nearest under it: its covering and caps may stand so far over the
        // roof's top; a wall's piece in a roof cube, or over one, not at all.
        if let Some(top) = roofs.top_over(middle.x, middle.z, middle.y - 0.6) {
            // A roof's own piece over the roof it lies on, the nearest under it: where its
            // eaves pass over a lower roof's, not over that one. Looked for a centimetre in
            // behind the face, so a covering's edge closed down on its square's side is over
            // its own square, not the next.
            let behind = middle - t.n * 0.01;
            let top = if is_roof {
                roofs
                    .top_under(behind.x, behind.z, middle.y)
                    .map_or(top, |under| under.max(top))
            } else {
                top
            };
            // A wall's own face standing up beside a roof is not in it.
            let inside = middle - t.n * 0.01;
            let roof_cube = cube_at(inside).filter(|c| c.kind.is_roof()).or_else(|| {
                cube_at(inside)
                    .is_none()
                    .then(|| cube_at(inside - Vec3::Y * CELL_M))
                    .flatten()
                    .filter(|c| c.kind.is_roof())
            });
            let in_roof = is_roof || roof_cube.is_some();
            // A roof's own pieces as far as they reach; a wall's into the roof's covering,
            // which closes over it, but not through it.
            let allowed = if is_roof {
                roof_reach(kit)
            } else {
                roof_cube
                    .and_then(|c| kits.of(c.kind))
                    .map_or(0.0, |roof| roof.roof.thick)
                    .max(0.02)
            };
            if in_roof && middle.y > top + allowed {
                note(t.mesh, middle.y - top - allowed, middle, "over the roof");
            }
        } else if is_roof {
            // A roof's piece where no roof is, near or under it: out past its edges.
            let reach = edge_reach(kit);
            let near_edge = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z]
                .iter()
                .any(|d| {
                    roofs
                        .top_over(
                            middle.x + d.x * reach,
                            middle.z + d.z * reach,
                            middle.y - 0.6,
                        )
                        .is_some()
                });
            let under = roofs.top_over(middle.x, middle.z, middle.y - 3.0).is_some();
            if !near_edge && !under {
                note(t.mesh, EDGE_M, middle, "out past the roof's edge");
            }
        }
        if !is_roof {
            // Out of its cubes: how far from the nearest cube of the base, of any kind, out
            // of any one of its faces (a trim at a corner stands out of both), unless it
            // is in the ground, as a plinth sunk to meet it is. Over a stair, the cube over
            // it counts too: a flight's stringers rise along the line of its noses, over the
            // back of each step.
            for p in t.p {
                let cell = (p / CELL_M).floor().as_ivec3();
                // The ground as the layout sinks a plinth to it: where the base keeps no
                // sample, as beside its cubes, from the samples beside the column.
                let ground = crate::layout::roles::ground_ticks(base, [cell.x, cell.z]) as f32
                    * (CELL_M / crate::layout::TICKS as f32);
                if p.y < ground {
                    continue;
                }
                let mut nearest = f32::MAX;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            let c = cell + IVec3::new(dx, dy, dz);
                            let over_stair = base
                                .cube(c - IVec3::Y)
                                .is_some_and(|under| matches!(under.shape, crate::Shape::Stair(_)));
                            if base.cube(c).is_none() && !over_stair {
                                continue;
                            }
                            let low = c.as_vec3() * CELL_M;
                            let out = (low - p)
                                .max(p - (low + Vec3::splat(CELL_M)))
                                .max(Vec3::ZERO);
                            nearest = nearest.min(out.max_element());
                        }
                    }
                }
                if nearest > TRIM_M {
                    note(t.mesh, nearest - TRIM_M, p, "out of its cubes");
                }
            }
        }
    }
    for ((mesh, how), (by, at)) in worst {
        findings.push(Finding {
            fault: Fault::PokesOut,
            detail,
            at,
            by,
            what: format!(
                "{}: {how}, {:.0} cm too far",
                drawn.meshes[mesh as usize],
                by * 100.0
            ),
        });
    }
}

/// Lines in under a roof, through every roof cube standing on a solid wall and open to the
/// air on the side the wall's face is: between the wall's top and the roof's underside the
/// wall is carried up, so each meets it. Returns how many were cast.
fn under_roof(
    base: &Base,
    kits: &Kits,
    roofs: &crate::layout::roofs::Roofs,
    drawn: &Drawn,
    detail: Detail,
    findings: &mut Vec<Finding>,
) -> usize {
    let grid = Grid::new(drawn);
    let mut cast = 0;
    let open = |at: IVec3| {
        base.cube(at)
            .is_none_or(|c| c.fill == Fill::Open && !c.kind.is_roof())
    };
    for (at, cube) in base.cubes() {
        if !cube.kind.is_roof() || !cube.is_built() || !solid(base, kits, at - IVec3::Y) {
            continue;
        }
        let bottom = at.y as f32 * CELL_M;
        'faces: for face in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
            if !open(at + face) || !open(at + face - IVec3::Y) {
                continue;
            }
            // Far off, only from outside: a face open to the room or the loft under the
            // roof's cubes is seen only from within, and no one within is far from it.
            let indoors = (1..=crate::STOREY_CUBES * 2).any(|k| {
                base.cube(at + face + IVec3::Y * k)
                    .is_some_and(|c| c.kind.is_roof())
            });
            if indoors && matches!(detail, Detail::Far | Detail::Proxy) {
                continue;
            }
            let n = face.as_vec3();
            let along = Vec3::new(n.z.abs(), 0.0, n.x.abs());
            let middle = (at.as_vec3() + Vec3::splat(0.5)) * CELL_M;
            for u in [0.25, 0.5, 0.75] {
                let on_face = middle + n * (CELL_M * 0.5) + along * (u - 0.5) * CELL_M;
                // Half a cube in from the face, where the wall carried up stands.
                let inside = on_face - n * (CELL_M * 0.5);
                let Some(under) = roofs.under_over(inside.x, inside.z, bottom) else {
                    continue;
                };
                for h in [0.1, 0.3, 0.5, 0.7, 0.9] {
                    let y = bottom + h * CELL_M;
                    if y > under - 0.03 {
                        break;
                    }
                    let from = Vec3::new(on_face.x, y, on_face.z) + n * RAY_START_M;
                    cast += 1;
                    if grid
                        .hit(drawn, from, -n, 0.0, RAY_START_M + CELL_M + 0.02)
                        .is_none()
                    {
                        findings.push(Finding {
                            by: 0.0,
                            fault: Fault::UnderRoof,
                            detail,
                            at: Vec3::new(on_face.x, y, on_face.z),
                            what: format!(
                                "roof cube {at} on a wall: open {:.0} cm over the wall's top, under the roof, through its {face} face",
                                h * CELL_M * 100.0
                            ),
                        });
                        continue 'faces;
                    }
                }
            }
        }
    }
    cast
}

/// The bases inspected: every sensible roof over every plan in each wall kind, and each
/// two wall kinds mixed every way `roof_rows` mixes them, on ground falling a metre across
/// them, on their footings; `only` keeps those whose names hold it.
pub fn cases(kits: &Kits, only: Option<&str>) -> Vec<(String, Base)> {
    let walls: Vec<&crate::kit::Kit> = kits
        .0
        .iter()
        .filter(|kit| kit.pattern != Pattern::Roof)
        .collect();
    let roof = |walls: Kind| kits.roof_for(walls).unwrap_or(Kind::FIRST_ROOF);
    let mut out = Vec::new();
    let mut add = |name: String, ops: Vec<crate::BuildOp>| {
        if only.is_some_and(|only| !name.contains(only)) {
            return;
        }
        out.push((name, on_ground(kits, &ops)));
    };
    for kit in &walls {
        for (row, ops) in crate::roof_rows(&[kit.kind()], false, &roof) {
            add(format!("{}: {row}", kit.id), ops);
        }
    }
    for (i, a) in walls.iter().enumerate() {
        for b in &walls[i + 1..] {
            for (row, ops) in crate::roof_rows(&[a.kind(), b.kind()], true, &roof) {
                add(format!("{} with {}: {row}", a.id, b.id), ops);
            }
        }
    }
    // The QA build film's cabin as a player leaves it (`qa_cabin`): a storey over a roof,
    // a dormer, a deck, a wall changed and a window carved, in each wall kit, the wall changed
    // to the next of its pattern, and in all of them mixed.
    // On ground falling half a metre across it, as the film's does where its stairs come down.
    let height = |[x, z]: [i32; 2]| (-(x as f32) * 2.0 - z as f32) as i16;
    let other = |kit: &crate::kit::Kit| {
        let like: Vec<&&crate::kit::Kit> = walls
            .iter()
            .filter(|other| other.pattern == kit.pattern)
            .collect();
        let pool: Vec<&crate::kit::Kit> = if like.len() > 1 {
            like.into_iter().copied().collect()
        } else {
            walls.clone()
        };
        let at = pool
            .iter()
            .position(|other| other.id == kit.id)
            .unwrap_or(0);
        pool[(at + 1) % pool.len()].kind()
    };
    let mut cabin = |name: String, kinds: Vec<Kind>, other: Kind| {
        if only.is_some_and(|only| !name.contains(only)) {
            return;
        }
        out.push((name, crate::qa_cabin::cabin(kits, &kinds, other, &height)));
    };
    for kit in &walls {
        cabin(
            format!("{}: QA cabin", kit.id),
            vec![kit.kind()],
            other(kit),
        );
    }
    if let Some(first) = walls.first() {
        let ids: Vec<String> = walls.iter().map(|kit| kit.id.to_string()).collect();
        cabin(
            format!("{}: QA cabin", ids.join(" with ")),
            walls.iter().map(|kit| kit.kind()).collect(),
            other(first),
        );
    }
    // The castle, of 140,000 cubes (`castle`), in the footings' stone with each kind of
    // long members for its timber.
    if let Some(stone) = kits.footing() {
        for timber in walls.iter().filter(|kit| kit.pattern == Pattern::Logs) {
            let name = format!("{} with {}: castle", stone.id, timber.id);
            if only.is_some_and(|only| !name.contains(only)) {
                continue;
            }
            let ops = crate::castle::castle(stone.kind(), timber.kind(), roof(stone.kind()));
            out.push((name, on_ground(kits, &ops)));
        }
    }
    out
}

/// A base of shortcuts on ground falling a metre across it, with its footings laid, as the
/// game lays them (`Build::lay_footings`).
fn on_ground(kits: &Kits, ops: &[crate::BuildOp]) -> Base {
    let mut base = Base::default();
    for &op in ops {
        base.apply(op);
    }
    let height = |[x, z]: [i32; 2]| (-(x as f32) * 4.0 - z as f32 * 2.0) as i16;
    let ring = base.ground_ring();
    base.sample_ground(ring.into_iter().map(|column| crate::Sample {
        column,
        height_cm: height(column),
    }));
    if let Some(stone) = kits.footing() {
        base.lay_footings(stone.kind(), height);
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_roofs_eave_over_a_lower_roof_is_measured_from_its_own_roof() {
        // Two flat roof cubes, the second a cube east and a cube up, each carried on half a
        // metre past its edges: over x = 0.75 the first's eaves lie at 0.5 m under the
        // second's top at 1.0 m.
        let roof = crate::kit::Kit {
            id: Kind::FIRST_ROOF.0,
            pattern: Pattern::Roof,
            eaves: 0.5,
            ..Default::default()
        };
        let kits = Kits(vec![crate::kit::Kit::default(), roof]);
        let mut base = Base::default();
        for at in [[0, 0, 0], [1, 1, 0]] {
            base.apply(crate::BuildOp::Fill {
                min: at,
                max: at,
                cube: Some(crate::Cube::new(Kind::FIRST_ROOF, Fill::Solid)),
            });
        }
        let roofs = crate::layout::roofs::Roofs::whole(&base, &kits, crate::layout::Scope::ALL);
        let found = |y: f32| {
            let drawn = Drawn {
                tris: vec![tri(
                    [
                        Vec3::new(0.7, y, 0.2),
                        Vec3::new(0.8, y, 0.2),
                        Vec3::new(0.75, y, 0.3),
                    ],
                    0,
                )],
                meshes: vec!["covering".into()],
                kinds: vec![Some((Kind::FIRST_ROOF, Material::Covering))],
            };
            let mut findings = Vec::new();
            pokes_out(&base, &kits, &roofs, &drawn, Detail::Near, &mut findings);
            findings
        };
        // The upper roof's covering just over its own top: not over the lower roof.
        assert!(found(1.05).is_empty(), "{:?}", found(1.05));
        // Half a metre over the upper roof: over it.
        assert_eq!(found(1.5).len(), 1);
    }

    #[test]
    fn a_roofs_piece_may_stand_past_its_edge_as_far_as_its_verge_is_modelled_and_no_further() {
        // A roof kind whose verge is modelled standing 0.2 m out past the edge it is laid
        // along, as thatch's bound bundle wraps its covering's cut edge; one roof cube, with
        // no eaves, its top half a metre up over x and z from 0 to 0.5.
        use crate::layout::roofs::RoofPart;
        let verge = crate::dress::Stone {
            dims: [0.0; 3],
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.2], [0.0, 0.1, 0.0]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            triangles: vec![[0, 1, 2]],
            uvs: vec![],
        };
        let mut pieces = crate::dress::Pieces::default();
        pieces.modules.insert(
            "verge".into(),
            crate::dress::Module {
                length: 1.0,
                near: vec![verge],
                ..Default::default()
            },
        );
        let mut roof = crate::kit::Kit {
            id: Kind::FIRST_ROOF.0,
            pattern: Pattern::Roof,
            eaves: 0.0,
            ..Default::default()
        };
        roof.roof.parts.insert(
            RoofPart::Verge,
            crate::kit::Part {
                module: "verge".into(),
                material: Material::Piece,
                ends: None,
                tint: [1.0; 3],
                on_roof: false,
            },
        );
        roof.dress.made = Some(std::sync::Arc::new(pieces));
        let kits = Kits(vec![crate::kit::Kit::default(), roof]);
        let mut base = Base::default();
        base.apply(crate::BuildOp::Fill {
            min: [0, 0, 0],
            max: [0, 0, 0],
            cube: Some(crate::Cube::new(Kind::FIRST_ROOF, Fill::Solid)),
        });
        let roofs = crate::layout::roofs::Roofs::whole(&base, &kits, crate::layout::Scope::ALL);
        // A small face of the roof kind standing so far out past the roof's east edge.
        let found = |out: f32| {
            let x = 0.5 + out;
            let drawn = Drawn {
                tris: vec![tri(
                    [
                        Vec3::new(x - 0.01, 0.5, 0.2),
                        Vec3::new(x + 0.01, 0.5, 0.2),
                        Vec3::new(x, 0.5, 0.3),
                    ],
                    0,
                )],
                meshes: vec!["verge".into()],
                kinds: vec![Some((Kind::FIRST_ROOF, Material::Piece))],
            };
            let mut findings = Vec::new();
            pokes_out(&base, &kits, &roofs, &drawn, Detail::Near, &mut findings);
            findings
        };
        assert!(found(0.15).is_empty(), "{:?}", found(0.15));
        let past = found(0.4);
        assert_eq!(past.len(), 1, "{past:?}");
        assert_eq!(past[0].fault, Fault::PokesOut);
    }

    #[test]
    fn a_piece_under_a_block_surface_is_buried_and_one_proud_of_it_is_not() {
        let stone = crate::kit::Kit {
            id: 2,
            pattern: Pattern::Masonry,
            ..Default::default()
        };
        let kits = Kits(vec![crate::kit::Kit::default(), stone]);
        // A square of wall face a metre across facing +Z at `z`, as two triangles.
        let square = |z: f32, mesh: u32| {
            let c = |x: f32, y: f32| Vec3::new(x, y, z);
            [
                tri([c(0.0, 0.0), c(1.0, 0.0), c(1.0, 1.0)], mesh),
                tri([c(0.0, 0.0), c(1.0, 1.0), c(0.0, 1.0)], mesh),
            ]
        };
        let found = |piece_z: f32| {
            let mut tris = square(0.25, 0).to_vec();
            tris.extend(square(piece_z, 1));
            let drawn = Drawn {
                tris,
                meshes: vec!["daub".into(), "frame".into()],
                kinds: vec![
                    Some((Kind(2), Material::Masonry)),
                    Some((Kind(1), Material::Hewn)),
                ],
            };
            let mut findings = Vec::new();
            buried(&kits, &drawn, Detail::Near, &mut findings);
            findings
        };
        // A frame's face 2 cm behind the daub's: under it.
        let under = found(0.23);
        assert_eq!(under.len(), 1, "{under:?}");
        assert_eq!(under[0].fault, Fault::Buried);
        // 3 cm proud of it, as the frame is meant to stand: not.
        assert!(found(0.28).is_empty());
        // A wall's thickness behind: another face, not relief over it.
        assert!(found(0.0).is_empty());
    }

    #[test]
    fn a_posts_top_in_a_deck_is_as_solid_as_the_deck() {
        // A post two cubes high, a deck beside its top cube on its layer: the deck is laid
        // over the post's top (`layout::post_in_deck`), so lines through the deck in by its
        // side there are inside it, as by any other of its cubes. A kind laying decks.
        let mut kit = crate::kit::Kit::default();
        kit.textures.insert(
            Material::Floor,
            crate::kit::Texture {
                path: "boards".into(),
                metres: [1.0, 1.0],
                depth: 0.0,
                reflectance: crate::kit::matte(),
                strips: None,
                cells: 1,
                worn: None,
            },
        );
        let kits = Kits(vec![kit]);
        let mut base = Base::default();
        for (at, shape) in [
            ([0, 0, 0], crate::Shape::Post),
            ([0, 1, 0], crate::Shape::Post),
            ([1, 1, 0], crate::Shape::Whole),
        ] {
            base.apply(crate::BuildOp::Fill {
                min: at,
                max: at,
                cube: Some(crate::Cube {
                    shape,
                    ..crate::Cube::new(Kind(1), Fill::Solid)
                }),
            });
        }
        assert!(solid(&base, &kits, IVec3::new(0, 1, 0)));
        assert!(!solid(&base, &kits, IVec3::new(0, 0, 0)));
    }

    #[test]
    fn a_walls_end_carried_on_up_into_a_roof_behind_is_looked_into_along_its_core() {
        // A wall cube, its south face open, with a roof's cube behind it to the north on a
        // wall cube: the wall runs on behind it, carried up into the roof as a gable is, so
        // a line in through its south face near its sides runs along the wall in the air
        // beside a timber wall's faces. Drawn: a member up its middle alone.
        let roof = crate::kit::Kit {
            id: Kind::FIRST_ROOF.0,
            pattern: Pattern::Roof,
            ..Default::default()
        };
        let kits = Kits(vec![crate::kit::Kit::default(), roof]);
        let wall = |at: [i32; 3], kind: Kind| crate::BuildOp::Fill {
            min: at,
            max: at,
            cube: Some(crate::Cube::new(kind, Fill::Solid)),
        };
        let found = |gable: bool| {
            let mut base = Base::default();
            base.apply(wall([0, 1, 1], Kind(1)));
            base.apply(wall([0, 0, 0], Kind(1)));
            if gable {
                base.apply(wall([0, 1, 0], Kind::FIRST_ROOF));
            }
            // Up its middle, and across it each other way, whole.
            let c = |x: f32, y: f32| Vec3::new(x, y, 0.75);
            let across = |p: [Vec3; 4]| [tri([p[0], p[1], p[2]], 0), tri([p[0], p[2], p[3]], 0)];
            let mut tris =
                across([c(0.15, 0.4), c(0.35, 0.4), c(0.35, 1.1), c(0.15, 1.1)]).to_vec();
            let x = |y: f32, z: f32| Vec3::new(0.25, y, z);
            tris.extend(across([x(0.4, 0.4), x(1.1, 0.4), x(1.1, 1.1), x(0.4, 1.1)]));
            let y = |x: f32, z: f32| Vec3::new(x, 0.75, z);
            tris.extend(across([
                y(-0.1, 0.4),
                y(0.6, 0.4),
                y(0.6, 1.1),
                y(-0.1, 1.1),
            ]));
            let drawn = Drawn {
                tris,
                meshes: vec!["log".into()],
                kinds: vec![Some((Kind(1), Material::Log))],
            };
            let mut findings = Vec::new();
            see_through(&base, &kits, &drawn, Detail::Near, false, &mut findings);
            findings
                .iter()
                .any(|f| f.fault == Fault::SeeThrough && f.what.contains("cube [0, 1, 1]"))
        };
        assert!(!found(true), "a gable's end looked into beside its core");
        // With nothing behind it, the cube is a wall of its own, looked through anywhere.
        assert!(found(false));
    }

    #[test]
    fn far_off_a_gap_under_a_roof_counts_only_from_outside() {
        // A wall cube with a roof cube on it, nothing drawn: the gap over the wall's top is
        // open through both its east face, to a loft under the roof cube over the east,
        // and its west face, to the open air.
        let roof = crate::kit::Kit {
            id: Kind::FIRST_ROOF.0,
            pattern: Pattern::Roof,
            eaves: 0.0,
            ..Default::default()
        };
        let kits = Kits(vec![crate::kit::Kit::default(), roof]);
        let mut base = Base::default();
        base.apply(crate::BuildOp::Fill {
            min: [0, 0, 0],
            max: [0, 0, 0],
            cube: Some(crate::Cube::new(Kind(1), Fill::Solid)),
        });
        for at in [[0, 1, 0], [1, 2, 0]] {
            base.apply(crate::BuildOp::Fill {
                min: at,
                max: at,
                cube: Some(crate::Cube::new(Kind::FIRST_ROOF, Fill::Solid)),
            });
        }
        let roofs = crate::layout::roofs::Roofs::whole(&base, &kits, crate::layout::Scope::ALL);
        let drawn = Drawn {
            tris: vec![],
            meshes: vec![],
            kinds: vec![],
        };
        let faces = |detail: Detail| {
            let mut findings = Vec::new();
            under_roof(&base, &kits, &roofs, &drawn, detail, &mut findings);
            // The east and west faces, whose middles lie inside the cube north to south.
            let mut faces: Vec<bool> = findings
                .iter()
                .filter(|f| f.at.z > 0.01 && f.at.z < 0.49)
                .map(|f| f.at.x > 0.25)
                .collect();
            faces.sort();
            faces
        };
        // Near, both: someone in the loft sees the one, someone outside the other.
        assert_eq!(faces(Detail::Near), vec![false, true]);
        // Far, only the west, open to the air.
        assert_eq!(faces(Detail::Far), vec![false]);
    }

    fn tri(p: [Vec3; 3], mesh: u32) -> Tri {
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize();
        Tri {
            p,
            n,
            d: n.dot(p[0]),
            mesh,
        }
    }

    /// A box's twelve triangles, wound outward.
    fn cuboid(min: Vec3, max: Vec3, mesh: u32) -> Vec<Tri> {
        let mut out = Vec::new();
        for axis in 0..3 {
            for high in [false, true] {
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                let corner = |a: f32, b: f32| {
                    let mut p = if high { max } else { min };
                    p[u] = if a > 0.0 { max[u] } else { min[u] };
                    p[v] = if b > 0.0 { max[v] } else { min[v] };
                    p
                };
                let q = [
                    corner(0.0, 0.0),
                    corner(1.0, 0.0),
                    corner(1.0, 1.0),
                    corner(0.0, 1.0),
                ];
                let (a, b) = if high {
                    ([0, 1, 2], [0, 2, 3])
                } else {
                    ([0, 2, 1], [0, 3, 2])
                };
                out.push(tri(a.map(|i| q[i]), mesh));
                out.push(tri(b.map(|i| q[i]), mesh));
            }
        }
        out
    }

    #[test]
    fn a_line_along_a_timber_wall_beside_its_logs_is_not_through_it_and_a_hole_is() {
        // A wall of logs three cubes long on the ground, its lower course narrower than its
        // cubes as a log wall is, under a plate as wide as them, with daub between them at
        // the wall's core: a line in from the wall's end beside the course, in the air its
        // cubes hold, runs along the wall under the plate and meets nothing.
        let kits = Kits(vec![crate::kit::Kit::default()]);
        let kind = kits.0[0].kind();
        let mut base = Base::default();
        base.apply(crate::BuildOp::Fill {
            min: [0, 0, 0],
            max: [2, 0, 0],
            cube: Some(crate::Cube::new(kind, Fill::Solid)),
        });
        let wall = |gap: Option<(f32, f32)>| {
            let mut tris = Vec::new();
            let spans = match gap {
                Some((a, b)) => vec![(0.0, a), (b, 1.5)],
                None => vec![(0.0, 1.5)],
            };
            for (a, b) in spans {
                tris.extend(cuboid(Vec3::new(a, 0.0, 0.14), Vec3::new(b, 0.24, 0.36), 0));
                tris.extend(cuboid(Vec3::new(a, 0.24, 0.2), Vec3::new(b, 0.26, 0.3), 0));
                tris.extend(cuboid(Vec3::new(a, 0.26, 0.0), Vec3::new(b, 0.5, 0.5), 0));
            }
            let drawn = Drawn {
                tris,
                meshes: vec!["logs".into()],
                kinds: vec![Some((kind, Material::Log))],
            };
            let mut findings = Vec::new();
            see_through(&base, &kits, &drawn, Detail::Near, true, &mut findings);
            findings
        };
        let whole = wall(None);
        assert!(whole.is_empty(), "{whole:?}");
        let holed = wall(Some((0.7, 1.3)));
        assert!(
            holed.iter().any(|f| f.fault == Fault::SeeThrough),
            "{holed:?}"
        );
    }

    #[test]
    fn a_trim_out_of_its_cubes_is_measured_off_each_face_and_not_in_the_ground() {
        // A stone cube on the ground, and a trim's triangle round its corner, standing out
        // of each face by `out`, at `y`.
        let stone = crate::kit::Kit {
            id: 2,
            pattern: Pattern::Masonry,
            ..Default::default()
        };
        let kits = Kits(vec![stone]);
        let mut base = Base::default();
        base.apply(crate::BuildOp::Fill {
            min: [0, 0, 0],
            max: [0, 0, 0],
            cube: Some(crate::Cube::new(Kind(2), Fill::Solid)),
        });
        let roofs = crate::layout::roofs::Roofs::whole(&base, &kits, crate::layout::Scope::ALL);
        let found = |out: f32, y: f32| {
            let c = |x: f32, z: f32| Vec3::new(x, y, z);
            let drawn = Drawn {
                tris: vec![tri([c(-out, -out), c(0.2, -out), c(-out, 0.2)], 0)],
                meshes: vec!["plinth".into()],
                kinds: vec![Some((Kind(2), Material::Dressed))],
            };
            let mut findings = Vec::new();
            pokes_out(&base, &kits, &roofs, &drawn, Detail::Near, &mut findings);
            findings
        };
        // An eighth of a cube out of both faces at a corner is a trim's reach.
        assert!(found(0.06, 0.2).is_empty());
        // Further out of one is not.
        assert_eq!(found(0.1, 0.2).len(), 1);
        // Under the ground, where a plinth is sunk to meet it, nothing is out.
        assert!(found(0.1, -0.2).is_empty());
        // The ground 40 cm up under the cube, and none kept beside it, where no cube is: the
        // ground there is as the layout takes it, from beside it, and the trim in it.
        let mut raised = base.clone();
        raised.sample_ground([crate::Sample {
            column: [0, 0],
            height_cm: 40,
        }]);
        let c = |x: f32, z: f32| Vec3::new(x, 0.3, z);
        let drawn = Drawn {
            tris: vec![tri([c(-0.1, -0.1), c(0.2, -0.1), c(-0.1, 0.2)], 0)],
            meshes: vec!["plinth".into()],
            kinds: vec![Some((Kind(2), Material::Dressed))],
        };
        let mut findings = Vec::new();
        pokes_out(&raised, &kits, &roofs, &drawn, Detail::Near, &mut findings);
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_wall_reaching_into_a_roofs_covering_is_not_through_it() {
        // A stone cube under a roof's sloping cube whose covering is 10 cm thick, and a
        // level triangle of stone over the middle of the roof cube, at `over` above the
        // roof's deck there.
        let stone = crate::kit::Kit {
            id: 2,
            pattern: Pattern::Masonry,
            ..Default::default()
        };
        let mut roof = crate::kit::Kit {
            id: Kind::FIRST_ROOF.0,
            pattern: Pattern::Roof,
            ..Default::default()
        };
        roof.roof.thick = 0.1;
        let kits = Kits(vec![stone, roof]);
        let mut base = Base::default();
        base.apply(crate::BuildOp::Fill {
            min: [0, 0, 0],
            max: [0, 0, 0],
            cube: Some(crate::Cube::new(Kind(2), Fill::Solid)),
        });
        base.apply(crate::BuildOp::Fill {
            min: [0, 1, 0],
            max: [0, 1, 0],
            cube: Some(crate::Cube {
                shape: crate::Shape::Slope(crate::Facing::North),
                ..crate::Cube::new(Kind::FIRST_ROOF, Fill::Solid)
            }),
        });
        let roofs = crate::layout::roofs::Roofs::whole(&base, &kits, crate::layout::Scope::ALL);
        let (x, z) = (0.25, 0.25);
        let deck = roofs.top_over(x, z, 0.0).expect("a roof");
        let found = |over: f32| {
            let c = |dx: f32, dz: f32| Vec3::new(x + dx, deck + over, z + dz);
            let drawn = Drawn {
                tris: vec![tri([c(-0.01, -0.01), c(-0.01, 0.02), c(0.02, -0.01)], 0)],
                meshes: vec!["gable".into()],
                kinds: vec![Some((Kind(2), Material::Floor))],
            };
            let mut findings = Vec::new();
            pokes_out(&base, &kits, &roofs, &drawn, Detail::Near, &mut findings);
            findings
        };
        assert!(found(0.05).is_empty(), "{:?}", found(0.05));
        assert_eq!(found(0.15).len(), 1);
    }

    #[test]
    fn two_triangles_in_one_plane_overlap_by_their_shared_area() {
        let a = tri([Vec3::ZERO, Vec3::X, Vec3::Y], 0);
        let b = tri([Vec3::ZERO, Vec3::X, Vec3::Y], 1);
        assert!((overlap(&a, &b) - 0.5).abs() < 1e-5);
        let c = tri(
            [
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(2.0, 1.0, 0.0),
                Vec3::new(1.0, 2.0, 0.0),
            ],
            1,
        );
        assert!(overlap(&a, &c) < 1e-6);
    }

    #[test]
    fn triangles_side_by_side_far_from_the_origin_do_not_overlap() {
        // A rafter's sawn end, 45 degrees across, 90 metres out: a sliver laid beside a
        // triangle along its edge, touching it and nowhere over it.
        let a = tri(
            [
                Vec3::new(-40.42636, 17.429008, -82.43467),
                Vec3::new(-40.526676, 17.39709, -82.402756),
                Vec3::new(-40.568623, 17.445776, -82.45143),
            ],
            0,
        );
        let b = tri(
            [
                Vec3::new(-40.526676, 17.39709, -82.402756),
                Vec3::new(-40.561783, 17.43503, -82.44069),
                Vec3::new(-40.55909, 17.434713, -82.44038),
            ],
            0,
        );
        assert!(overlap(&a, &b) < 1e-6, "{} cm²", overlap(&a, &b) * 1e4);
    }

    #[test]
    fn coplanar_triangles_of_two_meshes_are_found_and_apart_ones_are_not() {
        let mut drawn = Drawn {
            tris: vec![
                tri([Vec3::ZERO, Vec3::X, Vec3::Y], 0),
                tri(
                    [
                        Vec3::new(0.1, 0.1, 0.0),
                        Vec3::new(0.9, 0.1, 0.0),
                        Vec3::new(0.1, 0.9, 0.0),
                    ],
                    1,
                ),
            ],
            meshes: vec!["a".into(), "b".into()],
            kinds: vec![None, None],
        };
        let mut found = Vec::new();
        coplanar(&drawn, Detail::Near, &mut found);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].fault, Fault::Coplanar);
        // Pressed against a face turned to meet them over where they overlap, as two
        // pieces' undersides on a footing's top, they are never seen.
        let mut pressed = Drawn {
            tris: drawn.tris.clone(),
            meshes: vec!["a".into(), "b".into(), "c".into()],
            kinds: vec![None, None, None],
        };
        pressed.tris.push(tri(
            [
                Vec3::new(-1.0, -1.0, 0.0),
                Vec3::new(-1.0, 3.0, 0.0),
                Vec3::new(3.0, -1.0, 0.0),
            ],
            2,
        ));
        found.clear();
        coplanar(&pressed, Detail::Near, &mut found);
        assert!(found.is_empty(), "{found:?}");
        // Let a few millimetres into stone whose face that is, the same; into anything
        // else, whose face may be a sheet seen from behind, not.
        for p in &mut pressed.tris[2].p {
            p.z -= 0.005;
        }
        pressed.tris[2].d = pressed.tris[2].n.dot(pressed.tris[2].p[0]);
        found.clear();
        coplanar(&pressed, Detail::Near, &mut found);
        assert_eq!(found.len(), 1, "{found:?}");
        pressed.kinds[2] = Some((Kind(2), Material::Masonry));
        found.clear();
        coplanar(&pressed, Detail::Near, &mut found);
        assert!(found.is_empty(), "{found:?}");
        // A centimetre out of the plane, they do not fight.
        for p in &mut drawn.tris[1].p {
            p.z += 0.01;
        }
        drawn.tris[1].d += 0.01;
        found.clear();
        coplanar(&drawn, Detail::Near, &mut found);
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_line_meets_a_triangle_in_its_way() {
        let t = tri(
            [
                Vec3::new(-1.0, -1.0, 2.0),
                Vec3::new(3.0, -1.0, 2.0),
                Vec3::new(-1.0, 3.0, 2.0),
            ],
            0,
        );
        assert!((meets(Vec3::ZERO, Vec3::Z, &t).unwrap() - 2.0).abs() < 1e-5);
        assert!(meets(Vec3::new(5.0, 5.0, 0.0), Vec3::Z, &t).is_none());
        let cells = cells_along(Vec3::new(0.25, 0.25, 0.25), Vec3::X, 1.0);
        assert_eq!(cells, vec![[0, 0, 0], [1, 0, 0], [2, 0, 0]]);
    }
}
