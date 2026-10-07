// Spike for D22: the same simulation tick as planet_test/sim_bench.gd, built as a
// Godot extension, to check that Rust loads in the pinned engine build and what it costs.
use godot::prelude::*;
use std::time::Instant;

struct TerraSimBench;

#[gdextension]
unsafe impl ExtensionLibrary for TerraSimBench {}

const FIELDS: usize = 4;
const NEIGHBOURS: usize = 6;

#[derive(GodotClass)]
#[class(base=RefCounted, init)]
struct SimBench {
    neighbours: Vec<u32>,
    current: Vec<Vec<f32>>,
    next: Vec<Vec<f32>>,
    base: Base<RefCounted>,
}

#[godot_api]
impl SimBench {
    #[func]
    fn setup(&mut self, cells: i64) {
        let cells = cells as usize;
        let mut seed: u64 = 7;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        self.neighbours = (0..cells * NEIGHBOURS).map(|_| (rnd() % cells as u64) as u32).collect();
        self.current = (0..FIELDS)
            .map(|_| (0..cells).map(|_| (rnd() % 10000) as f32 / 10000.0).collect())
            .collect();
        self.next = self.current.clone();
    }

    /// Runs `ticks` ticks and returns the average milliseconds per tick.
    #[func]
    fn run(&mut self, ticks: i64) -> f64 {
        let cells = self.current[0].len();
        let start = Instant::now();
        for _ in 0..ticks {
            for f in 0..FIELDS {
                let src = &self.current[f];
                let other = &self.current[(f + 1) % FIELDS];
                let dst = &mut self.next[f];
                for i in 0..cells {
                    let base = i * NEIGHBOURS;
                    let mut sum = 0.0f32;
                    for n in 0..NEIGHBOURS {
                        sum += src[self.neighbours[base + n] as usize];
                    }
                    let mut v = src[i];
                    v += 0.1 * (sum / NEIGHBOURS as f32 - v);
                    v += 0.01 * (other[i] - v);
                    dst[i] = v.clamp(0.0, 1.0);
                }
            }
            std::mem::swap(&mut self.current, &mut self.next);
        }
        start.elapsed().as_secs_f64() * 1000.0 / ticks as f64
    }

    /// Hands one field to the engine, as rendering or the UI would need each tick.
    #[func]
    fn field(&self, index: i64) -> PackedFloat32Array {
        PackedFloat32Array::from(self.current[index as usize].as_slice())
    }
}
