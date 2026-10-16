//! A base nobody is building drawn a detail at a time (D213, D278), as an open world streams
//! a level of detail: each of a group's details (near, a few metres off, far, its hull) is
//! laid on its own when the camera comes within where it is needed, or will be a second
//! ahead, and kept while it is wanted or there is room for it, so a house the camera walks
//! past is laid once at each detail rather than at a finer detail and again at the one it
//! fades into each time the camera crosses a line, and walking back past it lays nothing.
//! The details held hand over to each other by Bevy's dithered crossfade (`VisibilityRange`)
//! over the meshes already made; one missing, its neighbours stretch over its distances.

use super::super::store::{self, Drawn, OnScreen};
use super::*;
use terra_build::sections::Detail;

/// A group's details, finest first: each one slot of [`Drawn::parts`].
pub(in crate::build) const DETAILS: [Detail; 4] =
    [Detail::Near, Detail::Mid, Detail::Far, Detail::Proxy];

/// A detail's slot among [`DETAILS`].
pub(in crate::build) fn slot(detail: Detail) -> usize {
    DETAILS
        .iter()
        .position(|each| *each == detail)
        .unwrap_or(DETAILS.len() - 1)
}

/// The reach a detail held as its finest counts as, for what else reads a section's reach.
pub(in crate::build) fn reach_of(detail: Detail) -> Reach {
    match detail {
        Detail::Near => Reach::Near,
        Detail::Mid => Reach::Mid,
        Detail::Far => Reach::Far,
        Detail::Proxy => Reach::Hull,
    }
}

/// Where each detail is seen from and to, authored (each fading over its margin): the near
/// detail from the camera, the hull to where its base's cells take over.
const STARTS: [f32; 4] = [0.0, NEAR_M, MID_M, FAR_M];
const ENDS: [f32; 4] = [NEAR_M, MID_M, FAR_M, PROXY_M];

/// Where each detail is needed, as authored, by a group's distance from the camera
/// (`store::distance_from`): from a few metres before it is seen (`STARTS` less its fade)
/// to a few metres after (`ENDS` and its fade), what laying one takes at a run. The near
/// detail is seen to 26.5 m and needed within 32; the middle detail from 23.5 m to 63.6 and
/// needed from 20 to 70; the far detail from 56.4 to 127, needed from 52 to 140; the hull
/// from 112.8 m, needed from 108.
const NEEDED: [(f32, f32); 4] = [
    (0.0, 32.0),
    (20.0, 70.0),
    (52.0, 140.0),
    (108.0, f32::INFINITY),
];

/// The details a group needs at this distance. Where the near detail casts its own shadow
/// (`close`, the high preset) the middle detail is needed only where it is seen; below it,
/// whenever the near detail is, as the sun sees the middle detail (`show`).
pub(in crate::build) fn needed(distance: f32, close: bool) -> [bool; 4] {
    let mut needed = NEEDED.map(|(from, to)| distance >= from && distance < to);
    needed[1] |= needed[0] && !close;
    needed
}

/// The detail to lay next for a group, and whether it is needed now (`now`) rather than only
/// a second ahead (`soon`): of those needed now and not held, the one seen at this distance
/// first, then the finest; then of those needed soon, the finest.
pub(in crate::build) fn next(
    held: [bool; 4],
    now: [bool; 4],
    soon: [bool; 4],
    distance: f32,
) -> Option<(Detail, bool)> {
    let seen = ENDS
        .iter()
        .position(|&end| distance < end * (1.0 + FADE))
        .unwrap_or(DETAILS.len() - 1);
    let lacks = |i: usize, wanted: [bool; 4]| wanted[i] && !held[i];
    if lacks(seen, now) {
        return Some((DETAILS[seen], true));
    }
    (0..DETAILS.len())
        .find(|&i| lacks(i, now))
        .map(|i| (DETAILS[i], true))
        .or_else(|| {
            (0..DETAILS.len())
                .find(|&i| lacks(i, soon))
                .map(|i| (DETAILS[i], false))
        })
}

