//! A far card's outline (far_forest.wesl): the few bands, stacked up the card, that hold all
//! its tree can be, so the depth prepass works out the tree's shape only where the tree may
//! be, not over the card's empty air (a pine's card is open air between the undergrowth at
//! its foot and its high crown, but for the trunk). As an impostor's card is cut to its
//! picture's outline (Unreal's and SpeedTree's billboard cutouts, Kaiser's "Imposter
//! Generator" cut-out polygons); here the outline is worked out from the bounds of the
//! shader's own `tree()`, not from a picture.
//!
//! Each bound is the shader's: `mass()` lifts a crown's edge out by at most half its
//! raggedness and 0.13 more (its fine and spray noises, each 0 to 1), the undergrowth stands
//! at most 4.2 m (1 + 2.5, times 1.2), a trunk is at most 1.3 times its width at the foot, and a
//! spruce's whorls droop at most 1.15 times its cone. A change to `tree()` changes these.
//! The vertex shader widens every band by two pixels at its distance, outward where it
//! stands proud of the band beside it, so an edge's dither and TAA's jitter stay inside.

/// The tallest the undergrowth round a tree's foot stands (far_forest.wesl's `tall`).
const UNDERGROWTH_M: f32 = 4.2;

/// One band of a card: from `low` to `high` metres up its tree, `half` metres either side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Band {
    pub low: f32,
    pub high: f32,
    pub half: f32,
}

/// A crown of masses as `crown()` lays it: its middle's height, its half sizes and its
/// raggedness, with how many bands it is cut into.
struct Crown {
    middle: f32,
    half: [f32; 2],
    ragged: f32,
    bands: usize,
}

impl Crown {
    /// How far out a mass's edge can reach, as a share of its half sizes.
    fn grow(&self) -> f32 {
        1.0 + 0.5 * self.ragged + 0.07 + 0.06
    }

    /// The masses' middles lie in this ellipse about the crown's (`crown()`'s `at`: reach at
    /// most 0.8, times 0.62, the height times 0.8 more).
    fn spread(&self) -> [f32; 2] {
        [self.half[0] * 0.8 * 0.62, self.half[1] * 0.8 * 0.62 * 0.8]
    }

    /// How low and how high any of its masses can reach.
    fn reach(&self) -> (f32, f32) {
        let g = self.grow();
        let low = (0.75 * self.half[1] * g).max(self.spread()[1] + 0.6 * self.half[1] * g);
        (self.middle - low, self.middle + low)
    }

    /// The most the crown can reach either side `dy` above its middle: the middle mass, or
    /// the largest of the others (0.6 its half sizes) wherever its middle lies.
    fn half_at(&self, dy: f32) -> f32 {
        let g = self.grow();
        let middle = ellipse(0.7 * self.half[0] * g, 0.75 * self.half[1] * g, dy);
        let [ax, ay] = self.spread();
        let (bx, by) = (0.6 * self.half[0] * g, 0.6 * self.half[1] * g);
        // A mass's middle round the spread's edge (inside it reaches no further out), tried
        // every few degrees; what falls between is within the slack added.
        let steps = 128;
        let others = (0..=steps)
            .filter_map(|i| {
                let turn = core::f32::consts::PI * (i as f32 / steps as f32 - 0.5);
                let at = ay * libm::sinf(turn);
                let across = ellipse(bx, by, dy - at);
                (across >= 0.0).then_some(ax * libm::cosf(turn) + across)
            })
            .fold(-1.0_f32, f32::max);
        let others = if others >= 0.0 {
            others + 0.05 * ax
        } else {
            0.0
        };
        middle.max(others)
    }
}

/// An ellipse's half width `dy` from its middle, below 0 outside it.
fn ellipse(a: f32, b: f32, dy: f32) -> f32 {
    let t = dy / b;
    if t.abs() > 1.0 {
        -1.0
    } else {
        a * libm::sqrtf(1.0 - t * t)
    }
}

