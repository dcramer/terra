//! The texture bake (D188), `cargo xtask cook`: every model's pictures, the ground's,
//! flora's, foliage's and barks' material arrays and the fire's flipbooks cooked into
//! block-compressed KTX2 with their mipmaps, into the cache the game reads them from
//! (`terra_core::cooked`). What is cooked already is kept; `--force` cooks everything again.

mod picture;

use picture::{Block, Layer};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::time::Instant;
use terra_bake::glb;
use terra_core::cooked::{self, Cook, Kind};

/// The widest a model's map is kept: a body's budget (D188's), the most any model has.
const MODEL_SIDE: u32 = 2048;
/// The widest a small prop's map is kept.
const SMALL_PROP_SIDE: u32 = 1024;
/// The nearest a prop is seen from, in metres: standing at a table, the eye a metre from it.
const NEAREST_M: f64 = 1.0;
/// Screen pixels a metre covers at [`NEAREST_M`], at 1080p and the narrowest field of view
/// the game offers (55 degrees high): 1080 / (2 tan 27.5°).
const SCREEN_PX_PER_M: f64 = 1037.0;

/// The side a prop `largest_m` across needs: its map wraps a surface about its box's, six
/// faces of the largest side, so texels along a metre are side / (√6 × largest); more than
/// the screen shows at the nearest it is seen is never sampled.
fn prop_side(largest_m: f64) -> u32 {
    let needed = SCREEN_PX_PER_M / NEAREST_M * 6f64.sqrt() * largest_m;
    if needed <= f64::from(SMALL_PROP_SIDE) {
        SMALL_PROP_SIDE
    } else {
        MODEL_SIDE
    }
}

/// What was cooked, by class, for the report: count, GPU MiB as RGBA8 and as cooked.
#[derive(Default, Clone, Copy)]
struct Tally {
    pictures: usize,
    before: f64,
    after: f64,
}

impl std::ops::AddAssign for Tally {
    fn add_assign(&mut self, o: Self) {
        self.pictures += o.pictures;
        self.before += o.before;
        self.after += o.after;
    }
}

struct Job {
    class: &'static str,
    source: PathBuf,
    /// For a model, the side its maps are capped at; for a picture, how it is cooked.
    what: What,
}

enum What {
    Model { side: u32 },
    Picture { kind_hint: Kind, mips: bool },
}

fn main() {
    let force = std::env::args().any(|a| a == "--force");
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let Some(out) = cooked::folder() else {
        eprintln!("✗ textures: no cache folder (XDG_CACHE_HOME or HOME) to cook into");
        std::process::exit(1);
    };
    std::fs::create_dir_all(&out).expect("the cache folder can be made");
    let log_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/bake/textures.log");
    let _ = std::fs::create_dir_all(log_path.parent().expect("a parent"));
    let start = Instant::now();
    let jobs = jobs(&assets);
    let results: Vec<(String, Outcome)> = jobs
        .par_iter()
        .map(|job| (job.class.to_string(), run(job, &out, force)))
        .collect();
    let mut log = String::new();
    let mut classes: Vec<(String, Tally)> = Vec::new();
    let mut failed = 0;
    for (job, (class, result)) in jobs.iter().zip(results) {
        match result {
            Ok((tally, line)) => {
                log += &format!("{}: {line}\n", job.source.display());
                match classes.iter_mut().find(|(c, _)| *c == class) {
                    Some((_, t)) => *t += tally,
                    None => classes.push((class, tally)),
                }
            }
            Err(problem) => {
                failed += 1;
                log += &format!("✗ {}: {problem}\n", job.source.display());
                eprintln!("✗ textures: {}: {problem}", job.source.display());
            }
        }
    }
    let mut total = Tally::default();
    for (class, t) in &classes {
        log += &format!(
            "{class}: {} pictures, {:.0} MiB as RGBA8 -> {:.0} MiB cooked\n",
            t.pictures, t.before, t.after
        );
        total += *t;
    }
    let _ = std::fs::write(&log_path, &log);
    println!(
        "textures: {} pictures in {} files, {:.0} -> {:.0} MiB on the GPU, {:.0} s; {} (log target/bake/textures.log)",
        total.pictures,
        jobs.len(),
        total.before,
        total.after,
        start.elapsed().as_secs_f64(),
        out.display(),
    );
    if failed > 0 {
        std::process::exit(1);
    }
}

/// Every source the bake cooks.
fn jobs(assets: &Path) -> Vec<Job> {
    let dims = prop_dims(assets);
    let mut jobs = Vec::new();
    for source in files(&assets.join("models"), &["glb"]) {
        let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let in_props = source.parent().is_some_and(|p| p.ends_with("props"));
        let side = match dims.iter().find(|(file, _)| file == stem) {
            Some((_, largest)) if in_props => prop_side(*largest),
            _ => MODEL_SIDE,
        };
        jobs.push(Job {
            class: if side < MODEL_SIDE {
                "small props"
            } else {
                "models"
            },
            source,
            what: What::Model { side },
        });
    }
    let materials = assets.join("materials");
    for dir in ["ground", "flora", "foliage", "litter"]
        .into_iter()
        .map(|d| materials.join(d))
        .chain(
            std::fs::read_dir(&materials)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.contains("bark"))
                }),
        )
    {
        for source in files(&dir, &["jpg", "png"]) {
            let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let kind_hint = match stem {
                "color" | "cards" => Kind::Colour,
                "normal" => Kind::Normal,
                _ => Kind::Data,
            };
            jobs.push(Job {
                class: "materials",
                source,
                what: What::Picture {
                    kind_hint,
                    mips: true,
                },
            });
        }
    }
    for source in files(&assets.join("textures/fire"), &["png"]) {
        let name = source.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name == "ramp.png" {
            continue;
        }
        let kind_hint = if name.ends_with(".motion.png") {
            Kind::Two
        } else {
            Kind::Data
        };
        // The fire's shaders read only the top level (fire.wesl, fire_smoke.wesl), frames in
        // an atlas whose smaller copies would bleed one frame into the next.
        jobs.push(Job {
            class: "fire",
            source,
            what: What::Picture {
                kind_hint,
                mips: false,
            },
        });
    }
    jobs
}

