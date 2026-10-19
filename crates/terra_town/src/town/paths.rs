//! The path graph a town is walked by: the streets' centre lines, the square, the lanes to
//! every door and thing, and every part joined.

use super::*;

impl Town {
    /// Whether a walk between two points keeps clear of everything built and every thing
    /// in the way.
    pub fn walkable(&self, a: Vec2, b: Vec2) -> bool {
        self.footprints
            .iter()
            .flatten()
            .all(|quad| segment_polygon(a, b, quad) >= CLEAR_M)
            && self
                .things
                .iter()
                .filter_map(Thing::outline)
                .all(|quad| segment_polygon(a, b, &quad) >= CLEAR_M)
            && self
                .fences
                .iter()
                .all(|fence| segments(a, b, fence.from, fence.to) >= CLEAR_M)
            && self
                .things
                .iter()
                .filter(|thing| thing.kind == ThingKind::Gathering)
                .all(|thing| point_segment(thing.at, a, b) >= GATHERED_M)
    }

    /// Joins a node that reaches none of the streets' or the square's nodes (in a narrow
    /// street, the walk slantwise to one passes too near the fronts; on the square, the
    /// market stands in the way) to the nearest street straight across from it, a node there
    /// on the centre line between that stretch's own two; or else a step in onto the square.
    pub(super) fn straight_across(
        &self,
        nodes: &mut Vec<(Vec2, On)>,
        edges: &mut BTreeSet<[u32; 2]>,
        node: u32,
    ) {
        let at = nodes[node as usize].0;
        let mut best: Option<(f32, Vec2, u32, u32, usize)> = None;
        for (index, (a, b)) in nodes
            .iter()
            .enumerate()
            .zip(nodes.iter().enumerate().skip(1))
            .filter_map(|((i, (p, on)), (j, (q, next)))| match (on, next) {
                (On::Street(s), On::Street(t)) if s == t => Some((s, ((i, *p), (j, *q)))),
                _ => None,
            })
            .map(|(&s, pair)| (s, pair))
        {
            let ((i, p), (j, q)) = (a, b);
            let way = q - p;
            let t = ((at - p).dot(way) / way.length_squared().max(1e-6)).clamp(0.0, 1.0);
            let foot = p + way * t;
            let d = foot.distance(at);
            if d < 8.0 && best.is_none_or(|b| d < b.0) && self.walkable(at, foot) {
                best = Some((d, foot, i as u32, j as u32, index));
            }
        }
        if let Some((_, foot, i, j, street)) = best {
            let new = nodes.len() as u32;
            nodes.push((foot, On::Street(street)));
            for (a, b) in [(new, i), (new, j), (new, node)] {
                if a != b {
                    edges.insert([a.min(b), a.max(b)]);
                }
            }
            return;
        }
        // On the square, a step in toward its middle, and on from there to the square's
        // nearest nodes it can walk to.
        let outline = &self.square.outline;
        let middle = outline.iter().copied().sum::<Vec2>() / outline.len() as f32;
        let step = at + (middle - at).normalize_or_zero() * 1.5;
        if !self.walkable(at, step) {
            return;
        }
        let mut near: Vec<(f32, u32)> = nodes
            .iter()
            .enumerate()
            .filter(|(_, (_, on))| *on == On::Square)
            .map(|(k, (p, _))| (p.distance(step), k as u32))
            .filter(|&(d, _)| d < 10.0)
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        let reached: Vec<u32> = near
            .into_iter()
            .filter(|&(_, k)| self.walkable(step, nodes[k as usize].0))
            .take(3)
            .map(|(_, k)| k)
            .collect();
        if reached.is_empty() {
            return;
        }
        let new = nodes.len() as u32;
        nodes.push((step, On::Square));
        edges.insert([node.min(new), node.max(new)]);
        for k in reached {
            edges.insert([k.min(new), k.max(new)]);
        }
    }

