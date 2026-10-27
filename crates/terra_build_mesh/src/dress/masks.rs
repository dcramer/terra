//! The masks that ground and weather what a kit draws: dirt and damp from the ground, worn
//! edges, moss, rain's shelter and a roof's weathering.

use super::*;

/// The ground's height under a point of a base, in metres in its own space, as the roles
/// read it (`layout::roles::Roles::ground`).
pub fn ground_at(base: &Base, point: Vec3) -> f32 {
    let column = [
        libm::floorf(point.x / CELL_M) as i32,
        libm::floorf(point.z / CELL_M) as i32,
    ];
    layout::metres(layout::roles::ground_ticks(base, column))
}

/// Smooth value noise over lattices so many metres across, from 0 to 1, read at many
/// points in turn: the corners of the cells last read are kept, a few to hand for each
/// lattice by where they are, as a surface's corners come face by face, so most fall in a
/// cell read just before, and a cell's corners are eight hashes.
#[derive(Default)]
pub(super) struct Noise {
    /// Each lattice by its size and seed, and the cells of it kept.
    lattices: Vec<(f32, u32, Box<Cells>)>,
}

/// A lattice's cells kept to hand, each its corner and its eight corners' values.
pub(super) type Cells = [Option<(IVec3, [f32; 8])>; CELLS_KEPT];

/// How many cells of each lattice are kept to hand.
pub(super) const CELLS_KEPT: usize = 32;

impl Noise {
    fn at(&mut self, p: Vec3, size: f32, seed: u32) -> f32 {
        let q = p / size;
        let base = q.floor();
        let f = q - base;
        let f = f * f * (Vec3::splat(3.0) - 2.0 * f);
        let b = base.as_ivec3();
        let lattice = match self
            .lattices
            .iter()
            .position(|(s, k, _)| *s == size && *k == seed)
        {
            Some(at) => at,
            None => {
                self.lattices
                    .push((size, seed, Box::new([None; CELLS_KEPT])));
                self.lattices.len() - 1
            }
        };
        let cells = &mut self.lattices[lattice].2;
        let slot = (b.x.wrapping_mul(73_856_093)
            ^ b.y.wrapping_mul(19_349_663)
            ^ b.z.wrapping_mul(83_492_791)) as u32 as usize
            % CELLS_KEPT;
        let v = match cells[slot] {
            Some((cell, v)) if cell == b => v,
            _ => {
                let at = |dx: i32, dy: i32, dz: i32| {
                    hash::unit(hash::of(&[
                        (b.x + dx) as u32,
                        (b.y + dy) as u32,
                        (b.z + dz) as u32,
                        seed,
                    ]))
                };
                let v = [
                    at(0, 0, 0),
                    at(1, 0, 0),
                    at(0, 1, 0),
                    at(1, 1, 0),
                    at(0, 0, 1),
                    at(1, 0, 1),
                    at(0, 1, 1),
                    at(1, 1, 1),
                ];
                cells[slot] = Some((b, v));
                v
            }
        };
        let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
        let x00 = lerp(v[0], v[1], f.x);
        let x10 = lerp(v[2], v[3], f.x);
        let x01 = lerp(v[4], v[5], f.x);
        let x11 = lerp(v[6], v[7], f.x);
        lerp(lerp(x00, x10, f.y), lerp(x01, x11, f.y), f.z)
    }
}

/// `x` to the power one and a half, as `libm::powf` gives it to within a rounding, a
/// tenth of the time: masks raise a fraction so at most corners.
pub(super) fn three_halves(x: f32) -> f32 {
    x * x.sqrt()
}

/// Where a point is among a block kind's lifts so tall: how many lifts up, the seams
/// wandering a few centimetres up and down along the wall. The surface's ridges
/// (`surface::Packed`) follow it, and the masks' strata the same through their kept
/// `Noise`, which reads the same lattice, so they agree.
pub(crate) fn lift_at(p: Vec3, lift: f32) -> f32 {
    p.y / lift + 0.35 * (smooth(p, 1.1, 11) - 0.5)
}

