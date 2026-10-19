//! A framed kind's frame (D236, docs/building-kits.md): wattle and daub, and later plaster or
//! brick nogging, are a block kind's cubes as the infill, drawn as their surface (D197), with
//! a frame of squared timber laid over them by rule, as a carpenter frames a wall and the
//! panels are filled after. The kit's `framing` says the frame's sizes and spacing; nothing
//! about a kind of frame is code.
//!
//! **Faces.** Every level face of the kind's whole cubes open to the air is a cell of a
//! face, gathered by the plane it lies in. Along each face, row by row:
//!
//! - a **sill plate** along its foot and a **plate** along its top, and a doubled **girt**
//!   where one storey of it stands on another, continuous, the posts standing between;
//! - **posts** at every `bay` cubes along the grid, but none within a metre of a row's end,
//!   so a post stands under and over an opening as a stud does, and an **end post** where a
//!   face ends against another kind or in an inside corner;
//! - **rails** at the kit's heights in each storey, between the posts: one in a storey's
//!   lower half under its line, as a window's sill beam is, one in its upper half over it,
//!   as a head beam is, so they run on into an opening's frame;
//! - **braces** in the panels at each end of a face, the lowest and the highest, leaning in
//!   to the post at the end.
//!
//! **Corners.** A square **corner post** up every outside corner, standing out of both faces.
//!
//! **Gables.** A gable end, the wall carried up into its roof (D241), is framed by the same
//! rule as the wall under it, as a triangle, not as the steps of its cubes: its **studs**
//! stand on the girt where the wall's posts stand, every `bay` cubes along the grid, each
//! cut off under the roof; its **rails** at the kit's heights run between them out to the
//! roof; a **brace** at each end leans in to it, from the girt up to the outer stud, under
//! the roof and along it; and nothing is laid along its steps, under the roof, or up them.
//!
//! **Gables.** A gable end over the wall's plate (`roofs::gables`) is framed by the same rule
//! as the wall under it: its **studs** stand on the plate where the wall's posts stand, every
//! `bay` cubes along the grid, each cut off under the roof; its **rails** at the kit's heights
//! run between them as far as the roof lets them; and a **brace** in each end's first whole
//! panel leans in to the end, from the next stud's foot up to the outer stud.
//!
//! **Openings.** An opening's frame goes through the wall, lining its reveals: a **jamb** up
//! each side where the wall stands beside it, a **head** over it and a **sill** under it
//! where the wall goes on, each running over the jambs. Any hole is framed, a cut cube's as a
//! doorway's.
//!
//! Every member stands `proud` of the infill's face and reaches `deep` into it, so the
//! panels read set back in the frame; where members meet, one ends at the other's side, so
//! no two faces lie in one plane. Each is a `Piece::Beam`, drawn by `timber` as a squared
//! member of the kit's `framing.material`.

use super::roles::Dir;
use super::{Piece, Place, Scope, drawn_fill, hash, ticks};
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use terra_build::kit::{Braces, Framing, Kit};
use terra_build::{Base, CELL_M, Fill, STOREY_CUBES, Shape};

/// How far round a scope a frame is worked out, level and up and down, in cubes: further
/// than a face's members reach from the cube that owns them (a plate is cut every
/// [`LONGEST`] cubes, a post a storey), and its posts' spacing looks.
const REACH: i32 = 14;
const REACH_UP: i32 = STOREY_CUBES + 1;
/// The longest a plate or rail runs before it is jointed, in cubes, at every multiple of it
/// along the grid: four metres, as a log is (D174).
const LONGEST: i32 = 8;
/// How far an opening's frame is set into it, so its faces are not in its reveal's plane.
const INTO_M: f32 = 0.015;
/// How far a brace is set back from the members it runs into, so its ends are behind them.
const BRACE_BACK_M: f32 = 0.012;
/// How far out of the frame's middle depth a brace is set, one way for those leaning one
/// way and the other for the rest, so two crossing in a panel are not in one plane.
const BRACE_CROSS_M: f32 = 0.003;

/// How a run of a face's cells ends along a row: in the open, round an outside corner; in an
/// inside corner, the cube beside covered; at an opening; against another kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    Outside,
    Inside,
    Opening,
    Meet,
}

/// A base's cubes of one framed kind, asked about.
struct Framer<'a> {
    base: &'a Base,
    kit: &'a Kit,
    framing: &'a Framing,
}

impl Framer<'_> {
    /// A whole solid cube of the kind: what the infill is.
    fn standing(&self, at: IVec3) -> bool {
        self.base.cube(at).is_some_and(|cube| {
            cube.kind == self.kit.kind()
                && drawn_fill(&cube) == Fill::Solid
                && cube.shape == Shape::Whole
        })
    }

    /// An opening's cube of the kind: cut open, glazed, or being cut.
    fn hole(&self, at: IVec3) -> bool {
        self.base
            .cube(at)
            .is_some_and(|cube| cube.kind == self.kit.kind() && drawn_fill(&cube) != Fill::Solid)
    }

    /// A roof's cube (D241).
    fn roof(&self, at: IVec3) -> bool {
        self.base.cube(at).is_some_and(|cube| cube.kind.is_roof())
    }

    /// Whether a standing cube is in a gable: its column, up from it within its storey,
    /// stops short of the storey's top under a roof.
    fn gabled(&self, at: IVec3) -> bool {
        let band_top = at.y.div_euclid(STOREY_CUBES) * STOREY_CUBES + STOREY_CUBES - 1;
        let mut top = at;
        while top.y < band_top && self.standing(top + IVec3::Y) {
            top += IVec3::Y;
        }
        top.y < band_top && self.roof(top + IVec3::Y)
    }

    /// How a run ends at the cell beyond it, `at`.
    fn end(&self, at: IVec3) -> End {
        match self.base.cube(at) {
            None => End::Outside,
            Some(_) if self.hole(at) => End::Opening,
            Some(_) if self.standing(at) => End::Inside,
            Some(_) => End::Meet,
        }
    }
}

/// A plane of faces: the way they look, and which layer of cubes along that way they are on.
type Plane = (Dir, i32);

/// The cube a face's cell is of: `u` along the plane, `y` up.
fn cube_of((dir, layer): Plane, u: i32, y: i32) -> IVec3 {
    if dir.axis() == 0 {
        IVec3::new(layer, y, u)
    } else {
        IVec3::new(u, y, layer)
    }
}

/// The axis a plane's cells run along: Z for a face looking east or west, else X.
fn along(dir: Dir) -> Vec3 {
    if dir.axis() == 0 { Vec3::Z } else { Vec3::X }
}

