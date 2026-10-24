//! One line in the log saying what the frame rate depends on besides the machine (D448):
//! the window's mode, its size in physical pixels and scale, whether it has focus, the
//! present mode, the screen's rate, the frame-rate limit and the quality preset; said once
//! the run has settled at its start, and again whenever any of it changes in play, so a
//! session's log explains its frame times (`debug` says the machine and the frames).
//!
//! It reads what the options menu sets (the window, `FrameLimit`, the preset), not the
//! menu, so it says what applies however it was set.

use crate::{
    pacing::{FrameLimit, ScreenRate},
    quality::{Asked, Automatic, Quality},
    shell::Options,
};
use bevy::{
    prelude::*,
    window::{PresentMode, PrimaryWindow, WindowMode},
};
use std::time::{Duration, Instant};

/// How long a setup must hold before it is said, so a window dragged to a new size is
/// said once, at the size it ends at.
const SETTLE: Duration = Duration::from_secs(1);

/// What the frame rate depends on, as plain values.
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    /// The window's mode, size and scale and whether it has focus; `None` offscreen.
    pub window: Option<(WindowMode, UVec2, f32, bool)>,
    /// The picture an offscreen run draws, if it draws one.
    pub picture: Option<UVec2>,
    pub present: PresentMode,
    /// The screen's refresh rate in millihertz, if known.
    pub screen: Option<u32>,
    pub limit: FrameLimit,
    /// The preset that applies, and whether it was asked for rather than chosen by the GPU.
    pub quality: Option<(Quality, bool)>,
}

impl Setup {
    /// The line the log keeps.
    pub fn describe(&self) -> String {
        let shown = match (self.picture, self.window) {
            (Some(picture), _) => format!("offscreen, a {}x{} picture", picture.x, picture.y),
            (None, Some((mode, size, scale, focused))) => {
                let mode = match mode {
                    WindowMode::Windowed => "windowed",
                    WindowMode::BorderlessFullscreen(_) => "borderless fullscreen",
                    WindowMode::Fullscreen(..) => "exclusive fullscreen",
                };
                let focus = if focused { "focused" } else { "not focused" };
                format!(
                    "{mode} {}x{} px at scale {scale:.2}, {focus}",
                    size.x, size.y
                )
            }
            (None, None) => "no window".to_string(),
        };
        let screen = self.screen.map_or_else(
            || "unknown".into(),
            |mhz| format!("{:.0} Hz", f64::from(mhz) / 1000.0),
        );
        let quality = match self.quality {
            Some((quality, true)) => format!("{} (asked for)", quality.name()),
            Some((quality, false)) => format!("{} (chosen from the GPU)", quality.name()),
            None => "none".into(),
        };
        format!(
            "Display: {shown}; present mode {:?}; screen {screen}; frame limit {}; quality {quality}.",
            self.present,
            self.limit.name()
        )
    }
}

/// Holds a setup back until it has stood for [`SETTLE`], and gives it once.
#[derive(Default)]
pub struct Settle {
    said: Option<Setup>,
    seen: Option<(Setup, Duration)>,
}

impl Settle {
    /// Sees the setup at `now`; gives it when it has stood long enough and differs from
    /// the one last given.
    pub fn see(&mut self, setup: Setup, now: Duration) -> Option<Setup> {
        if self.seen.as_ref().is_none_or(|(seen, _)| *seen != setup) {
            self.seen = Some((setup, now));
            return None;
        }
        let (seen, since) = self.seen.as_ref()?;
        if now.saturating_sub(*since) < SETTLE || self.said.as_ref() == Some(seen) {
            return None;
        }
        self.said = Some(seen.clone());
        self.said.clone()
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Last, say);
}

