//! Prototype planetary simulation for the simulation toy (docs/prototypes.md, build 2).
//!
//! The planet is a sphere of cells. Each cell holds a few local values; a few more are
//! global. Machines change one or the other, and local changes add up to the planet.
//! Every rule and number here is a first draft meant to be tuned by playing with it.
//!
//! Pure logic: no engine, no I/O, deterministic for a given seed and sequence of calls.
//! Trigonometry goes through `libm` so every platform computes the same planet (D30).

mod grid;

pub use grid::Grid;

/// Height range of the terrain around the nominal radius, in metres.
const RELIEF_M: f32 = 150.0;

const HEATER_RADIUS_RAD: f32 = 0.15;
const HEATER_PEAK_C: f32 = 40.0;
const GAS_PER_TICK: f32 = 0.0002;
/// Share of the atmosphere lost to space each tick. With the rate above, each generator
/// holds up about 0.4 of pressure, so how many you run sets the planet's climate.
const GAS_LOSS: f32 = 0.0005;
const START_PRESSURE: f32 = 0.01;
const MAX_PRESSURE: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Machine {
    /// Warms the ground around it. Local.
    Heater,
    /// Thickens the atmosphere, which warms the whole planet. Global.
    GasGenerator,
    /// Plants life in its cell, which then spreads by itself where it can survive. Local.
    Seeder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Dead,
    Thawing,
    Wet,
    Green,
    Breathable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Surface,
    Temperature,
    Moisture,
    Biomass,
}

#[derive(Clone, Copy, Debug)]
pub struct Summary {
    pub tick: u64,
    pub pressure: f32,
    pub mean_temperature: f32,
    pub sea_level_m: f32,
    pub ocean_fraction: f32,
    pub ice_fraction: f32,
    pub mean_land_biomass: f32,
    pub oxygen: f32,
    pub phase: Phase,
}

/// One wave of the terrain: `amplitude_m * sin(frequency * dot(direction, p) + phase)`,
/// where `p` is the unit vector from the planet's centre.
#[derive(Clone, Copy, Debug)]
pub struct Wave {
    pub direction: [f32; 3],
    pub frequency: f32,
    pub phase: f32,
    pub amplitude_m: f32,
}

pub struct Sim {
    pub grid: Grid,
    /// The terrain's shape. Anything that draws the ground uses these, so that it
    /// agrees with `elevation`.
    pub waves: Vec<Wave>,
    /// Ground height relative to the nominal radius, in metres. Never changes.
    pub elevation: Vec<f32>,
    pub temperature: Vec<f32>,
    /// Ice on the cell, in metres of water.
    pub ice: Vec<f32>,
    pub moisture: Vec<f32>,
    pub biomass: Vec<f32>,
    heat_source: Vec<f32>,
    /// Temperature from sunlight alone, by latitude. Fixed, so worked out once.
    sunlight: Vec<f32>,
    seeded: Vec<bool>,
    gas_generators: u32,
    pub pressure: f32,
    pub oxygen: f32,
    /// Melted water, in metres averaged over the whole planet.
    pub water: f32,
    pub sea_level_m: f32,
    pub tick: u64,
    sorted_elevation: Vec<f32>,
    scratch: Vec<f32>,
}

