//! The workbench's film: each `--anim` clip once through at its own speed, seen from each
//! of `--sides`, as numbered frames `<prefix>-<clip>-<side>-<frame>.png`, thirty to the
//! clip's second, for ffmpeg to make a film of (the README has the line). Speed, weight
//! and a breath cannot be judged on a sheet's stills. Each frame is the clip held at that
//! frame's time, so a slow machine films the same clip as a fast one. The clip is first
//! measured at every frame, and one camera a side holds every model's joints through all
//! of it, as the sheet's does (`sheet::stand`). Then the sandbox exits.

use std::path::PathBuf;
use terra_world::planet::MainCamera;

use bevy::{prelude::*, render::view::screenshot::ScreenshotCaptured};

use super::{Bench, pose, sheet};
use terra_world::shell::Options;

/// Frames to a second of the clip.
pub(super) const RATE: f32 = 30.0;
/// Frames to let a pose settle before each capture, so nothing of the frame before is left
/// in it by the renderer's history.
const SETTLE_POSE: u32 = 8;

#[derive(Resource, Default)]
pub(super) struct Film {
    /// The clip, the side and the frame next.
    clip: usize,
    side: usize,
    frame: u32,
    waited: u32,
    /// Screenshots on their way.
    waiting: u32,
    /// How many of this clip's frames have been measured, and where every model's joints
    /// were at each (`sheet::reach`).
    measured: u32,
    reach: Vec<Vec3>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Film>();
}

/// How many frames a clip `length` seconds long is filmed in: one each thirtieth of a
/// second from its start to before its end, so a loop's last frame is followed by its
/// first with no pose shown twice; a clip of no length is one frame.
fn frames(length: f32) -> u32 {
    (libm::ceilf(length * RATE - 1e-3).max(1.0)) as u32
}

/// How far through a clip `length` seconds long its `frame` is.
fn fraction(frame: u32, length: f32) -> f32 {
    if length <= 0.0 {
        return 0.0;
    }
    (frame as f32 / RATE / length).min(1.0)
}

/// What is kept of a picture `width` by `height`: its left edge and width, the middle
/// three quarters of its height across (a figure alone) or the `whole` of it (a row with
/// props), an even number of pixels, as a film's encoder needs.
fn kept(width: u32, height: u32, whole: bool) -> (u32, u32) {
    let keep = if whole {
        width
    } else {
        (height * 3 / 4).min(width)
    } & !1;
    ((width - keep) / 2, keep)
}

