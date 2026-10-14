//! What laying a part means in operations (D139, D152; docs/building.md, planned then
//! constructed): a room, a wall, a floor or a roof of cubes (D241) is laid as plans of its
//! cubes, each its
//! own part, where nothing is yet; on the ground, with footings under it down to the ground
//! as a part of their own, laid first so they are built first (D194); an opening is planned
//! cubes made open, or built cubes marked to be carved out; a door or window is fitted into
//! an opening. The hammer and the remote both lay through these, so they lay the same. The
//! ground under a plan is filled in by whoever acts, from the planet.

use std::collections::{BTreeMap, BTreeSet};
use terra_build::ground::{cube_cm, footing as footing_cubes};
use terra_build::ops::{Op, Planned};
use terra_build::roofer::{self, Style};
use terra_build::{
    Base, Facing, Fill, Kind, Panel, PartId, Pending, Rules, STOREY_CUBES, Shape, Terms,
    floor_layer, storey_of, wall_layer,
};

use bevy::math::{IVec2, IVec3};

/// A plan of these cubes in a part, leaving out any already there; none if all are.
fn plan(
    base: &Base,
    part: PartId,
    kind: Kind,
    cubes: impl IntoIterator<Item = IVec3>,
) -> Option<Op> {
    shaped(
        base,
        part,
        kind,
        cubes.into_iter().map(|at| (at, Shape::Whole)),
    )
}

/// A plan of these cubes, each cut to its shape (D193), in a part, leaving out any already
/// there; none if all are.
fn shaped(
    base: &Base,
    part: PartId,
    kind: Kind,
    cubes: impl IntoIterator<Item = (IVec3, Shape)>,
) -> Option<Op> {
    let mut seen = BTreeSet::new();
    let cubes: Vec<Planned> = cubes
        .into_iter()
        .filter(|&(at, _)| base.cube(at).is_none() && base.prop_in(at).is_none())
        .filter(|(at, _)| seen.insert(at.to_array()))
        .map(|(at, shape)| Planned {
            at: at.to_array(),
            shape,
        })
        .collect();
    (!cubes.is_empty()).then(|| Op::LayPlan {
        part: Some(part),
        kind,
        cubes,
        ground: Vec::new(),
    })
}

/// Footings under what is laid on the ground (D194): their kind, the ground's height
/// under a column, in centimetres in the base's own space, and the block kinds, whose
/// floors stand on footings of their own kind.
#[derive(Clone, Copy)]
pub struct Footings<'a> {
    pub kind: Kind,
    pub ground_cm: &'a dyn Fn([i32; 2]) -> i16,
    pub blocks: &'a BTreeSet<Kind>,
}

impl Footings<'_> {
    /// The footings under a floor of a kind: of the floor's kind where it is a block kind,
    /// as a stone floor stands on stone; else of their own.
    fn under(self, floor: Kind) -> Self {
        Footings {
            kind: if self.blocks.contains(&floor) {
                floor
            } else {
                self.kind
            },
            ..self
        }
    }

    /// A footing's plan under columns, each from its top layer down to the ground.
    fn plan(
        &self,
        base: &Base,
        part: PartId,
        columns: impl IntoIterator<Item = ([i32; 2], i32)>,
    ) -> Option<Op> {
        plan(
            base,
            part,
            self.kind,
            footing_cubes(base, columns, self.ground_cm),
        )
    }
}

/// Numbers parts from the base's next, in the order they are laid.
struct Numbers(u32);

impl Numbers {
    fn of(base: &Base) -> Self {
        Numbers(base.next_part().0)
    }

    fn next(&mut self) -> PartId {
        self.0 += 1;
        PartId(self.0 - 1)
    }
}

/// A wall tile's cubes, a storey high, its top included.
fn column(tile: IVec3) -> impl Iterator<Item = IVec3> {
    let foot = wall_layer(tile.y);
    (foot..foot + STOREY_CUBES).map(move |y| IVec3::new(tile.x, y, tile.z))
}

/// The room shortcut (D152) over a block of tiles, its corners both included, on a storey:
/// a floor under all of it, a wall along each side, and a flat roof over the inside, each
/// its own part, where nothing is yet. On the ground storey with footings, a footing under
/// its walls from the floor's layer down to the ground comes first, and the floor is laid
/// inside them.
pub fn room(
    base: &Base,
    min: IVec3,
    max: IVec3,
    walls: Kind,
    floor: Kind,
    footings: Option<Footings>,
) -> Vec<Op> {
    let mut parts = Numbers::of(base);
    let y = min.y;
    let tiles = |x0: i32, x1: i32, z0: i32, z1: i32| {
        (x0..=x1).flat_map(move |x| (z0..=z1).map(move |z| IVec3::new(x, y, z)))
    };
    let mut ops = Vec::new();
    let footings = footings.filter(|_| y == 0).map(|f| f.under(floor));
    let edge =
        |tile: IVec3| tile.x == min.x || tile.x == max.x || tile.z == min.z || tile.z == max.z;
    if let Some(footings) = footings {
        let columns = tiles(min.x, max.x, min.z, max.z)
            .filter(|&tile| edge(tile))
            .map(|tile| ([tile.x, tile.z], floor_layer(y)));
        ops.extend(footings.plan(base, parts.next(), columns));
    }
    let under = tiles(min.x, max.x, min.z, max.z)
        .filter(|&tile| footings.is_none() || !edge(tile))
        .map(|tile| tile.with_y(floor_layer(y)));
    ops.extend(plan(base, parts.next(), floor, under));
    // North and south run the whole width; east and west between them.
    let sides = [
        tiles(min.x, max.x, min.z, min.z).collect::<Vec<_>>(),
        tiles(min.x, max.x, max.z, max.z).collect(),
        tiles(min.x, min.x, min.z + 1, max.z - 1).collect(),
        tiles(max.x, max.x, min.z + 1, max.z - 1).collect(),
    ];
    for side in sides {
        let cubes = side.into_iter().flat_map(column);
        ops.extend(plan(base, parts.next(), walls, cubes));
    }
    let over = tiles(min.x + 1, max.x - 1, min.z + 1, max.z - 1)
        .map(|tile| tile.with_y(floor_layer(y + 1)));
    ops.extend(plan(base, parts.next(), walls, over));
    ops
}

