//! The woods' floor past the load's (D622, D619): beyond `REACH_M` of the woods' middle its
//! rocks, logs, stumps and undergrowth stream in round the camera a square piece at a time,
//! laid by the load's own rules with the kit it made (`floor::Kit`), as the woods' trees
//! stream in round it (`terra_world::woods::NearWoods`), and go again once it is far.
//!
//! - **Under the frame's budget.** One piece a frame at most, when the world's streaming lets
//!   one more of the flora in (`Streaming::may_enter`), spawned through its commands so what
//!   it costs is measured and charged; the nearest first.
//! - **Among the trees once they are in.** A piece is laid only when the trees round it have
//!   arrived on the floor (`NearWoods::trees_in`), so what is laid among them finds them.
//! - **The same each time.** A piece rolls its own dice, by where it is, so streamed out and
//!   in again it lays the same things in the same places.
//! - **The floor knows.** What a piece lays and takes off goes through `GrownFloor::change`,
//!   so the ground cover there is grown again round it.

use super::super::floor::{self, Area, Floor, Kit, Lay, MossyScans, Put, Stand, Variants, Ways};
use super::super::shade::NearShadows;
use super::super::{FloorLooks, Foliage};
use super::REACH_M;
use bevy::ecs::system::SystemParam;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use std::time::Instant;
use terra_gen::Seed;
use terra_world::cover::forest::{GrownFloor, NO_BARK_M};
use terra_world::flora_kinds::Flora;
use terra_world::grown::Cards;
use terra_world::instance::InstanceApp;
use terra_world::planet::{MainCamera, Planet};
use terra_world::streaming::{Class, Streaming};
use terra_world::woods::NearWoods;

/// A piece's side, metres: small enough that laying one stays well inside a frame's budget.
const PIECE_M: f32 = 16.0;
/// Pieces are laid whose middle is within this of the camera, past what the undergrowth is
/// drawn to (its farthest kinds' `seen_m`).
const ENTER_M: f32 = 200.0;
/// And go once their middle is this far, so walking to and fro at the edge keeps them.
const LEAVE_M: f32 = 260.0;
/// The cover is grown again this far round a piece's square, as its clumps spill past it.
const SPILL_M: f32 = 4.0;
/// How many pieces are laid between the log's lines of what one costs.
const TOLD_EVERY: u32 = 64;

/// A piece by its place on the start's map: its south-west corner over `PIECE_M`.
type Key = (i32, i32);

pub(super) fn plugin(app: &mut App) {
    app.world_resource::<FloorStream>().add_systems(
        Update,
        stream
            .run_if(terra_world::instance::in_world)
            .run_if(resource_exists::<FloorStream>),
    );
}

/// What streams the woods' floor in past the load's: what it is laid with, where, the pieces
/// laid and the plants cut in each.
#[derive(Resource)]
pub(super) struct FloorStream {
    kit: Kit,
    seed: Seed,
    /// The woods' middle on the start's map, within `REACH_M` of which the load laid it all.
    middle: Vec2,
    start: Vec3,
    east: Vec3,
    south: Vec3,
    pieces: HashMap<Key, Piece>,
    /// The plants of each piece cut or cleared away, by their place in its order of plants.
    cut: HashMap<Key, HashSet<u32>>,
    /// The pieces still to lay round the camera's piece, nearest first, and that piece.
    waiting: Vec<Key>,
    here: Option<Key>,
    /// Pieces laid since the log last said, and the main thread's milliseconds they took in
    /// all and at most.
    cost: (u32, f32, f32),
}

/// A piece laid: what it put, and where its plants were filed for their near shadows.
struct Piece {
    put: Put,
    shadows: Vec<((i32, i32), Entity)>,
}

impl FloorStream {
    pub(super) fn new(kit: Kit, seed: Seed, middle: Vec2, (start, east, south): Frame) -> Self {
        Self {
            kit,
            seed,
            middle,
            start,
            east,
            south,
            pieces: HashMap::default(),
            cut: HashMap::default(),
            waiting: Vec::new(),
            here: None,
            cost: (0, 0.0, 0.0),
        }
    }

    fn frame(&self) -> Frame {
        (self.start, self.east, self.south)
    }

    /// Where a place on the start's map is in the world, at the start's height.
    fn point(&self, at: Vec2) -> Vec3 {
        self.start + self.east * at.x + self.south * at.y
    }

    /// Counts a piece laid in, and says now and then what one costs.
    fn tell(&mut self, ms: f32) {
        let (count, all, most) = &mut self.cost;
        *count += 1;
        *all += ms;
        *most = most.max(ms);
        if *count >= TOLD_EVERY || (self.waiting.is_empty() && *count >= TOLD_EVERY / 4) {
            info!(
                "Woods: floor pieces: {count} streamed in, {:.2} ms each on the main thread, at most {:.2} ({} laid, {} waiting)",
                *all / *count as f32,
                *most,
                self.pieces.len(),
                self.waiting.len()
            );
            self.cost = (0, 0.0, 0.0);
        }
    }
}