    /// Whether a point is clear to stand on.
    pub(super) fn standable(&self, point: Vec2, margin: f32) -> bool {
        self.walkable(point, point)
            && self
                .things
                .iter()
                .filter(|thing| thing.kind == ThingKind::Gathering)
                .all(|thing| thing.at.distance(point) >= GATHERED_M + margin)
            && self
                .footprints
                .iter()
                .flatten()
                .all(|quad| segment_polygon(point, point, quad) >= margin)
    }

    /// The path graph: the streets' centre lines, a grid over the square, a node before
    /// every thing and lanes beside houses to their yards, joined where the walk between is
    /// clear, and every part joined to the rest.
    pub(super) fn paths(&mut self) {
        let mut nodes: Vec<(Vec2, On)> = Vec::new();
        let mut edges: BTreeSet<[u32; 2]> = BTreeSet::new();
        let edge = |edges: &mut BTreeSet<[u32; 2]>, a: u32, b: u32| {
            if a != b {
                edges.insert([a.min(b), a.max(b)]);
            }
        };
        // The square's grid.
        let (along, across) = (self.square.along, self.square.across);
        let outline = self.square.outline.clone();
        let (low_a, high_a) = extent(&outline, along);
        let (low_c, high_c) = extent(&outline, across);
        let mut grid: Vec<((i32, i32), u32)> = Vec::new();
        let (columns, rows) = (
            ((high_a - low_a) / GRID_M) as i32,
            ((high_c - low_c) / GRID_M) as i32,
        );
        for i in 0..=columns {
            for j in 0..=rows {
                let point = along * (low_a + (high_a - low_a) * i as f32 / columns.max(1) as f32)
                    + across * (low_c + (high_c - low_c) * j as f32 / rows.max(1) as f32);
                let edge_room = (0..outline.len())
                    .map(|k| point_segment(point, outline[k], outline[(k + 1) % outline.len()]))
                    .fold(f32::MAX, f32::min);
                if !inside(point, &outline) || edge_room < 1.0 || !self.standable(point, 0.8) {
                    continue;
                }
                grid.push(((i, j), nodes.len() as u32));
                nodes.push((point, On::Square));
            }
        }
        for &((i, j), a) in &grid {
            for (di, dj) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
                if let Some(&(_, b)) = grid.iter().find(|((x, y), _)| *x == i + di && *y == j + dj)
                    && self.walkable(nodes[a as usize].0, nodes[b as usize].0)
                {
                    edge(&mut edges, a, b);
                }
            }
        }
        // The streets.
        for (index, street) in self.streets.iter().enumerate() {
            let mut last: Option<u32> = None;
            for &point in &street.points {
                let node = nodes.len() as u32;
                nodes.push((point, On::Street(index)));
                if let Some(last) = last {
                    edge(&mut edges, last, node);
                }
                last = Some(node);
            }
        }
        let ways = nodes.len();
        // Joins a new node to the nearest of the streets' and the square's that it can walk
        // to, up to a few of them.
        let join = |town: &Town,
                    nodes: &[(Vec2, On)],
                    edges: &mut BTreeSet<[u32; 2]>,
                    node: u32,
                    most: usize| {
            let at = nodes[node as usize].0;
            let mut near: Vec<(f32, u32)> = (0..ways as u32)
                .map(|other| (nodes[other as usize].0.distance(at), other))
                .filter(|&(d, _)| d < 16.0)
                .collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut joined = 0;
            for (_, other) in near {
                if joined == most {
                    break;
                }
                if town.walkable(at, nodes[other as usize].0) {
                    edge(edges, node, other);
                    joined += 1;
                }
            }
            joined
        };
        // Each street's first point to the square.
        for street in 0..self.streets.len() {
            if let Some(first) = nodes.iter().position(|(_, on)| *on == On::Street(street)) {
                join(self, &nodes, &mut edges, first as u32, 3);
            }
        }
        // Before each thing, where someone coming to it arrives; a gate at its street's end.
        let stands: Vec<Vec2> = self.things.iter().map(|thing| self.stand(thing)).collect();
        for (index, stand) in stands.into_iter().enumerate() {
            // A gate is its street's last node; a place to gather is come to by the square's
            // nearest, so no one walks through its middle to get round it.
            let at_end = match self.things[index].kind {
                ThingKind::Gate => nodes.iter().position(|(point, on)| {
                    matches!(on, On::Street(_)) && point.distance(stand) < 0.01
                }),
                ThingKind::Gathering => nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, (_, on))| *on == On::Square)
                    .min_by(|a, b| a.1.0.distance(stand).total_cmp(&b.1.0.distance(stand)))
                    .map(|(node, _)| node),
                _ => None,
            };
            let node = match at_end {
                Some(node) => node as u32,
                None => {
                    let node = nodes.len() as u32;
                    nodes.push((stand, On::Thing(index)));
                    // A door between a building's wings first steps out past them.
                    let out = self.out_of(&self.things[index]);
                    match out {
                        Some(out) if self.walkable(stand, out) => {
                            let step = nodes.len() as u32;
                            nodes.push((out, On::Thing(index)));
                            edge(&mut edges, node, step);
                            if join(self, &nodes, &mut edges, step, 2) == 0 {
                                self.straight_across(&mut nodes, &mut edges, step);
                            }
                        }
                        _ => {
                            if join(self, &nodes, &mut edges, node, 2) == 0 {
                                self.straight_across(&mut nodes, &mut edges, node);
                            }
                        }
                    }
                    node
                }
            };
            self.things[index].node = node;
            // Its other places to stand, a stall's back.
            let thing = &self.things[index];
            for &extra in thing.kind.stands().iter().skip(1) {
                let point = thing.place(extra);
                let more = nodes.len() as u32;
                nodes.push((point, On::Thing(index)));
                join(self, &nodes, &mut edges, more, 2);
            }
        }
        // The lanes to the yards: on from the door's last step, to the yard's gate, and from
        // there to the yard's own things.
        for (home, lane) in self.lanes.clone() {
            let Some(door) = self
                .things
                .iter()
                .find(|t| t.kind == ThingKind::Door && t.building == Some(home))
                .map(|t| t.node)
            else {
                continue;
            };
            // The door's node, then its step out past its wings if it has one.
            let stepped = self
                .things
                .iter()
                .find(|t| t.kind == ThingKind::Door && t.building == Some(home))
                .and_then(|t| self.out_of(t));
            let skip = if stepped.is_some() && lane.get(1) == stepped.as_ref() {
                2
            } else {
                1
            };
            let mut last = if skip == 2 { door + 1 } else { door };
            let mut gate = last;
            for &point in &lane[skip..] {
                let node = nodes.len() as u32;
                nodes.push((point, On::Lane(home)));
                edge(&mut edges, last, node);
                last = node;
                gate = node;
            }
            for thing in &self.things {
                if thing.building == Some(home)
                    && thing.kind != ThingKind::Door
                    && self.walkable(nodes[gate as usize].0, nodes[thing.node as usize].0)
                {
                    edge(&mut edges, gate, thing.node);
                }
            }
        }
        self.paths = Paths {
            nodes,
            edges: edges.into_iter().collect(),
        };
        self.join_parts();
    }

    /// Joins any part of the path graph cut off from the largest to it, by the shortest
    /// clear walk between them.
    pub(super) fn join_parts(&mut self) {
        loop {
            let parts = self.paths.parts();
            let mut sizes = std::collections::BTreeMap::<usize, usize>::new();
            for &part in &parts {
                *sizes.entry(part).or_default() += 1;
            }
            let Some((&largest, _)) = sizes
                .iter()
                .max_by_key(|(part, size)| (**size, usize::MAX - **part))
            else {
                return;
            };
            if sizes.len() == 1 {
                return;
            }
            let mut best: Option<(f32, u32, u32)> = None;
            for (a, &part_a) in parts.iter().enumerate() {
                if part_a == largest {
                    continue;
                }
                for (b, &part_b) in parts.iter().enumerate() {
                    if part_b != largest {
                        continue;
                    }
                    let (pa, pb) = (self.paths.nodes[a].0, self.paths.nodes[b].0);
                    let d = pa.distance(pb);
                    if d < 30.0
                        && best.is_none_or(|(least, _, _)| d < least)
                        && self.walkable(pa, pb)
                    {
                        best = Some((d, a as u32, b as u32));
                    }
                }
            }
            match best {
                Some((_, a, b)) => self.paths.edges.push([a.min(b), a.max(b)]),
                None => return,
            }
        }
    }

    /// The lane from before a home's door, out past its wings if it has any, along one of
    /// its sides to its yard's gate, if either side is clear of everything built and every
    /// other yard.
    pub(super) fn lane(
        &self,
        home: usize,
        low: Vec2,
        high: Vec2,
        quad: &[Vec2; 4],
    ) -> Option<Vec<Vec2>> {
        let building = &self.buildings[home];
        let (at, facing) = (Vec2::from(building.at), building.facing);
        let door = self
            .things
            .iter()
            .find(|t| t.kind == ThingKind::Door && t.building == Some(home))?;
        let stand = self.stand(door);
        let mut start = vec![stand];
        start.extend(self.out_of(door).filter(|&out| self.walkable(stand, out)));
        let gate = (quad[2] + quad[3]) * 0.5 + door.front() * 0.8;
        for x in [high.x + 0.8, low.x - 0.8, high.x + 1.2, low.x - 1.2] {
            let front = place(at, facing, Vec2::new(x, high.y.max(0.0) + 0.9));
            let side = place(at, facing, Vec2::new(x, low.y - 0.8));
            let mut chain = start.clone();
            chain.extend([front, side, gate]);
            let clear = chain.windows(2).all(|pair| {
                self.walkable(pair[0], pair[1])
                    && self
                        .yards
                        .iter()
                        .all(|(_, yard)| segment_polygon(pair[0], pair[1], yard) >= 0.3)
            });
            if clear {
                return Some(chain);
            }
        }
        None
    }

    /// Where someone leaving a door between its building's wings steps out to, past them;
    /// nothing for a door in a plain front.
    pub(super) fn out_of(&self, thing: &Thing) -> Option<Vec2> {
        let building = &self.buildings[thing.building?];
        if thing.kind != ThingKind::Door {
            return None;
        }
        let forward = ground_rects(building)
            .iter()
            .map(|r| r.1.y)
            .fold(0.0f32, f32::max);
        (forward > 0.5).then(|| thing.place(Vec2::new(0.0, forward + 0.9)))
    }

    /// Where someone coming to a thing stands: before a door, a stall's counter, the well;
    /// at a pen's gate, between it and the house; on a yard, a place to gather, a post.
    pub(super) fn stand(&self, thing: &Thing) -> Vec2 {
        match thing.kind {
            ThingKind::Door => [0.0, -0.5, 0.5, -1.0, 1.0, -1.5, 1.5]
                .into_iter()
                .map(|aside| thing.place(Vec2::new(aside, 1.0)))
                .find(|&point| self.standable(point, CLEAR_M + 0.05))
                .unwrap_or_else(|| thing.place(Vec2::new(0.0, 1.0))),
            ThingKind::Pen => match self.yards.iter().find(|y| Some(y.0) == thing.building) {
                Some((_, quad)) => (quad[2] + quad[3]) * 0.5 + thing.front() * 0.8,
                None => thing.at,
            },
            ThingKind::Yard | ThingKind::Gathering | ThingKind::Gate | ThingKind::Post => thing.at,
            _ => thing.place(thing.kind.stands()[0]),
        }
    }
}

impl Paths {
    /// Which connected part each node is in, as the lowest node in it.
    pub fn parts(&self) -> Vec<usize> {
        let mut part: Vec<usize> = (0..self.nodes.len()).collect();
        fn root(part: &mut [usize], mut node: usize) -> usize {
            while part[node] != node {
                part[node] = part[part[node]];
                node = part[node];
            }
            node
        }
        for &[a, b] in &self.edges {
            let (ra, rb) = (root(&mut part, a as usize), root(&mut part, b as usize));
            if ra != rb {
                part[ra.max(rb)] = ra.min(rb);
            }
        }
        (0..self.nodes.len())
            .map(|node| root(&mut part, node))
            .collect()
    }
}
