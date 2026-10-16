//! The square and the streets out of it, and what stands on the square and at the streets'
//! ends.

use super::*;

/// The square: a shape of the seed's size, its middle at the origin, along the contour.
pub(super) fn square(seed: u32, shape: SquareShape, along: Vec2, across: Vec2) -> Square {
    let size = |salt: u32, low: f32, high: f32| low + unit(hash(&[seed, 2, salt])) * (high - low);
    let corner = |a: f32, u: f32| along * a + across * u;
    let outline = match shape {
        SquareShape::Long => {
            let (long, wide) = (size(1, 28.0, 34.0) * 0.5, size(2, 18.0, 21.0) * 0.5);
            vec![
                corner(-long, -wide),
                corner(long, -wide),
                corner(long, wide),
                corner(-long, wide),
            ]
        }
        SquareShape::Triangle => {
            // Its base downhill, its point up the slope, its middle at the origin.
            let (base, deep) = (size(1, 42.0, 46.0) * 0.5, size(2, 27.0, 30.0));
            vec![
                corner(-base, -deep / 3.0),
                corner(base, -deep / 3.0),
                corner(0.0, deep * 2.0 / 3.0),
            ]
        }
        SquareShape::Widened => {
            let (long, wide) = (size(1, 44.0, 50.0) * 0.5, size(2, 12.0, 14.0) * 0.5);
            vec![
                corner(-long, -wide),
                corner(long, -wide),
                corner(long, wide),
                corner(-long, wide),
            ]
        }
    };
    Square {
        shape,
        outline,
        along,
        across,
    }
}

/// The lowest and highest a polygon reaches along a way.
pub(super) fn extent(outline: &[Vec2], way: Vec2) -> (f32, f32) {
    outline
        .iter()
        .map(|p| p.dot(way))
        .fold((f32::MAX, f32::MIN), |(l, h), x| (l.min(x), h.max(x)))
}

/// The streets out of the square: the main street out of either end along the contour, and
/// for some squares a third, from a corner or a side, up or down the slope until it can
/// turn along it.
pub(super) fn streets(seed: u32, square: &Square, heights: Heights) -> Vec<Street> {
    let (along, across) = (square.along, square.across);
    let long = |salt: u32| 80.0 + unit(hash(&[seed, 3, salt])) * 25.0;
    let mut starts: Vec<(Vec2, Vec2, f32, f32)> = Vec::new();
    match square.shape {
        SquareShape::Triangle => {
            // From each corner of its base along the contour, and from its point up the slope.
            let [a, b, c] = [square.outline[0], square.outline[1], square.outline[2]];
            starts.push((a, -along, long(1), MAIN_WIDE_M));
            starts.push((b, along, long(2), MAIN_WIDE_M));
            starts.push((c, across, long(3) * 0.9, SIDE_WIDE_M));
        }
        _ => {
            let (low, high) = extent(&square.outline, along);
            starts.push((along * low, -along, long(1), MAIN_WIDE_M));
            starts.push((along * high, along, long(2), MAIN_WIDE_M));
            if !hash(&[seed, 4]).is_multiple_of(3) {
                // A side street from one long side, up or down the slope.
                let way = if hash(&[seed, 5]).is_multiple_of(2) {
                    across
                } else {
                    -across
                };
                let shift = (unit(hash(&[seed, 6])) - 0.5) * (high - low) * 0.4;
                let (_, edge) = extent(&square.outline, way);
                starts.push((along * shift + way * edge, way, long(3) * 0.9, SIDE_WIDE_M));
            }
        }
    }
    let mut streets: Vec<Street> = Vec::new();
    for (index, (from, way, length, wide)) in starts.into_iter().enumerate() {
        let points = trace(
            hash(&[seed, 7, index as u32]),
            from,
            way,
            length,
            heights,
            &streets,
            &square.outline,
            // A main street leaves the square straight; a side street climbs or falls away
            // from it far enough for a row of plots between, then turns along the slope.
            if wide == MAIN_WIDE_M { 2 } else { 5 },
        );
        streets.push(Street { points, wide });
    }
    streets
}

