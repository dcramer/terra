//! What no kit draws, plain by colour.

use super::*;

/// What no kit draws, plain by colour: kept to the triangles in the section, and the
/// plan's planned plain cubes added to its ghost shell. Only worked out when the section
/// has something of a kind no kit draws.
pub(super) fn plain(section: &Section, drawn: &mut Drawn) {
    let (base, kits) = (section.base, section.kits);
    let origin = section.at * terra_build::cubes::SECTION;
    let scope = layout::Scope {
        low: origin - IVec3::ONE,
        high: origin + IVec3::splat(terra_build::cubes::SECTION),
    };
    let kitless = |kind: Kind| kits.of(kind).is_none();
    let any = scope
        .cubes(base, None)
        .iter()
        .any(|(_, cube)| kitless(cube.kind));
    if !any {
        return;
    }
    let keep = |surface: &Surface| {
        let mut out = Surface::default();
        for i in (0..surface.positions.len()).step_by(3) {
            let middle =
                (surface.positions[i] + surface.positions[i + 1] + surface.positions[i + 2]) / 3.0;
            if terra_build::cubes::section_of((middle / CELL_M).floor().as_ivec3()) != section.at {
                continue;
            }
            out.positions.extend(&surface.positions[i..i + 3]);
            out.normals.extend(&surface.normals[i..i + 3]);
            out.colours.extend(&surface.colours[i..i + 3]);
            if surface.uvs.len() == surface.positions.len() {
                out.uvs.extend(&surface.uvs[i..i + 3]);
            }
        }
        out
    };
    let (shown, planned) = match section.progress {
        Progress::Cubes { .. } => (
            base.view(|cube| cube.is_in() || cube.fill == Fill::Open),
            Some(base.view(|cube| !cube.is_in() && cube.fill != Fill::Open)),
        ),
        _ => (base.clone(), None),
    };
    drawn.plain = keep(&crate::mesh::mesh_for_kits(&shown, kits));
    if let Some(planned) = planned {
        drawn
            .shell
            .append(keep(&crate::mesh::mesh_for_kits(&planned, kits)));
    }
}