/// Smooth value noise over a lattice so many metres across, from 0 to 1.
pub(crate) fn smooth(p: Vec3, size: f32, seed: u32) -> f32 {
    let q = p / size;
    let base = q.floor();
    let f = q - base;
    let f = f * f * (Vec3::splat(3.0) - 2.0 * f);
    let b = base.as_ivec3();
    let at = |dx: i32, dy: i32, dz: i32| {
        hash::unit(hash::of(&[
            (b.x + dx) as u32,
            (b.y + dy) as u32,
            (b.z + dz) as u32,
            seed,
        ]))
    };
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = lerp(at(0, 0, 0), at(1, 0, 0), f.x);
    let x10 = lerp(at(0, 1, 0), at(1, 1, 0), f.x);
    let x01 = lerp(at(0, 0, 1), at(1, 0, 1), f.x);
    let x11 = lerp(at(0, 1, 1), at(1, 1, 1), f.x);
    lerp(lerp(x00, x10, f.y), lerp(x01, x11, f.y), f.z)
}

/// How high moss climbs a timber wall from the ground, in metres, its top wandering.
pub(super) const TIMBER_MOSS_M: f32 = 0.7;

/// A corner's shade by the kit's masks: dirt and damp rising unevenly from the ground,
/// outside edges worn lighter by their normal's turn, a block kind's lifts and its storey's
/// top paler, dry under the eaves, and, for timber, grey higher up.
pub(super) fn masked(
    masks: &Masks,
    (timber, layered): (bool, bool),
    p: Vec3,
    normal: Vec3,
    ground: f32,
    noise: &mut Noise,
) -> [f32; 3] {
    let mut c = Vec3::ONE;
    // A block kind's lifts, each a little lighter or darker, wandering along the wall.
    if layered && masks.strata > 0.0 {
        // Each lift its own shade, a darker seam where one was rammed on the last, the
        // seams wandering a few centimetres up and down along the wall.
        let lift = masks.lift.max(0.05);
        let y = p.y / lift + 0.35 * (noise.at(p, 1.1, 11) - 0.5);
        let layer = libm::floorf(y);
        let within = y - layer;
        let own = hash::unit(hash::of(&[layer as i32 as u32, 17])) - 0.5;
        let seam = (1.0 - (within.min(1.0 - within) / 0.06).min(1.0)) * 0.7;
        // Within a lift, finer layers where each basketful was rammed, fainter.
        let fine = libm::sinf(y * std::f32::consts::TAU * 3.0 + 2.0 * noise.at(p, 0.7, 15)) * 0.2;
        // Grit in coarse, sparse patches, not an even speckle.
        let speckle = noise.at(p, 0.55, 13) - 0.5;
        c *= 1.0 + masks.strata * (own * 1.4 + fine + speckle * 0.35 - seam);
    }
    let height = p.y - ground;
    // Paler from a storey's height above the ground up, dry under the eaves and out of
    // reach of the splash, the line wandering along the wall.
    if layered && masks.dry > 0.0 {
        let reach = masks.dry_reach.max(0.1);
        let wander = reach * 0.35 * (noise.at(p, 0.9, 29) - 0.5);
        let dry = (1.0 - (terra_build::STOREY_M - height + wander) / reach).clamp(0.0, 1.0);
        if dry > 0.0 {
            let pale = masks.dry * dry * dry * (0.7 + 0.6 * noise.at(p, 0.3, 31));
            let luma = (c.x + c.y + c.z) / 3.0;
            c = c.lerp(Vec3::splat(luma), pale * 0.5) * (1.0 + pale);
        }
    }
    // Dirt rises at most 1.4 times its height, so above that it is not worked out.
    if masks.dirt > 0.0 && height < masks.dirt * 1.4 {
        let rise = masks.dirt * (0.6 + 0.8 * noise.at(p, 0.45, 3));
        let dirt = three_halves((1.0 - height / rise).clamp(0.0, 1.0));
        c = c.lerp(Vec3::from_array(masks.dirt_tint), dirt);
    }
    // Timber's foot, where the splash keeps it damp: moss in blotches, thinning upward.
    if timber && masks.moss > 0.0 && height < TIMBER_MOSS_M * 1.4 {
        let rise = TIMBER_MOSS_M * (0.6 + 0.8 * noise.at(p, 0.5, 41));
        let low = (1.0 - height / rise).clamp(0.0, 1.0);
        let blotch = ((noise.at(p, 0.22, 43) - 0.35) / 0.4).clamp(0.0, 1.0);
        let moss = (masks.moss * low * blotch).clamp(0.0, 0.85);
        c = c.lerp(c * Vec3::from_array(masks.moss_tint), moss);
    }
    if masks.wear > 0.0 {
        let a = normal.abs();
        let round = ((1.0 - a.x.max(a.y).max(a.z)) / 0.29).clamp(0.0, 1.0);
        c *= 1.0 + masks.wear * round;
    }
    if timber && masks.weather > 0.0 {
        let grey = masks.weather * (height / 3.0).clamp(0.0, 1.0) * (0.5 + noise.at(p, 0.8, 5));
        let luma = (c.x + c.y + c.z) / 3.0;
        c = c.lerp(Vec3::splat(luma * 1.05), grey.clamp(0.0, 1.0));
    }
    (c * underside(normal)).to_array()
}

