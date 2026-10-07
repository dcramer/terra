//! Thin Godot binding for the prototype simulation. All logic lives in `terra_sim`.
//!
//! The simulation runs on its own thread at a requested rate, so it never costs the
//! renderer a frame. Calls from Godot take a short lock to read or change it.

use godot::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use terra_sim::{Machine, Phase, Sim, View};

struct TerraSimExtension;

#[gdextension]
unsafe impl ExtensionLibrary for TerraSimExtension {}

/// Ticks the worker runs before letting go of the lock, so readers never wait long.
const BATCH: u32 = 4;

struct Shared {
    sim: Mutex<Sim>,
    ticks_per_second: AtomicU32,
    reader_waiting: AtomicBool,
    stop: AtomicBool,
}

#[derive(GodotClass)]
#[class(base=RefCounted, init)]
struct TerraSim {
    shared: Option<Arc<Shared>>,
    worker: Option<JoinHandle<()>>,
    /// For each texel of the surface texture, the cell it shows.
    texel_cells: Vec<u32>,
    texture_size: (usize, usize),
    base: Base<RefCounted>,
}

fn run_worker(shared: Arc<Shared>) {
    let mut owed = 0.0f64;
    let mut last = Instant::now();
    while !shared.stop.load(Ordering::Relaxed) {
        let rate = shared.ticks_per_second.load(Ordering::Relaxed) as f64;
        let now = Instant::now();
        // Never carry more than a quarter second of backlog: if the machine can't keep
        // up, the simulation runs slower rather than falling further behind.
        owed = (owed + (now - last).as_secs_f64() * rate).min(rate * 0.25);
        last = now;
        if owed < 1.0 || shared.reader_waiting.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_micros(if owed < 1.0 { 1000 } else { 50 }));
            continue;
        }
        let ticks = (owed as u32).min(BATCH);
        {
            let mut sim = shared.sim.lock().unwrap();
            for _ in 0..ticks {
                sim.step();
            }
        }
        owed -= ticks as f64;
    }
}

#[godot_api]
impl TerraSim {
    #[func]
    fn setup(&mut self, subdivisions: i64, seed: i64) {
        self.shutdown();
        let shared = Arc::new(Shared {
            sim: Mutex::new(Sim::new(subdivisions as u32, seed as u64)),
            ticks_per_second: AtomicU32::new(0),
            reader_waiting: AtomicBool::new(false),
            stop: AtomicBool::new(false),
        });
        let for_worker = shared.clone();
        self.worker = Some(std::thread::spawn(move || run_worker(for_worker)));
        self.shared = Some(shared);
    }

    /// How fast the simulation runs by itself in the background. Zero pauses it.
    #[func]
    fn set_rate(&self, ticks_per_second: i64) {
        self.shared().ticks_per_second.store(ticks_per_second.max(0) as u32, Ordering::Relaxed);
    }

    /// Runs ticks right now, on the calling thread. For tests and scripted runs.
    #[func]
    fn step(&self, ticks: i64) {
        let mut sim = self.sim();
        for _ in 0..ticks {
            sim.step();
        }
    }

    /// `machine`: 0 heater, 1 gas generator, 2 seeder.
    #[func]
    fn place(&self, machine: i64, cell: i64) {
        let machine = match machine {
            0 => Machine::Heater,
            1 => Machine::GasGenerator,
            _ => Machine::Seeder,
        };
        self.sim().place(machine, cell as usize);
    }

    #[func]
    fn cell_at(&self, direction: Vector3) -> i64 {
        self.sim().grid.nearest([direction.x, direction.y, direction.z]) as i64
    }

    #[func]
    fn cell_direction(&self, cell: i64) -> Vector3 {
        let p = self.sim().grid.position(cell as usize);
        Vector3::new(p[0], p[1], p[2])
    }

    #[func]
    fn is_ocean(&self, cell: i64) -> bool {
        self.sim().is_ocean(cell as usize)
    }

    #[func]
    fn cell_count(&self) -> i64 {
        self.sim().grid.len() as i64
    }

    /// temperature, moisture, biomass, ice (m), elevation (m), latitude (rad).
    #[func]
    fn cell_values(&self, cell: i64) -> PackedFloat32Array {
        let sim = self.sim();
        let i = cell as usize;
        [sim.temperature[i], sim.moisture[i], sim.biomass[i], sim.ice[i], sim.elevation[i], sim.grid.latitude(i)]
            .into_iter()
            .collect()
    }

