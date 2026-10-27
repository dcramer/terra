//! A room dressed as its room type (the theme's `dressing`, docs/dungeons.md's room types,
//! D539), as a level designer sets dressing out once the room's kit is in: its hero prop on
//! the wall that faces the way in, a vignette that tells what happened there, wall props in
//! rhythm along its bare walls and clutter in its corners; all off the tour and the doorways
//! and apart from what already stands.

use super::modules::{Class, Look};
use super::{CELL, CUBE, Frame, LightKind, MID, Prop, Stamp, hash, stands};
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
pub(crate) const OFF_DOOR_M: f32 = 1.6;
/// From what already stands (props, objects, chests, mobs, fires) and from each other.
const APART_M: f32 = 1.0;
/// Clutter from everything: small things lie closer.
const CLUTTER_APART_M: f32 = 0.6;
/// What a hero keeps clear round it, so it reads alone: a coffin's or an altar's length.
const HERO_M: f32 = 1.5;
/// Out from the wall's cell edge, in cubes: 0.3 m clear of its face.
const OUT: f32 = 1.6;

/// A place against a bare wall: its cell and the wall's side, how far along it and out, in
/// cubes of the wall's frame, the point, and how far it is from the room's nearest doorway.
#[derive(Clone, Copy)]
struct Spot {
    rank: u32,
    at: IVec3,
    d: usize,
    x: f32,
    p: Vec3,
    door: f32,
    /// Floor for a long prop: two cubes either side.
    wide: bool,
}

impl Spot {
    /// A point from this spot, along its wall and out from it, in metres.
    fn offset(self, along: f32, out: f32) -> Vec3 {
        Frame::new(self.at, self.d).point(self.x + along / CUBE, 0.0, OUT + out / CUBE)
    }
}

/// What a room's dressing must keep clear of, and where it may stand.
struct Room<'a> {
    route: &'a [Vec3],
    doors: Vec<Vec3>,
    map: &'a HashMap<IVec3, Cube>,
}

impl Room<'_> {
    /// Off the tour and the doorways, on a floor with headroom.
    fn clear(&self, p: Vec3) -> bool {
        off(self.route, p) > OFF_ROUTE_M
            && self.doors.iter().all(|d| d.distance(p) > OFF_DOOR_M)
            && stands(self.map, (p / CUBE).floor().as_ivec3())
    }
}

/// Whether a point keeps `gap` from everything taken, and each taken thing's own clearance.
fn free(taken: &[(Vec3, f32)], p: Vec3, gap: f32) -> bool {
    taken
        .iter()
        .all(|&(t, keep)| t.distance(p) >= gap.max(keep))
}

fn put(out: &mut Stamp, (model, scale): &(String, f32), at: Vec3, facing: usize) {
    out.props.push(Prop {
        model: model.clone(),
        at,
        facing,
        scale: *scale,
    });
}

/// The side facing the nearest of `doors` from `p`, 0..4 = north, east, south, west.
fn toward(p: Vec3, doors: &[Vec3]) -> usize {
    let near = doors
        .iter()
        .min_by(|a, b| a.distance(p).total_cmp(&b.distance(p)));
    let Some(d) = near.map(|d| *d - p) else {
        return 2;
    };
    match (d.x.abs() > d.z.abs(), d.x > 0.0, d.z > 0.0) {
        (true, true, _) => 1,
        (true, false, _) => 3,
        (false, _, true) => 2,
        (false, _, false) => 0,
    }
}

fn pick(list: &[(String, f32)], salt: u32) -> &(String, f32) {
    &list[salt as usize % list.len()]
}