/// A wall along these tiles, a storey high, as one part; on the ground storey with
/// footings, a footing under it from the floor's layer down to the ground first.
pub fn wall(base: &Base, tiles: &[IVec3], kind: Kind, footings: Option<Footings>) -> Vec<Op> {
    let mut parts = Numbers::of(base);
    let mut ops = Vec::new();
    if let Some(footings) = footings {
        let columns = tiles
            .iter()
            .filter(|tile| tile.y == 0)
            .map(|tile| ([tile.x, tile.z], floor_layer(0)));
        ops.extend(footings.plan(base, parts.next(), columns));
    }
    ops.extend(plan(
        base,
        parts.next(),
        kind,
        tiles.iter().copied().flat_map(column),
    ));
    ops
}

/// A floor under a block of tiles on a storey, as one part; on the ground storey with
/// footings, a footing under its edges down to the ground first, where the ground is below
/// it.
pub fn floor(
    base: &Base,
    min: IVec3,
    max: IVec3,
    kind: Kind,
    footings: Option<Footings>,
) -> Vec<Op> {
    let mut parts = Numbers::of(base);
    let layer = floor_layer(min.y);
    let mut ops = Vec::new();
    if let Some(footings) = footings.filter(|_| min.y == 0).map(|f| f.under(kind)) {
        let edge = |x: i32, z: i32| x == min.x || x == max.x || z == min.z || z == max.z;
        let columns = (min.x..=max.x)
            .flat_map(|x| (min.z..=max.z).map(move |z| [x, z]))
            .filter(|&[x, z]| edge(x, z))
            .filter(|&column| layer * cube_cm() > i32::from((footings.ground_cm)(column)))
            .map(|column| (column, layer - 1));
        ops.extend(footings.plan(base, parts.next(), columns));
    }
    let cubes = (min.x..=max.x).flat_map(|x| (min.z..=max.z).map(move |z| IVec3::new(x, layer, z)));
    ops.extend(plan(base, parts.next(), kind, cubes));
    ops
}

/// The layer of cubes the floor of the storey over a cube is: a flight climbs to it, a post
/// carries it. A cube in a storey's floor layer, as one set into ground lower than the
/// base's floor is, climbs to the floor over that storey.
fn floor_over(cube: IVec3) -> i32 {
    floor_layer(storey_of(cube.y + 1) + 1)
}

/// A flight of stairs or a ramp (D193), as one part, from its foot at `foot`, the first
/// cube over the floor or ground it starts on, rising toward `facing` a cube a column up to
/// the floor of the storey above, `wide` columns across: each column whole cubes under a
/// stair or a slope. On the ground storey with footings, a footing under it first.
pub fn flight(
    base: &Base,
    foot: IVec3,
    facing: Facing,
    wide: i32,
    top: Shape,
    kind: Kind,
    footings: Option<Footings>,
) -> Vec<Op> {
    let mut parts = Numbers::of(base);
    let rise = (floor_over(foot) - foot.y + 1).max(1);
    let (forward, across) = (facing.step(), facing.turned(1).step());
    let columns: Vec<IVec3> = (0..rise)
        .flat_map(|i| (0..wide.max(1)).map(move |w| foot + forward * i + across * w + IVec3::Y * i))
        .collect();
    let mut ops = Vec::new();
    if let Some(footings) = footings.filter(|_| storey_of(foot.y) == 0) {
        let under = columns.iter().map(|top| ([top.x, top.z], foot.y - 1));
        ops.extend(footings.plan(base, parts.next(), under));
    }
    let cubes = columns.iter().flat_map(|&top_cube| {
        (foot.y..=top_cube.y).map(move |y| {
            let at = top_cube.with_y(y);
            (at, if y == top_cube.y { top } else { Shape::Whole })
        })
    });
    ops.extend(shaped(base, parts.next(), kind, cubes));
    ops
}

/// A post (D193), as one part: a column of post cubes from `foot` up to the floor of the
/// storey above, which it carries. On the ground storey with footings, a footing under it.
pub fn post(base: &Base, foot: IVec3, kind: Kind, footings: Option<Footings>) -> Vec<Op> {
    let mut parts = Numbers::of(base);
    let mut ops = Vec::new();
    if let Some(footings) = footings.filter(|_| storey_of(foot.y) == 0) {
        ops.extend(footings.plan(base, parts.next(), [([foot.x, foot.z], foot.y - 1)]));
    }
    let cubes = (foot.y..=floor_over(foot)).map(|y| (foot.with_y(y), Shape::Post));
    ops.extend(shaped(base, parts.next(), kind, cubes));
    ops
}

/// One cube of a shape, placed as Enshrouded's blocks are: in the part of the cubes of its
/// kind beside it laid on the same terms, built or planned, every such part it touches
/// merged into one (D260), so a mass built cube by cube is one part; else a part of its own.
/// On the ground, with footings, a footing under it down to the ground, as under any wall
/// (D194).
pub fn block(
    base: &Base,
    at: IVec3,
    shape: Shape,
    kind: Kind,
    rules: &Rules,
    footings: Option<Footings>,
) -> Vec<Op> {
    laid_by_hand(base, kind, rules, footings, vec![(at, shape)])
}

