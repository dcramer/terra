//! The planet's cells: the vertices of a subdivided icosahedron. Every cell has five or
//! six neighbours and all cells are close to the same size.

use std::collections::HashMap;

pub struct Grid {
    positions: Vec<[f32; 3]>,
    latitudes: Vec<f32>,
    neighbours: Vec<u32>,
    neighbour_start: Vec<u32>,
    triangles: Vec<u32>,
}

impl Grid {
    /// `subdivisions` of 4 gives 2,562 cells, 5 gives 10,242 and 6 gives 40,962.
    pub fn icosphere(subdivisions: u32) -> Self {
        let t = (1.0 + 5.0f32.sqrt()) / 2.0;
        let mut positions: Vec<[f32; 3]> = [
            [-1.0, t, 0.0], [1.0, t, 0.0], [-1.0, -t, 0.0], [1.0, -t, 0.0],
            [0.0, -1.0, t], [0.0, 1.0, t], [0.0, -1.0, -t], [0.0, 1.0, -t],
            [t, 0.0, -1.0], [t, 0.0, 1.0], [-t, 0.0, -1.0], [-t, 0.0, 1.0],
        ]
        .iter()
        .map(|&p| normalise(p))
        .collect();
        let mut triangles: Vec<[u32; 3]> = vec![
            [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
            [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
            [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
            [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
        ];

        for _ in 0..subdivisions {
            let mut midpoints: HashMap<(u32, u32), u32> = HashMap::new();
            let mut midpoint = |a: u32, b: u32, positions: &mut Vec<[f32; 3]>| {
                *midpoints.entry((a.min(b), a.max(b))).or_insert_with(|| {
                    let (p, q) = (positions[a as usize], positions[b as usize]);
                    positions.push(normalise([p[0] + q[0], p[1] + q[1], p[2] + q[2]]));
                    positions.len() as u32 - 1
                })
            };
            let mut next = Vec::with_capacity(triangles.len() * 4);
            for [a, b, c] in triangles {
                let ab = midpoint(a, b, &mut positions);
                let bc = midpoint(b, c, &mut positions);
                let ca = midpoint(c, a, &mut positions);
                next.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
            }
            triangles = next;
        }

        let mut lists: Vec<Vec<u32>> = vec![Vec::new(); positions.len()];
        for &[a, b, c] in &triangles {
            for (from, to) in [(a, b), (b, c), (c, a), (b, a), (c, b), (a, c)] {
                if !lists[from as usize].contains(&to) {
                    lists[from as usize].push(to);
                }
            }
        }
        let mut neighbours = Vec::new();
        let mut neighbour_start = Vec::with_capacity(positions.len() + 1);
        for list in &lists {
            neighbour_start.push(neighbours.len() as u32);
            neighbours.extend(list);
        }
        neighbour_start.push(neighbours.len() as u32);

        Grid {
            latitudes: positions.iter().map(|p| p[1].clamp(-1.0, 1.0).asin()).collect(),
            positions,
            neighbours,
            neighbour_start,
            triangles: triangles.into_iter().flatten().collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.positions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Unit vector from the planet's centre to the cell.
    pub fn position(&self, cell: usize) -> [f32; 3] {
        self.positions[cell]
    }

    /// Latitude in radians, with the poles on the Y axis.
    pub fn latitude(&self, cell: usize) -> f32 {
        self.latitudes[cell]
    }

    pub fn neighbours(&self, cell: usize) -> &[u32] {
        &self.neighbours[self.neighbour_start[cell] as usize..self.neighbour_start[cell + 1] as usize]
    }

    pub fn neighbour_mean(&self, cell: usize, values: &[f32]) -> f32 {
        let list = self.neighbours(cell);
        list.iter().map(|&i| values[i as usize]).sum::<f32>() / list.len() as f32
    }

    /// Cell indices, three per triangle, for drawing the globe.
    pub fn triangles(&self) -> &[u32] {
        &self.triangles
    }

    /// The cell closest to a direction, found by walking downhill from `start`. Much
    /// faster than `nearest` when `start` is already close.
    pub fn nearest_from(&self, start: usize, direction: [f32; 3]) -> usize {
        let d = normalise(direction);
        let along = |i: usize| {
            let p = self.positions[i];
            p[0] * d[0] + p[1] * d[1] + p[2] * d[2]
        };
        let mut best = (start, along(start));
        loop {
            let mut improved = false;
            for &nb in self.neighbours(best.0) {
                let a = along(nb as usize);
                if a > best.1 {
                    best = (nb as usize, a);
                    improved = true;
                }
            }
            if !improved {
                return best.0;
            }
        }
    }

    /// The cell closest to a direction from the planet's centre.
    pub fn nearest(&self, direction: [f32; 3]) -> usize {
        let d = normalise(direction);
        let mut best = (0, f32::MIN);
        for (i, p) in self.positions.iter().enumerate() {
            let along = p[0] * d[0] + p[1] * d[1] + p[2] * d[2];
            if along > best.1 {
                best = (i, along);
            }
        }
        best.0
    }
}

fn normalise(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
    [v[0] / len, v[1] / len, v[2] / len]
}