pub fn capture(
    mut commands: Commands,
    bench: Option<ResMut<Bench>>,
    options: Res<Options>,
    picture: Option<Res<terra_world::shell::Picture>>,
    mut film: ResMut<Film>,
    mut players: Query<&mut AnimationPlayer>,
    mut camera: Query<(&mut Transform, &Projection), With<MainCamera>>,
    (children, names): (Query<&Children>, Query<&Name>),
    (globals, boxes): (
        Query<&GlobalTransform>,
        Query<(&bevy::camera::primitives::Aabb, &GlobalTransform)>,
    ),
    mut frames_run: Local<u32>,
    (screen, hold): (
        Res<State<terra_world::loading::Screen>>,
        Res<terra_world::loading::Hold>,
    ),
    mut exit: MessageWriter<AppExit>,
) {
    let (Some(mut bench), Some(prefix)) = (bench, options.film.as_ref()) else {
        return;
    };
    if !terra_world::shell::counting(&options, &screen, &hold) {
        return;
    }
    *frames_run += 1;
    if *frames_run < options.settle_frames || !bench.ready() {
        return;
    }
    let Some(clip) = bench.clips.get(film.clip).cloned() else {
        // Every frame asked for; the run ends once the last is saved.
        if film.waiting == 0 {
            exit.write(AppExit::Success);
        }
        return;
    };
    let sides: &[f32] = if options.sides.is_empty() {
        &sheet::SIDES
    } else {
        &options.sides
    };
    // The clip's length on the model it is longest on.
    let length = bench
        .models
        .iter()
        .flat_map(|model| model.clips.iter().filter(|each| each.name == clip))
        .map(|each| each.length)
        .fold(0.0, f32::max);
    let count = frames(length);
    // First where the clip takes each body, a frame at a time.
    if options.focus.is_none() && film.measured < count {
        if film.waited == 0 {
            let at = fraction(film.measured, length);
            pose(&mut bench, &clip, Some(at), &mut players);
        }
        film.waited += 1;
        if film.waited >= sheet::SETTLE_REACH {
            film.waited = 0;
            film.measured += 1;
            sheet::reach(&bench, &children, (&globals, &boxes), &mut film.reach);
        }
        return;
    }
    let side = sides[film.side];
    if film.waited == 0 {
        let at = fraction(film.frame, length);
        pose(&mut bench, &clip, Some(at), &mut players);
    }
    if let Ok((mut transform, projection)) = camera.single_mut() {
        *transform = sheet::stand(
            &bench,
            (options.focus.as_deref(), side),
            &film.reach,
            projection,
            (&children, &names),
            (&globals, &boxes),
        );
    }
    film.waited += 1;
    if film.waited < SETTLE_POSE {
        return;
    }
    film.waited = 0;
    let stem = format!("{}-{clip}-{side:.0}", prefix.display());
    let path = PathBuf::from(format!("{stem}-{:04}.png", film.frame));
    film.waiting += 1;
    let whole = bench.wide;
    commands
        .spawn(terra_world::shell::screenshot(picture.as_deref()))
        .observe(
            move |captured: On<ScreenshotCaptured>, mut film: ResMut<Film>| {
                film.waiting -= 1;
                save(&captured.image, whole, &path);
            },
        );
    film.frame += 1;
    if film.frame < count {
        return;
    }
    println!("SANDBOX filmed {stem}-%04d.png: {count} frames, {length:.2} s");
    film.frame = 0;
    film.side += 1;
    if film.side == sides.len() {
        film.side = 0;
        film.clip += 1;
        film.measured = 0;
        film.reach.clear();
    }
}

/// Saves what a film keeps of a screenshot (`kept`) as `path`.
fn save(image: &Image, whole: bool, path: &PathBuf) {
    let picture = match image.clone().try_into_dynamic() {
        Ok(picture) => picture,
        Err(error) => {
            error!("workbench: could not read a screenshot: {error}");
            return;
        }
    };
    let (left, keep) = kept(picture.width(), picture.height(), whole);
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).ok();
    }
    let frame = picture.crop_imm(left, 0, keep, picture.height() & !1);
    if let Err(error) = frame.to_rgb8().save(path) {
        error!("workbench: could not save {}: {error}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clip_is_filmed_thirty_frames_to_its_second_from_its_start_to_before_its_end() {
        // Two seconds: frames at 0, 1/30 … 59/30 s; the pose at 2 s is a loop's first again.
        assert_eq!(frames(2.0), 60);
        assert_eq!(fraction(0, 2.0), 0.0);
        assert!((fraction(30, 2.0) - 0.5).abs() < 1e-6);
        assert!((fraction(59, 2.0) * 2.0 - 59.0 / 30.0).abs() < 1e-5);
        // A clip ending between two frames keeps the frame before its end.
        assert_eq!(frames(2.01), 61);
        assert!(fraction(60, 2.01) < 1.0);
        assert_eq!(frames(0.05), 2);
        // A clip of no length (a model held still) is one frame, at its start.
        assert_eq!((frames(0.0), fraction(0, 0.0)), (1, 0.0));
        // No frame is past the clip's end, whatever is asked.
        assert_eq!(fraction(90, 2.0), 1.0);
    }

    #[test]
    fn a_frame_keeps_the_middle_at_an_even_width() {
        // 1920 by 1080: the middle 810 across, from 555.
        assert_eq!(kept(1920, 1080, false), (555, 810));
        assert_eq!(kept(1920, 1080, true), (0, 1920));
        // 1000 high: three quarters is 750; 1001 high: 750 too, made even.
        assert_eq!(kept(1920, 1001, false).1, 750);
        // A picture narrower than that is kept whole.
        assert_eq!(kept(601, 1000, false), (0, 600));
    }
}