/// Lays the frame over a framed kind's cubes in a scope.
pub(crate) fn lay(
    base: &Base,
    kit: &Kit,
    framing: &Framing,
    scope: Scope,
    pieces: &mut Vec<Piece>,
) {
    let framer = Framer { base, kit, framing };
    let wide = Scope {
        low: scope.low - IVec3::new(REACH, REACH_UP, REACH),
        high: scope.high + IVec3::new(REACH, REACH_UP, REACH),
    };
    let cubes: Vec<IVec3> = wide
        .cubes(base, None)
        .into_iter()
        .filter(|(_, cube)| cube.kind == kit.kind())
        .map(|(at, _)| at)
        .collect();
    // Every face open to the air, by plane, as (y, u).
    let mut faces: BTreeMap<Plane, BTreeSet<(i32, i32)>> = BTreeMap::new();
    for &at in &cubes {
        if !framer.standing(at) {
            continue;
        }
        for dir in Dir::LEVEL {
            if base.cube(at + dir.step()).is_none() {
                let layer = at[dir.axis()];
                let u = if dir.axis() == 0 { at.z } else { at.x };
                faces.entry((dir, layer)).or_default().insert((at.y, u));
            }
        }
    }
    let mut out = Out { pieces: Vec::new() };
    for (&plane, cells) in &faces {
        face(&framer, plane, cells, &mut out);
    }
    corners(&framer, &cubes, &mut out);
    openings(&framer, &cubes, &mut out);
    posts(&framer, &cubes, &mut out);
    pieces.extend(out.pieces);
}

/// The members laid, and how.
struct Out {
    pieces: Vec<Piece>,
}

impl Out {
    /// A member from one point to another along its middle, so wide and deep, its depth
    /// along `face`'s way.
    fn beam(&mut self, from: Vec3, to: Vec3, face: Dir, wide: f32, deep: f32) {
        if from.distance(to) < 0.02 {
            return;
        }
        let (from, to) = (Place::near(from), Place::near(to));
        let seed = hash::of(&[
            from.0[0] as u32,
            from.0[1] as u32,
            from.0[2] as u32,
            to.0[0] as u32,
            to.0[1] as u32,
            to.0[2] as u32,
        ]);
        self.pieces.push(Piece::Beam {
            from,
            to,
            face,
            wide: ticks(wide),
            deep: ticks(deep),
            seed,
        });
    }
}

/// A run of a face's row: its row, its first and last cells, and how its low and high ends
/// end.
type Run = (i32, i32, i32, End, End);

/// A post standing across rows: the boundary it stands on, which side of it (-1 inside a
/// row's low end, 1 inside its high end, 0 across it), and how far along the face it reaches.
type Crossing = (i32, i32, f32, f32);

/// One plane of a framed kind's faces, worked out once for its members to be laid on it.
struct Face<'a> {
    framer: &'a Framer<'a>,
    f: &'a Framing,
    plane: Plane,
    cells: &'a BTreeSet<(i32, i32)>,
    /// The runs of each row.
    rows: Vec<Run>,
    /// The storeys of this face that are gables, by the layer each starts on: one with a
    /// column stopping under a roof short of the storey's top.
    gables: BTreeSet<i32>,
}

impl<'a> Face<'a> {
    fn new(framer: &'a Framer<'a>, plane: Plane, cells: &'a BTreeSet<(i32, i32)>) -> Self {
        let mut face = Face {
            framer,
            f: framer.framing,
            plane,
            cells,
            rows: Vec::new(),
            gables: BTreeSet::new(),
        };
        // The runs of each row: (y, first, last, how its low and high ends end).
        for &(y, u) in cells {
            match face.rows.last_mut() {
                Some(run) if run.0 == y && run.2 + 1 == u => run.2 = u,
                _ => face.rows.push((y, u, u, End::Meet, End::Meet)),
            }
        }
        for run in &mut face.rows {
            run.3 = framer.end(cube_of(plane, run.1 - 1, run.0));
            run.4 = framer.end(cube_of(plane, run.2 + 1, run.0));
        }
        face.gables = cells
            .iter()
            .filter(|&&(y, u)| !face.has(y + 1, u) && framer.gabled(cube_of(plane, u, y)))
            .map(|&(y, _)| y.div_euclid(STOREY_CUBES) * STOREY_CUBES)
            .collect();
        // A gable's row ending under its roof ends in the open, with no post.
        let gables = face.gables.clone();
        let in_gable = |y: i32| gables.contains(&(y.div_euclid(STOREY_CUBES) * STOREY_CUBES));
        for run in &mut face.rows {
            if in_gable(run.0) {
                for (end, u) in [(&mut run.3, run.1 - 1), (&mut run.4, run.2 + 1)] {
                    if framer.roof(cube_of(plane, u, run.0)) {
                        *end = End::Outside;
                    }
                }
            }
        }
        face
    }

    fn has(&self, y: i32, u: i32) -> bool {
        self.cells.contains(&(y, u))
    }

    /// Where on the face a point is: so far along it and so high, its middle as deep as a
    /// member's.
    fn at(&self, u: f32, y: f32) -> Vec3 {
        let (dir, layer) = self.plane;
        let face_m = (layer + i32::from(dir.sign() > 0)) as f32 * CELL_M;
        let normal = dir.step().as_vec3();
        let mut p = along(dir) * u + Vec3::Y * y;
        p[dir.axis()] = face_m;
        p + normal * (self.f.proud - self.f.deep * 0.5)
    }

    /// How far in from a row's end at a boundary its members stop, by how it ends: at a
    /// corner post's side, at an opening's frame's side, at an end post's side or at the end.
    fn inset(&self, end: End, post: bool) -> f32 {
        let (f, w) = (self.f, self.f.wide);
        match end {
            End::Outside => f.corner - f.proud,
            End::Opening => w - INTO_M,
            End::Inside | End::Meet if post => w,
            End::Inside | End::Meet => 0.0,
        }
    }

    fn in_gable(&self, y: i32) -> bool {
        self.gables
            .contains(&(y.div_euclid(STOREY_CUBES) * STOREY_CUBES))
    }

    fn row_of(&self, y: i32, u: i32) -> Option<Run> {
        self.rows
            .iter()
            .find(|run| run.0 == y && run.1 <= u && u <= run.2)
            .copied()
    }

    /// How high the roof's underside is over a point along the face, from a row up: over the
    /// top of the column there, if a roof's cube is over it, or the roof's cube on the row.
    fn roof_at(&self, u: f32, y: i32) -> Option<f32> {
        let column = libm::floorf(u / CELL_M) as i32;
        let mut top = y;
        if self.has(top, column) {
            while self.has(top + 1, column) {
                top += 1;
            }
            top += 1;
        }
        let over = cube_of(self.plane, column, top);
        let cube = self
            .framer
            .base
            .cube(over)
            .filter(|cube| cube.kind.is_roof())?;
        let under = super::roofs::underside(over, cube)?;
        Some(super::roofs::gable_cut(over, &under, self.at(u, 0.0)))
    }