pub(super) fn lay(
    dungeon: &Dungeon,
    dressing: &[Dress],
    looks: &[Look],
    map: &HashMap<IVec3, Cube>,
    out: &mut Stamp,
) {
    let layout = &dungeon.layout;
    let fires = out.lights.iter().filter(|l| l.kind != LightKind::Torch);
    let mut taken: Vec<(Vec3, f32)> = (out.props.iter().map(|p| p.at))
        .chain(out.objects.iter().map(|o| o.at))
        .chain(out.chests.iter().map(|c| c.at))
        .chain(out.mobs.iter().map(|m| m.at))
        .chain(fires.map(|l| l.at))
        .map(|p| (p, 0.0))
        .collect();
    let route = out.route.clone();
    for (r, room) in dungeon.graph.rooms.iter().enumerate() {
        let dresses: Vec<&Dress> = dressing
            .iter()
            .filter(|d| d.roles.contains(&room.role))
            .collect();
        if dresses.is_empty() {
            continue;
        }
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
        let place = Room {
            route: &route,
            doors,
            map,
        };
        let near_door = |p: Vec3| {
            place
                .doors
                .iter()
                .map(|d| d.distance(p))
                .fold(99.0, f32::min)
        };
        // Against each walled side of a bare cell, a metre, two and three along it; and the
        // corners where two walls meet, for clutter.
        let (mut spots, mut corners) = (Vec::new(), Vec::new());
        for &(at, c) in cells.iter().filter(|&&(_, c)| bare(looks[c].class)) {
            let sides = layout.cells[c].sides;
            for d in (0..4).filter(|&d| sides[d] == Side::Wall) {
                let f = Frame::new(at, d);
                for x in [2, 4, 6] {
                    let p = f.point(x as f32, 0.0, OUT);
                    let floor = (x - 1..=x).all(|x| stands(map, f.cube(x, 0, 1)));
                    if floor && place.clear(p) {
                        let wide = (x - 2..=x + 1).all(|x| x > 0 && x < CELL - 1)
                            && (x - 2..=x + 1).all(|x| stands(map, f.cube(x, 0, 1)));
                        let rank = hash(at, 500 + 8 * d as u32 + x as u32);
                        let (x, door) = (x as f32, near_door(p));
                        spots.push(Spot {
                            rank,
                            at,
                            d,
                            x,
                            p,
                            door,
                            wide,
                        });
                    }
                }
                if sides[(d + 1) % 4] == Side::Wall {
                    let p = f.point(CELL as f32 - OUT, 0.0, OUT);
                    if place.clear(p) {
                        corners.push((hash(at, 540 + d as u32), p, opposite(d)));
                    }
                }
            }
        }
        spots.sort_by_key(|s| (s.rank, s.p.to_array().map(f32::to_bits), s.d));
        corners.sort_by_key(|&(rank, p, d)| (rank, p.to_array().map(f32::to_bits), d));
        let salt = |k: u32| hash(rect.min, k);

        // The hero, unless the room's feature or tomb is: the wall spot farthest from the
        // doorways, a long one's floor first.
        let featured = cells.iter().any(|&(_, c)| {
            matches!(
                looks[c].class,
                Class::Goal | Class::Key | Class::Rest | Class::Sarcophagus
            )
        });
        // Open floor, for what finds no bare wall: a room of niches, rubble or racks.
        let mut floor = Vec::new();
        for &(at, c) in &cells {
            if matches!(looks[c].class, Class::Collapse | Class::Stair) {
                continue;
            }
            let f = Frame::new(at, 0);
            for (x, z) in [2, 4, 6].into_iter().flat_map(|x| [(x, 2), (x, 4), (x, 6)]) {
                let p = f.point(x as f32, 0.0, z as f32);
                if place.clear(p) {
                    floor.push((
                        hash(at, 560 + (x * CELL + z) as u32),
                        p,
                        toward(p, &place.doors),
                    ));
                }
            }
        }
        floor.sort_by_key(|&(rank, p, d)| (rank, p.to_array().map(f32::to_bits), d));
        if let Some(dress) = dresses.iter().find(|d| !d.hero.is_empty() && !featured) {
            let mut order = spots.clone();
            order.sort_by(|a, b| b.wide.cmp(&a.wide).then(b.door.total_cmp(&a.door)));
            let mut open = floor.clone();
            open.sort_by(|a, b| near_door(b.1).total_cmp(&near_door(a.1)));
            let at = (order.iter())
                .find(|s| free(&taken, s.p, APART_M))
                .map(|s| (s.p, opposite(s.d)))
                .or_else(|| {
                    let mut open = open.iter();
                    open.find(|o| free(&taken, o.1, APART_M))
                        .map(|o| (o.1, o.2))
                });
            if let Some((p, facing)) = at {
                put(out, pick(&dress.hero, salt(530)), p, facing);
                taken.push((p, HERO_M));
            }
        }
        let laying = Laying {
            dresses: &dresses,
            spots: &spots,
            corners: &corners,
            floor: &floor,
            place: &place,
            cells: cells.len(),
            min: rect.min,
        };
        laying.vignette(&mut taken, out);
        laying.wall_props(&mut taken, out);
        laying.clutter(&mut taken, out);
    }
}

