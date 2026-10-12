//! The icon baker (D129): renders every item in the catalogue from its look with the icon
//! renderer (`crate::icons`), in the house style (D130), and writes `assets/icons/<item>.png`,
//! the item's cells times `PX_PER_CELL`, and `assets/icons/manifest.txt`, saying what each
//! was rendered from. Items whose look does not exist yet are listed and skipped; icons for
//! items no longer in the catalogue are removed. Run it as `cargo xtask icons`. The
//! playbook is `.claude/skills/item-icons/SKILL.md`.
//!
//! `--model <path>` renders only the looks named, one cell square, and writes nothing to
//! the assets: a preview of a look before it is an item. `--sheet <prefix>` also lays the
//! icons out on empty slots and saves `<prefix>.png` at the size rendered and
//! `<prefix>-half.png` at half, a backpack cell on a plain display. Then it exits.

use std::path::{Path, PathBuf};

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use super::play::inventory::Items;
use crate::{
    icons::{self, Icon, Icons, PX_PER_CELL},
    shell::Options,
};

/// The empty slot behind an icon on the sheet, and the gap between slots, as the play
/// screens have them.
const SLOT: [f32; 3] = [0.12, 0.13, 0.16];
const GAP: [f32; 3] = [0.05, 0.06, 0.08];
/// Slots across the sheet.
const COLUMNS: usize = 8;

/// One icon to render: the item it is for, if it is baked, and what it is rendered from.
struct Wanted {
    item: Option<String>,
    icon: Icon,
}

#[derive(Resource, Default)]
struct Baking {
    wanted: Vec<Wanted>,
}

pub fn plugin(app: &mut App) {
    app.add_plugins(icons::plugin)
        .init_resource::<Baking>()
        .add_systems(Startup, ask)
        .add_systems(Update, finish);
}

fn assets() -> PathBuf {
    bevy::asset::io::file::FileAssetReader::get_base_path().join("assets")
}

/// Every item in the catalogue whose look exists, with its id and what its icon is
/// rendered from, and the ids of those whose look does not exist yet.
fn catalogue() -> (Vec<(String, Icon)>, Vec<String>) {
    let items = Items::default();
    let (mut found, mut missing) = (Vec::new(), Vec::new());
    for (kind, _) in items.catalogue.iter() {
        let Some((item, icon)) = items.icon_source(kind) else {
            continue;
        };
        if assets().join(&icon.look).exists() {
            found.push((item, icon));
        } else {
            missing.push(item);
        }
    }
    (found, missing)
}

fn ask(options: Res<Options>, mut baking: ResMut<Baking>, mut icons: ResMut<Icons>) {
    baking.wanted = if options.models.is_empty() {
        let (found, missing) = catalogue();
        if !missing.is_empty() {
            println!("ICONS no look yet, shown by name: {}", missing.join(", "));
        }
        found
            .into_iter()
            .map(|(item, icon)| Wanted {
                item: Some(item),
                icon,
            })
            .collect()
    } else {
        options
            .models
            .iter()
            .map(|look| Wanted {
                item: None,
                icon: Icon::new(look.clone(), (1, 1)),
            })
            .collect()
    };
    for wanted in &baking.wanted {
        icons.want(&wanted.icon);
    }
}

/// Once every icon is rendered: writes the baked ones and the manifest, saves the sheet if
/// asked, and exits.
fn finish(
    options: Res<Options>,
    icons: Res<Icons>,
    baking: Res<Baking>,
    mut exit: MessageWriter<AppExit>,
) {
    if !icons.settled() {
        return;
    }
    let rendered: Vec<(&Wanted, &[u8])> = baking
        .wanted
        .iter()
        .filter_map(|wanted| Some((wanted, icons.get(&wanted.icon)?)))
        .collect();
    let failed = baking.wanted.len() - rendered.len();
    if failed > 0 {
        error!("icons: {failed} could not be rendered; the log above says why");
    }
    if options.models.is_empty() {
        bake(&rendered);
    }
    if let Some(prefix) = &options.sheet {
        sheet(&rendered, prefix);
    }
    exit.write(if failed > 0 {
        AppExit::error()
    } else {
        AppExit::Success
    });
}

/// Writes each item's icon, removes icons of items no longer baked, and writes the
/// manifest.
fn bake(rendered: &[(&Wanted, &[u8])]) {
    let folder = assets().join("icons");
    std::fs::create_dir_all(&folder).ok();
    let mut lines = Vec::new();
    for (wanted, rgba) in rendered {
        let Some(item) = &wanted.item else { continue };
        let (w, h) = wanted.icon.cells;
        let size = (u32::from(w) * PX_PER_CELL, u32::from(h) * PX_PER_CELL);
        save(rgba.to_vec(), size, &assets().join(icons::file(item)));
        let look = std::fs::read(assets().join(&wanted.icon.look)).unwrap_or_default();
        lines.push(format!(
            "{item} {} {:016x} {}",
            wanted.icon.look,
            icons::fingerprint(&look),
            icons::STYLE
        ));
    }
    lines.sort();
    let baked: Vec<String> = lines
        .iter()
        .filter_map(|line| line.split(' ').next())
        .map(|item| format!("{item}.png"))
        .collect();
    for old in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
        let name = old.file_name().to_string_lossy().into_owned();
        if name.ends_with(".png") && !baked.contains(&name) {
            std::fs::remove_file(old.path()).ok();
            println!("ICONS removed {name}");
        }
    }
    let manifest = assets().join(icons::MANIFEST);
    match std::fs::write(&manifest, lines.join("\n") + "\n") {
        Ok(()) => println!("ICONS baked {} into {}", lines.len(), folder.display()),
        Err(error) => error!("icons: could not write {}: {error}", manifest.display()),
    }
}

