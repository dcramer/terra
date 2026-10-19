//! Project tasks, and the one place every tool is started from. Run as `cargo xtask <task>`;
//! `cargo xtask tools` lists them all. Each module owning tasks lists its own in a `TASKS`
//! (D410), and `MODULES` below gathers them: a new tool gets a line in its module's, a new
//! module's table a line in `MODULES`. A tool kept elsewhere (TypeScript, a crate's program,
//! Blender) is started from a module in `tasks/` by its area; each has its own README.

mod baselines;
mod bench;
mod build;
mod check;
mod doctor;
mod load;
mod nightly;
mod primitives;
mod scenes;
mod scope;
mod seed;
mod sizes;
mod stage;
mod tasks;
mod windows;
mod worktrees;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

/// A task: its name, what it takes, what it does, the group it is listed under, and how
/// it runs, given the arguments after its name.
struct Task {
    name: &'static str,
    takes: &'static str,
    does: &'static str,
    group: &'static str,
    run: fn(&[String]) -> bool,
}

/// Every module's tasks. Within a group `tools` lists them in this order, each module's in
/// its own.
const MODULES: &[&[Task]] = &[
    doctor::TASKS,
    windows::TASKS,
    tasks::play::TASKS,
    bench::TASKS,
    check::TASKS,
    sizes::TASKS,
    build::TASKS,
    nightly::TASKS,
    tasks::world::TASKS,
    tasks::sandbox::TASKS,
    tasks::art::TASKS,
    seed::TASKS,
    worktrees::TASKS,
    tasks::machine::TASKS,
    tasks::docs::TASKS,
    TASKS,
];

/// The groups `tools` lists, in order; every task names one.
const GROUPS: &[&str] = &[
    "Play and check",
    "Look at things",
    "Make art",
    "This machine",
];

/// The tasks this file runs itself.
const TASKS: &[Task] = &[Task {
    name: "tools",
    takes: "",
    does: "this list",
    group: "This machine",
    run: |_| {
        println!("{}", usage());
        true
    },
}];

/// Every task, in the order `tools` lists them.
fn all_tasks() -> impl Iterator<Item = &'static Task> {
    MODULES.iter().flat_map(|tasks| tasks.iter())
}

/// Every task, by group, for `tools` and for a task that is not known.
fn usage() -> String {
    let mut text = String::from("usage: cargo xtask <task> [args]\n");
    let width = all_tasks()
        .map(|task| task.name.len() + task.takes.len() + 1)
        .max()
        .unwrap_or(0)
        .min(34);
    for group in GROUPS {
        text.push_str(&format!("\n{group}\n"));
        for task in all_tasks().filter(|task| task.group == *group) {
            let call = format!("{} {}", task.name, task.takes);
            let call = call.trim_end();
            if call.len() > width {
                text.push_str(&format!("  {call}\n  {:width$}  {}\n", "", task.does));
            } else {
                text.push_str(&format!("  {call:width$}  {}\n", task.does));
            }
        }
    }
    text
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_default();
    let rest: Vec<String> = args.collect();
    use_hooks();
    let ok = match all_tasks()
        .find(|task| task.name == name || (name == "help" && task.name == "tools"))
    {
        Some(task) => (task.run)(&rest),
        None => {
            eprintln!("{}", usage());
            false
        }
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Points git at the repository's hooks, so `cargo fmt` checks every commit and `cargo xtask check`
/// (formatting and clippy) every push (D37), and at the merge driver of the rendered
/// indexes (D409, `.gitattributes`).
/// Any task does it, so a new clone has it as soon as anyone runs one; worktrees share it.
fn use_hooks() {
    if output("git", &["config", "--get", "merge.rendered.driver"]).as_deref() != Some(MERGE_DRIVER)
    {
        let _ = output(
            "git",
            &[
                "config",
                "merge.rendered.name",
                "rendered index, row by row (D409)",
            ],
        );
        let _ = output("git", &["config", "merge.rendered.driver", MERGE_DRIVER]);
    }
    if output("git", &["config", "--get", "core.hooksPath"]).as_deref() != Some(".githooks") {
        let set = output("git", &["config", "core.hooksPath", ".githooks"]).is_some();
        if set {
            eprintln!(
                "git now runs the formatter before each commit and the formatter and clippy before each push (.githooks)"
            );
        }
    }
}

/// How git merges a page whose index is rendered from files of their own (D409).
pub(crate) const MERGE_DRIVER: &str = "node tools/docs/merge.ts %O %A %B %P";

/// A TypeScript tool, run by Node from the root.
fn node(script: &str, args: &[String]) -> bool {
    let mut all = vec![script.to_string()];
    all.extend(args.iter().cloned());
    run_owned("node", &all)
}

fn run_owned(program: &str, args: &[String]) -> bool {
    run(
        program,
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run(program: &str, args: &[&str]) -> bool {
    println!("\n> {program} {}", args.join(" "));
    Command::new(program)
        .args(args)
        .current_dir(root())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(root())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_task_has_its_own_name() {
        // The first task of a name is the one that runs, so a second would never run.
        for task in all_tasks() {
            assert_eq!(
                all_tasks().filter(|other| other.name == task.name).count(),
                1,
                "{} twice",
                task.name
            );
        }
    }

    #[test]
    fn every_task_is_listed_under_a_group() {
        // `tools` lists a task under its group, so one naming a group not in GROUPS is never shown.
        for task in all_tasks() {
            assert!(
                GROUPS.contains(&task.group),
                "{} in {}",
                task.name,
                task.group
            );
        }
    }
}