impl Sim {
    pub fn new(subdivisions: u32, seed: u64) -> Self {
        let grid = Grid::icosphere(subdivisions);
        let n = grid.len();
        let (waves, elevation) = terrain(&grid, seed);
        let mut sorted_elevation = elevation.clone();
        sorted_elevation.sort_by(|a, b| a.total_cmp(b));

        // A dead planet: a thin frost everywhere and thick caps at the poles.
        let ice = (0..n)
            .map(|i| {
                let lat = grid.latitude(i).abs().to_degrees();
                20.0 + 150.0 * smoothstep(50.0, 80.0, lat)
            })
            .collect();

        let mut sim = Sim {
            temperature: vec![0.0; n],
            ice,
            moisture: vec![0.0; n],
            biomass: vec![0.0; n],
            heat_source: vec![0.0; n],
            sunlight: (0..n)
                .map(|i| -62.0 + 47.0 * libm::cosf(grid.latitude(i)))
                .collect(),
            seeded: vec![false; n],
            gas_generators: 0,
            pressure: START_PRESSURE,
            oxygen: 0.0,
            water: 0.0,
            sea_level_m: sorted_elevation[0],
            tick: 0,
            scratch: vec![0.0; n],
            sorted_elevation,
            elevation,
            waves,
            grid,
        };
        for i in 0..n {
            sim.temperature[i] = sim.equilibrium_temperature(i);
        }
        sim
    }

    pub fn place(&mut self, machine: Machine, cell: usize) {
        match machine {
            Machine::GasGenerator => self.gas_generators += 1,
            Machine::Seeder => self.seeded[cell] = true,
            Machine::Heater => {
                let centre = self.grid.position(cell);
                for i in 0..self.grid.len() {
                    let angle = libm::acosf(dot(centre, self.grid.position(i)).clamp(-1.0, 1.0));
                    if angle < HEATER_RADIUS_RAD {
                        let falloff = 1.0 - angle / HEATER_RADIUS_RAD;
                        self.heat_source[i] += HEATER_PEAK_C * falloff;
                    }
                }
            }
        }
    }

    pub fn is_ocean(&self, cell: usize) -> bool {
        self.elevation[cell] < self.sea_level_m
    }

    /// The temperature a cell settles at if left alone: sunlight by latitude, plus the
    /// greenhouse effect of the atmosphere, cooler on high ground, plus any heaters.
    fn equilibrium_temperature(&self, cell: usize) -> f32 {
        let greenhouse = 60.0 * self.pressure / (self.pressure + 0.35);
        self.sunlight[cell] + greenhouse - 0.0065 * self.elevation[cell] + self.heat_source[cell]
    }

    pub fn step(&mut self) {
        let n = self.grid.len();
        self.pressure +=
            GAS_PER_TICK * self.gas_generators as f32 - GAS_LOSS * (self.pressure - START_PRESSURE);
        self.pressure = self.pressure.min(MAX_PRESSURE);

        // Temperature: drift toward equilibrium and even out with neighbours.
        for i in 0..n {
            let t = self.temperature[i];
            let mean = self.grid.neighbour_mean(i, &self.temperature);
            self.scratch[i] = t + 0.05 * (self.equilibrium_temperature(i) - t) + 0.1 * (mean - t);
        }
        std::mem::swap(&mut self.temperature, &mut self.scratch);

        // Ice melts where it is warm, and the meltwater joins the one global sea.
        let mut melted = 0.0;
        for i in 0..n {
            if self.temperature[i] > 0.0 && self.ice[i] > 0.0 {
                let melt = (0.02 * self.temperature[i]).min(self.ice[i]);
                self.ice[i] -= melt;
                melted += melt;
            }
        }
        if melted > 0.0 {
            self.water += melted / n as f32;
            self.sea_level_m = self.level_for_water(self.water);
        }

        // Moisture: open sea is the source; air carries it inland, and thin air carries
        // little. Land dries out, more slowly where plants hold the water.
        let carry = 0.15 * (self.pressure * 2.0).min(1.0);
        for i in 0..n {
            self.scratch[i] = if self.is_ocean(i) {
                if self.temperature[i] > -2.0 { 1.0 } else { 0.0 }
            } else {
                let m = self.moisture[i];
                let mean = self.grid.neighbour_mean(i, &self.moisture);
                let drying = 0.004 * (1.0 - 0.7 * self.biomass[i]);
                (m + carry * (mean - m) - drying * m).clamp(0.0, 1.0)
            };
        }
        std::mem::swap(&mut self.moisture, &mut self.scratch);

        // Life: grows where it can survive, creeps into neighbouring cells, dies back
        // where it cannot.
        let mut land = 0u32;
        let mut total_biomass = 0.0;
        for i in 0..n {
            let b = self.biomass[i];
            self.scratch[i] = if self.is_ocean(i) {
                0.0
            } else {
                land += 1;
                let suitability = self.suitability(i);
                let mut next = b;
                if suitability > 0.0 {
                    if self.seeded[i] {
                        next = next.max(0.2);
                    }
                    let nearby = self.grid.neighbour_mean(i, &self.biomass);
                    next += 0.04 * next * (1.0 - next) * suitability;
                    // One thriving neighbour out of six is enough to take hold.
                    if nearby > 0.1 {
                        next += 0.005 * nearby * suitability;
                    }
                } else {
                    next *= 0.98;
                }
                next.clamp(0.0, 1.0)
            };
            total_biomass += self.scratch[i];
        }
        std::mem::swap(&mut self.biomass, &mut self.scratch);

        // Oxygen follows how much of the land is alive.
        let cover = total_biomass / (land.max(1) as f32 * 0.5);
        let target = 0.21 * cover.min(1.0);
        self.oxygen += 0.001 * (target - self.oxygen);

        self.tick += 1;
    }

