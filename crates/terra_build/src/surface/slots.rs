//! A block kind's cubes drawn as their surface, a slot of faces at a time
//! ([`draw_slots`]), in stages: what is round each cube ([`Neighbourhood`]), where each
//! face's points fall across it ([`Spacing`]), each face placed and cut into triangles
//! ([`draw_face`]), the normals smoothed across the faces that meet ([`smooth_normals`]),
//! and the faces put into their surfaces and kept for next time ([`put`]).

use super::*;

/// As [`draw_part`], each kept cube's faces into the surface of the slot `keep` gives it,
/// as a section's near detail is drawn a part of it at a time (`sections::CLUSTER`).
#[allow(clippy::too_many_arguments)]
pub fn draw_slots(
    cubes: &BTreeSet<[i32; 3]>,
    keep: &dyn Fn(IVec3) -> Option<usize>,
    kin: &dyn Fn(IVec3) -> bool,
    hides: &dyn Fn(IVec3) -> bool,
    look: &Look,
    heights: &Heights,
    cuts: &Cuts,
    colour: [f32; 3],
    outs: &mut [Surface],
    tops: &mut [Surface],
    caching: Option<Caching>,
) {
    // Kept faces are known by everything round them for three cubes.
    let asked = if caching.is_some() { 3 } else { 1 };
    let nb = Neighbourhood::of(cubes, kin, hides, cuts, look, asked);
    let n = look.steps.max(1) as usize;
    // What a cube's faces are worked out from, as two numbers: where their points are, from
    // which cubes are drawn and full two round it, and how those drawn are cut; and their
    // normals along their rims, which take the faces beside them, from those three round
    // it. Each a box of cells' numbers XORed together (Zobrist hashing, as chess engines and
    // voxel meshers key a neighbourhood), read in a few steps from the grid's running XOR
    // rather than cell by cell: a cube's is asked of every cube with faces. A cube cut with
    // no number for its cut is not kept.
    let windows = caching.is_some().then(|| nb.windows());
    let spacing = Spacing::new(n, &nb, look, heights, kin);
    let call = caching.as_ref().map_or(0, |caching| caching.call);
    let old = caching.as_ref().and_then(|caching| caching.old);
    let ctx = FaceContext {
        nb: &nb,
        spacing: &spacing,
        look,
        heights,
        kin,
        old,
        call,
        n,
        fine_step: CELL_M / (HOLED_FINER * n) as f32,
    };
    // Each face drawn: a grid of points, placed once each, and its triangles' corners in it.
    let mut faces: Vec<Face> = Vec::new();
    let mut scratch = Scratch::default();
    for &cube in cubes {
        let cube = IVec3::from_array(cube);
        let slot = keep(cube);
        let keeping = slot.is_some();
        let slot = slot.unwrap_or(0);
        // A cube left out matters only where it meets one kept, a step away.
        if !keeping
            && !NEIGHBOURHOOD
                .iter()
                .any(|&offset| keep(cube + offset).is_some())
        {
            continue;
        }
        let low = cube.as_vec3() * CELL_M;
        // The inside edges round it, looked for once a face of it is worked out anew.
        scratch.edges_found = false;
        let own_box = nb.shrunk_at(cube).unwrap_or(Rounded {
            low,
            high: low + Vec3::splat(CELL_M),
            cube: low,
            bites: 0,
        });
        let cut_here = nb.is_cut(cube);
        let signed = if caching.is_some() {
            nb.signature(windows.as_ref(), heights, cube)
        } else {
            None
        };
        for (index, (axis, side)) in FACES.into_iter().enumerate() {
            let at = CubeAt {
                cube,
                low,
                slot,
                keeping,
                own_box,
                cut_here,
                signed,
            };
            if let Some(face) = draw_face(&ctx, &at, &mut scratch, index, axis, side) {
                faces.push(face);
            }
        }
    }
    smooth_normals(&mut faces, &nb.grid);
    put(faces, outs, tops, colour, caching);
}

/// The cubes drawn and those round them: which are drawn, which full, which cut and by
/// what, each drawn cube's box shrunk by the bevel and bitten at its inside corners, and
/// every inside edge, kept by the cell its middle is in.
struct Neighbourhood<'a> {
    grid: Grid,
    member: Vec<bool>,
    fill: Vec<u8>,
    cut_of: Vec<bool>,
    cut_keys: Vec<Option<u64>>,
    shrunk: Vec<(Vec3, Vec3)>,
    bitten: Vec<u16>,
    first_edge: Vec<u32>,
    edges: Vec<(u32, (Vec3, Vec3))>,
    hides: &'a dyn Fn(IVec3) -> bool,
    cuts: &'a Cuts<'a>,
}

