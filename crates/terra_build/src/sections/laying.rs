//! A base laid whole and cut into its sections ([`Whole`]), and a section's pieces with how
//! far each is in ([`lay`]).

use super::*;

impl Whole {
    /// Whether it was laid by these kits, in this order, so its pieces' kits are theirs.
    pub fn laid_by(&self, kits: &Kits) -> bool {
        self.laid_by.len() == kits.0.len()
            && self
                .laid_by
                .iter()
                .zip(&kits.0)
                .all(|(kind, kit)| *kind == kit.kind())
    }

    /// About how many bytes its pieces take in memory.
    pub fn bytes(&self) -> usize {
        let filed = |by_section: &BTreeMap<[i32; 3], std::sync::Arc<Vec<Filed>>>| {
            let pieces: usize = by_section.values().map(|pieces| pieces.len()).sum();
            pieces * size_of::<(usize, usize, Piece)>() + by_section.len() * 64
        };
        filed(&self.by_section)
            + self
                .standing
                .as_ref()
                .map_or(0, |standing| filed(&standing.by_section))
    }

    /// Lays every kit's pieces of a base: of its plan, and where anything is not yet
    /// built, of what stands of it too.
    pub fn lay(base: &Base, kits: &Kits) -> Whole {
        let mut whole = layout::roofs::remembered(|| Self::lay_all(base, kits));
        if base.store().planned() > 0 {
            let view = base.what_stands();
            let by_section = layout::roofs::remembered(|| Self::lay_all(&view, kits)).by_section;
            whole.standing = Some(Standing {
                base: std::sync::Arc::new(view),
                by_section,
                reached: BTreeSet::new(),
            });
        }
        whole
    }

