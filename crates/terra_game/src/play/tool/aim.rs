//! What the builder is aimed at, and what it would cost there.

use super::*;
use crate::build::Site;
use crate::build::act::Building;
use crate::planet::Planet;
use crate::play::View;
use crate::play::catalogue::{Builds, Kind};
use crate::play::claim::Claim;
use crate::play::flora::Obstacles;
use crate::play::props::{self, Blocked, Models, Prop, Standing};
use crate::shell::Options;
use terra_build::RoofSection;
use terra_build::{CELL_M, DOOR_WIDTH_M, Panel, STOREY_M, Side, cells};

pub(super) fn aim(
    planet: Res<Planet>,
    site: Res<Site>,
    options: Res<Options>,
    models: Res<Models>,
    builds: Res<Builds>,
    obstacles: Res<Obstacles>,
    props: Query<(Entity, &Prop, &Transform, &Visibility)>,
    mut players: Query<(&Transform, &View, &Intent, &mut Builder), With<Player>>,
) {
    for (player, view, intent, mut builder) in &mut players {
        // What is being worked on stays the target until the work is done or dropped.
        if builder.work.is_some() {
            continue;
        }
        builder.viewed = None;
        // Not in hand, it aims at nothing, and looks for nothing to aim at.
        if !builder.in_hand {
            builder.target = Target::Nothing;
            continue;
        }
        // A fixed camera is for taking pictures, and an open dial is for choosing.
        if options.vantage.is_some() || intent.choosing {
            builder.target = Target::Nothing;
            continue;
        }
        let to_base = site.frame.compute_affine().inverse();
        let Some(aim) = intent.aim else {
            builder.target = Target::Nothing;
            continue;
        };
        let (origin, direction) = (aim.origin, *aim.direction);
        let local_origin = to_base.transform_point3(origin);
        let local_direction = to_base.transform_vector3(direction).normalize();
        let within = |point: Vec3, reach: f32| point.distance(player.translation) <= reach;
        // The first of the base's walls, floors and roofs the view meets; the ground; and
        // the first item.
        let hull = site.base.raycast(local_origin, local_direction, 200.0);
        // The same, meeting openings too: a doorway is aimed at, not through.
        let any = site
            .base
            .raycast_where(local_origin, local_direction, 200.0, |_| true);
        builder.aimed = any
            .filter(|hit| within(origin + direction * hit.distance, STRUCTURE_REACH_M))
            .map(|hit| hit.cube);
        let ground = planet.raycast(origin, direction, 200.0);
        let met = |distance: f32| local_origin + local_direction * distance;
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
        // With nothing picked, the hammer builds whatever of the base it is on.
        if builder.stowed {
            builder.target = hull
                .filter(|hit| within(origin + direction * hit.distance, STRUCTURE_REACH_M))
                .map_or(Target::Nothing, |hit| Target::Structure { cube: hit.cube });
            continue;
        }

        if !builder.draws(&builds) {
            builder.drag = None;
            builder.point = None;
        }
        // Where an item would stand. On top of an item the view is on, or flush against its
        // side; else the floor, or flush against a wall; else the ground outside. If none of
        // that is within reach, a few steps in front of the player, so there is always
        // something to see.
        let spot = || {
            let half = builder
                .item(&builds)
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
            let on_item = item.filter(|(_, prop, ..)| prop.is_solid()).and_then(
                |(_, prop, distance, side)| {
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
                },
            );
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
                        // Clear of the wall's face.
                        let gap = 0.02;
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
            // Raised as far as the player has asked, unless it must stand where it is.
            let lift = lift
                + builder
                    .item(&builds)
                    .filter(|item| !super::grounded(item))
                    .map_or(0.0, |_| builder.raise);
            let world = site.frame.transform_point(at);
            let wet = !indoors && planet.under_sea(world);
            let blocked = props::blocked(
                (&site.base, &site.footprint),
                &boxed(at, lift, indoors),
                props
                    .iter()
                    .filter(|(_, _, _, visibility)| **visibility != Visibility::Hidden)
                    .map(|(entity, prop, _, _)| (entity, prop)),
                &obstacles,
            )
            .or(wet.then_some(Blocked::Water));
            Target::Spot {
                at,
                lift,
                indoors,
                blocked,
            }
        };

        let wall_at = |tile: IVec3| Target::Wall {
            tile,
            fits: true,
            open: site
                .base
                .filling(tile)
                .is_some_and(|panel| panel != Panel::Solid),
        };
        // So many tiles of one wall, centred on one of them. Empty if the wall does not run
        // that far.
        let stretch = |tile: IVec3, count: i32| -> Vec<IVec3> {
            let along = site.base.along(tile);
            let first = tile - along * ((count - 1) / 2);
            let covered: Vec<IVec3> = (0..count).map(|step| first + along * step).collect();
            if covered.iter().all(|&at| site.base.filling(at).is_some()) {
                covered
            } else {
                Vec::new()
            }
        };
        // Any wall the view is on, within reach, an opening in it included.
        let any_wall = any
            .filter(|hit| hit.wall)
            .filter(|hit| within(origin + direction * hit.distance, STRUCTURE_REACH_M));
        let opening_on = |width_m: f32, run: &mut Vec<IVec3>| {
            any_wall.map_or(Target::Nothing, |hit| {
                *run = stretch(hit.face.tile(), cells(width_m));
                wall_at(hit.face.tile())
            })
        };
        // Where the view meets the level of a storey, over the small bumps of the ground;
        // where the ground is well above or below it, as on a hillside, where the view
        // meets the ground, so what is placed is under the aim; and out of reach, at the
        // edge of reach along the view (D144). Walls and rooms are aimed so, so they do not
        // jump as the view crosses grass and stones.
        let feet = to_base.transform_point3(player.translation);
        let on_level = |floor_m: f32| {
            let flat = local_direction.with_y(0.0).normalize_or_zero();
            let along = (floor_m - local_origin.y) / local_direction.y;
            let on_ground = ground.map(|ground| to_base.transform_point3(ground.point));
            let reached = match on_ground {
                _ if local_direction.y < -0.01
                    && along > 0.0
                    && on_ground.is_none_or(|point| (point.y - floor_m).abs() < LEVEL_BUMPS_M) =>
                {
                    local_origin + local_direction * along
                }
                Some(point) => point,
                None => feet + flat * STRUCTURE_REACH_M,
            };
            let out = (reached - feet).with_y(0.0);
            feet.with_y(floor_m) + out.clamp_length_max(STRUCTURE_REACH_M - 1.0)
        };
        // The point of the grid a wall or room is drawn to, followed by what is being
        // dragged out.
        let draw_to = |builder: &mut Builder, at: Vec3, level: i32| {
            let held = builder
                .drag
                .map(|drag| drag.to)
                .or(builder.point.map(|(point, _)| point));
            let point = stick(held, Vec2::new(at.x, at.z) / CELL_M);
            builder.point = Some((point, level));
            if let Some(drag) = builder.drag.as_mut() {
                drag.to = point;
                drag.moved |= point != drag.from;
            }
            builder.drawn()
        };
        let mut run = Vec::new();
        builder.target = match &builder.picked(&builds).kind {
            // A window or a door goes in an outside wall or an interior one alike.
            Kind::Window(shapes) => opening_on(shapes[builder.variant()].1, &mut run),
            Kind::Door => opening_on(DOOR_WIDTH_M, &mut run),
            Kind::Wall(lengths) => {
                // On the grid line nearest the floor the view is on, on its storey; else
                // on the level of the storey chosen, through a roof seen from above
                // (hidden while the player is inside). The wheel turns it.
                let (on_floor, level) = hull
                    .filter(|hit| !hit.wall && hit.face.side == Side::Down)
                    .filter(|hit| within(origin + direction * hit.distance, STRUCTURE_REACH_M))
                    .map(|hit| (met(hit.distance), hit.face.tile[1]))
                    .unwrap_or_else(|| {
                        (on_level(builder.storey as f32 * STOREY_M), builder.storey)
                    });
                // Drawn from tile to tile: the tile under the aim is the one nearest it.
                let half_tile = Vec3::new(CELL_M, 0.0, CELL_M) * 0.5;
                let tile = match draw_to(&mut builder, on_floor - half_tile, level) {
                    // Drawn by dragging, or laid: from one tile to another.
                    Some((from, to, level)) => {
                        run = drawn_wall(from, to, level);
                        run[run.len() / 2]
                    }
                    // The variant's own length, centred where the view is.
                    None => {
                        let tile = (on_floor / CELL_M).floor().as_ivec3().with_y(level);
                        let along = if builder.wall_turned {
                            IVec3::Z
                        } else {
                            IVec3::X
                        };
                        let count = cells(lengths[builder.variant()].1);
                        let first = tile - along * ((count - 1) / 2);
                        run = (0..count).map(|step| first + along * step).collect();
                        tile
                    }
                };
                Target::Wall {
                    tile,
                    fits: true,
                    open: false,
                }
            }
            // A flight, a post or a block: against the face of the base aimed at, or on the
            // ground, whichever the view meets first.
            Kind::Flight(_) | Kind::Post | Kind::Block(_) => {
                let on_base = hull
                    .filter(|hit| within(origin + direction * hit.distance, STRUCTURE_REACH_M))
                    .filter(|hit| ground.is_none_or(|ground| hit.distance < ground.distance))
                    .map(|hit| hit.cube + hit.normal);
                let on_ground = ground
                    .filter(|ground| within(ground.point, STRUCTURE_REACH_M))
                    .map(|ground| {
                        (to_base.transform_point3(ground.point) / CELL_M)
                            .floor()
                            .as_ivec3()
                    });
                on_base
                    .or(on_ground)
                    .map_or(Target::Nothing, |at| Target::Place { at })
            }
            Kind::Room(_) | Kind::Roof(_) | Kind::Floor => {
                let roof = matches!(builder.picked(&builds).kind, Kind::Roof(_));
                let floor = matches!(builder.picked(&builds).kind, Kind::Floor);
                // Across a room already there, which it then extends or roofs; else on the
                // level of the storey chosen.
                let on_base = hull
                    .filter(|hit| ground.is_none_or(|ground| hit.distance < ground.distance))
                    .map(|hit| met(hit.distance).with_y(0.0));
                let floor_m = builder.storey as f32 * STOREY_M;
                let aimed = on_base.unwrap_or_else(|| on_level(floor_m).with_y(0.0));
                let storey = builder.storey;
                let drawn = draw_to(&mut builder, aimed, storey);
                builder.room(&builds).map_or(Target::Nothing, |size| {
                    let (snapped, size) = match drawn {
                        // Drawn by dragging, or laid: the rectangle between two corners.
                        Some((from, to, _)) => drawn_room(from, to),
                        // A roof over the whole of the room aimed into.
                        None if roof
                            && site.base.contains(
                                (aimed / CELL_M).floor().as_ivec3().with_y(builder.storey),
                            ) =>
                        {
                            let tile = (aimed / CELL_M).floor().as_ivec3().with_y(builder.storey);
                            let room = site.base.room(tile);
                            let cells = site
                                .base
                                .tiles()
                                .filter(|&(_, each)| Some(each) == room)
                                .map(|(cell, _)| IVec2::new(cell.x, cell.z));
                            let (low, high) = cells
                                .fold((IVec2::MAX, IVec2::MIN), |(lo, hi), c| {
                                    (lo.min(c), hi.max(c))
                                });
                            // Over its walls too, which are a tile thick (D181).
                            (low - IVec2::ONE, high - low + IVec2::splat(3))
                        }
                        // The variant's own size, centred on the place aimed at, sticking
                        // to where it last snapped until the aim has moved well off it.
                        None => {
                            let raw = Vec2::new(
                                aimed.x / CELL_M - size.x as f32 * 0.5,
                                aimed.z / CELL_M - size.y as f32 * 0.5,
                            );
                            let snapped = stick(builder.held_corner, raw);
                            builder.held_corner = Some(snapped);
                            (snapped, size)
                        }
                    };
                    // On the storey chosen: the base's floor, or above or below it (D143).
                    let corner = IVec3::new(snapped.x, builder.storey, snapped.y);
                    // Set into the ground, a room cuts the ground away where it stands,
                    // but on the base's floor or above, not deeper than its own height.
                    // Below the floor it is a cellar, wholly underground. Checked every
                    // metre or so.
                    let every = cells(1.0).max(1) as usize;
                    let fits = roof
                        || floor
                        || builder.storey < 0
                        || (0..=size.x).step_by(every).chain([size.x]).all(|i| {
                            (0..=size.y).step_by(every).chain([size.y]).all(|j| {
                                let at =
                                    (corner.with_y(0) + IVec3::new(i, 0, j)).as_vec3() * CELL_M;
                                let at_roof = at + Vec3::Y * (floor_m + STOREY_M);
                                planet.distance(site.frame.transform_point(at_roof)) > 0.02
                            })
                        });
                    // Trees and rocks are cleared by the player first, not by the room.
                    let low = corner.with_y(0).as_vec3() * CELL_M;
                    let high =
                        (corner.with_y(0) + IVec3::new(size.x, 0, size.y)).as_vec3() * CELL_M;
                    let middle = site.frame.transform_point((low + high) * 0.5);
                    let reach = (high - low).length() * 0.5 + STOREY_M * 2.0;
                    let clear = roof
                        || !obstacles.near(middle, reach).any(|(at, radius)| {
                            let local = to_base.transform_point3(at);
                            let nearest = local.with_y(0.0).clamp(low, high);
                            nearest.distance(local.with_y(0.0)) < radius
                                && local.y > -STOREY_M
                                && local.y < STOREY_M * 2.0
                        });
                    // Nor is anything set down outdoors where it would stand, such as the
                    // claim stone: it is moved or taken apart first.
                    let footprint = Standing {
                        at: (low + high) * 0.5,
                        lift: -100.0,
                        turn: 0.0,
                        half: ((high - low) * 0.5).with_y(200.0),
                        indoors: false,
                    };
                    let item = props
                        .iter()
                        .filter(|_| !roof)
                        .filter(|(_, prop, _, visibility)| {
                            **visibility != Visibility::Hidden && !prop.indoors && prop.is_solid()
                        })
                        .find(|(_, prop, ..)| footprint.overlaps(&Standing::of(prop)))
                        .map(|(entity, ..)| entity);
                    // A room adds where there is no room or wall yet; a roof, where there is
                    // no roof.
                    let adds = (0..size.x).any(|i| {
                        (0..size.y).any(|j| {
                            let cell = corner + IVec3::new(i, 0, j);
                            if floor {
                                !site.base.has_floor(cell)
                            } else if roof {
                                roofs_anew(
                                    &site.base,
                                    &builder.picked(&builds).kind,
                                    builder.variant(),
                                    cell,
                                )
                            } else {
                                !site.base.contains(cell) && site.base.filling(cell).is_none()
                            }
                        })
                    });
                    Target::Ground {
                        corner,
                        tiles: size,
                        fits,
                        adds,
                        clear,
                        item,
                    }
                })
            }
            Kind::Items(_) => spot(),
            Kind::Move if builder.is_carrying() => spot(),
            Kind::Move => item.map_or(Target::Nothing, |(entity, ..)| Target::Prop(entity)),
            // An item; a door or window fitted; else what of the base is aimed at.
            Kind::Remove => match (item, builder.aimed) {
                (Some((entity, ..)), _) => Target::Prop(entity),
                (None, Some(cube)) => match site.base.prop_in(cube) {
                    Some(prop) => Target::Fitted { at: prop.min },
                    None => Target::Structure { cube },
                },
                _ => Target::Nothing,
            },
        };
        builder.run = run;
    }
}

