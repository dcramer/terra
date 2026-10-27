//! Tunables in a run (D547, `terra_gen::tune`): those `--tune NAME=VALUE` gives are set before
//! anything starts, and the remote lists, reads and sets them while the run goes, so a value is
//! tried without a rebuild (`node tools/sandbox/remote.ts tune …`):
//!
//! - `tune.list {prefix?}`: every tunable whose name starts with `prefix`, by name.
//! - `tune.get {name}`: one, with its default, range, help and what last set it.
//! - `tune.set {name, value}`: sets it within its range (`null` for its default) and says what
//!   it was; the run logs `Tune: NAME was -> now`, so a script can `wait` for it.
//! - `tune.reset {prefix?}`: every changed one starting with `prefix` back to its default.

use crate::shell::Options;
use bevy::{
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
};
use serde::Deserialize;
use serde_json::{Value, json};
use terra_gen::tune::{self, SetBy, Tunable};

pub fn plugin(app: &mut App) {
    app.add_systems(PreStartup, from_the_command_line);
}

/// The remote's methods, added with the remote's own.
pub fn remote(app: &mut App) {
    crate::remote::method(app, "tune.list", list);
    crate::remote::method(app, "tune.get", get);
    crate::remote::method(app, "tune.set", set);
    crate::remote::method(app, "tune.reset", reset);
}

/// `--tune`, once every plugin has registered its own; a name or value it cannot take ends the
/// run, since an A/B run on a mistyped name would measure nothing.
fn from_the_command_line(options: Res<Options>, mut exit: MessageWriter<AppExit>) {
    for (name, value) in &options.tunes {
        let done = tune::named(name).and_then(|t| t.set(*value, SetBy::CommandLine).map(|_| t));
        match done {
            Ok(t) => info!("Tune: {} = {} (default {})", t.name, t.get(), t.default),
            Err(why) => {
                error!("Tune: --tune {name}={value}: {why}");
                exit.write(AppExit::error());
            }
        }
    }
}

fn answer(t: &'static Tunable) -> Value {
    json!({
        "name": t.name,
        "value": t.get(),
        "default": t.default,
        "min": t.min,
        "max": t.max,
        "doc": t.doc,
        "set_by": t.set_by().name(),
    })
}

#[derive(Deserialize, Default)]
struct Prefix {
    prefix: Option<String>,
}

fn starting(prefix: &Option<String>) -> impl Iterator<Item = &'static Tunable> + '_ {
    tune::all().into_iter().filter(move |t| {
        prefix
            .as_ref()
            .is_none_or(|p| t.name.starts_with(p.as_str()))
    })
}

fn list(In(params): In<Option<Value>>) -> BrpResult {
    let Prefix { prefix } = parse_some(params.or(Some(json!({}))))?;
    Ok(Value::Array(starting(&prefix).map(answer).collect()))
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

fn get(In(params): In<Option<Value>>) -> BrpResult {
    let Named { name } = parse_some(params)?;
    tune::named(&name)
        .map(answer)
        .map_err(crate::remote::refused)
}

#[derive(Deserialize)]
struct Set {
    name: String,
    value: Option<f32>,
}

/// Sets one, or puts it back to its default with `None`, and logs it (`Tune: NAME was -> now`)
/// for a script's `wait`: what the remote and the debug menu (D81) both set through, so each
/// sees what the other set. Answers what it was.
pub fn set_logged(t: &'static Tunable, value: Option<f32>, by: SetBy) -> Result<f32, String> {
    let was = match value {
        Some(value) => t.set(value, by)?,
        None => t.reset(),
    };
    info!(
        "Tune: {} {was} -> {} (default {}, by {})",
        t.name,
        t.get(),
        t.default,
        t.set_by().name()
    );
    Ok(was)
}

/// Every changed one whose name starts with `prefix` back to its default, logged; their names.
pub fn reset_logged(prefix: &Option<String>) -> Vec<&'static str> {
    let mut reset = Vec::new();
    for t in starting(prefix).filter(|t| t.get() != t.default) {
        let was = t.reset();
        info!("Tune: {} {was} -> {} (default)", t.name, t.get());
        reset.push(t.name);
    }
    reset
}

fn set(In(params): In<Option<Value>>) -> BrpResult {
    let Set { name, value } = parse_some(params)?;
    let t = tune::named(&name).map_err(crate::remote::refused)?;
    let was = set_logged(t, value, SetBy::Remote).map_err(crate::remote::refused)?;
    Ok(json!({ "was": was, "tunable": answer(t) }))
}

fn reset(In(params): In<Option<Value>>) -> BrpResult {
    let Prefix { prefix } = parse_some(params.or(Some(json!({}))))?;
    Ok(json!({ "reset": reset_logged(&prefix) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // The tests' own tunables: the list is the process's, shared with tests on other threads.
    static SHADE: Tunable = Tunable::new("test.world.shade", 1.0, 0.0..=2.0, "for the tests");
    static ARGS: Tunable = Tunable::new("test.world.args", 3.0, 0.0..=10.0, "for the tests");

    fn run_with(args: &[&str]) -> App {
        let options = Options::from_args(args.iter().map(|a| a.to_string())).unwrap();
        let mut app = App::new();
        app.insert_resource(options).add_plugins(plugin);
        app.update();
        app
    }

    fn exited(app: &App) -> bool {
        !app.world().resource::<Messages<AppExit>>().is_empty()
    }

    #[test]
    fn the_command_line_sets_a_tunable_before_the_run_starts() {
        tune::register(&[&ARGS]);
        let app = run_with(&["--tune", "test.world.args=7.5"]);
        assert_eq!(ARGS.get(), 7.5);
        assert_eq!(ARGS.set_by(), SetBy::CommandLine);
        assert!(!exited(&app));
    }

    #[test]
    fn a_mistyped_or_out_of_range_tune_ends_the_run() {
        tune::register(&[&ARGS]);
        assert!(exited(&run_with(&["--tune", "test.world.argz=1"])));
        assert!(exited(&run_with(&["--tune", "test.world.args=11"])));
        assert!(
            Options::from_args(["--tune", "test.world.args"].map(String::from).into_iter())
                .is_err()
        );
    }

    #[test]
    fn the_remote_sets_reads_and_resets_a_tunable() {
        tune::register(&[&SHADE]);
        let answer = set(In(Some(json!({"name": "test.world.shade", "value": 0.25})))).unwrap();
        assert_eq!(answer["was"], 1.0);
        assert_eq!(answer["tunable"]["value"], 0.25);
        assert_eq!(answer["tunable"]["set_by"], "remote");
        let got = get(In(Some(json!({"name": "test.world.shade"})))).unwrap();
        assert_eq!(got["value"], 0.25);
        let listed = list(In(Some(json!({"prefix": "test.world.sh"})))).unwrap();
        assert_eq!(listed.as_array().unwrap().len(), 1);
        assert!(set(In(Some(json!({"name": "test.world.shade", "value": 3.0})))).is_err());
        let reset = reset(In(Some(json!({"prefix": "test.world.shade"})))).unwrap();
        assert_eq!(reset["reset"], json!(["test.world.shade"]));
        assert_eq!(SHADE.get(), 1.0);
        let refused = get(In(Some(json!({"name": "test.world.shad"})))).unwrap_err();
        assert!(
            refused.message.contains("test.world.shade"),
            "{}",
            refused.message
        );
    }
}
