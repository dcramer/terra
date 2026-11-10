//! The kinds of undergrowth (`Under`) and where each likes to grow.

use super::*;

/// The kinds of undergrowth: the floor's, knee-high and below, and the middle layer's, one
/// to five metres (D268).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Under {
    Bracken,
    Fern,
    Frond,
    Herb,
    Hazel,
    Bilberry,
    Scrub,
    Sapling,
    Branches,
    Flowers,
    Coppice,
    Holly,
    Spruce,
    Bramble,
    Snag,
    Twigs,
    /// Bracken on a path's sunlit verge, as it lines a woodland ride: the lit fronds by the
    /// way that lead the eye down it (the forest walk of 10-19). Last, so its dice leave every
    /// other kind where it was.
    Verge,
    /// A realm's own kind, a modelled plant placed by its row of `data/realms.ron` (D642,
    /// D136): where it likes to grow is its row's (`Placement::likes`), not a rule here.
    Placed,
}

/// A shrub or young tree, above knee height, stands at least so far from a path's edge, so
/// the way's line stays open down its length and what crosses it is seen crossing (the forest
/// walk's hazel across the path and over the stag, lake round 31).
pub(super) const PATH_LINE_M: f32 = 2.5;
/// Verge bracken's clumps are started within so far outside a path's edge (`Path::beside`).
pub(super) const VERGE_OUT_M: f32 = 2.0;
/// The sun is looked for past the crowns from so high over the ground, a frond's height,
/// and so far toward it: past the tallest crown's top in a low morning sun.
pub(super) const FRONDS_M: f32 = 0.5;
pub(super) const SUN_LOOKS_M: f32 = 120.0;

/// A place uniformly in a ring round the clearing's middle, from `from` to `to` metres out.
pub(super) fn disc(dice: &mut Rng, from: f32, to: f32) -> Vec2 {
    let angle = dice.range(0.0, TAU);
    let distance = libm::sqrtf(dice.range(from * from, to * to));
    Vec2::new(libm::cosf(angle), libm::sinf(angle)) * distance
}

/// A bell from `low` to `high`, highest between.
pub(super) fn between(low: f32, high: f32, value: f32) -> f32 {
    let middle = (low + high) * 0.5;
    smoothstep(low, middle, value) * (1.0 - smoothstep(middle, high, value))
}

