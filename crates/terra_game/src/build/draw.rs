//! Drawing a build from its cubes: the plain mesh of floors, roofs and walls with no kit,
//! each kit's pieces, block kinds' surfaces (walls, floors and footings alike), glass, and
//! stone steps from each door down to the ground. Moved out of `build.rs` whole; how pieces
//! are laid stays with `terra_build`.

use super::{Build, Shown, pieces, relief};
use crate::planet::Planet;
use crate::play::{Footprint, Steps};
use bevy::{
    asset::RenderAssetUsages, light::NotShadowCaster, prelude::*,
    render::render_resource::PrimitiveTopology,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use terra_build::kit::{Kit, Kits, Pattern};
use terra_build::layout::{self, Piece};
use terra_build::surface::{Heights, Look};
use terra_build::{Base, CELL_M, Fill, Kind, Surface};
use terra_build::{Material, Surfaces};

/// How far a block kind's floors' flagstones stand out, as a fraction of its walls' stone:
/// flatter, as they are walked on.
const FLOOR_RELIEF: f32 = 0.3;

fn to_mesh(surface: &Surface) -> Option<Mesh> {
    if surface.positions.is_empty() {
        return None;
    }
    // The base's colours are as they should look on screen; a mesh wants them linear.
    // Runs of corners share a colour, a log's or a face's, so each is turned once.
    let mut last: Option<([f32; 3], [f32; 4])> = None;
    let colours: Vec<[f32; 4]> = surface
        .colours
        .iter()
        .map(|c| match last {
            Some((from, to)) if from == *c => to,
            _ => {
                let to = Color::srgb(c[0], c[1], c[2]).to_linear().to_f32_array();
                last = Some((*c, to));
                to
            }
        })
        .collect();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, surface.positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, surface.normals.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    // Textured: where each corner falls on its texture, and the tangents its normal map
    // needs, worked out per triangle (`Surface::tangents`), as MikkTSpace took a tenth of a
    // second on a cabin.
    if surface.uvs.len() == surface.positions.len() {
        mesh = mesh
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, surface.uvs.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, surface.tangents());
    }
    Some(mesh)
}

/// Each kit's materials, from its textures (D14), so two timber kinds can differ in bark:
/// by kind, and for what is drawn for no one kind (a roof, a floor, a gable's boards) the
/// first kit's to name the material. Kits naming the same textures share one material,
/// so their pieces draw together.
#[derive(Default)]
pub(super) struct KitMaterials {
    by_kind: BTreeMap<(Kind, Material), Handle<StandardMaterial>>,
    first: BTreeMap<Material, Handle<StandardMaterial>>,
}

impl KitMaterials {
    /// A kind's material, or the first kit's for no kind.
    fn get(&self, kind: Option<Kind>, material: Material) -> Option<&Handle<StandardMaterial>> {
        kind.and_then(|kind| self.by_kind.get(&(kind, material)))
            .or_else(|| self.first.get(&material))
    }
}

