//! A room dressed by what it is for (the theme's `dressing`): props laid against its walls
//! after the run-in, off the tour and the doorways and apart from what already stands, as a
//! level designer sets a guard room out with its fallen and a treasury with its offerings
//! once the room's kit is in.

use super::modules::{Class, Look};
use super::{Frame, LightKind, MID, Prop, Stamp, hash, stands};
use crate::Dungeon;
use crate::place::{Side, opposite};
use crate::theme::Dress;
use glam::{IVec3, Vec2, Vec3};
use std::collections::HashMap;
use terra_build::Cube;

/// How far a prop keeps from the tour's line, in metres, so the camera never passes through
/// one: the eye keeps 0.5 m off walls, and a prop stands out from a wall at most 0.6 m.
pub(crate) const OFF_ROUTE_M: f32 = 1.3;
/// From a doorway's middle, so none stands in the way in.
const OFF_DOOR_M: f32 = 1.6;
/// From what already stands (props, objects, chests, mobs, fires) and from each other.
const APART_M: f32 = 1.0;
/// Out from the wall's cell edge, in cubes: 0.3 m clear of its face.
const OUT: f32 = 1.6;

pub(super) fn lay(
    dungeon: &Dungeon,
    dressing: &[Dress],
    looks: &[Look],
    map: &HashMap<IVec3, Cube>,
    out: &mut Stamp,
) {
    let layout = &dungeon.layout;
    let mut taken: Vec<Vec3> = out.props.iter().map(|p| p.at).collect();
    taken.extend(out.objects.iter().map(|o| o.at));
    taken.extend(out.chests.iter().map(|c| c.at));
    taken.extend(out.mobs.iter().map(|m| m.at));
    let fires = out.lights.iter().filter(|l| l.kind != LightKind::Torch);
    taken.extend(fires.map(|l| l.at));
    let route = out.route.clone();
    for (r, room) in dungeon.graph.rooms.iter().enumerate() {
        let rect = layout.rooms[r];
        let cells: Vec<(IVec3, usize)> = rect
            .cells()
            .filter_map(|at| layout.cell(at).map(|c| (at, c)))
            .collect();
        let doors: Vec<Vec3> = cells
            .iter()
            .flat_map(|&(at, c)| {
                let sides = layout.cells[c].sides;
                (0..4)
                    .filter(move |&d| matches!(sides[d], Side::Door(_) | Side::Way))
                    .map(move |d| Frame::new(at, d).point(MID, 0.0, 0.0))
            })
            .collect();
        // Against each walled side of a plain cell, a metre, two and three along it.
        let mut spots: Vec<(u32, Vec3, usize)> = Vec::new();
        for &(at, c) in cells.iter().filter(|&&(_, c)| bare(looks[c].class)) {
            for d in (0..4).filter(|&d| layout.cells[c].sides[d] == Side::Wall) {
                let f = Frame::new(at, d);
                for x in [2, 4, 6] {
                    let p = f.point(x as f32, 0.0, OUT);
                    let floor = (x - 1..=x).all(|x| stands(map, f.cube(x, 0, 1)));
                    let clear = off(&route, p) > OFF_ROUTE_M
                        && doors.iter().all(|d| d.distance(p) > OFF_DOOR_M);
                    if floor && clear {
                        spots.push((hash(at, 500 + 8 * d as u32 + x as u32), p, d));
                    }
                }
            }
        }
        spots.sort_by_key(|&(rank, p, d)| (rank, p.to_array().map(f32::to_bits), d));
        for (k, dress) in dressing
            .iter()
            .enumerate()
            .filter(|(_, dress)| dress.roles.contains(&room.role) && !dress.props.is_empty())
        {
            let (least, most) = dress.count;
            let spread = most.saturating_sub(least) + 1;
            let per = least + hash(rect.min, 520 + k as u32) % spread;
            let wanted = (per as usize * cells.len()).div_ceil(4);
            let mut laid = 0;
            for &(rank, p, d) in &spots {
                if laid == wanted {
                    break;
                }
                if taken.iter().any(|t| t.distance(p) < APART_M) {
                    continue;
                }
                let (model, scale) = &dress.props[rank as usize % dress.props.len()];
                taken.push(p);
                out.props.push(Prop {
                    model: model.clone(),
                    at: p,
                    facing: opposite(d),
                    scale: *scale,
                });
                laid += 1;
            }
        }
    }
}

