//! The baselines stage of both checks (D414): a ratchet's baseline is lowered only by the
//! full run on main, never by a change, so a lowering tested against one main never
//! lands beside another change tested against the old, higher count and turns main red. Each
//! baseline in the working tree is compared with the base, HEAD before a commit and the
//! merge-base with origin/main before a push, a row at a time by its first two columns:
//!
//! - a count lowered or a row dropped fails, unless `TERRA_BASELINE_JOB=1` (the full run on main);
//! - in the artifacts baseline a count raised or a row added needs a why, its fourth column,
//!   not empty, not `-` and not the old row's (`cargo xtask artifacts --raise "<why>"`);
//! - in the size baseline any rise or new row fails: split instead (D396).

use crate::{output, root};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::process::{Command, Stdio};

/// The artifacts ratchet's baseline, a file a set and group: `<set>/<group>.tsv`, rows
/// group, fault, count, why.
pub(crate) const ARTIFACTS: &str = "crates/terra_build/examples/artifacts-baseline";

/// The single file it was before D414 (set, group, fault, count), read at a base older than
/// the folder so the move compares equal. Delete once no branch is based before it.
const LEGACY: &str = "crates/terra_build/examples/artifacts-baseline.tsv";

/// The size ratchet's baseline (D396): path, function, lines.
pub(crate) const SIZES: &str = "xtask/sizes-baseline.tsv";

/// Set to 1 by the full run on main (`cargo xtask nightly`), the one place a baseline is lowered.
pub(crate) const JOB: &str = "TERRA_BASELINE_JOB";

/// A row as read: its count, its why (empty where the baseline has none) and its file.
#[derive(Debug, Clone, PartialEq)]
struct Row {
    count: usize,
    why: String,
    file: String,
}

/// A baseline's rows by table ("sizes", or "artifacts <set>") and their first two columns.
type Rows = BTreeMap<(String, String, String), Row>;

/// Reads the rows of the baseline file at `path`, relative to the root, from its `text`.
fn parse(path: &str, text: &str, rows: &mut Rows) {
    let set = path
        .strip_prefix(ARTIFACTS)
        .and_then(|rest| rest.strip_prefix('/'))
        .and_then(|rest| rest.split('/').next());
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let cells: Vec<&str> = line.split('\t').collect();
        let row = match (path, set, &cells[..]) {
            (SIZES, _, [file, item, count]) => Some(("sizes".to_string(), file, item, count, &"")),
            (LEGACY, _, [set, group, fault, count]) => {
                Some((format!("artifacts {set}"), group, fault, count, &"-"))
            }
            (_, Some(set), [group, fault, count, why]) => {
                Some((format!("artifacts {set}"), group, fault, count, why))
            }
            _ => None,
        };
        if let Some((table, a, b, count, why)) = row
            && let Ok(count) = count.parse()
        {
            let row = Row {
                count,
                why: why.trim().to_string(),
                file: path.to_string(),
            };
            rows.insert((table, a.to_string(), b.to_string()), row);
        }
    }
}

/// What a change to the baselines breaks, a line each; none when it holds. `job` is the
/// scheduled run, which may lower.
fn judge(then: &Rows, now: &Rows, job: bool) -> Vec<String> {
    let mut faults = Vec::new();
    let what = |(table, a, b): &(String, String, String)| format!("{table}: {a} {b}");
    for (key, was) in then.iter().filter(|_| !job) {
        let is = now.get(key).map_or(0, |row| row.count);
        if is < was.count {
            faults.push(format!(
                "{}: {} lowered from {} to {is}; a fall passes the ratchet as it is, and lowering is the full run on main (`cargo xtask nightly`)'s (D414)",
                was.file,
                what(key),
                was.count
            ));
        }
    }
    for (key, row) in now {
        let was = then.get(key);
        let from = was.map_or(0, |was| was.count);
        if row.count <= from {
            continue;
        }
        let rise = format!("{} raised from {from} to {}", what(key), row.count);
        if key.0 == "sizes" {
            faults.push(format!(
                "{}: {rise}; the size baseline never grows: split it (D396)",
                row.file
            ));
            continue;
        }
        let said = !row.why.is_empty() && row.why != "-";
        if !said || was.is_some_and(|was| was.why == row.why) {
            faults.push(format!(
                "{}: {rise} with no new why: `cargo xtask artifacts --raise \"<why>\"` (or fuzz's) writes it",
                row.file
            ));
        }
    }
    faults
}