/// How much light a face looking down keeps: none of the sky's, and of the ground's only
/// what the wall it stands out from, or the opening it spans, lets it see. The sky light
/// lights an underside by the sunlit ground's whole bounce (`terra_world::skylight`), which
/// left a lintel's soffit and every quoin's underside lit brighter and greener than the
/// wall's shaded face beside it; a modelled piece bakes this occlusion into its texture, as
/// the logs' grooves are shaded (D262).
pub(super) fn underside(normal: Vec3) -> f32 {
    let t = ((-normal.y - UNDER_FROM) / (1.0 - UNDER_FROM)).clamp(0.0, 1.0);
    terra_build::light_share(1.0 - UNDERSIDE * t * t * (3.0 - 2.0 * t))
}

/// Faces within about 55 degrees of straight down are shaded, the most `UNDERSIDE` darker.
pub(super) const UNDER_FROM: f32 = 0.55;
pub(super) const UNDERSIDE: f32 = 0.65;

/// Where the rain marks a block kind's wall (D215), from its cubes: the damp splashed up
/// over its own foot, and the streaks drawn down under a sill or an open top; each column's
/// foot and each cell's source looked up once.
pub(super) struct Rained<'a> {
    pub(super) base: &'a Base,
    pub(super) kind: terra_build::Kind,
    pub(super) feet: BTreeMap<[i32; 3], f32>,
    pub(super) sources: BTreeMap<[i32; 3], Option<f32>>,
}

impl Rained<'_> {
    fn ours(&self, at: IVec3) -> bool {
        self.base
            .cube(at)
            .is_some_and(|cube| cube.kind == self.kind && layout::drawn_fill(&cube) == Fill::Solid)
    }

    /// The cell of the wall a point of its face is on: a little in from the face.
    fn cell(p: Vec3, n: Vec3) -> IVec3 {
        let level = Vec3::new(n.x, 0.0, n.z).normalize_or_zero();
        ((p - level * (CELL_M * 0.5)) / CELL_M).floor().as_ivec3()
    }

    /// The bottom of the wall under a cell, in metres: as far down as the kind goes.
    fn foot(&mut self, cell: IVec3) -> f32 {
        if let Some(&foot) = self.feet.get(&cell.to_array()) {
            return foot;
        }
        let mut at = cell;
        for _ in 0..terra_build::STOREY_CUBES * 2 {
            if !self.ours(at - IVec3::Y) {
                break;
            }
            at -= IVec3::Y;
        }
        let foot = at.y as f32 * CELL_M;
        self.feet.insert(cell.to_array(), foot);
        foot
    }

    /// The height a streak runs down from over a cell: the foot of an opening or the open
    /// top of the wall over it, within reach; none under a roof or another kind.
    fn source(&mut self, cell: IVec3, reach: f32) -> Option<f32> {
        if let Some(&found) = self.sources.get(&cell.to_array()) {
            return found;
        }
        let mut found = None;
        let cells = libm::ceilf(reach / CELL_M) as i32;
        for up in 1..=cells.max(1) {
            let at = cell + IVec3::Y * up;
            if self.ours(at) {
                continue;
            }
            // An opening's foot, or a top open to the sky, not under the eaves of a roof.
            let open = match self.base.cube(at) {
                None => (1..=terra_build::STOREY_CUBES)
                    .all(|over| self.base.cube(at + IVec3::Y * over).is_none()),
                Some(cube) => cube.kind == self.kind,
            };
            if open {
                found = Some(at.y as f32 * CELL_M);
            }
            break;
        }
        self.sources.insert(cell.to_array(), found);
        found
    }

    /// What a point of the wall's face is shaded by: darker over its foot, and in streaks
    /// under a sill or an open top. A face in a dug room (`floor`, the top of the floor it
    /// looks out over, `Dug::ground`) is splashed from that floor where it is over the wall's
    /// foot: a dungeon's walls run on down through its floor slab into the walls of the level
    /// under it, so their foot was metres under the floor and the splash never showed (barrow
    /// rounds 11 and 12: clean wall feet everywhere).
    pub(super) fn shade(&mut self, masks: &Masks, p: Vec3, n: Vec3, floor: Option<f32>) -> Vec3 {
        let mut k = Vec3::ONE;
        // Only the sides: a top is not splashed nor streaked.
        if n.y.abs() > 0.7 {
            return k;
        }
        let cell = Self::cell(p, n);
        let tint = Vec3::from_array(masks.dirt_tint);
        if masks.splash > 0.0 {
            let foot = self.foot(cell);
            let up = p.y - floor.map_or(foot, |floor| floor.max(foot));
            let reach = masks.splash * (0.7 + 0.6 * smooth(p, 0.35, 51));
            let wet = libm::powf((1.0 - up / reach).clamp(0.0, 1.0), 0.8);
            k = k.lerp(tint * (1.0 - masks.splash_dark), wet);
        }
        if masks.streaks > 0.0
            && let Some(from) = self.source(cell, masks.streak_reach)
        {
            let down = from - p.y;
            if down > 0.0 && down < masks.streak_reach {
                // Long and narrow, as water runs: stretched up the wall, fine across it.
                let across = Vec3::new(p.x * 9.0, p.y * 0.7, p.z * 9.0);
                let streak = ((smooth(across, 1.0, 53) - 0.42) * 3.0).clamp(0.0, 1.0);
                let fade = 1.0 - down / masks.streak_reach;
                k = k.lerp(k * tint, masks.streaks * streak * fade);
            }
        }
        k
    }
}