    /// How far a gable's member at a height may run out along it toward the roof from a
    /// point, the way given: until the roof comes down to it.
    fn out_to_roof(&self, from: f32, toward: f32, height: f32, y: i32) -> f32 {
        let mut u = from;
        while (u - from).abs() < CELL_M * 3.0
            && self
                .roof_at(u + toward * 0.02, y)
                .is_some_and(|h| h > height + 0.01)
        {
            u += toward * 0.02;
        }
        u
    }

    /// A rail's line, and whether it sits under it.
    fn rail(&self, line: i32) -> Option<bool> {
        let r = line.rem_euclid(STOREY_CUBES);
        (r != 0 && self.f.rails.contains(&r)).then_some(r * 2 < STOREY_CUBES)
    }

    /// What lies across a boundary between rows at a cell, into the row under the line: a
    /// member over or under the line, as far into that row.
    fn below_reach(&self, line: i32, u: i32) -> f32 {
        let w = self.f.wide;
        let (under, over) = (self.has(line - 1, u), self.has(line, u));
        if !under {
            return 0.0;
        }
        if !over || line.rem_euclid(STOREY_CUBES) == 0 {
            return w;
        }
        match self.rail(line) {
            Some(true) => w,
            _ => 0.0,
        }
    }

    /// The same, into the row over the line.
    fn above_reach(&self, line: i32, u: i32) -> f32 {
        let w = self.f.wide;
        let (under, over) = (self.has(line - 1, u), self.has(line, u));
        if !over {
            return 0.0;
        }
        let beyond = self.framer.hole(cube_of(self.plane, u, line - 1));
        if !under || line.rem_euclid(STOREY_CUBES) == 0 {
            return if beyond { w - INTO_M } else { w };
        }
        match self.rail(line) {
            Some(false) => w,
            _ => 0.0,
        }
    }

    /// A member from one point on the face to another, as wide as the frame's members.
    fn beam(&self, (from, to): (Vec3, Vec3), wide: f32, out: &mut Out) {
        out.beam(from, to, self.plane.0, wide, self.f.deep);
    }

    /// A member along a boundary between rows, from cell `first` to `last`, its run's ends as
    /// given, so high: jointed every LONGEST cubes along the grid; `post` says whether an end
    /// post stands at an inside or meeting end, which rails stop at.
    fn level(
        &self,
        line_y: f32,
        (first, last): (i32, i32),
        ends: (End, End),
        post: bool,
        out: &mut Out,
    ) {
        let w = self.f.wide;
        let start = first as f32 * CELL_M + self.inset(ends.0, post);
        let end = (last + 1) as f32 * CELL_M - self.inset(ends.1, post);
        let mut cut = start;
        let joints = (first.div_euclid(LONGEST) + 1..=last.div_euclid(LONGEST))
            .map(|n| (n * LONGEST) as f32 * CELL_M)
            .filter(|&joint| joint > start + 0.3 && joint < end - 0.3);
        for joint in joints.chain(std::iter::once(end)) {
            self.beam((self.at(cut, line_y), self.at(joint, line_y)), w, out);
            cut = joint;
        }
    }
}

/// A face's members: its plates, girts, posts, rails and braces.
fn face(framer: &Framer, plane: Plane, cells: &BTreeSet<(i32, i32)>, out: &mut Out) {
    let face = Face::new(framer, plane, cells);
    let (crossing, studs) = face_posts(&face, out);
    let lines: BTreeSet<i32> = cells.iter().flat_map(|&(y, _)| [y, y + 1]).collect();
    plates(&face, &lines, out);
    rails(&face, &lines, (&crossing, &studs), out);
    if face.f.braces == Braces::Ends {
        let walls: Vec<_> = face
            .rows
            .iter()
            .copied()
            .filter(|run| !face.in_gable(run.0))
            .collect();
        braces(
            framer,
            plane,
            cells,
            &walls,
            &|line| face.rail(line),
            &|u, y| face.at(u, y),
            out,
        );
        gable_braces(&face, &studs, out);
    }
}

/// A face's posts, each up a run of rows a storey at most, between what lies across their
/// ends: at each row's ends inside the wall, and every bay; a gable's studs where the wall's
/// posts stand. The posts standing across each row, and a gable's studs by the storey they
/// stand on, with where each is along the face and how high it reaches.
#[allow(clippy::type_complexity)]
fn face_posts(
    face: &Face,
    out: &mut Out,
) -> (BTreeMap<i32, Vec<Crossing>>, BTreeMap<i32, Vec<(f32, f32)>>) {
    let (f, w) = (face.f, face.f.wide);
    // Posts on each row, by where they stand (a boundary along the face, and which side of it
    // they are on: -1 inside a row's low end, 1 inside its high end, 0 across it), as rows.
    let mut post_rows: BTreeMap<(i32, i32), BTreeSet<i32>> = BTreeMap::new();
    for &(y, first, last, low, high) in &face.rows {
        // A face a cube wide, a wall's end or a pier, is its corner posts' and its
        // openings' frames alone.
        if first == last {
            continue;
        }
        // A gable's studs stand where the wall's posts do, on its widest row, wherever the
        // row holds both cubes either side of one.
        if face.in_gable(y) {
            let band = y.div_euclid(STOREY_CUBES) * STOREY_CUBES;
            let under = (1..=STOREY_CUBES).find_map(|d| face.row_of(band - d, first));
            if let Some((_, first0, last0, ..)) = under.or_else(|| face.row_of(band, first)) {
                for b in first0 + 2..=last0 - 1 {
                    if b.rem_euclid(f.bay.max(1)) == 0 && face.has(y, b - 1) && face.has(y, b) {
                        post_rows.entry((b, 0)).or_default().insert(y);
                    }
                }
            }
            continue;
        }
        if matches!(low, End::Inside | End::Meet) {
            post_rows.entry((first, -1)).or_default().insert(y);
        }
        if matches!(high, End::Inside | End::Meet) {
            post_rows.entry((last + 1, 1)).or_default().insert(y);
        }
        for b in first + 2..=last - 1 {
            if b.rem_euclid(f.bay.max(1)) == 0 {
                post_rows.entry((b, 0)).or_default().insert(y);
            }
        }
    }
    let mut crossing: BTreeMap<i32, Vec<Crossing>> = BTreeMap::new();
    let mut studs: BTreeMap<i32, Vec<(f32, f32)>> = BTreeMap::new();
    for (&(b, side), ys) in &post_rows {
        let u_cell = if side > 0 { b - 1 } else { b };
        let mut spans: Vec<(i32, i32)> = Vec::new();
        for &y in ys {
            match spans.last_mut() {
                Some(span) if span.1 + 1 == y && y.rem_euclid(STOREY_CUBES) != 0 => span.1 = y,
                _ => spans.push((y, y)),
            }
        }
        let (u_low, u_high) = match side {
            -1 => (b as f32 * CELL_M, b as f32 * CELL_M + w),
            1 => (b as f32 * CELL_M - w, b as f32 * CELL_M),
            _ => (b as f32 * CELL_M - w * 0.5, b as f32 * CELL_M + w * 0.5),
        };
        let u_mid = (u_low + u_high) * 0.5;
        for (y0, y1) in spans {
            let foot = y0 as f32 * CELL_M
                + face
                    .above_reach(y0, u_cell)
                    .max(face.above_reach(y0, b - 1));
            let mut top = (y1 + 1) as f32 * CELL_M
                - face
                    .below_reach(y1 + 1, u_cell)
                    .max(face.below_reach(y1 + 1, b - 1));
            // A gable's stud cut off under the roof.
            let roofed = [u_low, u_high]
                .into_iter()
                .filter_map(|u| face.roof_at(u, y1))
                .reduce(f32::min);
            if face.in_gable(y1)
                && let Some(roof) = roofed
            {
                top = roof - 0.01;
            }
            if top - foot > 0.05 {
                face.beam((face.at(u_mid, foot), face.at(u_mid, top)), w, out);
                if face.in_gable(y0) && y0.rem_euclid(STOREY_CUBES) == 0 && side == 0 {
                    studs.entry(y0).or_default().push((u_mid, top));
                }
            }
            for y in y0..=y1 {
                crossing
                    .entry(y)
                    .or_default()
                    .push((b, side, u_low, u_high));
            }
        }
    }
    (crossing, studs)
}

