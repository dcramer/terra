//! What the builder is aimed at, and what it would cost there.

use super::*;
use crate::build::Site;
use crate::planet::MainCamera;
use crate::planet::Planet;
use crate::play::catalogue::{self, Kind};
use crate::play::claim::Claim;
use crate::play::dial::Dial;
use crate::play::flora::Obstacles;
use crate::play::inventory::{Belongings, Cost};
use crate::play::props::{self, Blocked, Models, Prop, Standing};
use crate::play::{Player, View};
use crate::shell::Options;
use terra_build::{CELL_M, DOOR_WIDTH_M, Face, Panel, STOREY_M, Side, cells};

pub(super) fn aim(
    planet: Res<Planet>,
    site: Res<Site>,
    options: Res<Options>,
    dial: Res<Dial>,
    view: Res<View>,
    models: Res<Models>,
    obstacles: Res<Obstacles>,
    camera: Single<&GlobalTransform, With<MainCamera>>,
    player: Single<&Transform, With<Player>>,
    props: Query<(Entity, &Prop, &Transform, &Visibility)>,
    mut builder: ResMut<Builder>,
) {
    // What is being worked on stays the target until the work is done or dropped.
    if builder.work.is_some() {
        return;
    }
    builder.viewed = None;
    // A fixed camera is for taking pictures, and an open dial is for choosing.
    if options.vantage.is_some() || dial.open {
        builder.target = Target::Nothing;
        return;
    }
    let to_base = site.frame.compute_affine().inverse();
    let origin = camera.translation();
    let direction = *camera.forward();
    let local_origin = to_base.transform_point3(origin);
    let local_direction = to_base.transform_vector3(direction).normalize();
    let within = |point: Vec3, reach: f32| point.distance(player.translation) <= reach;
    // The first of the base's walls, floors and roofs the view meets, interior walls
    // included; the ground; and the first item.
    let hull = site.base.raycast(local_origin, local_direction, 200.0);
    let ground = planet.raycast(origin, direction, 200.0);
    let met = |distance: f32| local_origin + local_direction * distance;
    let outside_wall = hull
        .filter(|hit| !hit.partition && hit.face.side.is_wall())
        .filter(|hit| within(origin + direction * hit.distance, STRUCTURE_REACH_M));
    let limit = hull.map_or(f32::MAX, |hit| hit.distance + 0.05);
    let item = props
        .iter()
        .filter(|(_, _, _, visibility)| **visibility != Visibility::Hidden)
        .filter_map(|(entity, prop, transform, _)| {
            props::ray_box(transform, prop.half, origin, direction)
                .map(|(distance, side)| (entity, prop, distance, side))
        })
        .filter(|&(_, _, distance, _)| {
            distance < limit && within(origin + direction * distance, ITEM_REACH_M)
        })
        .min_by(|a, b| a.2.total_cmp(&b.2));
    builder.viewed = item.map(|(entity, ..)| entity);
    if builder.stowed {
        builder.target = Target::Nothing;
        return;
    }

    // Where an item would stand. On top of an item the view is on, or flush against its
    // side; else the floor, or flush against a wall; else the ground outside. If none of
    // that is within reach, a few steps in front of the player, so there is always
    // something to see.
    let spot = || {
        let half = builder
            .item()
            .and_then(|item| models.of(item))
            .map_or(Vec3::splat(0.3), |model| model.half);
        let boxed = |at: Vec3, lift: f32, indoors: bool| Standing {
            at,
            lift,
            turn: builder.turn,
            half,
            indoors,
        };
        let on_grid = |at: Vec3| {
            if builder.snapping {
                ((at / GRID_M).round() * GRID_M).with_y(at.y)
            } else {
                at
            }
        };
        let on_item =
            item.filter(|(_, prop, ..)| prop.is_solid())
                .and_then(|(_, prop, distance, side)| {
                    let touched = met(distance);
                    let out = to_base.transform_vector3(side).normalize();
                    if out.y > 0.7 {
                        // Its top. On the grid that means the middle of it.
                        let at = if builder.snapping {
                            prop.at
                        } else {
                            touched.with_y(prop.at.y)
                        };
                        Some((at, prop.lift + prop.half.y * 2.0, prop.indoors))
                    } else if out.y.abs() < 0.3 {
                        // A side: stand flush beside it, lined up with it if nearly so.
                        let out = out.with_y(0.0).normalize();
                        let along_side = out.cross(Vec3::Y);
                        let mut along = (touched - prop.at).dot(along_side);
                        if builder.snapping && along.abs() < 0.35 {
                            along = 0.0;
                        }
                        let its_reach = Standing::of(prop).reach_toward(out);
                        let ours = boxed(prop.at, prop.lift, prop.indoors).reach_toward(out);
                        let at = prop.at + along_side * along + out * (its_reach + ours + 0.02);
                        Some((at, prop.lift, prop.indoors))
                    } else {
                        None
                    }
                });
        let on_ground = ground
            .filter(|ground| hull.is_none_or(|hit| ground.distance < hit.distance))
            .map(|ground| (on_grid(to_base.transform_point3(ground.point)), 0.0, false));
        let in_base = hull.and_then(|hit| {
            let floor = hit.face.tile[1] as f32 * STOREY_M;
            let touched = met(hit.distance);
            match hit.face.side {
                Side::Down => Some((on_grid(touched), 0.0, true)),
                Side::Up => None,
                side => {
                    // Flush against the wall, on whichever side of it the view is, and
                    // on the grid along it.
                    let out = side.offset().as_vec3();
                    let toward_viewer = -out * local_direction.dot(out).signum();
                    let tile = ((touched + toward_viewer * 0.2) / CELL_M)
                        .floor()
                        .as_ivec3()
                        .with_y(hit.face.tile[1]);
                    let indoors = site.base.contains(tile);
                    let along = on_grid(touched);
                    let gap = if hit.partition { 0.08 } else { 0.02 };
                    let against = boxed(touched, 0.0, indoors).reach_toward(out) + gap;
                    let at = along + out * (touched - along).dot(out) + toward_viewer * against;
                    Some((at.with_y(if indoors { floor } else { 0.0 }), 0.0, indoors))
                }
            }
        });
        let ahead = || {
            let feet = to_base.transform_point3(player.translation);
            let forward = to_base
                .transform_vector3(view.forward)
                .with_y(0.0)
                .normalize_or_zero();
            let at = on_grid(feet + forward * ITEM_NEAR_M);
            let tile = (at / CELL_M).floor().as_ivec3().with_y(0);
            let indoors = site.base.contains(tile) && feet.y < STOREY_M * 0.5;
            (at.with_y(0.0), 0.0, indoors)
        };
        let (at, lift, indoors) = on_item
            .or(on_ground)
            .or(in_base)
            .filter(|(at, ..)| within(site.frame.transform_point(*at), ITEM_REACH_M))
            .unwrap_or_else(ahead);
        let world = site.frame.transform_point(at);
        let wet = !indoors && planet.under_sea(world);
        let blocked = props::blocked(
            &site,
            &boxed(at, lift, indoors),
            props
                .iter()
                .filter(|(_, _, _, visibility)| **visibility != Visibility::Hidden)
                .map(|(entity, prop, _, _)| (entity, prop)),
            &obstacles.0,
        )
        .or(wet.then_some(Blocked::Water));
        Target::Spot {
            at,
            lift,
            indoors,
            blocked,
        }
    };

    let edge = |face: Face| Target::Edge {
        face,
        fits: site.base.can_partition(face),
        open: site
            .base
            .partition(face)
            .is_some_and(|panel| panel != Panel::Solid),
    };
    // So many cells of one wall, centred on one of them. Empty if the wall does not run
    // that far, or changes from outside wall to interior on the way.
    let stretch = |face: Face, count: i32| -> Vec<Face> {
        let along = face.side.along();
        let first = face.tile() - along * ((count - 1) / 2);
        let covered: Vec<Face> = (0..count)
            .map(|step| Face::new(first + along * step, face.side))
            .collect();
        let outside = site.base.is_hull(face);
        if covered
            .iter()
            .all(|&cell| site.base.is_hull(cell) == outside && site.base.filling(cell).is_some())
        {
            covered
        } else {
            Vec::new()
        }
    };
    // Any upright wall the view is on, outside or interior, within reach.
    let any_wall = hull
        .filter(|hit| hit.face.side.is_wall())
        .filter(|hit| within(origin + direction * hit.distance, STRUCTURE_REACH_M));
    let opening_on = |width_m: f32, run: &mut Vec<Face>| {
        any_wall.map_or(Target::Nothing, |hit| {
            *run = stretch(hit.face, cells(width_m));
            if hit.partition {
                edge(hit.face)
            } else {
                Target::Wall(hit.face)
            }
        })
    };
    let mut run = Vec::new();
    builder.target = match builder.picked().kind {
        Kind::Reshape => outside_wall.map_or(Target::Nothing, |hit| Target::Wall(hit.face)),
        // A window or a door goes in an outside wall or an interior one alike.
        Kind::Window(shapes) => opening_on(shapes[builder.variant()].1, &mut run),
        Kind::Door => opening_on(DOOR_WIDTH_M, &mut run),
        Kind::Wall(lengths) => {
            // On the grid line nearest the floor the view is on; or, looking at no
            // floor, a couple of steps ahead. The wheel turns it.
            let on_floor = hull
                .filter(|hit| !hit.partition && hit.face.side == Side::Down)
                .filter(|hit| within(origin + direction * hit.distance, STRUCTURE_REACH_M))
                .map(|hit| met(hit.distance).with_y(hit.face.tile[1] as f32 * STOREY_M + 0.01))
                .unwrap_or_else(|| {
                    let feet = to_base.transform_point3(player.translation);
                    let forward = to_base
                        .transform_vector3(view.forward)
                        .with_y(0.0)
                        .normalize_or_zero();
                    let storey = (feet.y / STOREY_M).round();
                    (feet + forward * 2.0).with_y(storey * STOREY_M + 0.01)
                });
            let cell = (on_floor / CELL_M).floor().as_ivec3();
            let across = on_floor / CELL_M - cell.as_vec3();
            // A wall stands on the east or south side of a cell.
            let face = if builder.wall_turned {
                Face::new(cell - IVec3::X * i32::from(across.x < 0.5), Side::East)
            } else {
                Face::new(cell - IVec3::Z * i32::from(across.z < 0.5), Side::South)
            };
            let count = cells(lengths[builder.variant()].1);
            let along = face.side.along();
            let first = face.tile() - along * ((count - 1) / 2);
            run = (0..count)
                .map(|step| Face::new(first + along * step, face.side))
                .collect();
            Target::Edge {
                face,
                fits: run.iter().all(|&cell| site.base.can_partition(cell)),
                open: false,
            }
        }
        Kind::Room(_) => {
            // On open ground, or across a room already there, which it then extends.
            let on_base = hull
                .filter(|hit| ground.is_none_or(|ground| hit.distance < ground.distance))
                .map(|hit| site.frame.transform_point(met(hit.distance).with_y(0.0)));
            on_base
                .or(ground.map(|ground| ground.point))
                .filter(|point| within(*point, STRUCTURE_REACH_M))
                .zip(builder.room())
                .map_or(Target::Nothing, |(point, size)| {
                    // Centred on the place aimed at.
                    let local = to_base.transform_point3(point);
                    let corner = IVec3::new(
                        (local.x / CELL_M - size.x as f32 * 0.5).round() as i32,
                        0,
                        (local.z / CELL_M - size.y as f32 * 0.5).round() as i32,
                    );
                    // The room's floor is the base's floor. Set into the ground, it
                    // cuts the ground away where it stands, but not deeper than its own
                    // height. Checked every metre or so.
                    let every = cells(1.0).max(1) as usize;
                    let fits = (0..=size.x).step_by(every).chain([size.x]).all(|i| {
                        (0..=size.y).step_by(every).chain([size.y]).all(|j| {
                            let on_floor = (corner + IVec3::new(i, 0, j)).as_vec3() * CELL_M;
                            let at_roof = on_floor + Vec3::Y * STOREY_M;
                            planet.distance(site.frame.transform_point(at_roof)) > 0.02
                        })
                    });
                    // Trees and rocks are cleared by the player first, not by the room.
                    let low = corner.as_vec3() * CELL_M;
                    let high = (corner + IVec3::new(size.x, 0, size.y)).as_vec3() * CELL_M;
                    let clear = !obstacles.0.iter().any(|&(at, radius)| {
                        let local = to_base.transform_point3(at);
                        let nearest = local.with_y(0.0).clamp(low, high);
                        nearest.distance(local.with_y(0.0)) < radius
                            && local.y > -STOREY_M
                            && local.y < STOREY_M * 2.0
                    });
                    let adds = (0..size.x).any(|i| {
                        (0..size.y).any(|j| !site.base.contains(corner + IVec3::new(i, 0, j)))
                    });
                    Target::Ground {
                        corner,
                        tiles: size,
                        fits,
                        adds,
                        clear,
                    }
                })
        }
        Kind::Items(_) => spot(),
        Kind::Move if builder.is_carrying() => spot(),
        Kind::Move => item.map_or(Target::Nothing, |(entity, ..)| Target::Prop(entity)),
        Kind::Remove => match (item, hull, outside_wall) {
            (Some((entity, ..)), _, _) => Target::Prop(entity),
            (None, Some(hit), _) if hit.partition => {
                let target = edge(hit.face);
                run = if matches!(target, Target::Edge { open: true, .. }) {
                    // The window or doorway in it.
                    site.base.run(hit.face)
                } else {
                    // The whole straight stretch of plain wall.
                    let along = hit.face.side.along();
                    let plain = |cell: IVec3| {
                        site.base.partition(Face::new(cell, hit.face.side)) == Some(Panel::Solid)
                    };
                    let mut first = hit.face.tile();
                    while plain(first - along) {
                        first -= along;
                    }
                    let mut found = Vec::new();
                    while plain(first) {
                        found.push(Face::new(first, hit.face.side));
                        first += along;
                    }
                    found
                };
                target
            }
            (None, _, Some(hit)) if site.base.panel(hit.face) != Panel::Solid => {
                run = site.base.run(hit.face);
                Target::Wall(hit.face)
            }
            _ => Target::Nothing,
        },
    };
    builder.run = run;
}

