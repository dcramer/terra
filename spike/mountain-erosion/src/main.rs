//! Spike (D133): stream-power erosion baked on a coarse grid round the settlement's range.
//! See the README for the question. Knobs by environment: KT (erodibility times time),
//! STEPS, M (area exponent), KDT (hillslope diffusion times time, m²), UPLIFT (share of the
//! range's height raised back over the run), SIZE and CELL (metres).
//!
//! Solver: Braun and Willett 2013's implicit scheme for n = 1 (from the paper), routed down
//! the steepest descent of a Priority-Flood fill with an epsilon (Barnes et al. 2014), the
//! law's terms as Schott et al. 2023's (https://github.com/H-Schott/StreamPowerErosion, MIT,
//! Copyright (c) 2023 Schott): stream power K A^m S^n less a hillslope Laplacian.

use glam::{Vec2, Vec3};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::time::Instant;
use terra_core::PlanetSpec;
use terra_gen::Seed;
use terra_land::region::Region;
use terra_land::{Land, PLANET_START, ground, lakes, start_climate};
use terra_voxel::{GeneratedPlanet, Mountains};

const D8: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
const SITE_KM: Vec2 = Vec2::new(2.22, 2.12);

fn env(name: &str, default: f32) -> f32 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[derive(PartialEq)]
struct Wait(f32, u32);
impl Eq for Wait {}
impl Ord for Wait {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
    }
}
impl PartialOrd for Wait {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// The routing a height grid gives: water levels (lakes filled flat), the epsilon fill that
/// drains every cell, each cell's receiver, the cells from outlets up, and drainage area (m²).
struct Routing {
    level: Vec<f32>,
    receiver: Vec<u32>,
    order: Vec<u32>,
    area: Vec<f32>,
}

fn route(h: &[f32], n: usize, cell: f32, p_flow: f32) -> Routing {
    let mut level = h.to_vec();
    let mut fill = h.to_vec();
    let mut done = vec![false; n * n];
    let mut heap = BinaryHeap::new();
    for k in 0..n * n {
        let (x, y) = (k % n, k / n);
        if x == 0 || y == 0 || x == n - 1 || y == n - 1 {
            done[k] = true;
            heap.push(Wait(fill[k], k as u32));
        }
    }
    while let Some(Wait(f, k)) = heap.pop() {
        let (x, y) = ((k as usize % n) as i32, (k as usize / n) as i32);
        for (dx, dy) in D8 {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= n as i32 || ny >= n as i32 {
                continue;
            }
            let j = ny as usize * n + nx as usize;
            if done[j] {
                continue;
            }
            done[j] = true;
            level[j] = level[j].max(level[k as usize]);
            fill[j] = fill[j].max(f + 1e-3);
            heap.push(Wait(fill[j], j as u32));
        }
    }
    let receiver: Vec<u32> = (0..n * n)
        .map(|k| {
            let (x, y) = ((k % n) as i32, (k / n) as i32);
            if x == 0 || y == 0 || x == n as i32 - 1 || y == n as i32 - 1 {
                return k as u32;
            }
            let mut best = (0.0, k as u32);
            for (dx, dy) in D8 {
                let j = (y + dy) as usize * n + (x + dx) as usize;
                let s = (fill[k] - fill[j]) / (cell * ((dx * dx + dy * dy) as f32).sqrt());
                if s > best.0 {
                    best = (s, j as u32);
                }
            }
            best.1
        })
        .collect();
    let mut order: Vec<u32> = (0..(n * n) as u32).collect();
    order.sort_unstable_by(|&a, &b| fill[a as usize].total_cmp(&fill[b as usize]));
    // Drainage area: down the steepest way (D8), or with PFLOW > 0 spread over every lower
    // neighbour by slope to that power, as Schott et al.'s WaterLp does.

    let mut area = vec![cell * cell; n * n];
    for &k in order.iter().rev() {
        let k = k as usize;
        let r = receiver[k] as usize;
        if r == k {
            continue;
        }
        if p_flow <= 0.0 {
            area[r] += area[k];
            continue;
        }
        let (x, y) = ((k % n) as i32, (k / n) as i32);
        let mut lower = [(0usize, 0.0f32); 8];
        let mut sum = 0.0;
        for (i, (dx, dy)) in D8.iter().enumerate() {
            let j = (y + dy) as usize * n + (x + dx) as usize;
            let s = (fill[k] - fill[j]) / (cell * ((dx * dx + dy * dy) as f32).sqrt());
            if s > 0.0 {
                let w = libm::powf(s, p_flow);
                lower[i] = (j, w);
                sum += w;
            }
        }
        let a = area[k];
        for (j, w) in lower {
            if w > 0.0 {
                area[j] += a * w / sum;
            }
        }
    }
    Routing { level, receiver, order, area }
}

fn erode(h0: &[f32], n: usize, cell: f32) -> Vec<f32> {
    let (kt, steps, m, kdt, uplift) = (env("KT", 0.5), env("STEPS", 24.0) as usize, env("M", 0.5), env("KDT", 3000.0), env("UPLIFT", 0.0));
    let (kdt_step, kd_step) = (kt / steps as f32, kdt / steps as f32 / (cell * cell));
    let raise: Vec<f32> = h0.iter().map(|&v| uplift * (v - 950.0).max(0.0) / steps as f32).collect();
    // Erodibility varied cell by cell, as Schott et al.'s noise factor does, so flow down a
    // plane slope does not cut parallel rills all of one depth.
    let noise = env("NOISE", 0.5);
    let k_of: Vec<f32> = (0..n * n)
        .map(|k| 1.0 + noise * (terra_gen::hash::unit(terra_gen::hash::lowbias32(k as u32 ^ 0x9e37_79b9)) - 0.5))
        .collect();
    // FLAT < 1 starts from the range lowered to that share of its height over the valleys, the
    // rest raised back over the run (UPLIFT): the network forms as it rises, not inherited.
    let flat = env("FLAT", 1.0);
    let mut h: Vec<f32> = h0.iter().map(|&v| if v > 950.0 { 950.0 + flat * (v - 950.0) } else { v }).collect();
    // The lakes the ground had: their beds are left as they are, so they keep their water.
    let start = route(h0, n, cell, 0.0);
    let lake: Vec<bool> = (0..n * n).map(|k| start.level[k] > h0[k] + 0.05).collect();
    for _ in 0..steps {
        for (v, u) in h.iter_mut().zip(&raise) {
            *v += u;
        }
        // Hillslope diffusion first, so the step ends on the stream power's pass, which leaves
        // every eroded cell above the one it drains to.
        // Explicit, so in substeps under the stability limit (a quarter); never on a lake or
        // beside one, where raising its outlet would raise its water.
        let subs = (kd_step / 0.2).ceil().max(1.0) as usize;
        for _ in 0..subs {
            let before = h.clone();
            for y in 1..n - 1 {
                for x in 1..n - 1 {
                    let k = y * n + x;
                    if lake[k] || lake[k - 1] || lake[k + 1] || lake[k - n] || lake[k + n] {
                        continue;
                    }
                    let lap = before[k - 1] + before[k + 1] + before[k - n] + before[k + n] - 4.0 * before[k];
                    h[k] += kd_step / subs as f32 * lap;
                }
            }
        }
        let r = route(&h, n, cell, env("PFLOW", 0.0));
        for &k in &r.order {
            let (k, rc) = (k as usize, r.receiver[k as usize] as usize);
            if rc == k || lake[k] {
                continue;
            }
            let base = if lake[rc] { h[rc].max(r.level[rc]) } else { h[rc] };
            if h[k] <= base {
                // A hollow the step made (uplift or diffusion), not a lake the ground had:
                // filled with sediment to drain over its receiver, as FastScape fills its
                // depressions, so no new pit is left to hold water.
                h[k] = base + 0.01;
                continue;
            }
            let (dx, dy) = ((k % n) as f32 - (rc % n) as f32, (k / n) as f32 - (rc / n) as f32);
            let f = kdt_step * k_of[k] * r.area[k].powf(m) / (cell * (dx * dx + dy * dy).sqrt());
            h[k] = (h[k] + f * base) / (1.0 + f);
        }
    }
    h
}

/// What the measures say of a grid within a mask.
fn measure(name: &str, h: &[f32], region: &Region, mask: &[bool]) -> Vec<Option<u32>> {
    let (n, cell) = (region.cells, region.cell_m);
    let r = route(h, n, cell, 0.0);
    let channel = |k: usize| r.area[k] >= 0.05e6;
    let mut order = vec![0u8; n * n];
    let mut top = vec![(0u8, 0u8); n * n];
    let mut confluences = 0;
    let mut by_order = [0usize; 8];
    for &k in r.order.iter().rev() {
        let k = k as usize;
        if !channel(k) {
            continue;
        }
        let (best, count) = top[k];
        order[k] = if best == 0 { 1 } else if count >= 2 { best + 1 } else { best };
        let rc = r.receiver[k] as usize;
        if rc != k && channel(rc) {
            let t = &mut top[rc];
            if order[k] > t.0 {
                *t = (order[k], 1);
            } else if order[k] == t.0 {
                t.1 += 1;
            }
        }
    }
    let mut donors = vec![0u8; n * n];
    for k in 0..n * n {
        let rc = r.receiver[k] as usize;
        if rc != k && channel(k) {
            donors[rc] += 1;
        }
    }
    let masked: usize = mask.iter().filter(|&&m| m).count();
    for k in 0..n * n {
        if mask[k] && channel(k) {
            if donors[k] >= 2 {
                confluences += 1;
            }
            by_order[order[k].min(7) as usize] += 1;
        }
    }
    let mut relief = (0.0, 0);
    for y in (4..n - 4).step_by(3) {
        for x in (4..n - 4).step_by(3) {
            if !mask[y * n + x] {
                continue;
            }
            let (mut lo, mut hi) = (f32::MAX, f32::MIN);
            for j in y - 4..=y + 4 {
                for i in x - 4..=x + 4 {
                    lo = lo.min(h[j * n + i]);
                    hi = hi.max(h[j * n + i]);
                }
            }
            relief = (relief.0 + hi - lo, relief.1 + 1);
        }
    }
    let mut copy = region.clone();
    copy.heights = h.to_vec();
    let found = lakes::fill(&copy, Some(ground::SEA_LEVEL_M));
    let kept: Vec<_> = found.lakes.iter().filter(|l| l.cells >= 3 && l.deepest_m >= 1.0).collect();
    let km2 = masked as f32 * cell * cell / 1e6;
    let max_order = (1..8).rev().find(|&o| by_order[o] > 0).unwrap_or(0);
    println!(
        "{name}: wall {km2:.1} km²; channels by Strahler order 1-6: {:?} cells, highest {max_order}; {:.2} confluences a km²; relief within 290 m {:.0} m; lakes {} ({} of 3+ cells and 1 m+ deep, {:.2} km²)",
        &by_order[1..7],
        confluences as f32 / km2,
        relief.0 / relief.1.max(1) as f32,
        found.lakes.len(),
        kept.len(),
        kept.iter().map(|l| l.area_m2).sum::<f32>() / 1e6,
    );
    found.lake
}

struct View<'a> {
    region: &'a Region,
    radius: f32,
    up: Vec3,
}