/// The start, and east and south of it: the start's map.
pub(super) type Frame = (Vec3, Vec3, Vec3);

/// The piece a place on the map is in.
fn piece_of(at: Vec2) -> Key {
    let key = (at / PIECE_M).floor().as_ivec2();
    (key.x, key.y)
}

/// A piece's south-west corner on the map.
fn low(key: Key) -> Vec2 {
    Vec2::new(key.0 as f32, key.1 as f32) * PIECE_M
}

fn middle_of(key: Key) -> Vec2 {
    low(key) + Vec2::splat(PIECE_M / 2.0)
}

/// The corners a piece's laying or taking off changes the floor round, its clumps' spill in.
fn corners(key: Key) -> [Vec2; 2] {
    [
        low(key) - Vec2::splat(SPILL_M),
        low(key) + Vec2::splat(PIECE_M + SPILL_M),
    ]
}

/// The pieces to lay round the camera at `eye`, nearest first: within `ENTER_M`, not laid,
/// and not wholly within `REACH_M` of the woods' middle, which the load laid.
fn waiting(eye: Vec2, middle: Vec2, laid: &HashMap<Key, Piece>) -> Vec<Key> {
    let (from, to) = (piece_of(eye - ENTER_M), piece_of(eye + ENTER_M));
    let half = PIECE_M / 2.0;
    let mut waiting: Vec<(f32, Key)> = (from.1..=to.1)
        .flat_map(|y| (from.0..=to.0).map(move |x| (x, y)))
        .filter(|key| !laid.contains_key(key))
        .filter_map(|key| {
            let at = middle_of(key);
            let farthest = (at - middle).abs() + Vec2::splat(half);
            let away = at.distance(eye);
            (away <= ENTER_M && farthest.length() > REACH_M).then_some((away, key))
        })
        .collect();
    waiting.sort_by(|a, b| a.0.total_cmp(&b.0));
    waiting.into_iter().map(|(_, key)| key).collect()
}

/// Lays one piece with the kit, by its own dice, leaving out the plants cut before (`cut`):
/// everything tried in its square, none within `REACH_M` of the woods' middle.
fn lay_piece<W: Ways>(
    lay: &mut Lay,
    kit: &Kit,
    map: &mut Floor<W>,
    key: Key,
    (middle, seed): (Vec2, Seed),
    cut: HashSet<u32>,
) -> Put {
    let area = Area {
        middle,
        from_m: 0.0,
        to_m: 0.0,
        inner_m: 0.0,
        outer_m: 0.0,
        among_trees: true,
        square: Some((low(key), PIECE_M)),
        skip_within_m: REACH_M,
    };
    let packed = (u64::from(key.0 as u32) << 32) | u64::from(key.1 as u32);
    let mut dice = seed.derive("floor piece").index(packed).rng();
    let mut put = Put::default();
    put.cut = cut;
    floor::lay_with(lay, kit, map, &area, &mut dice, &mut put);
    put
}

/// Lays a piece as `lay_piece` does, as if the streamed pieces round it (`around`) were not
/// there: what each laid is off the floor while it is laid, then back. So a piece lays the
/// same things wherever the camera came from and whichever of its neighbours were in, against
/// the trees and the load's floor alone; what lies at a piece's edge may overlap what lies
/// over it in the next.
fn lay_alone<W: Ways>(
    lay: &mut Lay,
    kit: &Kit,
    map: &mut Floor<W>,
    (key, at, cut): (Key, (Vec2, Seed), HashSet<u32>),
    around: &[&Put],
) -> Put {
    for put in around {
        take_off(map, put);
    }
    let put = lay_piece(lay, kit, map, key, at, cut);
    for put in around {
        put_back(map, put);
    }
    put
}

/// The pieces next to one, which what it lays at its edge touches.
fn around(key: Key) -> impl Iterator<Item = Key> {
    (-1..=1)
        .flat_map(move |y| (-1..=1).map(move |x| (key.0 + x, key.1 + y)))
        .filter(move |&next| next != key)
}

/// Takes what a piece laid off the floor: its lying, its plants' feet and its stumps.
fn take_off<W: Ways>(map: &mut Floor<W>, put: &Put) {
    for &(at, radius) in &put.lying {
        map.lying.remove(at, radius);
    }
    for &(at, radius) in &put.feet {
        map.feet.remove(at, radius);
    }
    for &(at, _) in &put.stumps {
        map.canopy.remove_stump(at);
    }
}

/// Files what a piece laid on the floor again, as it laid it.
fn put_back<W: Ways>(map: &mut Floor<W>, put: &Put) {
    for &(at, radius) in &put.lying {
        map.lying.add(at, radius);
    }
    for &(at, radius) in &put.feet {
        map.feet.add(at, radius);
    }
    for &(at, trunk_m) in &put.stumps {
        map.canopy.add(Stand::stump(at, trunk_m));
    }
}

