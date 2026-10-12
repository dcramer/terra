//! What the builder is aimed at, and what it would cost there.

use super::*;
use crate::build::Site;
use crate::planet::Planet;
use crate::play::View;
use crate::play::catalogue::{self, Builds, Kind};
use crate::play::claim::Claim;
use crate::play::flora::Obstacles;
use crate::play::inventory::Belongings;
use crate::play::props::{self, Blocked, Models, Prop, Standing};
use crate::shell::Options;
use terra_build::{CELL_M, DOOR_WIDTH_M, Face, Panel, STOREY_M, Side, cells};

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
        if builder.stowed {
            builder.target = Target::Nothing;
            continue;
        }

        // What was drawn with another part, or another variant, is let go of.
        let (part, variant) = (builder.part, builder.variant());
        if builder
            .laid
            .is_some_and(|laid| laid.part != part || laid.variant != variant)
        {
            builder.laid = None;
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
                        // Clear of the wall's thickness.
                        let gap = 0.07;
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
                &site,
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

        let edge = |face: Face| Target::Edge {
            face,
            fits: true,
            open: site
                .base
                .filling(face)
                .is_some_and(|panel| panel != Panel::Solid),
        };
        // So many cells of one wall, centred on one of them. Empty if the wall does not run
        // that far.
        let stretch = |face: Face, count: i32| -> Vec<Face> {
            let along = face.side.along();
            let first = face.tile() - along * ((count - 1) / 2);
            let covered: Vec<Face> = (0..count)
                .map(|step| Face::new(first + along * step, face.side))
                .collect();
            if covered
                .iter()
                .all(|&cell| site.base.filling(cell).is_some())
            {
                covered
            } else {
                Vec::new()
            }
        };
        // Any wall the view is on, within reach.
        let any_wall = hull
            .filter(|hit| hit.wall)
            .filter(|hit| within(origin + direction * hit.distance, STRUCTURE_REACH_M));
        let opening_on = |width_m: f32, run: &mut Vec<Face>| {
            any_wall.map_or(Target::Nothing, |hit| {
                *run = stretch(hit.face, cells(width_m));
                edge(hit.face)
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
                let face = match draw_to(&mut builder, on_floor, level) {
                    // Drawn by dragging, or laid: from one point of the grid to another.
                    Some((from, to, level)) => {
                        run = drawn_wall(from, to, level);
                        run[run.len() / 2]
                    }
                    // The variant's own length, centred where the view is.
                    None => {
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
                        face
                    }
                };
                Target::Edge {
                    face,
                    fits: true,
                    open: false,
                }
            }
            Kind::Room(_) | Kind::Roof(_) => {
                let roof = matches!(builder.picked(&builds).kind, Kind::Roof(_));
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
                            (low, high - low + IVec2::ONE)
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
                    // A room adds where there is no room yet; a roof, where there is no roof.
                    let adds = (0..size.x).any(|i| {
                        (0..size.y).any(|j| {
                            let cell = corner + IVec3::new(i, 0, j);
                            if roof {
                                roofs_anew(
                                    &site.base,
                                    &builder.picked(&builds).kind,
                                    builder.variant(),
                                    cell,
                                )
                            } else {
                                !site.base.contains(cell)
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
            Kind::Remove => match (item, hull) {
                (Some((entity, ..)), _) => Target::Prop(entity),
                (None, Some(hit)) if hit.wall => {
                    let target = edge(hit.face);
                    run = if matches!(target, Target::Edge { open: true, .. }) {
                        // The window or doorway in it.
                        site.base.run(hit.face)
                    } else {
                        // The whole straight stretch of plain wall.
                        let along = hit.face.side.along();
                        let plain = |cell: IVec3| {
                            site.base.filling(Face::new(cell, hit.face.side)) == Some(Panel::Solid)
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
                _ => Target::Nothing,
            },
        };
        builder.run = run;
    }
}

/// Works out what the thing aimed at would cost to build, and whether it can be paid.
pub(super) fn afford(
    site: Res<Site>,
    builds: Res<Builds>,
    props: Query<&Prop>,
    mut belongings: Belongings,
    mut players: Query<(Entity, &Claim, &mut Builder), With<Player>>,
) {
    for (player, claim, mut builder) in &mut players {
        let Some(belongings) = belongings.of(player) else {
            continue;
        };
        if builder.work.is_some() {
            continue;
        }
        // Nothing is built or taken apart outside your claim (D94), except the claim stone
        // that makes one; and there is one claim each.
        let placing_claim = matches!(builder.picked(&builds).kind, Kind::Items(_))
            && builder.item(&builds).is_some_and(|item| item.claims)
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
        let price: Option<Price> = match (&builder.picked(&builds).kind, builder.target) {
            (
                Kind::Room(_) | Kind::Roof(_),
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
                    .filter(|&step| {
                        let cell = corner + step;
                        if matches!(builder.picked(&builds).kind, Kind::Roof(_)) {
                            roofs_anew(
                                &site.base,
                                &builder.picked(&builds).kind,
                                builder.variant(),
                                cell,
                            )
                        } else {
                            !site.base.contains(cell)
                        }
                    })
                    .count();
                Some((builds.floor().to_vec(), catalogue::floor_units(new_cells)))
            }
            (Kind::Wall(_), Target::Edge { fits: true, .. }) => {
                let new_cells = builder
                    .run
                    .iter()
                    .filter(|&&cell| site.base.filling(cell).is_none())
                    .count();
                (new_cells > 0).then(|| (builds.wall().to_vec(), catalogue::wall_units(new_cells)))
            }
            (Kind::Window(_), Target::Edge { .. }) if cuts => Some((
                builds.window().to_vec(),
                catalogue::window_units(builder.run.len()),
            )),
            (Kind::Door, Target::Edge { .. }) if cuts => Some((builds.door().to_vec(), 1)),
            (Kind::Items(_), Target::Spot { blocked: None, .. }) => {
                builder.item(&builds).map(|item| (item.cost.clone(), 1))
            }
            _ => None,
        };
        builder.short = price
            .as_ref()
            .and_then(|(cost, times)| belongings.missing(cost, *times));
        builder.price = price;
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