    /// The sections what stands of the base was laid again in by the edit this was laid
    /// again after (D358): beyond those its plan's `reach` names, a column first built in
    /// turning the logs along the floor from it (`enclosure_moved`), so they are drawn
    /// again too.
    pub fn standing_reached(&self) -> impl Iterator<Item = [i32; 3]> + '_ {
        self.standing
            .iter()
            .flat_map(|standing| standing.reached.iter().copied())
    }

    pub(super) fn lay_all(base: &Base, kits: &Kits) -> Whole {
        let mut whole = Whole {
            laid_by: kits.0.iter().map(Kit::kind).collect(),
            ..Whole::default()
        };
        // A kit lays only its own kind's cubes: one whose kind the base has none of lays
        // nothing, and is not asked, so a hundred materials cost a base what its own do.
        let kinds = kinds_of(base);
        for (index, kit) in kits.0.iter().enumerate() {
            if !kinds.contains(&kit.kind()) {
                whole.counts.push(0);
                continue;
            }
            let pieces = layout::lay_among(base, kit, kits).pieces;
            whole.counts.push(pieces.len());
            for (order, piece) in pieces.into_iter().enumerate() {
                std::sync::Arc::make_mut(
                    whole
                        .by_section
                        .entry(piece.section().to_array())
                        .or_default(),
                )
                .push((index, order, piece));
            }
        }
        whole
    }

    /// This base laid again after edits that marked some sections (`Base::take_dirty`),
    /// from `before` as it was laid: only the buildings the marks are in, before the edits
    /// and after (an edit may join two or split one), are laid again, and every other
    /// building's sections kept as they were, as a claim may hold many. What a section is
    /// cut from it is what laying the base whole gives. Where its kits' pieces come in
    /// the order the whole base goes in is not kept, so a plan shown so far through
    /// (`Progress::Fraction`) is laid whole.
    pub fn lay_again(
        &self,
        before: &Base,
        base: &Base,
        kits: &Kits,
        marked: &BTreeSet<[i32; 3]>,
    ) -> Whole {
        // Laid by other kits, read again since: its pieces' kits are not these, so the base
        // is laid whole.
        if !self.laid_by(kits) {
            return Whole::lay(base, kits);
        }
        // The plan, unless all the edit did was fill cubes further in, which no piece of a
        // plan is laid by (its ghosts are drawn by how far each cube is in).
        let mut whole = if only_filled(before, base, marked) {
            Whole {
                standing: None,
                ..self.clone()
            }
        } else {
            layout::roofs::remembered(|| self.lay_part(before, base, kits, marked))
        };
        if base.store().planned() == 0 {
            return whole;
        }
        // What stands, laid again from what stood only where it changed: as the plan's
        // pieces where nothing was planned before.
        let (was, was_by_section) = match &self.standing {
            Some(standing) => (standing.base.clone(), standing.by_section.clone()),
            None => (
                std::sync::Arc::new(before.what_stands()),
                self.by_section.clone(),
            ),
        };
        let (view, changed) = base.what_stands_again(&was, marked);
        let (by_section, reached) =
            if changed || was.ground() != view.ground() || was.props_differ(&view) {
                let stood = Whole {
                    by_section: was_by_section,
                    counts: self.counts.clone(),
                    laid_by: self.laid_by.clone(),
                    standing: None,
                };
                let groups = reach(&was, &view, kits, marked);
                let laid = layout::roofs::remembered(|| {
                    stood.lay_groups(&was, &view, kits, marked, &groups)
                });
                (laid.by_section, groups.into_iter().flatten().collect())
            } else {
                (was_by_section, BTreeSet::new())
            };
        whole.standing = Some(Standing {
            base: std::sync::Arc::new(view),
            by_section,
            reached,
        });
        whole
    }

    pub(super) fn lay_part(
        &self,
        before: &Base,
        base: &Base,
        kits: &Kits,
        marked: &BTreeSet<[i32; 3]>,
    ) -> Whole {
        let groups = reach(before, base, kits, marked);
        self.lay_groups(before, base, kits, marked, &groups)
    }

    /// As [`Whole::lay_part`], the sections it reaches given.
    pub(super) fn lay_groups(
        &self,
        before: &Base,
        base: &Base,
        kits: &Kits,
        marked: &BTreeSet<[i32; 3]>,
        groups: &[BTreeSet<[i32; 3]>],
    ) -> Whole {
        // The first group, round the marks, is laid again in every kit; the rest are rows
        // of logs along the floor from them, which only a log kit's courses reach: every
        // other kit's pieces there are as they were, laid from cubes that did not change.
        // A roof's pieces are laid from its own cubes and those a step or two round them
        // (what is under its eaves, what stands over it), so a roof kit lays nothing again
        // where no cube changed within a few of a roof's.
        let roofed = roof_near_changes(before, base, marked);
        let laid_in = |group: usize, kit: &Kit| match kit.pattern {
            Pattern::Roof => group == 0 && roofed,
            Pattern::Logs => true,
            _ => group == 0,
        };
        let again: BTreeMap<[i32; 3], usize> = groups
            .iter()
            .enumerate()
            .flat_map(|(group, sections)| sections.iter().map(move |&at| (at, group)))
            .collect();
        let mut whole = Whole {
            by_section: self
                .by_section
                .iter()
                .map(|(at, pieces)| {
                    let kept = match again.get(at) {
                        None => pieces.clone(),
                        Some(&group) => std::sync::Arc::new(
                            pieces
                                .iter()
                                .filter(|(index, _, _)| !laid_in(group, &kits.0[*index]))
                                .cloned()
                                .collect(),
                        ),
                    };
                    (*at, kept)
                })
                .filter(|(_, pieces)| !pieces.is_empty())
                .collect(),
            counts: self.counts.clone(),
            laid_by: self.laid_by.clone(),
            standing: None,
        };
        // A group at a time, each a box no bigger than a piece's reach round the edit.
        let kinds = kinds_of(base);
        for (group, sections) in groups.iter().enumerate() {
            for (index, kit) in kits.0.iter().enumerate() {
                if !laid_in(group, kit) || !kinds.contains(&kit.kind()) {
                    continue;
                }
                let pieces = layout::lay_sections(base, kit, kits, sections).pieces;
                for (order, piece) in pieces.into_iter().enumerate() {
                    std::sync::Arc::make_mut(
                        whole
                            .by_section
                            .entry(piece.section().to_array())
                            .or_default(),
                    )
                    .push((index, order, piece));
                }
            }
        }
        whole
    }
}

/// Every kind a base has a cube of.
pub(super) fn kinds_of(base: &Base) -> BTreeSet<Kind> {
    base.kinds().collect()
}