impl View<'_> {
    fn local(&self, p: Vec3) -> Option<Vec2> {
        let d = p.normalize();
        let along = d.dot(self.up);
        let q = d * (self.radius / along) - self.region.middle;
        let at = Vec2::new(q.dot(self.region.east), q.dot(self.region.north));
        let half = self.region.cells as f32 * 0.5 * self.region.cell_m;
        (along > 0.0 && at.x.abs() < half && at.y.abs() < half).then_some(at)
    }
    fn clearance(&self, p: Vec3) -> Option<f32> {
        self.local(p).map(|at| p.length() - self.radius - self.region.height_at(at))
    }
    fn normal(&self, p: Vec3) -> Vec3 {
        let at = self.local(p).unwrap_or_default();
        let c = self.region.cell_m;
        let hx = |o: Vec2| self.region.height_at(at + o);
        let gx = (hx(Vec2::new(c, 0.0)) - hx(Vec2::new(-c, 0.0))) / (2.0 * c);
        let gy = (hx(Vec2::new(0.0, c)) - hx(Vec2::new(0.0, -c))) / (2.0 * c);
        let d = p.normalize();
        let e = (self.region.east - d * d.dot(self.region.east)).normalize();
        let nn = (self.region.north - d * d.dot(self.region.north)).normalize();
        (d - e * gx - nn * gy).normalize()
    }
}

