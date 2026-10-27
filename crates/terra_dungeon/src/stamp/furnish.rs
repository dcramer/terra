//! A room dressed as its room type (the theme's `dressing`, docs/dungeons.md's room types,
//! D539), as a level designer sets dressing out once the room's kit is in: its hero prop on
//! the wall that faces the way in, lit by a lamp of its own where no torch is by it, a
//! vignette that tells what happened there, and wall props and clutter in groups round the
//! hero and the far corner, the walls between bare; all off the tour and the doorways and
//! apart from what already stands.

use super::modules::{Class, Look, feature_at};
use super::{CELL, CUBE, Frame, Light, LightKind, MID, Prop, Stamp, hash, stands};
use crate::Dungeon;
use crate::place::{Layout, Side, opposite};
use crate::theme::Dress;
use glam::{IVec3, Vec2, Vec3};
use std::collections::HashMap;
use terra_build::Cube;

/// How far a prop keeps from the tour's line, in metres, so the camera never passes through
/// one: the eye keeps 0.5 m off walls, and a prop stands out from a wall at most 0.6 m.
pub(crate) const OFF_ROUTE_M: f32 = 1.3;
/// From a doorway's middle, so none stands in the way in.
pub(crate) const OFF_DOOR_M: f32 = 1.6;
/// From what already stands (props, objects, chests, fires) and from each other.
const APART_M: f32 = 1.0;
/// Clutter from everything: small things lie closer.
const CLUTTER_APART_M: f32 = 0.6;
/// What a hero keeps clear round it, so it reads alone: a coffin's or an altar's length.
const HERO_M: f32 = 1.5;
/// How near a light stands to a hero for it to read lit, in metres: a torch on the wall over
/// it or beside it, not one across the room.
pub(crate) const HERO_LIT_M: f32 = 3.0;
/// A lamp's light over its prop's foot: at its flames, a candle's or a lantern's.
const LAMP_UP_M: f32 = 0.4;
/// Where a hero's lamp may stand from the hero's wall spot, (along, out) in metres, the first
/// that is clear: past either end of a coffin 2 m long, then before it, then farther along.
const LAMP_BY_WALL: [(f32, f32); 10] = [
    (1.6, 0.0),
    (-1.6, 0.0),
    (1.5, 0.6),
    (-1.5, 0.6),
    (0.8, 1.45),
    (-0.8, 1.45),
    (2.2, 0.2),
    (-2.2, 0.2),
    (0.0, 1.7),
    (2.0, 1.0),
];
/// The directions round a laid prop, a turn in eight from east (+X), south (+Z) second: a
/// hero's lamp stands on one where its wall gives no place, and its clutter on three, to one
/// side; another prop's clutter on any (`Laying::clutter`). A prop facing `f` (0..4 = north, east, south, west) faces
/// `RING[6 + 2f]`, mod 8.
const RING: [(f32, f32); 8] = [
    (1.0, 0.0),
    (0.71, 0.71),
    (0.0, 1.0),
    (-0.71, 0.71),
    (-1.0, 0.0),
    (-0.71, -0.71),
    (0.0, -1.0),
    (0.71, -0.71),
];
/// A vignette's pieces drawn together along its wall by these, in turn, to fit a short one:
/// no closer, or a shroud and the axes laid by it overlap.
const VIGNETTE_DRAWN: [f32; 2] = [1.0, 0.85];
/// How near a room's focus (its hero or fire) a vignette is laid first, in metres, from the
/// wall spot or floor point it stands from: a hero's clearance and two wall spots.
const FOCUS_M: f32 = 3.0;
/// Clutter lies so much past what a prop keeps clear round it.
const RING_GAP_M: f32 = 0.1;
/// How far past a hero's clearance no clutter but its own lies, in metres: its own lies
/// `RING_GAP_M` past, on one side (`Laying::clutter`).
const HERO_SIDE_M: f32 = 0.4;
/// How far apart the walls' feet's debris keeps from each other and all else, in metres: a
/// sparse strew, a piece every third cell, never a carpet.
const SCATTER_APART_M: f32 = 1.2;

/// Twice how far a piece of debris is nudged along its wall at most, each way, in metres; it
/// keeps this much farther from the tour's line than `OFF_ROUTE_M`, since the eye swings off
/// that line.
const SCATTER_NUDGE_M: f32 = 0.35;

