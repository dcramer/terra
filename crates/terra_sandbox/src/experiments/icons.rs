//! A sheet of item icons, drawn by the game's own renderer (`crate::icons`), to check
//! looks against the house style (D130). The playbook is
//! `.claude/skills/item-icons/SKILL.md`.
//!
//! `--model <path>` draws one look under `assets/` (repeatable); with none, every held
//! look, every item's own look and every worn look on one base body. Each is drawn one cell square. `--sheet
//! <prefix>` lays them out on empty slots and saves `<prefix>.png` at the size drawn and
//! `<prefix>-half.png` at half that, the size of a backpack cell on a plain display, then
//! exits.

use std::path::{Path, PathBuf};

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use crate::{
    icons::{Icon, Icons, PX_PER_CELL},
    shell::Options,
};

const COLUMNS: usize = 8;
/// The empty slot behind an icon on the sheet, and the gap between slots, as the play
/// screens have them.
const SLOT: [f32; 3] = [0.12, 0.13, 0.16];
const GAP: [f32; 3] = [0.05, 0.06, 0.08];

#[derive(Resource, Default)]
struct Sheet {
    subjects: Vec<Icon>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Sheet>()
        .add_systems(Startup, ask)
        .add_systems(Update, read_back);
}

/// What `--model` names, or every held look, every item's own look, and every worn look on
/// the first base body.
fn files(options: &Options) -> Vec<String> {
    if !options.models.is_empty() {
        return options.models.clone();
    }
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let listed = |folder: &str| {
        let mut names: Vec<String> = std::fs::read_dir(assets.join(folder))
            .into_iter()
            .flatten()
            .flatten()
            .map(|file| file.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    let looks = |folder: String| {
        listed(&folder)
            .into_iter()
            .filter(|name| name.ends_with(".glb"))
            .map(move |name| format!("{folder}/{name}"))
    };
    let mut files: Vec<String> = looks("models/held".to_string())
        .chain(looks("models/items".to_string()))
        .collect();
    if let Some(body) = listed("models/gear").first() {
        files.extend(looks(format!("models/gear/{body}")));
    }
    files
}

fn ask(options: Res<Options>, mut sheet: ResMut<Sheet>, mut icons: ResMut<Icons>) {
    sheet.subjects = files(&options)
        .into_iter()
        .map(|file| Icon::new(file, (1, 1)))
        .collect();
    if sheet.subjects.is_empty() {
        error!("icons: no looks to draw");
    }
    icons.keep_copies = true;
    for subject in &sheet.subjects {
        icons.want(subject);
    }
}

/// Once every icon is made, saves the sheet of their copies and exits.
fn read_back(
    options: Res<Options>,
    icons: Res<Icons>,
    sheet: Res<Sheet>,
    mut exit: MessageWriter<AppExit>,
) {
    if !icons.settled() {
        return;
    }
    // An icon not drawn is left out; the renderer said why.
    let copies: Vec<&[u8]> = sheet
        .subjects
        .iter()
        .filter_map(|subject| icons.copy(subject))
        .filter(|copy| !copy.is_empty())
        .collect();
    if let Some(prefix) = &options.sheet {
        save(&copies, prefix);
    }
    exit.write(AppExit::Success);
}

/// Lays the icons out on empty slots, `COLUMNS` across, at the size drawn and at half.
fn save(icons: &[&[u8]], prefix: &Path) {
    let drawn = PX_PER_CELL as usize;
    for (size, path) in [
        (drawn, PathBuf::from(format!("{}.png", prefix.display()))),
        (
            drawn / 2,
            PathBuf::from(format!("{}-half.png", prefix.display())),
        ),
    ] {
        let gap = (size / 16).max(3);
        let columns = COLUMNS.min(icons.len()).max(1);
        let rows = icons.len().div_ceil(columns).max(1);
        let (width, height) = (columns * (size + gap) + gap, rows * (size + gap) + gap);
        let mut rgba: Vec<u8> = (0..width * height).flat_map(|_| bytes(GAP)).collect();
        for (at, icon) in icons.iter().enumerate() {
            let (left, top) = (
                gap + (at % columns) * (size + gap),
                gap + (at / columns) * (size + gap),
            );
            let shrunk = shrink(icon, drawn, size);
            for y in 0..size {
                for x in 0..size {
                    let from = (y * size + x) * 4;
                    let alpha = shrunk[from + 3];
                    let to = ((top + y) * width + left + x) * 4;
                    for c in 0..3 {
                        // The shrunk colour is already weighted by its coverage.
                        let value = shrunk[from + c] + SLOT[c] * (1.0 - alpha);
                        rgba[to + c] = (value.clamp(0.0, 1.0) * 255.0) as u8;
                    }
                }
            }
        }
        let image = Image::new(
            Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).ok();
        }
        match image.try_into_dynamic().map(|icon| icon.save(&path)) {
            Ok(Ok(())) => println!("SANDBOX saved {}", path.display()),
            Ok(Err(error)) => error!("icons: could not save {}: {error}", path.display()),
            Err(error) => error!("icons: could not make {}: {error}", path.display()),
        }
    }
}

fn bytes(colour: [f32; 3]) -> [u8; 4] {
    [
        (colour[0] * 255.0) as u8,
        (colour[1] * 255.0) as u8,
        (colour[2] * 255.0) as u8,
        255,
    ]
}

/// A square RGBA icon averaged down to `to` on a side, each channel 0 to 1, colour
/// weighted by coverage so edges blend with what is behind them.
fn shrink(rgba: &[u8], from: usize, to: usize) -> Vec<f32> {
    let step = from / to;
    let mut out = vec![0.0; to * to * 4];
    for y in 0..to {
        for x in 0..to {
            let mut sum = [0.0f32; 4];
            for dy in 0..step {
                for dx in 0..step {
                    let at = ((y * step + dy) * from + x * step + dx) * 4;
                    let alpha = f32::from(rgba[at + 3]) / 255.0;
                    for c in 0..3 {
                        sum[c] += f32::from(rgba[at + c]) / 255.0 * alpha;
                    }
                    sum[3] += alpha;
                }
            }
            let count = (step * step) as f32;
            for c in 0..4 {
                out[(y * to + x) * 4 + c] = sum[c] / count;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinking_weights_colour_by_coverage() {
        // Two by two: one opaque white pixel, three clear, to one pixel a quarter covered.
        let rgba = [255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let out = shrink(&rgba, 2, 1);
        assert_eq!(out, vec![0.25, 0.25, 0.25, 0.25]);
    }
}