/// One room's places and dressings, for laying its vignette, wall props and clutter.
struct Laying<'a> {
    dresses: &'a [&'a Dress],
    spots: &'a [Spot],
    corners: &'a [(u32, Vec3, usize)],
    floor: &'a [(u32, Vec3, usize)],
    place: &'a Room<'a>,
    cells: usize,
    min: IVec3,
}

impl Laying<'_> {
    fn salt(&self, k: u32) -> u32 {
        hash(self.min, k)
    }

    /// A vignette at the first spot where all of it fits.
    fn vignette(&self, taken: &mut Vec<(Vec3, f32)>, out: &mut Stamp) {
        for dress in self.dresses.iter().filter(|d| !d.vignette.is_empty()) {
            let fits = |s: &Spot| {
                let at: Vec<Vec3> = dress
                    .vignette
                    .iter()
                    .map(|(_, _, (along, o))| s.offset(*along, *o))
                    .collect();
                at.iter()
                    .all(|&p| self.place.clear(p) && free(taken, p, CLUTTER_APART_M))
                    .then_some(at)
            };
            let found = self.spots.iter().find_map(|s| fits(s).map(|at| (*s, at)));
            if let Some((s, at)) = found {
                for ((model, scale, _), p) in dress.vignette.iter().zip(at) {
                    put(out, &(model.clone(), *scale), p, opposite(s.d));
                    taken.push((p, CLUTTER_APART_M));
                }
                return;
            }
        }
    }

    /// Wall props, so many per four cells.
    fn wall_props(&self, taken: &mut Vec<(Vec3, f32)>, out: &mut Stamp) {
        for (k, dress) in (self.dresses.iter().enumerate()).filter(|(_, d)| !d.props.is_empty()) {
            let (least, most) = dress.count;
            let per = least + self.salt(520 + k as u32) % (most.saturating_sub(least) + 1);
            let wanted = (per as usize * self.cells).div_ceil(4);
            let mut laid = 0;
            for s in self.spots {
                if laid == wanted {
                    break;
                }
                if free(taken, s.p, APART_M) {
                    put(out, pick(&dress.props, s.rank), s.p, opposite(s.d));
                    taken.push((s.p, 0.0));
                    laid += 1;
                }
            }
        }
    }

    /// Clutter a cell: the corners first, then by the walls between what stands there.
    fn clutter(&self, taken: &mut Vec<(Vec3, f32)>, out: &mut Stamp) {
        for (k, dress) in (self.dresses.iter().enumerate()).filter(|(_, d)| !d.clutter.is_empty()) {
            let (least, most) = dress.litter;
            let per = least + self.salt(550 + k as u32) % (most.saturating_sub(least) + 1);
            let wanted = per as usize * self.cells;
            // Between the wall props and a little out from them.
            let by_walls = self.spots.iter().flat_map(|s| {
                let facing = (opposite(s.d) + s.rank as usize) % 4;
                [-0.5, 0.5].map(|along| (s.rank, s.offset(along, 0.45), facing))
            });
            let mut laid = 0;
            let candidates = self.corners.iter().copied().chain(by_walls);
            for (rank, p, facing) in candidates.chain(self.floor.iter().copied()) {
                if laid == wanted {
                    break;
                }
                if self.place.clear(p) && free(taken, p, CLUTTER_APART_M) {
                    put(out, pick(&dress.clutter, rank), p, facing);
                    taken.push((p, 0.0));
                    laid += 1;
                }
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
#[path = "furnish_tests.rs"]
mod tests;