/// Cubes laid by hand in a kind on its terms: footings first under those resting on the
/// ground, then the cubes, each in the part they join, merging the parts they touch that
/// agree, or a part of their own (D194, D260).
fn laid_by_hand(
    base: &Base,
    kind: Kind,
    rules: &Rules,
    footings: Option<Footings>,
    cubes: Vec<(IVec3, Shape)>,
) -> Vec<Op> {
    let mut parts = Numbers::of(base);
    let mut ops = Vec::new();
    if let Some(footings) = footings.map(|footings| footings.under(kind)) {
        let laying: BTreeSet<[i32; 3]> = cubes.iter().map(|(at, _)| at.to_array()).collect();
        let cube = cube_cm();
        // The lowest of each column laid, with nothing under it, its foot at the ground or
        // less than a cube over it: resting on the ground.
        let columns: Vec<([i32; 2], i32)> = cubes
            .iter()
            .map(|&(at, _)| at)
            .filter(|&at| {
                let under = at - IVec3::Y;
                let ground = i32::from((footings.ground_cm)([at.x, at.z]));
                !laying.contains(&under.to_array())
                    && base.cube(under).is_none()
                    && at.y * cube >= ground
                    && at.y * cube - ground < cube
            })
            .map(|at| ([at.x, at.z], at.y - 1))
            .collect();
        let under: Vec<(IVec3, Shape)> = footing_cubes(base, columns, footings.ground_cm)
            .into_iter()
            .map(|at| (at, Shape::Whole))
            .collect();
        if !under.is_empty() {
            ops.extend(joined(
                base,
                footings.kind,
                &rules.of(footings.kind),
                under,
                &mut parts,
            ));
        }
    }
    let laid = joined(base, kind, &rules.of(kind), cubes, &mut parts);
    if laid.is_empty() {
        return Vec::new();
    }
    ops.extend(laid);
    ops
}

/// Cubes of a kind laid in the part of the cubes beside them that agree, the parts they
/// touch merged into it (D260), or a part of their own, numbered from `parts`.
fn joined(
    base: &Base,
    kind: Kind,
    terms: &Terms,
    cubes: Vec<(IVec3, Shape)>,
    parts: &mut Numbers,
) -> Vec<Op> {
    let at: Vec<IVec3> = cubes
        .iter()
        .map(|&(at, _)| at)
        .filter(|&at| base.cube(at).is_none() && base.prop_in(at).is_none())
        .collect();
    let (part, mut ops) = base
        .joining(&at, kind, terms)
        .unwrap_or_else(|| (parts.next(), Vec::new()));
    match shaped(base, part, kind, cubes) {
        Some(lay) => ops.push(lay),
        None => return Vec::new(),
    }
    ops
}

/// The cubes of a brush `size` cubes across round a cube (D226): the box of that size on
/// the brush's own grid that holds it, as Enshrouded's larger blocks snap to theirs.
pub fn brush(at: IVec3, size: i32) -> Vec<IVec3> {
    let size = size.max(1);
    let low = at.div_euclid(IVec3::splat(size)) * size;
    let mut cubes = Vec::new();
    for x in 0..size {
        for y in 0..size {
            for z in 0..size {
                cubes.push(low + IVec3::new(x, y, z));
            }
        }
    }
    cubes
}

/// A box of cubes of a shape, a brush's (D226), laid as [`block`] lays one: whole cubes,
/// a cut shape on the box's face it is cut on, its top layer, or its bottom for an upper
/// half or a shape upside down, so a box of slopes is a ramp on a block, not a saw's teeth.
pub fn blocks(
    base: &Base,
    cubes: &[IVec3],
    shape: Shape,
    kind: Kind,
    rules: &Rules,
    footings: Option<Footings>,
) -> Vec<Op> {
    let Some(first) = cubes.first() else {
        return Vec::new();
    };
    if cubes.len() == 1 {
        return block(base, *first, shape, kind, rules, footings);
    }
    let (low, high) = cubes
        .iter()
        .fold((*first, *first), |(lo, hi), &c| (lo.min(c), hi.max(c)));
    let under = shape.planed_under() || matches!(shape, Shape::InvertedStair(_));
    let cut: Vec<(IVec3, Shape)> = cubes
        .iter()
        .map(|&at| {
            let face = if under { at.y == low.y } else { at.y == high.y };
            (at, if face { shape } else { Shape::Whole })
        })
        .collect();
    laid_by_hand(base, kind, rules, footings, cut)
}

/// The built or part-built cubes of a box changed to a kind (D232): marked, the old taken
/// out, swapped and the new built, as a part of its own; none if none can be.
pub fn replace(base: &Base, cubes: &[IVec3], kind: Kind) -> Vec<Op> {
    let changed: Vec<[i32; 3]> = cubes
        .iter()
        .filter(|&&at| {
            base.cube(at).is_some_and(|cube| {
                cube.fill == Fill::Solid && cube.pending == Pending::None && cube.kind != kind
            })
        })
        .map(|at| at.to_array())
        .collect();
    if changed.is_empty() {
        return Vec::new();
    }
    vec![Op::Replace {
        cubes: changed,
        kind,
        part: Some(Numbers::of(base).next()),
    }]
}

/// The cubes of a box with anything in them marked to be carved out (D226); with footings,
/// steps planned from any doorway that makes (D261).
pub fn carve(base: &Base, cubes: &[IVec3], footings: Option<Footings>) -> Vec<Op> {
    let marked: Vec<[i32; 3]> = cubes
        .iter()
        .filter(|&&at| {
            base.cube(at)
                .is_some_and(|cube| cube.is_in() && cube.pending == Pending::None)
        })
        .map(|at| at.to_array())
        .collect();
    if marked.is_empty() {
        return Vec::new();
    }
    let opened: Vec<IVec3> = marked.iter().map(|&at| IVec3::from_array(at)).collect();
    let mut ops = vec![Op::Carve { cubes: marked }];
    ops.extend(steps(base, &opened, footings));
    ops
}