/// The kits' materials, each from its textures: colour, normal, roughness and occlusion,
/// and parallax where it is given a depth (D14).
pub(super) fn kit_materials(
    kits: &Kits,
    assets: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
) -> Arc<KitMaterials> {
    let linear = |path: String| -> Handle<Image> {
        assets
            .load_builder()
            .with_settings(|settings: &mut bevy::image::ImageLoaderSettings| {
                settings.is_srgb = false;
            })
            .load(path)
    };
    let mut made: BTreeMap<(String, [u32; 2]), Handle<StandardMaterial>> = BTreeMap::new();
    let mut out = KitMaterials::default();
    for kit in &kits.0 {
        for (material, texture) in &kit.textures {
            let path = &texture.path;
            let handle = made
                .entry((
                    path.clone(),
                    [texture.depth.to_bits(), texture.reflectance.to_bits()],
                ))
                .or_insert_with(|| {
                    let arm: Handle<Image> = linear(format!("{path}/arm.jpg"));
                    let parallax = texture.depth > 0.0;
                    materials.add(StandardMaterial {
                        base_color_texture: Some(assets.load(format!("{path}/color.jpg"))),
                        normal_map_texture: Some(linear(format!("{path}/normal.jpg"))),
                        metallic_roughness_texture: Some(arm.clone()),
                        occlusion_texture: Some(arm),
                        perceptual_roughness: 1.0,
                        metallic: 1.0,
                        reflectance: texture.reflectance,
                        depth_map: parallax.then(|| linear(format!("{path}/height.jpg"))),
                        parallax_depth_scale: texture.depth,
                        parallax_mapping_method:
                            bevy::pbr::ParallaxMappingMethod::DEFAULT_RELIEF_MAPPING,
                        max_parallax_layer_count: 16.0,
                        ..default()
                    })
                })
                .clone();
            out.by_kind.insert((kit.kind(), *material), handle.clone());
            out.first.entry(*material).or_insert(handle);
        }
    }
    Arc::new(out)
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
            let layouts: Vec<layout::Layout> = kits
                .0
                .iter()
                .map(|kit| layout::lay_among(&site.base, kit, &kits))
                .collect();
            let laid: Vec<(&Kit, &[Piece])> = kits
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
            let drawn = draw_pieces(&laid, &states, &site.base, site.reliefs.as_deref());
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
    let (mut placed, mut ghost, next) = states;
    // Stone steps from each door down to the ground, in the footings' own stone.
    let frame = site.frame;
    site.flights = layout::ground::flights(&site.base, |at| {
        -planet.distance(frame.transform_point(at.with_y(0.0)))
    });
    if let Some(kits) = site.kits.as_deref()
        && let Some(stone) = super::footing_kind(kits).and_then(|kind| kits.of(kind))
    {
        let steps = draw_steps(&site.flights, stone, site.reliefs.as_deref());
        let own = placed.entry(stone.kind()).or_default();
        *own = joined(std::mem::take(own), steps);
    }
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
    // What is built in each kind's own materials, and the rest in the first kit's.
    let built = std::iter::once((None, &mesh.textured)).chain(
        placed
            .iter()
            .map(|(kind, surfaces)| (Some(*kind), surfaces)),
    );
    for (kind, surfaces, state) in built
        .map(|(kind, surfaces)| (kind, surfaces, 0))
        .chain([(None, &ghost, 1), (None, &next, 2)])
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
                    .get(kind, *material)
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

    site.footprint = Footprint {
        frame: site.frame,
        cell_m: CELL_M,
        ground: site.base.ground_columns().collect(),
        standing: site.base.occupied_columns().collect(),
        steps: site
            .flights
            .iter()
            .map(|flight| Steps {
                top: flight.top,
                outward: flight.outward,
                length: flight.length,
                width: flight.width,
            })
            .collect(),
    };
}

/// Colour of a plan's pieces not yet in: each a shade apart, so they can be counted.
const GHOST: [f32; 3] = [0.62, 0.84, 1.0];

/// The kits' pieces as surfaces by material: those in, by their kind, each drawn in its
/// own kit's materials; those still a ghost, and the next to go in, which are drawn alike
/// whatever their kind. Doors and windows are props, placed apart.
fn draw_pieces(
    laid: &[(&Kit, &[Piece])],
    states: &[Vec<State>],
    base: &Base,
    reliefs: Option<&relief::Reliefs>,
) -> (BTreeMap<Kind, Surfaces>, Surfaces, Surfaces) {
    let mut placed = BTreeMap::new();
    let (mut ghost, mut next) = (Surfaces::default(), Surfaces::default());
    for (&(kit, pieces), states) in laid.iter().zip(states) {
        let shape = relief::of(reliefs, kit);
        let [p, g, n] = draw_kit(pieces, kit, states, shape);
        let top = relief::of_top(reliefs, kit);
        let [bp, bg, bn] = draw_blocks(pieces, kit, states, base, shape, top);
        let own: &mut Surfaces = placed.entry(kit.kind()).or_default();
        *own = joined(std::mem::take(own), joined(p, bp));
        ghost = joined(ghost, joined(g, bg));
        next = joined(next, joined(n, bn));
    }
    (placed, ghost, next)
}

/// A block kind's look as its surface is drawn (D197): from its kit, its tops in its
/// `Floor` material where it has one.
fn block_look(kit: &Kit) -> Look {
    let metres = |material: Material| kit.textures.get(&material).map(|t| t.metres);
    Look {
        bevel: kit.bevel,
        relief: kit.relief,
        steps: SURFACE_STEPS,
        metres: metres(Material::Masonry).unwrap_or([2.0, 2.0]),
        top: metres(Material::Floor).map(|metres| (metres, kit.relief * FLOOR_RELIEF)),
    }
}

