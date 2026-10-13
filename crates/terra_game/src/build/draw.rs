//! Drawing a build from its cubes: the plain mesh of floors, roofs and walls with no kit,
//! each kit's pieces, block kinds' surfaces, glass, and the stand-in footings and steps.
//! Moved out of `build.rs` whole; how pieces are laid stays with `terra_build`.

use super::{Build, Shown, pieces, relief};
use crate::planet::Planet;
use crate::play::{Footprint, Steps};
use bevy::{
    asset::RenderAssetUsages, light::NotShadowCaster, prelude::*,
    render::render_resource::PrimitiveTopology,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use terra_build::logs::{self, Kits, LogKit, Pattern};
use terra_build::{Base, CELL_M, Fill, Kind, Panel, ROOF_STAGES, Surface};
use terra_build::{Material, Surfaces};

/// The foundation stands this far out from the walls, and reaches this far into the ground.
const FOUNDATION_LEDGE_M: f32 = 0.14;
const FOUNDATION_DEPTH_M: f32 = 0.7;
/// Half the width of the footing under a wall that stands on the ground.
const WALL_FOOTING_HALF_M: f32 = 0.1;
/// Steps run this far out for every metre they have to climb.
const RAMP_RUN_PER_RISE: f32 = 2.4;
/// Steps are this much wider than the doorway they lead to.
const STEPS_WIDER_M: f32 = 0.5;
const STEP_RISE_M: f32 = 0.17;

/// A sloping walkway from a door's sill down to the ground, in the base's own space.
pub(super) struct Ramp {
    /// The middle of the door's sill.
    pub(super) top: Vec3,
    /// Level, pointing away from the door.
    pub(super) outward: Vec3,
    pub(super) length: f32,
    /// How far the far end is below the sill.
    pub(super) drop: f32,
    pub(super) width: f32,
}

impl Ramp {
    /// The ramp drawn as a flight of steps, each a block reaching down into the ground.
    fn steps(&self, surface: &mut Blocks) {
        let across = self.outward.cross(Vec3::Y) * (self.width * 0.5);
        let count = (self.drop / STEP_RISE_M).ceil().max(1.0);
        let tread = self.length / count;
        for step in 0..count as usize {
            let near = self.top + self.outward * (tread * step as f32);
            let far = near + self.outward * tread;
            // Level with the ramp at the middle of the tread.
            let top = self.top.y - self.drop * (step as f32 + 0.5) / count;
            let bottom = self.top.y - self.drop - FOUNDATION_DEPTH_M;
            surface.block(
                (near - across).min(far + across).with_y(bottom),
                (near - across).max(far + across).with_y(top),
            );
        }
    }

    /// The height of the ramp's surface under a point, if the point is over the ramp.
    pub(super) fn height_at(&self, point: Vec3) -> Option<f32> {
        let offset = point - self.top;
        let along = offset.dot(self.outward);
        let across = offset.dot(self.outward.cross(Vec3::Y));
        let on_ramp = (0.0..=self.length).contains(&along) && across.abs() <= self.width * 0.5;
        on_ramp.then(|| self.top.y - self.drop * along / self.length)
    }
}

/// Axis-aligned boxes in the base's own space, as flat-faced triangles of one colour.
#[derive(Default)]
pub(super) struct Blocks {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
}

impl Blocks {
    fn block(&mut self, min: Vec3, max: Vec3) {
        let corner = |x: bool, y: bool, z: bool| {
            Vec3::new(
                if x { max.x } else { min.x },
                if y { max.y } else { min.y },
                if z { max.z } else { min.z },
            )
        };
        // Each side as four corners, anticlockwise seen from outside.
        let sides = [
            (
                Vec3::X,
                [
                    (true, false, true),
                    (true, false, false),
                    (true, true, false),
                    (true, true, true),
                ],
            ),
            (
                Vec3::NEG_X,
                [
                    (false, false, false),
                    (false, false, true),
                    (false, true, true),
                    (false, true, false),
                ],
            ),
            (
                Vec3::Y,
                [
                    (false, true, true),
                    (true, true, true),
                    (true, true, false),
                    (false, true, false),
                ],
            ),
            (
                Vec3::NEG_Y,
                [
                    (false, false, false),
                    (true, false, false),
                    (true, false, true),
                    (false, false, true),
                ],
            ),
            (
                Vec3::Z,
                [
                    (false, false, true),
                    (true, false, true),
                    (true, true, true),
                    (false, true, true),
                ],
            ),
            (
                Vec3::NEG_Z,
                [
                    (true, false, false),
                    (false, false, false),
                    (false, true, false),
                    (true, true, false),
                ],
            ),
        ];
        for (normal, corners) in sides {
            let [a, b, c, d] = corners.map(|(x, y, z)| corner(x, y, z));
            self.positions.extend([a, b, c, a, c, d]);
            self.normals.extend([normal; 6]);
        }
    }

    fn into_mesh(self) -> Option<Mesh> {
        if self.positions.is_empty() {
            return None;
        }
        Some(
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals),
        )
    }
}