/// Taking out what is in a box (D226): a step out of its highest layer with anything in,
/// those taken out to nothing out of the plan with it (D193), which the base refuses if
/// anything rests on them; with nothing of it in, its planned
/// cubes taken out of the plan, and any part that leaves in pieces split (D260); none if
/// there is nothing in it to take.
pub fn take_out(base: &Base, cubes: &[IVec3]) -> Vec<Op> {
    let ins: Vec<IVec3> = cubes
        .iter()
        .copied()
        .filter(|&at| base.is_in(at) && base.cube(at).is_some_and(|c| c.pending == Pending::None))
        .collect();
    let Some(top) = ins.iter().map(|at| at.y).max() else {
        let bare: Vec<IVec3> = cubes
            .iter()
            .copied()
            .filter(|&at| {
                base.cube(at).is_some_and(|cube| {
                    cube.part.is_some()
                        && cube.fill == Fill::Solid
                        && cube.level() == 0
                        && cube.pending == Pending::None
                })
            })
            .collect();
        return unplan(base, &bare);
    };
    let layer: Vec<IVec3> = ins.into_iter().filter(|at| at.y == top).collect();
    base.taking_apart(&layer)
}

/// Planned cubes taken out of the plan, and the parts that leaves in pieces split (D260);
/// none for no cubes.
pub fn unplan(base: &Base, cubes: &[IVec3]) -> Vec<Op> {
    if cubes.is_empty() {
        return Vec::new();
    }
    let mut ops = vec![Op::Unplan {
        cubes: cubes.iter().map(|at| at.to_array()).collect(),
    }];
    ops.extend(base.splitting(cubes));
    ops
}

/// A flat roof over a block of tiles on a storey, as one part.
pub fn flat_roof(base: &Base, min: IVec3, max: IVec3, kind: Kind) -> Vec<Op> {
    floor(base, min + IVec3::Y, max + IVec3::Y, kind, None)
}

/// "Add a roof with this material" (D226, D241): a roof of cubes in a roof kind planned over
/// a footprint of columns on a storey, as the roofer lays it in a style (`roofer::over`),
/// resting on the tops of the storey's walls; the walls under its gable ends carried up to
/// it, planned in the kind of each wall's top, a part a kind, laid first as they hold it;
/// and a flat roof planned there, with nothing in it, giving way to it. A plan like any
/// other, built a cube a stage at a time, and changed or built over by hand.
pub fn roof(
    base: &Base,
    footprint: &BTreeSet<[i32; 2]>,
    storey: i32,
    style: Style,
    along_x: Option<bool>,
    kind: Kind,
) -> Vec<Op> {
    let top = floor_layer(storey + 1);
    let flat: Vec<[i32; 3]> = footprint
        .iter()
        .map(|&[x, z]| IVec3::new(x, top, z))
        .filter(|&at| {
            !base.in_wall(at)
                && base
                    .cube(at)
                    .is_some_and(|cube| cube.level() == 0 && cube.fill == Fill::Solid)
        })
        .map(|at| at.to_array())
        .collect();
    let mut ops = Vec::new();
    if !flat.is_empty() {
        ops.push(Op::Unplan {
            cubes: flat.clone(),
        });
    }
    // What is there once the flat roof's plan has gone.
    let gone: BTreeSet<[i32; 3]> = flat.into_iter().collect();
    let free = |at: IVec3| base.cube(at).is_none() || gone.contains(&at.to_array());
    let layer = wall_layer(storey) + STOREY_CUBES;
    let plan = roofer::over(footprint, layer, style, along_x);
    let mut parts = Numbers::of(base);
    // The gables, by the kind of the wall's top under each.
    let mut gables: BTreeMap<Kind, Vec<IVec3>> = BTreeMap::new();
    for at in plan.gables {
        let wall = (layer - STOREY_CUBES..layer)
            .rev()
            .find_map(|y| base.cube(IVec3::new(at.x, y, at.z)));
        if let Some(wall) = wall.filter(|wall| !wall.kind.is_roof())
            && free(at)
        {
            gables.entry(wall.kind).or_default().push(at);
        }
    }
    let lay = |part: PartId, kind: Kind, cubes: Vec<(IVec3, Shape)>| {
        let cubes: Vec<Planned> = cubes
            .into_iter()
            .filter(|&(at, _)| free(at) && base.prop_in(at).is_none())
            .map(|(at, shape)| Planned {
                at: at.to_array(),
                shape,
            })
            .collect();
        (!cubes.is_empty()).then(|| Op::LayPlan {
            part: Some(part),
            kind,
            cubes,
            ground: Vec::new(),
        })
    };
    for (wall, cubes) in gables {
        let cubes = cubes.into_iter().map(|at| (at, Shape::Whole)).collect();
        ops.extend(lay(parts.next(), wall, cubes));
    }
    ops.extend(lay(parts.next(), kind, plan.roof));
    ops
}

/// The footprint a roof over an enclosed space has: every column of it on its storey from
/// the tile aimed into, as far as the walls round it, planned or built, the walls and their
/// corners too; none if it is not closed in within `ROOF_REACH` tiles.
pub fn room_footprint(base: &Base, tile: IVec3) -> BTreeSet<[i32; 2]> {
    let wall = |x: i32, z: i32| base.filling(IVec3::new(x, tile.y, z)).is_some();
    if wall(tile.x, tile.z) {
        return BTreeSet::new();
    }
    let mut inside = BTreeSet::from([[tile.x, tile.z]]);
    let mut frontier = vec![[tile.x, tile.z]];
    while let Some([x, z]) = frontier.pop() {
        if (x - tile.x).abs() > ROOF_REACH || (z - tile.z).abs() > ROOF_REACH {
            return BTreeSet::new();
        }
        for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            let next = [x + dx, z + dz];
            if !wall(next[0], next[1]) && inside.insert(next) {
                frontier.push(next);
            }
        }
    }
    let mut footprint = inside.clone();
    for &[x, z] in &inside {
        for dx in -1..=1 {
            for dz in -1..=1 {
                if wall(x + dx, z + dz) {
                    footprint.insert([x + dx, z + dz]);
                }
            }
        }
    }
    footprint
}