/// Plates and girts: along the foot of each row's cells not over another row, the top of
/// those under none, and doubled where a storey stands on another. Not over or under an
/// opening, whose frame is there.
fn plates(face: &Face, lines: &BTreeSet<i32>, out: &mut Out) {
    let (framer, plane, w) = (face.framer, face.plane, face.f.wide);
    for &line in lines {
        // Cells along the line with a plate: (u, its middle's height).
        let mut plated: Vec<(i32, f32, End, End)> = Vec::new();
        for &(y, first, last, low, high) in &face.rows {
            if (y != line && y + 1 != line) || first == last {
                continue;
            }
            for u in first..=last {
                let (under, over) = (face.has(line - 1, u), face.has(line, u));
                let middle = if y == line && !under {
                    // A sill plate, unless an opening is under it.
                    if framer.hole(cube_of(plane, u, line - 1)) {
                        continue;
                    }
                    line as f32 * CELL_M + w * 0.5
                } else if y + 1 == line && !over {
                    if framer.hole(cube_of(plane, u, line))
                        || (face.in_gable(y)
                            && line.rem_euclid(STOREY_CUBES) != 0
                            && framer.roof(cube_of(plane, u, line)))
                    {
                        continue;
                    }
                    // Under a gable the wall's plate is its girt, across the wall's width.
                    if face.in_gable(line)
                        && line.rem_euclid(STOREY_CUBES) == 0
                        && framer.roof(cube_of(plane, u, line))
                    {
                        line as f32 * CELL_M
                    } else {
                        line as f32 * CELL_M - w * 0.5
                    }
                } else if y == line && under && line.rem_euclid(STOREY_CUBES) == 0 {
                    // A girt, laid once, from the row over it.
                    line as f32 * CELL_M
                } else {
                    continue;
                };
                // Its ends as its row's, or, where an opening's frame stops it inside the
                // row, at that frame's side.
                let end_low = if u == first { low } else { End::Opening };
                let end_high = if u == last { high } else { End::Opening };
                plated.push((u, middle, end_low, end_high));
            }
        }
        plated.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        let mut i = 0;
        while i < plated.len() {
            let (first, middle, low, _) = plated[i];
            let mut j = i;
            while j + 1 < plated.len()
                && plated[j + 1].1 == middle
                && plated[j + 1].0 == plated[j].0 + 1
            {
                j += 1;
            }
            let (last, _, _, high) = plated[j];
            let girt = (middle - line as f32 * CELL_M).abs() < 1e-4;
            if girt {
                let start = first as f32 * CELL_M + face.inset(low, false);
                let end = (last + 1) as f32 * CELL_M - face.inset(high, false);
                face.beam((face.at(start, middle), face.at(end, middle)), w * 2.0, out);
            } else {
                face.level(middle, (first, last), (low, high), false, out);
            }
            i = j + 1;
        }
    }
}