/// Each prop's model file and its largest side in metres, from `data/builds.ron`.
fn prop_dims(assets: &Path) -> Vec<(String, f64)> {
    let text = std::fs::read_to_string(assets.join("data/builds.ron")).unwrap_or_default();
    text.split("file: \"")
        .skip(1)
        .filter_map(|rest| {
            let (file, rest) = rest.split_once('"')?;
            let dims = rest.split_once("dims: (")?.1.split_once(')')?.0;
            let largest = dims
                .split(',')
                .filter_map(|d| d.trim().parse::<f64>().ok())
                .fold(0.0, f64::max);
            Some((file.to_string(), largest))
        })
        .collect()
}

fn files(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !path.ends_with("spike") {
                    stack.push(path);
                }
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| extensions.contains(&e))
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// What cooking one file came to: its tally and its line of the log, or why it failed.
type Outcome = Result<(Tally, String), String>;

fn run(job: &Job, out: &Path, force: bool) -> Outcome {
    let bytes = std::fs::read(&job.source).map_err(|p| p.to_string())?;
    match job.what {
        What::Model { side } => model(&bytes, side, out, force),
        What::Picture { kind_hint, mips } => {
            let strip = picture::decode(&bytes)?;
            // A strip taller than wide by a whole number is that many square layers.
            let layers = if strip.height > strip.width && strip.height % strip.width == 0 {
                strip.height / strip.width
            } else {
                1
            };
            // A normal map carrying something else in its alpha (the leaf cards' roughness,
            // D292) keeps all four channels.
            let kind =
                if kind_hint == Kind::Normal && strip.rgba.chunks_exact(4).any(|p| p[3] != 255) {
                    Kind::Data
                } else {
                    kind_hint
                };
            let cook = Cook { kind, layers, mips };
            let name = cooked::picture_name(&bytes, cook);
            let (w, h) = (strip.width, strip.height / layers);
            let parts = picture::layers_of(strip, layers);
            let block = picture::block(kind, false);
            let tally = Tally {
                pictures: 1,
                before: picture::gpu_bytes(w, h, layers, Block::Bc7, mips) * 4.0 / MIB,
                after: picture::gpu_bytes(w, h, layers, block, mips) / MIB,
            };
            let path = out.join(&name);
            if force || !path.is_file() {
                let ktx2 = picture::cook(&parts, kind, block, mips)?;
                write(&path, &ktx2)?;
            }
            Ok((tally, format!("{name} {w}x{h}x{layers} {}", block.name())))
        }
    }
}

const MIB: f64 = 1024.0 * 1024.0;

fn model(bytes: &[u8], side: u32, out: &Path, force: bool) -> Result<(Tally, String), String> {
    let name = cooked::model_name(bytes);
    let path = out.join(&name);
    let mut glb = glb::read(bytes)?;
    let uses = glb::uses(&glb.json);
    let mut tally = Tally::default();
    let mut cooked = Vec::new();
    let mut line = format!("{name} at {side}:");
    let write_it = force || !path.is_file();
    for (i, (kind, occlusion_only)) in uses.iter().copied().enumerate() {
        let Some(source) = glb::image_bytes(&glb, i) else {
            continue;
        };
        let decoded = picture::decode(source)?;
        let (w0, h0) = (decoded.width, decoded.height);
        let layer = picture::capped(decoded, side);
        let layers: Vec<Layer> = vec![layer];
        let block = picture::block(kind, occlusion_only && picture::one_channel(&layers));
        let (w, h) = (layers[0].width, layers[0].height);
        tally += Tally {
            pictures: 1,
            before: picture::gpu_bytes(w0, h0, 1, Block::Bc7, true) * 4.0 / MIB,
            after: picture::gpu_bytes(w, h, 1, block, true) / MIB,
        };
        line += &format!(" {i} {kind:?} {w0}->{w} {}", block.name());
        if write_it {
            cooked.push((i, picture::cook(&layers, kind, block, true)?));
        }
    }
    if write_it {
        glb::replace(&mut glb, cooked);
        write(&path, &glb::write(&glb))?;
    }
    Ok((tally, line))
}

/// Writes a cooked file whole or not at all, so a game reading it never sees half of one.
fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let part = path.with_extension("part");
    std::fs::write(&part, bytes).map_err(|p| p.to_string())?;
    std::fs::rename(&part, path).map_err(|p| p.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_prop_whose_map_outresolves_the_screen_is_capped() {
        // A goblet a few centimetres across at a metre: 2048 texels a side are four
        // texels to a pixel.
        assert_eq!(prop_side(0.29), SMALL_PROP_SIDE);
        // A cart two metres long needs every texel at a metre.
        assert_eq!(prop_side(2.0), MODEL_SIDE);
    }
}