/// Every baseline's rows in the working tree.
fn in_tree() -> Rows {
    let mut rows = Rows::new();
    let read = |path: &str, rows: &mut Rows| {
        let text = std::fs::read_to_string(root().join(path)).unwrap_or_default();
        parse(path, &text, rows);
    };
    read(SIZES, &mut rows);
    let sets = std::fs::read_dir(root().join(ARTIFACTS))
        .into_iter()
        .flatten();
    for set in sets.flatten() {
        let files = std::fs::read_dir(set.path()).into_iter().flatten();
        for file in files.flatten() {
            let path = format!(
                "{ARTIFACTS}/{}/{}",
                set.file_name().to_string_lossy(),
                file.file_name().to_string_lossy()
            );
            read(&path, &mut rows);
        }
    }
    rows
}

/// Every baseline's rows at `base`, in one `git grep` of its every line, or `None` when git
/// cannot read it.
fn at(base: &str) -> Option<Rows> {
    let out = Command::new("git")
        .args([
            "grep",
            "--no-color",
            "-e",
            "",
            base,
            "--",
            ARTIFACTS,
            LEGACY,
            SIZES,
        ])
        .current_dir(root())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    // 1 is no line found: no baseline at the base.
    if !matches!(out.status.code(), Some(0 | 1)) {
        return None;
    }
    let mut by_file: BTreeMap<&str, String> = BTreeMap::new();
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let Some((path, line)) = line
            .strip_prefix(base)
            .and_then(|rest| rest.strip_prefix(':'))
            .and_then(|rest| rest.split_once(':'))
        else {
            continue;
        };
        let file = by_file.entry(path).or_default();
        file.push_str(line);
        file.push('\n');
    }
    let mut rows = Rows::new();
    for (path, text) in &by_file {
        parse(path, text, &mut rows);
    }
    Some(rows)
}