/// Rails: across a run of cells with a row either side of their line, between the posts.
#[allow(clippy::type_complexity)]
fn rails(
    face: &Face,
    lines: &BTreeSet<i32>,
    (crossing, studs): (
        &BTreeMap<i32, Vec<Crossing>>,
        &BTreeMap<i32, Vec<(f32, f32)>>,
    ),
    out: &mut Out,
) {
    let w = face.f.wide;
    for &line in lines {
        let Some(under_line) = face.rail(line) else {
            continue;
        };
        let middle = line as f32 * CELL_M + if under_line { -w * 0.5 } else { w * 0.5 };
        let row = if under_line { line - 1 } else { line };
        let us: Vec<i32> = face
            .cells
            .iter()
            .filter(|&&(y, u)| {
                // A gable's along its own row, which the roof stops, not the other.
                let across = if face.in_gable(row) {
                    y == row
                } else {
                    y == line && face.has(line - 1, u)
                };
                across && face.row_of(row, u).is_some_and(|run| run.1 < run.2)
            })
            .map(|&(_, u)| u)
            .collect();
        let mut i = 0;
        while i < us.len() {
            let mut j = i;
            while j + 1 < us.len() && us[j + 1] == us[j] + 1 {
                j += 1;
            }
            let (first, last) = (us[i], us[j]);
            // How the rail's own row ends at each end: as that row's run, or, where the row
            // goes on past (the other row stops at an opening), at that opening's frame.
            let end_of = |u: i32, high: bool| -> End {
                match face.row_of(row, u) {
                    Some(run) if high && run.2 == u => run.4,
                    Some(run) if !high && run.1 == u => run.3,
                    _ => End::Opening,
                }
            };
            let mut start = first as f32 * CELL_M + face.inset(end_of(first, false), true);
            let mut end = (last + 1) as f32 * CELL_M - face.inset(end_of(last, true), true);
            // A gable's runs out to its roof.
            if face.in_gable(row) {
                let top = middle + w * 0.5;
                if end_of(first, false) == End::Outside {
                    start = face.out_to_roof(first as f32 * CELL_M, -1.0, top, row);
                }
                if end_of(last, true) == End::Outside {
                    end = face.out_to_roof((last + 1) as f32 * CELL_M, 1.0, top, row);
                }
            }
            // Split at every post standing across its line, in the rows either side.
            let across = |y: i32| -> Vec<(i32, f32, f32)> {
                crossing
                    .get(&y)
                    .into_iter()
                    .flatten()
                    .filter(|&&(b, side, ..)| side == 0 && b > first && b <= last)
                    .map(|&(b, _, low, high)| (b, low, high))
                    .collect()
            };
            let other = across(if under_line { line } else { line - 1 });
            let mut stops: Vec<(f32, f32)> = if face.in_gable(row) {
                // A gable's studs that reach it, cut off under the roof as they are.
                let band = row.div_euclid(STOREY_CUBES) * STOREY_CUBES;
                studs
                    .get(&band)
                    .into_iter()
                    .flatten()
                    .filter(|&&(u, top)| top > middle - w * 0.5 && u > start && u < end)
                    .map(|&(u, _)| (u - w * 0.5, u + w * 0.5))
                    .collect()
            } else {
                across(row)
                    .into_iter()
                    .filter(|post| other.contains(post))
                    .map(|(_, low, high)| (low, high))
                    .collect()
            };
            stops.sort_by(|a, b| a.0.total_cmp(&b.0));
            // No sliver of a rail between a gable's stud and its roof.
            let shortest = if face.in_gable(row) { 0.15 } else { 0.0 };
            stops.push((end, end));
            for (low, high) in stops {
                if low - start > shortest {
                    face.beam((face.at(start, middle), face.at(low, middle)), w, out);
                }
                start = high;
            }
            i = j + 1;
        }
    }
}

/// A gable's braces: at each end, from the girt up to the outer stud, along the roof and
/// under it, and under its lowest rail.
fn gable_braces(face: &Face, studs: &BTreeMap<i32, Vec<(f32, f32)>>, out: &mut Out) {
    let (f, w, dir) = (face.f, face.f.wide, face.plane.0);
    for (&foot, standing) in studs {
        let bottom = foot as f32 * CELL_M + w;
        let rail_under = f
            .rails
            .iter()
            .filter(|&&r| r * 2 < STOREY_CUBES)
            .map(|&r| (foot + r) as f32 * CELL_M - w)
            .reduce(f32::min);
        let low = standing
            .iter()
            .copied()
            .reduce(|a, b| if b.0 < a.0 { b } else { a });
        let high = standing
            .iter()
            .copied()
            .reduce(|a, b| if b.0 > a.0 { b } else { a });
        for (stud, toward) in [(low, -1.0f32), (high, 1.0)] {
            let Some((u, stud_top)) = stud else {
                continue;
            };
            let side = u + toward * w * 0.5;
            let eaves = face.out_to_roof(side, toward, bottom, foot);
            let run = (eaves - side).abs();
            let rise = stud_top - bottom;
            if run < 0.5 || rise * 0.4 < f.brace {
                continue;
            }
            let mut up = rise * 0.6;
            if let Some(rail) = rail_under {
                up = up.min(rail - bottom);
            }
            if up < 0.25 {
                continue;
            }
            let along = side + toward * run * up / rise;
            // Thinner than what it runs into both ways, and as proud of the infill as most
            // of its depth (as `braces`).
            let shift = dir.step().as_vec3() * (BRACE_CROSS_M * toward.signum());
            out.beam(
                face.at(along, bottom) + shift,
                face.at(side, bottom + up) + shift,
                dir,
                f.brace,
                f.deep - BRACE_BACK_M * 2.0,
            );
        }
    }
}

/// Braces in the lowest and highest panels at each end of a face, leaning in to the end.
#[allow(clippy::too_many_arguments)]
fn braces(
    framer: &Framer,
    plane: Plane,
    cells: &BTreeSet<(i32, i32)>,
    rows: &[(i32, i32, i32, End, End)],
    rail: &dyn Fn(i32) -> Option<bool>,
    at: &dyn Fn(f32, f32) -> Vec3,
    out: &mut Out,
) {
    let f = framer.framing;
    let w = f.wide;
    let has = |y: i32, u: i32| cells.contains(&(y, u));
    let bay = f.bay.max(1);
    for &(y, first, last, low, high) in rows {
        for (end, toward) in [(low, -1), (high, 1)] {
            if end == End::Opening {
                continue;
            }
            // The end's column and the cell beyond it.
            let column = if toward < 0 { first } else { last };
            // The bay: from the end to the first post in from it, or the other end.
            let span: Vec<i32> = if toward < 0 {
                let post = (first + 2..=last - 1).find(|b| b.rem_euclid(bay) == 0);
                (first..post.unwrap_or(last + 1)).collect()
            } else {
                let post = (first + 2..=last - 1)
                    .rev()
                    .find(|b| b.rem_euclid(bay) == 0);
                (post.unwrap_or(first)..=last).collect()
            };
            if span.len() < 2 {
                continue;
            }
            let (s0, s1) = (span[0], span[span.len() - 1]);
            let inner_post = if toward < 0 { s1 < last } else { s0 > first };
            // The lowest panel, from this row as the foot of the face, up to its first
            // rail; the highest, from this row as the top, down to its last.
            let foot = (s0..=s1).all(|u| !has(y - 1, u));
            let top = (s0..=s1).all(|u| !has(y + 1, u));
            for lowest in [true, false] {
                if (lowest && !foot) || (!lowest && !top) {
                    continue;
                }
                let mut rows_in = vec![y];
                let mut line = None;
                for step in 1..STOREY_CUBES {
                    let next = if lowest { y + step } else { y - step };
                    let boundary = if lowest { next } else { next + 1 };
                    if rail(boundary).is_some() || boundary.rem_euclid(STOREY_CUBES) == 0 {
                        line = Some(boundary);
                        break;
                    }
                    rows_in.push(next);
                }
                let Some(line) = line else { continue };
                if rows_in.len() < 2
                    || !rows_in.iter().all(|&r| (s0..=s1).all(|u| has(r, u)))
                    || !rows_in.iter().all(|&r| {
                        let beyond = if toward < 0 { s0 - 1 } else { s1 + 1 };
                        !has(r, beyond) && (has(r, column))
                    })
                {
                    continue;
                }
                // Its far end, at the line, clear of what lies across it there.
                let line_m = line as f32 * CELL_M;
                let far = match rail(line) {
                    Some(true) if lowest => line_m - w,
                    Some(false) if !lowest => line_m + w,
                    _ if line.rem_euclid(STOREY_CUBES) == 0 && lowest => line_m - w,
                    _ if line.rem_euclid(STOREY_CUBES) == 0 => line_m + w,
                    _ => line_m,
                };
                let near = if lowest {
                    y as f32 * CELL_M + w
                } else {
                    (y + 1) as f32 * CELL_M - w
                };
                let inset = |end: End| match end {
                    End::Outside => f.corner - f.proud,
                    _ => w,
                };
                let end_u = if toward < 0 {
                    s0 as f32 * CELL_M + inset(end)
                } else {
                    (s1 + 1) as f32 * CELL_M - inset(end)
                };
                let inner_u = match (toward < 0, inner_post) {
                    (true, true) => (s1 + 1) as f32 * CELL_M - w * 0.5,
                    (false, true) => s0 as f32 * CELL_M + w * 0.5,
                    (true, false) => (s1 + 1) as f32 * CELL_M - inset(high),
                    (false, false) => s0 as f32 * CELL_M + inset(low),
                };
                if (end_u - inner_u).abs() < 0.4 {
                    continue;
                }
                // The brace from the inner post's foot to the end's top, or from the end's
                // top down to the inner post at the rail; thinner than they are both ways, so
                // its ends are inside them, and standing as proud of the infill as most of
                // its depth, so its face, however its timber waves, is never in the infill's.
                // Those leaning one way a hair out of the others' depth, where two cross.
                let shift = plane.0.step().as_vec3()
                    * (BRACE_CROSS_M * if toward < 0 { -1.0 } else { 1.0 });
                let (a, b) = (at(inner_u, near) + shift, at(end_u, far) + shift);
                out.beam(a, b, plane.0, f.brace, f.deep - BRACE_BACK_M * 2.0);
            }
        }
    }
}