impl<'a> Neighbourhood<'a> {
    fn of(
        cubes: &BTreeSet<[i32; 3]>,
        kin: &dyn Fn(IVec3) -> bool,
        hides: &'a dyn Fn(IVec3) -> bool,
        cuts: &'a Cuts<'a>,
        look: &Look,
        asked: i32,
    ) -> Neighbourhood<'a> {
        let grid = Grid::around(cubes);
        // Which cells are cubes drawn, and whether each cell round them is full, asked of each
        // once: `hides` reads the base.
        let mut member = vec![false; grid.len()];
        let mut fill = vec![UNKNOWN; grid.len()];
        for &cube in cubes {
            if let Some(i) = grid.index(IVec3::from_array(cube)) {
                member[i] = true;
                fill[i] = FULL;
            }
        }
        for &cube in cubes {
            let cube = IVec3::from_array(cube);
            for z in -asked..=asked {
                for y in -asked..=asked {
                    for x in -asked..=asked {
                        let at = cube + IVec3::new(x, y, z);
                        if let Some(i) = grid.index(at)
                            && fill[i] == UNKNOWN
                        {
                            fill[i] = if hides(at) { FULL } else { EMPTY };
                        }
                    }
                }
            }
        }
        // Whether each cube drawn is cut, and what its cut is from, asked of each once: `cuts`
        // reads the base.
        let mut cut_of = vec![false; grid.len()];
        let mut cut_keys: Vec<Option<u64>> = vec![None; grid.len()];
        for &cube in cubes {
            let cube = IVec3::from_array(cube);
            if let Some(i) = grid.index(cube)
                && (cuts.of)(cube)
            {
                cut_of[i] = true;
                cut_keys[i] = (cuts.key)(cube);
            }
        }
        let mut nb = Neighbourhood {
            grid,
            member,
            fill,
            cut_of,
            cut_keys,
            shrunk: Vec::new(),
            bitten: Vec::new(),
            first_edge: Vec::new(),
            edges: Vec::new(),
            hides,
            cuts,
        };
        let (grid, cut) = (&nb.grid, cuts.at);
        let is_cut = |at: IVec3| nb.is_cut(at);
        let full = |at: IVec3| nb.full(at);
        let really_cut = |c: IVec3| nb.really_cut(c);
        // Each cube shrunk by the bevel on every side nothing hides, but for a top cut away over
        // its middle, as a gable's top is at its roof or a slope's at its ramp: what is left
        // ends at the cut, sawn straight, not rounded off at every cube along it.
        let mut shrunk: Vec<(Vec3, Vec3)> = vec![(Vec3::ZERO, Vec3::ZERO); grid.len()];
        for &cube in cubes {
            let cube = IVec3::from_array(cube);
            let mut low = cube.as_vec3() * CELL_M;
            let mut high = low + Vec3::splat(CELL_M);
            let cut_top = is_cut(cube)
                && cut(cube, low + Vec3::new(CELL_M * 0.5, CELL_M, CELL_M * 0.5))[1]
                    < high.y - 1e-4;
            for axis in 0..3 {
                if !full(cube - step(axis, 1)) {
                    low[axis] += look.bevel;
                }
                if !full(cube + step(axis, 1)) && !(axis == 1 && cut_top) {
                    high[axis] -= look.bevel;
                }
            }
            if let Some(i) = grid.index(cube) {
                shrunk[i] = (low, high);
            }
        }
        // Every inside edge: a cube's edge with three of the four cubes round it full, kept by
        // the cell its middle is in (a list a cell, linked through `edges`), so a point looks
        // only at those near it.
        let mut first_edge = vec![u32::MAX; grid.len()];
        let mut edges: Vec<(u32, (Vec3, Vec3))> = Vec::new();
        for &cube in cubes {
            let cube = IVec3::from_array(cube);
            for axis in 0..3 {
                let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
                for (da, db) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    // The edge along `axis` at this corner of the cube, and the four cubes round
                    // it.
                    let corner = cube + step(a, da) + step(b, db);
                    let round = [(0, 0), (-1, 0), (0, -1), (-1, -1)]
                        .map(|(ea, eb)| full(corner + step(a, ea) + step(b, eb)));
                    if round.iter().filter(|&&f| f).count() != 3 {
                        continue;
                    }
                    let from = corner.as_vec3() * CELL_M;
                    let to = from + unit(axis, CELL_M);
                    let cell = ((from + to) * 0.5 / CELL_M).floor().as_ivec3();
                    let Some(slot) = grid.index(cell) else {
                        continue;
                    };
                    let mut at = first_edge[slot];
                    let mut known = false;
                    while at != u32::MAX {
                        let (next, edge) = edges[at as usize];
                        if edge == (from, to) {
                            known = true;
                            break;
                        }
                        at = next;
                    }
                    if !known {
                        edges.push((first_edge[slot], (from, to)));
                        first_edge[slot] = (edges.len() - 1) as u32;
                    }
                }
            }
        }
        // Along each edge of a cube where the two cubes across its faces are drawn into the same
        // surface and the one diagonally across it is not full, an inside edge's corner cube:
        // the edge bitten out of its shrunk box a bevel round, a bit an edge (`Rounded`), so the
        // box's corner, unshrunk both ways, does not hold the surface out past the inside edge,
        // and where the two outside edges either side of it meet a face, the three round into
        // one another. Not a cube cut, whose top stays on its cut as it is.
        let mut bitten: Vec<u16> = vec![0; grid.len()];
        for &cube in cubes {
            let cube = IVec3::from_array(cube);
            let Some(i) = grid.index(cube) else {
                continue;
            };
            if really_cut(cube) {
                continue;
            }
            for axis in 0..3 {
                let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
                for (da, db) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (across_a, across_b) = (step(a, 2 * da - 1), step(b, 2 * db - 1));
                    if kin(cube + across_a)
                        && kin(cube + across_b)
                        && !full(cube + across_a + across_b)
                    {
                        bitten[i] |= 1 << bite_bit(axis, da, db);
                    }
                }
            }
        }
        (nb.shrunk, nb.first_edge, nb.edges, nb.bitten) = (shrunk, first_edge, edges, bitten);
        nb
    }

    /// Whether a cube may be cut.
    fn is_cut(&self, at: IVec3) -> bool {
        match self.grid.index(at) {
            Some(i) => self.cut_of[i],
            None => (self.cuts.of)(at),
        }
    }

    /// Whether a cube is one of those drawn.
    fn is_member(&self, at: IVec3) -> bool {
        self.grid.index(at).is_some_and(|i| self.member[i])
    }

    /// Whether a cube is full.
    fn full(&self, at: IVec3) -> bool {
        match self.grid.index(at) {
            Some(i) if self.fill[i] != UNKNOWN => self.fill[i] == FULL,
            Some(i) => self.member[i] || (self.hides)(at),
            None => (self.hides)(at),
        }
    }

    // The inside edges near a cube, which a point of its faces may be near: those further
    // off are further than the fade from it.
    fn near_inside(&self, cube: IVec3, found: &mut Vec<(Vec3, Vec3)>) {
        let (grid, first_edge, edges) = (&self.grid, &self.first_edge, &self.edges);
        found.clear();
        for z in -1..=2 {
            for y in -1..=2 {
                for x in -1..=2 {
                    let Some(slot) = grid.index(cube + IVec3::new(x, y, z)) else {
                        continue;
                    };
                    let mut at = first_edge[slot];
                    while at != u32::MAX {
                        let (next, edge) = edges[at as usize];
                        found.push(edge);
                        at = next;
                    }
                }
            }
        }
    }

    // Whether a cube that may be cut is: its cut passes through it somewhere over its top's
    // corners or middle.
    fn really_cut(&self, c: IVec3) -> bool {
        let cut = self.cuts.at;
        let is_cut = |at: IVec3| self.is_cut(at);
        is_cut(c) && {
            let low = c.as_vec3() * CELL_M;
            [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0], [0.5, 0.5]]
                .iter()
                .any(|&[u, v]| {
                    let p = low + Vec3::new(u, 0.5, v) * CELL_M;
                    let [under, over] = cut(c, p);
                    over < low.y + CELL_M - 1e-4 || under > low.y + 1e-4
                })
        }
    }

    /// A drawn cube's box, shrunk and bitten.
    fn shrunk_at(&self, at: IVec3) -> Option<Rounded> {
        let (grid, member, shrunk, bitten) = (&self.grid, &self.member, &self.shrunk, &self.bitten);
        grid.index(at).filter(|&i| member[i]).map(|i| Rounded {
            low: shrunk[i].0,
            high: shrunk[i].1,
            cube: at.as_vec3() * CELL_M,
            bites: bitten[i],
        })
    }

    /// The running tables a cube's [`Signature`] is read from.
    fn windows(&self) -> [Running; 3] {
        let (grid, member, fill, cut_of, cut_keys) = (
            &self.grid,
            &self.member,
            &self.fill,
            &self.cut_of,
            &self.cut_keys,
        );
        let state = |i: usize| u64::from(member[i]) << 1 | u64::from(fill[i] == FULL);
        // By where the cell is, not its place in the grid, which an edit may move.
        let zobrist = |i: usize, what: u64| {
            let i = i as i32;
            let at = grid.low
                + IVec3::new(
                    i % grid.size.x,
                    i / grid.size.x % grid.size.y,
                    i / (grid.size.x * grid.size.y),
                );
            crate::layout::splitmix(
                u64::from(at.x as u32)
                    ^ u64::from(at.y as u32).rotate_left(21)
                    ^ u64::from(at.z as u32).rotate_left(42)
                    ^ what.rotate_left(60),
            )
        };
        let cells = grid.len();
        let states: Vec<u64> = (0..cells).map(|i| zobrist(i, state(i) + 1)).collect();
        let cuts: Vec<u64> = (0..cells)
            .map(|i| match (member[i] && cut_of[i], cut_keys[i]) {
                (true, Some(key)) => crate::layout::splitmix(zobrist(i, 7) ^ key),
                _ => 0,
            })
            .collect();
        let unkeyed: Vec<u64> = (0..cells)
            .map(|i| u64::from(member[i] && cut_of[i] && cut_keys[i].is_none()))
            .collect();
        [states, cuts, unkeyed].map(|values| Running::of(grid, &values))
    }

    /// What a cube's faces are worked out from ([`Neighbourhood::windows`]), if it can be
    /// kept.
    fn signature(
        &self,
        windows: Option<&[Running; 3]>,
        heights: &Heights,
        cube: IVec3,
    ) -> Option<Signature> {
        let grid = &self.grid;
        let [states, cuts, unkeyed] = windows?;
        if unkeyed.sum(grid, cube, 2) != 0 {
            return None;
        }
        let cut = cuts.xor(grid, cube, 2);
        let beds = heights
            .bed_key
            .map_or(0, |bed_key| crate::layout::splitmix(bed_key(cube) ^ 0x5bed))
            ^ heights
                .holed
                .and_then(|holed| holed(cube))
                .map_or(0, |key| crate::layout::splitmix(key ^ 0x401e));
        Some(Signature {
            shape: states.xor(grid, cube, 2) ^ cut ^ beds,
            whole: states.xor(grid, cube, 3) ^ cut ^ beds ^ 0x3,
        })
    }
}

