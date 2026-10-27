//! Tunables (D547): named numbers a running game can change, so trying a value is a call to a
//! running sandbox, not a rebuild and a restart. Unreal's console variables, ported in outline
//! ("Console Variables in C++", Epic's Unreal Engine documentation: `TAutoConsoleVariable`,
//! `IConsoleManager`, `ECVF_SetBy*`): the module that owns a value declares it where it uses
//! it, as a static with its name, default, range and a line of help, and reads it each time it
//! needs it; the sandbox's remote lists, reads and sets them (`tune.*`, `r tune`) and the
//! command line sets them at the start (`--tune NAME=VALUE`).
//!
//! ```ignore
//! /// Where the clouds' body noise starts (cloud_volume.wesl).
//! pub static BODY_LOW: Tunable = Tunable::new("sky.cloud.body_low", 0.6, 0.0..=1.0, "...");
//! // In the owner's plugin, so `tune list` and `--tune` know it from the start:
//! tune::register(&[&BODY_LOW]);
//! // Wherever it is used:
//! let low = BODY_LOW.get();
//! ```
//!
//! Plain Rust with no engine, so any crate's code can own one (D28). Rust has no code before
//! `main` without `unsafe` (Unreal registers from static constructors), so the owner registers
//! its tunables when its plugin is built; one read before that registers it too. A change is
//! seen by polling, as Epic's page recommends over callbacks: read the value where it is used,
//! or compare [`changes`] with the count last seen. Changed values last only as long as the run.

use std::ops::RangeInclusive;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

/// A named number a running game can change.
#[derive(Debug)]
pub struct Tunable {
    /// Dotted, lowercase, from the owner down: `sky.cloud.body_low`.
    pub name: &'static str,
    /// The value it has unless changed.
    pub default: f32,
    /// The least and most it may be set to.
    pub min: f32,
    pub max: f32,
    /// One line: what it does and what more or less of it looks like.
    pub doc: &'static str,
    bits: AtomicU32,
    by: AtomicU8,
    registered: AtomicBool,
}

/// What last set a tunable, as Unreal's `ECVF_SetBy*` keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetBy {
    Default,
    CommandLine,
    Remote,
}

impl SetBy {
    pub fn name(self) -> &'static str {
        match self {
            SetBy::Default => "default",
            SetBy::CommandLine => "command line",
            SetBy::Remote => "remote",
        }
    }

    fn from_u8(n: u8) -> Self {
        match n {
            1 => SetBy::CommandLine,
            2 => SetBy::Remote,
            _ => SetBy::Default,
        }
    }
}

impl Tunable {
    pub const fn new(
        name: &'static str,
        default: f32,
        range: RangeInclusive<f32>,
        doc: &'static str,
    ) -> Self {
        let (min, max) = (*range.start(), *range.end());
        assert!(
            min <= default && default <= max,
            "a tunable's default is in its range"
        );
        Self {
            name,
            default,
            min,
            max,
            doc,
            bits: AtomicU32::new(default.to_bits()),
            by: AtomicU8::new(0),
            registered: AtomicBool::new(false),
        }
    }

    /// Its value now.
    pub fn get(&'static self) -> f32 {
        if !self.registered.load(Ordering::Relaxed) {
            register(&[self]);
        }
        f32::from_bits(self.bits.load(Ordering::Relaxed))
    }

    /// Sets it, within its range, and answers what it was.
    pub fn set(&'static self, value: f32, by: SetBy) -> Result<f32, String> {
        if !(self.min..=self.max).contains(&value) {
            return Err(format!(
                "{} is {} to {}, not {value}",
                self.name, self.min, self.max
            ));
        }
        let was = self.get();
        self.bits.store(value.to_bits(), Ordering::Relaxed);
        self.by.store(by as u8, Ordering::Relaxed);
        CHANGES.fetch_add(1, Ordering::Relaxed);
        Ok(was)
    }

    /// Back to its default, and what it was.
    pub fn reset(&'static self) -> f32 {
        self.set(self.default, SetBy::Default)
            .unwrap_or(self.default)
    }

    /// What last set it.
    pub fn set_by(&self) -> SetBy {
        SetBy::from_u8(self.by.load(Ordering::Relaxed))
    }
}

static TUNABLES: Mutex<Vec<&'static Tunable>> = Mutex::new(Vec::new());
static CHANGES: AtomicU64 = AtomicU64::new(0);

fn tunables() -> MutexGuard<'static, Vec<&'static Tunable>> {
    // A panic elsewhere while it was held leaves the list whole: every change is one push.
    TUNABLES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Makes the tunables known by name, from the owner's plugin, so they are listed and can be set
/// from the start. Registering one twice does nothing; two with one name is a mistake.
pub fn register(list: &[&'static Tunable]) {
    let mut all = tunables();
    for &tunable in list {
        if tunable.registered.swap(true, Ordering::AcqRel) {
            continue;
        }
        assert!(
            well_named(tunable.name),
            "a tunable's name is dotted lowercase words, not `{}`",
            tunable.name
        );
        assert!(
            all.iter().all(|other| other.name != tunable.name),
            "two tunables are called {}",
            tunable.name
        );
        all.push(tunable);
    }
}

fn well_named(name: &str) -> bool {
    name.split('.').count() >= 2
        && name.split('.').all(|word| {
            !word.is_empty()
                && word
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        })
}

/// The tunable called exactly `name`.
pub fn find(name: &str) -> Option<&'static Tunable> {
    tunables().iter().copied().find(|t| t.name == name)
}

/// Every tunable known, by name.
pub fn all() -> Vec<&'static Tunable> {
    let mut all = tunables().clone();
    all.sort_by_key(|t| t.name);
    all
}

