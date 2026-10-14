//! What stops a walker in the forest, fitted to what is drawn (D243, D248, D267).
//!
//! Each kind of model is read once into its relief, the highest point over each cell of a
//! fine grid over its plan (`terra_core::Relief`); each placed thing then fits pieces round
//! the cells taller than a step, convex outlines with rounded edges, and holds a walker up on
//! the rest (`terra_core::Solid`). Trees fit their trunk the same way, cut off where the root
//! flare spreads into the ground and above a body's height, and keep a spruce's skirt.
//!
//! For checking it without a person (D69, `tools/sandbox/collide.ts`), two remote methods
//! measure against the meshes themselves, not the pieces fitted to them:
//!
//! - `lush.solids {at, within}`: the solid things within so far of a spot, east and south of
//!   the start, with their names, where they are and how high they stand.
//! - `lush.footprint {points, thing?}`: for each spot, how far it is from the nearest part of
//!   any solid mesh taller than a step, between a step and a body's height over the ground
//!   (nothing inside), and from any but `thing`'s (`other`), and from `thing`'s whole drawn
//!   footprint up to a body's height; where the player is and how high their feet are over
//!   the ground; and the same of every frame since it was last asked, as `track`: east,
//!   south and that height.

use super::floor::Sink;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::remote::{BrpResult, builtin_methods::parse_some};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use terra_core::{Piece, Placement, Relief, Solid};
use terra_world::flora::Obstacles;
use terra_world::planet::Planet;
use terra_world::start::Start;

/// A walker's step (`terra_build::Walker`), the most it steps onto without jumping.
const STEP_M: f32 = 0.4;
/// A body's height: above it nothing stops a walker.
const BODY_M: f32 = 1.8;
/// Models are read up to this high over their origin, in their own units: past any body.
const CROP_M: f32 = 3.0;
/// A crown is read up to this high over its origin, in its own units, for what of it hangs
/// within a body's height of the ground: a tree is sunk 0.3 m.
const LOW_CROWN_M: f32 = 2.1;
/// How far out a skirt stops a walker, of the way to where its leaves reach (D248).
const SKIRT: f32 = 0.7;
/// How fine the checks' footprints are.
const PRINT_CELL_M: f32 = 0.02;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Shapes>()
        .init_resource::<Prints>()
        .init_resource::<Track>()
        .add_systems(Update, (footings, trunks));
    if app
        .world()
        .contains_resource::<bevy::remote::RemoteMethods>()
    {
        app.add_systems(PostUpdate, record_track);
    }
    terra_world::remote::method(app, "lush.solids", solids);
    terra_world::remote::method(app, "lush.footprint", footprint);
}

/// Where the player has stood, east and south, and how high their feet were over the ground,
/// every frame since `lush.footprint` last asked: a walk is checked frame by frame, however
/// often a script can ask.
#[derive(Resource, Default)]
struct Track {
    player: Option<Entity>,
    steps: Vec<[f32; 3]>,
}

fn record_track(
    mut track: ResMut<Track>,
    named: Query<(Entity, &Name)>,
    places: Query<&Transform>,
    start: Option<Res<Start>>,
    planet: Option<Res<Planet>>,
) {
    let (Some(start), Some(planet)) = (start, planet) else {
        return;
    };
    let feet = match track.player.and_then(|player| places.get(player).ok()) {
        Some(place) => place.translation,
        None => {
            track.player = named
                .iter()
                .find(|(_, name)| name.as_str() == "Player")
                .map(|(entity, _)| entity);
            return;
        }
    };
    // A script that stopped asking does not fill the memory.
    if track.steps.len() < 10_000 {
        let local = start.local(feet);
        let lift = feet.length() - planet.ground_at(feet).length();
        track.steps.push([local.x, local.z, lift]);
    }
}

/// Something on the forest floor that stops a walker or holds one up, its shape still to be
/// fitted to its mesh: a rock, boulder, stump, log or snag, standing on its own (not a
/// child), named for what it is. Its mesh must keep its data on the CPU (a scan's does; a
/// grown one's is made with `RenderAssetUsages::all()`). A rock is first sunk its share of
/// its height into the ground (`Sink`, D246).
#[derive(Component)]
pub(super) struct Footing;

