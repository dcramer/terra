//! A section drawn in parts, clusters of cubes each keyed by what its drawing reads, so a
//! change draws again only the parts whose key it changed and keeps the rest as drawn.

use super::*;

impl Drawing {
    /// How many faces and pieces it holds.
    pub fn len(&self) -> usize {
        self.faces.len() + self.pieces.values().map(|part| part.len()).sum::<usize>()
    }

    /// How many of its block surfaces' faces were taken from the last drawing.
    pub fn faces_taken(&self) -> usize {
        self.faces.taken()
    }

    /// How many block surfaces' faces it holds.
    pub fn faces(&self) -> usize {
        self.faces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A part's pieces drawn and masked, by kit, piece and the damage round it ([`harm_key`]).
pub(super) type PiecesDrawn =
    terra_gen::hash::QuickMap<(usize, Piece, u64), std::sync::Arc<Surfaces>>;

/// The damage a member is broken by, as a number (`timber::draw_harmed`): that of the cubes
/// round the box it fills, which a cube's spreads into (`loss::lost`); none for any other
/// piece, which damage does not change.
pub(super) fn harm_key(piece: &Piece, worn: &BTreeMap<[i32; 3], u8>) -> u64 {
    if worn.is_empty() || !matches!(piece, Piece::Log { .. }) {
        return 0;
    }
    let Some((low, high)) = piece.bounds() else {
        return 0;
    };
    let (low, high) = (
        (low / CELL_M).floor().as_ivec3() - IVec3::ONE,
        (high / CELL_M).floor().as_ivec3() + IVec3::ONE,
    );
    let mut key = 0;
    for (at, quarters) in worn.range(low.to_array()..=high.to_array()) {
        let at = IVec3::from_array(*at);
        if at.cmpge(low).all() && at.cmple(high).all() {
            key = layout::splitmix(
                key ^ u64::from(*quarters)
                    ^ ((at.x as u64 & 0xFFFF) << 8
                        | (at.y as u64 & 0xFFFF) << 24
                        | (at.z as u64 & 0xFFFF) << 40),
            );
        }
    }
    key
}

/// As [`draw`], taking what is unchanged from how the section was drawn last time, if
/// that is given, and keeping how it is drawn now in `keep`, if that is: the same surfaces
/// as drawing it afresh, to the bit.
pub fn draw_again(
    section: &Section,
    laid: &Laid,
    reach: Reach,
    last: Option<&Drawing>,
    keep: Option<&mut Drawing>,
) -> Drawn {
    let mut drawn = draw_parts(section, laid, reach, last, keep, None);
    // Its parts as one.
    for (by, into) in [
        (std::mem::take(&mut drawn.near_by), &mut drawn.near),
        (std::mem::take(&mut drawn.mid_by), &mut drawn.mid),
    ] {
        for part in by {
            for (key, surface) in part {
                into.entry(key).or_default().append(surface);
            }
        }
    }
    drawn
}

/// How far a section's parts are each way, in cubes: near and a few metres off, a section
/// is drawn a part at a time, so an edit puts on screen again only the parts it reached,
/// as a voxel game meshes small chunks.
pub const CLUSTER: i32 = 8;

/// How many parts a section has.
pub const CLUSTERS: usize = ((terra_build::cubes::SECTION / CLUSTER) as usize).pow(3);

/// Which part of its section a cube is in.
pub fn cluster_of(at: IVec3) -> usize {
    let local = (at - terra_build::cubes::section_of(at) * terra_build::cubes::SECTION) / CLUSTER;
    let side = terra_build::cubes::SECTION / CLUSTER;
    (local.x + side * (local.y + side * local.z)) as usize
}

/// A number for each part of a section ([`CLUSTER`]) near and a few metres off, for
/// everything its surfaces there are drawn from: the pieces of the section and round it
/// that reach within a few cubes of it, with how far in each is; every cube from a few
/// below it up over it, with a few round it, whose faces, shading and rain it takes; the
/// ground; and the roofs round, which a roof's masks follow. A part whose number is as it
/// was is drawn as it was, so the game leaves it on screen (`draw_parts`). None far off.
pub fn cluster_keys(section: &Section, laid: &Laid, reach: Reach) -> Vec<u64> {
    if reach <= Reach::Far {
        return Vec::new();
    }
    let origin = section.at * terra_build::cubes::SECTION;
    let roofs = std::cell::OnceCell::new();
    let roofs_round = || {
        *roofs.get_or_init(|| {
            layout::roofs::Roofs::whole(
                section.base,
                section.kits,
                layout::Scope {
                    low: origin - IVec3::ONE,
                    high: origin + IVec3::splat(terra_build::cubes::SECTION),
                },
            )
            .fingerprint()
        })
    };
    let roofing = |placed: &Placed| {
        section
            .kits
            .0
            .get(placed.0)
            .is_some_and(|kit| kit.pattern == Pattern::Roof)
    };
    let ground = section.base.ground();
    // How many cubes up a streak of rain runs down from, at most, in any kit.
    let streak_cubes = section
        .kits
        .0
        .iter()
        .map(|kit| libm::ceilf(kit.dress.masks.streak_reach / CELL_M) as i32)
        .max()
        .unwrap_or(0)
        .max(1);
    // How many cubes up the splash darkens a wall over its foot, at most, in any kit
    // (`dress::Rained`, at most 1.3 times the kit's splash): a wall's foot further below a
    // part than that leaves it dry wherever the foot is.
    let splash_cubes = section
        .kits
        .0
        .iter()
        .map(|kit| libm::ceilf(kit.dress.masks.splash * 1.3 / CELL_M) as i32)
        .max()
        .unwrap_or(0)
        + 1;
    let (size, side) = (CLUSTER, terra_build::cubes::SECTION / CLUSTER);
    // Each piece's box in cubes, worked out once.
    let boxes: Vec<(IVec3, IVec3, &Placed)> = laid
        .pieces
        .iter()
        .chain(&laid.edge)
        .map(|placed| {
            let (low, high) = placed.1.bounds().map_or_else(
                || (placed.1.anchor(), placed.1.anchor()),
                |(low, high)| {
                    // The cubes it overlaps: one ending on a cube's side is not in the next.
                    let low = (low / CELL_M).floor().as_ivec3();
                    (
                        low,
                        ((high / CELL_M).ceil().as_ivec3() - IVec3::ONE).max(low),
                    )
                },
            );
            (
                low.min(placed.1.anchor()),
                high.max(placed.1.anchor()),
                placed,
            )
        })
        .collect();
    // Each part's box grown by what reaches it, and its number begun: the pieces reaching
    // it hashed in their order, in one pass over the pieces for every part.
    struct Part {
        low: IVec3,
        high: IVec3,
        near_low: IVec3,
        near_high: IVec3,
        hasher: std::collections::hash_map::DefaultHasher,
        roofed: bool,
        reach_low: IVec3,
        reach_high: IVec3,
        /// The cubes its own faces and its own pieces' masks look at, level.
        looks_low: IVec3,
        looks_high: IVec3,
    }
    let mut parts: Vec<Part> = (0..(side * side * side))
        .map(|c| {
            let low = origin + IVec3::new(c % side, c / side % side, c / (side * side)) * size;
            let high = low + IVec3::splat(size - 1);
            let (near_low, near_high) = (
                low - IVec3::splat(KEY_REACH),
                high + IVec3::splat(KEY_REACH),
            );
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            reach.hash(&mut hasher);
            Part {
                low,
                high,
                near_low,
                near_high,
                hasher,
                roofed: false,
                reach_low: near_low,
                reach_high: near_high,
                looks_low: low - IVec3::ONE,
                looks_high: high + IVec3::ONE,
            }
        })
        .collect();
    for (piece_low, piece_high, placed) in &boxes {
        // A piece is drawn in the part its anchor is in, whatever it spans (`slot_of`), so
        // it is that part's. Beside that it shapes what is drawn of the cubes beside the
        // cubes it overlaps, no further, where it shapes anything: a block kind's cube the
        // faces of the cubes beside it, its surface rounded, faded and smoothed within a few
        // centimetres of where it is (`surface::tests`), and a lintel's or sill's bed the
        // stone it is set in (`dress::bed`). A coping's or a band's run, a roof's piece, is
        // nobody else's: a course laid along a wall redraws the parts it is drawn in, not
        // every part its run passes.
        let shapes_round = matches!(
            placed.1,
            Piece::Block { .. } | Piece::Gable { .. } | Piece::Wattle { .. }
        ) || section
            .kits
            .0
            .get(placed.0)
            .is_some_and(|kit| crate::dress::bed(&placed.1, kit).is_some());
        let anchor = placed.1.anchor();
        for part in parts.iter_mut() {
            let own = anchor.cmpge(part.low).all() && anchor.cmple(part.high).all();
            let beside = shapes_round
                && piece_low.cmple(part.high + IVec3::ONE).all()
                && piece_high.cmpge(part.low - IVec3::ONE).all();
            if own || beside {
                placed.hash(&mut part.hasher);
                part.roofed |= roofing(placed);
                (part.reach_low, part.reach_high) = (
                    part.reach_low.min(*piece_low),
                    part.reach_high.max(*piece_high),
                );
                // A piece drawn in the part is shaded by the rain over every cube it spans.
                if own {
                    (part.looks_low, part.looks_high) = (
                        part.looks_low.min(*piece_low - IVec3::ONE),
                        part.looks_high.max(*piece_high + IVec3::ONE),
                    );
                }
            }
        }
    }
    parts
        .into_iter()
        .map(|part| {
            let Part {
                near_low,
                near_high,
                mut hasher,
                roofed,
                reach_low,
                reach_high,
                looks_low,
                looks_high,
                ..
            } = part;
            // A roof's masks follow where on the roofs round each point is.
            if roofed {
                roofs_round().hash(&mut hasher);
            }
            // The ground under it and its pieces, which their dirt rises from.
            for x in reach_low.x..=reach_high.x {
                for z in reach_low.z..=reach_high.z {
                    ground.sample([x, z]).hash(&mut hasher);
                }
            }
            // The cubes it is drawn from, and those its masks look at: a cube round its own
            // and its own pieces' (a face is shaped by the cubes beside it, and the rain
            // marks a point by its own column), down as far as a wall's foot leaves it wet
            // (`splash_cubes`; a foot further down is dry above, wherever it is) and up to
            // where it streaks down from (an opening's foot or an open top within a streak's
            // reach, open for a storey over it); and over that only roofs, which shelter it
            // however high (`dress::sheltered`). A cube filled redraws the parts it is beside
            // and those whose rain it moves, not every part within a few cubes of it nor
            // every part two storeys over it.
            let (low, high) = (
                looks_low - IVec3::Y * splash_cubes,
                looks_high + IVec3::Y * (streak_cubes + terra_build::STOREY_CUBES),
            );
            for (at, cube) in section.base.store().within(low, high) {
                at.hash(&mut hasher);
                cube.word().hash(&mut hasher);
            }
            for (at, cube) in section.base.store().within(
                looks_low.with_y(high.y + 1),
                looks_high.with_y(i32::MAX / 4),
            ) {
                if cube.kind.is_roof() {
                    at.hash(&mut hasher);
                    cube.word().hash(&mut hasher);
                }
            }
            // And the damage its cubes have taken, kept beside them (D305), which wears it.
            for (at, quarters) in &laid.worn {
                let at = IVec3::from_array(*at);
                if at.cmpge(near_low).all() && at.cmple(near_high).all() {
                    (at, quarters).hash(&mut hasher);
                }
            }
            hasher.finish()
        })
        .collect()
}

/// How far round a part drawn alone its block kind's cubes are worked on, in cubes: a face
/// depends on the cubes three round it, and the faces a step beyond it, whose shading at
/// its edges it shares, on theirs.
pub(super) const PARTS_REACH: i32 = 5;

/// How many cubes round a part the damage its cubes' neighbours have taken, and the ground,
/// are taken into its key: the pieces and cubes themselves reach a cube (`cluster_keys`).
pub(super) const KEY_REACH: i32 = 3;

/// As [`draw_again`], its near and middle detail left a part at a time (`near_by`,
/// `mid_by`), as the game puts them on screen.
/// Its near and middle detail is drawn a part at a time if `only` is given, and only the
/// parts it says, as the game draws again only the parts of a base being built whose
/// [`cluster_keys`] changed; the rest are left empty, but what they were drawn from last
/// time is kept on in `keep`. Without it, all in one part (`near_by` and `mid_by` of one),
/// as a base not being built is drawn, with fewer meshes.
pub fn draw_parts(
    section: &Section,
    laid: &Laid,
    reach: Reach,
    last: Option<&Drawing>,
    keep: Option<&mut Drawing>,
    only: Option<&[bool]>,
) -> Drawn {
    // Several parts drawn again are drawn side by side, a thread each, as a terrain mesher
    // meshes its chunks: a storey's step draws every part of its section again after an
    // edit, each part works only on the cubes round it, and what they draw is the same as
    // drawn together.
    let mut keep = keep;
    let drawn = if let Some(only) = only
        && matches!(reach, Reach::Close | Reach::Near | Reach::Mid)
        && last.is_some()
        && only.iter().filter(|&&wanted| wanted).count() > 1
    {
        draw_side_by_side(section, laid, reach, last, keep.as_deref_mut(), only)
    } else {
        draw_together(section, laid, reach, last, keep.as_deref_mut(), only)
    };
    // The faces of the parts not drawn again carried on as they were, for the next edit.
    if let (Some(keep), Some(last), Some(only)) = (keep, last, only) {
        keep.faces.carry_on(&last.faces, &|cube| {
            terra_build::cubes::section_of(cube) == section.at
                && !only.get(cluster_of(cube)).copied().unwrap_or(true)
        });
    }
    drawn
}