/// Runs the work of one base's section, every roof it asks for worked out once
/// ([`layout::roofs::Roofs::whole`]): a section's parts' keys, its roofs' masks and how they
/// were drawn ask for the same.
pub fn remembered<T>(run: impl FnOnce() -> T) -> T {
    layout::roofs::remembered(run)
}

/// A section's pieces, cut from its base laid whole, with how far each is in: its own,
/// and a cube beyond its edges, as `layout::lay_section_and_edge` lays them.
pub fn lay(section: &Section, whole: &Whole) -> Laid {
    let (base, kits) = (section.base, section.kits);
    let origin = section.at * crate::cubes::SECTION;
    // Only the kits whose kinds are there, in its cubes or round them.
    let near = layout::Scope {
        low: origin - IVec3::splat(KINDS_REACH),
        high: origin + IVec3::splat(crate::cubes::SECTION - 1 + KINDS_REACH),
    };
    let kinds: BTreeSet<Kind> = near
        .cubes(base, None)
        .into_iter()
        .map(|(_, cube)| cube.kind)
        .collect();
    let present: Vec<bool> = kits
        .0
        .iter()
        .map(|kit| kinds.contains(&kit.kind()))
        .collect();
    // Each of the whole's kits as one of these, by its kind: where the kits were read again
    // since it was laid, a kit taken out lays nothing and one added nothing until the base
    // is laid again, which a change of kits asks for.
    let kit_of: Vec<Option<usize>> = whole
        .laid_by
        .iter()
        .map(|kind| kits.0.iter().position(|kit| kit.kind() == *kind))
        .collect();
    let low = origin - IVec3::ONE;
    let high = origin + IVec3::splat(crate::cubes::SECTION);
    // So far through, one piece of the base is next, as a builder has one in hand: the
    // lowest of each kit's next, so a plan shows one lit piece at its foot, not one a kit
    // lying about it.
    let next_kit = match section.progress {
        Progress::Fraction(fraction) => whole
            .by_section
            .values()
            .flat_map(|pieces| pieces.iter())
            .filter(|(index, order, _)| *order == done(whole.counts[*index], *fraction))
            .min_by_key(|(index, _, piece)| (piece.anchor().y, *index))
            .map(|(index, _, _)| *index),
        _ => None,
    };
    let stage = |index: usize, order: usize, piece: &Piece| match section.progress {
        // In the order the whole base goes in.
        Progress::Fraction(fraction) => {
            let done = done(whole.counts[index], *fraction);
            match order.cmp(&done) {
                std::cmp::Ordering::Less => Stage::In,
                std::cmp::Ordering::Equal if next_kit == Some(index) => Stage::Next,
                _ => Stage::Ghost,
            }
        }
        Progress::Cubes { next } => stage_by_cubes(base, piece, next),
        Progress::Whole => Stage::In,
    };
    // Its own, then those of the sections round it a cube past its edges: a block kind's
    // surface needs them to round its edge as its neighbour does.
    let mut own: Vec<(usize, usize, Placed)> = Vec::new();
    let mut edge: Vec<(usize, usize, Placed)> = Vec::new();
    let fraction = matches!(section.progress, Progress::Fraction(_));
    // Built as far as each cube is in, with a plan in the base (D358): what is in from
    // what stands, drawn as finished from it, and the rest from the plan, as ghosts and
    // what goes in next. Otherwise every piece from the plan.
    // What stands is staged by what stands: a piece's end against a cube only planned does
    // not wait for that cube.
    type Source<'a> = (
        &'a BTreeMap<[i32; 3], std::sync::Arc<Vec<Filed>>>,
        Option<bool>,
        &'a Base,
    );
    let sources: Vec<Source> = match (&whole.standing, section.progress) {
        (Some(standing), Progress::Cubes { .. }) => vec![
            (&standing.by_section, Some(true), &*standing.base),
            (&whole.by_section, Some(false), base),
        ],
        _ => vec![(&whole.by_section, None, base)],
    };
    for (by_section, wanted_in, staged_by) in sources {
        for offset in NEIGHBOURS.iter().chain([&IVec3::ZERO]) {
            let at = section.at + *offset;
            let Some(pieces) = by_section.get(&at.to_array()) else {
                continue;
            };
            for (laid, order, piece) in pieces.iter() {
                let Some(index) = kit_of.get(*laid).copied().flatten() else {
                    continue;
                };
                if !present[index] {
                    continue;
                }
                let staged = match section.progress {
                    Progress::Cubes { next } => stage_by_cubes(staged_by, piece, next),
                    _ => stage(*laid, *order, piece),
                };
                if wanted_in.is_some_and(|wanted| wanted != (staged == Stage::In)) {
                    continue;
                }
                let placed = |piece: &Piece| (index, *order, (index, piece.clone(), staged));
                if at == section.at {
                    own.push(placed(piece));
                } else if ((fraction || matches!(piece, Piece::Block { .. } | Piece::Gable { .. }))
                // Blocks round the edge, and the lintels and sills bedded in them, whose
                // beds reach a little past their cubes (`dress::Bed`), a cube further: the
                // cubes round the edge are drawn as finely as a bed near them asks (D284).
                && piece.anchor().cmpge(low).all()
                && piece.anchor().cmple(high).all())
                    || (matches!(
                        piece,
                        Piece::Trim {
                            role: layout::Role::Head | layout::Role::Sill,
                            ..
                        }
                    ) && piece.anchor().cmpge(low - IVec3::ONE).all()
                        && piece.anchor().cmple(high + IVec3::ONE).all())
                {
                    edge.push(placed(piece));
                }
            }
        }
    }
    // Kit by kit: shown so far through, each in the order its pieces go in; otherwise in
    // an order of the pieces themselves, so a section is laid the same whether its base
    // was laid whole or only as far as an edit reached (`Whole::lay_again`), whose
    // pieces come in another order.
    if fraction {
        own.sort_by_key(|(index, order, _)| (*index, *order));
        edge.sort_by_key(|(index, order, _)| (*index, *order));
    } else {
        let key = |(index, _, placed): &(usize, usize, Placed)| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            placed.1.hash(&mut hasher);
            (*index, hasher.finish())
        };
        own.sort_by_cached_key(key);
        edge.sort_by_cached_key(key);
    }
    // Its damaged cubes, and those a cube round it whose wear reaches over its edge.
    let worn = worn_within(base, kits, low, high).into_iter().collect();
    // And those a fire has blackened (D377), as far round.
    let scorched = base.scorched_within(low, high).collect();
    Laid {
        pieces: own.into_iter().map(|(_, _, placed)| placed).collect(),
        edge: edge.into_iter().map(|(_, _, placed)| placed).collect(),
        worn,
        scorched,
    }
}