/// How far a point's relief is faded by the inside edges near it.
fn fade(p: Vec3, edges: &[(Vec3, Vec3)]) -> f32 {
    edges
        .iter()
        .map(|&(from, to)| {
            let along = (to - from).normalize();
            let t = (p - from).dot(along).clamp(0.0, CELL_M);
            (p.distance(from + along * t) / INSIDE_FADE_M).min(1.0)
        })
        .fold(1.0_f32, f32::min)
}

// Where a cube's faces' points fall across it along an axis, as fractions of a cube: a
// cube cut, or with a piece bedded near it, evenly at its steps, as its
// shape there needs; any other only where its shape bends (`across`), its stone flat
// between, its relief left to its maps. A cube's, whichever of its faces asks, so the
// faces round a line agree on it.
struct Spacing<'a> {
    n: usize,
    even: Vec<f32>,
    fine: Vec<f32>,
    half: Vec<f32>,
    /// Each cube's, worked out once: the faces round a line each ask for the four cubes'.
    known: std::cell::RefCell<QuickMap<IVec3, [Arc<[f32]>; 3]>>,
    nb: &'a Neighbourhood<'a>,
    look: &'a Look,
    heights: &'a Heights<'a>,
    kin: &'a dyn Fn(IVec3) -> bool,
}

impl<'a> Spacing<'a> {
    fn new(
        n: usize,
        nb: &'a Neighbourhood<'a>,
        look: &'a Look,
        heights: &'a Heights<'a>,
        kin: &'a dyn Fn(IVec3) -> bool,
    ) -> Spacing<'a> {
        let even: Vec<f32> = (0..=n).map(|i| i as f32 / n as f32).collect();
        // Holed by damage, finer than its steps (`HOLED_FINER`), so a stone's hole is found.
        let fine: Vec<f32> = {
            let m = HOLED_FINER * n;
            (0..=m).map(|i| i as f32 / m as f32).collect()
        };
        // Round a bed, at half its steps: the wall's lap over a lintel's edge every 10 cm near.
        let half: Vec<f32> = {
            let m = n.div_ceil(2).max(1);
            (0..=m).map(|i| i as f32 / m as f32).collect()
        };
        Spacing {
            n,
            even,
            fine,
            half,
            known: Default::default(),
            nb,
            look,
            heights,
            kin,
        }
    }

    // Where a cube's cut, a roof's underside or a shape's top, crosses its top or its foot
    // along an axis across it, as fractions of the cube, read along its four edges that
    // way: a cut runs straight across a cube, so points there and at its corners follow it
    // exactly (D284).
    fn crossings(&self, c: IVec3, axis: usize) -> Vec<f32> {
        let (n, cut) = (self.n, self.nb.cuts.at);
        let low = c.as_vec3() * CELL_M;
        let other = if axis == 0 { 2 } else { 0 };
        let mut out = Vec::new();
        for o in [0.0, 1.0] {
            let at = |t: f32| {
                let mut p = low + Vec3::Y * CELL_M * 0.5;
                p[axis] += t * CELL_M;
                p[other] += o * CELL_M;
                cut(c, p)
            };
            // Read at every step, for where it bends (a gable's apex, a hip's crease) as
            // well as where it crosses.
            let samples: Vec<[f32; 2]> = (0..=n).map(|i| at(i as f32 / n as f32)).collect();
            for side in 0..2 {
                let h: Vec<f32> = samples.iter().map(|s| s[side]).collect();
                if h.iter().any(|v| v.abs() > 1e30) {
                    continue;
                }
                for i in 0..n {
                    let (from, to) = (h[i], h[i + 1]);
                    for level in [low.y, low.y + CELL_M] {
                        if (from - level) * (to - level) < 0.0 {
                            let t = (i as f32 + (level - from) / (to - from)) / n as f32;
                            // Not where a corner or another point already is, a sliver away.
                            if t > 0.02 && t < 0.98 {
                                out.push(t);
                            }
                        }
                    }
                    if i > 0 && (h[i - 1] - 2.0 * h[i] + h[i + 1]).abs() > 1e-4 {
                        out.push(i as f32 / n as f32);
                    }
                }
            }
        }
        out
    }

    // Whether a piece is bedded where it changes a cube's faces: each face read on a grid
    // a quarter of a cube apart, no point of it more than 9 cm from one,
    // first at its corners and middle to pass over the many faces nowhere near a piece.
    fn bedded_near(&self, c: IVec3) -> bool {
        let heights = self.heights;
        heights.bed.is_some_and(|bed| {
            let low = c.as_vec3() * CELL_M;
            let at = |axis: usize, side: f32, u: f32, v: f32| {
                let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
                let mut p = low;
                p[axis] += if side > 0.0 { CELL_M } else { 0.0 };
                p[a] += u * CELL_M;
                p[b] += v * CELL_M;
                p
            };
            let within = |p: Vec3, reach: f32| bed(p).is_some_and(|(inside, ..)| inside > -reach);
            FACES.iter().any(|&(axis, side)| {
                let near = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0], [0.5, 0.5]]
                    .iter()
                    .any(|&[u, v]| within(at(axis, side, u, v), BED_LAP_M + CELL_M * 0.5));
                near && (0..=4).any(|i| {
                    (0..=4).any(|j| {
                        let p = at(axis, side, i as f32 / 4.0, j as f32 / 4.0);
                        within(p, BED_LAP_M + 0.09)
                    })
                })
            })
        })
    }

    // Asked only of a cube itself, never of those round it, so a section and its neighbour,
    // each knowing the cubes a step past its edge and no further, agree on the cubes they
    // share. A side against a cut cube follows the cut's line between its corners, which a
    // slope's or a roof's underside is, straight across a cube.
    // Whether a cube's side toward another is open as far as where its points fall goes:
    // but against a cube drawn into the same surface. Read from `kin` alone, the same in
    // every call that draws into the surface; open where it is not quite (against another
    // kind's cube, say) only adds points where none are needed.
    fn worked_out(&self, c: IVec3) -> [Arc<[f32]>; 3] {
        let (look, heights) = (self.look, self.heights);
        let (even, fine, half) = (&self.even, &self.fine, &self.half);
        let really_cut = |c: IVec3| self.nb.really_cut(c);
        let bedded_near = |c: IVec3| self.bedded_near(c);
        let crossings = |c: IVec3, axis: usize| self.crossings(c, axis);
        let open = |c: IVec3| !(self.kin)(c);
        let cut = really_cut(c);
        // Earth's packing, cut, follows its lumps up to the cut: evenly, as a bed's lap does.
        if look.packed.is_some() && cut {
            let even: Arc<[f32]> = even.clone().into();
            return [even.clone(), even.clone(), even];
        }
        if heights.holed.is_some_and(|holed| holed(c).is_some()) {
            let fine: Arc<[f32]> = fine.clone().into();
            return [fine.clone(), fine.clone(), fine];
        }
        if bedded_near(c) {
            let half: Arc<[f32]> = half.clone().into();
            return [half.clone(), half.clone(), half];
        }
        [0, 1, 2].map(|axis| {
            let mut out = across(open(c - step(axis, 1)), open(c + step(axis, 1)), look);
            if axis != 1 && cut {
                // The cut's crossings both ways across, each mirrored, and its middle: a
                // corner's hip or valley crosses its top on a diagonal, bending the cut
                // between its edges (`roofs::gable_cut`), and with the same points both ways
                // every square it crosses has its corners on it, to split along it.
                for t in crossings(c, 0).into_iter().chain(crossings(c, 2)) {
                    out.extend([t, 1.0 - t]);
                }
                out.push(0.5);
                out.sort_by(f32::total_cmp);
                out.dedup_by(|x, y| (*x - *y).abs() < 0.02);
            }
            out.into()
        })
    }

    /// Where a cube's face's points fall across it along an axis, worked out once a cube.
    fn grid_of(&self, c: IVec3, axis: usize) -> Arc<[f32]> {
        if let Some(found) = self.known.borrow().get(&c) {
            return found[axis].clone();
        }
        let made = self.worked_out(c);
        let out = made[axis].clone();
        self.known.borrow_mut().insert(c, made);
        out
    }
}

