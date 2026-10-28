//! The icon baker (D129): renders every item in the catalogue from its look with the icon
//! renderer (`terra_game::icons`), in the house style (D130), and writes
//! `assets/icons/<item>.png`, the item's cells times `PX_PER_CELL`, and
//! `assets/icons/manifest.txt`, saying what each was rendered from. An icon painted by
//! `tools/icons/paint.ts` is kept, never replaced by its render. Items whose look does not
//! exist yet are listed and skipped; icons for items no longer in the catalogue are
//! removed. Run it as `cargo xtask icons`. The playbook is `.agents/skills/item-icons/SKILL.md`.
//!
//! `--renders <folder>` also writes every render there, for the painter. `--sheet <prefix>`
//! also lays the icons out on empty slots and saves `<prefix>.png` at the size rendered and
//! `<prefix>-half.png` at half, a backpack cell on a plain display. Then it exits.
//!
//! `--keep <item>` with nothing else to write renders nothing ([`keep_alone`]): a painting is
//! kept by fingerprinting its look's file, which needs no window and no GPU.

use std::path::{Path, PathBuf};

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use terra_game::play::inventory::Items;
use terra_game::{
    icons::{self, Baked, Icon, Icons, PX_PER_CELL},
    shell::Options,
};

/// The empty slot behind an icon on the sheet, and the gap between slots, as the play
/// screens have them.
const SLOT: [f32; 3] = [0.12, 0.13, 0.16];
const GAP: [f32; 3] = [0.05, 0.06, 0.08];
/// Slots across the sheet.
const COLUMNS: usize = 8;