fn to_mesh(surface: &Surface) -> Option<Mesh> {
    if surface.positions.is_empty() {
        return None;
    }
    // The base's colours are as they should look on screen; a mesh wants them linear.
    let colours: Vec<[f32; 4]> = surface
        .colours
        .iter()
        .map(|c| Color::srgb(c[0], c[1], c[2]).to_linear().to_f32_array())
        .collect();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, surface.positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, surface.normals.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    // Textured: where each corner falls on its texture, and the tangents its normal map
    // needs.
    if surface.uvs.len() == surface.positions.len() {
        let count = surface.positions.len() as u32;
        mesh = mesh
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, surface.uvs.clone())
            .with_inserted_indices(bevy::mesh::Indices::U32((0..count).collect()));
        if let Err(problem) = mesh.generate_tangents() {
            warn!("No tangents for a textured surface: {problem}");
        }
    }
    Some(mesh)
}

/// The kits' materials, each from its textures: colour, normal, roughness and occlusion,
/// and parallax where it is given a depth (D14). Where two kits name the same material,
/// the first's textures are used.
pub(super) fn kit_materials(
    kits: &Kits,
    assets: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
) -> Arc<BTreeMap<Material, Handle<StandardMaterial>>> {
    let linear = |path: String| -> Handle<Image> {
        assets
            .load_builder()
            .with_settings(|settings: &mut bevy::image::ImageLoaderSettings| {
                settings.is_srgb = false;
            })
            .load(path)
    };
    let mut textures = BTreeMap::new();
    for kit in &kits.0 {
        for (material, texture) in &kit.textures {
            textures.entry(*material).or_insert(texture);
        }
    }
    Arc::new(
        textures
            .into_iter()
            .map(|(material, texture)| {
                let path = &texture.path;
                let arm: Handle<Image> = linear(format!("{path}/arm.jpg"));
                let parallax = texture.depth > 0.0;
                let standard = StandardMaterial {
                    base_color_texture: Some(assets.load(format!("{path}/color.jpg"))),
                    normal_map_texture: Some(linear(format!("{path}/normal.jpg"))),
                    metallic_roughness_texture: Some(arm.clone()),
                    occlusion_texture: Some(arm),
                    perceptual_roughness: 1.0,
                    metallic: 1.0,
                    depth_map: parallax.then(|| linear(format!("{path}/height.jpg"))),
                    parallax_depth_scale: texture.depth,
                    parallax_mapping_method:
                        bevy::pbr::ParallaxMappingMethod::DEFAULT_RELIEF_MAPPING,
                    max_parallax_layer_count: 16.0,
                    ..default()
                };
                (material, materials.add(standard))
            })
            .collect(),
    )
}