fn render(region: &Region, radius: f32, w: usize, hgt: usize) -> Vec<u8> {
    let up = region.middle.normalize();
    let view = View { region, radius, up };
    let eye = up * (radius + region.height_at(Vec2::ZERO) + 60.0);
    let forward = (region.north * 1500.0 + up * 10.0).normalize();
    let right = region.east;
    let cam_up = right.cross(forward).normalize() * -1.0;
    let cam_up = if cam_up.dot(up) < 0.0 { -cam_up } else { cam_up };
    let (az, el) = (95f32.to_radians(), 20f32.to_radians());
    let sun = ((region.east * libm::sinf(az) + region.north * libm::cosf(az)) * libm::cosf(el) + up * libm::sinf(el)).normalize();
    let half = libm::tanf(30f32.to_radians());
    let horizon = Vec3::new(0.74, 0.8, 0.88);
    let rows: Vec<usize> = (0..hgt).collect();
    let lines = terra_gen::par::map(&rows, |&y| {
        let mut line = Vec::with_capacity(w * 3);
        for x in 0..w {
            let sx = (2.0 * (x as f32 + 0.5) / w as f32 - 1.0) * half;
            let sy = (1.0 - 2.0 * (y as f32 + 0.5) / hgt as f32) * half * hgt as f32 / w as f32;
            let ray = (forward + right * sx + cam_up * sy).normalize();
            let sky = horizon.lerp(Vec3::new(0.36, 0.55, 0.85), ray.dot(up).clamp(0.0, 0.4) / 0.4);
            let mut t = 20.0;
            let mut hit = None;
            while t < 32_000.0 {
                let Some(c) = view.clearance(eye + ray * t) else { break };
                if c < 0.0 {
                    let (mut lo, mut hi) = (t - 4.0f32.max(0.0), t);
                    for _ in 0..8 {
                        let mid = 0.5 * (lo + hi);
                        if view.clearance(eye + ray * mid).unwrap_or(1.0) < 0.0 { hi = mid } else { lo = mid }
                    }
                    hit = Some(hi);
                    break;
                }
                t += (c * 0.35).clamp(3.0, 150.0);
            }
            let colour = match hit {
                None => sky,
                Some(t) => {
                    let p = eye + ray * t;
                    let nrm = view.normal(p);
                    let height = p.length() - radius;
                    let slope = libm::acosf(nrm.dot(p.normalize()).clamp(-1.0, 1.0)).to_degrees();
                    let albedo = if slope > 38.0 {
                        Vec3::new(0.27, 0.26, 0.25)
                    } else if height > 2250.0 {
                        Vec3::new(0.86, 0.88, 0.92)
                    } else if height > 1750.0 {
                        Vec3::new(0.40, 0.40, 0.34)
                    } else {
                        Vec3::new(0.12, 0.19, 0.09)
                    };
                    let mut lit = nrm.dot(sun).max(0.0);
                    if lit > 0.0 {
                        let mut s = 5.0;
                        while s < 12_000.0 {
                            match view.clearance(p + nrm * 2.0 + sun * s) {
                                Some(c) if c < 0.0 => {
                                    lit = 0.0;
                                    break;
                                }
                                Some(c) => s += (c * 0.5).clamp(8.0, 200.0),
                                None => break,
                            }
                        }
                    }
                    let c = albedo * (Vec3::new(0.24, 0.27, 0.34) + Vec3::splat(1.15 * lit));
                    let through = libm::expf(-t / 14_000.0);
                    c * through + horizon * 0.92 * (1.0 - through)
                }
            };
            for v in colour.to_array() {
                line.push((libm::powf(v.clamp(0.0, 1.0), 1.0 / 2.2) * 255.0) as u8);
            }
        }
        line
    });
    lines.concat()
}