    /// How well life does in a cell, 0 to 1. Zero means it cannot survive there.
    fn suitability(&self, cell: usize) -> f32 {
        let t = self.temperature[cell];
        if !(2.0..=40.0).contains(&t) || self.moisture[cell] < 0.15 || self.pressure < 0.2 {
            return 0.0;
        }
        let warmth = 1.0 - ((t - 22.0) / 20.0).abs().min(1.0);
        self.moisture[cell] * (0.2 + 0.8 * warmth)
    }

    /// Sea level that holds a given amount of water, filling the lowest ground first.
    fn level_for_water(&self, water: f32) -> f32 {
        let n = self.sorted_elevation.len();
        let target = water * n as f32;
        let mut below = 0.0;
        for k in 1..n {
            let step = (self.sorted_elevation[k] - self.sorted_elevation[k - 1]) * k as f32;
            if below + step >= target {
                return self.sorted_elevation[k - 1] + (target - below) / k as f32;
            }
            below += step;
        }
        self.sorted_elevation[n - 1] + (target - below) / n as f32
    }

    pub fn summary(&self) -> Summary {
        let n = self.grid.len() as f32;
        let ocean = (0..self.grid.len()).filter(|&i| self.is_ocean(i)).count() as f32;
        let iced = self.ice.iter().filter(|&&x| x > 1.0).count() as f32;
        let land_biomass: f32 = self.biomass.iter().sum::<f32>() / (n - ocean).max(1.0);
        let ocean_fraction = ocean / n;
        let phase = if self.oxygen >= 0.15 {
            Phase::Breathable
        } else if land_biomass >= 0.05 {
            Phase::Green
        } else if ocean_fraction >= 0.05 {
            Phase::Wet
        } else if self.pressure >= 0.15 {
            Phase::Thawing
        } else {
            Phase::Dead
        };
        Summary {
            tick: self.tick,
            pressure: self.pressure,
            mean_temperature: self.temperature.iter().sum::<f32>() / n,
            sea_level_m: self.sea_level_m,
            ocean_fraction,
            ice_fraction: iced / n,
            mean_land_biomass: land_biomass,
            oxygen: self.oxygen,
            phase,
        }
    }

    /// Height to draw a cell at, in metres: the ground, or the sea surface over it.
    pub fn surface_height_m(&self, cell: usize) -> f32 {
        self.elevation[cell].max(self.sea_level_m)
    }