/// A street's centre line traced from a point a way out, a step at a time, each step turned
/// a little at most the way the ground is most level, so it follows the contour: its first
/// `out_steps` straight on, so it leaves the square cleanly, and away from the other
/// streets and the square.
#[allow(clippy::too_many_arguments)]
pub(super) fn trace(
    seed: u32,
    from: Vec2,
    way: Vec2,
    length: f32,
    heights: Heights,
    others: &[Street],
    square: &[Vec2],
    out_steps: usize,
) -> Vec<Vec2> {
    let mut points = vec![from];
    let (mut at, mut heading) = (from, way.normalize());
    let steps = (length / STEP_M).ceil() as usize;
    for step in 0..steps {
        let mut best: Option<(f32, Vec2, Vec2)> = None;
        for turn in -2i32..=2 {
            let angle = turn as f32 * TURN;
            let ahead = Vec2::from_angle(angle).rotate(heading);
            let next = at + ahead * STEP_M;
            let rise = (heights(next) - heights(at)).abs() / STEP_M;
            // Straight on out of the square, then by the ground, with a little of its own.
            let straight = if step < out_steps { 4.0 } else { 0.003 };
            let mut cost = rise * 8.0
                + turn.unsigned_abs() as f32 * straight
                + unit(hash(&[seed, step as u32, turn as u32])) * 0.004;
            for street in others {
                for &point in street.points.iter().skip(2) {
                    let near = 14.0 - next.distance(point);
                    if near > 0.0 {
                        cost += near * 0.5;
                    }
                }
            }
            if step > 0 && inside(next, square) {
                cost += 100.0;
            }
            if best.is_none_or(|(least, _, _)| cost < least) {
                best = Some((cost, next, ahead));
            }
        }
        let Some((_, next, ahead)) = best else {
            break;
        };
        at = next;
        heading = ahead;
        points.push(at);
    }
    points
}