impl Under {
    /// How much a kind likes a place, 0 to 1, before its drifts.
    pub(super) fn likes(self, here: &Here) -> f32 {
        let near = |d: f32, from: f32, to: f32| 1.0 - smoothstep(from, to, d);
        let broadleaf = 1.0 - smoothstep(0.2, 0.6, here.needles);
        // The understorey's shrubs thin on a pine stand's needles but do not go (D359): gone
        // there, the stand's floor read as a park's lawn between its boles, its undergrowth
        // hiding a fifth of the view at eye height where a wild wood's hides a third to a half.
        let understorey = 0.45 + 0.55 * broadleaf;
        // The middle layer is thickest where light comes down the side of something open,
        // a clearing's or a path's edge or a gap, and thinner in the deep shade.
        let edge = near(here.path, 2.0, 9.0);
        match self {
            // Half shade: the edges of clearings, gaps, the paths' sides.
            Under::Bracken => between(0.05, 1.2, here.light) * (0.4 + 0.6 * broadleaf),
            // The shade, the damp, round the trunks and along the logs.
            Under::Fern | Under::Frond => {
                (1.0 - smoothstep(0.35, 0.8, here.light))
                    * (0.35 + 0.65 * here.wet)
                    * (0.5 + 0.5 * near(here.bark, 1.0, 4.0).max(near(here.lying, 0.6, 2.5)))
            }
            Under::Herb => {
                (1.0 - smoothstep(0.3, 0.7, here.light)) * broadleaf * (0.3 + 0.7 * here.wet)
            }
            // The understorey, under the edge of the canopy.
            Under::Hazel => between(-0.2, 0.8, here.light) * understorey,
            // Under the pines, on their needles.
            Under::Bilberry => smoothstep(0.3, 0.7, here.needles) * between(0.05, 1.0, here.light),
            // In the open and along the paths.
            Under::Scrub => {
                smoothstep(0.45, 0.9, here.light) * (0.5 + 0.5 * near(here.path, 1.0, 6.0))
            }
            // Where a gap lets the light down; a broadleaf's, fewer on a pine stand's needles,
            // whose high crowns let light down nearly everywhere (D318).
            Under::Sapling => {
                smoothstep(0.15, 0.6, here.light)
                    * smoothstep(2.0, 4.0, here.bark)
                    * (0.6 + 0.4 * edge)
                    * (0.5 + 0.5 * broadleaf)
            }
            // Fallen from the trees, round their feet and by the logs.
            Under::Branches => {
                (1.0 - smoothstep(0.4, 0.8, here.light))
                    * (0.4 + 0.6 * near(here.bark, 2.0, 6.0).max(near(here.lying, 1.0, 3.0)))
            }
            // In the sun, and along the paths most.
            Under::Flowers => {
                smoothstep(0.65, 0.95, here.light) * (0.4 + 0.6 * near(here.path, 0.5, 4.0))
            }
            // Hazel stools under oaks, thickest in half shade at the edges.
            Under::Coppice => {
                (0.45 + 0.55 * between(0.05, 1.1, here.light)) * understorey * (0.6 + 0.4 * edge)
            }
            // Holly takes the deep shade under the oaks that little else does.
            Under::Holly => (1.0 - smoothstep(0.55, 0.95, here.light)) * understorey,
            // Young spruces come up in the gaps under the spruces, and few under the pines,
            // whose stands stay open under their high crowns, so a pine stand reads as pines
            // rather than a spruce thicket (D289).
            Under::Spruce => {
                smoothstep(0.25, 0.6, here.needles)
                    * (0.55 + 0.45 * between(0.05, 1.1, here.light))
                    * (0.2 + 0.8 * smoothstep(0.15, 0.55, here.spruce))
            }
            // Bramble in the light: gaps, the paths' and the clearing's edges.
            Under::Bramble => smoothstep(0.3, 0.75, here.light) * (0.5 + 0.5 * edge),
            // Small trees that lost the race for light, among the big ones.
            Under::Snag => {
                (1.0 - smoothstep(0.5, 0.9, here.light)) * smoothstep(1.5, 3.5, here.bark)
            }
            // Twigs fallen from the canopy, thickest under it and round the trunks.
            Under::Twigs => {
                (1.0 - smoothstep(0.5, 0.95, here.light)) * (0.5 + 0.5 * near(here.bark, 1.0, 6.0))
            }
            // On a path's verge, in the sun's patches where the floor is told where the sun
            // falls, else where the sky's light comes down the path's corridor.
            Under::Verge => smoothstep(0.35, 0.8, here.sun) * near(here.path, 0.4, 2.2),
            // Anywhere, as far as its kind goes: its row says how it likes the ground.
            Under::Placed => 1.0,
        }
    }

    /// Where a clump of it is tried: what lines a path beside one, none where there are no
    /// paths, the rest over the floor's area.
    pub(super) fn tried_at(self, ways: &impl Ways, area: &Area, dice: &mut Rng) -> Option<Vec2> {
        if self == Under::Verge {
            ways.beside(dice, VERGE_OUT_M)
        } else {
            Some(area.place_undergrowth(dice))
        }
    }

    /// Whether it keeps the path's line open from a place so far from the path's edge: a
    /// shrub or young tree only past `PATH_LINE_M`, what grows knee-high and below anywhere
    /// it is clear of the path (`clear_of`).
    pub(super) fn keeps_path_line(self, path_m: f32) -> bool {
        let tall = matches!(
            self,
            Under::Hazel
                | Under::Scrub
                | Under::Sapling
                | Under::Coppice
                | Under::Holly
                | Under::Spruce
                | Under::Bramble
                | Under::Snag
        );
        !tall || path_m >= PATH_LINE_M
    }

    /// Whether it stands in a well of its own in the snow and the litter, as a tree does: a
    /// shrub's stool or a young tree, not what grows knee-high or lies dead.
    pub(super) fn stands_in_a_well(self) -> bool {
        self != Under::Snag && !self.keeps_path_line(0.0)
    }

    /// How far it stays from bark, from what lies on the floor, and from a path's edge.
    pub(super) fn clear_of(self) -> (f32, f32, f32) {
        match self {
            Under::Hazel | Under::Scrub => (1.0, 0.3, 0.7),
            Under::Sapling | Under::Holly | Under::Spruce => (2.0, 0.6, 1.4),
            Under::Coppice => (1.8, 0.6, 1.8),
            Under::Bramble => (1.0, 0.3, 1.2),
            Under::Snag => (1.6, 0.8, 2.5),
            Under::Branches => (0.5, 0.1, 0.4),
            Under::Twigs => (0.3, 0.05, 0.3),
            Under::Flowers => (0.6, 0.1, 0.05),
            _ => (0.35, 0.1, 0.2),
        }
    }
}