/// How many cubes down from the cell a face under the ground looks into its floor is sought,
/// sixteen metres: deeper than any dug room is tall.
pub(super) const DUG_REACH: i32 = 32;

/// The floors that faces under the ground stand over. A face under its column's ground is
/// seen only where the ground is dug out, as a dungeon's rooms are, and there its dirt and
/// damp rise from the floor in front of it, not from the ground metres over the room. A
/// dungeon's base samples no ground, so its columns keep the base's level over its walls,
/// and every wall took its dirt's whole colour: the crypt's walls drawn about 7 stops under
/// mid-grey against D488's 3 to 6 (dungeons r24, r25).
pub(super) struct Dug<'a> {
    pub(super) base: &'a Base,
    /// The floor's top under each cell looked into, if one is within reach.
    pub(super) floors: BTreeMap<[i32; 3], Option<f32>>,
}

impl Dug<'_> {
    /// The ground a point of a face is dressed from: its column's, or where the point is
    /// under that, the top of the first solid cube under the cell the face looks into, if
    /// there is one within reach (none outside a building, so a footing's face is as it was).
    pub(super) fn ground(&mut self, p: Vec3, n: Vec3, ground: f32) -> f32 {
        if p.y >= ground {
            return ground;
        }
        let cell = ((p + n * (CELL_M * 0.5)) / CELL_M).floor().as_ivec3();
        let base = self.base;
        let floor = *self.floors.entry(cell.to_array()).or_insert_with(|| {
            (0..DUG_REACH)
                .map(|down| cell - IVec3::Y * down)
                .find(|&at| {
                    base.cube(at)
                        .is_some_and(|cube| layout::drawn_fill(&cube) == Fill::Solid)
                })
                .map(|at| (at.y + 1) as f32 * CELL_M)
        });
        floor.map_or(ground, |floor| floor.min(ground))
    }
}

/// The walls round a floor's top (`Masks::grime`): for each cell over a top, which cubes beside
/// it, `reach` cells each way at its height, are solid, a bit each.
pub(super) struct Grimed<'a> {
    pub(super) base: &'a Base,
    pub(super) reach: i32,
    pub(super) walls: BTreeMap<[i32; 3], u64>,
}