/// The map: a hillshade of the window round the middle, its channels over 0.25 km² in blue,
/// the deeper the bigger, and its lakes in cyan.
fn map(h: &[f32], region: &Region, lake: &[Option<u32>], side: usize) -> Vec<u8> {
    let (n, cell) = (region.cells, region.cell_m);
    let r = route(h, n, cell, 0.0);
    let start = (n - side) / 2;
    let light = Vec3::new(-0.6, 0.6, 0.55).normalize();
    let mut out = Vec::with_capacity(side * side * 3);
    for y in start..start + side {
        for x in start..start + side {
            let k = y * n + x;
            let gx = (h[k + 1] - h[k - 1]) / (2.0 * cell);
            let gy = (h[k - n] - h[k + n]) / (2.0 * cell);
            let shade = Vec3::new(-gx, -gy, 1.0).normalize().dot(light).max(0.0);
            let tone = 0.15 + 0.85 * shade;
            let mut c = Vec3::splat(tone) * Vec3::new(1.0, 0.97, 0.92);
            if lake[k].is_some() {
                c = Vec3::new(0.2, 0.75, 0.9);
            } else if r.area[k] > 0.25e6 {
                let s = (libm::log10f(r.area[k] / 0.25e6) / 2.0).clamp(0.25, 1.0);
                c = c.lerp(Vec3::new(0.1, 0.3, 1.0), s);
            }
            for v in c.to_array() {
                out.push((v.clamp(0.0, 1.0) * 255.0) as u8);
            }
        }
    }
    out
}