/// What a piece is laid with besides the commands: the world's ground, kinds and assets, and
/// the floor's looks.
#[derive(SystemParam)]
struct Laying<'w> {
    planet: Res<'w, Planet>,
    assets: Res<'w, AssetServer>,
    meshes: ResMut<'w, Assets<Mesh>>,
    plain: ResMut<'w, Assets<StandardMaterial>>,
    mossy: ResMut<'w, Assets<floor::Mossy>>,
    mossy_scans: ResMut<'w, MossyScans>,
    flora: Res<'w, Flora>,
    cards: Res<'w, Cards>,
    variants: ResMut<'w, Variants>,
    foliage: ResMut<'w, Assets<Foliage>>,
    looks: Res<'w, FloorLooks>,
}

impl Laying<'_> {
    fn lay<'a, 'w, 's>(
        &'a mut self,
        commands: &'a mut Commands<'w, 's>,
        (start, east, south): Frame,
        seed: Seed,
    ) -> Lay<'a, 'w, 's> {
        Lay {
            commands,
            planet: &self.planet,
            start,
            east,
            south,
            assets: &self.assets,
            meshes: &mut self.meshes,
            plain: &mut self.plain,
            mossy: &mut self.mossy,
            mossy_scans: &mut self.mossy_scans,
            flora: &self.flora,
            cards: &self.cards,
            variants: &mut self.variants,
            bark: self.looks.bark.clone(),
            pine_bark: self.looks.pine_bark.clone(),
            spruce_bark: self.looks.spruce_bark.clone(),
            atlas: self.looks.floor_cards.clone(),
            thicket: self.looks.thicket_cards.clone(),
            seed,
            foliage: &mut self.foliage,
            shrinking: HashMap::default(),
            shade: NearShadows::new(start, east, south),
            sun: None,
        }
    }
}

/// Lays the nearest piece waiting round the camera whose trees are in, one a frame under the
/// streaming's budget, and takes off those far behind it as it moves on.
#[allow(clippy::too_many_arguments)]
fn stream(
    mut commands: Commands,
    mut streaming: ResMut<Streaming>,
    mut stream: ResMut<FloorStream>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    (near, floor, mut shadows): (
        Option<Res<NearWoods>>,
        Option<ResMut<GrownFloor>>,
        Option<ResMut<NearShadows>>,
    ),
    mut laying: Laying,
) {
    let (Ok(camera), Some(mut floor)) = (camera.single(), floor) else {
        return;
    };
    let stream = &mut *stream;
    let off = camera.translation() - stream.start;
    let eye = Vec2::new(off.dot(stream.east), off.dot(stream.south));
    let here = piece_of(eye);
    if stream.here != Some(here) {
        stream.here = Some(here);
        let far: Vec<Key> = stream
            .pieces
            .keys()
            .copied()
            .filter(|&key| middle_of(key).distance(eye) > LEAVE_M)
            .collect();
        for key in far {
            let Some(piece) = stream.pieces.remove(&key) else {
                continue;
            };
            for &thing in &piece.put.things {
                commands.entity(thing).try_despawn();
            }
            floor.change(corners(key), |map| take_off(map, &piece.put));
            if let Some(shadows) = shadows.as_mut() {
                shadows.remove(&piece.shadows);
            }
        }
        stream.waiting = waiting(eye, stream.middle, &stream.pieces);
    }
    if stream.waiting.is_empty() || !streaming.may_enter(Class::Flora) {
        return;
    }
    let reach = PIECE_M * std::f32::consts::FRAC_1_SQRT_2 + NO_BARK_M;
    let ready = stream.waiting.iter().position(|&key| {
        near.as_ref()
            .is_none_or(|near| near.trees_in(stream.point(middle_of(key)), reach))
    });
    let Some(k) = ready else {
        return;
    };
    let key = stream.waiting.remove(k);
    let began = Instant::now();
    let cut = stream.cut.get(&key).cloned().unwrap_or_default();
    let mut put = Put::default();
    let shade = {
        let mut commands = streaming.commands(Class::Flora, &commands);
        let mut lay = laying.lay(&mut commands, stream.frame(), stream.seed);
        let (kit, at) = (&stream.kit, (stream.middle, stream.seed));
        let next: Vec<&Put> = around(key)
            .filter_map(|next| stream.pieces.get(&next).map(|piece| &piece.put))
            .collect();
        floor.change(corners(key), |map| {
            put = lay_alone(&mut lay, kit, map, (key, at, cut), &next);
        });
        lay.shade
    };
    let shadows = shadows.map_or_else(Vec::new, |mut live| live.merge(shade));
    stream.pieces.insert(key, Piece { put, shadows });
    streaming.entered(Class::Flora, began, 0);
    stream.tell(began.elapsed().as_secs_f32() * 1000.0);
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