/// The damaged cubes from one cube to another, each with how many quarters of its health
/// it has lost.
pub(super) fn worn_within(
    base: &Base,
    kits: &Kits,
    low: IVec3,
    high: IVec3,
) -> BTreeMap<[i32; 3], u8> {
    base.damaged()
        .filter(|(at, _)| at.cmpge(low).all() && at.cmple(high).all())
        .filter_map(|(at, damage)| {
            let kind = base.cube(at)?.kind;
            let toughness = kits
                .of(kind)
                .map(|kit| kit.toughness.rules())
                .unwrap_or_default();
            Some((at.to_array(), toughness.quarters(damage)))
        })
        .collect()
}

/// How many of so many pieces are in, so far through.
pub fn done(count: usize, fraction: f32) -> usize {
    ((count as f32 * fraction.clamp(0.0, 1.0)) as usize).min(count)
}

/// What a walker meets of a base shown so far through: the cubes of the pieces in.
pub fn standing(base: &Base, kits: &Kits, fraction: f32) -> Base {
    let laid: Vec<Vec<Piece>> = kits
        .0
        .iter()
        .map(|kit| layout::lay_among(base, kit, kits).pieces)
        .collect();
    base.standing(
        laid.iter()
            .flat_map(|pieces| pieces[..done(pieces.len(), fraction)].iter()),
    )
}

/// How far a piece is in on a base built by plans: by the cubes it stands in, or its roof
/// cube's stage; lit where it stands in a cube in `next`, or is its roof cube's next stage.
pub fn stage_by_cubes(base: &Base, piece: &Piece, next: &BTreeSet<[i32; 3]>) -> Stage {
    if let Some(cell) = piece.roof_cell() {
        let stage = base
            .cube(cell.cube)
            .map_or(0, |cube| layout::roofs::stage_of(&cube));
        return if stage >= cell.stage {
            Stage::In
        } else if stage + 1 == cell.stage && next.contains(&cell.cube.to_array()) {
            Stage::Next
        } else {
            Stage::Ghost
        };
    }
    let (is_in, cubes) = piece_in(base, piece);
    if is_in {
        Stage::In
    } else if cubes.iter().any(|at| next.contains(at)) {
        Stage::Next
    } else {
        Stage::Ghost
    }
}

