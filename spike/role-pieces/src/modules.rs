//! Candidate 4, whole modules per role: every exposed side face of a block kind's cube is a
//! module modelled in Blender (`blender/pieces.py`'s tiles): coursed dressed stone in a
//! half bond whose stones run on across the modules' edges, chosen by what the face's four
//! edges are: a coping or a sill over it, a plinth, a lintel or a fascia under it, an
//! outside or inside corner either side, each band modelled to the profiles candidate 3
//! sweeps and standing out as far. Tops and undersides stay the kind's surface, as do a
//! gable's cubes, cut by their roof. Timber is not modules: its logs are generated along
//! runs already (D200), so it is drawn as candidate 3 draws it.

use crate::layers::{self, Mode};
use crate::model::Model;
use crate::profiles::{self, Pieces};
use crate::roles::{Beyond, Edge, Neighbours, edge_faces, edge_index, face_index};
use crate::shapes::Case;
use crate::today::{self, Laid};
use crate::{ASSETS, Drawn, Reliefs, add, add_surfaces, folder};
use glam::{IVec3, Mat3, Quat, Vec3};
use std::collections::BTreeMap;
use terra_build::kit::{Kits, Pattern};
use terra_build::layout::Piece;
use terra_build::{CELL_M, Material, Surface, Surfaces};

pub struct Set {
    pub tiles: BTreeMap<String, Model>,
}

impl Set {
    pub fn load() -> Set {
        let tiles = Model::load_nodes("out/tiles.glb", &format!("{ASSETS}/models/spike"), Some(()))
            .into_iter()
            .collect();
        Set { tiles }
    }
}

/// The edge of a cube between two of its faces.
fn edge_between(a: (usize, i32), b: (usize, i32)) -> usize {
    let axis = 3 - a.0 - b.0;
    let side = |of: usize| if a.0 == of { a.1 } else { b.1 };
    let index = edge_index(axis, [side((axis + 1) % 3) > 0, side((axis + 2) % 3) > 0]);
    debug_assert!(edge_faces(index).contains(&a) && edge_faces(index).contains(&b));
    index
}

