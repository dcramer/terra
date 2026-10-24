//! How fragments are drawn: their meshes, made once a kind and cut, and what comes off a
//! cube still standing; each drawn as blackened as what they came from was (D377): drawn clean, the
//! castle's burnt south-west tower turned pale the frame it came down (castle film r29,
//! 14.23 s, upper tower luma 79 to 110).

use super::*;

/// What came off the cubes harm hit and left standing (`Fallen::chipped`), into `made`, each
/// as blackened as its cube is, into `chars`.
pub(super) fn chipped_off(
    fallen: &Fallen,
    breaks: &dyn Fn(Kind) -> Breaks,
    share: f32,
    most: usize,
    made: &mut Vec<Fragment>,
    chars: &mut Vec<u8>,
) {
    // What came off a cube still standing: a few pieces a quarter of its health lost,
    // out of its open face, where its stones and shingles were.
    for (at, cube, from, to, out) in &fallen.chipped {
        let lost = f32::from(to.saturating_sub(*from)) / 4.0;
        let pieces = fragments::burst(
            &[(*at, *cube)],
            &|_| true,
            breaks,
            share * CHIPPED_SHARE * lost,
            most.saturating_sub(made.len()),
        );
        // Bits off its face, never a slab of a roof still standing (`fragments::burst`).
        let pieces: Vec<Fragment> = pieces
            .into_iter()
            .filter(|piece| piece.shape != Shape::Whole)
            .collect();
        chars.extend(std::iter::repeat_n(of_cube(fallen, *at), pieces.len()));
        made.extend(pieces.into_iter().map(|mut piece| {
            // Off its face, not inside it.
            let centre = (at.as_vec3() + 0.5) * CELL_M;
            let across = piece.at - centre;
            piece.at = centre + across - *out * across.dot(*out)
                + *out * (CELL_M * 0.5 + piece.size.max_element() * 0.5);
            piece.velocity += *out * 0.8;
            piece
        }));
    }
}

/// The mesh and material a kind's fragments of a cut are drawn with (`Fragment::mesh`, given
/// how often its kind's texture repeats), `char` blackened ([`Live::char`]), made the first
/// time.
#[allow(clippy::too_many_arguments)]
pub(super) fn mesh_for(
    shards: &mut Shards,
    look: &BuildLook,
    arrays: Option<&Arrays>,
    meshes: &mut Assets<Mesh>,
    kind: Kind,
    char: u8,
    cut_of: impl FnOnce([u8; 3]) -> Cut,
) -> Option<(Handle<Mesh>, Drawn)> {
    let kit = look.kits.as_deref().and_then(|kits| kits.of(kind));
    let cut = cut_of(kit.map_or([1; 3], fragments::repeat));
    if let Some(made) = shards.meshes.get(&(kind, cut, char)) {
        return made.clone();
    }
    let made = (|| {
        let kit = kit?;
        let surfaces: BTreeMap<_, _> = fragments::surfaces(kit, &cut)
            .0
            .into_iter()
            .map(|(material, surface)| ((kind, material), surface))
            .collect();
        let mut used = BTreeSet::new();
        let (in_arrays, rest) = draw::in_arrays(
            &surfaces,
            &look.layers,
            Vec3::ZERO,
            &BTreeMap::new(),
            &mut used,
        );
        if let (Some(mesh), Some(arrays)) = (in_arrays, arrays) {
            shards.layers.extend(used);
            let mesh = charred(mesh, char);
            return Some((meshes.add(mesh), Drawn::Arrays(arrays.material.clone())));
        }
        let (kind, material, mesh) = rest.into_iter().next()?;
        let own = look
            .materials
            .get(kind, material)
            .or_else(|| look.materials.get(Kind::PLAIN, material))?;
        Some((meshes.add(mesh), Drawn::Own(own.clone())))
    })();
    shards.meshes.insert((kind, cut, char), made.clone());
    made
}

/// How blackened each fragment of a fall is drawn ([`char_of`]).
pub(super) fn chars_of(made: &[Fragment], fallen: &Fallen) -> Vec<u8> {
    let cubes: BTreeSet<[i32; 3]> = if fallen.scorch.is_empty() {
        BTreeSet::new()
    } else {
        fallen.fell.iter().map(|(at, _)| at.to_array()).collect()
    };
    made.iter()
        .map(|fragment| char_of(fragment, &fallen.scorch, &cubes))
        .collect()
}

/// How blackened what comes off a cube still standing is drawn: as it is.
pub(super) fn of_cube(fallen: &Fallen, at: IVec3) -> u8 {
    fallen
        .scorch
        .get(&at.to_array())
        .map_or(0, |scorch| scorch.level.min(Scorch::LEVELS))
}

/// A fragment's mesh in the arrays drawn `char` blackened, as a section draws a scorched
/// cube's corners: its layer's id carries the char (`draw::in_arrays_scorched`).
pub(super) fn charred(mut mesh: Mesh, char: u8) -> Mesh {
    let share = Scorch { level: char }.share();
    if share > 0.0
        && let Some(VertexAttributeValues::Float32x2(ids)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_UV_1)
    {
        for id in ids {
            id[0] += terra_build::fire::CHAR_SHARE * share;
        }
    }
    mesh
}

/// How blackened a fragment is drawn, in [`Scorch`]'s levels: as the cubes it came from were
/// on the mean, those of `fell` its box covers where it is made (D377).
fn char_of(
    fragment: &Fragment,
    scorch: &BTreeMap<[i32; 3], Scorch>,
    fell: &BTreeSet<[i32; 3]>,
) -> u8 {
    if scorch.is_empty() {
        return 0;
    }
    let h = fragment.size * 0.5;
    let turn = fragment.turn;
    let half =
        (turn * Vec3::X * h.x).abs() + (turn * Vec3::Y * h.y).abs() + (turn * Vec3::Z * h.z).abs();
    let low = ((fragment.at - half) / CELL_M + 0.05).floor().as_ivec3();
    let high = ((fragment.at + half) / CELL_M - 0.05)
        .floor()
        .as_ivec3()
        .max(low);
    let (mut sum, mut count) = (0.0f32, 0.0f32);
    for x in low.x..=high.x {
        for y in low.y..=high.y {
            for z in low.z..=high.z {
                if fell.contains(&[x, y, z]) {
                    sum += scorch.get(&[x, y, z]).map_or(0.0, Scorch::share);
                    count += 1.0;
                }
            }
        }
    }
    (sum / count.max(1.0) * f32::from(Scorch::LEVELS)).round() as u8
}