/// Whether a cell's module leaves its walls bare for dressing.
fn bare(class: Class) -> bool {
    matches!(
        class,
        Class::Plain | Class::Runner | Class::Decor | Class::Pillar
    )
}

/// How far a point is from the tour's line, across the floor, of its legs within a level's
/// height of it.
pub(crate) fn off(route: &[Vec3], p: Vec3) -> f32 {
    let flat = |v: Vec3| Vec2::new(v.x, v.z);
    let q = flat(p);
    route
        .windows(2)
        .filter(|w| w.iter().any(|v| (v.y - p.y).abs() < 3.0))
        .map(|w| {
            let (a, b) = (flat(w[0]), flat(w[1]));
            let ab = b - a;
            let t = ((q - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            (a + ab * t).distance(q)
        })
        .fold(f32::MAX, f32::min)
}

#[cfg(test)]
mod tests {
    use super::super::tests::kind;
    use super::super::{Theme, stamp};
    use crate::theme::assets;
    use crate::{ModuleSet, generate};
    use glam::Vec2;
    use terra_gen::Seed;

    /// The props a theme's dressing laid: those of its stamp not in the stamp undressed.
    #[test]
    fn rooms_are_dressed_by_purpose_off_the_cameras_way() {
        for theme in Theme::all(&assets()).unwrap() {
            if theme.dressing.is_empty() {
                continue;
            }
            let bare_theme = Theme {
                dressing: Vec::new(),
                ..theme.clone()
            };
            let set = ModuleSet::new(&theme).unwrap();
            let (mut rooms, mut dressed, mut props, mut nearest) = (0, 0, 0, f32::MAX);
            for s in 0..40 {
                let Ok(dungeon) = generate(&theme, &set, Seed(s)) else {
                    continue;
                };
                let full = stamp(&dungeon, &theme, &kind);
                let bare = stamp(&dungeon, &bare_theme, &kind);
                assert_eq!(
                    full.route, bare.route,
                    "{} seed {s}: the same tour",
                    theme.name
                );
                let laid: Vec<_> = full.props[bare.props.len()..].to_vec();
                props += laid.len();
                // The eye's path, not the floor's route the dressing kept off.
                let flat = |v: glam::Vec3| Vec2::new(v.x, v.z);
                for p in &laid {
                    let gap = full
                        .view
                        .iter()
                        .filter(|v| (v.y - p.at.y).abs() < 3.0)
                        .map(|v| flat(*v).distance(flat(p.at)))
                        .fold(f32::MAX, f32::min);
                    nearest = nearest.min(gap);
                }
                for (r, room) in dungeon.graph.rooms.iter().enumerate() {
                    if !theme.dressing.iter().any(|d| d.roles.contains(&room.role)) {
                        continue;
                    }
                    rooms += 1;
                    let rect = dungeon.layout.rooms[r];
                    let lo = rect.min.as_vec3() * 4.0;
                    let hi =
                        lo + glam::Vec3::new(rect.size.x as f32, 0.0, rect.size.y as f32) * 4.0;
                    let floor = super::super::floor_of(rect.min.y) as f32 * super::super::CUBE;
                    let inside = |p: glam::Vec3| {
                        (p.y - floor).abs() < 1.0
                            && p.x > lo.x
                            && p.x < hi.x
                            && p.z > lo.z
                            && p.z < hi.z
                    };
                    dressed += laid.iter().any(|p| inside(p.at)) as u32;
                }
            }
            println!(
                "{}: {props} props dressing {dressed} of {rooms} rooms, the nearest {nearest:.2} m from the eye's path",
                theme.name
            );
            assert!(dressed * 10 >= rooms * 8, "{}: rooms dressed", theme.name);
            assert!(
                nearest > 0.9,
                "{}: a prop {nearest:.2} m from the eye",
                theme.name
            );
        }
    }
}