/// The bands of a card `width` by `height` metres for a tree of its kind (oak 0, pine 1,
/// spruce 2), from its foot to its top, none overlapping, none wider than the card.
pub(super) fn outline(kind: usize, width: f32, height: f32) -> Vec<Band> {
    let (w, h) = (width, height);
    let edge = 0.5 * w;
    // Where each kind's leaves begin, its trunk's height and width, and its crown.
    let (crown, bole, trunk) = match kind {
        0 => (
            Some(Crown {
                middle: 0.64 * h,
                half: [edge, 0.34 * h],
                ragged: 0.4,
                bands: 4,
            }),
            0.42 * h,
            0.028 * h,
        ),
        1 => (
            Some(Crown {
                middle: 0.82 * h,
                half: [edge, 0.17 * h],
                ragged: 0.5,
                bands: 3,
            }),
            0.86 * h,
            0.022 * h,
        ),
        _ => (None, 0.32 * h, 0.02 * h),
    };
    let leaves_from = crown.as_ref().map_or(bole, |c| c.reach().0.max(0.0));
    let bands = crown.as_ref().map_or(5, |c| c.bands);
    let mut levels = vec![0.0, UNDERGROWTH_M.min(h), leaves_from.min(h)];
    levels.extend((0..=bands).map(|i| leaves_from + (h - leaves_from) * i as f32 / bands as f32));
    levels.retain(|y| (0.0..=h).contains(y));
    levels.sort_by(f32::total_cmp);
    levels.dedup_by(|a, b| (*a - *b).abs() < 1.0e-3);
    let widest = |low: f32, high: f32| -> f32 {
        let mut half: f32 = 0.0;
        if low < UNDERGROWTH_M {
            half = edge;
        }
        if low < bole {
            half = half.max(1.3 * trunk);
        }
        match &crown {
            Some(c) => {
                let (from, to) = c.reach();
                if high > from && low < to {
                    half = half.max(c.half_at(c.middle.clamp(low, high) - c.middle));
                }
            }
            // A spruce's cone narrows from its foot: widest at the band's foot.
            None if high > bole => {
                let up = ((low - bole) / (h - bole)).clamp(0.0, 1.0);
                half = half.max(1.15 * edge * libm::powf(1.0 - up, 0.9));
            }
            None => {}
        }
        half.min(edge)
    };
    let mut out: Vec<Band> = Vec::with_capacity(levels.len());
    for pair in levels.windows(2) {
        let half = widest(pair[0], pair[1]);
        match out.last_mut() {
            Some(last) if (last.half - half).abs() < 1.0e-4 => last.high = pair[1],
            _ => out.push(Band {
                low: pair[0],
                high: pair[1],
                half,
            }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The most `tree()` can reach either side at a height, by its bounds alone, worked out
    /// directly: every mass's middle and size tried, not the bands' reasoning.
    fn could_reach(kind: usize, w: f32, h: f32, y: f32) -> f32 {
        let mut half: f32 = 0.0;
        if y < UNDERGROWTH_M {
            half = 0.5 * w;
        }
        let (bole, trunk) = [(0.42, 0.028), (0.86, 0.022), (0.32, 0.02)][kind];
        if y < bole * h {
            half = half.max(1.3 * trunk * h);
        }
        if kind == 2 {
            if y > bole * h && y < h {
                let up = (y - bole * h) / (h - bole * h);
                half = half.max(0.575 * w * libm::powf(1.0 - up, 0.9));
            }
        } else {
            let (middle, hy, ragged) = [(0.64, 0.34, 0.4), (0.82, 0.17, 0.5)][kind];
            let (middle, hx, hy) = (middle * h, 0.5 * w, hy * h);
            let g = 1.0 + 0.5 * ragged + 0.13;
            let mut masses = vec![(0.0, middle, 0.7 * hx * g, 0.75 * hy * g)];
            for i in 0..=40 {
                let a = i as f32 / 40.0 * core::f32::consts::TAU;
                for reach in [0.55, 0.65, 0.8] {
                    let x = libm::cosf(a) * hx * reach * 0.62;
                    let at = middle + libm::sinf(a) * 0.8 * hy * reach * 0.62;
                    masses.push((x, at, 0.6 * hx * g, 0.6 * hy * g));
                }
            }
            for (x, at, a, b) in masses {
                let across = ellipse(a, b, y - at);
                if across >= 0.0 {
                    half = half.max(x.abs() + across);
                }
            }
        }
        half.min(0.5 * w)
    }

    #[test]
    fn a_cards_bands_hold_all_its_tree_can_be_and_stack_up_it() {
        for kind in 0..3 {
            for (w, h) in [(6.0, 12.0), (10.0, 24.0), (14.0, 32.0), (4.0, 3.0)] {
                let bands = outline(kind, w, h);
                assert_eq!(bands[0].low, 0.0, "kind {kind}: from the foot");
                assert!(
                    (bands.last().unwrap().high - h).abs() < 1.0e-3,
                    "kind {kind}: to the top"
                );
                for pair in bands.windows(2) {
                    assert_eq!(
                        pair[0].high, pair[1].low,
                        "kind {kind}: stacked, not overlapping"
                    );
                }
                for i in 0..=400 {
                    let y = h * i as f32 / 400.0;
                    let need = could_reach(kind, w, h, y);
                    let has = bands
                        .iter()
                        .filter(|b| y >= b.low && y <= b.high)
                        .map(|b| b.half)
                        .fold(0.0_f32, f32::max);
                    assert!(
                        has >= need - 1.0e-3,
                        "kind {kind} {w}x{h} at {y:.2} m: {need:.2} reached, {has:.2} held"
                    );
                    assert!(has <= 0.5 * w + 1.0e-4);
                }
            }
        }
    }

    #[test]
    fn a_pines_card_is_mostly_cut_away_between_its_undergrowth_and_crown() {
        let (w, h) = (10.0, 24.0);
        let area: f32 = outline(1, w, h)
            .iter()
            .map(|b| 2.0 * b.half * (b.high - b.low))
            .sum();
        assert!(area < 0.7 * w * h, "{area:.0} of {:.0} m²", w * h);
    }
}