fn save(path: &str, w: usize, h: usize, rgb: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.write_header().unwrap().write_image_data(rgb).unwrap();
}

fn main() {
    let (size, cell) = (env("SIZE", 30_000.0), env("CELL", 32.0));
    let spec = PlanetSpec { seed: Seed(1), radius_m: 30_000.0 };
    let climate = start_climate().unwrap();
    let full = Land::new(ground::world(spec), climate.clone());
    let smooth = Mountains { gullies_m: 0.0, couloirs_m: 0.0, ..Mountains::START };
    let planet = GeneratedPlanet::new(spec, ground::RELIEF_M, Some(ground::SEA_LEVEL_M));
    let base_land = Land::new(planet.with_mountains(smooth), climate);
    let toward = full.along(PLANET_START, SITE_KM * 1000.0);
    let started = Instant::now();
    let before = Region::sample(&full, toward, size, cell);
    let base = Region::sample(&base_land, toward, size, cell);
    let sampled = started.elapsed().as_secs_f32();
    let n = before.cells;
    let started = Instant::now();
    let eroded = erode(&base.heights, n, cell);
    let baked = started.elapsed().as_secs_f32();
    // The per-point gullies laid back over the eroded ground, as the plan has them.
    let after: Vec<f32> = (0..n * n).map(|k| eroded[k] + before.heights[k] - base.heights[k]).collect();
    println!(
        "grid {n}² of {cell} m ({:.1} km across): sampled twice in {sampled:.1} s, eroded in {baked:.1} s (KT {} STEPS {} M {} KDT {} UPLIFT {})",
        size / 1000.0,
        env("KT", 0.5),
        env("STEPS", 24.0),
        env("M", 0.5),
        env("KDT", 3000.0),
        env("UPLIFT", 0.0)
    );
    let cut: Vec<f32> = (0..n * n).map(|k| base.heights[k] - eroded[k]).collect();
    let deepest = cut.iter().cloned().fold(f32::MIN, f32::max);
    let raised = cut.iter().cloned().fold(f32::MAX, f32::min);
    println!("cut: deepest {deepest:.0} m, most raised {:.0} m", -raised);
    // The wall: steep ground over the valley within 8 km of the settlement.
    let mask: Vec<bool> = (0..n * n)
        .map(|k| {
            let (x, y) = (k % n, k / n);
            let off = before.offset(x, y);
            let g = before.gradient(x.clamp(1, n - 2), y.clamp(1, n - 2)).length();
            off.length() < 8000.0 && g > 0.36 && before.heights[k] > 1100.0
        })
        .collect();
    std::fs::create_dir_all("out").unwrap();
    let lakes_before = measure("before", &before.heights, &before, &mask);
    let lakes_base = measure("without gullies", &base.heights, &base, &mask);
    let lakes_eroded = measure("eroded", &eroded, &base, &mask);
    let lakes_after = measure("after (eroded + gullies)", &after, &before, &mask);
    // Water where there was none: in a lake the ground had, grown, or in a hollow of its own.
    let changed = |old: &[Option<u32>], new: &[Option<u32>], what: &str| {
        let mut wet_before = std::collections::HashSet::new();
        for k in 0..n * n {
            if let (Some(l), Some(_)) = (new[k], old[k]) {
                wet_before.insert(l);
            }
        }
        let fresh: Vec<usize> = (0..n * n).filter(|&k| new[k].is_some() && old[k].is_none()).collect();
        let grown = fresh.iter().filter(|&&k| wet_before.contains(&new[k].unwrap())).count();
        let hollows: std::collections::HashSet<u32> = fresh.iter().filter_map(|&k| new[k]).filter(|l| !wet_before.contains(l)).collect();
        let a = cell * cell / 1e6;
        if env("WHERE", 0.0) > 0.0 {
            for l in hollows.iter().take(6) {
                let cells: Vec<usize> = fresh.iter().cloned().filter(|&k| new[k] == Some(*l)).collect();
                let (sx, sy) = cells.iter().fold((0, 0), |s, &k| (s.0 + k % n, s.1 + k / n));
                println!("  hollow {l}: {} cells about column {} row {}", cells.len(), sx / cells.len(), sy / cells.len());
            }
        }
        println!(
            "{what}: lakes the ground had grown by {:.2} km²; {} new hollows holding water, {:.3} km²",
            grown as f32 * a,
            hollows.len(),
            (fresh.len() - grown) as f32 * a
        );
    };
    changed(&lakes_base, &lakes_eroded, "eroded against the ground without gullies");
    changed(&lakes_before, &lakes_after, "after against before");
    if env("NOVIEW", 0.0) > 0.0 {
        return;
    }
    let (w, hgt) = (960, 540);
    let mut shown = before.clone();
    save("out/before-view.png", w, hgt, &render(&shown, spec.radius_m, w, hgt));
    save("out/before-map.png", 540, 540, &map(&before.heights, &before, &lakes_before, 540.min(n - 2)));
    shown.heights = after.clone();
    save("out/after-view.png", w, hgt, &render(&shown, spec.radius_m, w, hgt));
    save("out/after-map.png", 540, 540, &map(&after, &before, &lakes_after, 540.min(n - 2)));
}