fn say(
    window: Query<&Window, With<PrimaryWindow>>,
    options: Option<Res<Options>>,
    limit: Res<FrameLimit>,
    screen: Res<ScreenRate>,
    quality: Option<Res<Quality>>,
    asked: Option<Res<Asked>>,
    automatic: Option<Res<Automatic>>,
    mut settle: Local<Settle>,
    // The wall clock's: a scripted run's clocks, `Time<Real>`'s too, move a sixtieth a frame.
    mut began: Local<Option<Instant>>,
) {
    let window = window.single().ok();
    let offscreen = options
        .as_ref()
        .is_some_and(|options| options.offscreen && options.unattended());
    // Asked for by `--quality` or the menu, or otherwise not what the GPU chose.
    let asked = asked.is_some_and(|asked| asked.0.is_some())
        || automatic.map(|automatic| automatic.0) != quality.as_deref().copied();
    let setup = Setup {
        window: window.filter(|_| !offscreen).map(|w| {
            (
                w.mode,
                UVec2::new(w.physical_width(), w.physical_height()),
                w.scale_factor(),
                w.focused,
            )
        }),
        picture: options.filter(|_| offscreen).map(|options| options.picture),
        present: window.map_or(PresentMode::AutoVsync, |w| w.present_mode),
        screen: screen.0,
        limit: *limit,
        quality: quality.map(|quality| (*quality, asked)),
    };
    let now = began.get_or_insert_with(Instant::now).elapsed();
    if let Some(setup) = settle.see(setup, now) {
        info!("{}", setup.describe());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::window::MonitorSelection;

    fn windowed() -> Setup {
        Setup {
            window: Some((WindowMode::Windowed, UVec2::new(2560, 1440), 1.5, true)),
            picture: None,
            present: PresentMode::AutoVsync,
            screen: Some(143_998),
            limit: FrameLimit::Off,
            quality: Some((Quality::High, false)),
        }
    }

    #[test]
    fn says_what_the_frame_rate_depends_on() {
        assert_eq!(
            windowed().describe(),
            "Display: windowed 2560x1440 px at scale 1.50, focused; present mode AutoVsync; \
             screen 144 Hz; frame limit off; quality high (chosen from the GPU)."
        );
        let offscreen = Setup {
            window: None,
            picture: Some(UVec2::new(1920, 1080)),
            screen: None,
            limit: FrameLimit::Fps(60),
            quality: Some((Quality::Low, true)),
            ..windowed()
        };
        assert_eq!(
            offscreen.describe(),
            "Display: offscreen, a 1920x1080 picture; present mode AutoVsync; screen unknown; \
             frame limit 60; quality low (asked for)."
        );
    }

    #[test]
    fn a_setup_is_said_once_it_holds_and_again_when_it_changes() {
        let second = |s: f32| Duration::from_secs_f32(s);
        let mut settle = Settle::default();
        assert_eq!(settle.see(windowed(), second(0.0)), None);
        assert_eq!(settle.see(windowed(), second(0.5)), None);
        assert_eq!(settle.see(windowed(), second(1.0)), Some(windowed()));
        // Said once only.
        assert_eq!(settle.see(windowed(), second(5.0)), None);
        // Dragged through sizes: only the last, once it holds, is said.
        let sized = |x: u32| Setup {
            window: Some((WindowMode::Windowed, UVec2::new(x, 900), 1.5, true)),
            ..windowed()
        };
        assert_eq!(settle.see(sized(1700), second(6.0)), None);
        assert_eq!(settle.see(sized(1800), second(6.5)), None);
        assert_eq!(settle.see(sized(1800), second(7.0)), None);
        assert_eq!(settle.see(sized(1800), second(7.6)), Some(sized(1800)));
        // Fullscreen and back to the first: each is said.
        let full = Setup {
            window: Some((
                WindowMode::BorderlessFullscreen(MonitorSelection::Current),
                UVec2::new(3840, 2160),
                1.5,
                true,
            )),
            ..windowed()
        };
        settle.see(full.clone(), second(8.0));
        assert!(settle.see(full, second(9.0)).is_some());
        settle.see(windowed(), second(10.0));
        assert_eq!(settle.see(windowed(), second(11.0)), Some(windowed()));
    }
}
