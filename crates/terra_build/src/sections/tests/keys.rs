//! A part's key against how it is drawn, swept over every base's edits, every section either
//! side of each and both reaches: each case on its own core (`terra_gen::par`, D406).

use super::*;

/// A base edited each way, and which edit harmed which cube.
type Edits = (Vec<Base>, Option<(usize, IVec3)>);

/// One section of one base before and after one of its edits.
struct Case<'a> {
    index: usize,
    which: usize,
    base: &'a Base,
    edited: &'a Base,
    /// The edit that harmed a cube, and the cube.
    harm: Option<(usize, IVec3)>,
    at: [i32; 3],
}

// Slow, about a minute on every core: a sweep.
#[test]
fn a_part_whose_key_is_as_it_was_draws_as_it_was_and_parts_drawn_alone_draw_as_with_the_rest() {
    let kits = shipped();
    let maps = Maps::default();
    let bases = bases(&kits);
    let edits: Vec<Edits> = bases.iter().map(|base| edits(&kits, base)).collect();
    let mut cases = Vec::new();
    for (index, (base, (edits, harm))) in bases.iter().zip(&edits).enumerate() {
        for (which, edited) in edits.iter().enumerate() {
            for &at in drawable(base).union(&drawable(edited)) {
                cases.push(Case {
                    index,
                    which,
                    base,
                    edited,
                    harm: *harm,
                    at,
                });
            }
        }
    }
    let counted = terra_gen::par::map(&cases, |case| {
        [Reach::Near, Reach::Mid].map(|reach| case.drawn_as_keyed(&kits, &maps, reach))
    });
    let (kept, changed) = counted
        .iter()
        .flatten()
        .fold((0, 0), |(k, c), (kept, changed)| (k + kept, c + changed));
    // A wall's cube taken out reaches a few parts round it, not every one.
    assert!(
        kept > 2 * changed && changed > 0,
        "{kept} kept, {changed} drawn again"
    );
}

/// A base's edits: a wall's cube a metre up taken out, and harmed (kept beside the cubes, not
/// in them, D305); wall cubes either side of where two parts meet taken out, since a cube
/// shapes the faces of the cubes beside it across the parts' edge too; the highest cube that
/// is not a roof's, so what is far under it keeps its key only if it draws as it did; a roof's
/// lowest cube; and the ground under it changed. With which edit harmed which cube.
fn edits(kits: &Kits, base: &Base) -> Edits {
    let mut edits: Vec<Base> = Vec::new();
    let mut harm = None;
    let taken_out = |at: IVec3| {
        let mut edited = base.clone();
        edited.apply(BuildOp::Fill {
            min: at.to_array(),
            max: at.to_array(),
            cube: None,
        });
        edited
    };
    if let Some(wall) = base
        .cubes()
        .map(|(at, _)| at)
        .find(|&at| at.y == 2 && base.in_wall(at))
    {
        edits.push(taken_out(wall));
        let mut harmed = base.clone();
        let rules = kits.rules();
        let damage = crate::ops::Op::Damage {
            cubes: vec![wall.to_array()],
            amount: 150,
        };
        if harmed
            .act(&damage, &crate::ops::Context::free(&rules))
            .is_ok()
        {
            harm = Some((edits.len(), wall));
            edits.push(harmed);
        }
    }
    for side in [CLUSTER - 1, 0] {
        if let Some(wall) = base
            .cubes()
            .map(|(at, _)| at)
            .find(|&at| at.y == 3 && base.in_wall(at) && at.x.rem_euclid(CLUSTER) == side)
        {
            edits.push(taken_out(wall));
        }
    }
    if let Some((top, _)) = base
        .cubes()
        .filter(|(_, cube)| !cube.kind.is_roof())
        .max_by_key(|(at, _)| (at.y, at.x, at.z))
    {
        edits.push(taken_out(top));
    }
    if let Some((roof, _)) = base
        .cubes()
        .filter(|(_, cube)| cube.kind.is_roof())
        .min_by_key(|(at, _)| (at.y, at.x, at.z))
    {
        edits.push(taken_out(roof));
    }
    let mut edited = base.clone();
    let ring = edited.ground_ring();
    edited.sample_ground(ring.into_iter().map(|column| crate::ground::Sample {
        column,
        height_cm: 30,
    }));
    edits.push(edited);
    (edits, harm)
}

impl Case<'_> {
    /// Each part whose key is as it was is drawn as it was, and each drawn alone as with the
    /// rest, drawn again from how it was drawn as the game does: how many parts kept their key
    /// and how many were drawn again, counted for a wall's cube taken out.
    fn drawn_as_keyed(&self, kits: &Kits, maps: &Maps, reach: Reach) -> (usize, usize) {
        let (index, at) = (self.index, self.at);
        let section = |base| Section {
            base,
            kits,
            maps,
            at: IVec3::from_array(at),
            progress: &Progress::Whole,
        };
        let (before, after) = (section(self.base), section(self.edited));
        let (laid_before, laid_after) = (laid(&before), laid(&after));
        let keys_before = cluster_keys(&before, &laid_before, reach);
        let keys_after = cluster_keys(&after, &laid_after, reach);
        // A part with a cube harmed in it is drawn again, worn.
        if let Some((edit, cube)) = self.harm
            && edit == self.which
            && IVec3::from_array(at) == cube.div_euclid(IVec3::splat(crate::cubes::SECTION))
        {
            assert_ne!(
                keys_before, keys_after,
                "base {index}: harmed, its keys the same"
            );
        }
        let all = [true; CLUSTERS];
        let mut drawing = Drawing::default();
        let was = draw_parts(
            &before,
            &laid_before,
            reach,
            None,
            Some(&mut drawing),
            Some(&all),
        );
        let now = draw_parts(&after, &laid_after, reach, None, None, Some(&all));
        let only: Vec<bool> = keys_before
            .iter()
            .zip(&keys_after)
            .map(|(a, b)| a != b)
            .collect();
        // Drawn again from how it was drawn, as the game does, keeping how the parts not drawn
        // again were drawn for the next time.
        let mut kept_on = Drawing::default();
        let some = draw_parts(
            &after,
            &laid_after,
            reach,
            Some(&drawing),
            Some(&mut kept_on),
            Some(&only),
        );
        let again = draw_parts(&after, &laid_after, reach, Some(&kept_on), None, Some(&all));
        assert!(
            again.near_by == now.near_by && again.mid_by == now.mid_by,
            "base {index} section {at:?}: drawn from what was kept on, it differs"
        );
        let (mut kept, mut changed) = (0, 0);
        // Counted for a wall's cube taken out.
        let counted = usize::from(self.which == 0);
        for (c, &alone) in only.iter().enumerate() {
            let pick = |drawn: &Drawn| {
                let of = |by: &Vec<BTreeMap<(Kind, Material), Surface>>| {
                    by.get(c).cloned().unwrap_or_default()
                };
                (of(&drawn.near_by), of(&drawn.mid_by))
            };
            if alone {
                changed += counted;
                assert!(
                    pick(&some) == pick(&now),
                    "base {index} section {at:?} part {c}: drawn alone differs"
                );
            } else {
                kept += counted;
                assert!(
                    pick(&was) == pick(&now),
                    "base {index} section {at:?} part {c}: its key is as it was but it is not"
                );
                let (near, mid) = pick(&some);
                assert!(
                    near.values()
                        .chain(mid.values())
                        .all(|s| s.triangle_count() == 0)
                );
            }
        }
        (kept, changed)
    }
}
