//! A frame-rate limit, as the player sets it (D25, D332): off, a number of frames a second, or
//! the screen's own rate. A frame limiter, as shipped games and RivaTuner have, that waits
//! out the rest of each frame so frames begin an even period apart, for a player who turns
//! vsync off and wants a steady rate without tearing through hundreds of frames, or less
//! heat and fan.
//!
//! Bevy 0.20 has no limiter of its own (`WinitSettings` only wakes a window that waits for
//! input). After bevy_framepace (MIT or Apache 2.0, Aevyrie), which has no release for
//! Bevy 0.20: the wait is at the end of the render world's frame, after the frame is
//! handed to the screen and before the render world sends the time of day to the game, so
//! frames reach the screen evenly and the game's clock steps by the period. It sleeps the
//! system's sleep for most of the wait and spins the last millisecond, which the system's
//! sleep cannot be trusted with, as `spin_sleep` does. Unlike bevy_framepace's, the next
//! frame is due a period after the last one was due, not after it ended, so a frame a
//! little late is made up and the rate averages exactly what was asked.
//!
//! The game says what limit applies through [`FrameLimit`]; nothing else does, so a
//! benchmark and every scripted run stay uncapped and measure the true frame time.

use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
    },
    window::{Monitor, OnMonitor, PrimaryMonitor, PrimaryWindow},
};
use std::time::{Duration, Instant};

/// The frame-rate limit that applies now. The game sets it from the player's settings,
/// and leaves it off while vsync paces frames instead.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Resource)]
pub enum FrameLimit {
    /// As fast as frames can be made.
    #[default]
    Off,
    /// At most this many frames a second.
    Fps(u32),
    /// The screen's own refresh rate, or off if the system does not say it.
    Screen,
}

impl FrameLimit {
    /// The choices a player is offered, in order.
    pub const CHOICES: [FrameLimit; 7] = [
        FrameLimit::Off,
        FrameLimit::Fps(30),
        FrameLimit::Fps(60),
        FrameLimit::Fps(75),
        FrameLimit::Fps(120),
        FrameLimit::Fps(144),
        FrameLimit::Screen,
    ];

    /// Its name in the settings file.
    pub fn name(self) -> String {
        match self {
            FrameLimit::Off => "off".into(),
            FrameLimit::Fps(fps) => fps.to_string(),
            FrameLimit::Screen => "screen".into(),
        }
    }

    pub fn from_name(name: &str) -> Option<FrameLimit> {
        match name {
            "off" => Some(FrameLimit::Off),
            "screen" => Some(FrameLimit::Screen),
            fps => fps
                .parse()
                .ok()
                .filter(|fps| (10..=1000).contains(fps))
                .map(FrameLimit::Fps),
        }
    }

    /// How long a frame takes at this limit, given the screen's refresh rate if known.
    pub fn period(self, screen_millihertz: Option<u32>) -> Option<Duration> {
        let hertz = match self {
            FrameLimit::Off => return None,
            FrameLimit::Fps(fps) => f64::from(fps),
            FrameLimit::Screen => f64::from(screen_millihertz.filter(|&mhz| mhz > 0)?) / 1000.0,
        };
        Some(Duration::from_secs_f64(1.0 / hertz))
    }
}

/// The screen's refresh rate the window is on, in millihertz, if the system says it.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScreenRate(pub Option<u32>);

/// How long a frame takes at the limit, for the render world to wait out.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Period(Option<Duration>);

impl ExtractResource<RenderApp> for Period {
    type Source = Period;