/// Flights of steps, each step a slab of a block kind's material, drawn as its surface.
fn draw_steps(
    flights: &[layout::ground::Flight],
    kit: &Kit,
    reliefs: Option<&relief::Reliefs>,
) -> Surfaces {
    let look = block_look(kit);
    let (sides, top) = (relief::of(reliefs, kit), relief::of_top(reliefs, kit));
    let sides = |uv: [f32; 2]| sides.map_or(0.5, |relief| relief.at(uv));
    let top = |uv: [f32; 2]| top.map_or(0.5, |relief| relief.at(uv));
    let heights = Heights {
        sides: &sides,
        top: &top,
    };
    let mut out = Surfaces::default();
    let (mut faces, mut tops) = (Surface::default(), Surface::default());
    for &(min, max) in flights.iter().flat_map(|flight| &flight.steps) {
        terra_build::surface::slab(min, max, &look, &heights, [1.0; 3], &mut faces, &mut tops);
    }
    *out.of(Material::Masonry) = faces;
    *out.of(Material::Floor) = tops;
    out
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
    pieces: &[Piece],
    kit: &Kit,
    states: &[State],
    base: &Base,
    relief: Option<&terra_build::relief::Relief>,
    top: Option<&terra_build::relief::Relief>,
) -> [Surfaces; 3] {
    if kit.pattern != Pattern::Masonry {
        return Default::default();
    }
    let mut sets: [std::collections::BTreeSet<[i32; 3]>; 3] = Default::default();
    // A gable's cubes, with the roof's underside over each, which cuts them.
    let mut gables: BTreeMap<[i32; 3], [[i32; 3]; 3]> = BTreeMap::new();
    for (piece, state) in pieces.iter().zip(states) {
        let cube = match piece {
            Piece::Block { cube } => cube,
            Piece::Gable { cube, under } => {
                gables.insert(cube.to_array(), *under);
                cube
            }
            _ => continue,
        };
        let set = match state {
            State::In => 0,
            State::Next => 2,
            State::Ghost => 1,
            State::Left => continue,
        };
        sets[set].insert(cube.to_array());
    }
    let cut = |cube: IVec3, at: Vec3| {
        gables
            .get(&cube.to_array())
            .map_or(f32::MAX, |under| layout::roofs::gable_cut(cube, under, at))
    };
    let look = block_look(kit);
    let sides = |uv: [f32; 2]| relief.map_or(0.5, |relief| relief.at(uv));
    let top = |uv: [f32; 2]| top.map_or(0.5, |relief| relief.at(uv));
    let heights = Heights {
        sides: &sides,
        top: &top,
    };
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
        let mut tops = Surface::default();
        terra_build::surface::draw_cut(
            &sets[set],
            &hides,
            &look,
            &heights,
            &cut,
            colour,
            out[set].of(Material::Masonry),
            &mut tops,
        );
        *out[set].of(Material::Floor) = tops;
    }
    out
}

/// How finely a block kind's cube faces are divided: a step every 5 cm or so.
const SURFACE_STEPS: u32 = 10;

/// One kit's pieces: those in, those not yet, and the next. `face` is the height map its
/// logs' bark stands out by, once read.
fn draw_kit(
    pieces: &[Piece],
    kit: &Kit,
    states: &[State],
    face: Option<&terra_build::relief::Relief>,
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
        let plain = *state == State::Ghost;
        terra_build::draw::draw_as(piece, kit, face, plain, surfaces);
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
fn states(pieces: &[Piece], build: &Build, by_cubes: bool) -> Vec<State> {
    let opening = |piece: &Piece| build.fitted_props && matches!(piece, Piece::Opening { .. });
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
            // A piece of roof is up with its cell's stage; the stage after is next once
            // nothing of the walls is.
            if let Some(cell) = piece.roof_cell() {
                let stage = build
                    .base
                    .roof(cell.roof)
                    .and_then(|roof| roof.stage(cell.cell))
                    .unwrap_or(0);
                return if stage >= cell.stage {
                    State::In
                } else if stage + 1 == cell.stage && build.next.is_empty() {
                    State::Next
                } else {
                    State::Ghost
                };
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
fn piece_in(base: &Base, piece: &Piece) -> (bool, Vec<[i32; 3]>) {
    let point_cube = |point: Vec3| (point / CELL_M).floor().as_ivec3();
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
                // reaches into, nor an opening it frames, nor one being carved.
                let Some(cube) = base
                    .cube(at)
                    .filter(|cube| layout::drawn_fill(cube) != Fill::Open)
                else {
                    continue;
                };
                is_in &= cube.level() >= needed;
                cubes.push(at.to_array());
            }
        }
    }
    if cubes.is_empty() {
        // Flat, as a floor's boards on its top, or over a wall's top, as a gable is: in
        // with the first cube under it.
        let mut at = point_cube((min + max) * 0.5 - Vec3::Y * 0.01);
        for _ in 0..terra_build::STOREY_CUBES * 2 {
            if base.cube(at).is_some() {
                break;
            }
            at -= IVec3::Y;
        }
        let is_in = base.cube(at).is_none_or(|cube| cube.is_in());
        return (is_in, vec![at.to_array()]);
    }
    (is_in, cubes)
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