/// What every face drawn in a call is drawn from.
struct FaceContext<'a> {
    nb: &'a Neighbourhood<'a>,
    spacing: &'a Spacing<'a>,
    look: &'a Look,
    heights: &'a Heights<'a>,
    kin: &'a dyn Fn(IVec3) -> bool,
    old: Option<&'a FaceCache>,
    call: u32,
    n: usize,
    fine_step: f32,
}

/// The cube whose faces are drawn, and what is known of it.
struct CubeAt {
    cube: IVec3,
    low: Vec3,
    slot: usize,
    keeping: bool,
    own_box: Rounded,
    cut_here: bool,
    signed: Option<Signature>,
}

/// What a face's drawing reuses from one face to the next.
#[derive(Default)]
struct Scratch {
    /// Whether the cube's inside edges have been looked for.
    edges_found: bool,
    cube_edges: Vec<(Vec3, Vec3)>,
    face_edges: Vec<(Vec3, Vec3)>,
    round: Vec<Rounded>,
    flat: Vec<Vec3>,
}

/// One face of a cube, toward `axis` on `side`: as it was last time where nothing it is
/// drawn from has changed, else its points placed, rounded, displaced and cut, and its
/// triangles; `None` where it does not show.
fn draw_face(
    ctx: &FaceContext,
    at: &CubeAt,
    scratch: &mut Scratch,
    index: usize,
    axis: usize,
    side: f32,
) -> Option<Face> {
    let &FaceContext {
        nb,
        spacing,
        look,
        heights,
        kin,
        old,
        call,
        n,
        fine_step,
    } = ctx;
    let &CubeAt {
        cube,
        low,
        slot,
        keeping,
        own_box,
        cut_here,
        signed,
    } = at;
    let Scratch {
        edges_found,
        cube_edges,
        face_edges,
        round,
        flat,
    } = scratch;
    let cut = nb.cuts.at;
    let (grid, cut_of, cut_keys) = (&nb.grid, &nb.cut_of, &nb.cut_keys);
    let full = |at: IVec3| nb.full(at);
    let is_member = |at: IVec3| nb.is_member(at);
    let is_cut = |at: IVec3| nb.is_cut(at);
    let shrunk_at = |at: IVec3| nb.shrunk_at(at);
    let near_inside = |cube: IVec3, found: &mut Vec<(Vec3, Vec3)>| nb.near_inside(cube, found);
    let grid_of = |c: IVec3, axis: usize| spacing.grid_of(c, axis);
    let out_step = step(axis, side as i32);
    let beyond = cube + out_step;
    let normal = unit(axis, side);
    let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
    let plane = if side > 0.0 {
        low[axis] + CELL_M
    } else {
        low[axis]
    };
    let point = |i: usize, j: usize| {
        let mut p = Vec3::ZERO;
        p[axis] = plane;
        p[a] = low[a] + CELL_M * i as f32 / n as f32;
        p[b] = low[b] + CELL_M * j as f32 / n as f32;
        p
    };
    // Against anything full that is not one of these cubes cut, nothing shows.
    if full(beyond) && !(axis != 1 && is_member(beyond) && is_cut(beyond)) {
        return None;
    }
    // A face kept from last time was drawn from the same cuts round it, so shows as
    // it did: whether it shows against a cut neighbour is not asked again.
    // Its points as they were last time, if nothing two round it has changed, but its
    // rims' normals to be worked out again from the faces beside it.
    if let (Some(signed), Some(old)) = (signed, old)
        && let Some(data) = old.faces.get(&(call, cube, index as u8))
        && data.signature.is_some_and(|was| was.shape == signed.shape)
        && data.signature.is_some_and(|was| was.whole != signed.whole)
    {
        let mut data = FaceData::clone(data);
        data.signature = Some(signed);
        return Some(Face {
            data: Arc::new(data),
            keeping,
            slot,
            cube,
            index: index as u8,
            fresh: true,
            rims_only: true,
        });
    }
    // As it was drawn last time, if nothing round it has changed since.
    if let (Some(signed), Some(old)) = (signed, old)
        && let Some(data) = old.faces.get(&(call, cube, index as u8))
        && data.signature == Some(signed)
    {
        return Some(Face {
            data: data.clone(),
            keeping,
            slot,
            cube,
            index: index as u8,
            fresh: false,
            rims_only: false,
        });
    }
    // A side against another of the cubes drawn shows where that one is cut lower
    // than this one, as a slope's side shows over a half beside it, or a tall wall's
    // over a gable carried up into a roof beside it. It is drawn whole, its part under
    // the other's cut inside the other's stone and never seen: brought up onto a
    // sloping cut between its points, it folded over itself in its own plane.
    // Against anything else full, nothing shows; nor against a cube not cut at all,
    // which is never lower.
    let against_cut = axis != 1
        && is_member(beyond)
        && is_cut(beyond)
        && (0..=n).any(|i| {
            (0..=n).any(|j| {
                let p = point(i, j);
                cut(beyond, p)[1] < cut(cube, p)[1].min(p.y) - 1e-4
            })
        });
    if full(beyond) && !against_cut {
        return None;
    }
    // Where its points are across it, as fractions of a cube each way (`grid_of`),
    // and along each of its sides every point any face on that side's line has, so
    // faces meet point for point with no crack: each side's line is shared by the
    // four cubes round it, and all of them take the points of all four.
    let along_a = grid_of(cube, a);
    let along_b = grid_of(cube, b);
    let (na, nb) = (along_a.len() - 1, along_b.len() - 1);
    let line = |along: usize, over: usize, toward: i32| -> Vec<f32> {
        let mut all = Vec::new();
        for x in [
            cube,
            cube + step(over, toward),
            beyond,
            beyond + step(over, toward),
        ] {
            if kin(x) {
                all.extend(grid_of(x, along).iter().copied());
            }
        }
        all.sort_by(f32::total_cmp);
        all.dedup_by(|x, y| (*x - *y).abs() < 1e-5);
        all
    };
    let missing = |line: Vec<f32>, own: &[f32]| -> Vec<f32> {
        line.into_iter()
            .filter(|&t| own.iter().all(|&o| (o - t).abs() >= 1e-5))
            .collect()
    };
    // The points each side lacks of its line's: along `a` on the sides at `b`'s low
    // and high edges, along `b` on those at `a`'s.
    let more = [
        missing(line(a, b, -1), &along_a),
        missing(line(a, b, 1), &along_a),
        missing(line(b, a, -1), &along_b),
        missing(line(b, a, 1), &along_b),
    ];
    let param = |u: f32, v: f32| {
        let mut p = Vec3::ZERO;
        p[axis] = plane;
        p[a] = low[a] + CELL_M * u;
        p[b] = low[b] + CELL_M * v;
        p
    };
    let point = |i: usize, j: usize| param(along_a[i], along_b[j]);
    // Only the boxes that may be nearest a point of this face: none further from
    // the face than its own cube's box can be, at most the bevel along every axis.
    let (face_low, face_high) = {
        let (mut l, mut h) = (low, low + Vec3::splat(CELL_M));
        l[axis] = plane;
        h[axis] = plane;
        (l, h)
    };
    let reach = look.bevel * look.bevel * 3.0 + 1e-6;
    round.clear();
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                let Some(rounded) = shrunk_at(cube + IVec3::new(x, y, z)) else {
                    continue;
                };
                let (l, h) = (rounded.low, rounded.high);
                let gap = (l - face_high).max(face_low - h).max(Vec3::ZERO);
                if gap.length_squared() <= reach {
                    round.push(rounded);
                }
            }
        }
    }
    if !*edges_found {
        near_inside(cube, cube_edges);
        *edges_found = true;
    }
    // Only the inside edges that come within the fade of this face: the rest fade
    // nothing anywhere on it.
    face_edges.clear();
    face_edges.extend(cube_edges.iter().copied().filter(|&(from, to)| {
        let gap = (from.min(to) - face_high)
            .max(face_low - from.max(to))
            .max(Vec3::ZERO);
        gap.length() < INSIDE_FADE_M
    }));
    // Everything its points are placed from, as a number: where they fall across it,
    // the boxes they are rounded to, the inside edges that fade them, its cut and
    // its neighbour's, and the pieces bedded near. A face whose cubes round it
    // changed, but not anything it is placed from, as most two cubes from an edit,
    // keeps its points and has only its rims' normals worked out again.
    let placed = signed.map(|_| {
        let mut hash = crate::mesh::Corners::default();
        let mut floats = |values: &[f32]| {
            hash.write_usize(values.len());
            for v in values {
                hash.write_u32(v.to_bits());
            }
        };
        floats(&along_a);
        floats(&along_b);
        for more in &more {
            floats(more);
        }
        for rounded in round.iter().chain([&own_box]) {
            floats(&rounded.low.to_array());
            floats(&rounded.high.to_array());
            floats(&[f32::from(rounded.bites)]);
        }
        for (l, h) in face_edges.iter() {
            floats(&l.to_array());
            floats(&h.to_array());
        }
        hash.write_u8(u8::from(against_cut));
        for c in [cube, beyond] {
            if let Some(i) = grid.index(c)
                && cut_of[i]
            {
                hash.write_u64(cut_keys[i].unwrap_or(u64::MAX));
            }
        }
        if let Some(bed_key) = heights.bed_key {
            hash.write_u64(bed_key(cube));
        }
        if let Some(key) = heights.holed.and_then(|holed| holed(cube)) {
            hash.write_u64(key);
        }
        hash.finish()
    });
    if let (Some(signed), Some(old), Some(placed)) = (signed, old, placed)
        && let Some(data) = old.faces.get(&(call, cube, index as u8))
        && data.placed == Some(placed)
    {
        let mut data = FaceData::clone(data);
        data.signature = Some(signed);
        return Some(Face {
            data: Arc::new(data),
            keeping,
            slot,
            cube,
            index: index as u8,
            fresh: true,
            rims_only: true,
        });
    }
    // A point further than the bevel from every edge of a face open to the air is
    // nearer its own cube's box than any other's: every other cube's is beyond the
    // face's edges, or behind it.
    let margin = look.bevel + 1e-3;
    let inner = |p: Vec3| {
        !against_cut
            && p[a] - face_low[a] > margin
            && face_high[a] - p[a] > margin
            && p[b] - face_low[b] > margin
            && face_high[b] - p[b] > margin
    };
    // Holed by damage: where the hole is, and each point beside it moved onto its
    // outline, so the hole's rim runs along its stones' edges rather than stepping
    // round the grid's squares, as a contour is marched (marching squares): along
    // each line of the grid from a point left standing to one gone, the place it
    // crosses the outline, found by halving. A point inside the face takes the
    // middle of its crossings, or the nearest where the middle falls in the hole;
    // one on the face's side slides along that side only, the same from the face
    // beside it, and only where the wall runs on flat past it, so faces still meet.
    let holing = axis != 1
        && heights.hole.is_some()
        && heights.holed.is_some_and(|holed| holed(cube).is_some());
    let hole_raw = |p: Vec3| heights.hole.map_or(0.0, |hole| hole(p, normal));
    // Read once at each point of the fine grid and one round it, which every point
    // and its neighbours fall on; between them, as asked.
    let fine_n = HOLED_FINER * n;
    let lattice: Vec<f32> = if holing {
        (0..(fine_n + 3) * (fine_n + 3))
            .map(|k| {
                let (i, j) = (k % (fine_n + 3), k / (fine_n + 3));
                let mut p = Vec3::ZERO;
                p[axis] = plane;
                p[a] = low[a] + (i as f32 - 1.0) * fine_step;
                p[b] = low[b] + (j as f32 - 1.0) * fine_step;
                hole_raw(p)
            })
            .collect()
    } else {
        Vec::new()
    };
    let on_lattice = |p: Vec3| -> Option<usize> {
        let (u, v) = ((p[a] - low[a]) / fine_step, (p[b] - low[b]) / fine_step);
        let (i, j) = (u.round(), v.round());
        (!lattice.is_empty()
            && (u - i).abs() < 1e-3
            && (v - j).abs() < 1e-3
            && (-1.0..=fine_n as f32 + 1.0).contains(&i)
            && (-1.0..=fine_n as f32 + 1.0).contains(&j))
        .then(|| (j as usize + 1) * (fine_n + 3) + i as usize + 1)
    };
    let hole_at = |p: Vec3| on_lattice(p).map_or_else(|| hole_raw(p), |at| lattice[at]);
    let sunk_at = |p: Vec3| hole_at(p) > 1e-4;
    // How far a point is pushed back: as deep as its hole where it has gone; and a
    // speck left standing in a hole, too small for the grid to draw but as a spike,
    // goes with it: a point of the grid with no more than two of the eight round it
    // standing.
    let depth_of = |p: Vec3| -> f32 {
        let here = hole_at(p);
        if here > 1e-4 || !holing || on_lattice(p).is_none() {
            return here;
        }
        let (mut standing, mut deepest) = (0, 0.0f32);
        for (da, db) in [
            (1, 0),
            (1, 1),
            (0, 1),
            (-1, 1),
            (-1, 0),
            (-1, -1),
            (0, -1),
            (1, -1),
        ] {
            let mut q = p;
            q[a] += da as f32 * fine_step;
            q[b] += db as f32 * fine_step;
            let there = hole_at(q);
            if there <= 1e-4 {
                standing += 1;
                if standing > 2 {
                    return here;
                }
            }
            deepest = deepest.max(there);
        }
        deepest
    };
    let deep = |p: Vec3| depth_of(p) > 1e-4;
    let on_side = |p: Vec3, k: usize| -> Option<i32> {
        if (p[k] - face_low[k]).abs() < 1e-5 {
            Some(-1)
        } else if (face_high[k] - p[k]).abs() < 1e-5 {
            Some(1)
        } else {
            None
        }
    };
    let runs_on =
        |k: usize, toward: i32| kin(cube + step(k, toward)) && !full(beyond + step(k, toward));
    let snap = |p: Vec3| -> Vec3 {
        if !holing || deep(p) {
            return p;
        }
        let (ways, reach): (&[usize], f32) = match (on_side(p, a), on_side(p, b)) {
            (None, None) => (&[a, b], 1.0),
            (Some(toward), None) if runs_on(a, toward) => (&[b], RIM_SLIDE),
            (None, Some(toward)) if runs_on(b, toward) => (&[a], RIM_SLIDE),
            _ => return p,
        };
        let (mut sum, mut count, mut nearest) = (Vec3::ZERO, 0.0, None::<Vec3>);
        for &k in ways {
            for sign in [-1.0f32, 1.0] {
                let mut d = Vec3::ZERO;
                d[k] = sign * fine_step;
                if !deep(p + d) {
                    continue;
                }
                // Between the two, on the outline itself, a speck aside.
                let (mut standing, mut gone) = (0.0f32, 1.0f32);
                for _ in 0..4 {
                    let middle = (standing + gone) * 0.5;
                    if sunk_at(p + d * middle) {
                        gone = middle;
                    } else {
                        standing = middle;
                    }
                }
                let crossing = p + d * standing.min(reach);
                sum += crossing;
                count += 1.0;
                if nearest.is_none_or(|n| n.distance_squared(p) > crossing.distance_squared(p)) {
                    nearest = Some(crossing);
                }
            }
        }
        let Some(nearest) = nearest else {
            return p;
        };
        let middle = sum / count;
        if sunk_at(middle) { nearest } else { middle }
    };
    let own = [own_box];
    let place = |p: Vec3| -> (Vec3, Vec3) {
        let boxes: &[Rounded] = if inner(p) { &own } else { round };
        let nearest = boxes
            .iter()
            .map(|rounded| rounded.nearest(p, look.bevel))
            .min_by(|a, b| a.distance_squared(p).total_cmp(&b.distance_squared(p)))
            .unwrap_or(p);
        let away = p - nearest;
        let length = away.length();
        let (rounded, outward) = if length > 1e-6 {
            let outward = away / length;
            (nearest + outward * look.bevel, outward)
        } else {
            (p, normal)
        };
        let faded = if face_edges.is_empty() {
            1.0
        } else {
            fade(p, face_edges)
        };
        // A cube cut to a roof keeps to the roof's line: its lifts ease out of their steps
        // toward the cut.
        let stepping = if cut_here {
            ((cut(cube, p)[1] - p.y - 0.05) / 0.15).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let packed = look
            .packed
            .map_or(0.0, |packed| packed.at(p, outward, stepping));
        // Flat but for its packing: its stone's relief is drawn from its maps.
        let relief = 0.0;
        let mut out = (relief + packed) * faded;
        // Sunk behind a piece set into it, a little deeper than its deepest relief,
        // and built up over its edges round its outline.
        if let Some((inside, face, lapped)) = heights.bed.and_then(|bed| bed(p))
            && inside > -BED_LAP_M
        {
            // How far over the piece's edge the wall is built here, by its own
            // relief, so the edge it leaves is as uneven as the wall.
            let high = (relief / look.relief.max(1e-4) + 0.5).clamp(0.0, 1.0);
            let reach = BED_LAP_M * (0.25 + high);
            let over = ((reach - inside) / 0.01).clamp(0.0, 1.0)
                * (1.0 + inside / BED_LAP_M).clamp(0.0, 1.0);
            let sunk = ((inside - reach) / 0.015).clamp(0.0, 1.0);
            let deepest = -(look.relief * 0.5) - packed.abs() - BED_DEPTH_M;
            out += (deepest - out) * sunk;
            // Built up only where it laps over the edge: elsewhere a floor of
            // nothing would undo the sinking, leaving the wall in the plane of a
            // flush piece (a lintel's underside in its opening's head).
            if lapped && over > 0.0 {
                out = out.max(over * (face + BED_OVER_M));
            }
        }
        // Pushed back where damage has taken its stones out.
        out -= if holing { depth_of(p) } else { hole_raw(p) };
        // Its normal the rounding's where it is no more than rounded, the same
        // worked out from any face; else from its triangles (`ZERO`).
        let exact = if out.abs() < 1e-7 {
            outward
        } else {
            Vec3::ZERO
        };
        (rounded + outward * out, exact)
    };
    let sunk = std::cell::RefCell::new(Vec::<bool>::new());
    // Wound so each triangle faces out along the face's normal.
    let flip = side < 0.0;
    let top = axis == 1 && side > 0.0 && look.top.is_some();
    let metres = match look.top {
        Some((metres, _)) if top => metres,
        _ => look.metres,
    };
    let mut face = FaceData {
        signature: signed,
        placed,
        points: Vec::with_capacity((na + 1) * (nb + 1)),
        rim: Vec::with_capacity((na + 1) * (nb + 1)),
        exact: Vec::with_capacity((na + 1) * (nb + 1)),
        uvs: Vec::with_capacity((na + 1) * (nb + 1)),
        triangles: Vec::with_capacity(na * nb * 2),
        top,
        local: Vec::new(),
        normals: Vec::new(),
        order: Vec::new(),
        indices: Vec::new(),
        sunk: Vec::new(),
    };
    // Each point cut to the heights there, and where it would be unrounded, to
    // choose each square's diagonal by; a cube not cut is all there.
    let clipped = |p: Vec3| {
        if !cut_here {
            return p.y;
        }
        let [low, high] = cut(cube, p);
        p.y.max(low).min(high)
    };
    flat.clear();
    // A point of the face added: placed, where it would be unrounded, and textured.
    let add = |p: Vec3, rim: bool, face: &mut FaceData, flat: &mut Vec<Vec3>| {
        let [low, high] = if cut_here {
            cut(cube, p)
        } else {
            [f32::MIN, f32::MAX]
        };
        // A side is cut before it is rounded and displaced, and textured where the
        // point ends up, so the texture runs on unstretched to the cut and points cut
        // to the same place stay one.
        let s = if axis == 1 || !cut_here {
            p
        } else {
            p.with_y(p.y.max(low).min(high))
        };
        let s = snap(s);
        let (mut q, mut exact) = place(s);
        sunk.borrow_mut().push(holing && deep(s));
        let placed = q;
        if cut_here {
            q.y = q.y.max(low).min(high);
            // And a side's where it ends up, so nothing a lump or the rounding moves
            // along stands over the cut. Not a top's: its rounding and lumps, brought
            // onto a sloping cut where they end up, fold over themselves in the cut;
            // a top's point they would carry over the cut stays on it, as it was.
            let [under, over] = cut(cube, q);
            if axis != 1 {
                q.y = q.y.max(under).min(over);
            } else if q.y > over {
                q = s.with_y(s.y.max(low).min(high));
            } else if look.packed.is_some() {
                q.y = q.y.max(under);
            }
        }
        if q != placed {
            exact = Vec3::ZERO;
        }
        face.points.push(q);
        face.exact.push(exact);
        face.rim.push(rim);
        flat.push(p.with_y(clipped(p)));
        face.uvs.push(planar(s, normal, metres));
        (face.points.len() - 1) as u32
    };
    for i in 0..=na {
        for j in 0..=nb {
            let rim = i == 0 || j == 0 || i == na || j == nb;
            add(point(i, j), rim, &mut face, &mut *flat);
        }
    }
    // The points its sides lack of their lines', each with where it is along its side.
    let [bottom, top_side, left, right] = [0, 1, 2, 3].map(|s| {
        more[s]
            .iter()
            .map(|&t| {
                let p = match s {
                    0 => param(t, 0.0),
                    1 => param(t, 1.0),
                    2 => param(0.0, t),
                    _ => param(1.0, t),
                };
                (t, add(p, true, &mut face, &mut *flat))
            })
            .collect::<Vec<(f32, u32)>>()
    });
    let at = |i: usize, j: usize| (i * (nb + 1) + j) as u32;
    let within = |extra: &[(f32, u32)], from: f32, to: f32| -> Vec<(f32, u32)> {
        extra
            .iter()
            .copied()
            .filter(|&(t, _)| t > from && t < to)
            .collect()
    };
    for i in 0..na {
        for j in 0..nb {
            // The points a cell on the face's rim takes from its neighbours' sides,
            // round it in order: along its bottom, up its right, back along its top,
            // down its left.
            let (a0, a1, b0, b1) = (along_a[i], along_a[i + 1], along_b[j], along_b[j + 1]);
            let mut ring: Vec<u32> = vec![at(i, j)];
            if j == 0 {
                ring.extend(within(&bottom, a0, a1).iter().map(|&(_, c)| c));
            }
            ring.push(at(i + 1, j));
            if i + 1 == na {
                ring.extend(within(&right, b0, b1).iter().map(|&(_, c)| c));
            }
            ring.push(at(i + 1, j + 1));
            if j + 1 == nb {
                ring.extend(within(&top_side, a0, a1).iter().rev().map(|&(_, c)| c));
            }
            ring.push(at(i, j + 1));
            if i == 0 {
                ring.extend(within(&left, b0, b1).iter().rev().map(|&(_, c)| c));
            }
            let triangle = |t: [u32; 3], face: &mut FaceData| {
                let [x, y, z] = t;
                let t = if flip { [x, z, y] } else { t };
                let [pa, pb, pc] = t.map(|c| face.points[c as usize]);
                let cross = (pb - pa).cross(pc - pa);
                // Cut away to nothing, or folded back on itself where a cut or the
                // rounding pinches a cell: drawn from inside, it would show dark. A
                // smooth face's triangle turned more than 84° from its face is folded
                // too, edge on in another face's plane (a foot's rounding pinched at
                // an inner corner); its rounding's own steps turn no more than 75°.
                // A hole's side, from its rim down to its floor, stands square to the
                // face or near it, and is kept.
                let side_of_hole = {
                    let sunk = sunk.borrow();
                    holing
                        && t.iter().any(|&c| sunk[c as usize])
                        && t.iter().any(|&c| !sunk[c as usize])
                };
                let folded = if look.packed.is_some() {
                    -0.5 * cross.length()
                } else if side_of_hole {
                    -0.2 * cross.length()
                } else {
                    0.1 * cross.length()
                };
                if cross.length_squared() < 1e-12 || cross.dot(normal) < folded {
                    return;
                }
                face.triangles.push(t);
            };
            if ring.len() > 4 {
                // Its corners in the ring, and which of its sides took points.
                let corner_at = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
                let took = [
                    j == 0 && !within(&bottom, a0, a1).is_empty(),
                    i + 1 == na && !within(&right, b0, b1).is_empty(),
                    j + 1 == nb && !within(&top_side, a0, a1).is_empty(),
                    i == 0 && !within(&left, b0, b1).is_empty(),
                ];
                // A fan from a corner on neither side that took points, which every
                // point round the cell sees across it, as a cell's taken points lie
                // along its sides; failing one, from the cell's middle.
                let apex = (0..4)
                    .find(|&k| !took[k] && !took[(k + 3) % 4])
                    .map(|k| corner_at[k]);
                let (apex, open) = match apex {
                    Some(apex) => (apex, false),
                    None => {
                        let middle = add(
                            param((a0 + a1) * 0.5, (b0 + b1) * 0.5),
                            false,
                            &mut face,
                            &mut *flat,
                        );
                        (middle, true)
                    }
                };
                let start = ring.iter().position(|&c| c == apex).unwrap_or(0);
                let n_ring = ring.len();
                for k in 0..n_ring {
                    let (x, y) = (ring[(start + k) % n_ring], ring[(start + k + 1) % n_ring]);
                    if !open && (x == apex || y == apex) {
                        continue;
                    }
                    triangle([apex, x, y], &mut face);
                }
                continue;
            }
            let corners = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            // Split along the diagonal that follows the cut where it creases across
            // the square, as along a corner slope's hip or valley: the one whose
            // middle is nearer the cut surface at the square's middle.
            let f = corners.map(|c| flat[c as usize]);
            let middle = (f[0] + f[2]) * 0.5;
            let middle = middle.with_y(clipped(point(i, j).lerp(point(i + 1, j + 1), 0.5)));
            let along_02 = ((f[0] + f[2]) * 0.5).distance(middle);
            let along_13 = ((f[1] + f[3]) * 0.5).distance(middle);
            let mut split = if along_13 + 1e-5 < along_02 {
                [[0, 1, 3], [1, 2, 3]]
            } else {
                [[0, 1, 2], [0, 2, 3]]
            };
            // Where the rounding has pinched the square into a dart, seen square to
            // the face, along the diagonal from its reflex corner, which stays
            // inside it: the other would lay one triangle over its neighbours and
            // fold the other back.
            let flat2 = corners.map(|c| {
                let p = face.points[c as usize];
                glam::Vec2::new(p[a], p[b])
            });
            let turn = |k: usize| {
                let (p, q, r) = (flat2[(k + 3) % 4], flat2[k], flat2[(k + 1) % 4]);
                (q - p).perp_dot(r - q)
            };
            let turns = [0, 1, 2, 3].map(turn);
            let positive = turns.iter().filter(|t| **t > 0.0).count();
            if positive == 1 || positive == 3 {
                let odd = turns
                    .iter()
                    .position(|t| (*t > 0.0) == (positive == 1))
                    .unwrap_or(0);
                split = if odd % 2 == 0 {
                    [[0, 1, 2], [0, 2, 3]]
                } else {
                    [[0, 1, 3], [1, 2, 3]]
                };
            }
            // Beside a hole, along the diagonal that keeps a corner on its own side
            // of the outline: the rim runs between the two corners moved onto it.
            if holing {
                let sunk = sunk.borrow();
                let gone = corners.map(|c| sunk[c as usize]);
                let count = gone.iter().filter(|g| **g).count();
                if count == 1 || count == 3 {
                    let odd = gone.iter().position(|g| *g == (count == 1)).unwrap_or(0);
                    split = if odd % 2 == 0 {
                        [[0, 1, 3], [1, 2, 3]]
                    } else {
                        [[0, 1, 2], [0, 2, 3]]
                    };
                }
            }
            for [x, y, z] in split {
                triangle([corners[x], corners[y], corners[z]], &mut face);
            }
        }
    }
    if holing {
        face.sunk = sunk
            .borrow()
            .iter()
            .map(|&s| f32::from(u8::from(s)))
            .collect();
    }
    Some(Face {
        data: Arc::new(face),
        keeping,
        slot,
        cube,
        index: index as u8,
        fresh: true,
        rims_only: false,
    })
}