/// Whether a file, from the root, is a ratchet's baseline: a file of the artifacts folder or
/// the size baseline.
pub(crate) fn is_baseline(file: &str) -> bool {
    file == SIZES
        || file
            .strip_prefix(ARTIFACTS)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The stage before a commit: the baselines against HEAD.
pub(crate) fn hold_before_commit(out: &mut String) -> bool {
    hold(Some("HEAD".to_string()), out)
}

/// The stage before a push: the baselines against where HEAD left origin/main.
pub(crate) fn hold_before_push(out: &mut String) -> bool {
    hold(output("git", &["merge-base", "HEAD", "origin/main"]), out)
}

fn hold(base: Option<String>, out: &mut String) -> bool {
    let Some(base) = base else {
        let _ = writeln!(
            out,
            "no base to compare the baselines with (no origin/main?)"
        );
        return false;
    };
    let Some(then) = at(&base) else {
        let _ = writeln!(out, "git cannot read the baselines at {base}");
        return false;
    };
    let job = std::env::var(JOB).is_ok_and(|value| value == "1");
    let faults = judge(&then, &in_tree(), job);
    for fault in &faults {
        let _ = writeln!(out, "{fault}");
    }
    if !faults.is_empty() {
        let _ = writeln!(
            out,
            "A baseline file is put back as it was with `git checkout {base} -- <file>` (D414)."
        );
    }
    faults.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHEET: &str = "crates/terra_build/examples/artifacts-baseline/quick/sheet.tsv";

    fn rows(files: &[(&str, &str)]) -> Rows {
        let mut rows = Rows::new();
        for (path, text) in files {
            parse(path, text, &mut rows);
        }
        rows
    }

    fn sheet(text: &str) -> Rows {
        rows(&[(SHEET, text)])
    }

    #[test]
    fn an_unchanged_baseline_holds_and_a_fall_left_in_place_holds() {
        let base = sheet("sheet\tCoplanar\t39\t-\nsheet\tPokesOut\t18\t-\n");
        assert!(judge(&base, &base.clone(), false).is_empty());
    }

    #[test]
    fn a_lowered_or_dropped_count_fails_but_for_the_scheduled_run() {
        let base = sheet("sheet\tCoplanar\t39\t-\nsheet\tPokesOut\t18\t-\n");
        let lowered = sheet("sheet\tCoplanar\t30\t-\nsheet\tPokesOut\t18\t-\n");
        let dropped = sheet("sheet\tCoplanar\t39\t-\n");
        assert_eq!(judge(&base, &lowered, false).len(), 1);
        assert_eq!(judge(&base, &dropped, false).len(), 1);
        assert!(judge(&base, &lowered, true).is_empty());
        assert!(judge(&base, &dropped, true).is_empty());
    }

    #[test]
    fn a_raised_or_new_artifacts_count_needs_a_new_why() {
        let base = sheet("sheet\tCoplanar\t39\tthe gable's end\n");
        let raised = |why: &str| sheet(&format!("sheet\tCoplanar\t41\t{why}\n"));
        assert!(judge(&base, &raised("the new eave meets the post"), false).is_empty());
        assert_eq!(judge(&base, &raised("the gable's end"), false).len(), 1);
        assert_eq!(judge(&base, &raised("-"), false).len(), 1);
        assert_eq!(judge(&base, &raised(" "), false).len(), 1);
        let added = |why: &str| {
            sheet(&format!(
                "sheet\tCoplanar\t39\tthe gable's end\nsheet\tBuried\t2\t{why}\n"
            ))
        };
        assert!(judge(&base, &added("a post sunk in the deck"), false).is_empty());
        assert_eq!(judge(&base, &added("-"), false).len(), 1);
        // The scheduled run lowers; it never raises without a why either.
        assert_eq!(judge(&base, &added("-"), true).len(), 1);
    }

    #[test]
    fn the_size_baseline_never_grows_or_takes_a_row() {
        let base = rows(&[(SIZES, "# path\tfunction\tlines\na.rs\tfuzz\t152\n")]);
        let grew = rows(&[(SIZES, "a.rs\tfuzz\t160\n")]);
        let added = rows(&[(SIZES, "a.rs\tfuzz\t152\nb.rs\t-\t1200\n")]);
        let shrank = rows(&[(SIZES, "a.rs\tfuzz\t151\n")]);
        assert_eq!(judge(&base, &grew, true).len(), 1);
        assert_eq!(judge(&base, &added, true).len(), 1);
        assert_eq!(judge(&base, &shrank, false).len(), 1);
        assert!(judge(&base, &shrank, true).is_empty());
    }

    #[test]
    fn the_single_file_before_the_folder_reads_as_the_same_rows() {
        let legacy = rows(&[(
            LEGACY,
            "# set\tkits\tfault\tcount\nquick\tsheet\tCoplanar\t39\n",
        )]);
        let folder = sheet("sheet\tCoplanar\t39\t-\n");
        assert_eq!(
            legacy.keys().collect::<Vec<_>>(),
            folder.keys().collect::<Vec<_>>()
        );
        assert!(judge(&legacy, &folder, false).is_empty());
        // A set is its folder: the same group in another set is another row.
        let all = rows(&[(
            "crates/terra_build/examples/artifacts-baseline/all/sheet.tsv",
            "sheet\tCoplanar\t39\t-\n",
        )]);
        assert_eq!(judge(&folder, &all, false).len(), 2);
    }
}