    /// One vertex per cell on a unit sphere, raised by `relief` per metre of height.
    #[func]
    fn vertices(&self, relief: f32) -> PackedVector3Array {
        let sim = self.sim();
        (0..sim.grid.len())
            .map(|i| {
                let p = sim.grid.position(i);
                let r = 1.0 + sim.surface_height_m(i) * relief;
                Vector3::new(p[0] * r, p[1] * r, p[2] * r)
            })
            .collect()
    }

    #[func]
    fn triangles(&self) -> PackedInt32Array {
        self.sim().grid.triangles().iter().map(|&i| i as i32).collect()
    }

    /// `view`: 0 surface, 1 temperature, 2 moisture, 3 biomass.
    #[func]
    fn colours(&self, view: i64) -> PackedColorArray {
        let view = match view {
            1 => View::Temperature,
            2 => View::Moisture,
            3 => View::Biomass,
            _ => View::Surface,
        };
        let sim = self.sim();
        (0..sim.grid.len())
            .map(|i| {
                let c = sim.colour(i, view);
                Color::from_rgb(c[0], c[1], c[2])
            })
            .collect()
    }

    /// The terrain's shape, six numbers per wave: direction x, y, z, frequency, phase,
    /// amplitude in metres. Height at unit vector p is the sum over waves of
    /// `amplitude * sin(frequency * dot(direction, p) + phase)`.
    #[func]
    fn terrain_waves(&self) -> PackedFloat32Array {
        self.sim()
            .waves
            .iter()
            .flat_map(|w| [w.direction[0], w.direction[1], w.direction[2], w.frequency, w.phase, w.amplitude_m])
            .collect()
    }

    /// The surface view as RGB8 pixels on a latitude-longitude map, for texturing the
    /// ground. Column u is longitude `atan2(x, z)`, row v is latitude, both mapped to 0..1.
    #[func]
    fn surface_texture(&mut self, width: i64, height: i64) -> PackedByteArray {
        let (w, h) = (width as usize, height as usize);
        if self.texture_size != (w, h) {
            let sim = self.sim();
            let mut cells = Vec::with_capacity(w * h);
            let mut cell = 0;
            for py in 0..h {
                let lat = ((py as f32 + 0.5) / h as f32 - 0.5) * std::f32::consts::PI;
                for px in 0..w {
                    let lon = ((px as f32 + 0.5) / w as f32 - 0.5) * std::f32::consts::TAU;
                    let dir = [lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos()];
                    cell = sim.grid.nearest_from(cell, dir);
                    cells.push(cell as u32);
                }
            }
            drop(sim);
            self.texel_cells = cells;
            self.texture_size = (w, h);
        }
        // Work out each cell's colour once, under the lock, then paint texels from that.
        let cell_colours: Vec<[u8; 3]> = {
            let sim = self.sim();
            (0..sim.grid.len())
                .map(|i| sim.colour(i, View::Surface).map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8))
                .collect()
        };
        let mut bytes = Vec::with_capacity(w * h * 3);
        for &cell in &self.texel_cells {
            bytes.extend(cell_colours[cell as usize]);
        }
        PackedByteArray::from(bytes.as_slice())
    }

    /// tick, pressure, mean temperature, sea level (m), ocean fraction, ice fraction,
    /// mean land biomass, oxygen, phase (0 dead .. 4 breathable).
    #[func]
    fn summary(&self) -> PackedFloat32Array {
        let s = self.sim().summary();
        let phase = match s.phase {
            Phase::Dead => 0.0,
            Phase::Thawing => 1.0,
            Phase::Wet => 2.0,
            Phase::Green => 3.0,
            Phase::Breathable => 4.0,
        };
        [
            s.tick as f32,
            s.pressure,
            s.mean_temperature,
            s.sea_level_m,
            s.ocean_fraction,
            s.ice_fraction,
            s.mean_land_biomass,
            s.oxygen,
            phase,
        ]
        .into_iter()
        .collect()
    }
}

impl TerraSim {
    fn shared(&self) -> &Arc<Shared> {
        self.shared.as_ref().expect("setup() first")
    }

    /// Locks the simulation, asking the worker to stand aside so the wait is short.
    fn sim(&self) -> MutexGuard<'_, Sim> {
        let shared = self.shared();
        shared.reader_waiting.store(true, Ordering::Relaxed);
        let guard = shared.sim.lock().unwrap();
        shared.reader_waiting.store(false, Ordering::Relaxed);
        guard
    }

    fn shutdown(&mut self) {
        if let Some(shared) = self.shared.take() {
            shared.stop.store(true, Ordering::Relaxed);
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for TerraSim {
    fn drop(&mut self) {
        self.shutdown();
    }
}