impl Town {
    /// The well, the trough, the stalls, benches and places to gather on the square.
    pub(super) fn on_the_square(&mut self, seed: u32) {
        let (along, across) = (self.square.along, self.square.across);
        let outline = self.square.outline.clone();
        let middle = outline.iter().copied().sum::<Vec2>() / outline.len() as f32;
        let (low_a, high_a) = extent(&outline, along);
        let (low_c, high_c) = extent(&outline, across);
        let roll = |salt: u32| unit(hash(&[seed, 10, salt])) - 0.5;
        // Inside the square by a margin, to be stood on.
        let well_in = |point: Vec2, margin: f32| {
            inside(point, &outline)
                && (0..outline.len()).all(|i| {
                    point_segment(point, outline[i], outline[(i + 1) % outline.len()]) >= margin
                })
        };
        let well = middle
            + along * roll(1) * (high_a - low_a) * 0.25
            + across * roll(2) * (high_c - low_c) * 0.2;
        let side = facing_of(along);
        self.things
            .push(Thing::new(ThingKind::Well, well, side, None));
        // The trough beside it is laid with the town's furnishing (`furnish`).
        // The market: two rows of stalls facing each other across an aisle, along the
        // square on the side away from the well.
        let line = middle - across * (well - middle).dot(across).signum() * (high_c - low_c) * 0.1;
        let stalls = 4 + hash(&[seed, 11]) as usize % 4;
        let mut placed = 0;
        let mut slot = 0usize;
        while placed < stalls && slot < 24 {
            let row = if slot.is_multiple_of(2) { -1.0 } else { 1.0 };
            let m = slot / 2;
            let offset = if m == 0 {
                0.0
            } else {
                let sign = if m % 2 == 1 { 1.0 } else { -1.0 };
                sign * m.div_ceil(2) as f32 * STALL_STEP_M
            } + roll(20) * 1.0;
            slot += 1;
            let at = line + along * offset + across * row * 2.7;
            let facing = facing_of(across * -row);
            let stall = Thing::new(ThingKind::Stall, at, facing, None);
            let Some(ground) = stall.outline() else {
                continue;
            };
            let fits = ground.iter().all(|&corner| well_in(corner, 1.5))
                && stall
                    .kind
                    .stands()
                    .iter()
                    .all(|&stand| well_in(stall.place(stand), 0.5))
                && self.things.iter().all(|other| match other.outline() {
                    // A market's stalls stand in rows close beside each other, as Kingdom
                    // Come's do; anything else keeps room to walk round.
                    Some(theirs) if other.kind == ThingKind::Stall => {
                        polygons(&ground, &theirs) >= 0.5
                    }
                    Some(theirs) => polygons(&ground, &theirs) >= 2.2,
                    None => true,
                });
            if fits {
                self.things.push(stall);
                placed += 1;
            }
        }
        // Benches near the square's edges, facing in.
        for index in 0..outline.len() {
            let (a, b) = (outline[index], outline[(index + 1) % outline.len()]);
            let edge = b - a;
            let inward = (middle - a).dot(edge.perp()).signum() * edge.perp().normalize();
            let at = a + edge * (0.3 + roll(30 + index as u32) * 0.2) + inward * 2.2;
            let bench = Thing::new(ThingKind::Bench, at, facing_of(inward), None);
            let clear = bench.outline().is_some_and(|ground| {
                self.things.iter().all(|other| match other.outline() {
                    Some(theirs) => polygons(&ground, &theirs) >= 1.8,
                    None => true,
                })
            });
            if clear && well_in(at, 1.5) {
                self.things.push(bench);
            }
        }
        // Two places to gather: the open ground farthest from anything on the square.
        let mut open: Vec<(f32, Vec2)> = Vec::new();
        for i in 0..24 {
            for j in 0..24 {
                let point = along * (low_a + (high_a - low_a) * (i as f32 + 0.5) / 24.0)
                    + across * (low_c + (high_c - low_c) * (j as f32 + 0.5) / 24.0);
                if !well_in(point, 3.5) {
                    continue;
                }
                // Out of the way of anyone coming onto the square from a street.
                let mouths = self
                    .streets
                    .iter()
                    .map(|street| street.points[0].distance(point) - 5.0)
                    .fold(f32::MAX, f32::min);
                let room = self
                    .things
                    .iter()
                    .map(|thing| match thing.outline() {
                        Some(ground) => segment_polygon(point, point, &ground),
                        None => thing.at.distance(point),
                    })
                    .fold(mouths, f32::min);
                open.push((room, point));
            }
        }
        open.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut gathered: Vec<Vec2> = Vec::new();
        for (room, point) in open {
            if room < 3.0 || gathered.len() == 2 {
                break;
            }
            if gathered.iter().all(|other| other.distance(point) > 9.0) {
                gathered.push(point);
                self.things.push(Thing::new(
                    ThingKind::Gathering,
                    point,
                    facing_of(along),
                    None,
                ));
            }
        }
    }

    /// At each street's far end, the gate out to the fields and a guard's post beside it.
    pub(super) fn ends(&mut self) {
        let mut posts = Vec::new();
        for street in &self.streets {
            let count = street.points.len();
            if count < 3 {
                continue;
            }
            let (end, before) = (street.points[count - 1], street.points[count - 2]);
            let out = (end - before).normalize_or(Vec2::X);
            posts.push(Thing::new(ThingKind::Gate, end, facing_of(out), None));
            // The post on whichever side of the street is clear to stand on.
            for side in [1.0, -1.0] {
                let across = out.perp() * side;
                let post = before + across * (street.wide * 0.5 + 0.6);
                if self.standable(post, 1.0) {
                    posts.push(Thing::new(ThingKind::Post, post, facing_of(-across), None));
                    break;
                }
            }
        }
        self.things.extend(posts);
    }
}
