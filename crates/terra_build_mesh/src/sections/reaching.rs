//! Which sections a change reaches: those round the cubes it marked, as far as any piece
//! reaches from the cubes it is laid from, and the whole of a line or region a layout lays
//! from all of at once (a row of logs, a deck), before the change and after.

use super::*;

/// Every section a base may draw something in: those with cubes, and those beside them that
/// their cubes come within a few of, which overhangs, eaves and steps reach into.
pub fn drawable(base: &Base) -> BTreeSet<[i32; 3]> {
    let side = terra_build::cubes::SECTION;
    let mut out = BTreeSet::new();
    let mut reach = |low: IVec3, high: IVec3| {
        let (from, to) = (
            terra_build::cubes::section_of(low),
            terra_build::cubes::section_of(high),
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
pub(super) const REACH: i32 = 3;

/// The sections and those round them.
pub(super) fn round(sections: impl IntoIterator<Item = IVec3>) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    for section in sections {
        for offset in NEIGHBOURS.iter().chain([&IVec3::ZERO]) {
            out.insert((section + *offset).to_array());
        }
    }
    out
}

/// The 26 sections round one.
pub(super) const NEIGHBOURS: [IVec3; 26] = {
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

/// Whether all an edit did was fill cubes further in, in the sections it marked: some
/// cube changed, each only in how far it is in, with no change pending on it, and the
/// ground and fitted props as they were. No piece of a plan is laid by how far a cube is
/// in, so its plan is laid as it was.
pub(super) fn only_filled(before: &Base, after: &Base, marked: &BTreeSet<[i32; 3]>) -> bool {
    if before.ground() != after.ground() || before.props_differ(after) {
        return false;
    }
    let side = terra_build::cubes::SECTION;
    let mut any = false;
    for &at in marked {
        let low = IVec3::from_array(at) * side;
        let high = low + IVec3::splat(side - 1);
        let (was, now) = (
            before.store().within(low, high),
            after.store().within(low, high),
        );
        if was.len() != now.len() {
            return false;
        }
        for ((a, x), (b, y)) in was.iter().zip(&now) {
            if a != b {
                return false;
            }
            if x == y {
                continue;
            }
            let plan = |cube: &terra_build::Cube| terra_build::Cube {
                state: terra_build::State::Planned { level: 0 },
                ..*cube
            };
            if plan(x) != plan(y)
                || x.pending != terra_build::Pending::None
                || y.pending != terra_build::Pending::None
            {
                return false;
            }
            any = true;
        }
    }
    any
}

/// How far round a cube that changed a roof's pieces may change, in cubes: further than
/// its eaves run on past its walls or anything under or over it is looked at.
pub(super) const ROOF_REACH: i32 = 4;

/// Whether any cube that changed between two bases, in the sections marked, has a roof's
/// cube within [`ROOF_REACH`] of it, before or after.
pub(super) fn roof_near_changes(before: &Base, after: &Base, marked: &BTreeSet<[i32; 3]>) -> bool {
    changed_cubes(before, after, marked).iter().any(|&cube| {
        let reach = IVec3::splat(ROOF_REACH);
        [before, after].iter().any(|base| {
            !base
                .store()
                .within_where(cube - reach, cube + reach, |c| c.kind.is_roof())
                .is_empty()
        })
    })
}

/// The sections whose pieces an edit that marked these may change, before it and after,
/// in groups each laid together as a box: those round the marks, as far as any piece
/// reaches from the cubes it is laid from (an opening's dress, a frame's members: under a
/// section); and the whole of what a layout lays from all of one line or region, through
/// the marks: a row of logs along either way, a deck over every cube joined to it on its
/// layer. Never the rest of the building, nor another building.
pub fn reach(
    before: &Base,
    after: &Base,
    kits: &Kits,
    marked: &BTreeSet<[i32; 3]>,
) -> Vec<BTreeSet<[i32; 3]>> {
    let mut drawn: BTreeSet<[i32; 3]> = drawable(before);
    drawn.extend(drawable(after));
    let round_marks = |marked: &BTreeSet<[i32; 3]>| -> BTreeSet<[i32; 3]> {
        round(marked.iter().copied().map(IVec3::from_array))
            .intersection(&drawn)
            .copied()
            .collect()
    };
    let mut near = round_marks(marked);
    // The kits whose cubes are round the marks, before the edit or after.
    let here = |kit: &Kit, near: &BTreeSet<[i32; 3]>| {
        [before, after].iter().any(|base| {
            near.iter().any(|&at| {
                let low = IVec3::from_array(at) * terra_build::cubes::SECTION;
                base.store()
                    .within(low, low + IVec3::splat(terra_build::cubes::SECTION - 1))
                    .iter()
                    .any(|(_, cube)| cube.kind == kit.kind())
            })
        })
    };
    // A frame's members run the length of their wall, and are laid only as far round a
    // box as a member reaches from a post, so a framed building is laid again whole.
    if kits
        .0
        .iter()
        .any(|kit| kit.framing.is_some() && here(kit, &near))
    {
        let mut whole = affected(before, marked);
        whole.extend(affected(after, marked));
        return vec![whole];
    }
    // Which way a log looks out of its building (D261) is worked out from what is within
    // the building beside its wall (`layout::enclosed`), which only an edit that empties or
    // first fills a column, or changes what covers it, can change: the sections that can
    // see such a column along the floor are laid again as though marked, with the rows of
    // logs along from them. A window or a hole cut through a wall changes none.
    let logs_here = kits
        .0
        .iter()
        .any(|kit| kit.pattern == Pattern::Logs && here(kit, &near));
    let mut marked = marked.clone();
    if logs_here {
        let moved = enclosure_moved(before, after, &marked, &drawn);
        if !moved.is_empty() {
            marked.extend(moved);
            near = round_marks(&marked);
        }
    }
    let mut groups = vec![near.clone()];
    // A row of logs is jointed at places fixed along it (`layout::runs`), so an edit
    // changes its logs no further along it than a log's reach: the sections that far either
    // way along the floor from the marks, at the heights round them.
    let along = kits
        .0
        .iter()
        .filter(|kit| kit.pattern == Pattern::Logs && here(kit, &near))
        .map(layout::runs::reach_cubes)
        .max();
    if let Some(along) = along {
        let far = (along + terra_build::cubes::SECTION - 1) / terra_build::cubes::SECTION;
        let mut taken = near.clone();
        for step in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
            let line: BTreeSet<[i32; 3]> = near
                .iter()
                .flat_map(|&at| (1..far).map(move |n| IVec3::from_array(at) + step * n))
                .map(|at| at.to_array())
                .filter(|at| drawn.contains(at) && !taken.contains(at))
                .collect();
            if !line.is_empty() {
                taken.extend(line.iter().copied());
                groups.push(line);
            }
        }
        // A deck's boards run its region's longer way, or square to the walls it meets
        // (`layout::floors`), so a cube joined to it, or a wall beside it, lays all of it
        // again: the sections of every deck beside a changed cube, before or after.
        let decks: BTreeSet<[i32; 3]> = decks_beside(before, after, kits, &marked)
            .into_iter()
            .filter(|at| drawn.contains(at) && !taken.contains(at))
            .collect();
        if !decks.is_empty() {
            groups.push(decks);
        }
    }
    groups
}

/// The cubes that differ between two bases in the sections marked.
pub(super) fn changed_cubes(
    before: &Base,
    after: &Base,
    marked: &BTreeSet<[i32; 3]>,
) -> Vec<IVec3> {
    let side = terra_build::cubes::SECTION;
    let mut changed: Vec<IVec3> = Vec::new();
    for &at in marked {
        let low = IVec3::from_array(at) * side;
        let high = low + IVec3::splat(side - 1);
        let (was, now) = (
            before.store().within(low, high),
            after.store().within(low, high),
        );
        let (mut i, mut j) = (0, 0);
        while i < was.len() || j < now.len() {
            match (was.get(i), now.get(j)) {
                (Some(a), Some(b)) if a.0 == b.0 => {
                    if a.1 != b.1 {
                        changed.push(a.0);
                    }
                    (i, j) = (i + 1, j + 1);
                }
                (Some(a), Some(b)) if a.0.to_array() < b.0.to_array() => {
                    changed.push(a.0);
                    i += 1;
                }
                (Some(_), Some(b)) => {
                    changed.push(b.0);
                    j += 1;
                }
                (Some(a), None) => {
                    changed.push(a.0);
                    i += 1;
                }
                (None, Some(b)) => {
                    changed.push(b.0);
                    j += 1;
                }
                (None, None) => break,
            }
        }
    }
    changed
}

/// The sections of every timber deck (`layout::decked`) a cube that changed is in or
/// beside, before the change or after, each region of it joined on its layer whole, and a
/// cube round it.
pub(super) fn decks_beside(
    before: &Base,
    after: &Base,
    kits: &Kits,
    marked: &BTreeSet<[i32; 3]>,
) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    let changed = changed_cubes(before, after, marked);
    for base in [before, after] {
        let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
        for &cube in &changed {
            for offset in round_it(cube).filter(|at| (*at - cube).abs().max_element() <= 1) {
                let Some(kind) = layout::decked(base, kits, offset) else {
                    continue;
                };
                if !seen.insert(offset.to_array()) {
                    continue;
                }
                let mut next = vec![offset];
                let (mut low, mut high) = (offset, offset);
                while let Some(at) = next.pop() {
                    for side in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
                        let beside = at + side;
                        if !seen.contains(&beside.to_array())
                            && layout::decked(base, kits, beside) == Some(kind)
                        {
                            seen.insert(beside.to_array());
                            (low, high) = (low.min(beside), high.max(beside));
                            next.push(beside);
                        }
                    }
                }
                let (from, to) = (
                    terra_build::cubes::section_of(low - IVec3::ONE),
                    terra_build::cubes::section_of(high + IVec3::ONE),
                );
                for x in from.x..=to.x {
                    for y in from.y..=to.y {
                        for z in from.z..=to.z {
                            out.insert([x, y, z]);
                        }
                    }
                }
            }
        }
    }
    out
}