/// A square post up every outside corner of the kind's cubes, standing out of both faces, a
/// storey at most.
fn corners(framer: &Framer, cubes: &[IVec3], out: &mut Out) {
    let f = framer.framing;
    let open = |at: IVec3| framer.base.cube(at).is_none();
    // Each corner, by its column and which ways its faces look, as rows.
    let mut columns: BTreeMap<([i32; 2], Dir, Dir), BTreeSet<i32>> = BTreeMap::new();
    for &at in cubes {
        if !framer.standing(at) {
            continue;
        }
        if framer.gabled(at) {
            continue;
        }
        for x in [Dir::East, Dir::West] {
            for z in [Dir::South, Dir::North] {
                if open(at + x.step()) && open(at + z.step()) {
                    let corner = [
                        at.x + i32::from(x.sign() > 0),
                        at.z + i32::from(z.sign() > 0),
                    ];
                    columns.entry((corner, x, z)).or_default().insert(at.y);
                }
            }
        }
    }
    for ((corner, x, z), ys) in columns {
        let middle = Vec3::new(corner[0] as f32, 0.0, corner[1] as f32) * CELL_M
            + (x.step().as_vec3() + z.step().as_vec3()) * (f.proud - f.corner * 0.5);
        let mut y = *ys.first().unwrap_or(&0);
        while let Some(&start) = ys.range(y..).next() {
            let mut end = start;
            while ys.contains(&(end + 1)) && (end + 1).rem_euclid(STOREY_CUBES) != 0 {
                end += 1;
            }
            out.beam(
                middle.with_y(start as f32 * CELL_M),
                middle.with_y((end + 1) as f32 * CELL_M),
                x,
                f.corner,
                f.corner,
            );
            y = end + 1;
        }
    }
}

/// Each opening's frame through the wall: jambs up its sides, a head over it and a sill under
/// it, where the wall stands beside, over and under it.
fn openings(framer: &Framer, cubes: &[IVec3], out: &mut Out) {
    let f = framer.framing;
    let w = f.wide;
    let deep = CELL_M + f.proud * 2.0;
    let ours = |at: IVec3| framer.standing(at) || framer.hole(at);
    // Each hole's wall runs east and west if the kind is either side of it that way.
    let runs_x = |at: IVec3| ours(at + IVec3::X) || ours(at - IVec3::X);
    // Jambs by (across, boundary along, which side the wall is) as rows; heads and sills by
    // (across, line, over) as cells along.
    let mut jambs: BTreeMap<(bool, i32, i32, i32), BTreeSet<i32>> = BTreeMap::new();
    let mut levels: BTreeMap<(bool, i32, i32, bool), BTreeSet<i32>> = BTreeMap::new();
    for &at in cubes {
        if !framer.hole(at) {
            continue;
        }
        let x = runs_x(at);
        let step = if x { IVec3::X } else { IVec3::Z };
        let (u, across) = if x { (at.x, at.z) } else { (at.z, at.x) };
        if framer.standing(at - step) {
            jambs.entry((x, across, u, -1)).or_default().insert(at.y);
        }
        if framer.standing(at + step) {
            jambs.entry((x, across, u + 1, 1)).or_default().insert(at.y);
        }
        if framer.standing(at + IVec3::Y) {
            levels
                .entry((x, across, at.y + 1, true))
                .or_default()
                .insert(u);
        }
        if framer.standing(at - IVec3::Y) {
            levels
                .entry((x, across, at.y, false))
                .or_default()
                .insert(u);
        }
    }
    let point = |x: bool, across: i32, u: f32, y: f32| {
        let middle = (across as f32 + 0.5) * CELL_M;
        if x {
            Vec3::new(u, y, middle)
        } else {
            Vec3::new(middle, y, u)
        }
    };
    let face = |x: bool| if x { Dir::South } else { Dir::East };
    let cell = |x: bool, across: i32, u: i32, y: i32| {
        if x {
            IVec3::new(u, y, across)
        } else {
            IVec3::new(across, y, u)
        }
    };
    for (&(x, across, b, side), ys) in &jambs {
        let mut spans: Vec<(i32, i32)> = Vec::new();
        for &y in ys {
            match spans.last_mut() {
                Some(span) if span.1 + 1 == y => span.1 = y,
                _ => spans.push((y, y)),
            }
        }
        // The hole's column and the wall's beside it; the jamb in the wall's, a little into
        // the hole.
        let (hole_u, wall_u) = if side < 0 { (b, b - 1) } else { (b - 1, b) };
        let u = if side < 0 {
            b as f32 * CELL_M - w * 0.5 + INTO_M
        } else {
            b as f32 * CELL_M + w * 0.5 - INTO_M
        };
        for (y0, y1) in spans {
            let foot = if framer.standing(cell(x, across, hole_u, y0 - 1)) {
                y0 as f32 * CELL_M + INTO_M
            } else if framer.hole(cell(x, across, wall_u, y0 - 1)) {
                y0 as f32 * CELL_M + w - INTO_M
            } else {
                y0 as f32 * CELL_M
            };
            let top = if framer.standing(cell(x, across, hole_u, y1 + 1)) {
                (y1 + 1) as f32 * CELL_M - INTO_M
            } else if framer.hole(cell(x, across, wall_u, y1 + 1)) {
                (y1 + 1) as f32 * CELL_M - w + INTO_M
            } else {
                (y1 + 1) as f32 * CELL_M
            };
            out.beam(
                point(x, across, u, foot),
                point(x, across, u, top),
                face(x),
                w,
                deep,
            );
        }
    }
    for (&(x, across, line, over), us) in &levels {
        let row = if over { line - 1 } else { line };
        let middle = line as f32 * CELL_M
            + if over {
                w * 0.5 - INTO_M
            } else {
                INTO_M - w * 0.5
            };
        let list: Vec<i32> = us.iter().copied().collect();
        let mut i = 0;
        while i < list.len() {
            let mut j = i;
            while j + 1 < list.len() && list[j + 1] == list[j] + 1 {
                j += 1;
            }
            let (first, last) = (list[i], list[j]);
            // Over each jamb at its ends.
            let reach = w - INTO_M;
            let start = first as f32 * CELL_M
                - if framer.standing(cell(x, across, first - 1, row)) {
                    reach
                } else {
                    0.0
                };
            let end = (last + 1) as f32 * CELL_M
                + if framer.standing(cell(x, across, last + 1, row)) {
                    reach
                } else {
                    0.0
                };
            out.beam(
                point(x, across, start, middle),
                point(x, across, end, middle),
                face(x),
                w,
                deep,
            );
            i = j + 1;
        }
    }
}

