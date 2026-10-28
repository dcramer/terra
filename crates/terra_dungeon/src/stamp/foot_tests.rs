use super::tests::{SEEDS, kind};
use super::*;
use crate::place::STEPS;
use crate::theme::assets;
use crate::{ModuleSet, generate};
use terra_gen::Seed;

/// A wall's cube with nothing under it beside a floor's top: a groove at the wall's foot,
/// the wall's bottom edge open and the mesher bevelling it round under, the floor's edge
/// round down into the hole.
pub(super) fn grooves(map: &HashMap<IVec3, Cube>, wall: Kind, floor: Kind) -> Vec<IVec3> {
    let mut out = Vec::new();
    for (&at, cube) in map {
        if cube.kind != wall || cube.fill != Fill::Solid || map.contains_key(&(at - IVec3::Y)) {
            continue;
        }
        let under = at - IVec3::Y;
        let beside = STEPS.iter().map(|&s| under + s).any(|n| {
            map.get(&n)
                .is_some_and(|c| c.kind == floor && c.fill == Fill::Solid)
                && !map.solid(n + IVec3::Y)
        });
        if beside {
            out.push(at);
        }
    }
    out.sort_by_key(|a| a.to_array());
    out
}

/// Every wall stands on cubes down to the floor beside it. A basin's floor sunk a cube left
/// the wall's foot over an empty cube: round 6's mine stills showed a pale band of the rock's
/// own blue-grey, its normals tilted down, along every wall of a flooded room (views 2, 3, 5).
#[test]
fn a_wall_stands_down_to_the_floor_beside_it() {
    let mut failed = Vec::new();
    for theme in Theme::all(&assets()).unwrap() {
        let set = ModuleSet::new(&theme).unwrap();
        let (wall, floor) = (kind(&theme.walls), kind(&theme.floors));
        let seeds: Vec<u64> = (0..SEEDS).collect();
        let found = terra_gen::par::map(&seeds, |&s| {
            let dungeon = generate(&theme, &set, Seed(s)).ok()?;
            let stamp = stamp(&dungeon, &theme, &kind);
            let at = grooves(&cubes(&stamp.ops), wall, floor);
            Some((s, at))
        });
        let found: Vec<_> = found.into_iter().flatten().collect();
        let count: usize = found.iter().map(|(_, at)| at.len()).sum();
        println!(
            "{}: {count} wall cubes over a groove in {} dungeons",
            theme.name,
            found.len()
        );
        if let Some((s, at)) = found.iter().find(|(_, at)| !at.is_empty()) {
            failed.push(format!("{}: {count}, seed {s} at {}", theme.name, at[0]));
        }
    }
    assert!(
        failed.is_empty(),
        "walls over a groove at their foot: {failed:?}"
    );
}