/// Lays the icons out, each in the middle of a slot as big as the biggest, `COLUMNS`
/// across, at the size rendered and at half, on empty slots.
fn sheet(rendered: &[(&Wanted, &[u8])], prefix: &Path) {
    let cells = rendered
        .iter()
        .map(|(wanted, _)| wanted.icon.cells.0.max(wanted.icon.cells.1))
        .max()
        .unwrap_or(1) as usize;
    for (step, path) in [
        (1, PathBuf::from(format!("{}.png", prefix.display()))),
        (2, PathBuf::from(format!("{}-half.png", prefix.display()))),
    ] {
        let cell = PX_PER_CELL as usize / step;
        let slot = cells * cell;
        let gap = (cell / 16).max(3);
        let columns = COLUMNS.min(rendered.len()).max(1);
        let rows = rendered.len().div_ceil(columns).max(1);
        let (width, height) = (columns * (slot + gap) + gap, rows * (slot + gap) + gap);
        let mut rgba: Vec<u8> = (0..width * height)
            .flat_map(|_| bytes(GAP))
            .collect::<Vec<_>>();
        for at in 0..rendered.len() {
            let (left, top) = (
                gap + (at % columns) * (slot + gap),
                gap + (at / columns) * (slot + gap),
            );
            for y in 0..slot {
                let to = ((top + y) * width + left) * 4;
                for x in 0..slot {
                    rgba[to + x * 4..to + x * 4 + 4].copy_from_slice(&bytes(SLOT));
                }
            }
        }
        for (at, (wanted, icon)) in rendered.iter().enumerate() {
            let (w, h) = (
                usize::from(wanted.icon.cells.0) * PX_PER_CELL as usize,
                usize::from(wanted.icon.cells.1) * PX_PER_CELL as usize,
            );
            let (shrunk, sw, sh) = shrink(icon, w, h, step);
            let (left, top) = (
                gap + (at % columns) * (slot + gap) + (slot - sw) / 2,
                gap + (at / columns) * (slot + gap) + (slot - sh) / 2,
            );
            for y in 0..sh {
                for x in 0..sw {
                    let from = (y * sw + x) * 4;
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
        save(rgba, (width as u32, height as u32), &path);
    }
}

/// Saves RGBA as a PNG.
fn save(rgba: Vec<u8>, (width, height): (u32, u32), path: &Path) {
    let image = Image::new(
        Extent3d {
            width,
            height,
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
    match image.try_into_dynamic().map(|picture| picture.save(path)) {
        Ok(Ok(())) => println!("SANDBOX saved {}", path.display()),
        Ok(Err(error)) => error!("icons: could not save {}: {error}", path.display()),
        Err(error) => error!("icons: could not make {}: {error}", path.display()),
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

/// RGBA `width` by `height` averaged down by `step` each way, each channel 0 to 1, colour
/// weighted by coverage so edges blend with what is behind them; and its new size.
fn shrink(rgba: &[u8], width: usize, height: usize, step: usize) -> (Vec<f32>, usize, usize) {
    let (to_w, to_h) = (width / step, height / step);
    let mut out = vec![0.0; to_w * to_h * 4];
    for y in 0..to_h {
        for x in 0..to_w {
            let mut sum = [0.0f32; 4];
            for dy in 0..step {
                for dx in 0..step {
                    let at = ((y * step + dy) * width + x * step + dx) * 4;
                    let alpha = f32::from(rgba[at + 3]) / 255.0;
                    for c in 0..3 {
                        sum[c] += f32::from(rgba[at + c]) / 255.0 * alpha;
                    }
                    sum[3] += alpha;
                }
            }
            let count = (step * step) as f32;
            for c in 0..4 {
                out[(y * to_w + x) * 4 + c] = sum[c] / count;
            }
        }
    }
    (out, to_w, to_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinking_weights_colour_by_coverage() {
        // Two by two: one opaque white pixel, three clear, to one pixel a quarter covered.
        let rgba = [255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let (out, w, h) = shrink(&rgba, 2, 2, 2);
        assert_eq!((w, h), (1, 1));
        assert_eq!(out, vec![0.25, 0.25, 0.25, 0.25]);
    }

    /// Every item whose look exists has an icon baked from that look as it is now, in the
    /// style as it is now. When this fails, run `cargo xtask icons` and commit the icons.
    #[test]
    fn every_icon_is_baked_from_its_current_look() {
        let manifest = std::fs::read_to_string(assets().join(icons::MANIFEST)).unwrap_or_default();
        let baked: Vec<Vec<&str>> = manifest
            .lines()
            .map(|line| line.split(' ').collect())
            .collect();
        let (found, _) = catalogue();
        let mut stale = Vec::new();
        for (item, icon) in &found {
            let look = std::fs::read(assets().join(&icon.look)).unwrap_or_default();
            let expected = [
                item.clone(),
                icon.look.clone(),
                format!("{:016x}", icons::fingerprint(&look)),
                icons::STYLE.to_string(),
            ];
            let fresh = baked
                .iter()
                .any(|line| line[..] == expected.each_ref().map(String::as_str))
                && assets().join(icons::file(item)).exists();
            if !fresh {
                stale.push(item.as_str());
            }
        }
        assert!(
            stale.is_empty(),
            "icons missing or stale for {stale:?}: run `cargo xtask icons` and commit assets/icons"
        );
    }
}