/// Works out what the thing aimed at would cost to build, and whether it can be paid.
pub(super) fn afford(
    site: Res<Site>,
    claim: Res<Claim>,
    props: Query<&Prop>,
    belongings: Belongings,
    mut builder: ResMut<Builder>,
) {
    if builder.work.is_some() {
        return;
    }
    // Nothing is built or taken apart outside your claim (D94), except the claim stone
    // that makes one; and there is one claim each.
    let placing_claim = matches!(builder.picked().kind, Kind::Items(_))
        && builder.item().is_some_and(|item| item.claims)
        && !builder.is_carrying();
    let on_planet =
        target_point(&builder.target, &props).map(|point| site.frame.transform_point(point));
    builder.unclaimed = match on_planet {
        Some(_) if placing_claim && claim.stone.is_some() => {
            Some("You already have a claim; take its stone away to move it")
        }
        Some(_) if placing_claim => None,
        Some(point) if !claim.covers(point) => Some(if claim.stone.is_none() {
            "Place a claim stone to build here: it is under Claim in the build menu"
        } else {
            "That is outside your claim"
        }),
        _ => None,
    };
    // Cutting an opening is paid for where the wall was solid; changing the shape of
    // one already there is free.
    let cuts = builder
        .run
        .iter()
        .any(|&cell| site.base.filling(cell) == Some(Panel::Solid));
    let price: Option<(Cost, u32)> = match (&builder.picked().kind, builder.target) {
        (
            Kind::Room(_),
            Target::Ground {
                corner,
                tiles,
                fits: true,
                adds: true,
                ..
            },
        ) => {
            let new_cells = (0..tiles.x)
                .flat_map(|i| (0..tiles.y).map(move |j| IVec3::new(i, 0, j)))
                .filter(|&step| !site.base.contains(corner + step))
                .count();
            Some((catalogue::FLOOR, catalogue::floor_units(new_cells)))
        }
        (Kind::Reshape, Target::Wall(face)) if !builder.pull => Some((
            catalogue::FLOOR,
            catalogue::floor_units(site.base.wall(face).len()),
        )),
        (Kind::Wall(_), Target::Edge { fits: true, .. }) => {
            let new_cells = builder
                .run
                .iter()
                .filter(|&&cell| site.base.partition(cell).is_none())
                .count();
            (new_cells > 0).then(|| (catalogue::WALL, catalogue::wall_units(new_cells)))
        }
        (Kind::Window(_), Target::Wall(_) | Target::Edge { .. }) if cuts => Some((
            catalogue::WINDOW,
            catalogue::window_units(builder.run.len()),
        )),
        (Kind::Door, Target::Wall(_) | Target::Edge { .. }) if cuts => Some((catalogue::DOOR, 1)),
        (Kind::Items(_), Target::Spot { blocked: None, .. }) => {
            builder.item().map(|item| (item.cost, 1))
        }
        _ => None,
    };
    builder.short = price.and_then(|(cost, times)| belongings.missing(cost, times));
    builder.price = price;
}