/// How far from the tile aimed into a space may reach and still be roofed as enclosed.
pub const ROOF_REACH: i32 = 48;

/// The cubes of an opening of a panel's shape cut along a stretch of wall tiles: a door's
/// from the floor, a window's from its sill to its head.
pub fn opening_cubes(tiles: &[IVec3], panel: Panel) -> Vec<IVec3> {
    tiles
        .iter()
        .flat_map(|&tile| {
            let foot = wall_layer(tile.y);
            (0..STOREY_CUBES)
                .filter(move |&step| panel.fill(step) != Fill::Solid)
                .map(move |step| IVec3::new(tile.x, foot + step, tile.z))
        })
        .collect()
}

/// An opening into a wall: its planned cubes with nothing in them made open, and those with
/// something in marked to be carved out, which building then takes out a course at a time
/// from under the wall over it and makes open (construction-001, experience 4). Cubes
/// already open or marked are left as they are; what cannot be is refused by the
/// operation, with its reason.
/// With footings, the doorways it makes have steps planned down to the ground (D261).
pub fn opening(base: &Base, cubes: &[IVec3], footings: Option<Footings>) -> Vec<Op> {
    let (mut open, mut carve) = (Vec::new(), Vec::new());
    for &at in cubes {
        match base.cube(at) {
            Some(cube) if cube.fill == Fill::Open || cube.pending == Pending::Carve => {}
            Some(cube) if cube.level() > 0 => carve.push(at.to_array()),
            _ => open.push(at.to_array()),
        }
    }
    let mut ops = Vec::new();
    if !open.is_empty() {
        ops.push(Op::Open { cubes: open });
    }
    if !carve.is_empty() {
        ops.push(Op::Carve { cubes: carve });
    }
    if !ops.is_empty() {
        ops.extend(steps(base, cubes, footings));
    }
    ops
}

/// Door steps (D261): stair and half cubes of the footings' kind, a part of their own, from
/// each doorway that opening these cubes makes, or that has none yet, down to the ground,
/// as the footings take it; none without footings, or where none is wanted.
pub fn steps(base: &Base, opening: &[IVec3], footings: Option<Footings>) -> Option<Op> {
    let footings = footings?;
    let opening: BTreeSet<[i32; 3]> = opening.iter().map(|at| at.to_array()).collect();
    let cubes = base.door_steps(&opening, footings.ground_cm);
    shaped(base, Numbers::of(base).next(), footings.kind, cubes)
}

/// The opening a cube is part of: every open cube joined to it, side by side and up and
/// down, as the block from its lowest to its highest; `None` if it is not open.
pub fn hole(base: &Base, cube: IVec3) -> Option<(IVec3, IVec3)> {
    let open = |at: IVec3| base.cube(at).is_some_and(|cube| cube.fill == Fill::Open);
    if !open(cube) {
        return None;
    }
    let mut seen = BTreeSet::from([cube.to_array()]);
    let mut frontier = vec![cube];
    let (mut low, mut high) = (cube, cube);
    while let Some(at) = frontier.pop() {
        low = low.min(at);
        high = high.max(at);
        for step in terra_build::support::FACES {
            let next = at + step;
            if open(next) && seen.insert(next.to_array()) {
                frontier.push(next);
            }
        }
    }
    Some((low, high))
}

