//! The houses: plots cut along the streets and round the square, and the yards behind them.

use super::*;

impl Town {
    /// The houses: plots cut along each street's sides and round the square, each building
    /// the next of the rules' in the seed's order that fits, its door to the way, until the
    /// town has its count. The first on the main street is the smithy: a woodshed open to
    /// the street with a forge before it.
    pub(super) fn plots(&mut self, seed: u32, picks: impl Picks + Copy) {
        let want = BUILDINGS.0 + hash(&[seed, 12]) as usize % (BUILDINGS.1 - BUILDINGS.0 + 1);
        let mut queue: Vec<(Plan, Scheme, Roof, i32)> = village::every_building()
            .into_iter()
            // Houses, not sheds: a lean-to's one tall gable reads as a house half built
            // among a town's fronts (the town review, round 4).
            .filter(|(plan, _, roof, _)| {
                matches!(
                    plan,
                    Plan::Rectangle | Plan::L | Plan::T | Plan::U | Plan::TwoStoreys
                ) && *roof != Roof::LeanTo
            })
            .collect();
        for i in (1..queue.len()).rev() {
            let j = hash(&[seed, 13, i as u32]) as usize % (i + 1);
            queue.swap(i, j);
        }
        let mut fronts: Vec<Front> = Vec::new();
        let outline = self.square.outline.clone();
        let middle = outline.iter().copied().sum::<Vec2>() / outline.len() as f32;
        for index in 0..outline.len() {
            let (a, b) = (outline[index], outline[(index + 1) % outline.len()]);
            // Outward from the square: the side of the edge away from its middle.
            let side = -(middle - a).dot((b - a).perp()).signum();
            fronts.push(Front {
                points: vec![a, b],
                side,
                off: 0.0,
                at: SQUARE_CORNER_M,
                done: false,
                grand: true,
            });
        }
        for street in &self.streets {
            for side in [1.0, -1.0] {
                fronts.push(Front {
                    points: street.points.clone(),
                    side,
                    off: street.wide * 0.5,
                    at: STREET_FIRST_M,
                    done: false,
                    grand: false,
                });
            }
        }
        let mut next = 0;
        let mut smithy = false;
        let mut houses = 0;
        while houses < want && fronts.iter().any(|front| !front.done) {
            let square_done = fronts.iter().all(|front| !front.grand || front.done);
            for f in 0..fronts.len() {
                if houses >= want {
                    break;
                }
                // The square's edges are built up first, the streets' houses after.
                if fronts[f].done || (!fronts[f].grand && !square_done) {
                    continue;
                }
                let first_street = f == outline.len();
                let building_seed = hash(&[seed, 14, self.buildings.len() as u32]);
                let kinds = picks.pick(building_seed);
                if first_street && !smithy {
                    let choice = (Plan::Shed, Scheme::Timber, Roof::LeanTo, 6);
                    match self.try_plot(f, &mut fronts, building_seed, choice, kinds, true) {
                        Some(true) => smithy = true,
                        Some(false) => fronts[f].at += 2.0,
                        None => fronts[f].done = true,
                    }
                    continue;
                }
                let (first, length) = (next % queue.len(), queue.len());
                // The streets' first stretch out of the square, its busiest, is of houses of
                // two storeys where the queue has them, so it is closed in as a town's is.
                if !fronts[f].grand
                    && fronts[f].at < TALL_WITHIN_M
                    && unit(hash(&[building_seed, 16])) < TALL_SHARE
                    && let Some(look) = (0..length)
                        .find(|look| matches!(queue[(first + look) % length].0, Plan::TwoStoreys))
                {
                    queue.swap(first, (first + look) % length);
                }
                if !fronts[f].grand {
                    match self.try_plot(f, &mut fronts, building_seed, queue[first], kinds, false) {
                        Some(true) => {
                            next += 1;
                            houses += 1;
                        }
                        Some(false) => fronts[f].at += 2.0,
                        None => fronts[f].done = true,
                    }
                    continue;
                }
                // The square is closed in by house fronts, as a market square is: its edges
                // take the larger houses where the queue has them, and where the next will
                // not fit (by a street's mouth, at a corner, at an edge's end) a smaller one
                // that does, before the plot moves on a metre.
                let tall = unit(hash(&[building_seed, 16])) < TALL_SHARE;
                if let Some(look) = (0..length)
                    .find(|look| {
                        tall && matches!(queue[(first + look) % length].0, Plan::TwoStoreys)
                    })
                    .or_else(|| {
                        (0..6).find(|look| {
                            let (plan, _, _, wide) = queue[(first + look) % length];
                            matches!(plan, Plan::T) || wide >= 14
                        })
                    })
                {
                    queue.swap(first, (first + look) % length);
                }
                let mut room = false;
                let mut built = false;
                for look in 0..SQUARE_TRIES.min(length) {
                    let at = (first + look) % length;
                    // Fronts flat to the square: a wing brought forward either side of a door
                    // (an L, a U) makes a notch its crowd jams in.
                    if matches!(queue[at].0, Plan::L | Plan::U) {
                        continue;
                    }
                    match self.try_plot(f, &mut fronts, building_seed, queue[at], kinds, false) {
                        Some(true) => {
                            queue.swap(first, at);
                            built = true;
                            break;
                        }
                        Some(false) => room = true,
                        None => {}
                    }
                }
                if built {
                    next += 1;
                    houses += 1;
                } else if room {
                    fronts[f].at += 1.0;
                } else {
                    fronts[f].done = true;
                }
            }
        }
    }