/// Which details of a group are held, laid as it is now (`key`).
pub(in crate::build) fn held(drawn: &Drawn) -> [bool; 4] {
    let mut held = [false; 4];
    if drawn.by_detail {
        for (i, part) in drawn.parts.iter().enumerate().take(DETAILS.len()) {
            held[i] = part.key == Some(drawn.key) && !part.entities.is_empty();
        }
    }
    held
}

/// What each detail held is seen over, as authored, so those held cover every distance from
/// the camera out with no gap: each from where the next finer one held ends, its own start,
/// or from the camera if none finer is held; to where the next coarser one held starts, or
/// its own end. A detail laid in hands over to the one stretched over its distances by the
/// same crossfade as any other.
pub(in crate::build) fn ranges(held: [bool; 4]) -> [Option<VisibilityRange>; 4] {
    let order: Vec<usize> = (0..DETAILS.len()).filter(|&i| held[i]).collect();
    let mut ranges = [None, None, None, None];
    for (n, &i) in order.iter().enumerate() {
        let start = (n > 0).then_some(STARTS[i]);
        let end = order.get(n + 1).map(|&next| STARTS[next]);
        let mut range = super::range(start, end.unwrap_or(ENDS[i]));
        if end.is_none() && i == DETAILS.len() - 1 {
            range.end_margin = super::proxy_fade();
        }
        ranges[i] = Some(range);
    }
    ranges
}

/// A group's range moved out by as far as its sections reach from its middle, on the detail
/// ranges' grid (`show`).
fn shifted(mut range: VisibilityRange, further: f32) -> VisibilityRange {
    let shift = |r: std::ops::Range<f32>| {
        if r.end <= 0.0 {
            return r;
        }
        let at = |m: f32| ((m + further) * 4.0).round() / 4.0;
        at(r.start).max(0.0)..at(r.end).max(0.0)
    };
    range.start_margin = shift(range.start_margin.clone());
    range.end_margin = shift(range.end_margin.clone());
    range
}

fn further(drawn: &Drawn) -> f32 {
    drawn
        .bounds
        .map_or(0.0, |bounds| store::sphere(bounds).1 - store::GROUP_SHIFT_M)
}

/// Gives every detail held its range for what is held now.
pub(in crate::build) fn hand_over(commands: &mut Commands, drawn: &mut Drawn) {
    let further = further(drawn);
    let ranges = ranges(held(drawn));
    for (part, range) in drawn.parts.iter().zip(ranges) {
        let Some(range) = range else {
            continue;
        };
        let range = shifted(range, further);
        for id in part.entities.iter().filter(|id| !part.casters.contains(id)) {
            if let Ok(mut entity) = commands.get_entity(*id) {
                terra_world::detail::set_range(&mut entity, range.clone());
            }
        }
    }
    drawn.reach = DETAILS
        .iter()
        .zip(held(drawn))
        .find(|(_, held)| *held)
        .map_or(Reach::Hull, |(detail, _)| reach_of(*detail));
    drawn.gpu_bytes = drawn.parts.iter().map(|part| part.gpu_bytes).sum();
}

/// Lets a detail go: its entities, with their meshes.
pub(in crate::build) fn let_go(commands: &mut Commands, part: &mut OnScreen) {
    for id in part.entities.drain(..) {
        commands.entity(id).try_despawn();
    }
    *part = OnScreen::default();
}