/// The tiles along a wall from one to another, in the grid's four directions only (D146):
/// east and west, or north and south, whichever is further. Both ends included.
pub fn wall_tiles(from: IVec2, to: IVec2, level: i32) -> Vec<IVec3> {
    let step = to - from;
    let (along, length) = if step.x.abs() >= step.y.abs() {
        (IVec2::new(step.x.signum(), 0), step.x.abs())
    } else {
        (IVec2::new(0, step.y.signum()), step.y.abs())
    };
    (0..=length)
        .map(|i| {
            let at = from + along * i;
            IVec3::new(at.x, level, at.y)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_build::ops::Context;
    use terra_build::{Rules, State};

    const LOGS: Kind = Kind(1);
    const STONE: Kind = Kind(2);
    const SHAKES: Kind = Kind(0x800);

    #[test]
    fn a_room_is_a_floor_four_walls_and_a_roof_each_its_own_part() {
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        let ops = room(
            &base,
            IVec3::new(0, 0, 0),
            IVec3::new(9, 0, 7),
            LOGS,
            STONE,
            None,
        );
        assert_eq!(ops.len(), 6);
        base.act_all(&ops, &cx).unwrap();
        assert_eq!(base.parts().count(), 6);
        // A floor of 80 cubes, walls round the edge six high, a roof over the inside.
        assert_eq!(base.progress(PartId(0)).cubes, 80);
        let walls: usize = (1..5).map(|part| base.progress(PartId(part)).cubes).sum();
        assert_eq!(walls, 32 * 6);
        assert_eq!(base.progress(PartId(5)).cubes, 48);
        assert_eq!(base.part(PartId(0)).unwrap().kind, STONE);
        // Nothing of it is built, so it closes no room in.
        assert_eq!(base.room_cubes(), 0);
        assert!(
            base.cubes()
                .all(|(_, cube)| cube.state == State::Planned { level: 0 })
        );
        // Laid again over itself, it lays nothing.
        assert!(
            room(
                &base,
                IVec3::new(0, 0, 0),
                IVec3::new(9, 0, 7),
                LOGS,
                STONE,
                None
            )
            .is_empty()
        );
    }

    #[test]
    fn a_room_on_a_slope_stands_on_a_footing_laid_first_down_to_the_ground() {
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        // The ground falls away to the east, 20 cm a column, from the floor's level.
        let ground = |column: [i32; 2]| (-20 * column[0]) as i16;
        let footings = Footings {
            kind: STONE,
            ground_cm: &ground,
            blocks: &BTreeSet::from([STONE]),
        };
        let (min, max) = (IVec3::new(0, 0, 0), IVec3::new(9, 0, 7));
        let ops = room(&base, min, max, LOGS, LOGS, Some(footings));
        assert_eq!(ops.len(), 7, "a footing, a floor, four walls and a roof");
        base.act_all(&ops, &cx).unwrap();
        // The footing is the first part, in stone, under every column of the walls.
        let footing: Vec<IVec3> = base.cubes_of(PartId(0)).map(|(at, _)| at).collect();
        assert_eq!(base.part(PartId(0)).unwrap().kind, STONE);
        let columns: BTreeSet<[i32; 2]> = footing.iter().map(|at| [at.x, at.z]).collect();
        assert_eq!(columns.len(), 32);
        for column in columns {
            let ys: Vec<i32> = footing
                .iter()
                .filter(|at| [at.x, at.z] == column)
                .map(|at| at.y)
                .collect();
            let lowest = *ys.iter().min().unwrap();
            assert_eq!(*ys.iter().max().unwrap(), -1, "from the floor's layer");
            // Down to the cube the ground passes through, and no further.
            let ground = i32::from(ground(column));
            assert!(lowest * cube_cm() <= ground, "{column:?}");
            assert!(
                lowest == -1 || (lowest + 1) * cube_cm() > ground,
                "{column:?}"
            );
        }
        // The floor is inside the walls only, over nothing on the low side.
        assert_eq!(base.progress(PartId(1)).cubes, 48);
        // Laid again, nothing.
        assert!(room(&base, min, max, LOGS, LOGS, Some(footings)).is_empty());
        let upstairs = room(
            &base,
            min + IVec3::Y,
            max + IVec3::Y,
            LOGS,
            LOGS,
            Some(footings),
        );
        // Higher up, no footing, and its floor the flat roof below: four walls and a roof.
        assert_eq!(upstairs.len(), 5);
    }

    #[test]
    fn a_floor_of_a_block_kind_stands_on_footings_of_its_own_kind() {
        // Stone by default; a floor of another block kind, brick, on brick.
        const BRICK: Kind = Kind(3);
        let ground = |_: [i32; 2]| -60i16;
        let blocks = BTreeSet::from([STONE, BRICK]);
        let footings = Footings {
            kind: STONE,
            ground_cm: &ground,
            blocks: &blocks,
        };
        let (min, max) = (IVec3::new(0, 0, 0), IVec3::new(5, 0, 5));
        let kind_of_footing =
            |floor: Kind| match &room(&Base::default(), min, max, LOGS, floor, Some(footings))[0] {
                Op::LayPlan { kind, .. } => *kind,
                other => panic!("{other:?}"),
            };
        assert_eq!(kind_of_footing(BRICK), BRICK);
        assert_eq!(kind_of_footing(LOGS), STONE);
    }

    #[test]
    fn a_doorway_is_planned_into_a_wall_and_leaves_a_hole_of_its_size() {
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        base.act_all(
            &room(
                &base,
                IVec3::new(0, 0, 0),
                IVec3::new(9, 0, 7),
                LOGS,
                LOGS,
                None,
            ),
            &cx,
        )
        .unwrap();
        let tiles = [IVec3::new(4, 0, 7), IVec3::new(5, 0, 7)];
        let cubes = opening_cubes(&tiles, Panel::Door);
        assert_eq!(cubes.len(), 8);
        // The ground outside falls away south of the wall, 30 cm a cube from 30 cm down.
        let ground = |[_, z]: [i32; 2]| (-30 - 30 * (z - 7).max(0)) as i16;
        let footings = Footings {
            kind: STONE,
            ground_cm: &ground,
            blocks: &BTreeSet::from([STONE]),
        };
        let parts = base.parts().count();
        base.act_all(&opening(&base, &cubes, Some(footings)), &cx)
            .unwrap();
        assert_eq!(
            hole(&base, IVec3::new(5, 2, 7)),
            Some((IVec3::new(4, 0, 7), IVec3::new(5, 3, 7)))
        );
        // Steps planned down from it, stairs of stone rising to the threshold, a part of
        // their own.
        assert_eq!(base.parts().count(), parts + 1);
        for x in [4, 5] {
            let step = base.cube(IVec3::new(x, -1, 8)).unwrap();
            assert_eq!(
                (step.kind, step.shape, step.is_built()),
                (STONE, Shape::Stair(Facing::North), false)
            );
            assert!(base.cube(IVec3::new(x, -2, 9)).is_some());
        }
        // A window is cut from its sill to its head.
        let window = opening_cubes(&[IVec3::new(0, 0, 3)], Panel::WINDOW);
        assert_eq!(window.iter().map(|at| at.y).collect::<Vec<_>>(), vec![2, 3]);
        let cut = opening(&base, &window, Some(footings));
        assert_eq!(cut.len(), 1, "no steps for a window");
        base.act_all(&cut, &cx).unwrap();
        assert_eq!(
            hole(&base, IVec3::new(0, 2, 3)),
            Some((IVec3::new(0, 2, 3), IVec3::new(0, 3, 3)))
        );
    }

    #[test]
    fn a_flight_climbs_a_storey_to_the_floor_above_and_a_post_carries_it() {
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        let ground = |_: [i32; 2]| 0i16;
        let footings = Footings {
            kind: STONE,
            ground_cm: &ground,
            blocks: &BTreeSet::from([STONE]),
        };
        let ops = flight(
            &base,
            IVec3::new(0, 0, 8),
            Facing::North,
            2,
            Shape::Stair(Facing::North),
            LOGS,
            Some(footings),
        );
        assert_eq!(ops.len(), 2, "a footing and the flight");
        base.act_all(&ops, &cx).unwrap();
        // Six columns, two across, the last at the floor of the storey above.
        let tops: Vec<(IVec3, Shape)> = base
            .cubes()
            .filter(|(_, cube)| cube.kind == LOGS && cube.shape != Shape::Whole)
            .map(|(at, cube)| (at, cube.shape))
            .collect();
        assert_eq!(tops.len(), 12);
        assert!(
            tops.iter()
                .all(|(_, shape)| *shape == Shape::Stair(Facing::North))
        );
        let highest = tops.iter().map(|(at, _)| at.y).max().unwrap();
        assert_eq!(highest, floor_layer(1));
        assert_eq!(tops.iter().map(|(at, _)| at.z).min(), Some(8 - 5));
        // A post a storey high, to the floor over it, in a part of its own.
        let post_ops = post(&base, IVec3::new(4, 0, 0), LOGS, None);
        base.act_all(&post_ops, &cx).unwrap();
        let column: Vec<i32> = base
            .cubes()
            .filter(|(_, cube)| cube.shape == Shape::Post)
            .map(|(at, _)| at.y)
            .collect();
        assert_eq!(column, (0..=floor_layer(1)).collect::<Vec<_>>());
        // A block beside a cube of its kind on the same terms joins its part; one alone is
        // a part of its own (D260).
        let parts = base.parts().count();
        base.act_all(
            &block(&base, IVec3::new(5, 0, 0), Shape::Half, LOGS, &rules, None),
            &cx,
        )
        .unwrap();
        assert_eq!(base.parts().count(), parts);
        let (left, right) = (IVec3::new(9, 0, 0), IVec3::new(11, 0, 0));
        for at in [left, right] {
            base.act_all(&block(&base, at, Shape::Whole, LOGS, &rules, None), &cx)
                .unwrap();
        }
        assert_eq!(base.parts().count(), parts + 2);
        // One between them joins the two into one; taken out of the plan, they are two
        // again, and its undo puts it back as one.
        let between = IVec3::new(10, 0, 0);
        base.act_all(
            &block(&base, between, Shape::Whole, LOGS, &rules, None),
            &cx,
        )
        .unwrap();
        assert_eq!(base.parts().count(), parts + 1);
        let part_of = |base: &Base, at: IVec3| base.cube(at).unwrap().part;
        assert_eq!(part_of(&base, left), part_of(&base, right));
        let joined = base.clone();
        let done = base.act_all(&take_out(&base, &[between]), &cx).unwrap();
        assert_eq!(base.parts().count(), parts + 2);
        assert_ne!(part_of(&base, left), part_of(&base, right));
        base.act_all(&done.undo, &cx).unwrap();
        assert_eq!(base, joined);
    }

    #[test]
    fn a_roof_of_cubes_takes_the_place_of_a_flat_one_planned_and_gables_its_ends() {
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        base.act_all(
            &room(
                &base,
                IVec3::new(0, 0, 0),
                IVec3::new(9, 0, 7),
                LOGS,
                LOGS,
                None,
            ),
            &cx,
        )
        .unwrap();
        // Over the room aimed into: its tiles and walls, corners too.
        let footprint = room_footprint(&base, IVec3::new(4, 0, 4));
        assert_eq!(footprint.len(), 80);
        let ops = roof(&base, &footprint, 0, Style::Gable, None, SHAKES);
        base.act_all(&ops, &cx).unwrap();
        assert_eq!(base.part(PartId(5)), None, "the flat roof's plan is gone");
        let roofed: Vec<_> = base.cubes().filter(|(_, c)| c.kind == SHAKES).collect();
        assert_eq!(roofed.len(), 80);
        assert!(
            roofed
                .iter()
                .all(|(_, c)| c.level() == 0 && c.part.is_some())
        );
        // The gable ends of logs, a part of their own laid first.
        let gable = base.cube(IVec3::new(0, 8, 3)).unwrap();
        assert_eq!(gable.kind, LOGS);
        assert!(gable.part < roofed[0].1.part);
        // Over nothing, nothing.
        assert!(room_footprint(&base, IVec3::new(40, 0, 40)).is_empty());
    }

    #[test]
    fn a_brush_lays_changes_carves_and_takes_out_a_box_on_its_own_grid() {
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let mut base = Base::default();
        // A box of slopes four cubes across, aimed anywhere in it: whole cubes under a
        // top layer of slopes, on the brush's grid.
        let cubes = brush(IVec3::new(5, 1, -3), 4);
        assert_eq!(cubes.len(), 64);
        assert_eq!(cubes[0], IVec3::new(4, 0, -4));
        let slope = Shape::Slope(Facing::North);
        base.act_all(&blocks(&base, &cubes, slope, STONE, &rules, None), &cx)
            .unwrap();
        let shapes = |y: i32| {
            base.cubes()
                .filter(|(at, _)| at.y == y)
                .map(|(_, cube)| cube.shape.code())
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(shapes(3), BTreeSet::from([slope.code()]));
        assert_eq!(shapes(0), BTreeSet::from([Shape::Whole.code()]));
        // Upside down, the cut layer is the bottom one.
        let under = brush(IVec3::new(0, 4, 0), 2);
        base.act_all(
            &blocks(&base, &under, Shape::UpperHalf, STONE, &rules, None),
            &cx,
        )
        .unwrap();
        assert_eq!(
            base.cube(IVec3::new(0, 4, 0)).unwrap().shape,
            Shape::UpperHalf
        );
        assert_eq!(base.cube(IVec3::new(0, 5, 0)).unwrap().shape, Shape::Whole);
        // Built, a box of two in it is changed to logs, carved, or taken out from the top.
        for at in &cubes {
            base.act(
                &Op::Fill {
                    cubes: vec![at.to_array()],
                },
                &cx,
            )
            .unwrap();
        }
        let two = brush(IVec3::new(4, 2, -4), 2);
        let change = replace(&base, &two, LOGS);
        assert!(matches!(&change[..], [Op::Replace { cubes, kind: LOGS, .. }] if cubes.len() == 8));
        assert!(replace(&base, &two, STONE).is_empty());
        let carving = carve(&base, &two, None);
        assert!(matches!(&carving[..], [Op::Carve { cubes }] if cubes.len() == 8));
        let out = take_out(&base, &brush(IVec3::new(4, 3, -4), 2));
        assert!(matches!(&out[0], Op::Empty { cubes } if cubes.iter().all(|at| at[1] == 3)));
        // Stone comes out in one step, so the cubes go with their plan, nothing left behind
        // (D193).
        assert!(matches!(&out[1], Op::Unplan { cubes } if cubes.len() == 4));
        let before = base.cubes().count();
        base.act_all(&out, &cx).unwrap();
        assert_eq!(base.cubes().count(), before - 4);
    }

    /// The room tool's plans laid as built cubes at once.
    fn built(base: &mut Base, ops: Vec<Op>) {
        for op in ops {
            match op {
                Op::LayPlan { kind, cubes, .. } => {
                    for planned in cubes {
                        let cube = terra_build::Cube {
                            shape: planned.shape,
                            ..terra_build::Cube::new(kind, Fill::Solid)
                        };
                        base.apply(terra_build::BuildOp::Fill {
                            min: planned.at,
                            max: planned.at,
                            cube: Some(cube),
                        });
                    }
                }
                Op::Unplan { cubes } => {
                    for at in cubes {
                        base.apply(terra_build::BuildOp::Fill {
                            min: at,
                            max: at,
                            cube: None,
                        });
                    }
                }
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn an_l_two_storeys_high_is_a_room_a_wing_a_storey_until_a_well_joins_them() {
        // Two wings by the room tool, each with its own walls, the one between them shared,
        // and a storey over each, as the sandbox's check built them (D261).
        let mut base = Base::default();
        for (min, max) in [
            ([-30, 0, -22], [-13, 0, -14]),
            ([-30, 0, -14], [-22, 0, 0]),
            ([-30, 1, -22], [-13, 1, -14]),
            ([-30, 1, -14], [-22, 1, 0]),
        ] {
            let ops = room(
                &base,
                IVec3::from_array(min),
                IVec3::from_array(max),
                STONE,
                STONE,
                None,
            );
            built(&mut base, ops);
        }
        // A room a wing a storey, the wall between the wings and the floor between the
        // storeys closing each in: each its walls' inside, five cubes high. The sandbox's
        // third room was the other wing, not a void.
        assert_eq!(base.room_count(), 4);
        assert_eq!(base.room_cubes(), 2 * (16 * 7 * 5) + 2 * (7 * 13 * 5));
        let (a_low, a_high) = (IVec3::new(-20, 2, -18), IVec3::new(-20, 8, -18));
        let b_low = IVec3::new(-26, 2, -6);
        assert_ne!(base.room(a_low), base.room(a_high));
        assert_ne!(base.room(a_low), base.room(b_low));
        // A well through the floor between one wing's storeys joins them in one room.
        base.apply(terra_build::BuildOp::Fill {
            min: [-24, 5, -20],
            max: [-21, 5, -17],
            cube: None,
        });
        assert_eq!(base.room_count(), 3);
        assert_eq!(base.room(a_low), base.room(a_high));
        assert_ne!(base.room(a_low), base.room(b_low));
    }

    #[test]
    fn blocks_laid_by_hand_on_the_ground_stand_on_footings_and_in_the_air_on_none() {
        // The ground 30 cm under the grid's level: a block of logs at its foot rests on it
        // (D194, the third goals review's finding 7).
        let rules = Rules::default();
        let cx = Context::free(&rules);
        let ground = |_: [i32; 2]| -30i16;
        let footings = Footings {
            kind: STONE,
            ground_cm: &ground,
            blocks: &BTreeSet::from([STONE]),
        };
        let mut base = Base::default();
        let ops = block(
            &base,
            IVec3::new(3, 0, 3),
            Shape::Whole,
            LOGS,
            &rules,
            Some(footings),
        );
        assert_eq!(ops.len(), 2, "a footing, then the block: {ops:?}");
        base.act_all(&ops, &cx).unwrap();
        let footing = base.cube(IVec3::new(3, -1, 3)).unwrap();
        assert_eq!(footing.kind, STONE);
        assert_ne!(footing.part, base.cube(IVec3::new(3, 0, 3)).unwrap().part);
        // The next block beside it joins both parts: its footing the footing's, itself the
        // block's, and laying a course of them makes two parts, not one a click.
        for x in 4..8 {
            let ops = block(
                &base,
                IVec3::new(x, 0, 3),
                Shape::Whole,
                LOGS,
                &rules,
                Some(footings),
            );
            base.act_all(&ops, &cx).unwrap();
        }
        assert_eq!(base.parts().count(), 2);
        assert_eq!(base.cubes().filter(|(at, _)| at.y == -1).count(), 5);
        // A block over another, or up in the air, takes none.
        let over = block(
            &base,
            IVec3::new(3, 1, 3),
            Shape::Whole,
            LOGS,
            &rules,
            Some(footings),
        );
        assert_eq!(over.len(), 1);
        let high = block(
            &base,
            IVec3::new(20, 4, 3),
            Shape::Whole,
            LOGS,
            &rules,
            Some(footings),
        );
        assert_eq!(high.len(), 1);
        // A box of them by the brush: a footing under each column of its foot.
        let cubes = brush(IVec3::new(12, 0, 12), 2);
        let ops = blocks(&base, &cubes, Shape::Whole, STONE, &rules, Some(footings));
        base.act_all(&ops, &cx).unwrap();
        assert_eq!(
            (12..14)
                .flat_map(|x| (12..14).map(move |z| IVec3::new(x, -1, z)))
                .filter(|&at| base.cube(at).is_some())
                .count(),
            4
        );
    }
}
