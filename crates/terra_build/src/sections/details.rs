//! A section's pieces drawn near and at middle distance: each kit's members, trims, roofs
//! and dress, and far off as boxes.

use super::*;

/// Draws a laid section at these details, finest first, as [`draw_again`] says.
pub(super) fn draw_details(
    section: &Section,
    laid: &Laid,
    details: &[Detail],
    last: Option<&Drawing>,
    keep: Option<&mut Drawing>,
    only: Option<&[bool]>,
) -> Drawn {
    let mut keep = keep;
    if let Some(keep) = keep.as_deref_mut() {
        keep.ground = Some(section.base.ground().clone());
    }
    let mut drawn = Drawn::default();
    let kits = section.kits;
    // The pieces drawn last time, kept masked, so to be taken only while the ground their
    // dirt rises from is as it was.
    // Where each point of its roofs is on them, for their masks, worked out once.
    let origin = section.at * crate::cubes::SECTION;
    let roofs = std::cell::OnceCell::new();
    let roofs_round = || {
        roofs.get_or_init(|| {
            layout::roofs::Roofs::whole(
                section.base,
                kits,
                layout::Scope {
                    low: origin - IVec3::ONE,
                    high: origin + IVec3::splat(crate::cubes::SECTION),
                },
            )
        })
    };
    // A roof's pieces are kept masked by where on the roofs they are: only while the roofs
    // round are as they were.
    let fingerprint = (last.is_some() || keep.is_some()).then(|| roofs_round().fingerprint());
    if let Some(keep) = keep.as_deref_mut() {
        keep.roofs = fingerprint;
    }
    let pieces_kept = last
        .filter(|last| {
            last.ground.as_ref() == Some(section.base.ground()) && last.roofs == fingerprint
        })
        .map(|last| &last.pieces);
    let finest = details[0];
    // Where its members have lost so much that chunks of them have gone (D305).
    let worn: BTreeMap<[i32; 3], u8> = laid.worn.iter().copied().collect();
    let harm = |p: Vec3, normal: Vec3| crate::loss::struck(&worn, p, normal);
    for &detail in details {
        let first = detail == finest && detail != Detail::Far;
        let harm = (!worn.is_empty() && matches!(detail, Detail::Near | Detail::Mid))
            .then_some(&harm as crate::timber::Harm);
        // Near and a few metres off, each part of the section ([`CLUSTER`]) apart, so the
        // game puts on screen again only the parts an edit reached; far off, one.
        let slots = if only.is_some() && matches!(detail, Detail::Near | Detail::Mid) {
            CLUSTERS
        } else {
            1
        };
        let slot_of = |at: IVec3| {
            if slots == 1 { 0 } else { cluster_of(at) }
        };
        // Whether a part is drawn this time.
        let wanted = |slot: usize| slots == 1 || only.is_none_or(|only| only[slot]);
        let mut built: Vec<BTreeMap<(Kind, Material), Surface>> = vec![BTreeMap::new(); slots];
        // Each part's pieces drawn this time, every kit's, to keep.
        let mut drawn_pieces: Vec<PiecesDrawn> = vec![PiecesDrawn::default(); slots];
        let kept_part =
            |slot: usize| pieces_kept.and_then(|parts| parts.get(&(detail, slots, slot)));
        let mut next = Surfaces::sharing();
        for (index, kit) in kits.0.iter().enumerate() {
            let own: Vec<&Placed> = laid.pieces.iter().filter(|p| p.0 == index).collect();
            let edge: Vec<&Placed> = laid.edge.iter().filter(|p| p.0 == index).collect();
            if own.is_empty() {
                continue;
            }
            let coarse = detail.kit(kit);
            // A roof's covering keeps its relief a few metres off too, where its swell is
            // what shows of it, and a block wall the stones its corner stones are cut from,
            // so they are the same near and a few metres off; walls' and logs' only near.
            let face = if detail == Detail::Near
                || (detail == Detail::Mid
                    && matches!(kit.pattern, Pattern::Roof | Pattern::Masonry))
            {
                section.maps.shape(kit)
            } else {
                None
            };
            // What is drawn in, masked (dirt from the ground, worn edges; D215) as it is
            // drawn, so a piece kept is kept masked; and what is drawn afresh, masked
            // together once all of it is.
            let mut mine: Vec<Surfaces> = (0..slots).map(|_| Surfaces::sharing()).collect();
            let mut fresh: Vec<Surfaces> = (0..slots).map(|_| Surfaces::sharing()).collect();
            let masked = |mut surfaces: Surfaces, stone: bool| {
                for (material, surface) in surfaces.0.iter_mut() {
                    if stone || *material != Material::Masonry {
                        crate::dress::mask(surface, kit, *material, section.base);
                    }
                    // What faces up under a roof, its sky hidden (D320).
                    if kit.pattern != Pattern::Roof && (stone || *material != Material::Masonry) {
                        crate::dress::sheltered(surface, roofs_round());
                    }
                    // A roof's own, by where on the roof each point is.
                    if matches!(material, Material::Covering | Material::Flashing) {
                        crate::dress::roof_mask(surface, kit, roofs_round());
                    }
                }
                surfaces
            };
            // Where pieces are made finely each is drawn and masked on its own, to be kept
            // if asked: the same whether kept or not, so drawing again draws the same.
            let kept = matches!(detail, Detail::Near | Detail::Mid);
            // The pieces drawn on their own, put together once all are, with room made at
            // once: a section's are many thousands.
            let mut made_pieces: Vec<(usize, std::sync::Arc<Surfaces>)> = Vec::new();
            for (count, (_, piece, stage)) in own.iter().enumerate() {
                if !detail.shows(piece) {
                    continue;
                }
                let slot = slot_of(piece.anchor());
                // What is drawn into the part, as against the plan's ghosts and the next
                // piece, which the section has whole.
                let in_part = *stage == Stage::In || (*stage == Stage::Next && !first);
                // Left as it is drawn: its part's pieces carried on whole below.
                if in_part && !wanted(slot) {
                    continue;
                }
                match stage {
                    Stage::In if far_box(piece, &coarse, detail, &mut fresh[slot]) => {}
                    Stage::In if kept => {
                        let key = (index, piece.clone(), harm_key(piece, &worn));
                        // Kept masked, so only while the ground its dirt rises from is as
                        // it was; but for stone, whose rain streaks hang from whatever is
                        // built above, which is masked afresh each time.
                        let made = match kept_part(slot).and_then(|part| part.get(&key)) {
                            Some(made) => made.clone(),
                            None => {
                                let mut made = Surfaces::sharing();
                                crate::draw::draw_harmed(
                                    piece, &coarse, face, false, harm, &mut made,
                                );
                                std::sync::Arc::new(masked(made, false))
                            }
                        };
                        made_pieces.push((slot, made.clone()));
                        if keep.is_some() {
                            drawn_pieces[slot].insert(key, made);
                        }
                    }
                    Stage::In => crate::draw::draw_harmed(
                        piece,
                        &coarse,
                        face,
                        detail == Detail::Far,
                        harm,
                        &mut fresh[slot],
                    ),
                    Stage::Next if first => {
                        crate::draw::draw_as(piece, &coarse, face, false, &mut next)
                    }
                    // Far off, what goes in next is drawn as though it were in.
                    Stage::Next => {
                        crate::draw::draw_as(piece, &coarse, None, true, &mut fresh[slot])
                    }
                    Stage::Ghost if first => {
                        drawn.ghosts.extend(ghost(piece, kit, count % 2 == 1));
                    }
                    Stage::Ghost => {}
                }
            }
            let mut room: BTreeMap<(usize, Material), [usize; 3]> = BTreeMap::new();
            for (slot, made) in &made_pieces {
                for (&material, surface) in &made.0 {
                    let room = room.entry((*slot, material)).or_default();
                    room[0] += surface.positions.len();
                    room[1] += surface.shared.positions.len();
                    room[2] += surface.shared.indices.len();
                }
            }
            for ((slot, material), [loose, shared, indices]) in room {
                let into = if material == Material::Masonry {
                    fresh[slot].of(material)
                } else {
                    mine[slot].of(material)
                };
                into.reserve(loose);
                into.shared.reserve(shared, indices);
            }
            for (slot, made) in &made_pieces {
                for (&material, surface) in &made.0 {
                    let into = if material == Material::Masonry {
                        &mut fresh[*slot]
                    } else {
                        &mut mine[*slot]
                    };
                    into.of(material).extend_from(surface);
                }
            }
            if kit.pattern == Pattern::Masonry {
                let caching = keep.as_deref_mut().map(|keep| BlockCaching {
                    call: (index as u32) << 8 | detail as u32,
                    old: last.map(|last| &last.faces),
                    new: &mut keep.faces,
                });
                let blocks = draw_blocks(
                    section, kit, &own, &edge, detail, first, caching, &slot_of, slots, &wanted,
                );
                for (set, by_slot) in blocks.into_iter().enumerate() {
                    for (slot, surfaces) in by_slot.into_iter().enumerate() {
                        match set {
                            0 => join(&mut fresh[slot], surfaces),
                            1 if first => {
                                for (_, surface) in surfaces.0 {
                                    drawn.shell.append(surface);
                                }
                            }
                            2 if first => join(&mut next, surfaces),
                            2 => join(&mut fresh[slot], surfaces),
                            _ => {}
                        }
                    }
                }
            }
            for (slot, (mut mine, fresh)) in mine.into_iter().zip(fresh).enumerate() {
                let fresh = masked(fresh, true);
                join(&mut mine, fresh);
                for (material, surface) in mine.0 {
                    built[slot]
                        .entry((kit.kind(), material))
                        .or_default()
                        .append(surface);
                }
            }
        }
        // Each part's pieces kept: those drawn, or as they were for a part not drawn again.
        if let Some(keep) = keep.as_deref_mut()
            && matches!(detail, Detail::Near | Detail::Mid)
        {
            for (slot, part) in drawn_pieces.into_iter().enumerate() {
                let part = if wanted(slot) {
                    Some(std::sync::Arc::new(part))
                } else {
                    kept_part(slot).cloned()
                };
                if let Some(part) = part {
                    keep.pieces.insert((detail, slots, slot), part);
                }
            }
        }
        if first {
            drawn.next = next;
        }
        // Damaged cubes worn by how much of their health they have lost, every material
        // alike (`loss`): in their colours far off, and near for what is drawn in no
        // material's textures, or in a pack's pieces' atlas; a material's textured surface
        // near reveals the shared damage mask instead, cracked and chipped (D290, D305).
        if !laid.worn.is_empty() {
            let far = matches!(detail, Detail::Far | Detail::Proxy);
            for slot in built.iter_mut() {
                for (&(kind, material), surface) in slot.iter_mut() {
                    let textured = section
                        .kits
                        .of(kind)
                        .is_some_and(|kit| kit.textures.contains_key(&material));
                    let masked = textured && !matches!(material, Material::Piece | Material::Trim);
                    if far || !masked {
                        crate::loss::wear(surface, &laid.worn);
                    }
                }
            }
        }
        match detail {
            Detail::Near => drawn.near_by = built,
            Detail::Mid => drawn.mid_by = built,
            Detail::Far => drawn.far = bake(section, built.into_iter().next().unwrap_or_default()),
            Detail::Proxy => drawn.proxy = proxy(section),
        }
    }
    plain(section, &mut drawn);
    drawn.worn = laid.worn.clone();
    drawn
}