impl Grimed<'_> {
    /// How far a point of a top is from the nearest solid cube beside the cell over it, in
    /// metres across the floor; `None` past `reach` cells.
    pub(super) fn distance(&mut self, p: Vec3, n: Vec3) -> Option<f32> {
        let cell = ((p + n * (CELL_M * 0.5)) / CELL_M).floor().as_ivec3();
        let (base, r) = (self.base, self.reach);
        let side = (2 * r + 1) as usize;
        let offset = |k: usize| IVec3::new((k / side) as i32 - r, 0, (k % side) as i32 - r);
        let bits = *self.walls.entry(cell.to_array()).or_insert_with(|| {
            let mut bits = 0u64;
            for k in 0..side * side {
                let solid = base
                    .cube(cell + offset(k))
                    .is_some_and(|cube| layout::drawn_fill(&cube) == Fill::Solid);
                if solid {
                    bits |= 1 << k;
                }
            }
            bits
        });
        let q = glam::Vec2::new(p.x, p.z);
        (0..side * side)
            .filter(|&k| bits & (1 << k) != 0)
            .map(|k| {
                let at = cell + offset(k);
                let low = glam::Vec2::new(at.x as f32, at.z as f32) * CELL_M;
                q.distance(q.clamp(low, low + CELL_M))
            })
            .reduce(f32::min)
    }
}

/// A kit's masks laid over a surface it drew in a material, in its base's own space.
pub fn mask(surface: &mut Surface, kit: &Kit, material: Material, base: &Base) {
    let masks = &kit.dress.masks;
    // Nothing drawn in it, so no columns to span: the table below would be empty bounds.
    if surface.positions.is_empty() && surface.shared.positions.is_empty() {
        return;
    }
    let timber = matches!(
        material,
        Material::Log | Material::LogEnd | Material::Board | Material::Daub
    );
    let layered = material == Material::Masonry;
    // The ground's height by column, looked up once each, in a table over the columns the
    // surface stands on.
    let column = |p: Vec3| [(p.x / CELL_M).floor() as i32, (p.z / CELL_M).floor() as i32];
    let (low, high) = surface
        .positions
        .iter()
        .chain(&surface.shared.positions)
        .map(|&p| column(p))
        .fold(([i32::MAX; 2], [i32::MIN; 2]), |(l, h), c| {
            (
                [l[0].min(c[0]), l[1].min(c[1])],
                [h[0].max(c[0]), h[1].max(c[1])],
            )
        });
    let wide = (high[0] - low[0] + 1).max(0) as usize;
    let deep = (high[1] - low[1] + 1).max(0) as usize;
    let mut known: Vec<f32> = vec![f32::NAN; wide * deep];
    let mut ground = |p: Vec3| {
        let at = column(p);
        let slot = (at[0] - low[0]) as usize + (at[1] - low[1]) as usize * wide;
        if known[slot].is_nan() {
            known[slot] = layout::metres(layout::roles::ground_ticks(base, at));
        }
        known[slot]
    };
    let mut noise = Noise::default();
    let mut rained = Rained {
        base,
        kind: kit.kind(),
        feet: BTreeMap::new(),
        sources: BTreeMap::new(),
    };
    let rains = layered && (masks.splash > 0.0 || masks.streaks > 0.0);
    let mut dug = Dug {
        base,
        floors: BTreeMap::new(),
    };
    // Reach 3 cells at most: 49 cubes round a cell, in a u64's bits.
    let mut grimed = Grimed {
        base,
        reach: (libm::ceilf(masks.grime * 1.3 / CELL_M) as i32).clamp(1, 3),
        walls: BTreeMap::new(),
    };
    let mut shade = |p: Vec3, n: Vec3, c: &mut [f32; 3]| {
        let column = ground(p);
        let ground = dug.ground(p, n, column);
        let mut k = Vec3::from_array(masked(masks, (timber, layered), p, n, ground, &mut noise));
        if rains {
            k *= rained.shade(masks, p, n, (ground < column).then_some(ground));
        }
        if masks.grime > 0.0
            && n.y > 0.7
            && let Some(d) = grimed.distance(p, n)
        {
            // The edge wanders, and the grime lies in blotches, never a ruled band.
            let reach = masks.grime * (0.7 + 0.6 * noise.at(p, 0.6, 61));
            let blotch = 0.65 + 0.35 * noise.at(p, 0.25, 63);
            let grime = three_halves((1.0 - d / reach).clamp(0.0, 1.0)) * blotch;
            k = k.lerp(k * Vec3::from_array(masks.dirt_tint), grime);
        }
        *c = [c[0] * k.x, c[1] * k.y, c[2] * k.z];
    };
    for ((p, n), c) in surface
        .positions
        .iter()
        .zip(&surface.normals)
        .zip(surface.colours.iter_mut())
    {
        shade(*p, *n, c);
    }
    let shared = &mut surface.shared;
    for ((p, n), c) in shared
        .positions
        .iter()
        .zip(&shared.normals)
        .zip(shared.colours.iter_mut())
    {
        shade(*p, *n, c);
    }
}