fn axis_side(v: Vec3) -> (usize, i32) {
    let a = v.abs();
    let axis = if a.x > 0.5 {
        0
    } else if a.y > 0.5 {
        1
    } else {
        2
    };
    (axis, if v[axis] > 0.0 { 1 } else { -1 })
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    case: &Case,
    kits: &Kits,
    reliefs: &Reliefs,
    door: &Model,
    window: &Model,
    _pieces: &Pieces,
    set: &Set,
    overhang: bool,
) -> Drawn {
    let laid: Vec<Laid> = today::lay(case, kits);
    let mut drawn = Drawn::default();
    today::plain(case, kits, &mut drawn);
    for Laid { kit, pieces: laid_pieces } in &laid {
        if kit.pattern == Pattern::Logs {
            profiles::timber(case, kit, kits, reliefs, overhang, &mut drawn, door, window);
            continue;
        }
        let bark = reliefs.of(kit, Material::Log);
        let mut s = Surfaces::default();
        for piece in laid_pieces {
            match piece {
                Piece::Opening { .. } => {
                    if case.fitted {
                        today::openings(piece, door, window, &mut drawn);
                    }
                }
                Piece::Dressed { .. } if !profiles::is_cap(case, kit, piece) => {}
                _ => terra_build::draw::draw(piece, kit, bark, &mut s),
            }
        }
        layers::shade_rest(case, kit, Material::Covering, &mut s);
        add_surfaces(&mut drawn, kit, s);
        let b = layers::blocks(case, kit, laid_pieces);
        // Tops and undersides, and a gable's cubes whole, as the surface; side faces are
        // the modules.
        let cut = |_: IVec3, p: Vec3| profiles::under_copings(&b.features, p);
        let gables = &b.gables;
        let faces = |cube: IVec3, axis: usize, _: f32| {
            axis == 1 || gables.contains_key(&cube.to_array())
        };
        let s = layers::draw_blocks(
            case,
            kit,
            &b,
            reliefs,
            Mode::Pieces { field_inset: 0.0 },
            true,
            &cut,
            &faces,
        );
        add_surfaces(&mut drawn, kit, s);
        let metres = kit
            .textures
            .get(&Material::Dressed)
            .map_or([1.2, 1.2], |t| t.metres);
        let ground = case.ground;
        let ground_fn = |x: f32, z: f32| ground(x, z);
        let n = Neighbours {
            base: &case.base,
            kind: kit.kind(),
            ground: &ground_fn,
        };
        let solid_same = |at: IVec3| {
            case.base.cube(at).is_some_and(|c| {
                c.kind == kit.kind() && c.fill == terra_build::Fill::Solid
            })
        };
        let mut tiles = Surface::default();
        let mut missing: BTreeMap<String, usize> = BTreeMap::new();
        for &at in &b.cubes {
            if gables.contains_key(&at) {
                continue;
            }
            let cube = IVec3::from_array(at);
            let roles = n.roles(cube);
            for (axis, side) in [(0usize, 1i32), (0, -1), (2, 1), (2, -1)] {
                let beyond = roles.faces[face_index(axis, side)];
                if !beyond.open() {
                    continue;
                }
                let mut normal = Vec3::ZERO;
                normal[axis] = side as f32;
                let right = Vec3::Y.cross(normal);
                let role = |other: (usize, i32)| roles.edges[edge_between((axis, side), other)];
                let top = match role((1, 1)) {
                    Edge::Crest => "coping",
                    Edge::Sill => "sill",
                    _ => "none",
                };
                let bottom = match role((1, -1)) {
                    Edge::Foot => "foot",
                    Edge::Head => "head",
                    Edge::Soffit => "fascia",
                    _ => "none",
                };
                let banded = top != "none" || bottom != "none";
                let side_of = |dir: Vec3| -> &'static str {
                    match role(axis_side(dir)) {
                        Edge::Arris | Edge::Jamb => "arris",
                        _ => {
                            let d = dir.as_ivec3();
                            if banded
                                && solid_same(cube + d)
                                && solid_same(cube + d + normal.as_ivec3())
                            {
                                "inside"
                            } else {
                                "plain"
                            }
                        }
                    }
                };
                let (left, right_role) = (side_of(-right), side_of(right));
                let parity = if axis == 0 { 0 } else { 1 };
                let plain = !banded && left == "plain" && right_role == "plain";
                let variant = if plain {
                    (layers::noise(&[cube.x, cube.y, cube.z, axis as i32 * 2 + side]) * 2.0)
                        as i32
                        % 2
                } else {
                    0
                };
                let name = format!("tile_{top}_{bottom}_{left}_{right_role}_{parity}_{variant}");
                let Some(model) = set.tiles.get(&name) else {
                    *missing.entry(name).or_default() += 1;
                    continue;
                };
                let centre = (cube.as_vec3() + Vec3::splat(0.5)) * CELL_M + normal * (CELL_M * 0.5);
                let origin = centre - right * (CELL_M * 0.5) - Vec3::Y * (CELL_M * 0.5);
                let rotation = Quat::from_mat3(&Mat3::from_cols(right, Vec3::Y, normal));
                let mut s = Surface::default();
                model.place((origin, rotation, Vec3::ONE), &mut s);
                profiles::texture(&mut s, metres, 0, &b.features, ground);
                crate::add_to(&mut tiles, s);
                let _ = beyond == Beyond::Hole;
            }
        }
        if !missing.is_empty() {
            eprintln!("{}: tiles missing {missing:?}", case.name);
        }
        // Modules are built only to the overhanging profiles (D192 as amended).
        let _ = overhang;
        add(&mut drawn, folder(kit, Material::Dressed), tiles);
    }
    drawn
}