/// A post of the kind (a column of cubes cut as posts, D220) as one squared timber a storey
/// high at most, drawn in place of its infill.
fn posts(framer: &Framer, cubes: &[IVec3], out: &mut Out) {
    let side = framer.framing.corner * 1.5;
    let mut columns: BTreeMap<[i32; 2], BTreeSet<i32>> = BTreeMap::new();
    for &at in cubes {
        if framer.base.cube(at).is_some_and(|cube| {
            cube.kind == framer.kit.kind() && cube.fill == Fill::Solid && cube.shape == Shape::Post
        }) {
            columns.entry([at.x, at.z]).or_default().insert(at.y);
        }
    }
    for ([x, z], ys) in columns {
        let middle = (Vec3::new(x as f32, 0.0, z as f32) + Vec3::new(0.5, 0.0, 0.5)) * CELL_M;
        let mut rest: Vec<i32> = ys.into_iter().collect();
        while let Some(&start) = rest.first() {
            let mut end = start;
            while rest.contains(&(end + 1)) && (end + 1).rem_euclid(STOREY_CUBES) != 0 {
                end += 1;
            }
            out.beam(
                middle.with_y(start as f32 * CELL_M),
                middle.with_y((end + 1) as f32 * CELL_M),
                Dir::South,
                side,
                side,
            );
            rest.retain(|&y| y > end);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::TICKS;

    /// How far a tick is.
    const TICK_M: f32 = CELL_M / TICKS as f32;
    use terra_build::kit::Pattern;
    use terra_build::{BuildOp, Kind, Panel};

    fn kit() -> Kit {
        Kit {
            id: 5,
            pattern: Pattern::Masonry,
            framing: Some(Framing::default()),
            ..Kit::default()
        }
    }

    fn beams(pieces: &[Piece]) -> Vec<(Vec3, Vec3, f32)> {
        pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Beam { from, to, wide, .. } => {
                    Some((from.metres(), to.metres(), super::super::metres(wide)))
                }
                _ => None,
            })
            .collect()
    }

    fn wall(base: &mut Base, from: i32, to: i32, z: i32, kind: Kind) {
        for x in from..=to {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, z],
                panel: Some(Panel::Solid),
                kind,
            });
        }
    }

    fn laid(base: &Base) -> Vec<Piece> {
        let kit = kit();
        let mut pieces = Vec::new();
        lay(
            base,
            &kit,
            kit.framing.as_ref().unwrap(),
            Scope::ALL,
            &mut pieces,
        );
        pieces
    }

    #[test]
    fn braces_stand_proud_of_the_infill_and_two_crossing_are_not_in_one_plane() {
        // A wall along x, its faces at z 0 and 0.5: every brace's face stands clear of its
        // face's infill by more than its timber's waves, and those leaning either way are
        // at depths of their own.
        let mut base = Base::default();
        wall(&mut base, 0, 9, 0, kit().kind());
        let mut depths = std::collections::BTreeMap::<(bool, bool), f32>::new();
        for piece in laid(&base) {
            let Piece::Beam {
                from,
                to,
                face,
                deep,
                ..
            } = piece
            else {
                continue;
            };
            let (a, b) = (from.metres(), to.metres());
            if (a.y - b.y).abs() < 1e-3 || ((a.x - b.x).abs() < 1e-3 && (a.z - b.z).abs() < 1e-3) {
                continue;
            }
            let out = face.step().as_vec3();
            let infill = if out.z > 0.0 { 0.5 } else { 0.0 };
            let front = (a.z - infill) * out.z + super::super::metres(deep) * 0.5;
            assert!(front > 0.012, "a brace's face {front} m out of the infill");
            let rising = (b.y - a.y) * (b.x - a.x) > 0.0;
            depths.insert((out.z > 0.0, rising), a.z);
        }
        for south in [false, true] {
            let (a, b) = (depths[&(south, true)], depths[&(south, false)]);
            assert!((a - b).abs() > 0.004, "{a} {b}");
        }
    }

    #[test]
    fn a_free_wall_is_framed_with_corner_posts_plates_posts_rails_and_braces() {
        let mut base = Base::default();
        wall(&mut base, 0, 9, 0, kit().kind());
        let found = beams(&laid(&base));
        let upright =
            |(a, b, _): &(Vec3, Vec3, f32)| (a.x - b.x).abs() < 1e-3 && (a.z - b.z).abs() < 1e-3;
        let level = |(a, b, _): &(Vec3, Vec3, f32)| (a.y - b.y).abs() < 1e-3;
        // Four corner posts, a storey high, standing out of both faces.
        let corner_posts: Vec<_> = found
            .iter()
            .filter(|b| upright(b) && (b.2 - kit().framing.unwrap().corner).abs() < TICK_M * 2.0)
            .collect();
        assert_eq!(corner_posts.len(), 4, "{found:?}");
        for (a, b, _) in &corner_posts {
            assert!((b.y - a.y - 3.0).abs() < 0.01);
            assert!(a.x < 0.1 || a.x > 4.9, "at the wall's ends: {a}");
        }
        // On each long face: posts at x 1.5 and 3 (not 4.5, under a metre from the end), a
        // sill plate and a plate, each jointed at four metres, and two rails in three pieces
        // each, between the posts. Across each end only its corner posts.
        let posts = found
            .iter()
            .filter(|b| upright(b) && b.0.x > 0.5 && b.0.x < 4.5)
            .count();
        assert_eq!(posts, 2 * 2, "{found:?}");
        let long =
            |b: &&(Vec3, Vec3, f32)| (b.0.z - b.1.z).abs() < 1e-3 && (b.0.x - b.1.x).abs() > 0.2;
        let levels = found.iter().filter(|b| level(b) && long(b)).count();
        assert_eq!(levels, 2 * (2 * 2 + 2 * 3), "{found:?}");
        let ends = found.iter().filter(|b| level(b) && !long(b)).count();
        assert_eq!(ends, 0, "{found:?}");
        // Braces: the lowest and highest panels at each end of each face.
        let braces = found.iter().filter(|b| !upright(b) && !level(b)).count();
        assert_eq!(braces, 2 * 2 * 2, "{found:?}");
        // Every member within an eighth of a cube of the wall's cubes (D192).
        for (a, b, _) in &found {
            for p in [a, b] {
                assert!(
                    p.z > -0.0625 - 0.01 - 0.1 && p.z < 0.5 + 0.0625 + 0.1,
                    "{p}"
                );
            }
        }
    }

    #[test]
    fn an_opening_is_framed_through_the_wall_and_the_face_stops_at_its_frame() {
        let mut base = Base::default();
        wall(&mut base, 0, 9, 0, kit().kind());
        for x in 4..=5 {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 0],
                panel: Some(Panel::WINDOW),
                kind: kit().kind(),
            });
        }
        let pieces = laid(&base);
        let deep = super::super::ticks(CELL_M + kit().framing.unwrap().proud * 2.0);
        let through: Vec<_> = pieces
            .iter()
            .filter(|p| matches!(p, Piece::Beam { deep: d, .. } if *d == deep))
            .collect();
        // Two jambs, a head and a sill.
        assert_eq!(through.len(), 4, "{through:?}");
        // Nothing of the face's frame crosses the window's opening.
        for (a, b, _) in beams(&pieces) {
            let crosses = |p: Vec3| p.x > 2.05 && p.x < 2.95 && p.y > 1.05 && p.y < 1.95;
            assert!(!crosses((a + b) * 0.5), "{a} {b}");
        }
    }

    #[test]
    fn a_gable_is_framed_with_studs_rails_and_braces_under_its_roof_not_up_its_steps() {
        use terra_build::roofer::{self, Style};
        use terra_build::{Cube, Kind};
        let kind = kit().kind();
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 13],
            kind,
        });
        // The flat roof the room tool lays, off; a gable roof of cubes over it as the roofer
        // lays one, its gable ends carried up in the wall's kind.
        base.apply(BuildOp::Fill {
            min: [-50, 5, -50],
            max: [50, 5, 50],
            cube: None,
        });
        let footprint = (0..=9)
            .flat_map(|x| (0..=13).map(move |z| [x, z]))
            .collect();
        let plan = roofer::over(&footprint, 6, Style::Gable, Some(true));
        for (at, shape) in plan.roof {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube {
                    shape,
                    ..Cube::new(Kind::FIRST_ROOF, Fill::Solid)
                }),
            });
        }
        assert!(!plan.gables.is_empty());
        for at in plan.gables {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube::new(kind, Fill::Solid)),
            });
        }
        let found = beams(&laid(&base));
        // The west gable's members, on its outer face, over the wall's girt at 3 m.
        let gable: Vec<_> = found
            .iter()
            .filter(|(a, b, _)| a.x < 0.05 && b.x < 0.05 && a.y.min(b.y) > 3.05)
            .collect();
        let upright = |(a, b, _): &&(Vec3, Vec3, f32)| (a.z - b.z).abs() < 1e-3;
        let level = |(a, b, _): &&(Vec3, Vec3, f32)| (a.y - b.y).abs() < 1e-3;
        // Studs where the wall's posts are, every metre and a half, from the girt; no post up
        // the steps of its cubes, and no corner post.
        let studs: Vec<_> = gable.iter().filter(|b| upright(b) && !level(b)).collect();
        let at: Vec<f32> = studs.iter().map(|(a, ..)| a.z).collect();
        for z in [1.5, 3.0, 4.5, 6.0] {
            assert!(at.iter().any(|s| (s - z).abs() < 0.01), "{at:?}");
        }
        assert_eq!(studs.len(), 4, "{studs:?}");
        assert!(gable.iter().any(level), "a rail: {gable:?}");
        let braces = gable.iter().filter(|b| !upright(b) && !level(b)).count();
        assert_eq!(braces, 2, "{gable:?}");
        // The gable ends as a triangle, its studs cut under the roof: each taller toward the
        // ridge, in the middle of the 7 m end.
        let height = |z: f32| {
            studs
                .iter()
                .find(|(a, ..)| (a.z - z).abs() < 0.01)
                .map(|(a, b, _)| a.y.max(b.y))
                .unwrap()
        };
        assert!(height(1.5) < height(3.0) && height(6.0) < height(4.5));
        assert!(height(3.0) - height(1.5) > 0.5, "{studs:?}");
    }

    #[test]
    fn a_frame_is_laid_the_same_for_a_section_as_for_the_whole() {
        let mut base = Base::default();
        wall(&mut base, -12, 30, 3, kit().kind());
        let whole = laid(&base);
        let kit = kit();
        for section in [IVec3::new(-1, 0, 0), IVec3::ZERO, IVec3::new(1, 0, 0)] {
            let mut part = Vec::new();
            lay(
                &base,
                &kit,
                kit.framing.as_ref().unwrap(),
                Scope::section(section),
                &mut part,
            );
            let mine: BTreeSet<String> = part
                .iter()
                .filter(|p| p.section() == section)
                .map(|p| format!("{p:?}"))
                .collect();
            let all: BTreeSet<String> = whole
                .iter()
                .filter(|p| p.section() == section)
                .map(|p| format!("{p:?}"))
                .collect();
            assert_eq!(mine, all, "{section}");
        }
    }
}