/// Rebuilds everything drawn for a build from its current shape.
pub(super) fn redraw(
    commands: &mut Commands,
    site: &mut Build,
    planet: &Planet,
    meshes: &mut Assets<Mesh>,
) {
    site.drawn = site.revision;
    site.stale = false;
    // The player's base is drawn by what each cube holds: what is in solid, what is
    // planned as the ghost, the next lit. Anything else is drawn whole, or, on the kit
    // sheet, as far in as it is shown.
    let by_cubes = site.fitted_props && site.shown == Shown::Built;
    let (in_view, planned) = if by_cubes {
        (
            site.base.view(
                |cube| cube.is_in() || cube.fill == Fill::Open,
                |roof| roof.standing(),
            ),
            Some(site.base.view(
                |cube| !cube.is_in() && cube.fill != Fill::Open,
                |roof| !roof.standing(),
            )),
        )
    } else {
        (site.base.clone(), None)
    };
    let (mesh, states) = match site.kits.clone() {
        Some(kits) => {
            let mesh = in_view.mesh_for_kits(&kits);
            let layouts: Vec<logs::Layout> = kits
                .0
                .iter()
                .map(|kit| logs::lay(&site.base, kit))
                .collect();
            let laid: Vec<(&LogKit, &[logs::Piece])> = kits
                .0
                .iter()
                .zip(&layouts)
                .map(|(kit, layout)| (kit, layout.pieces.as_slice()))
                .collect();
            let states: Vec<Vec<State>> = laid
                .iter()
                .map(|&(_, pieces)| states(pieces, site, by_cubes))
                .collect();
            pieces::place(commands, site, &laid, &states);
            site.standing = match site.shown {
                Shown::Built => None,
                shown => Some(site.base.standing(
                    laid.iter().flat_map(|&(_, pieces)| {
                        pieces[..pieces::done(pieces.len(), shown)].iter()
                    }),
                    false,
                )),
            };
            let models = site.kit_pieces.clone();
            let drawn = draw_pieces(
                &laid,
                &states,
                models.as_deref(),
                &site.base,
                site.relief.as_deref(),
            );
            (mesh, drawn)
        }
        None => {
            pieces::place(commands, site, &[], &[]);
            site.standing = None;
            (in_view.mesh(|_| None), Default::default())
        }
    };
    // Planned floors, roofs and walls with no kit, as the ghost.
    let planned_mesh = planned.map(|planned| match site.kits.as_deref() {
        Some(kits) => planned.mesh_for_kits(kits),
        None => planned.mesh(|_| None),
    });
    // What is drawn in each material, built and planned: a mesh for each, under the
    // entity the pieces are drawn on.
    for entity in site.textured_entities.drain(..) {
        commands.entity(entity).despawn();
    }
    let [placed, mut ghost, next] = states;
    if let Some(planned) = planned_mesh {
        let mut plain = Surfaces::default();
        *plain.of(Material::Plain) = planned.structure;
        let mut planned = joined(planned.textured, plain);
        // In the ghost's own blue, as the kit's pieces not yet in are.
        for surface in planned.0.values_mut() {
            surface.colours.fill(GHOST);
        }
        ghost = joined(ghost, planned);
    }
    let built = [&mesh.textured, &placed];
    for (surfaces, state) in built
        .into_iter()
        .map(|surfaces| (surfaces, 0))
        .chain([(&ghost, 1), (&next, 2)])
    {
        for (material, surface) in &surfaces.0 {
            let Some(drawn) = to_mesh(surface) else {
                continue;
            };
            let look = match state {
                1 => site.ghost_material.clone(),
                2 => site.next_material.clone(),
                _ => site
                    .materials
                    .get(material)
                    .cloned()
                    .unwrap_or_else(|| site.plain_material.clone()),
            };
            let entity = commands
                .spawn((
                    Mesh3d(meshes.add(drawn)),
                    MeshMaterial3d(look),
                    Transform::IDENTITY,
                    ChildOf(site.pieces),
                ))
                .id();
            if state == 1 {
                commands.entity(entity).insert(NotShadowCaster);
            }
            site.textured_entities.push(entity);
        }
    }
    let mut show = |entity: Entity, mesh: Option<Mesh>| {
        match mesh {
            Some(mesh) => commands.entity(entity).insert(Mesh3d(meshes.add(mesh))),
            None => commands.entity(entity).remove::<Mesh3d>(),
        };
    };
    show(site.structure, to_mesh(&mesh.structure));
    show(site.glass, to_mesh(&mesh.glass));

    // A foundation under every ground-floor cell with a floor, down into the ground,
    // standing a little proud of the walls.
    let mut foundation = Blocks::default();
    for tile in site.base.floors().filter(|tile| tile.y == 0) {
        let ledge = |step: IVec3| {
            if site.base.has_floor(tile + step) {
                0.0
            } else {
                FOUNDATION_LEDGE_M
            }
        };
        let min =
            tile.as_vec3() * CELL_M - Vec3::new(ledge(IVec3::NEG_X), 0.0, ledge(IVec3::NEG_Z));
        let max = (tile + IVec3::new(1, 0, 1)).as_vec3() * CELL_M
            + Vec3::new(ledge(IVec3::X), 0.0, ledge(IVec3::Z));
        let gap = [
            (min.x, min.z),
            (max.x, min.z),
            (min.x, max.z),
            (max.x, max.z),
        ]
        .map(|(x, z)| planet.distance(site.frame.transform_point(Vec3::new(x, 0.0, z))))
        .into_iter()
        .fold(0.0, f32::max);
        foundation.block(min.with_y(-gap - FOUNDATION_DEPTH_M), max.with_y(-0.004));
    }
    // A footing under every ground-floor wall with no floor beside it, as a free-standing
    // wall's, down into the ground.
    for (tile, _) in site.base.walls().filter(|(tile, _)| tile.y == 0) {
        if site.base.has_floor(tile) {
            continue;
        }
        let middle = (tile.with_y(0).as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL_M;
        let half = Vec3::new(CELL_M * 0.5, 0.0, CELL_M * 0.5);
        let reach = Vec3::new(WALL_FOOTING_HALF_M, 0.0, WALL_FOOTING_HALF_M) - half;
        let (min, max) = (middle - half - reach.max(Vec3::ZERO), middle + half);
        let gap = [min, max, middle.with_x(min.x), middle.with_z(max.z)]
            .map(|at| planet.distance(site.frame.transform_point(at.with_y(0.0))))
            .into_iter()
            .fold(0.0, f32::max);
        foundation.block(min.with_y(-gap - FOUNDATION_DEPTH_M), max.with_y(0.02));
    }
    show(site.foundation, foundation.into_mesh());

    // Steps from every ground-floor door in an outside wall down to the ground.
    site.ramps.clear();
    let mut steps = Blocks::default();
    // One flight for each doorway, however many cells wide it is.
    let doorways: Vec<Vec<[i32; 3]>> = site
        .base
        .walls()
        .filter(|&(tile, panel)| tile.y == 0 && panel == Panel::Door)
        .map(|(tile, _)| {
            let run = site.base.run(tile);
            run.iter().map(|tile| tile.to_array()).collect()
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    for doorway in &doorways {
        let (first, last) = (
            IVec3::from_array(doorway[0]),
            IVec3::from_array(doorway[doorway.len() - 1]),
        );
        let Some(outward) = site.base.outward(first) else {
            continue;
        };
        let outward = outward.as_vec3();
        // The foot of the doorway's outer face, from one side to the other.
        let foot = |tile: IVec3| {
            (tile.with_y(0).as_vec3() + Vec3::new(0.5, 0.0, 0.5) + outward * 0.5) * CELL_M
        };
        let along = (last - first).as_vec3().normalize_or_zero() * CELL_M * 0.5;
        let (near, far) = (foot(first) - along, foot(last) + along);
        let top = (near + far) * 0.5;
        // Long enough for the climb, measured where the steps start and where they end.
        let rise_here = planet.distance(site.frame.transform_point(top)).max(0.0);
        let length = (rise_here * RAMP_RUN_PER_RISE).clamp(1.2, 16.0);
        let far_end = site.frame.transform_point(top + outward * length);
        let drop = planet.distance(far_end).max(0.0) + 0.05;
        let ramp = Ramp {
            top,
            outward,
            length,
            drop,
            width: near.distance(far) + STEPS_WIDER_M,
        };
        ramp.steps(&mut steps);
        site.ramps.push(ramp);
    }
    show(site.steps, steps.into_mesh());
    site.footprint = Footprint {
        frame: site.frame,
        cell_m: CELL_M,
        ground: site.base.ground_columns().collect(),
        standing: site.base.occupied_columns().collect(),
        steps: site
            .ramps
            .iter()
            .map(|ramp| Steps {
                top: ramp.top,
                outward: ramp.outward,
                length: ramp.length,
                width: ramp.width,
            })
            .collect(),
    };
}

/// Colour of a plan's pieces not yet in: each a shade apart, so they can be counted.
const GHOST: [f32; 3] = [0.62, 0.84, 1.0];

/// A kit's pieces as three sets of surfaces by material: those in, those still a ghost,
/// and the next to go in. Pieces a model draws are left out, but for the daub under a log.
fn draw_pieces(
    laid: &[(&LogKit, &[logs::Piece])],
    states: &[Vec<State>],
    models: Option<&pieces::KitPieces>,
    base: &Base,
    relief: Option<&relief::Relief>,
) -> [Surfaces; 3] {
    let [mut placed, mut ghost, mut next] = [
        Surfaces::default(),
        Surfaces::default(),
        Surfaces::default(),
    ];
    for (&(kit, pieces), states) in laid.iter().zip(states) {
        let [p, g, n] = draw_kit(pieces, kit, states, models);
        let [bp, bg, bn] = draw_blocks(pieces, kit, states, base, relief);
        let (p, g, n) = (joined(p, bp), joined(g, bg), joined(n, bn));
        for (into, from) in [(&mut placed, p), (&mut ghost, g), (&mut next, n)] {
            for (material, surface) in from.0 {
                let to = into.of(material);
                to.positions.extend(surface.positions);
                to.normals.extend(surface.normals);
                to.colours.extend(surface.colours);
                to.uvs.extend(surface.uvs);
            }
        }
    }
    [placed, ghost, next]
}

/// Two sets of surfaces as one.
fn joined(mut a: Surfaces, b: Surfaces) -> Surfaces {
    for (material, surface) in b.0 {
        let to = a.of(material);
        to.positions.extend(surface.positions);
        to.normals.extend(surface.normals);
        to.colours.extend(surface.colours);
        to.uvs.extend(surface.uvs);
    }
    a
}

/// A block kind's cubes as its surface (D197): those in, those not yet, and the next, each
/// closed where it meets the others but for what is already built.
fn draw_blocks(
    pieces: &[logs::Piece],
    kit: &LogKit,
    states: &[State],
    base: &Base,
    relief: Option<&relief::Relief>,
) -> [Surfaces; 3] {
    if kit.pattern != Pattern::Masonry {
        return Default::default();
    }
    let mut sets: [std::collections::BTreeSet<[i32; 3]>; 3] = Default::default();
    for (piece, state) in pieces.iter().zip(states) {
        if let logs::Piece::Block { cube } = piece {
            let set = match state {
                State::In => 0,
                State::Next => 2,
                State::Ghost => 1,
                State::Left => continue,
            };
            sets[set].insert(cube.to_array());
        }
    }
    let look = terra_build::surface::Look {
        bevel: kit.bevel,
        relief: kit.relief,
        steps: SURFACE_STEPS,
        metres: kit
            .textures
            .get(&Material::Masonry)
            .map_or([2.0, 2.0], |texture| texture.metres),
    };
    let height = |uv: [f32; 2]| relief.map_or(0.5, |relief| relief.at(uv));
    // Floors, and walls of kinds drawn as boxes, hide a face; walls of other kits do not,
    // being thinner than the cube.
    let hides_always = |at: IVec3| {
        base.cube(at).is_some_and(|cube| {
            cube.fill != terra_build::Fill::Open
                && cube.kind != kit.kind()
                && (!base.in_wall(at) || cube.kind == Kind::PLAIN)
        })
    };
    let mut out: [Surfaces; 3] = Default::default();
    for (set, colour) in [(0, [1.0; 3]), (1, GHOST), (2, [1.0; 3])] {
        // What is built hides what is not; the rest show their own faces.
        let built = &sets[0];
        let hides = |at: IVec3| hides_always(at) || (set != 0 && built.contains(&at.to_array()));
        terra_build::surface::draw(
            &sets[set],
            &hides,
            &look,
            &height,
            colour,
            out[set].of(Material::Masonry),
        );
    }
    out
}

/// How finely a block kind's cube faces are divided: a step every 5 cm or so.
const SURFACE_STEPS: u32 = 10;

/// One kit's pieces: those in, those not yet, and the next.
fn draw_kit(
    pieces: &[logs::Piece],
    kit: &LogKit,
    states: &[State],
    models: Option<&pieces::KitPieces>,
) -> [Surfaces; 3] {
    let [mut placed, mut ghost, mut next] = [
        Surfaces::default(),
        Surfaces::default(),
        Surfaces::default(),
    ];
    for (index, (piece, state)) in pieces.iter().zip(states).enumerate() {
        let surfaces = match state {
            State::In => &mut placed,
            State::Next => &mut next,
            State::Ghost => &mut ghost,
            State::Left => continue,
        };
        let before: BTreeMap<Material, usize> = surfaces
            .0
            .iter()
            .map(|(material, surface)| (*material, surface.colours.len()))
            .collect();
        if models.is_some_and(|models| models.draws(piece)) {
            logs::draw_chinking(piece, kit, surfaces);
        } else {
            logs::draw(piece, kit, surfaces);
        }
        if *state == State::Ghost {
            let shade = if index % 2 == 0 { 1.0 } else { 0.86 };
            let colour = GHOST.map(|c| c * shade);
            for (material, surface) in surfaces.0.iter_mut() {
                let from = before.get(material).copied().unwrap_or(0);
                surface.colours[from..].fill(colour);
            }
        }
    }
    [placed, ghost, next]
}

/// How far a piece is in, as it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum State {
    In,
    /// What goes in next, lit.
    Next,
    /// Not yet in: the ghost.
    Ghost,
    /// Not drawn: a door or window, which is a prop fitted into its opening.
    Left,
}

/// How far each of a kit's pieces is in: by the cubes it stands in, on the player's base;
/// as far as the kit sheet shows it, in the order they go in; or all of it.
fn states(pieces: &[logs::Piece], build: &Build, by_cubes: bool) -> Vec<State> {
    let opening =
        |piece: &logs::Piece| build.fitted_props && matches!(piece, logs::Piece::Opening { .. });
    if !by_cubes {
        let done = pieces::done(pieces.len(), build.shown);
        return pieces
            .iter()
            .enumerate()
            .map(|(index, piece)| match index.cmp(&done) {
                _ if opening(piece) => State::Left,
                std::cmp::Ordering::Less => State::In,
                std::cmp::Ordering::Equal => State::Next,
                std::cmp::Ordering::Greater => State::Ghost,
            })
            .collect();
    }
    pieces
        .iter()
        .map(|piece| {
            if opening(piece) {
                return State::Left;
            }
            let (is_in, cubes) = piece_in(&build.base, piece);
            if is_in {
                State::In
            } else if cubes.iter().any(|at| build.next.contains(at)) {
                State::Next
            } else {
                State::Ghost
            }
        })
        .collect()
}

/// How far a cube must be filled, in sixteenths, for a piece to be in it: up to the middle
/// of the part of the piece within it, so a log is in once the fill reaches its middle.
const SLIVER_M: f32 = 0.05;

/// Whether a piece is in, by the cubes it stands in, and those cubes.
fn piece_in(base: &Base, piece: &logs::Piece) -> (bool, Vec<[i32; 3]>) {
    use logs::Piece;
    let point_cube = |point: Vec3| (point / CELL_M).floor().as_ivec3();
    match *piece {
        // A footing is in with the wall it stands under.
        Piece::Footing { from, to, .. } => {
            let at = point_cube((from + to) * 0.5 + Vec3::Y * (CELL_M * 0.5));
            (base.is_in(at), vec![at.to_array()])
        }
        // A roof's covering is in once the roof under it is covered.
        Piece::Shakes { corners } => {
            let middle = corners.iter().copied().sum::<Vec3>() * 0.25;
            (roof_covered(base, middle), Vec::new())
        }
        Piece::Ridge { from, to } => (roof_covered(base, (from + to) * 0.5), Vec::new()),
        _ => {
            let Some((min, max)) = piece.bounds() else {
                return (true, Vec::new());
            };
            let low = ((min + SLIVER_M) / CELL_M).floor().as_ivec3();
            let high = ((max - SLIVER_M) / CELL_M).floor().as_ivec3();
            let mut cubes = Vec::new();
            let mut is_in = true;
            for x in low.x..=high.x {
                for y in low.y..=high.y {
                    for z in low.z..=high.z {
                        let at = IVec3::new(x, y, z);
                        let bottom = y as f32 * CELL_M;
                        let (from, to) = (min.y.max(bottom), max.y.min(bottom + CELL_M));
                        let middle = ((from + to) * 0.5 - bottom) / CELL_M;
                        let needed = (middle * 16.0).ceil().clamp(1.0, 16.0) as u8;
                        // Only the cubes it stands in count: not air its rough edge
                        // reaches into, nor an opening it frames.
                        let Some(cube) = base.cube(at).filter(|cube| cube.fill != Fill::Open)
                        else {
                            continue;
                        };
                        is_in &= cube.level() >= needed;
                        cubes.push(at.to_array());
                    }
                }
            }
            if cubes.is_empty() {
                // Flat, as a floor's boards on its top: in with the cube under it.
                let at = point_cube((min + max) * 0.5 - Vec3::Y * 0.01);
                let is_in = base.cube(at).is_none_or(|cube| cube.is_in());
                return (is_in, vec![at.to_array()]);
            }
            (is_in, cubes)
        }
    }
}

/// Whether a roof over a point, in the base's own space, is covered there.
fn roof_covered(base: &Base, point: Vec3) -> bool {
    let column = [
        (point.x / CELL_M).floor() as i32,
        (point.z / CELL_M).floor() as i32,
    ];
    match base.roof_over(column).and_then(|roof| base.roof(roof)) {
        Some(roof) => roof.stage(column) == Some(ROOF_STAGES),
        // Past the walls, under the eaves: the nearest roof.
        None => base
            .roofs()
            .any(|(_, roof)| roof.stages.iter().all(|&stage| stage == ROOF_STAGES)),
    }
}

/// Draws the player's base again whenever it has changed since it was last drawn.
pub(super) fn redraw_changed(
    mut commands: Commands,
    mut site: ResMut<super::Site>,
    planet: Res<Planet>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if site.drawn != site.revision || site.stale {
        redraw(&mut commands, &mut site, &planet, &mut meshes);
    }
}