    /// Tries the next plot along a front: the building put up, set where its own wings let
    /// its front stand back from the way, and kept if it is clear of everything. Nothing
    /// when the front has no room left for it, false when this place did not fit.
    pub(super) fn try_plot(
        &mut self,
        f: usize,
        fronts: &mut [Front],
        seed: u32,
        choice: (Plan, Scheme, Roof, i32),
        kinds: Kinds,
        smithy: bool,
    ) -> Option<bool> {
        let front = &fronts[f];
        let length = line_length(&front.points);
        let probe = village::put_up(seed, choice, [0.0, 0.0], 0.0, kinds);
        let rects = ground_rects(&probe);
        if rects.is_empty() {
            return Some(false);
        }
        let low = rects.iter().map(|r| r.0).reduce(Vec2::min)?;
        let high = rects.iter().map(|r| r.1).reduce(Vec2::max)?;
        // Along the front, the building's local x runs against the line on its left side.
        let (from, to) = if front.side > 0.0 {
            (-high.x, -low.x)
        } else {
            (low.x, high.x)
        };
        let door_along = front.at - from;
        // The square's corners built close, its fronts running on to them; a street keeps
        // a little room at its end.
        let end = if front.grand { SQUARE_CORNER_M } else { 2.0 };
        if door_along + to > length - end {
            return None;
        }
        let (point, way) = along_line(&front.points, door_along)?;
        let outward = way.perp() * front.side;
        let back = if smithy { 2.6 } else { 0.0 };
        let off_way = if front.grand {
            SQUARE_OFF_M
        } else {
            OFF_STREET_M
        };
        let door = point + outward * (front.off + off_way + 0.2 + high.y.max(0.0) + back);
        let facing = facing_of(-outward);
        let building = Building {
            at: door.to_array(),
            facing,
            ..probe
        };
        let ground = footprint(&building, &rects);
        let gap = if hash(&[seed, 15]).is_multiple_of(PASSAGE_EVERY) {
            PASSAGE_M
        } else {
            APART_M
        };
        if !self.clear_for(&ground, APART_M) {
            return Some(false);
        }
        // Every door reached straight from the way, this one and those already built: with
        // the houses joined, a neighbour's corner on a bend can otherwise close one off.
        // Each door's way in, out from it and across before it, room for two abreast: a
        // door in a notch beside a neighbour's wing jammed those coming home.
        let way_in = |at: Vec2, facing: f32| {
            [
                (
                    place(at, facing, Vec2::new(0.0, 0.75)),
                    place(at, facing, Vec2::new(0.0, DOOR_WAY_M)),
                ),
                (
                    place(at, facing, Vec2::new(-DOOR_ABREAST_M, 0.9)),
                    place(at, facing, Vec2::new(DOOR_ABREAST_M, 0.9)),
                ),
            ]
        };
        let own = way_in(door, facing);
        let doors: Vec<(Vec2, Vec2)> = self
            .things
            .iter()
            .filter(|thing| thing.kind == ThingKind::Door)
            .flat_map(|thing| way_in(thing.at, thing.facing))
            .collect();
        if !self.footprints.iter().flatten().all(|quad| {
            own.iter()
                .all(|&(a, b)| segment_polygon(a, b, quad) >= CLEAR_M)
        }) || !ground.iter().all(|quad| {
            doors
                .iter()
                .all(|&(a, b)| segment_polygon(a, b, quad) >= CLEAR_M)
        }) {
            return Some(false);
        }
        fronts[f].at = door_along + to + gap;
        let index = self.buildings.len();
        self.buildings.push(building);
        self.footprints.push(ground);
        if smithy {
            self.things.push(Thing::new(
                ThingKind::Forge,
                place(door, facing, Vec2::new(0.0, 1.3)),
                facing,
                Some(index),
            ));
        } else {
            self.homes.push(index);
            self.things
                .push(Thing::new(ThingKind::Door, door, facing, Some(index)));
        }
        Some(true)
    }

