//! The workbench's contact sheet: each clip, held at each `--moments` fraction, seen
//! from each of `--sides`, as one picture per clip, `<prefix>-<clip>.png`. Rows are the
//! sides, top to bottom; columns are the moments, left to right; side by side models
//! share each picture. Then the sandbox exits.

use std::path::PathBuf;
use terra_game::planet::MainCamera;

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
};

use super::{Bench, pose, view};
use terra_game::shell::Options;

/// Directions to look from, in degrees round from the front, and fractions of a clip,
/// unless `--sides` and `--moments` say otherwise.
const SIDES: [f32; 4] = [30.0, 120.0, 210.0, 300.0];
const MOMENTS: [f32; 1] = [0.4];
/// Frames to let a pose and the camera settle before each capture.
const SETTLE_VIEW: u32 = 12;

#[derive(Resource, Default)]
pub(super) struct Sheet {
    /// Which capture is next, counting through every clip's.
    step: usize,
    waited: u32,
    /// A screenshot is on its way.
    waiting: bool,
    /// This clip's pictures so far.
    cells: Vec<Cell>,
}

/// One picture, cut to the middle and halved.
struct Cell {
    width: u32,
    height: u32,
    rgb: Vec<u8>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Sheet>();
}

pub fn capture(
    mut commands: Commands,
    bench: Option<Res<Bench>>,
    options: Res<Options>,
    mut sheet: ResMut<Sheet>,
    mut players: Query<&mut AnimationPlayer>,
    mut hands: Query<&mut terra_game::character::Carrying>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
    children: Query<&Children>,
    names: Query<&Name>,
    globals: Query<&GlobalTransform>,
    mut frames: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    let (Some(bench), Some(prefix)) = (bench, options.sheet.as_ref()) else {
        return;
    };
    *frames += 1;
    if *frames < options.settle_frames || !bench.ready() || sheet.waiting {
        return;
    }
    let sides: &[f32] = if options.sides.is_empty() {
        &SIDES
    } else {
        &options.sides
    };
    let moments: &[f32] = if options.moments.is_empty() {
        &MOMENTS
    } else {
        &options.moments
    };
    let per_clip = sides.len() * moments.len();
    if sheet.cells.len() == per_clip {
        let clip = &bench.clips[(sheet.step - 1) / per_clip];
        let path = PathBuf::from(format!("{}-{clip}.png", prefix.display()));
        save(&sheet.cells, moments.len(), &path);
        sheet.cells.clear();
    }
    if sheet.step >= bench.clips.len() * per_clip {
        exit.write(AppExit::Success);
        return;
    }
    let clip = &bench.clips[sheet.step / per_clip];
    let within = sheet.step % per_clip;
    let (side, moment) = (
        sides[within / moments.len()],
        moments[within % moments.len()],
    );
    if sheet.waited == 0 {
        pose(&bench, clip, Some(moment), &mut players, &mut hands);
    }
    // Every frame, so a camera on a moving part follows it into the pose.
    if let Ok(mut transform) = camera.single_mut() {
        *transform = view(
            &bench,
            options.focus.as_deref(),
            side,
            &children,
            &names,
            &globals,
        );
    }
    sheet.waited += 1;
    if sheet.waited < SETTLE_VIEW {
        return;
    }
    sheet.waited = 0;
    sheet.step += 1;
    sheet.waiting = true;
    commands.spawn(Screenshot::primary_window()).observe(
        |captured: On<ScreenshotCaptured>, mut sheet: ResMut<Sheet>| {
            sheet.waiting = false;
            match captured.image.clone().try_into_dynamic() {
                Ok(picture) => {
                    let picture = picture.to_rgb8();
                    let (width, height) = (picture.width(), picture.height());
                    sheet.cells.push(cut(width, height, &picture.into_raw()));
                }
                Err(error) => error!("workbench: could not read a screenshot: {error}"),
            }
        },
    );
}

/// The middle of a picture, as wide as three quarters of its height, at half size.
fn cut(width: u32, height: u32, rgb: &[u8]) -> Cell {
    let keep = (height * 3 / 4).min(width);
    let left = (width - keep) / 2;
    let (out_w, out_h) = (keep / 2, height / 2);
    let mut out = Vec::with_capacity((out_w * out_h * 3) as usize);
    for y in 0..out_h {
        for x in 0..out_w {
            for channel in 0..3 {
                let mut sum = 0u32;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let at = ((y * 2 + dy) * width + left + x * 2 + dx) * 3 + channel;
                    sum += u32::from(rgb[at as usize]);
                }
                out.push((sum / 4) as u8);
            }
        }
    }
    Cell {
        width: out_w,
        height: out_h,
        rgb: out,
    }
}

/// Lays the cells out `columns` wide, row by row, and saves them as one picture.
fn save(cells: &[Cell], columns: usize, path: &PathBuf) {
    let Some(first) = cells.first() else { return };
    let (cell_w, cell_h) = (first.width as usize, first.height as usize);
    let rows = cells.len().div_ceil(columns);
    let (width, height) = (cell_w * columns, cell_h * rows);
    let mut rgba = vec![255u8; width * height * 4];
    for (index, cell) in cells.iter().enumerate() {
        let (left, top) = ((index % columns) * cell_w, (index / columns) * cell_h);
        for y in 0..cell_h.min(cell.height as usize) {
            for x in 0..cell_w.min(cell.width as usize) {
                let from = (y * cell.width as usize + x) * 3;
                let to = ((top + y) * width + left + x) * 4;
                rgba[to..to + 3].copy_from_slice(&cell.rgb[from..from + 3]);
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
    match image.try_into_dynamic().map(|picture| picture.save(path)) {
        Ok(Ok(())) => println!("SANDBOX saved {}", path.display()),
        Ok(Err(error)) => error!("workbench: could not save {}: {error}", path.display()),
        Err(error) => error!("workbench: could not make {}: {error}", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_is_the_middle_at_half_size() {
        // 8 wide, 4 high, each pixel's red its column: the middle three quarters of the
        // height, 3 wide from column 2, halved to one pixel by two.
        let rgb: Vec<u8> = (0..4)
            .flat_map(|_| (0..8u8).flat_map(|x| [x * 10, 0, 0]))
            .collect();
        let cell = cut(8, 4, &rgb);
        assert_eq!((cell.width, cell.height), (1, 2));
        // The average of columns 2 and 3.
        assert_eq!(cell.rgb[0], 25);
    }
}