/// Puts a detail laid for a group on screen beside those it holds: those laid for an
/// earlier shape of it let go; what is of no one detail (`plain`, a plan's ghosts) kept
/// once.
#[allow(clippy::too_many_arguments)]
pub(super) fn show(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    look: &BuildLook,
    arrays: Option<&Handle<BuildMaterial>>,
    build: &mut Build,
    (at, ticket): ([i32; 3], u64),
    detail: Detail,
    key: u64,
    parts: Parts,
    quality: Quality,
) {
    let root = build.root;
    let drawn = build.sections.drawn.entry(at).or_insert_with(|| Drawn {
        entity: None,
        ticket,
        key,
        reach: Reach::Hull,
        drawing: None,
        gpu_bytes: 0,
        bounds: parts.bounds,
        parts: Vec::new(),
        rest: Vec::new(),
        by_detail: true,
    });
    // Drawn a whole reach at a time before: all of that goes.
    if !drawn.by_detail || drawn.parts.len() != DETAILS.len() {
        for id in drawn
            .entity
            .take()
            .into_iter()
            .chain(drawn.rest.drain(..))
            .chain(drawn.parts.drain(..).flat_map(|part| part.entities))
        {
            commands.entity(id).try_despawn();
        }
        drawn.parts = (0..DETAILS.len()).map(|_| OnScreen::default()).collect();
        drawn.by_detail = true;
    }
    drawn.ticket = ticket;
    drawn.bounds = parts.bounds.or(drawn.bounds);
    // Laid for a new shape: what was held of the old one goes.
    if drawn.key != key {
        for part in drawn.parts.iter_mut().filter(|part| part.key != Some(key)) {
            let_go(commands, part);
        }
        for id in drawn.rest.drain(..) {
            commands.entity(id).try_despawn();
        }
        drawn.key = key;
    }
    let i = slot(detail);
    let_go(commands, &mut drawn.parts[i]);
    let placed = Transform::from_translation(parts.centre);
    let further = further(drawn);
    let looks = &look.looks;
    let material = |kind: Kind, which: Material| {
        look.materials
            .get(kind, which)
            .cloned()
            .unwrap_or_else(|| looks.structure.clone())
    };
    let mut shown = OnScreen {
        key: Some(key),
        ..Default::default()
    };
    let Parts {
        parts: made,
        plain,
        next,
        far,
        proxy,
        ..
    } = parts;
    let coarse_shadows = quality != Quality::High;
    let level = Level::of(detail);
    let mut put = |commands: &mut Commands, mesh: Staged, shade: Shade, shown: &mut OnScreen| {
        shown.gpu_bytes += mesh.bytes();
        let handle = mesh.handle(meshes, false);
        // A range for now: the hand-over below gives each its own.
        let mut spawned = commands.spawn((
            Mesh3d(handle.clone()),
            range(None, 0.0),
            level,
            placed,
            ChildOf(root),
        ));
        match shade {
            Shade::Standard(material) => spawned.insert(MeshMaterial3d(material)),
            Shade::Arrays(material) => spawned.insert(MeshMaterial3d(material)),
        };
        shown.entities.push(spawned.id());
        handle
    };
    for part in made.into_iter().flatten() {
        let Part {
            arrays: [near_arrays, mid_arrays],
            layers,
            near,
            mid,
        } = part;
        shown.layers.extend(layers);
        let (surface, kits) = if detail == Detail::Near {
            (near_arrays, near)
        } else {
            (mid_arrays, mid)
        };
        let mut casters: Vec<Handle<Mesh>> = Vec::new();
        if let (Some(mesh), Some(arrays)) = (surface, arrays) {
            casters.push(put(
                commands,
                mesh,
                Shade::Arrays(arrays.clone()),
                &mut shown,
            ));
        }
        for (kind, which, mesh) in kits {
            casters.push(put(
                commands,
                mesh,
                Shade::Standard(material(kind, which)),
                &mut shown,
            ));
        }
        // Below the high preset neither the near detail nor the middle casts a shadow
        // itself: the middle detail's is cast by a copy only the sun sees (`show`).
        if coarse_shadows && matches!(detail, Detail::Near | Detail::Mid) {
            for id in &shown.entities {
                commands.entity(*id).insert(NotShadowCaster);
            }
        }
        if coarse_shadows && detail == Detail::Mid {
            for mesh in casters {
                let id = commands
                    .spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(looks.far.clone()),
                        shifted(range(None, MID_M), further),
                        RenderLayers::layer(SUN_ONLY),
                        placed,
                        ChildOf(root),
                    ))
                    .id();
                shown.entities.push(id);
                shown.casters.push(id);
            }
        }
    }
    if let Some(mesh) = far {
        put(
            commands,
            mesh,
            Shade::Standard(looks.far.clone()),
            &mut shown,
        );
    }
    if let Some(mesh) = proxy {
        put(
            commands,
            mesh,
            Shade::Standard(looks.far.clone()),
            &mut shown,
        );
    }
    // What is of no one detail, once. A base drawn whole has no plan: no ghosts, shell nor
    // next part (`layout::by_detail`).
    if drawn.rest.is_empty() {
        for (mesh, material, range) in next
            .into_iter()
            .map(|mesh| (mesh, looks.next.clone(), range(None, NEAR_M)))
            .chain(plain.map(|mesh| (mesh, looks.structure.clone(), range(None, FAR_M))))
        {
            let id = commands
                .spawn((
                    Mesh3d(mesh.handle(meshes, false)),
                    MeshMaterial3d(material),
                    shifted(range, further),
                    Level::Every,
                    placed,
                    ChildOf(root),
                ))
                .id();
            drawn.rest.push(id);
        }
    }
    drawn.parts[i] = shown;
    hand_over(commands, drawn);
}

