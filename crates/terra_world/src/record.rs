//! What the run has said, kept in memory as numbered lines so a script can ask what
//! happened since it last looked, or wait for something to happen, instead of counting
//! frames and searching a log file afterwards (D69). Every message the log keeps goes in,
//! with the frame it was said on, so anything a system already logs can be checked
//! without changing it.

use bevy::{
    diagnostic::FrameCount,
    log::{
        BoxedLayer, Level,
        tracing::{
            Event, Subscriber,
            field::{Field, Visit},
        },
        tracing_subscriber::{Layer, layer::Context},
    },
    prelude::*,
};
use std::{
    collections::VecDeque,
    fmt::Write,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

/// How many lines are kept; older ones are dropped.
const KEPT: usize = 20_000;

/// The run's lines so far, the newest last.
#[derive(Resource, Clone, Default)]
pub struct Record(Arc<Shared>);

#[derive(Default)]
struct Shared {
    lines: Mutex<Lines>,
    /// The frame now, copied from the world each frame, since a log message cannot reach it.
    frame: AtomicU32,
}

#[derive(Default)]
struct Lines {
    /// The number the next line will get.
    next: u64,
    kept: VecDeque<Line>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub number: u64,
    pub frame: u32,
    pub level: Level,
    /// The module that said it, such as `terra_sandbox::experiments::play::combat`.
    pub source: String,
    pub text: String,
}

impl Record {
    /// The lines numbered `since` or later whose text contains `containing`, the oldest
    /// first, at most `most` of them; and the number the next line will get.
    pub fn read(&self, since: u64, containing: Option<&str>, most: usize) -> (Vec<Line>, u64) {
        let lines = self
            .0
            .lines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let found = lines
            .kept
            .iter()
            .filter(|line| line.number >= since)
            .filter(|line| containing.is_none_or(|text| line.text.contains(text)))
            .take(most)
            .cloned()
            .collect();
        (found, lines.next)
    }

    /// Every line kept, the oldest first, and the number the next will get, for a crash
    /// capsule (`crash`). A panic can come while this thread holds the lines, so it tries
    /// for a moment rather than waiting, and gives none if they stay held.
    pub fn everything(&self) -> (Vec<Line>, u64) {
        for _ in 0..50 {
            if let Ok(lines) = self.0.lines.try_lock() {
                return (lines.kept.iter().cloned().collect(), lines.next);
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        (Vec::new(), 0)
    }

    /// The frame now, as the record last saw it.
    pub fn frame(&self) -> u32 {
        self.0.frame.load(Ordering::Relaxed)
    }

    fn add(&self, level: Level, source: &str, text: String) {
        let frame = self.0.frame.load(Ordering::Relaxed);
        let mut lines = self
            .0
            .lines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let number = lines.next;
        lines.next += 1;
        if lines.kept.len() == KEPT {
            lines.kept.pop_front();
        }
        lines.kept.push_back(Line {
            number,
            frame,
            level,
            source: source.to_string(),
            text,
        });
    }
}

/// Engine messages that say what is normal, kept out of the log so what is left is worth
/// reading. Warnings and errors still come through. `RUST_LOG` replaces all of this. The
/// render thread's span, `pipelined_rendering`'s, is let through for the frame recorder (D576).
const QUIET: &str = "\
bevy_render=warn,bevy_render::pipelined_rendering=info,bevy_pbr=warn,bevy_winit=warn,bevy_egui=warn,bevy_diagnostic=warn,\
sctk_adwaita=off";

/// Bevy's spans of each system run off and those of each schedule run on, unless
/// the session report is asked for (`--profile-report=<file>`) or a Chrome trace build, so a
/// `hitch-log` build (D576) names schedules unless a report wants systems. A span disabled
/// when it is made costs nothing to enter.
const SCHEDULES_ONLY: &str = ",bevy_ecs=warn,bevy_ecs::schedule::schedule=info";

/// The log plugin's filter.
pub fn filter() -> String {
    let systems = cfg!(feature = "trace") || crate::recorder::reporting();
    format!(
        "{},{QUIET}{}",
        bevy::log::DEFAULT_FILTER,
        if systems { "" } else { SCHEDULES_ONLY }
    )
}

/// For the log plugin's `custom_layer`: starts the record and hands the log its writers,
/// the record's and the log file's (`log_file`). They sit behind the log's filter, so they
/// keep what the log keeps.
pub fn layer(app: &mut App) -> Option<BoxedLayer> {
    let record = Record::default();
    crate::crash::watch(&record);
    let kind = app
        .world()
        .get_resource::<crate::shell::Options>()
        .map_or_else(
            || crate::log_file::kind(&[], false),
            |options| crate::log_file::kind(&options.experiments, options.yard()),
        );
    let mut writers: Vec<BoxedLayer> = vec![Box::new(Writer(record.clone()))];
    writers.extend(crate::log_file::layer(&record, &kind));
    // What ran long in a slow frame, said in the log (D576).
    writers.push(crate::recorder::layer(app));
    app.insert_resource(record).add_systems(First, count_frames);
    Some(Box::new(writers))
}

fn count_frames(record: Res<Record>, frame: Res<FrameCount>) {
    record.0.frame.store(frame.0, Ordering::Relaxed);
}

struct Writer(Record);

impl<S: Subscriber> Layer<S> for Writer {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let metadata = event.metadata();
        self.0
            .add(*metadata.level(), metadata.target(), said(event));
    }
}

/// What a log message says: its text, then any fields given beside it.
pub(crate) fn said(event: &Event<'_>) -> String {
    let mut text = Text::default();
    event.record(&mut text);
    text.message + &text.fields
}

/// A message, then any fields given beside it as ` name=value`.
#[derive(Default)]
struct Text {
    message: String,
    fields: String,
}

impl Visit for Text {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            write!(self.message, "{value:?}").ok();
        } else {
            write!(self.fields, " {}={value:?}", field.name()).ok();
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            write!(self.fields, " {}={value}", field.name()).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_was_said_since_a_line() {
        let record = Record::default();
        record.add(Level::INFO, "here", "Gained 2 Stone.".into());
        record.add(Level::WARN, "here", "No room.".into());
        record.add(Level::INFO, "here", "Gained 1 Wood.".into());
        let (lines, next) = record.read(1, None, 10);
        assert_eq!(next, 3);
        assert_eq!(
            lines.iter().map(|line| line.number).collect::<Vec<_>>(),
            [1, 2]
        );
        let (gained, _) = record.read(0, Some("Gained"), 10);
        assert_eq!(gained.len(), 2);
        let (first, _) = record.read(0, Some("Gained"), 1);
        assert_eq!(first[0].text, "Gained 2 Stone.");
    }

    #[test]
    fn forgets_the_oldest_lines_past_its_limit() {
        let record = Record::default();
        for i in 0..KEPT + 5 {
            record.add(Level::INFO, "here", format!("line {i}"));
        }
        let (lines, next) = record.read(0, None, usize::MAX);
        assert_eq!(lines.len(), KEPT);
        assert_eq!(lines[0].number, 5);
        assert_eq!(next, (KEPT + 5) as u64);
    }

    #[test]
    fn keeps_what_the_log_says() {
        use bevy::log::tracing_subscriber::{Registry, layer::SubscriberExt};
        let record = Record::default();
        let subscriber = Registry::default().with(Writer(record.clone()));
        bevy::log::tracing::subscriber::with_default(subscriber, || {
            info!(count = 2, "Gained {} {}.", 2, "Stone");
        });
        let (lines, _) = record.read(0, None, 10);
        assert_eq!(lines[0].text, "Gained 2 Stone. count=2");
        assert_eq!(lines[0].level, Level::INFO);
    }
}