/// What a roof over a wall hides of the sky from what faces up under it, shaded as baked
/// occlusion would (D320): a log's or a stone's upper face under an eave, a verge or a
/// ceiling sees the roof's underside, not the sky, which lit the worn pale tops of logs on a
/// gable and indoors blue-white. Up to `SHELTERED` darker for a face looking straight up
/// with a roof close over it, fading with the roof's height over it to `SHELTER_REACH_M`.
pub fn sheltered(surface: &mut Surface, roofs: &terra_build_layout::layout::roofs::Roofs) {
    if roofs.is_empty() {
        return;
    }
    let shade = |p: Vec3, n: Vec3, c: &mut [f32; 3]| {
        let up = n.y.max(0.0);
        if up < 0.05 {
            return;
        }
        // A hair out from the face, so a wall's own top under its roof's eave counts.
        let at = p + Vec3::new(n.x, 0.0, n.z) * 0.05;
        let Some(top) = roofs.top_over(at.x, at.z, p.y + 0.05) else {
            return;
        };
        let near = 1.0 - ((top - p.y) / SHELTER_REACH_M).clamp(0.0, 1.0);
        let k = terra_build::light_share(1.0 - SHELTERED * up * (0.4 + 0.6 * near));
        *c = c.map(|v| v * k);
    };
    for ((p, n), c) in surface
        .positions
        .iter()
        .zip(&surface.normals)
        .zip(surface.colours.iter_mut())
    {
        shade(*p, *n, c);
    }
    let shared = &mut surface.shared;
    for ((p, n), c) in shared
        .positions
        .iter()
        .zip(&shared.normals)
        .zip(shared.colours.iter_mut())
    {
        shade(*p, *n, c);
    }
}

/// How much darker a face looking up under a roof is, at most, and how high over it a roof
/// still shades it.
pub(super) const SHELTERED: f32 = 0.4;
pub(super) const SHELTER_REACH_M: f32 = 4.0;

/// A kit's masks laid over a roof's covering and its mortar, in its base's own space, as
/// the role pieces' are (D215): toward its eaves, where the rain runs off and lingers, the
/// covering greened by moss and lichen in blotches; up toward its ridge and hips greyed by
/// the sun and weather, as timber is; along its eaves and verges, its edges worn lighter.
/// Where each point is on its roof is `roofs`'s to say.
pub fn roof_mask(
    surface: &mut Surface,
    kit: &Kit,
    roofs: &terra_build_layout::layout::roofs::Roofs,
) {
    let masks = &kit.dress.masks;
    if masks.moss <= 0.0 && masks.weather <= 0.0 && masks.wear <= 0.0 {
        return;
    }
    let mut noise = Noise::default();
    let mut shade = |p: Vec3, c: &mut [f32; 3]| {
        let Some((up, inside)) = roofs.place_of(p) else {
            return;
        };
        let up: f32 = up.clamp(-0.5, 1.0);
        let mut colour = Vec3::from_array(*c);
        if masks.moss > 0.0 {
            let blotch = noise.at(p, 0.6, 17) * 0.7 + noise.at(p, 2.1, 19) * 0.6;
            let low = three_halves((1.0 - up).clamp(0.0, 1.0));
            let moss = (masks.moss * low * blotch * 1.8).clamp(0.0, 0.85);
            colour = colour.lerp(colour * Vec3::from_array(masks.moss_tint), moss);
        }
        if masks.weather > 0.0 {
            let grey = masks.weather * up.max(0.0) * (0.4 + noise.at(p, 1.3, 23));
            let luma = (colour.x + colour.y + colour.z) / 3.0;
            colour = colour.lerp(Vec3::splat(luma * 1.08), grey.clamp(0.0, 1.0));
        }
        // Lighter along its open edges, which take the weather first.
        let worn = masks.wear.max(masks.weather * 0.5);
        let edge = (1.0 - inside / 0.5).clamp(0.0, 1.0);
        colour *= 1.0 + worn * edge * (0.5 + noise.at(p, 0.4, 29));
        *c = colour.to_array();
    };
    for (p, c) in surface.positions.iter().zip(surface.colours.iter_mut()) {
        shade(*p, c);
    }
    let shared = &mut surface.shared;
    for (p, c) in shared.positions.iter().zip(shared.colours.iter_mut()) {
        shade(*p, c);
    }
}
