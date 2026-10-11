//! Showing what the builder would do before it is done: outlines, footprints and ghosts.

use super::*;
use crate::experiments::build::Site;
use crate::experiments::play::catalogue::Kind;
use crate::experiments::play::props::{self, Blocked, Models, Prop};
use crate::planet::Planet;
use bevy::{asset::RenderAssetUsages, render::render_resource::PrimitiveTopology};
use terra_build::{Base, CELL_M, DOOR_HEIGHT_M, Face, Panel, STOREY_M};

/// The rectangle a stretch of wall cells covers, in the base's own space: low-left,
/// low-right, high-right, high-left, as seen from the side the wall faces.
pub(super) fn bounds(run: &[Face]) -> Option<[Vec3; 4]> {
    let [origin, right, _, above] = Base::corners(*run.first()?);
    let across = (right - origin).normalize();
    let up = (above - origin).normalize();
    let (mut low, mut high) = (Vec2::MAX, Vec2::MIN);
    for corner in run.iter().flat_map(|&cell| Base::corners(cell)) {
        let at = Vec2::new((corner - origin).dot(across), (corner - origin).dot(up));
        low = low.min(at);
        high = high.max(at);
    }
    let at = |x: f32, y: f32| origin + across * x + up * y;
    Some([
        at(low.x, low.y),
        at(high.x, low.y),
        at(high.x, high.y),
        at(low.x, high.y),
    ])
}

/// The outline of the window or doorway a stretch of wall cells would be cut as.
pub(super) fn opening(run: &[Face], panel: Panel) -> Option<[Vec3; 4]> {
    let [low_left, low_right, _, high_left] = bounds(run)?;
    let up = (high_left - low_left).normalize();
    let (bottom, top) = match panel {
        Panel::Solid => return None,
        Panel::Window { sill, top } => (sill, STOREY_M - top),
        Panel::Door => (0.0, DOOR_HEIGHT_M),
    };
    Some([
        low_left + up * bottom,
        low_right + up * bottom,
        low_right + up * top,
        low_left + up * top,
    ])
}