/// What a campfire keeps clear round it, in metres: its ring of stones and a step to sit by
/// it, so the camp's bedroll lies beside the fire, not in it.
const FIRE_M: f32 = 1.1;

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

    /// How far a point is from the room's nearest doorway.
    fn near_door(&self, p: Vec3) -> f32 {
        (self.doors.iter().map(|d| d.distance(p))).fold(99.0, f32::min)
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

/// The middles of the doorways and open ways out of a room's `cells`, at the floor.
pub(super) fn doorways(layout: &Layout, cells: &[(IVec3, usize)]) -> Vec<Vec3> {
    (cells.iter())
        .flat_map(|&(at, c)| {
            let sides = layout.cells[c].sides;
            (0..4)
                .filter(move |&d| matches!(sides[d], Side::Door(_) | Side::Way))
                .map(move |d| Frame::new(at, d).point(MID, 0.0, 0.0))
        })
        .collect()
}

/// Dresses each room, and gives back its focus: each hero's and vignette piece's place, for the
/// mobs to keep clear of (`runin::FOCUS_CLEAR_M`).
pub(super) fn lay(
    dungeon: &Dungeon,
    dressing: &[Dress],
    looks: &[Look],
    map: &HashMap<IVec3, Cube>,
    out: &mut Stamp,
) -> Vec<Vec3> {
    let layout = &dungeon.layout;
    let fires = out.lights.iter().filter(|l| l.kind != LightKind::Torch);
    let keep = |l: &Light| match l.kind {
        LightKind::Campfire => (l.at, FIRE_M),
        _ => (l.at, 0.0),
    };
    let mut focus = Vec::new();
    let mut taken: Vec<(Vec3, f32)> = (out.props.iter().map(|p| p.at))
        .chain(out.objects.iter().map(|o| o.at))
        .chain(out.chests.iter().map(|c| c.at))
        .map(|p| (p, 0.0))
        .chain(fires.map(keep))
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
        let place = Room {
            route: &route,
            doors: doorways(layout, &cells),
            map,
        };
        let near_door = |p: Vec3| place.near_door(p);
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
        let laying = Laying {
            dresses: &dresses,
            spots: &spots,
            corners: &corners,
            floor: &floor,
            place: &place,
            cells: cells.len(),
            min: rect.min,
        };
        let first = out.props.len();
        // Its feature lit by its own dressings' lamp, else the theme's (a tomb room's dressing
        // may set none); the goal and a rest have fires of their own.
        let lamp = (dresses.iter().copied().chain(dressing)).find_map(|d| d.lamp.as_ref());
        let features = cells
            .iter()
            .filter_map(|&(_, c)| feature_at(&layout.cells[c], looks[c]));
        for at in features {
            if let Some(lamp) = lamp {
                laying.light_feature(at, lamp, &mut taken, out);
            }
        }
        let hero = laying.hero(featured, &mut taken, out);
        focus.extend(hero);
        // The vignette by the room's focus: its hero, else its fire (a camp round its fire).
        let fire = (cells.iter())
            .find(|&&(_, c)| looks[c].class == Class::Rest)
            .map(|&(at, _)| Frame::new(at, 0).point(MID, 0.0, MID));
        let pieces = out.props.len();
        let told = laying.vignette(hero.or(fire), &mut taken, out);
        focus.extend(out.props[pieces..].iter().map(|p| p.at));
        // The room's groups: round its hero (or its vignette, or the feature's first wall
        // spot), and in the far corner from it.
        let anchor = hero.or(told).or(spots.first().map(|s| s.p));
        laying.wall_props(anchor, &mut taken, out);
        laying.clutter(first, hero, &mut taken, out);
        laying.scatter(&mut taken, out);
    }
    focus
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

    /// The hero, unless the room's feature or tomb is: the wall spot farthest from the
    /// doorways, a long one's floor first, or else the open floor farthest from them; and its
    /// lamp where no light stands by it.
    fn hero(&self, featured: bool, taken: &mut Vec<(Vec3, f32)>, out: &mut Stamp) -> Option<Vec3> {
        let dress = (self.dresses.iter()).find(|d| !d.hero.is_empty() && !featured)?;
        let mut order = self.spots.to_vec();
        order.sort_by(|a, b| b.wide.cmp(&a.wide).then(b.door.total_cmp(&a.door)));
        let mut open = self.floor.to_vec();
        let near_door = |p: Vec3| self.place.near_door(p);
        open.sort_by(|a, b| near_door(b.1).total_cmp(&near_door(a.1)));
        let wall = order.iter().find(|s| free(taken, s.p, APART_M)).copied();
        let (p, facing) = (wall.map(|s| (s.p, opposite(s.d)))).or_else(|| {
            let mut open = open.iter();
            open.find(|o| free(taken, o.1, APART_M)).map(|o| (o.1, o.2))
        })?;
        put(out, pick(&dress.hero, self.salt(530)), p, facing);
        taken.push((p, HERO_M));
        if let Some(lamp) = &dress.lamp {
            let lit = out.lights.iter().any(|l| l.at.distance(p) < HERO_LIT_M);
            if !lit {
                self.light_hero(wall, p, lamp, taken, out);
            }
        }
        Some(p)
    }

    /// The room's feature (a tomb, the key's altar) lit as a hero is, by the first of its
    /// dressings' lamps, where no fire or lamp stands within `HERO_LIT_M` of it. A torch does
    /// not count: hung over a tomb it lights the wall and the lid, and the crypt's
    /// antechamber's sarcophagus under one still read as a dark box (barrow dress round 4,
    /// all three reviewers).
    fn light_feature(
        &self,
        at: Vec3,
        lamp: &(String, f32),
        taken: &mut Vec<(Vec3, f32)>,
        out: &mut Stamp,
    ) {
        let fire = |l: &Light| l.kind != LightKind::Torch && l.at.distance(at) < HERO_LIT_M;
        if out.lights.iter().any(fire) {
            return;
        }
        self.light_hero(None, at, lamp, taken, out);
    }

    /// A lamp beside the hero, past its end along its wall or before it, with its light: as a
    /// set dresser lights a focal prop from a source in the fiction. Lit only by a torch
    /// across the room, the crypt's robbed tomb rendered near black (barrow dress round 2,
    /// all three reviewers).
    fn light_hero(
        &self,
        wall: Option<Spot>,
        hero: Vec3,
        lamp: &(String, f32),
        taken: &mut Vec<(Vec3, f32)>,
        out: &mut Stamp,
    ) {
        let beside: Vec<(Vec3, usize)> = match wall {
            Some(s) => (LAMP_BY_WALL.iter())
                .map(|&(along, o)| (s.offset(along, o), opposite(s.d)))
                .collect(),
            None => (RING.iter())
                .map(|&(x, z)| {
                    let p = hero + Vec3::new(x, 0.0, z) * (HERO_M + RING_GAP_M);
                    (p, toward(hero, &self.place.doors))
                })
                .collect(),
        };
        let found = (beside.into_iter())
            .find(|&(p, _)| self.place.clear(p) && free(taken, p, CLUTTER_APART_M));
        if let Some((p, facing)) = found {
            put(out, lamp, p, facing);
            taken.push((p, CLUTTER_APART_M));
            out.lights.push(Light {
                at: p + Vec3::Y * LAMP_UP_M,
                kind: LightKind::Lamp,
                facing,
            });
        }
    }

    /// A vignette at the first wall spot where it fits, nearest the room's `focus` first (its
    /// hero, or its fire): all of it, then drawn closer, then its last pieces dropped one by
    /// one down to its first and one beside it; else so on the open floor, facing the nearest
    /// doorway; where it stands. Laid all or none at a wall spot, the crypt's guard post's,
    /// 2.2 m wide, fitted none on seed 7 (barrow dress 13); at a spot by its hash, the camp's
    /// bedroll lay a cell from its fire and the fallen guard across the room from the bench,
    /// so no frame held the two (barrow dress 15).
    fn vignette(
        &self,
        focus: Option<Vec3>,
        taken: &mut Vec<(Vec3, f32)>,
        out: &mut Stamp,
    ) -> Option<Vec3> {
        let (mut spots, mut floor) = (self.spots.to_vec(), self.floor.to_vec());
        if let Some(f) = focus {
            spots.sort_by(|a, b| a.p.distance(f).total_cmp(&b.p.distance(f)));
            floor.sort_by(|a, b| a.1.distance(f).total_cmp(&b.1.distance(f)));
        }
        for dress in self.dresses.iter().filter(|d| !d.vignette.is_empty()) {
            let n = dress.vignette.len();
            let tries: Vec<(usize, f32)> = (n.min(2)..=n)
                .rev()
                .flat_map(|keep| VIGNETTE_DRAWN.map(|k| (keep, k)))
                .collect();
            // Its pieces from a place on the floor, whose frame's wall side `d` it backs onto.
            let fits = |p: Vec3, d: usize, (keep, k): (usize, f32)| {
                let f = Frame::new(IVec3::ZERO, d);
                let o = f.point(0.0, 0.0, 0.0);
                let (along, away) = (f.point(1.0, 0.0, 0.0) - o, f.point(0.0, 0.0, 1.0) - o);
                let at: Vec<Vec3> = (dress.vignette[..keep].iter())
                    .map(|(_, _, (a, w))| p + (along * *a * k + away * *w) / CUBE)
                    .collect();
                at.iter()
                    .all(|&p| self.place.clear(p) && free(taken, p, CLUTTER_APART_M))
                    .then_some((at, opposite(d)))
            };
            // By the focus, as whole as fits at the nearest spot; then whole anywhere first.
            let by = |p: Vec3| focus.is_some_and(|f| p.distance(f) < FOCUS_M);
            let near = (spots.iter().filter(|s| by(s.p)).map(|s| (s.p, s.d)))
                .chain((floor.iter().filter(|o| by(o.1))).map(|o| (o.1, opposite(o.2))))
                .find_map(|(p, d)| (tries.iter()).find_map(|&t| fits(p, d, t).map(|f| (p, f))));
            let walls = near.or_else(|| {
                (tries.iter())
                    .find_map(|&t| (spots.iter()).find_map(|s| fits(s.p, s.d, t).map(|f| (s.p, f))))
            });
            let found = walls.or_else(|| {
                (tries.iter()).find_map(|&t| {
                    (floor.iter())
                        .find_map(|&(_, p, facing)| fits(p, opposite(facing), t).map(|f| (p, f)))
                })
            });
            if let Some((p, (at, facing))) = found {
                for ((model, scale, _), q) in dress.vignette.iter().zip(at) {
                    put(out, &(model.clone(), *scale), q, facing);
                    taken.push((q, CLUTTER_APART_M));
                }
                return Some(p);
            }
        }
        None
    }

    /// Wall props, so many per four cells, in two groups as a set dresser lays them: by the
    /// room's anchor (its hero, or what stands for it) and by the wall spot farthest from it,
    /// nearest each first, the walls between left bare. Taken by a hash along every wall
    /// foot, the crypt's rooms read as lined with props a metre apart (barrow dress round 2).
    fn wall_props(&self, anchor: Option<Vec3>, taken: &mut Vec<(Vec3, f32)>, out: &mut Stamp) {
        let Some(near) = anchor else { return };
        let from = |p: Vec3| p.distance(near);
        let far = (self.spots.iter().map(|s| s.p)).max_by(|a, b| from(*a).total_cmp(&from(*b)));
        let far = far.unwrap_or(near);
        let mut order: Vec<&Spot> = self.spots.iter().collect();
        let group = |s: &Spot| s.p.distance(near).min(s.p.distance(far));
        order.sort_by(|a, b| group(a).total_cmp(&group(b)));
        for (k, dress) in (self.dresses.iter().enumerate()).filter(|(_, d)| !d.props.is_empty()) {
            let (least, most) = dress.count;
            let per = least + self.salt(520 + k as u32) % (most.saturating_sub(least) + 1);
            let wanted = (per as usize * self.cells).div_ceil(4);
            let mut laid = 0;
            for &s in &order {
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

    /// Clutter a cell, by what the room's dressing laid from `first` on, a piece by each in
    /// turn so every group takes its share (a hero's group, a corner's, a vignette's); then
    /// between the wall props and in the corners where those give too few places, never the
    /// open floor, where it lay alone as the confetti every reviewer named (barrow dress
    /// round 18). By the hero it lies to one side, out on that side and then along its wall,
    /// as a set dresser heaps clutter against a thing; laid on each of a ring's eight ways in
    /// turn, it ringed the crypt's heroes with pebbles (dress round 17); and nothing else lies
    /// within `HERO_SIDE_M` past the hero's clearance, so the pieces between the wall props
    /// did not flank it from its other side. By any other prop it takes the eight ways in
    /// turn: on three, the groups of three or more fell from 92.5 to 87% (dress round 19).
    fn clutter(
        &self,
        first: usize,
        hero: Option<Vec3>,
        taken: &mut Vec<(Vec3, f32)>,
        out: &mut Stamp,
    ) {
        for (k, dress) in (self.dresses.iter().enumerate()).filter(|(_, d)| !d.clutter.is_empty()) {
            let (least, most) = dress.litter;
            let per = least + self.salt(550 + k as u32) % (most.saturating_sub(least) + 1);
            let wanted = per as usize * self.cells;
            let keep = |p: Vec3| (taken.iter().find(|t| t.0 == p)).map_or(0.0, |t| t.1);
            let rings: Vec<Vec<(u32, Vec3, usize, bool)>> = (out.props[first..].iter())
                .map(|prop| {
                    let r = keep(prop.at).max(CLUTTER_APART_M) + RING_GAP_M;
                    let start = hash((prop.at / CUBE).floor().as_ivec3(), 570 + k as u32);
                    let own = hero == Some(prop.at);
                    let at = |j: usize, way: usize| {
                        let (x, z) = RING[way % RING.len()];
                        let p = prop.at + Vec3::new(x, 0.0, z) * r;
                        (start.wrapping_add(j as u32), p, (prop.facing + j) % 4, own)
                    };
                    if !own {
                        // Each of the eight ways in turn, from one its hash picks.
                        return (0..RING.len()).map(|j| at(j, start as usize + j)).collect();
                    }
                    // The hero's out on one side, then beside it along its wall (often the
                    // next wall prop's place), then before it.
                    let front = 6 + 2 * prop.facing;
                    let turn = if start.is_multiple_of(2) { 1 } else { 7 };
                    [1, 2, 0].map(|j| at(j, front + turn * j)).to_vec()
                })
                .collect();
            let round = (0..RING.len())
                .flat_map(|j| rings.iter().filter_map(move |ring| ring.get(j).copied()));
            // Between the wall props and a little out from them.
            let by_walls = self.spots.iter().flat_map(|s| {
                let facing = (opposite(s.d) + s.rank as usize) % 4;
                [-0.5, 0.5].map(|along| (s.rank, s.offset(along, 0.45), facing))
            });
            let rest = (by_walls.chain(self.corners.iter().copied()))
                .map(|(rank, p, facing)| (rank, p, facing, false));
            let by_hero = |p: Vec3| hero.is_some_and(|h| h.distance(p) < HERO_M + HERO_SIDE_M);
            let mut laid = 0;
            for (rank, p, facing, own) in round.chain(rest) {
                if laid == wanted {
                    break;
                }
                let mine = own || !by_hero(p);
                if mine && self.place.clear(p) && free(taken, p, CLUTTER_APART_M) {
                    put(out, pick(&dress.clutter, rank), p, facing);
                    taken.push((p, 0.0));
                    laid += 1;
                }
            }
        }
    }

    /// Debris at the walls' feet, last, so the groups keep their places: a piece for every
    /// three cells at `strewn` a cell, by the wall spots taken by their hash and then the
    /// corners, never the open floor, since what falls or is swept aside comes to rest against
    /// a wall (barrow dress round 17: strewn over the middle, every reviewer read it as
    /// confetti). Each is nudged along its wall by up to half `SCATTER_NUDGE_M` so the pieces do not
    /// stand in rows, `SCATTER_APART_M` from each other and all else, off the tour and the
    /// doorways.
    fn scatter(&self, taken: &mut Vec<(Vec3, f32)>, out: &mut Stamp) {
        let unit = |h: u32| (h & 0xffff) as f32 / 65535.0 * 2.0 - 1.0;
        let feet: Vec<(u32, Vec3)> = (self.spots.iter())
            .flat_map(|s| {
                [(-0.5, 0), (0.5, 1)].map(|(along, i)| {
                    let rank = s.rank.wrapping_add(i * 0x9e37_79b9);
                    let along = along + unit(rank) * SCATTER_NUDGE_M * 0.5;
                    (rank, s.offset(along, unit(rank >> 16).abs() * 0.15))
                })
            })
            .chain(self.corners.iter().map(|&(rank, p, _)| (rank, p)))
            .collect();
        for (k, dress) in (self.dresses.iter().enumerate()).filter(|(_, d)| !d.scatter.is_empty()) {
            let (least, most) = dress.strewn;
            let per = least + self.salt(580 + k as u32) % (most.saturating_sub(least) + 1);
            let wanted = (per as usize * self.cells).div_ceil(3);
            let mut laid = 0;
            for &(rank, p) in &feet {
                if laid == wanted {
                    break;
                }
                let off_way = off(self.place.route, p) > OFF_ROUTE_M + SCATTER_NUDGE_M;
                if off_way && self.place.clear(p) && free(taken, p, SCATTER_APART_M) {
                    put(out, pick(&dress.scatter, rank / 7), p, (rank % 4) as usize);
                    taken.push((p, 0.0));
                    laid += 1;
                }
            }
        }
    }
}

/// Whether a cell's module leaves its walls bare for dressing: a rest's fire stands in its
/// cell's middle, its walls free for the camp laid round it.
fn bare(class: Class) -> bool {
    matches!(
        class,
        Class::Plain | Class::Runner | Class::Decor | Class::Pillar | Class::Rest
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