    /// Colour of a cell for a view, as linear RGB.
    pub fn colour(&self, cell: usize, view: View) -> [f32; 3] {
        match view {
            View::Temperature => ramp(
                (self.temperature[cell] + 60.0) / 100.0,
                [0.1, 0.2, 0.8],
                [0.9, 0.9, 0.9],
                [0.9, 0.15, 0.1],
            ),
            View::Moisture => mix([0.45, 0.35, 0.25], [0.1, 0.4, 0.9], self.moisture[cell]),
            View::Biomass => mix([0.25, 0.22, 0.2], [0.2, 0.85, 0.2], self.biomass[cell]),
            View::Surface => self.surface_colour(cell),
        }
    }

    fn surface_colour(&self, cell: usize) -> [f32; 3] {
        let t = self.temperature[cell];
        if self.is_ocean(cell) {
            return if t > -2.0 {
                [0.05, 0.2, 0.5]
            } else {
                [0.7, 0.82, 0.9]
            };
        }
        if self.ice[cell] > 30.0 {
            return [0.93, 0.95, 0.98];
        }
        let m = self.moisture[cell];
        let frost = smoothstep(1.0, 20.0, self.ice[cell]);
        let dry = if t > 30.0 {
            [0.8, 0.68, 0.42]
        } else {
            [0.5, 0.4, 0.33]
        };
        let bare = mix(
            mix(dry, [0.3, 0.24, 0.2], m),
            [0.75, 0.76, 0.8],
            frost * 0.6,
        );
        let plants = if t > 26.0 && m > 0.6 {
            [0.05, 0.35, 0.1]
        } else if t < 8.0 {
            [0.35, 0.45, 0.3]
        } else {
            [0.25, 0.55, 0.15]
        };
        mix(bare, plants, smoothstep(0.02, 0.6, self.biomass[cell]))
    }
}