    fn extract_resource(source: &Period) -> Self {
        *source
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<FrameLimit>()
        .register_type::<FrameLimit>()
        .init_resource::<ScreenRate>()
        .init_resource::<Period>()
        .add_systems(Last, (read_screen_rate, set_period).chain());
    if app.get_sub_app(RenderApp).is_some() {
        app.add_plugins(ExtractResourcePlugin::<Period>::default());
        app.sub_app_mut(RenderApp)
            .init_resource::<Period>()
            .add_systems(Render, pace.in_set(RenderSystems::PostCleanup));
    }
}

/// The refresh rate of the screen the window is on, or of the primary screen.
fn read_screen_rate(
    window: Query<Option<&OnMonitor>, With<PrimaryWindow>>,
    monitors: Query<(&Monitor, Has<PrimaryMonitor>)>,
    mut rate: ResMut<ScreenRate>,
) {
    let on = window.single().ok().flatten().map(|on| on.0);
    let found = on
        .and_then(|monitor| monitors.get(monitor).ok())
        .or_else(|| monitors.iter().find(|(_, primary)| *primary))
        .and_then(|(monitor, _)| monitor.refresh_rate_millihertz);
    if rate.0 != found {
        rate.0 = found;
    }
}

fn set_period(limit: Res<FrameLimit>, rate: Res<ScreenRate>, mut period: ResMut<Period>) {
    let wanted = Period(limit.period(rate.0));
    if *period != wanted {
        *period = wanted;
    }
}

/// Waits out the rest of the frame, in the render world after the frame is presented.
fn pace(period: Res<Period>, mut pacer: Local<Pacer>) {
    let Some(period) = period.0 else {
        pacer.next = None;
        return;
    };
    let now = Instant::now();
    sleep_until(now + pacer.wait(now, period));
}

/// When each frame may begin, so frames begin a period apart.
#[derive(Default)]
pub struct Pacer {
    /// When the next frame is due.
    next: Option<Instant>,
}

impl Pacer {
    /// How long to wait at `now`, as a frame ends, before the next begins, for frames a
    /// `period` apart. A frame a little late is made up on the next; one late by more than
    /// a whole period (a hitch, a load, the limit just set) starts the count again from
    /// now, rather than rushing frames out to catch up.
    pub fn wait(&mut self, now: Instant, period: Duration) -> Duration {
        let due = match self.next {
            Some(due) if now <= due + period => due,
            _ => now,
        };
        self.next = Some(due + period);
        due.saturating_duration_since(now)
    }
}

/// The last stretch of a wait spent spinning: a system's sleep can wake a millisecond
/// late, on Windows more without a high-resolution timer, which at 75 fps is a frame
/// uneven by a tenth.
const SPIN: Duration = Duration::from_millis(1);

/// Sleeps until `deadline`, precisely: the system's sleep for all but the last
/// millisecond, then yielding until it comes.
pub fn sleep_until(deadline: Instant) {
    loop {
        let now = Instant::now();
        if now >= deadline {
            return;
        }
        let left = deadline - now;
        if left > SPIN {
            std::thread::sleep(left - SPIN);
        } else {
            std::thread::yield_now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT_75: Duration = Duration::from_nanos(13_333_333);

    /// Runs frames whose work takes the given times through a pacer, on a made-up clock,
    /// and gives when each frame began.
    fn begin_times(work: &[Duration], period: Duration) -> Vec<Duration> {
        let start = Instant::now();
        let mut pacer = Pacer::default();
        // The frame before the first ends at the start, so the beat begins there.
        assert_eq!(pacer.wait(start, period), Duration::ZERO);
        let mut now = start;
        let mut begins = Vec::new();
        for &took in work {
            begins.push(now - start);
            now += took;
            now += pacer.wait(now, period);
        }
        begins
    }

    #[test]
    fn frames_quicker_than_the_limit_begin_a_period_apart() {
        // Work from 2 to 11 ms, uneven, against 75 a second: every frame still begins
        // 13.33 ms after the one before.
        let work: Vec<Duration> = (0..20)
            .map(|i| Duration::from_micros(2_000 + (i * 4_700) % 9_000))
            .collect();
        let begins = begin_times(&work, AT_75);
        for pair in begins.windows(2) {
            assert_eq!(pair[1] - pair[0], AT_75);
        }
    }

    #[test]
    fn a_frame_a_little_late_is_made_up_on_the_next() {
        // One frame takes 16 ms, past the 13.33: the next begins at once, and the one
        // after is back on the even beat, so the rate over the run is still 75.
        let mut work = vec![Duration::from_millis(5); 6];
        work[2] = Duration::from_millis(16);
        let begins = begin_times(&work, AT_75);
        assert_eq!(begins[3], begins[2] + Duration::from_millis(16));
        assert_eq!(begins[4], begins[0] + AT_75 * 4);
        assert_eq!(begins[5], begins[0] + AT_75 * 5);
    }

    #[test]
    fn a_hitch_starts_the_count_again_rather_than_rushing_frames_out() {
        // A 100 ms hitch: afterwards frames are a period apart again, not run back to
        // back to catch up the seven frames missed.
        let mut work = vec![Duration::from_millis(5); 6];
        work[1] = Duration::from_millis(100);
        let begins = begin_times(&work, AT_75);
        for pair in begins[2..].windows(2) {
            assert_eq!(pair[1] - pair[0], AT_75);
        }
    }

    #[test]
    fn frames_slower_than_the_limit_never_wait() {
        let mut pacer = Pacer::default();
        let mut now = Instant::now();
        for _ in 0..10 {
            now += Duration::from_millis(20);
            assert_eq!(pacer.wait(now, AT_75), Duration::ZERO);
        }
    }

    #[test]
    fn a_limit_is_a_period_and_the_screen_its_own_rate() {
        assert_eq!(FrameLimit::Off.period(Some(144_000)), None);
        assert_eq!(
            FrameLimit::Fps(75).period(None),
            Some(Duration::from_secs_f64(1.0 / 75.0))
        );
        assert_eq!(
            FrameLimit::Screen.period(Some(143_856)),
            Some(Duration::from_secs_f64(1.0 / 143.856))
        );
        // The system did not say: no limit, rather than a guess.
        assert_eq!(FrameLimit::Screen.period(None), None);
        assert_eq!(FrameLimit::Screen.period(Some(0)), None);
    }

    #[test]
    fn limits_are_named_and_read_back() {
        for limit in FrameLimit::CHOICES {
            assert_eq!(FrameLimit::from_name(&limit.name()), Some(limit));
        }
        assert_eq!(FrameLimit::from_name("fast"), None);
        assert_eq!(FrameLimit::from_name("0"), None);
    }

    #[test]
    fn sleeping_until_a_moment_never_wakes_before_it() {
        let deadline = Instant::now() + Duration::from_millis(3);
        sleep_until(deadline);
        assert!(Instant::now() >= deadline);
        // A moment already past returns at once.
        sleep_until(Instant::now() - Duration::from_millis(1));
    }
}