/// One icon to render: the item it is for, and what it is rendered from.
struct Wanted {
    item: String,
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

/// The checkout's assets, also for a Windows copy run from WSL (`cargo xtask win`), so the
/// icons and the manifest it writes land in the checkout, not in the copy.
fn assets() -> PathBuf {
    terra_world::data::source_assets()
}

/// `--keep` with nothing to render (no `--renders`, no `--sheet`): records each kept item's
/// painting as made from its look as it is now, opens no window, and gives the run's end.
/// None when the baker is to run. Under WSL the baker's window never came up on the CPU
/// renderer, and the fingerprint hashes the look's file, so rendering was never needed.
pub fn keep_alone(options: &Options) -> Option<AppExit> {
    let alone = options.experiments.iter().any(|name| name == "icons")
        && !options.keeps.is_empty()
        && options.renders.is_none()
        && options.sheet.is_none();
    if !alone {
        return None;
    }
    let (found, _) = catalogue();
    let mut lines = manifest();
    for item in &options.keeps {
        let Some((_, icon)) = found.iter().find(|(found, _)| found == item) else {
            eprintln!("ICONS ✗ {item} is not an item with a look");
            return Some(AppExit::error());
        };
        let Some(line) = lines
            .iter_mut()
            .find(|line| &line.item == item && line.painted)
        else {
            eprintln!("ICONS ✗ {item} has no painting to keep; `cargo xtask icons` renders it");
            return Some(AppExit::error());
        };
        keep(line, &icon.look);
    }
    Some(if write_manifest(&lines) {
        AppExit::Success
    } else {
        AppExit::error()
    })
}

/// Records a painting as made from `look` as it is now.
fn keep(line: &mut Baked, look: &str) {
    let bytes = std::fs::read(assets().join(look)).unwrap_or_default();
    line.look = Some(look.to_string());
    line.fingerprint = Some(icons::fingerprint(&bytes));
    println!("ICONS kept the painting of {}, now from {look}", line.item);
}

/// Whether an item's line in the manifest says its icon is made from its look as it is now.
#[cfg(test)]
fn fresh(line: &Baked, item: &str, icon: &Icon) -> bool {
    let look = std::fs::read(assets().join(&icon.look)).unwrap_or_default();
    line.item == item
        && line.look.as_ref() == Some(&icon.look)
        && line.fingerprint == Some(icons::fingerprint(&look))
        && (line.painted || line.style == Some(icons::STYLE))
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

fn ask(mut baking: ResMut<Baking>, mut icons: ResMut<Icons>) {
    let (found, missing) = catalogue();
    if !missing.is_empty() {
        println!("ICONS no look yet, shown by name: {}", missing.join(", "));
    }
    baking.wanted = found
        .into_iter()
        .map(|(item, icon)| Wanted { item, icon })
        .collect();
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
    bake(&rendered, &options.keeps);
    if let Some(folder) = &options.renders {
        write_renders(&rendered, folder);
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

/// The manifest as it stands.
fn manifest() -> Vec<Baked> {
    std::fs::read_to_string(assets().join(icons::MANIFEST))
        .unwrap_or_default()
        .lines()
        .filter_map(Baked::parse)
        .collect()
}

/// Writes each item's rendered icon, unless it has a painted one, which is kept as it is
/// even if its look has changed (the test then asks for it to be painted again, unless the
/// item is in `keeps`, whose paintings are recorded as made from their looks as they are now);
/// removes icons of items no longer in the catalogue; and writes the manifest.
fn bake(rendered: &[(&Wanted, &[u8])], keeps: &[String]) {
    let folder = assets().join("icons");
    std::fs::create_dir_all(&folder).ok();
    let before = manifest();
    let (found, missing) = catalogue();
    let items: Vec<&String> = found.iter().map(|(item, _)| item).chain(&missing).collect();
    let mut lines: Vec<Baked> = before
        .iter()
        .filter(|baked| baked.painted && items.contains(&&baked.item))
        .cloned()
        .collect();
    for (wanted, rgba) in rendered {
        let item = &wanted.item;
        if let Some(kept) = lines
            .iter_mut()
            .find(|baked| &baked.item == item && keeps.contains(item))
        {
            keep(kept, &wanted.icon.look);
            continue;
        }
        if lines.iter().any(|baked| &baked.item == item) {
            continue;
        }
        let (w, h) = wanted.icon.cells;
        let size = (u32::from(w) * PX_PER_CELL, u32::from(h) * PX_PER_CELL);
        save(rgba.to_vec(), size, &assets().join(icons::file(item)));
        let look = std::fs::read(assets().join(&wanted.icon.look)).unwrap_or_default();
        lines.push(Baked {
            item: item.clone(),
            painted: false,
            look: Some(wanted.icon.look.clone()),
            fingerprint: Some(icons::fingerprint(&look)),
            style: Some(icons::STYLE),
        });
    }
    lines.sort_by(|a, b| a.item.cmp(&b.item));
    for old in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
        let name = old.file_name().to_string_lossy().into_owned();
        let Some(item) = name.strip_suffix(".png") else {
            continue;
        };
        if !lines.iter().any(|baked| baked.item == item) {
            std::fs::remove_file(old.path()).ok();
            println!("ICONS removed {name}");
        }
    }
    write_manifest(&lines);
}

/// Writes the manifest, saying how many icons are rendered and painted; false if it could
/// not.
fn write_manifest(lines: &[Baked]) -> bool {
    let text: String = lines.iter().map(|baked| baked.line() + "\n").collect();
    let manifest = assets().join(icons::MANIFEST);
    match std::fs::write(&manifest, text) {
        Ok(()) => {
            println!(
                "ICONS {} rendered, {} painted, in {}",
                lines.iter().filter(|baked| !baked.painted).count(),
                lines.iter().filter(|baked| baked.painted).count(),
                manifest.display()
            );
            true
        }
        Err(error) => {
            eprintln!("ICONS ✗ could not write {}: {error}", manifest.display());
            false
        }
    }
}

/// Writes every render, item or not, into `folder`, for painting.
fn write_renders(rendered: &[(&Wanted, &[u8])], folder: &Path) {
    for (wanted, rgba) in rendered {
        let name = &wanted.item;
        let (w, h) = wanted.icon.cells;
        let size = (u32::from(w) * PX_PER_CELL, u32::from(h) * PX_PER_CELL);
        save(rgba.to_vec(), size, &folder.join(format!("{name}.png")));
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

    /// Only things neither held nor worn may go without a look, shown by name and a painted
    /// icon: something in hand or on the body with none is drawn as nothing at all.
    #[test]
    fn every_item_held_or_worn_has_a_look() {
        let items = Items::default();
        let (_, missing) = catalogue();
        let mut checked = 0;
        for kind in items.kinds() {
            if items.tool(kind).is_none() && items.worn(kind).is_none() {
                continue;
            }
            checked += 1;
            let (item, _) = items.icon_source(kind).unwrap();
            assert!(!missing.contains(&item), "{item} has no look");
        }
        assert!(checked > 0);
    }

    /// Keeping a painting whose look changed makes its line fresh again, with no render.
    #[test]
    fn keeping_a_painting_records_its_look_as_it_is_now() {
        let (found, _) = catalogue();
        let painted = manifest()
            .into_iter()
            .find(|line| line.painted && line.look.is_some());
        let mut line = painted.expect("a painted icon with a look");
        let (item, icon) = found.iter().find(|(item, _)| item == &line.item).unwrap();
        line.look = Some("models/elsewhere.glb".into());
        line.fingerprint = Some(1);
        assert!(!fresh(&line, item, icon));
        keep(&mut line, &icon.look);
        assert!(fresh(&line, item, icon));
    }

    /// Every item whose look exists has an icon made from that look as it is now: rendered
    /// in the style as it is now, or painted from it. Every icon in the manifest exists.
    /// When this fails, run `cargo xtask icons` (or paint again, `tools/icons`) and commit
    /// the icons.
    #[test]
    fn every_icon_is_made_from_its_current_look() {
        let baked = manifest();
        let (found, _) = catalogue();
        let mut stale = Vec::new();
        for (item, icon) in &found {
            if !baked.iter().any(|line| fresh(line, item, icon)) {
                stale.push(item.clone());
            }
        }
        for line in &baked {
            if !assets().join(icons::file(&line.item)).exists() {
                stale.push(line.item.clone());
            }
        }
        assert!(
            stale.is_empty(),
            "icons missing or stale for {stale:?}: run `cargo xtask icons`, or paint them again \
             (tools/icons), and commit assets/icons"
        );
    }
}