/// How far into a cube's height a piece must reach for it to count as standing in it.
pub(super) const SLIVER_M: f32 = 0.05;

/// Whether a piece is in, by the cubes it stands in, and those cubes: a cube must be
/// filled up to the middle of the part of the piece within it, so a log is in once the
/// fill reaches its middle.
pub(super) fn piece_in(base: &Base, piece: &Piece) -> (bool, Vec<[i32; 3]>) {
    let point_cube = |point: Vec3| (point / CELL_M).floor().as_ivec3();
    // A wall carried down into a roof cube it stands on goes in with it, not the roof, and
    // one the walls over and under fill with the one under (D343).
    match *piece {
        Piece::Gable {
            cube,
            carried: layout::Carried::Down,
            ..
        } => {
            return piece_in(
                base,
                &Piece::Block {
                    cube: cube + IVec3::Y,
                },
            );
        }
        Piece::Gable {
            cube,
            carried: layout::Carried::Through,
            ..
        } => {
            return piece_in(
                base,
                &Piece::Block {
                    cube: cube - IVec3::Y,
                },
            );
        }
        Piece::Curb { max, .. } => {
            let at = point_cube(max.metres() + Vec3::new(-0.01, 0.01, -0.01));
            return piece_in(base, &Piece::Block { cube: at });
        }
        _ => {}
    }
    // A framed kind's core and frame go in with a cube's first step, before its infill is
    // finished (D236): the wattle woven on the frame, then daubed.
    if let Piece::Wattle { cube } = *piece {
        let is_in = base
            .cube(cube)
            .is_some_and(|c| c.level() > 0 && c.fill == Fill::Solid);
        return (is_in, vec![cube.to_array()]);
    }
    let first_step = matches!(piece, Piece::Beam { .. });
    // An upright through several cubes, a post up an opening's side or a corner, is set
    // up with the first of them, as a builder stands a door's posts before the courses
    // reach their tops: the courses beside it end against it all the way up.
    let upright = matches!(piece, Piece::Log { from, to, .. }
        if from.0[0] == to.0[0] && from.0[2] == to.0[2] && from.0[1] != to.0[1]);
    let Some((min, max)) = piece.bounds() else {
        return (true, Vec::new());
    };
    let low = ((min + SLIVER_M) / CELL_M).floor().as_ivec3();
    let mut high = ((max - SLIVER_M) / CELL_M).floor().as_ivec3();
    if upright {
        high.y = low.y;
    }
    let mut cubes = Vec::new();
    let mut is_in = true;
    for x in low.x..=high.x {
        for y in low.y..=high.y {
            for z in low.z..=high.z {
                let at = IVec3::new(x, y, z);
                let bottom = y as f32 * CELL_M;
                let (from, to) = (min.y.max(bottom), max.y.min(bottom + CELL_M));
                let middle = ((from + to) * 0.5 - bottom) / CELL_M;
                let needed = if first_step {
                    1
                } else {
                    (middle * 16.0).ceil().clamp(1.0, 16.0) as u8
                };
                // Only the cubes it stands in count: not air its rough edge reaches
                // into, nor an opening it frames, nor one being carved.
                let Some(cube) = base
                    .cube(at)
                    .filter(|cube| layout::drawn_fill(cube) != Fill::Open)
                else {
                    continue;
                };
                // A roof's cube is filled by stages, not by height: a wall carried up into
                // it, a gable's courses, is in with its first.
                is_in &= if cube.kind.is_roof() {
                    cube.is_in()
                } else {
                    cube.level() >= needed
                };
                cubes.push(at.to_array());
            }
        }
    }
    if cubes.is_empty() {
        // Flat, as a floor's boards on its top, or over a wall's top, as a gable is: in
        // with the first cube under it.
        let mut at = point_cube((min + max) * 0.5 - Vec3::Y * 0.01);
        for _ in 0..crate::STOREY_CUBES * 2 {
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