/// How many times any tunable has been set: compare with the count last seen to know whether
/// anything changed.
pub fn changes() -> u64 {
    CHANGES.load(Ordering::Relaxed)
}

/// The tunable called `name`, or why not, naming those it may have meant.
pub fn named(name: &str) -> Result<&'static Tunable, String> {
    find(name).ok_or_else(|| {
        let near = near(name);
        if near.is_empty() {
            format!("no tunable is called {name}; `tune list` lists them")
        } else {
            format!(
                "no tunable is called {name}; did you mean {}?",
                near.join(", ")
            )
        }
    })
}

/// Names sharing its group (all but the last word) or its last word, at most five.
fn near(name: &str) -> Vec<&'static str> {
    let (group, last) = name.rsplit_once('.').unwrap_or(("", name));
    all()
        .into_iter()
        .map(|t| t.name)
        .filter(|n| {
            let (g, l) = n.rsplit_once('.').unwrap_or(("", n));
            (!group.is_empty() && g == group) || (!last.is_empty() && l.contains(last))
        })
        .take(5)
        .collect()
}

/// `NAME=VALUE`, as `--tune` takes it.
pub fn parse_assignment(text: &str) -> Result<(String, f32), String> {
    let (name, value) = text
        .split_once('=')
        .ok_or(format!("--tune takes NAME=VALUE, not `{text}`"))?;
    let value = value
        .trim()
        .parse::<f32>()
        .map_err(|_| format!("--tune {name} wants a number, not `{value}`"))?;
    Ok((name.trim().to_string(), value))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every test owns its tunables: the list is the process's, shared by tests on other threads.
    static SET: Tunable = Tunable::new("test.tune.set", 0.5, 0.0..=1.0, "for the tests");
    static LAZY: Tunable = Tunable::new("test.tune.lazy", 2.0, 1.0..=3.0, "for the tests");
    static RESET: Tunable = Tunable::new("test.tune.reset", 0.0, -1.0..=1.0, "for the tests");

    #[test]
    fn a_value_set_is_read_back_and_one_out_of_range_is_refused() {
        register(&[&SET]);
        assert_eq!(SET.get(), 0.5);
        assert_eq!(SET.set(0.25, SetBy::Remote), Ok(0.5));
        assert_eq!(SET.get(), 0.25);
        assert_eq!(SET.set_by(), SetBy::Remote);
        let refused = SET.set(1.5, SetBy::Remote).unwrap_err();
        assert!(refused.contains("0 to 1"), "{refused}");
        assert!(SET.set(f32::NAN, SetBy::Remote).is_err());
        assert_eq!(SET.get(), 0.25, "a refused value leaves it as it was");
    }

    #[test]
    fn a_tunable_read_before_its_owner_registered_it_is_known_by_name() {
        assert_eq!(LAZY.get(), 2.0);
        assert!(std::ptr::eq(find("test.tune.lazy").unwrap(), &LAZY));
        register(&[&LAZY]);
        assert_eq!(all().iter().filter(|t| t.name == LAZY.name).count(), 1);
    }

    #[test]
    fn reset_brings_back_the_default_and_counts_as_a_change() {
        RESET.set(0.75, SetBy::CommandLine).unwrap();
        let before = changes();
        assert_eq!(RESET.reset(), 0.75);
        assert_eq!(RESET.get(), 0.0);
        assert_eq!(RESET.set_by(), SetBy::Default);
        assert!(changes() > before);
    }

    #[test]
    fn a_wrong_name_says_which_it_may_have_meant() {
        register(&[&SET, &RESET]);
        let why = named("test.tune.sett").unwrap_err();
        assert!(
            why.contains("test.tune.set") && why.contains("test.tune.reset"),
            "{why}"
        );
        assert!(named("nothing.like.it").unwrap_err().contains("tune list"));
    }

    #[test]
    fn names_are_dotted_lowercase_words() {
        assert!(well_named("sky.cloud.body_low"));
        assert!(well_named("build.lod2"));
        assert!(!well_named("body_low"));
        assert!(!well_named("sky.Cloud"));
        assert!(!well_named("sky..cloud"));
    }

    #[test]
    fn the_command_line_takes_name_equals_value() {
        assert_eq!(
            parse_assignment("sky.cloud.body_low=0.45"),
            Ok(("sky.cloud.body_low".to_string(), 0.45))
        );
        assert!(parse_assignment("sky.cloud.body_low").is_err());
        assert!(parse_assignment("sky.cloud.body_low=lots").is_err());
    }
}