/// Each face's normals smoothed across every edge it shares with another.
fn smooth_normals(faces: &mut [Face], grid: &Grid) {
    // Smooth across every edge the faces share: the normal at a point is the sum of its
    // triangles' around it. Inside a face that is its own triangles; on a face's edge,
    // where other faces meet it, every triangle with a corner there, found by place: each
    // face's own sum there is added to the place's, a face at a time. A face kept from last
    // time keeps its normals, as does every face round it, so only the places on the edges
    // of faces worked out anew are summed, from the faces beside them.
    let edge = |rim: &[bool], index: usize| rim[index];
    let key = |p: Vec3| ((p * 2048.0).round()).as_ivec3();
    for face in faces
        .iter_mut()
        .filter(|face| face.fresh && !face.rims_only)
    {
        let data = Arc::get_mut(&mut face.data).expect("a face worked out anew is its own");
        let mut local = vec![Vec3::ZERO; data.points.len()];
        for t in &data.triangles {
            let [pa, pb, pc] = t.map(|c| data.points[c as usize]);
            let normal = (pb - pa).cross(pc - pa);
            for &c in t {
                local[c as usize] += normal;
            }
        }
        data.local = local;
    }
    let mut slots: QuickMap<IVec3, u32> = QuickMap::default();
    let mut sums: Vec<Vec3> = Vec::new();
    let mut anew = vec![false; grid.len()];
    for face in faces.iter().filter(|face| face.fresh) {
        if let Some(i) = grid.index(face.cube) {
            anew[i] = true;
        }
        for (c, &p) in face.data.points.iter().enumerate() {
            if edge(&face.data.rim, c) {
                slots.entry(key(p)).or_insert_with(|| {
                    sums.push(Vec3::ZERO);
                    (sums.len() - 1) as u32
                });
            }
        }
    }
    let beside_anew = |cube: IVec3| {
        NEIGHBOURHOOD
            .iter()
            .any(|&offset| grid.index(cube + offset).is_some_and(|i| anew[i]))
    };
    for face in faces
        .iter()
        .filter(|face| face.fresh || beside_anew(face.cube))
    {
        let data = &face.data;
        for (c, &p) in data.points.iter().enumerate() {
            if edge(&data.rim, c)
                && let Some(&slot) = slots.get(&key(p))
            {
                sums[slot as usize] += data.local[c];
            }
        }
    }
    // Each face's normals, and its points in the order its triangles first come to them,
    // with its triangles' corners among them: what it puts into a surface sharing corners.
    for face in faces.iter_mut().filter(|face| face.fresh) {
        let data = Arc::get_mut(&mut face.data).expect("a face worked out anew is its own");
        let rim = data.rim.clone();
        let sum_at = |c: usize| {
            if edge(&rim, c) {
                sums[slots[&key(data.points[c])] as usize]
            } else {
                data.local[c]
            }
        };
        let mut normals = vec![Vec3::ZERO; data.points.len()];
        let mut index: Vec<u32> = vec![u32::MAX; data.points.len()];
        let (mut order, mut indices) = (Vec::new(), Vec::with_capacity(data.triangles.len() * 3));
        for t in &data.triangles {
            let [pa, pb, pc] = t.map(|c| data.points[c as usize]);
            let fallback = (pb - pa).cross(pc - pa).normalize_or_zero();
            for &c in t {
                let slot = &mut index[c as usize];
                if *slot == u32::MAX {
                    *slot = order.len() as u32;
                    order.push(c);
                    let exact = data.exact[c as usize];
                    normals[c as usize] = if exact != Vec3::ZERO {
                        exact
                    } else {
                        sum_at(c as usize).normalize_or(fallback)
                    };
                }
                indices.push(*slot);
            }
        }
        (data.normals, data.order, data.indices) = (normals, order, indices);
    }
}