/// Works out what the thing aimed at would do and cost, and whether it can be done: an
/// item's price and what is short of it; for the base, the work and the base's answer.
pub(super) fn afford(
    builds: Res<Builds>,
    look: Res<crate::build::BuildLook>,
    props: Query<&Prop>,
    mut building: Building,
    mut players: Query<
        (
            Entity,
            &Transform,
            &Claim,
            &mut Builder,
            Has<crate::play::LocalPlayer>,
        ),
        With<Player>,
    >,
) {
    for (player, at, claim, mut builder, local) in &mut players {
        if builder.work.is_some() {
            continue;
        }
        let free = building.free();
        // Nothing is built or taken apart outside your claim (D94), except the claim stone
        // that makes one; and there is one claim each.
        let placing_claim = matches!(builder.picked(&builds).kind, Kind::Items(_))
            && builder.item(&builds).is_some_and(|item| item.claims)
            && !builder.is_carrying();
        let on_planet = target_point(&builder.target, &props)
            .map(|point| building.site.frame.transform_point(point));
        builder.unclaimed = match on_planet {
            // The yard, the flat test level, is all one claim.
            _ if free => None,
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
        let near = building.local(at.translation);
        builder.planned = plan_work(&builds, &look, &mut building, player, near, &builder);
        // What this machine's player would build next is lit in the base.
        if local {
            let next = match &builder.planned {
                Some(Planned {
                    doing: Doing::Build,
                    work,
                }) if work.check.is_ok() => work
                    .ops
                    .iter()
                    .flat_map(|op| match op {
                        Op::Fill { cubes } => cubes.clone(),
                        _ => Vec::new(),
                    })
                    .collect(),
                _ => Default::default(),
            };
            building.site.light(next);
        }
        let price: Option<Price> = match (&builder.picked(&builds).kind, builder.target) {
            (Kind::Items(_), Target::Spot { blocked: None, .. }) => {
                builder.item(&builds).map(|item| (item.cost.clone(), 1))
            }
            _ => None,
        };
        // On the yard, building costs nothing.
        let price = price.filter(|_| !free);
        builder.short = match (&price, building.belongings.of(player)) {
            (Some((cost, times)), Some(belongings)) => belongings.missing(cost, *times),
            _ => None,
        };
        builder.price = price;
    }
}

/// What the hammer would do to the base where it is aimed, with the base's answer: lay
/// what is drawn, cut or fit an opening, build what goes in next, or take apart what came
/// last.
fn plan_work(
    builds: &Builds,
    look: &crate::build::BuildLook,
    building: &mut Building,
    player: Entity,
    near: Vec3,
    builder: &Builder,
) -> Option<Planned> {
    use crate::build::plans;
    let base = &building.site.base;
    let kind = look.laying;
    let ground = |column: [i32; 2]| building.ground_cm(column);
    let footings = building.rules.footing.map(|kind| plans::Footings {
        kind,
        ground_cm: &ground,
    });
    let work = |building: &mut Building, doing: Doing, what: String, ops: Vec<Op>| {
        let ops = building.with_ground(ops);
        let check = building.check(Some(player), &ops);
        Some(Planned {
            doing,
            work: BaseWork { what, ops, check },
        })
    };
    let laying = |what: &str, ops: Vec<Op>| (what.to_string(), ops);
    let lay = match (&builder.picked(builds).kind, builder.target) {
        (Kind::Room(_), Target::Ground { corner, tiles, .. }) => Some(laying(
            "lay a room",
            plans::room(
                base,
                corner,
                corner + IVec3::new(tiles.x - 1, 0, tiles.y - 1),
                kind,
                kind,
                footings,
            ),
        )),
        (Kind::Roof(shapes), Target::Ground { corner, tiles, .. }) => {
            let (min, max) = (corner, corner + IVec3::new(tiles.x - 1, 0, tiles.y - 1));
            let shape = &shapes[builder.variant()];
            Some(match shape.style {
                // A gable's ridge runs along its longer side, unless turned.
                Some(style) => laying(
                    "lay a roof",
                    plans::roof(
                        base,
                        RoofSection {
                            min: min.to_array(),
                            max: max.to_array(),
                            style,
                            along_x: (tiles.x >= tiles.y) != builder.room_turned,
                            pitch: shape.pitch,
                        },
                        kind,
                    ),
                ),
                None => laying("lay a flat roof", plans::flat_roof(base, min, max, kind)),
            })
        }
        (Kind::Wall(_), Target::Wall { .. }) => Some(laying(
            "lay a wall",
            plans::wall(base, &builder.run, kind, footings),
        )),
        (Kind::Floor, Target::Ground { corner, tiles, .. }) => Some(laying(
            "lay a floor",
            plans::floor(
                base,
                corner,
                corner + IVec3::new(tiles.x - 1, 0, tiles.y - 1),
                kind,
                footings,
            ),
        )),
        (Kind::Flight(shapes), Target::Place { at }) => {
            let shape = &shapes[builder.variant()];
            let facing = builder.facing();
            let top = if shape.ramp {
                terra_build::Shape::Slope(facing)
            } else {
                terra_build::Shape::Stair(facing)
            };
            Some(laying(
                if shape.ramp {
                    "lay a ramp"
                } else {
                    "lay stairs"
                },
                plans::flight(base, at, facing, cells(shape.wide), top, kind, footings),
            ))
        }
        (Kind::Post, Target::Place { at }) => {
            Some(laying("lay a post", plans::post(base, at, kind, footings)))
        }
        (Kind::Block(shapes), Target::Place { at }) => {
            let shape = shapes[builder.variant()].1.shape(builder.facing());
            Some(laying("lay a block", plans::block(base, at, shape, kind)))
        }
        _ => None,
    };
    if let Some((what, ops)) = lay {
        return if ops.is_empty() {
            None
        } else {
            work(building, Doing::Lay, what, ops)
        };
    }
    match (&builder.picked(builds).kind, builder.target) {
        // An opening cut into a wall's plan; once the wall is up round it, a door or
        // window fitted into it.
        (Kind::Window(_) | Kind::Door, Target::Wall { .. }) if !builder.run.is_empty() => {
            let (panel, door) = match &builder.picked(builds).kind {
                Kind::Window(shapes) => (shapes[builder.variant()].2, false),
                _ => (Panel::Door, true),
            };
            let cubes = plans::opening_cubes(&builder.run, panel);
            let opened = plans::opening(base, &cubes);
            if opened.is_empty() {
                let prop = plans::prop_for(base, cubes[0], Some(door))?;
                let what = format!("fit a {}", crate::build::props::name(prop.kind));
                work(building, Doing::Fit, what, vec![Op::FitProp(prop)])
            } else {
                let what = if door {
                    "cut a doorway"
                } else {
                    "cut a window"
                };
                work(building, Doing::Lay, what.to_string(), opened)
            }
        }
        (Kind::Remove, Target::Fitted { at }) => {
            let what = base
                .prop(IVec3::from_array(at))
                .map_or("door", |prop| crate::build::props::name(prop.kind));
            work(
                building,
                Doing::TakeApart,
                format!("take the {what} out"),
                vec![Op::RemoveProp { at }],
            )
        }
        (Kind::Remove, Target::Structure { cube })
            if base
                .cube(cube)
                .is_some_and(|cube| cube.fill == terra_build::Fill::Open) =>
        {
            let (low, high) = plans::hole(base, cube)?;
            let cubes = (low.x..=high.x)
                .flat_map(|x| {
                    (low.y..=high.y).flat_map(move |y| (low.z..=high.z).map(move |z| [x, y, z]))
                })
                .collect();
            work(
                building,
                Doing::TakeApart,
                "close the opening".to_string(),
                vec![Op::Close { cubes }],
            )
        }
        (Kind::Remove, Target::Structure { cube }) => {
            let found = building.last_work(Some(player), cube, near)?;
            Some(Planned {
                doing: Doing::TakeApart,
                work: found,
            })
        }
        // With nothing picked, what goes in next.
        (_, Target::Structure { cube }) if builder.stowed => {
            let part = base.cube(cube).and_then(|cube| cube.part);
            let found = building.next_work(Some(player), part, near)?;
            Some(Planned {
                doing: Doing::Build,
                work: found,
            })
        }
        _ => None,
    }
}

/// Whether a roof of this kind would roof a cell anew: a pitched one where no pitched roof
/// is yet, taking the place of a flat one; a flat one where there is no roof at all.
fn roofs_anew(base: &terra_build::Base, kind: &Kind, variant: usize, cell: IVec3) -> bool {
    let pitched = base.sections().iter().any(|section| section.covers(cell));
    match kind {
        Kind::Roof(shapes) if shapes[variant].style.is_some() => !pitched,
        _ => !base.covered(cell),
    }
}