/// How many bytes of details held that the camera does not need now nor soon may stay on
/// the GPU, all bases together: walking back past a house finds them still there. Past it,
/// those whose distances the camera is furthest outside go first (`evict`). At the lower
/// presets, none: the integrated GPU's town, holding them, drew a fifth slower, its few
/// hundred megabytes of meshes nearly doubled in memory it shares with the CPU.
pub(in crate::build) fn spare_bytes(quality: Quality) -> usize {
    match quality {
        Quality::High => 192 << 20,
        _ => 0,
    }
}

/// A detail held and not wanted: whose, which, its bytes, and how far the camera is outside
/// the distances it is needed over.
pub(in crate::build) struct Spare {
    pub root: Entity,
    pub at: [i32; 3],
    pub slot: usize,
    pub bytes: usize,
    pub outside: f32,
}

/// How far a distance is outside where a detail is needed.
pub(in crate::build) fn outside(slot: usize, distance: f32) -> f32 {
    let (from, to) = NEEDED[slot];
    (from - distance).max(distance - to).max(0.0)
}

/// Which of the details held and not wanted to let go, so the rest fit in `budget`: those
/// the camera is furthest outside first; never a hull, a few hundred triangles that every
/// distance past the far detail needs.
pub(in crate::build) fn evict(mut spare: Vec<Spare>, budget: usize) -> Vec<Spare> {
    spare.retain(|each| each.slot < DETAILS.len() - 1);
    let mut total: usize = spare.iter().map(|each| each.bytes).sum();
    spare.sort_by(|a, b| b.outside.total_cmp(&a.outside));
    let mut out = Vec::new();
    for each in spare {
        if total <= budget {
            break;
        }
        total -= each.bytes;
        out.push(each);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walking_past_a_house_lays_each_detail_once_each_needed_wherever_it_is_seen() {
        // In from 200 m to 5 m and back out, laying what is needed and keeping it: each
        // detail laid once, and needed wherever it is seen.
        for close in [true, false] {
            let mut laid = [0usize; 4];
            let mut held = [false; 4];
            let mut walk: Vec<f32> = (0..=390).map(|i| 200.0 - i as f32 * 0.5).collect();
            walk.extend(walk.clone().into_iter().rev());
            for &d in &walk {
                let now = needed(d, close);
                for i in 0..4 {
                    let seen = d >= STARTS[i] * (1.0 - FADE) && d < ENDS[i] * (1.0 + FADE);
                    assert!(!seen || now[i], "{d} m: detail {i} seen and not needed");
                }
                while let Some((detail, urgent)) = next(held, now, [false; 4], d) {
                    assert!(urgent);
                    held[slot(detail)] = true;
                    laid[slot(detail)] += 1;
                }
            }
            assert_eq!(laid, [1, 1, 1, 1], "close {close}");
            // Below the high preset the middle detail is held with the near.
            assert_eq!(needed(5.0, close)[1], !close);
        }
    }

    #[test]
    fn the_detail_seen_is_laid_first_then_the_finest_needed_then_what_is_needed_soon() {
        let none = [false; 4];
        // At 30 m, near and middle needed: the middle is what is seen.
        let now = needed(30.0, true);
        assert_eq!(now, [true, true, false, false]);
        assert_eq!(next(none, now, none, 30.0), Some((Detail::Mid, true)));
        assert_eq!(
            next([false, true, false, false], now, none, 30.0),
            Some((Detail::Near, true))
        );
        // All held that are needed: what will be needed a second ahead, if not held.
        let soon = needed(10.0, true);
        assert_eq!(next([true, true, false, false], now, soon, 30.0), None);
        let ahead = needed(60.0, true);
        assert_eq!(
            next([true, true, false, false], now, ahead, 30.0),
            Some((Detail::Far, false))
        );
    }

    #[test]
    fn the_details_held_cover_every_distance_and_hand_over_where_they_meet() {
        let covered = |held: [bool; 4]| {
            let ranges = ranges(held);
            let order: Vec<(usize, VisibilityRange)> = ranges
                .iter()
                .enumerate()
                .filter_map(|(i, r)| r.clone().map(|r| (i, r)))
                .collect();
            // The finest held from the camera; each next from where the one before ends,
            // crossing over the same margin; the coarsest to its own end.
            assert_eq!(order[0].1.start_margin, 0.0..0.0, "{held:?}");
            for pair in order.windows(2) {
                assert_eq!(pair[0].1.end_margin, pair[1].1.start_margin, "{held:?}");
            }
            let (last, range) = order.last().unwrap();
            assert!(range.end_margin.start >= ENDS[*last] * (1.0 - PROXY_FADE) - 1e-3);
        };
        for bits in 1..16u8 {
            let held = [0, 1, 2, 3].map(|i| bits & (1 << i) != 0);
            covered(held);
        }
        // All held: each over its own distances, as a section drawn by reach was.
        let all = ranges([true; 4]);
        assert_eq!(all[1].clone().unwrap().start_margin, fade(NEAR_M));
        assert_eq!(all[1].clone().unwrap().end_margin, fade(MID_M));
        assert_eq!(all[3].clone().unwrap().end_margin, proxy_fade());
        // The middle detail missing: the near stretched over to the far.
        let gap = ranges([true, false, true, true]);
        assert_eq!(gap[0].clone().unwrap().end_margin, fade(MID_M));
    }

    #[test]
    fn what_is_let_go_is_furthest_from_where_it_is_needed_first_and_never_a_hull() {
        let root = Entity::PLACEHOLDER;
        let spare = |slot, bytes, outside| Spare {
            root,
            at: [0; 3],
            slot,
            bytes,
            outside,
        };
        let out = evict(
            vec![
                spare(0, 50, 5.0),
                spare(1, 50, 40.0),
                spare(3, 500, 99.0),
                spare(2, 50, 1.0),
            ],
            60,
        );
        let slots: Vec<usize> = out.iter().map(|each| each.slot).collect();
        assert_eq!(slots, [1, 0], "the most outside first, until the rest fit");
        assert!(evict(vec![spare(0, 50, 5.0)], 60).is_empty());
        assert_eq!(outside(0, 40.0), 8.0);
        assert_eq!(outside(2, 30.0), 22.0);
        assert_eq!(outside(1, 50.0), 0.0);
    }
}