/// The faces kept put into their slots' surfaces, and kept for next time.
fn put(
    faces: Vec<Face>,
    outs: &mut [Surface],
    tops: &mut [Surface],
    colour: [f32; 3],
    mut caching: Option<Caching>,
) {
    // Room for what is kept, at once.
    let mut room = vec![[0usize; 2]; outs.len() * 2];
    for face in faces.iter().filter(|face| face.keeping) {
        let room = &mut room[face.slot * 2 + usize::from(face.data.top)];
        room[0] += face.data.order.len();
        room[1] += face.data.indices.len();
    }
    for (slot, (out, top)) in outs.iter_mut().zip(tops.iter_mut()).enumerate() {
        for (into, [points, corners]) in [(out, room[slot * 2]), (top, room[slot * 2 + 1])] {
            if into.sharing {
                into.shared.reserve(points, corners);
            } else {
                into.reserve(corners);
            }
        }
    }
    for face in faces.iter().filter(|face| face.keeping) {
        let data = &face.data;
        let into = if data.top {
            &mut tops[face.slot]
        } else {
            &mut outs[face.slot]
        };
        if into.sharing {
            // Each point of the grid once, with the triangles' corners shared.
            let first = into.shared.positions.len() as u32;
            for &c in &data.order {
                let c = c as usize;
                into.shared.worn_corner(
                    data.points[c],
                    data.normals[c],
                    data.uvs[c],
                    colour,
                    data.sunk.get(c).copied().unwrap_or(0.0),
                );
            }
            into.shared
                .indices
                .extend(data.indices.iter().map(|&i| first + i));
            continue;
        }
        for t in &data.triangles {
            let corners = t.map(|c| data.points[c as usize]);
            let normals = t.map(|c| data.normals[c as usize]);
            into.textured(corners, normals, t.map(|c| data.uvs[c as usize]), colour);
            let sunk = t.map(|c| data.sunk.get(c as usize).copied().unwrap_or(0.0));
            if sunk.iter().any(|&s| s > 0.0) {
                crate::mesh::worn(&mut into.wear, into.positions.len() - 3);
                into.wear.extend(sunk);
            }
        }
    }
    // Only faces kept are kept for next time: a face worked out only for its neighbours'
    // sake, beyond what is drawn, has its rims shaded without what lies past it.
    if let Some(caching) = caching.as_mut() {
        for face in faces
            .into_iter()
            .filter(|face| face.keeping && face.data.signature.is_some())
        {
            caching.new.taken += usize::from(!face.fresh);
            caching
                .new
                .faces
                .insert((caching.call, face.cube, face.index), face.data);
        }
    }
}
