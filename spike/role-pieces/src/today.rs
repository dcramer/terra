//! Candidate 1, today's look: what the game draws from the same cubes (`terra_game`'s
//! `build/draw.rs`), every kit laid by `layout::lay_among`, timber and roofs by
//! `terra_build::draw`, block kinds by `terra_build::surface`, and the door and window
//! models in their holes. The other candidates start from parts of it.

use crate::model::Model;
use crate::shapes::Case;
use crate::{Drawn, Reliefs, add_surfaces};
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use terra_build::kit::{Kit, Kits, Pattern};
use terra_build::layout::{self, Piece};
use terra_build::surface::{Heights, Look};
use terra_build::{Base, Fill, Kind, Material, Surface, Surfaces};

/// How finely a block kind's cube faces are divided, as the game does.
pub const SURFACE_STEPS: u32 = 10;
/// How far a stone floor's flagstones stand out, as a fraction of its walls' stone.
pub const FLOOR_RELIEF: f32 = 0.3;

/// What a kit's laying gives, kept so candidates can draw it otherwise.
pub struct Laid {
    pub kit: Kit,
    pub pieces: Vec<Piece>,
}

pub fn lay(case: &Case, kits: &Kits) -> Vec<Laid> {
    kits.0
        .iter()
        .map(|kit| Laid {
            kit: kit.clone(),
            pieces: layout::lay_among(&case.base, kit, kits).pieces,
        })
        .collect()
}

/// A block kind's look, as the game makes it from its kit.
pub fn block_look(kit: &Kit) -> Look {
    let metres = |material: Material| kit.textures.get(&material).map(|t| t.metres);
    Look {
        bevel: kit.bevel,
        relief: kit.relief,
        steps: SURFACE_STEPS,
        metres: metres(Material::Masonry).unwrap_or([2.0, 2.0]),
        top: metres(Material::Floor).map(|metres| (metres, kit.relief * FLOOR_RELIEF)),
    }
}

/// The cubes of a block kind laid as blocks, and its gables' cuts.
pub fn blocks_of(pieces: &[Piece]) -> (BTreeSet<[i32; 3]>, BTreeMap<[i32; 3], [[i32; 3]; 3]>) {
    let mut cubes = BTreeSet::new();
    let mut gables = BTreeMap::new();
    for piece in pieces {
        match piece {
            Piece::Block { cube } => {
                cubes.insert(cube.to_array());
            }
            Piece::Gable { cube, under } => {
                cubes.insert(cube.to_array());
                gables.insert(cube.to_array(), *under);
            }
            _ => {}
        }
    }
    (cubes, gables)
}

/// What hides a block kind's face, as the game has it: floors, and walls drawn as boxes,
/// of other kinds; walls of other kits do not, being thinner than the cube.
pub fn hides_for(base: &Base, kind: Kind) -> impl Fn(IVec3) -> bool + '_ {
    move |at: IVec3| {
        base.cube(at).is_some_and(|cube| {
            cube.fill != Fill::Open
                && cube.kind != kind
                && (!base.in_wall(at) || cube.kind == Kind::PLAIN)
        })
    }
}

/// Draws the case as the game does today.
pub fn draw(case: &Case, kits: &Kits, reliefs: &Reliefs, door: &Model, window: &Model) -> Drawn {
    let laid = lay(case, kits);
    let mut drawn = Drawn::default();
    plain(case, kits, &mut drawn);
    for Laid { kit, pieces } in &laid {
        let bark = reliefs.of(kit, Material::Log);
        for piece in pieces {
            if let Piece::Opening { .. } = piece {
                if case.fitted {
                    openings(piece, door, window, &mut drawn);
                }
                continue;
            }
            let mut s = Surfaces::default();
            terra_build::draw::draw(piece, kit, bark, &mut s);
            add_surfaces(&mut drawn, kit, s);
        }
        if kit.pattern == Pattern::Masonry {
            let s = blocks(case, kit, pieces, reliefs);
            add_surfaces(&mut drawn, kit, s);
        }
    }
    drawn
}

/// Floors and flat roofs no kit lays, in plain colour, as the game draws them.
pub fn plain(case: &Case, kits: &Kits, drawn: &mut Drawn) {
    let mesh = case.base.mesh_for_kits(kits);
    crate::add(drawn, "plain".into(), mesh.structure);
    let first = &kits.0[0];
    add_surfaces(drawn, first, mesh.textured);
}

/// A block kind's cubes as its surface, as the game draws them.
pub fn blocks(case: &Case, kit: &Kit, pieces: &[Piece], reliefs: &Reliefs) -> Surfaces {
    let (cubes, gables) = blocks_of(pieces);
    let cut = |cube: IVec3, at: Vec3| {
        gables
            .get(&cube.to_array())
            .map_or(f32::MAX, |under| layout::roofs::gable_cut(cube, under, at))
    };
    let look = block_look(kit);
    let side = reliefs.of(kit, Material::Masonry);
    let top = reliefs.of(kit, Material::Floor);
    let sides = |uv: [f32; 2]| side.map_or(0.5, |r| r.at(uv));
    let tops = |uv: [f32; 2]| top.map_or(0.5, |r| r.at(uv));
    let heights = Heights {
        sides: &sides,
        top: &tops,
    };
    let hides = hides_for(&case.base, kit.kind());
    let mut out = Surfaces::default();
    let mut faces = Surface::default();
    let mut top_faces = Surface::default();
    terra_build::surface::draw_cut(
        &cubes,
        &hides,
        &look,
        &heights,
        &cut,
        [1.0; 3],
        &mut faces,
        &mut top_faces,
    );
    *out.of(Material::Masonry) = faces;
    *out.of(Material::Floor) = top_faces;
    out
}

/// A door or window model fitted to its opening's box, as the game fits it.
pub fn openings(piece: &Piece, door: &Model, window: &Model, drawn: &mut Drawn) {
    let Piece::Opening {
        min,
        max,
        along_x,
        door: is_door,
    } = *piece
    else {
        return;
    };
    let (min, max) = (min.metres(), max.metres());
    let m = if is_door { door } else { window };
    let x = if along_x { Vec3::X } else { Vec3::Z };
    let size = max - min;
    let (width, depth) = if along_x {
        (size.x, size.z)
    } else {
        (size.z, size.x)
    };
    let at = m.fill(
        (min + max) * 0.5,
        x,
        Vec3::Y,
        Vec3::new(width, size.y, depth),
    );
    m.place(at, drawn.entry(m.key.clone()).or_default());
}