/// The sections from which a column the edit changed, in the sections marked, can be seen
/// along the floor within `layout::ENCLOSE_REACH`: where a column was emptied or first
/// filled, or what is highest built in it changed, which whether a place is within a
/// building is read from (`layout::enclosed`).
pub(super) fn enclosure_moved(
    before: &Base,
    after: &Base,
    marked: &BTreeSet<[i32; 3]>,
    drawn: &BTreeSet<[i32; 3]>,
) -> BTreeSet<[i32; 3]> {
    let side = terra_build::cubes::SECTION;
    let profile = |base: &Base, x: i32, z: i32| {
        let top = base
            .column(x, z)
            .filter(|&y| {
                base.cube(IVec3::new(x, y, z))
                    .is_some_and(|cube| cube.fill != Fill::Open)
            })
            .max();
        (base.occupied(x, z), top)
    };
    let columns: BTreeSet<[i32; 2]> = marked
        .iter()
        .map(|&[x, _, z]| [x, z])
        .collect::<BTreeSet<_>>()
        .into_iter()
        .flat_map(|[sx, sz]| {
            (0..side).flat_map(move |i| (0..side).map(move |j| [sx * side + i, sz * side + j]))
        })
        .filter(|&[x, z]| profile(before, x, z) != profile(after, x, z))
        .collect();
    let mut seen: BTreeSet<[i32; 2]> = BTreeSet::new();
    for &[x, z] in &columns {
        for step in [
            glam::IVec2::X,
            glam::IVec2::NEG_X,
            glam::IVec2::Y,
            glam::IVec2::NEG_Y,
        ] {
            for k in 0..=layout::ENCLOSE_REACH {
                let at = glam::IVec2::new(x, z) + step * k;
                seen.insert([at.x.div_euclid(side), at.y.div_euclid(side)]);
            }
        }
    }
    drawn
        .iter()
        .filter(|&&[x, _, z]| seen.contains(&[x, z]))
        .copied()
        .collect()
}

/// How far a section's cubes are searched for the kinds whose kits may lay pieces in it.
pub(super) const KINDS_REACH: i32 = 4;
