//! Showing what the builder would do before it is done: outlines, footprints and ghosts.

use super::*;
use crate::build::Site;
use crate::planet::Planet;
use crate::play::catalogue::{Builds, Kind};
use crate::play::props::{self, Blocked, Models, Prop};
use bevy::{
    asset::RenderAssetUsages, ecs::system::SystemParam, render::render_resource::PrimitiveTopology,
};
use terra_build::{Base, CELL_M, DOOR_HEIGHT_M, Panel, STOREY_M};

/// The rectangle up the middle of a stretch of wall tiles, in the base's own space:
/// low-left, low-right, high-right, high-left, as seen from the side the wall faces. Along
/// the way the stretch runs, or for one tile the way its wall does.
pub(super) fn bounds(base: &Base, run: &[IVec3]) -> Option<[Vec3; 4]> {
    let (first, last) = (*run.first()?, *run.last()?);
    let along = if first == last {
        base.along(first)
    } else {
        (last - first).signum().abs()
    };
    let [origin, right, _, above] = terra_build::corners(first, along);
    let across = (right - origin).normalize();
    let up = (above - origin).normalize();
    let (mut low, mut high) = (Vec2::MAX, Vec2::MIN);
    for corner in run
        .iter()
        .flat_map(|&tile| terra_build::corners(tile, along))
    {
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

/// The outline of the window or doorway a stretch of wall tiles would be cut as.
pub(super) fn opening(base: &Base, run: &[IVec3], panel: Panel) -> Option<[Vec3; 4]> {
    let [low_left, low_right, _, high_left] = bounds(base, run)?;
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

/// The builder's own shapes (the ghost, the film over work, the item rising), changed
/// only where they differ, so one left alone is not prepared again every frame.
#[derive(SystemParam)]
pub(super) struct Shapes<'w, 's> {
    commands: Commands<'w, 's>,
    #[allow(clippy::type_complexity)]
    shapes: Query<
        'w,
        's,
        (
            Option<&'static Mesh3d>,
            Option<&'static MeshMaterial3d<StandardMaterial>>,
            &'static mut Transform,
            &'static mut Visibility,
        ),
        Without<Prop>,
    >,
}

impl Shapes<'_, '_> {
    pub(super) fn hide(&mut self, entity: Entity) {
        if let Ok((.., mut visibility)) = self.shapes.get_mut(entity) {
            visibility.set_if_neq(Visibility::Hidden);
        }
    }

    pub(super) fn show(
        &mut self,
        entity: Entity,
        mesh: Handle<Mesh>,
        material: Handle<StandardMaterial>,
        at: Transform,
    ) {
        let Ok((had_mesh, had_material, mut transform, mut visibility)) =
            self.shapes.get_mut(entity)
        else {
            return;
        };
        if had_mesh.map(|had| &had.0) != Some(&mesh) {
            self.commands.entity(entity).insert(Mesh3d(mesh));
        }
        if had_material.map(|had| &had.0) != Some(&material) {
            self.commands
                .entity(entity)
                .insert(MeshMaterial3d(material));
        }
        transform.set_if_neq(at);
        visibility.set_if_neq(Visibility::Visible);
    }
}

/// Shows what the tool would act on.
#[allow(clippy::too_many_arguments)]
/// Shows the door or shutters the hammer would fit as a ghost in the hole aimed at, while
/// it can be fitted there.
pub(super) fn preview_fit(
    builder: Single<&Builder, IsLocalPlayer>,
    mut preview: ResMut<crate::build::props::FitPreview>,
) {
    let wanted = builder
        .planned
        .as_ref()
        .filter(|planned| planned.doing == Doing::Fit && planned.work.check.is_ok())
        .and_then(|planned| match planned.work.ops.first() {
            Some(terra_build::ops::Op::FitProp(prop)) => Some(*prop),
            _ => None,
        })
        .filter(|_| builder.progress().is_none() && !builder.stowed);
    if preview.0 != wanted {
        preview.0 = wanted;
    }
}

pub(super) fn highlight(
    mut commands: Commands,
    mut shapes: Shapes,
    site: Res<Site>,
    planet: Res<Planet>,
    models: Res<Models>,
    builds: Res<Builds>,
    props: Query<(&Prop, &Transform)>,
    builder: Single<&Builder, IsLocalPlayer>,
    mut drawn: ResMut<BuilderLook>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut gizmos: Gizmos,
) {
    let kind = &builder.picked(&builds).kind;
    let refused = matches!(
        builder.target,
        Target::Ground { fits: false, .. }
            | Target::Ground { adds: false, .. }
            | Target::Ground { clear: false, .. }
            | Target::Ground { item: Some(_), .. }
            | Target::Wall { fits: false, .. }
            | Target::Spot {
                blocked: Some(_),
                ..
            }
    ) || builder.short.is_some()
        || builder.unclaimed.is_some()
        || (matches!(kind, Kind::Window(_) | Kind::Door)
            && matches!(builder.target, Target::Wall { .. })
            && builder.run.is_empty());
    let look = match (kind, builder.target) {
        (Kind::Move, Target::Prop(_)) => 1,
        _ if refused => 2,
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
        // A window or door that will not fit: the cell aimed at, in red.
        (Kind::Window(_) | Kind::Door, Target::Wall { tile, .. }) if builder.run.is_empty() => {
            vec![site.base.corners(tile)]
        }
        (Kind::Window(shapes), Target::Wall { .. }) => {
            opening(&site.base, &builder.run, shapes[builder.variant()].2)
                .into_iter()
                .collect()
        }
        (Kind::Door, Target::Wall { .. }) => opening(&site.base, &builder.run, Panel::Door)
            .into_iter()
            .collect(),
        // An opening to close, or a stretch of wall to build or take away.
        (Kind::Remove, Target::Wall { open: true, .. }) => builder
            .run
            .first()
            .and_then(|&cell| site.base.filling(cell))
            .and_then(|panel| opening(&site.base, &builder.run, panel))
            .into_iter()
            .collect(),
        (_, Target::Wall { .. }) => bounds(&site.base, &builder.run).into_iter().collect(),
        (_, Target::Ground { corner, tiles, .. }) => {
            // Its storey's floor, above or below the base's (D143). A roof, from the top
            // of its storey's walls as high as it would rise.
            let (bottom, height) = match kind {
                // Its cubes climb a cube a cube from the walls' tops (D241).
                Kind::Roof(shapes) => {
                    let shape = &shapes[builder.variant()];
                    let across = tiles.min_element() as f32 * CELL_M;
                    let rise = match shape.style {
                        Some(terra_build::roofer::Style::LeanTo) => {
                            tiles.max_element() as f32 * CELL_M
                        }
                        Some(_) => across * 0.5,
                        None => 0.0,
                    };
                    ((corner.y + 1) as f32 * STOREY_M, rise.max(0.1))
                }
                // A floor: the layer of cubes under its storey.
                Kind::Floor => (corner.y as f32 * STOREY_M - CELL_M, CELL_M),
                _ => (corner.y as f32 * STOREY_M, STOREY_M),
            };
            let low = (corner.with_y(0).as_vec3() * CELL_M).with_y(bottom);
            let high = low + Vec3::new(tiles.x as f32 * CELL_M, height, tiles.y as f32 * CELL_M);
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
    let filling = drawn.filling;
    match builder.work.as_ref() {
        Some(Work {
            job: Job::Base { .. },
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
            let mesh = meshes.add(
                Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::RENDER_WORLD,
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals),
            );
            let material = drawn.ghost_looks[look].clone();
            shapes.show(filling, mesh, material, site.frame);
        }
        _ => shapes.hide(filling),
    }

    // The cubes work on the base would touch: blue to build, amber to open or fit, red to
    // take apart or where it is refused.
    if let Some(planned) = builder.planned.as_ref() {
        let colour = match (&planned.work.check, planned.doing) {
            (Err(_), _) | (_, Doing::TakeApart) => REFUSED_COLOUR,
            (_, Doing::Fit) => OPENING_COLOUR,
            _ => BUILD_COLOUR,
        };
        let placed = matches!(builder.target, Target::Place { .. });
        for cube in touched(&planned.work.ops, placed).into_iter().take(400) {
            let low = cube.as_vec3() * CELL_M;
            let middle = site.frame.transform_point(low + Vec3::splat(CELL_M * 0.5));
            gizmos.cube(
                Transform::from_translation(middle)
                    .with_rotation(site.frame.rotation)
                    .with_scale(Vec3::splat(CELL_M * 1.02)),
                colour,
            );
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

    let ghost = drawn.ghost;
    match (builder.item(&builds), builder.target) {
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
                    let at = stood.with_translation(stood.translation + *stood.up() * 0.01);
                    shapes.show(ghost, mesh, drawn.ghost_looks[look].clone(), at);
                }
            }
            if let Some(Blocked::Item(other)) = blocked
                && let Ok((prop, transform)) = props.get(other)
            {
                boxed(&mut gizmos, prop.half, transform, REFUSED_COLOUR);
            }
        }
        // A room cannot go over something set down where it would stand.
        (
            _,
            Target::Ground {
                item: Some(other), ..
            },
        ) => {
            shapes.hide(ghost);
            if let Ok((prop, transform)) = props.get(other) {
                boxed(&mut gizmos, prop.half, transform, REFUSED_COLOUR);
            }
        }
        _ => shapes.hide(ghost),
    }

    if drawn.shown == (quads.clone(), look) {
        return;
    }
    drawn.shown = (quads.clone(), look);
    let mut film = commands.entity(drawn.highlight);
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
        MeshMaterial3d(drawn.looks[look].clone()),
        site.frame,
        Visibility::Visible,
    ));
}

/// The cubes operations on the base would touch, for outlining; none for a whole plan
/// being laid, which is outlined by what is drawn, unless `placed`: a flight, a post or a
/// block, outlined a cube at a time.
fn touched(ops: &[terra_build::ops::Op], placed: bool) -> Vec<IVec3> {
    use terra_build::ops::Op;
    ops.iter()
        .flat_map(|op| match op {
            Op::LayPlan { cubes, .. } if placed => cubes
                .iter()
                .map(|cube| IVec3::from_array(cube.at))
                .collect(),
            Op::Fill { cubes }
            | Op::Empty { cubes }
            | Op::Open { cubes }
            | Op::Close { cubes }
            | Op::Replace { cubes, .. }
            | Op::Carve { cubes } => cubes.iter().map(|&at| IVec3::from_array(at)).collect(),
            Op::FitProp(prop) => prop.cubes().collect(),
            _ => Vec::new(),
        })
        .collect()
}