/// A tree whose trunk is still to be fitted: its finest wood, its nearest crown (what a
/// walker sees of it), how far round its foot its trunk and roots reach, and how far its
/// branches reach within a body's height of the ground, in its model's units: if at all, it
/// has a skirt, fitted to the crown's cards there (D248).
#[derive(Component)]
pub(super) struct Trunk {
    pub wood: Handle<Mesh>,
    pub crown: Handle<Mesh>,
    pub reach_m: f32,
    pub skirt_m: f32,
}

/// A thing fitted: its model's shape, what of it is seen, and how high it stands over the
/// ground, for the checks.
#[derive(Component)]
struct Fitted {
    shape: Arc<SolidShape>,
    seen: Arc<SolidShape>,
    top_m: f32,
    /// Its lowest piece, if lower than a step, as a log's thin end may be: its middle, the
    /// level direction across it, and its top.
    low: Option<(Vec3, Vec3, f32)>,
}

/// A solid thing's lowest piece, if lower than a step: its middle, the level direction across
/// it (across the widest pair of its corners), and its top.
fn lowest(solid: &Solid) -> Option<(Vec3, Vec3, f32)> {
    let piece = solid
        .pieces
        .iter()
        .filter(|piece| piece.top < STEP_M)
        .min_by(|a, b| a.top.total_cmp(&b.top))?;
    let (middle, _) = piece.bounds();
    let mut along = Vec2::X;
    let mut widest = 0.0;
    for a in &piece.hull {
        for b in &piece.hull {
            if a.distance(*b) > widest {
                widest = a.distance(*b);
                along = (*b - *a).normalize_or(Vec2::X);
            }
        }
    }
    Some((
        solid.at + solid.world(middle),
        solid.world(along.perp()),
        piece.top,
    ))
}

/// A model as solid things are fitted to it: its relief, and its triangles up to `CROP_M`,
/// in its own frame.
pub(super) struct SolidShape {
    relief: Relief,
    triangles: Vec<[Vec3; 3]>,
    /// The lowest and highest of it.
    low: f32,
    high: f32,
}

impl SolidShape {
    /// From a model's vertices and triangles, those whose corners `keep` keeps, leaving out
    /// those that face down. A relief is the highest point over each cell, which a face
    /// turned down never is but under an overhang; and a closed model's floor, as a modelled
    /// trunk's under its roots is, filled the inside of its relief at the ground, so a cut
    /// trunk was a ring of pieces round a hollow a camera could stand in (D289). An open
    /// tube's top is filled as a hole is (`Relief::with_cell`).
    pub(super) fn of(positions: &[[f32; 3]], indices: &[u32], keep: impl Fn(Vec3) -> bool) -> Self {
        let triangles: Vec<[Vec3; 3]> = indices
            .chunks_exact(3)
            .map(|t| [0, 1, 2].map(|k| Vec3::from(positions[t[k] as usize])))
            .filter(|t| t.iter().all(|p| keep(*p)))
            .filter(|[a, b, c]| {
                let face = (*b - *a).cross(*c - *a);
                face.y > -0.5 * face.length()
            })
            .collect();
        let points: Vec<Vec3> = triangles.iter().flatten().copied().collect();
        let corners: Vec<u32> = (0..points.len() as u32).collect();
        let (low, high) = points.iter().fold((f32::MAX, f32::MIN), |(low, high), p| {
            (low.min(p.y), high.max(p.y))
        });
        Self {
            relief: Relief::of(&points, &corners),
            triangles,
            low,
            high,
        }
    }

    /// A grown tree's finest wood: its trunk, `trunk_m` thick at its foot before its roots
    /// flare, and the roots, up to past a body's height.
    pub(super) fn trunk(positions: &[[f32; 3]], indices: &[u32], trunk_m: f32) -> Self {
        let reach = trunk_reach(trunk_m);
        Self::of(positions, indices, |p| {
            p.y < CROP_M && Vec2::new(p.x, p.z).length() < reach
        })
    }

    /// A grown tree's nearest crown, what a walker sees of it near the ground.
    pub(super) fn crown(positions: &[[f32; 3]], indices: &[u32]) -> Self {
        Self::of(positions, indices, |p| p.y < LOW_CROWN_M)
    }

    fn of_mesh(mesh: &Mesh, keep: impl Fn(Vec3) -> bool) -> Option<Self> {
        let positions = mesh
            .try_attribute(Mesh::ATTRIBUTE_POSITION)
            .ok()?
            .as_float3()?;
        let indices: Vec<u32> = match mesh.try_indices_option().ok()? {
            Some(indices) => indices.iter().map(|i| i as u32).collect(),
            None => (0..positions.len() as u32).collect(),
        };
        Some(Self::of(positions, &indices, keep))
    }
}