/// Shows what the tool would act on.
pub(super) fn highlight(
    mut commands: Commands,
    site: Res<Site>,
    planet: Res<Planet>,
    models: Res<Models>,
    props: Query<(&Prop, &Transform)>,
    mut builder: ResMut<Builder>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut gizmos: Gizmos,
) {
    let kind = &builder.picked().kind;
    let refused = matches!(
        builder.target,
        Target::Ground { fits: false, .. }
            | Target::Ground { adds: false, .. }
            | Target::Edge { fits: false, .. }
            | Target::Spot {
                blocked: Some(_),
                ..
            }
    ) || builder.short.is_some()
        || builder.unclaimed.is_some()
        || (matches!(kind, Kind::Window(_) | Kind::Door)
            && matches!(builder.target, Target::Wall(_) | Target::Edge { .. })
            && builder.run.is_empty());
    let pulling = matches!(kind, Kind::Reshape) && builder.pull;
    let look = match (kind, builder.target) {
        (Kind::Move, Target::Prop(_)) => 1,
        _ if refused || pulling => 2,
        (Kind::Remove, _) => 2,
        (Kind::Window(_) | Kind::Door | Kind::Move, _) => 1,
        _ => 0,
    };
    let colour = [BUILD_COLOUR, OPENING_COLOUR, REFUSED_COLOUR][look];
    let in_world = |corners: [Vec3; 4]| corners.map(|corner| site.frame.transform_point(corner));
    let outline = |gizmos: &mut Gizmos, [a, b, c, d]: [Vec3; 4], colour: Color| {
        gizmos.linestrip([a, b, c, d, a], colour);
    };
    let boxed = |gizmos: &mut Gizmos, half: Vec3, stood: &Transform, colour: Color| {
        for height in [0.02, half.y * 2.0] {
            let corner =
                |x: f32, z: f32| stood.transform_point(Vec3::new(x * half.x, height, z * half.z));
            outline(
                gizmos,
                [
                    corner(-1.0, -1.0),
                    corner(1.0, -1.0),
                    corner(1.0, 1.0),
                    corner(-1.0, 1.0),
                ],
                colour,
            );
        }
    };

    // The rectangles to cover with film, in the base's own space.
    let quads: Vec<[Vec3; 4]> = match (kind, builder.target) {
        (Kind::Reshape, Target::Wall(face)) => bounds(&site.base.wall(face)).into_iter().collect(),
        // A window or door that will not fit: the cell aimed at, in red.
        (Kind::Window(_) | Kind::Door, Target::Wall(face) | Target::Edge { face, .. })
            if builder.run.is_empty() =>
        {
            vec![Base::corners(face)]
        }
        (Kind::Window(shapes), Target::Wall(_) | Target::Edge { .. }) => {
            opening(&builder.run, shapes[builder.variant()].2)
                .into_iter()
                .collect()
        }
        (Kind::Door, Target::Wall(_) | Target::Edge { .. }) => {
            opening(&builder.run, Panel::Door).into_iter().collect()
        }
        // An opening to close, or a stretch of interior wall to build or take away.
        (Kind::Remove, Target::Wall(_) | Target::Edge { open: true, .. }) => builder
            .run
            .first()
            .and_then(|&cell| site.base.filling(cell))
            .and_then(|panel| opening(&builder.run, panel))
            .into_iter()
            .collect(),
        (_, Target::Edge { .. }) => bounds(&builder.run).into_iter().collect(),
        (_, Target::Ground { corner, tiles, .. }) => {
            let low = corner.as_vec3() * CELL_M;
            let high = low + Vec3::new(tiles.x as f32 * CELL_M, STOREY_M, tiles.y as f32 * CELL_M);
            let at = |x: bool, y: bool, z: bool| {
                Vec3::new(
                    if x { high.x } else { low.x },
                    if y { high.y } else { low.y },
                    if z { high.z } else { low.z },
                )
            };
            vec![
                [
                    at(false, false, false),
                    at(true, false, false),
                    at(true, true, false),
                    at(false, true, false),
                ],
                [
                    at(false, false, true),
                    at(true, false, true),
                    at(true, true, true),
                    at(false, true, true),
                ],
                [
                    at(false, false, false),
                    at(false, false, true),
                    at(false, true, true),
                    at(false, true, false),
                ],
                [
                    at(true, false, false),
                    at(true, false, true),
                    at(true, true, true),
                    at(true, true, false),
                ],
                [
                    at(false, true, false),
                    at(true, true, false),
                    at(true, true, true),
                    at(false, true, true),
                ],
            ]
        }
        _ => Vec::new(),
    };
    // While a wall or room is being built, a stronger film climbs it; while one is being
    // taken apart, the film drains away.
    let filling = builder.filling;
    match builder.work.as_ref() {
        Some(Work {
            job: Job::Structure { .. },
            hold,
            removing,
            ..
        }) if !quads.is_empty() => {
            let done = hold.fraction();
            let covered = if *removing { 1.0 - done } else { done };
            let mut positions = Vec::new();
            for [a, b, c, d] in &quads {
                let (c, d) = (*b + (*c - *b) * covered, *a + (*d - *a) * covered);
                let lift = (*b - *a).cross(c - *a).normalize_or_zero() * 0.1;
                for offset in [lift, -lift] {
                    positions.extend([*a, *b, c, *a, c, d].map(|corner| corner + offset));
                }
            }
            let normals = vec![Vec3::Y; positions.len()];
            commands.entity(filling).insert((
                Mesh3d(
                    meshes.add(
                        Mesh::new(
                            PrimitiveTopology::TriangleList,
                            RenderAssetUsages::RENDER_WORLD,
                        )
                        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
                        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals),
                    ),
                ),
                MeshMaterial3d(builder.ghost_looks[look].clone()),
                site.frame,
                Visibility::Visible,
            ));
        }
        _ => {
            commands.entity(filling).insert(Visibility::Hidden);
        }
    }

    // Outlines are drawn afresh every frame; the film only when something changes.
    for quad in &quads {
        outline(&mut gizmos, in_world(*quad), colour);
    }
    // A box round an item that would be picked up or taken away.
    if let Target::Prop(entity) = builder.target
        && let Ok((prop, transform)) = props.get(entity)
    {
        boxed(&mut gizmos, prop.half, transform, colour);
    }

    let ghost = builder.ghost;
    match (builder.item(), builder.target) {
        (
            Some(item),
            Target::Spot {
                at,
                lift,
                indoors,
                blocked,
            },
        ) => {
            let stood =
                props::standing(&site, &planet, at, lift + item.lift, builder.turn, indoors);
            // The grid the item is snapping to, drawn on the floor or ground round it:
            // tile edges strongly, the steps between them faintly, fading with distance.
            if builder.snapping {
                let reach = 6;
                let onto = |east: i32, south: i32| {
                    let on_grid =
                        ((at / GRID_M).round() + IVec3::new(east, 0, south).as_vec3()) * GRID_M;
                    let point = site.frame.transform_point(on_grid.with_y(at.y + 0.03));
                    if indoors {
                        point
                    } else {
                        planet.ground_at(point) + point.normalize() * 0.05
                    }
                };
                let points: Vec<Vec<Vec3>> = (-reach..=reach)
                    .map(|east| (-reach..=reach).map(|south| onto(east, south)).collect())
                    .collect();
                // Fainter the further a stretch of line is from the item.
                let strength = |line: i32, along: i32| {
                    let far = ((line * line + along * along) as f32).sqrt() / reach as f32;
                    (1.0 - far).max(0.0) * 0.9
                };
                let tiles_every = (CELL_M / GRID_M).round() as i32;
                let centre = (at / GRID_M).round().as_ivec3();
                for line in -reach..=reach {
                    for along in -reach..reach {
                        let (i, j) = ((line + reach) as usize, (along + reach) as usize);
                        let fade = strength(line, along);
                        let weight = |coordinate: i32| {
                            if coordinate.rem_euclid(tiles_every) == 0 {
                                1.0
                            } else {
                                0.4
                            }
                        };
                        // A line running south, and one running east.
                        gizmos.line(
                            points[i][j],
                            points[i][j + 1],
                            Color::WHITE.with_alpha(fade * weight(centre.x + line)),
                        );
                        gizmos.line(
                            points[j][i],
                            points[j + 1][i],
                            Color::WHITE.with_alpha(fade * weight(centre.z + line)),
                        );
                    }
                }
            }
            // The item's own footprint, and whatever is in its way.
            if let Some(model) = models.of(item) {
                boxed(&mut gizmos, model.half, &stood, colour);
                if let Some(mesh) = model.solid.clone() {
                    commands.entity(ghost).insert((
                        Mesh3d(mesh),
                        MeshMaterial3d(builder.ghost_looks[look].clone()),
                        stood.with_translation(stood.translation + *stood.up() * 0.01),
                        Visibility::Visible,
                    ));
                }
            }
            if let Some(Blocked::Item(other)) = blocked
                && let Ok((prop, transform)) = props.get(other)
            {
                boxed(&mut gizmos, prop.half, transform, REFUSED_COLOUR);
            }
        }
        _ => {
            commands.entity(ghost).insert(Visibility::Hidden);
        }
    }

    if builder.shown == (quads.clone(), look) {
        return;
    }
    builder.shown = (quads.clone(), look);
    let mut film = commands.entity(builder.highlight);
    if quads.is_empty() {
        film.insert(Visibility::Hidden);
        return;
    }
    let mut positions = Vec::new();
    for [a, b, c, d] in quads {
        // A hair off the surface on both sides, so it shows from indoors and out.
        let lift = (b - a).cross(c - a).normalize_or_zero() * 0.08;
        for offset in [lift, -lift] {
            positions.extend([a, b, c, a, c, d].map(|corner| corner + offset));
        }
    }
    let normals = vec![Vec3::Y; positions.len()];
    film.insert((
        Mesh3d(
            meshes.add(
                Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::RENDER_WORLD,
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals),
            ),
        ),
        MeshMaterial3d(builder.looks[look].clone()),
        site.frame,
        Visibility::Visible,
    ));
}
