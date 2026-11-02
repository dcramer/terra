//! A machine whose builds all run on the render worker (`TERRA_NO_LOCAL_BUILDS=1`, D183,
//! D445): every task that builds or runs the game here refuses there, pointing at
//! `cargo xtask remote-render`, so an agent that skips the routine's "not when" cannot spend
//! the machine's CPU on a build of minutes. The tasks in `LIGHT` build nothing of ours, or are
//! the developer's own (`play`), and run anywhere.

/// The variable that marks a machine that builds nothing itself.
pub(crate) const VAR: &str = "TERRA_NO_LOCAL_BUILDS";

/// The tasks that run on a machine that builds nothing: the list, the machine's health, the
/// decisions, the render worker's client, the worktree cleanup, the size lint (a scan of the
/// sources), Tripo's web client, and the game the developer launches from their own terminal.
const LIGHT: &[&str] = &[
    "tools",
    "help",
    "doctor",
    "decision",
    "remote-render",
    "worktrees",
    "sizes",
    "tripo",
    "play",
    "yard",
];

/// Whether this machine builds and runs the game itself.
pub(crate) fn builds_here() -> bool {
    builds(std::env::var(VAR).ok().as_deref())
}

fn builds(var: Option<&str>) -> bool {
    var != Some("1")
}

/// Why a task may not run here, as an error says it (D556), or None when it may.
pub(crate) fn refusal(task: &str) -> Option<String> {
    refusal_given(task, builds_here())
}

fn refusal_given(task: &str, builds_here: bool) -> Option<String> {
    if builds_here || LIGHT.contains(&task) {
        return None;
    }
    let push = if task == "check" {
        "\n  Before a push, the hook runs this check on the worker when TERRA_REMOTE_CHECKS=1 is in .env (tools/git/remote-check.ts); set it rather than unsetting the variable."
    } else {
        ""
    };
    Some(format!(
        "✗ cargo xtask {task}: this machine builds nothing itself ({VAR}=1, D183, D445), and `{task}` builds or runs the game here.\n  \
         Send it to the render worker: cargo xtask remote-render -- cargo xtask {task} …\n  \
         (tests too: cargo xtask remote-render -- cargo test -p <crate>). Do not unset {VAR} to get past this.{push}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_mark_stops_a_build() {
        assert!(builds(None));
        assert!(builds(Some("0")));
        assert!(builds(Some("")));
        assert!(!builds(Some("1")));
    }

    #[test]
    fn a_machine_that_builds_nothing_refuses_the_heavy_tasks_only() {
        for heavy in [
            "seed",
            "sandbox",
            "bench",
            "check",
            "gpu",
            "workbench",
            "combat",
            "win",
        ] {
            let said = refusal_given(heavy, false).unwrap_or_default();
            assert!(said.contains("remote-render"), "{heavy}: {said}");
        }
        for light in [
            "remote-render",
            "play",
            "decision",
            "doctor",
            "worktrees",
            "tools",
        ] {
            assert_eq!(refusal_given(light, false), None, "{light}");
        }
        assert_eq!(refusal_given("seed", true), None);
    }

    #[test]
    fn every_light_task_exists() {
        // A light task renamed would otherwise be refused silently on such a machine.
        for light in LIGHT.iter().filter(|name| **name != "help") {
            assert!(
                crate::all_tasks().any(|task| task.name == *light),
                "{light} is in LIGHT but no task has that name"
            );
        }
    }
}