/// Each kind of model's shape, worked out once, by its mesh.
#[derive(Resource, Default)]
pub(super) struct Shapes(pub HashMap<AssetId<Mesh>, Arc<SolidShape>>);

impl Shapes {
    /// Remembers the shape of a mesh that keeps no data on the CPU, such as a grown tree's.
    pub(super) fn remember(&mut self, mesh: &Handle<Mesh>, shape: SolidShape) {
        self.0.insert(mesh.id(), Arc::new(shape));
    }

    /// A mesh's shape, from what is known or from the mesh once it has loaded.
    fn get(
        &mut self,
        mesh: &Handle<Mesh>,
        meshes: &Assets<Mesh>,
        keep: impl Fn(Vec3) -> bool,
    ) -> Option<Arc<SolidShape>> {
        if let Some(shape) = self.0.get(&mesh.id()) {
            return Some(shape.clone());
        }
        let shape = Arc::new(SolidShape::of_mesh(meshes.get(mesh)?, keep)?);
        self.0.insert(mesh.id(), shape.clone());
        Some(shape)
    }
}

/// How far round its foot a trunk `trunk_m` thick and its flared roots reach: its foot
/// flares to about twice its width (`species::flare`) and its roots stand out half as far
/// again (`Shape::at` in lush.rs).
pub(super) fn trunk_reach(trunk_m: f32) -> f32 {
    trunk_m * 3.4 + 0.25
}

