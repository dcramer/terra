//! The designs and decisions (docs/, tools/docs).

use crate::{Task, node};

pub(crate) const TASKS: &[Task] = &[Task {
    name: "decision",
    takes: "[D28 D198 …]",
    does: "the next free decision number, here and on origin/main after a fetch; given numbers, those decisions in full with their log lines (docs/decisions/, D389)",
    group: "This machine",
    run: |args| {
        let mut all = vec![if args.is_empty() { "--next" } else { "--show" }.to_string()];
        all.extend(args.iter().cloned());
        node("tools/docs/docs.ts", &all)
    },
}];