/// Draws a long member far off as a plain box, if it is one that is (`draw::member_box`):
/// a deck's plank past the middle detail, and an undressed log at the proxy's distance,
/// each its own shade as near. Their rounded and chamfered sections and their rings are
/// most of a far building's triangles and cannot be seen there.
pub(super) fn far_box(piece: &Piece, kit: &Kit, detail: Detail, surfaces: &mut Surfaces) -> bool {
    let shade = |seed: u32| {
        let mut random = layout::Random(seed);
        [1.0 - kit.timber.variation.shade * 0.8 * random.unit(); 3]
    };
    let metres = |material: Material| kit.textures.get(&material).map_or([2.0, 2.0], |t| t.metres);
    match (detail, piece) {
        (
            Detail::Far,
            Piece::Plank {
                from,
                to,
                wide,
                high,
                seed,
            },
        ) => {
            let half = [layout::metres(*wide) * 0.5, layout::metres(*high) * 0.5];
            crate::draw::member_box(
                surfaces.of(Material::Floor),
                from.metres(),
                to.metres(),
                half,
                metres(Material::Floor),
                shade(*seed),
            );
            true
        }
        (
            Detail::Far,
            Piece::Beam {
                from,
                to,
                wide,
                deep,
                seed,
                ..
            },
        ) => {
            // A box its depth across and its width up, a few pixels across, which way it
            // turns unseen.
            let half = [layout::metres(*deep) * 0.5, layout::metres(*wide) * 0.5];
            let material = kit.framing.as_ref().map_or(Material::Hewn, |f| f.material);
            crate::draw::member_box(
                surfaces.of(material),
                from.metres(),
                to.metres(),
                half,
                metres(material),
                shade(*seed),
            );
            true
        }
        _ => false,
    }
}

/// Surfaces added to others.
pub(super) fn join(into: &mut Surfaces, from: Surfaces) {
    for (material, surface) in from.0 {
        into.of(material).append(surface);
    }
}