/// Where a thing stands, as its relief is fitted: on the ground under its origin, as the
/// ground is drawn.
/// How high the ground under a placed relief is over the level of the ground under its
/// origin, at a point in its plan: sampled every half metre over it, as the ground is drawn,
/// and read between the samples.
fn ground_under(planet: &Planet, place: &Placement, relief: &Relief) -> impl Fn(Vec2) -> f32 {
    const EVERY_M: f32 = 0.5;
    let (low, high) = relief.cells.iter().fold(
        (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
        |(low, high), c| {
            let p = Vec2::new(c.x, c.z) * place.scale;
            (low.min(p), high.max(p))
        },
    );
    let low = low.min(Vec2::ZERO) - Vec2::splat(EVERY_M);
    let high = high.max(Vec2::ZERO) + Vec2::splat(EVERY_M);
    let count = ((high - low) / EVERY_M).ceil().as_uvec2() + UVec2::ONE;
    let (across, along) = (count.x as usize, count.y as usize);
    let mut heights = Vec::with_capacity(across * along);
    for j in 0..along {
        for i in 0..across {
            let p = low + Vec2::new(i as f32, j as f32) * EVERY_M;
            let over = place.ground + place.x * p.x + place.z * p.y;
            heights.push((planet.ground_at(over) - place.ground).dot(place.up));
        }
    }
    move |p: Vec2| {
        let f = ((p - low) / EVERY_M).clamp(Vec2::ZERO, (count - UVec2::ONE).as_vec2() - 1e-3);
        let (i, j) = (f.x as usize, f.y as usize);
        let (i1, j1) = ((i + 1).min(across - 1), (j + 1).min(along - 1));
        let t = f - Vec2::new(i as f32, j as f32);
        let h = |i: usize, j: usize| heights[j * across + i];
        let near = h(i, j) + (h(i1, j) - h(i, j)) * t.x;
        let far = h(i, j1) + (h(i1, j1) - h(i, j1)) * t.x;
        near + (far - near) * t.y
    }
}

fn placement(planet: &Planet, place: &Transform) -> Placement {
    let ground = planet.ground_at(place.translation);
    let up = place.rotation * Vec3::Y;
    Placement {
        ground,
        x: place.rotation * Vec3::X,
        z: place.rotation * Vec3::Z,
        up,
        scale: place.scale.x,
        lift: (place.translation - ground).dot(up),
    }
}

/// Gives each rock, boulder, stump, log and snag its solid shape, once its mesh has loaded.
fn footings(
    mut commands: Commands,
    mut pending: Query<
        (
            Entity,
            &Mesh3d,
            &mut Transform,
            Option<&Sink>,
            Option<&Name>,
        ),
        With<Footing>,
    >,
    meshes: Res<Assets<Mesh>>,
    mut shapes: ResMut<Shapes>,
    planet: Option<Res<Planet>>,
    mut obstacles: ResMut<Obstacles>,
) {
    let Some(planet) = planet else { return };
    for (entity, mesh, mut transform, sink, name) in &mut pending {
        if meshes.get(&mesh.0).is_none() {
            continue;
        }
        let Some(shape) = shapes.get(&mesh.0, &meshes, |_| true) else {
            warn!(
                "lush: {} has no mesh data to fit a footing to; it stops nobody",
                name.map_or("something", |name| name.as_str())
            );
            commands.entity(entity).remove::<Footing>();
            continue;
        };
        if let Some(Sink(share)) = sink {
            let height = (shape.high - shape.low) * transform.scale.y;
            let down = transform.rotation * Vec3::NEG_Y * height * share;
            transform.translation += down;
            commands.entity(entity).remove::<Sink>();
        }
        let place = placement(&planet, &transform);
        // How tall it stands over the ground under it, at its tallest.
        let mut top_m = 0.0;
        let mut low = None;
        let ground = ground_under(&planet, &place, &shape.relief);
        if let Some(solid) = Solid::fit(&shape.relief, place, ground, Vec::new()) {
            low = lowest(&solid);
            top_m = solid.top().unwrap_or(0.0);
            obstacles.add_solid(solid.at, solid);
        }
        commands.entity(entity).remove::<Footing>().insert(Fitted {
            seen: shape.clone(),
            shape,
            top_m,
            low,
        });
    }
}

/// Gives each tree its trunk's solid shape, at its real width over a step's height, root
/// flare and all, and its skirt where it has one: 0.7 of how far its branches reach near the
/// ground (D248). Filed at its foot, where it is felled from.
fn trunks(
    mut commands: Commands,
    pending: Query<(Entity, &Trunk, &Transform)>,
    meshes: Res<Assets<Mesh>>,
    mut shapes: ResMut<Shapes>,
    planet: Option<Res<Planet>>,
    mut obstacles: ResMut<Obstacles>,
) {
    let Some(planet) = planet else { return };
    for (entity, trunk, transform) in &pending {
        let reach = trunk.reach_m;
        let Some(wood) = shapes.get(&trunk.wood, &meshes, |p| {
            p.y < CROP_M && Vec2::new(p.x, p.z).length() < reach
        }) else {
            continue;
        };
        let seen = shapes
            .get(&trunk.crown, &meshes, |p| p.y < LOW_CROWN_M)
            .unwrap_or_else(|| wood.clone());
        let place = placement(&planet, transform);
        // Its skirt: SKIRT of the way out to where its leaves and twigs reach, at each turn
        // round it, where they hang within a body's height of the ground (D248).
        let skirt: Vec<Piece> = if trunk.skirt_m > 0.0 && !Arc::ptr_eq(&seen, &wood) {
            Solid::fit(
                &seen.relief,
                place,
                ground_under(&planet, &place, &seen.relief),
                Vec::new(),
            )
            .map(|low| {
                low.pieces
                    .into_iter()
                    .map(|piece| Piece {
                        hull: piece.hull.iter().map(|p| *p * SKIRT).collect(),
                        radius: piece.radius * SKIRT,
                        top: f32::INFINITY,
                        crest: f32::INFINITY,
                    })
                    .collect()
            })
            .unwrap_or_default()
        } else {
            Vec::new()
        };
        if let Some(mut solid) = trunk_solid(&planet, place, &wood.relief, skirt) {
            // Nobody stands on a trunk.
            for piece in &mut solid.pieces {
                piece.top = f32::INFINITY;
                piece.crest = f32::INFINITY;
            }
            obstacles.add_solid(transform.translation, solid);
        }
        commands.entity(entity).remove::<Trunk>().insert(Fitted {
            shape: wood,
            seen,
            top_m: f32::INFINITY,
            low: None,
        });
    }
}

/// A tree's trunk fitted where it stands, with its skirt. Its bole, its wood taller than a
/// body, is its own: what a camera's lens keeps out of (`Solid::clearance`), as wide as the
/// trunk and any low bough are at a head's height. Its flared foot and roots, lower, stop a
/// walker as its skirt does, but taken as its own would stand round the lens as wide as they
/// spread, metres from the bark.
fn trunk_solid(
    planet: &Planet,
    place: Placement,
    wood: &Relief,
    skirt: Vec<Piece>,
) -> Option<Solid> {
    let ground = ground_under(planet, &place, wood);
    let s = place.scale;
    let (bole, foot): (Vec<Vec3>, Vec<Vec3>) = wood
        .cells
        .iter()
        .partition(|c| c.y * s + place.lift - ground(Vec2::new(c.x, c.z) * s) > BODY_M);
    let (bole, foot) = if bole.is_empty() {
        (foot, Vec::new())
    } else {
        (bole, foot)
    };
    let cells = |cells| Relief {
        cells,
        cell: wood.cell,
    };
    let foot = Solid::fit(&cells(foot), place, &ground, Vec::new())
        .map(|foot| foot.pieces)
        .unwrap_or_default();
    Solid::fit(&cells(bole), place, &ground, [foot, skirt].concat())
}

#[derive(Deserialize)]
struct Around {
    at: [f32; 2],
    within: f32,
}

/// The solid things within so far of a spot: what each is called, where it stands, east and
/// south, and how high over the ground it reaches (99 for a tree); and those not yet fitted.
fn solids(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Around { at, within } = parse_some(params)?;
    let start = *world.resource::<Start>();
    let mut query = world.query::<(
        Entity,
        &Transform,
        Option<&Name>,
        Option<&Fitted>,
        Has<Footing>,
        Has<Trunk>,
    )>();
    let mut found = Vec::new();
    for (entity, transform, name, fitted, footing, trunk) in query.iter(world) {
        if fitted.is_none() && !footing && !trunk {
            continue;
        }
        let local = start.local(transform.translation);
        if Vec2::new(local.x - at[0], local.z - at[1]).length() > within {
            continue;
        }
        found.push(json!({
            "entity": entity.to_bits(),
            "name": name.map_or("", |name| name.as_str()),
            "at": [local.x, local.z],
            "top": fitted.map(|fitted| fitted.top_m.min(99.0)),
            "low": fitted.and_then(|fitted| fitted.low).map(|(middle, across, top)| {
                let (here, there) = (start.local(middle), start.local(middle + across));
                json!({
                    "at": [here.x, here.z],
                    "across": [there.x - here.x, there.z - here.z],
                    "top": top,
                })
            }),
            "fitted": fitted.is_some(),
        }));
    }
    Ok(Value::Array(found))
}

/// A fitted thing's footprint as drawn, for the checks: its cells over the ground in its own
/// plan, each with the height of the highest of the model there over the ground under it.
struct Print {
    at: Vec3,
    x: Vec3,
    z: Vec3,
    solid: Relief,
    seen: Relief,
    name: String,
}

/// Footprints worked out for the checks, by thing.
#[derive(Resource, Default)]
struct Prints(HashMap<Entity, Arc<Print>>);

#[derive(Deserialize)]
struct Spots {
    points: Vec<[f32; 2]>,
    thing: Option<u64>,
}

/// How far each spot is from the nearest solid mesh taller than a step, of anything and of
/// anything but one thing, and from that thing's drawn footprint; and where the player is.
fn footprint(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Spots { points, thing } = parse_some(params)?;
    let start = *world.resource::<Start>();
    let planet = world.resource::<Planet>().clone();
    let mut players = world.query::<(&Name, &Transform)>();
    let player = players
        .iter(world)
        .find(|(name, _)| name.as_str() == "Player")
        .map(|(_, transform)| {
            let feet = transform.translation;
            let local = start.local(feet);
            json!({
                "at": [local.x, local.z],
                "lift": feet.length() - planet.ground_at(feet).length(),
            })
        });
    let track = std::mem::take(&mut world.resource_mut::<Track>().steps);
    let spots: Vec<Vec3> = points
        .iter()
        .map(|[east, south]| planet.ground_at(start.at(*east, *south)))
        .collect();
    // The fitted things near any spot, and their footprints.
    let mut near: Vec<(Entity, Transform, Arc<SolidShape>, Arc<SolidShape>, String)> = Vec::new();
    let mut fitted = world.query::<(Entity, &Transform, &Fitted, Option<&Name>)>();
    for (entity, transform, fitted, name) in fitted.iter(world) {
        let reach =
            (fitted.shape.high.max(1.0) + fitted.shape.relief.cell) * transform.scale.x * 6.0;
        if Some(entity.to_bits()) == thing
            || spots
                .iter()
                .any(|spot| spot.distance(transform.translation) < reach.max(6.0))
        {
            near.push((
                entity,
                *transform,
                fitted.shape.clone(),
                fitted.seen.clone(),
                name.map_or(String::new(), |name| name.to_string()),
            ));
        }
    }
    let mut prints = Vec::new();
    for (entity, transform, shape, seen, name) in near {
        let known = world.resource::<Prints>().0.get(&entity).cloned();
        let print = known.unwrap_or_else(|| {
            let print = Arc::new(print_of(&planet, &transform, &shape, &seen, name));
            world
                .resource_mut::<Prints>()
                .0
                .insert(entity, print.clone());
            print
        });
        prints.push((entity, print));
    }
    let answers: Vec<Value> = spots
        .iter()
        .map(|spot| {
            let gaps: Vec<(f32, &String, bool)> = prints
                .iter()
                .filter_map(|(entity, print)| {
                    let d = gap(print, &print.solid, *spot, |h| h > STEP_M + 0.05)?;
                    Some((d, &print.name, Some(entity.to_bits()) == thing))
                })
                .collect();
            let nearest = |of: &dyn Fn(bool) -> bool| {
                gaps.iter()
                    .filter(|(_, _, own)| of(*own))
                    .min_by(|a, b| a.0.total_cmp(&b.0))
            };
            let solid = nearest(&|_| true);
            let other = nearest(&|own| !own);
            let seen = prints
                .iter()
                .find(|(entity, _)| Some(entity.to_bits()) == thing)
                .and_then(|(_, print)| gap(print, &print.seen, *spot, |h| h > 0.03));
            json!({
                "solid": solid.map(|(d, _, _)| *d),
                "solid_of": solid.map(|(_, name, _)| name),
                "other": other.map(|(d, _, _)| *d),
                "other_of": other.map(|(_, name, _)| name),
                "seen": seen,
            })
        })
        .collect();
    Ok(json!({ "player": player, "track": track, "points": answers }))
}

/// How far a spot is, in plan, from the nearest of a footprint's cells whose height `counts`;
/// none inside it. None when it has no such cell.
fn gap(print: &Print, relief: &Relief, spot: Vec3, counts: impl Fn(f32) -> bool) -> Option<f32> {
    let d = spot - print.at;
    let plan = Vec2::new(d.dot(print.x), d.dot(print.z));
    relief
        .cells
        .iter()
        .filter(|c| counts(c.y))
        .map(|c| Vec2::new(c.x, c.z).distance(plan))
        .reduce(f32::min)
        .map(|d| (d - relief.cell * 0.75).max(0.0))
}

/// A thing's footprint as drawn: its triangles in the world, each corner's height measured
/// over the ground under that corner, laid in cells over the plan at the ground under it.
fn print_of(
    planet: &Planet,
    transform: &Transform,
    shape: &SolidShape,
    seen: &SolidShape,
    name: String,
) -> Print {
    let at = planet.ground_at(transform.translation);
    let up = at.normalize();
    let x = up.any_orthonormal_vector();
    let z = up.cross(x);
    let lay = |shapes: &[&SolidShape]| {
        let mut points = Vec::with_capacity(shape.triangles.len() * 3);
        for triangle in shapes.iter().flat_map(|shape| &shape.triangles) {
            let corners = triangle.map(|corner| {
                let world = transform.transform_point(corner);
                let height = world.length() - planet.ground_at(world).length();
                let d = world - at;
                Vec3::new(d.dot(x), height, d.dot(z))
            });
            // What is all over a body's head is not in its way.
            if corners.iter().all(|c| c.y > BODY_M) {
                continue;
            }
            points.extend(corners.map(|c| c.with_y(c.y.min(BODY_M))));
        }
        let corners: Vec<u32> = (0..points.len() as u32).collect();
        // Only what shows above the ground, and what is under a body's height.
        let mut relief = Relief::with_cell(&points, &corners, PRINT_CELL_M);
        relief.cells.retain(|c| c.y > 0.0);
        relief
    };
    Print {
        at,
        x,
        z,
        solid: lay(&[shape]),
        // All of it a walker sees: its wood or stone, and its leaves.
        seen: if std::ptr::eq(shape, seen) {
            lay(&[shape])
        } else {
            lay(&[shape, seen])
        },
        name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{PlanetSpec, Seed};

    #[test]
    fn a_trunks_own_solid_is_its_bole_not_its_roots() {
        let planet = Planet::smooth(PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        });
        let ground = planet.ground_at(Vec3::Y * 2100.0);
        let place = Placement {
            ground,
            x: Vec3::X,
            z: Vec3::Z,
            up: Vec3::Y,
            scale: 1.0,
            lift: 0.0,
        };
        // A trunk 0.6 m thick standing 3 m up, and roots 0.35 m high out to 1.4 m along four
        // lines, and a little flare round its foot 0.8 m high.
        let mut cells = Vec::new();
        for i in -20..=20 {
            for j in -20..=20 {
                let plan = Vec2::new(i as f32, j as f32) * 0.1;
                let root = plan.length() < 1.4 && (plan.x.abs() < 0.1 || plan.y.abs() < 0.1);
                let height = if plan.length() < 0.3 {
                    3.0
                } else if plan.length() < 0.5 {
                    0.8
                } else if root {
                    0.35
                } else {
                    continue;
                };
                cells.push(Vec3::new(plan.x, height, plan.y));
            }
        }
        let wood = Relief { cells, cell: 0.1 };
        let solid = trunk_solid(&planet, place, &wood, Vec::new()).unwrap();
        assert!(solid.breadth() < 0.9, "{}", solid.breadth());
        // A lens 2 m up, a metre out over a root, is clear of it; one by the bole is not.
        let lens = |out: f32| ground + Vec3::new(out, 2.0, 0.0);
        assert!(
            solid.clearance(lens(1.0)) > 0.4,
            "{}",
            solid.clearance(lens(1.0))
        );
        assert!(solid.clearance(lens(0.45)) < 0.2);
        // The flare and the roots are still there to stop a walker.
        for out in [0.45, 0.8, 1.3] {
            let (plan, _) = solid.plan(ground + Vec3::X * out);
            let covered = solid
                .pieces
                .iter()
                .any(|piece| piece.distance(plan).0 < 0.0);
            assert!(covered, "nothing {out} m out");
        }
    }

    /// A closed model's floor, as a modelled trunk has under its roots, does not make the
    /// inside of its relief ground level: cut off above a body's height, a trunk two metres
    /// thick is solid through its middle, not a ring round a hollow (D289).
    #[test]
    fn a_closed_trunks_middle_is_solid() {
        // A closed cylinder 1 m round, 6 m tall, wound outward, with a floor and a lid.
        let (sides, rings) = (24, 13);
        let mut positions: Vec<[f32; 3]> = Vec::new();
        for ring in 0..rings {
            for side in 0..sides {
                let a = side as f32 / sides as f32 * std::f32::consts::TAU;
                positions.push([libm::cosf(a), ring as f32 * 0.5, libm::sinf(a)]);
            }
        }
        let (floor, lid) = (positions.len() as u32, positions.len() as u32 + 1);
        positions.push([0.0, 0.0, 0.0]);
        positions.push([0.0, 6.0, 0.0]);
        let mut indices = Vec::new();
        let at = |ring: u32, side: u32| ring * sides + side % sides;
        for ring in 0..rings - 1 {
            for side in 0..sides {
                let (a, b) = (at(ring, side), at(ring, side + 1));
                let (c, d) = (at(ring + 1, side), at(ring + 1, side + 1));
                indices.extend([a, c, b, b, c, d]);
            }
        }
        for side in 0..sides {
            indices.extend([floor, at(0, side), at(0, side + 1)]);
            indices.extend([lid, at(rings - 1, side + 1), at(rings - 1, side)]);
        }
        // Outward, as the game culls: each wall triangle's normal points away from the axis.
        let p = |i: u32| Vec3::from(positions[i as usize]);
        let face = (p(indices[1]) - p(indices[0])).cross(p(indices[2]) - p(indices[0]));
        assert!(
            face.dot(p(indices[0]).with_y(0.0)) > 0.0,
            "the test's walls face in"
        );
        let shape = SolidShape::of(&positions, &indices, |p| p.y < CROP_M);
        let middle = shape
            .relief
            .cells
            .iter()
            .filter(|c| Vec2::new(c.x, c.z).length() < 0.4)
            .map(|c| c.y)
            .fold(f32::MAX, f32::min);
        assert!(middle > 2.0, "its middle stands only {middle} m");
    }
}
