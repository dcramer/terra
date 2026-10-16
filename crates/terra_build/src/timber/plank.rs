//! A deck's planks, joists and rim beams, sawn square.

use super::*;

/// A deck's plank (`layout::floors`): a board, a joist or a rim beam, sawn square, its face
/// in the kit's `Floor` material, drawn from one board of it where the texture is of boards
/// side by side, a little lighter or darker by its seed; its sawn ends in `LogEnd`.
pub fn plank(piece: &Piece, kit: &Kit, surfaces: &mut Surfaces) {
    let Piece::Plank {
        from,
        to,
        wide,
        high,
        seed,
    } = *piece
    else {
        return;
    };
    let (from, to) = (from.metres(), to.metres());
    let (wide, high) = (layout::metres(wide), layout::metres(high));
    if from.distance(to) < 0.02 {
        return;
    }
    let frame = Frame::new(from, to);
    let mut random = Random(seed);
    let texture = kit.textures.get(&Material::Floor);
    let strips = texture.and_then(|texture| texture.strips);
    let strip = strips.map(|[first, apart]| {
        let count = ((1.0 - first) / apart).floor().max(1.0);
        let which = (random.unit() * count).floor().min(count - 1.0);
        // Inside the board's edges, so its seams fall in the gaps.
        [first + apart * (which + 0.06), apart * 0.88]
    });
    let look = Squared {
        half_wide: wide * 0.5,
        half_high: high * 0.5,
        round: PLANK_EDGE_M.min(high * 0.3),
        metres: texture.map_or([2.0, 2.0], |texture| texture.metres),
        strip,
        bend: 0.0,
        adzed: false,
        chamfer: 0.004,
        // A few metres off and further, its ends alone: a board's gentle wave along it is
        // nothing there, and a deck has many.
        step: if kit.dress.far {
            COARSE_STEP_M
        } else {
            PLANK_STEP_M
        },
        corner_steps: 1,
        tight: true,
        bedded: false,
    };
    let mut member = squared(&frame, &look, Split::Whole, &mut random);
    // A sloping one, a flight's stringer, cut level at the top of the cube its higher end
    // is in, as a carpenter cuts a stringer's head level with the deck it hangs from, its
    // square end against the rim and nothing of it standing over the boards.
    if (from.y - to.y).abs() > 0.05 {
        let cap = libm::ceilf(from.y.max(to.y) / crate::CELL_M - 1e-3) * crate::CELL_M;
        level_cut(&mut member, from, frame.axis, cap);
        // And its foot cut plumb where its line meets the floor, at the flight's front, so
        // its lower edge, a section's depth down the slope from its line, does not stand out
        // in front of the flight.
        let (low, high) = if from.y < to.y {
            (from, to)
        } else {
            (to, from)
        };
        let way = (low - high).with_y(0.0).normalize_or_zero();
        if way != Vec3::ZERO {
            plumb_cut(&mut member, from, frame.axis, low, way);
        }
    }
    let t = &kit.timber;
    let shade = 1.0 - t.variation.shade * 0.8 * random.unit();
    let warmth = t.variation.warmth * random.signed();
    let colour = [
        (shade * (1.0 + warmth)).clamp(0.0, 1.0),
        shade.clamp(0.0, 1.0),
        (shade * (1.0 - warmth)).clamp(0.0, 1.0),
    ];
    sweep(surfaces.of(Material::Floor), &member, colour);
    let spin = random.unit() * TAU;
    let last = member.grid.len() - 1;
    let ends = surfaces.of(Material::LogEnd);
    let cell = end_cell(kit, seed);
    for (n, (ring, outward)) in [
        (&member.grid[0], -frame.axis),
        (&member.grid[last], frame.axis),
    ]
    .into_iter()
    .enumerate()
    {
        let colour = end_colour(t, seed.wrapping_add(n as u32), shade, warmth);
        cap(
            ends,
            ring,
            outward,
            &frame,
            spin,
            member.grain,
            colour,
            cell,
        );
    }
}