    /// A yard behind each home that has room, and in it, by the seed, a goat pen of low
    /// walls, hens, a woodshed with a chopping block, or a chopping block alone.
    pub(super) fn yards(&mut self, seed: u32, picks: impl Picks + Copy) {
        let homes = self.homes.clone();
        let mut made = 0u32;
        for home in homes {
            let rects = ground_rects(&self.buildings[home]);
            let (Some(low), Some(high)) = (
                rects.iter().map(|r| r.0).reduce(Vec2::min),
                rects.iter().map(|r| r.1).reduce(Vec2::max),
            ) else {
                continue;
            };
            let (at, facing) = (
                Vec2::from(self.buildings[home].at),
                self.buildings[home].facing,
            );
            let mut yard = None;
            for deep in [8.0, 6.0, 4.5] {
                let quad = turned_box(
                    at,
                    facing,
                    Vec2::new(low.x, low.y - BEHIND_M - deep),
                    Vec2::new(high.x, low.y - BEHIND_M),
                );
                let clear = self
                    .footprints
                    .iter()
                    .flatten()
                    .all(|other| polygons(&quad, other) >= 0.6)
                    && self
                        .yards
                        .iter()
                        .all(|(_, other)| polygons(&quad, other) >= 0.6)
                    && self.streets.iter().all(|street| {
                        from_street(&quad, street) >= street.wide * 0.5 + OFF_STREET_M
                    })
                    && polygons(&quad, &self.square.outline) >= 1.0
                    && self
                        .things
                        .iter()
                        .filter_map(Thing::outline)
                        .all(|thing| polygons(&quad, &thing) >= 1.0)
                    && self.lanes.iter().all(|(_, lane)| {
                        lane.windows(2)
                            .all(|pair| segment_polygon(pair[0], pair[1], &quad) >= 0.6)
                    });
                if clear {
                    yard = Some((quad, deep));
                    break;
                }
            }
            let Some((quad, deep)) = yard else {
                continue;
            };
            // A yard is only a yard if it can be walked to, along one side of the house.
            let Some(lane) = self.lane(home, low, high, &quad) else {
                continue;
            };
            self.lanes.push((home, lane));
            self.yards.push((home, quad));
            let back = low.y - BEHIND_M - deep;
            let middle = place(
                at,
                facing,
                Vec2::new((low.x + high.x) * 0.5, back + deep * 0.5),
            );
            let kinds = picks.pick(hash(&[seed, 16, home as u32]));
            // Turn by turn, so a town has some of each, then by the seed.
            let what = (made + hash(&[seed, 17]) % 4) % 4;
            made += 1;
            match what {
                0 if deep >= 6.0 && high.x - low.x >= 5.0 => {
                    self.pen(home, at, facing, low.x, high.x, back, deep, kinds);
                }
                1 | 0 => {
                    self.things
                        .push(Thing::new(ThingKind::Yard, middle, facing, Some(home)));
                }
                2 if deep >= 6.0 => {
                    // A woodshed in the yard's far corner, open toward the house, a chopping
                    // block before it.
                    let corner = place(at, facing, Vec2::new(high.x - 1.8, back + 2.2));
                    let shed = village::put_up(
                        hash(&[seed, 18, home as u32]),
                        (Plan::Shed, Scheme::Timber, Roof::LeanTo, 6),
                        corner.to_array(),
                        facing,
                        kinds,
                    );
                    let rects = ground_rects(&shed);
                    self.footprints.push(footprint(&shed, &rects));
                    self.buildings.push(shed);
                    self.things.push(Thing::new(
                        ThingKind::Block,
                        place(at, facing, Vec2::new(high.x - 1.8, back + 4.0)),
                        facing,
                        Some(home),
                    ));
                }
                _ => {
                    self.things.push(Thing::new(
                        ThingKind::Block,
                        place(
                            at,
                            facing,
                            Vec2::new((low.x + high.x) * 0.5, back + deep * 0.6),
                        ),
                        facing,
                        Some(home),
                    ));
                }
            }
        }
    }

    /// A goat pen of low walls filling a yard, a gate in its side toward the house.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn pen(
        &mut self,
        home: usize,
        at: Vec2,
        facing: f32,
        left: f32,
        right: f32,
        back: f32,
        deep: f32,
        kinds: Kinds,
    ) {
        let corner = |x: f32, y: f32| place(at, facing, Vec2::new(x, y));
        let (l, r) = (left + 0.3, right - 0.3);
        let (b, f) = (back + 0.3, back + deep - 0.3);
        let gate = (l + r) * 0.5;
        let lengths = [
            (corner(l, b), corner(r, b)),
            (corner(r, b), corner(r, f)),
            (corner(r, f), corner(gate + 0.8, f)),
            (corner(gate - 0.8, f), corner(l, f)),
            (corner(l, f), corner(l, b)),
        ];
        for (index, (from, to)) in lengths.into_iter().enumerate() {
            // Each length starts a cube on from the corner the last ended at.
            let way = to - from;
            let from = from + way.normalize_or_zero() * CELL_M;
            let way = to - from;
            let long = (way.length() / CELL_M).round() as i32;
            if long < 2 {
                continue;
            }
            // A low wall runs along its own x, its middle at its place; its face behind it.
            let wall = village::put_up(
                hash(&[home as u32, index as u32]),
                (Plan::Wall, Scheme::Masonry, Roof::Gable, long),
                ((from + to) * 0.5).to_array(),
                libm::atan2f(-way.y, way.x),
                kinds,
            );
            let rects = ground_rects(&wall);
            self.footprints.push(footprint(&wall, &rects));
            self.buildings.push(wall);
        }
        let middle = corner((l + r) * 0.5, (b + f) * 0.5);
        self.things
            .push(Thing::new(ThingKind::Pen, middle, facing, Some(home)));
    }
}