/// Smooth, continent-sized terrain from a handful of waves in random directions.
fn terrain(grid: &Grid, seed: u64) -> (Vec<Wave>, Vec<f32>) {
    let mut rng = Rng(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1);
    let mut waves: Vec<Wave> = (0..14)
        .map(|_| {
            let direction = normalise([rng.signed(), rng.signed(), rng.signed()]);
            let frequency = 1.5 + rng.unit() * 6.0;
            let phase = rng.unit() * std::f32::consts::TAU;
            Wave {
                direction,
                frequency,
                phase,
                amplitude_m: 1.0 / frequency,
            }
        })
        .collect();
    let height = |waves: &[Wave], p: [f32; 3]| -> f32 {
        waves
            .iter()
            .map(|w| w.amplitude_m * libm::sinf(w.frequency * dot(p, w.direction) + w.phase))
            .sum()
    };
    let peak = (0..grid.len())
        .map(|i| height(&waves, grid.position(i)).abs())
        .fold(0.0f32, f32::max);
    for wave in &mut waves {
        wave.amplitude_m *= RELIEF_M / peak;
    }
    let heights = (0..grid.len())
        .map(|i| height(&waves, grid.position(i)))
        .collect();
    (waves, heights)
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalise(v: [f32; 3]) -> [f32; 3] {
    let len = dot(v, v).sqrt().max(1e-9);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn ramp(t: f32, low: [f32; 3], mid: [f32; 3], high: [f32; 3]) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        mix(low, mid, t * 2.0)
    } else {
        mix(mid, high, t * 2.0 - 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(sim: &mut Sim, ticks: u32) {
        for _ in 0..ticks {
            sim.step();
        }
    }

    #[test]
    fn grid_has_the_expected_shape() {
        let grid = Grid::icosphere(3);
        assert_eq!(grid.len(), 642);
        let fives = (0..grid.len())
            .filter(|&i| grid.neighbours(i).len() == 5)
            .count();
        let sixes = (0..grid.len())
            .filter(|&i| grid.neighbours(i).len() == 6)
            .count();
        assert_eq!((fives, sixes), (12, 630));
    }

    #[test]
    fn a_dead_planet_stays_dead() {
        let mut sim = Sim::new(3, 1);
        run(&mut sim, 2000);
        let s = sim.summary();
        assert_eq!(s.phase, Phase::Dead);
        assert_eq!(s.ocean_fraction, 0.0);
        assert_eq!(s.mean_land_biomass, 0.0);
    }

    #[test]
    fn more_gas_warms_the_whole_planet_more_and_raises_the_sea() {
        // The same planet run as long with no, two and four generators.
        let runs: Vec<Sim> = [0, 2, 4]
            .into_iter()
            .map(|generators| {
                let mut sim = Sim::new(3, 1);
                for _ in 0..generators {
                    sim.place(Machine::GasGenerator, 0);
                }
                run(&mut sim, 3000);
                sim
            })
            .collect();
        for pair in runs.windows(2) {
            let (less, more) = (&pair[0], &pair[1]);
            // Warmer everywhere, not only near the generators.
            for cell in 0..less.grid.len() {
                assert!(
                    more.temperature[cell] > less.temperature[cell],
                    "cell {cell}"
                );
            }
            let (less, more) = (less.summary(), more.summary());
            assert!(more.sea_level_m > less.sea_level_m);
            assert!(more.ice_fraction <= less.ice_fraction);
        }
        let (none, most) = (runs[0].summary(), runs[2].summary());
        assert!(most.ocean_fraction > none.ocean_fraction);
        assert!(most.ice_fraction < none.ice_fraction);
    }

    #[test]
    fn a_heater_only_warms_its_surroundings() {
        let by_latitude = |sim: &Sim, sign: f32| {
            (0..sim.grid.len())
                .max_by(|&a, &b| {
                    (sign * sim.grid.latitude(a)).total_cmp(&(sign * sim.grid.latitude(b)))
                })
                .unwrap()
        };
        let mut control = Sim::new(3, 1);
        let mut heated = Sim::new(3, 1);
        let (north, south) = (by_latitude(&heated, 1.0), by_latitude(&heated, -1.0));
        heated.place(Machine::Heater, north);
        run(&mut control, 500);
        run(&mut heated, 500);
        // On this coarse test grid the heater covers little more than one cell.
        assert!(heated.temperature[north] > control.temperature[north] + 10.0);
        assert!((heated.temperature[south] - control.temperature[south]).abs() < 0.01);
    }

    #[test]
    fn life_needs_air_warmth_and_water() {
        // The same planet given air and seas, twice, and left dead.
        let living = || {
            let mut sim = Sim::new(3, 1);
            for _ in 0..4 {
                sim.place(Machine::GasGenerator, 0);
            }
            run(&mut sim, 3000);
            sim
        };
        let (mut seeded, mut control) = (living(), living());
        let mut dead = Sim::new(3, 1);
        run(&mut dead, 3000);
        // The most habitable land, well above the shoreline so the still-rising sea does
        // not drown it.
        let wet = (0..seeded.grid.len())
            .filter(|&i| {
                seeded.elevation[i] > seeded.sea_level_m + 30.0 && seeded.suitability(i) > 0.0
            })
            .max_by(|&a, &b| seeded.suitability(a).total_cmp(&seeded.suitability(b)))
            .expect("some land should be habitable once there is air and sea");
        seeded.place(Machine::Seeder, wet);
        dead.place(Machine::Seeder, wet);
        for sim in [&mut seeded, &mut control, &mut dead] {
            run(sim, 3000);
        }
        // Where it was seeded it grows, and creeps out into the cells round it, more than
        // on the same planet unseeded.
        let elsewhere = |sim: &Sim| -> f32 {
            (0..sim.grid.len())
                .filter(|&i| i != wet)
                .map(|i| sim.biomass[i])
                .sum()
        };
        assert!(seeded.biomass[wet] > control.biomass[wet]);
        assert!(
            elsewhere(&seeded) > elsewhere(&control),
            "life should creep into neighbouring cells"
        );
        // On the dead planet the same seeding does nothing at all.
        assert_eq!(dead.biomass[wet], 0.0);
        assert_eq!(elsewhere(&dead), 0.0);
    }
}
