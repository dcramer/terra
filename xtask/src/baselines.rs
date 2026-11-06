//! The ratchets' baselines (D414): a count is raised by a change with its why, and lowered
//! only by the full run on main (`cargo xtask nightly`), which needs to know which files they
//! are; and the push hook, which refuses a change that raises one by hand.

use std::collections::BTreeMap;

/// The artifacts ratchet's baseline, a file a set and group: `<set>/<group>.tsv`, rows
/// group, fault, count, why.
pub(crate) const ARTIFACTS: &str = "crates/terra_build_scenes/examples/artifacts-baseline";

/// The size ratchet's baseline (D396): path, function, lines.
pub(crate) const SIZES: &str = "xtask/sizes-baseline.tsv";

/// The combat scorecard's baseline (D452): id, value, why.
pub(crate) const COMBAT: &str = "tools/combat/baseline.tsv";

/// The controller scorecard's baseline (D457): id, value, why.
pub(crate) const CONTROLLER: &str = "tools/controller/baseline.tsv";

/// The animation scorecard's baseline (D631): id, value, why.
pub(crate) const ANIM: &str = "tools/anim/baseline.tsv";

/// Whether a file, from the root, is a ratchet's baseline: a file of the artifacts folder, the
/// size baseline or a scorecard's baseline.
pub(crate) fn is_baseline(file: &str) -> bool {
    file == SIZES
        || file == COMBAT
        || file == CONTROLLER
        || file == ANIM
        || file
            .strip_prefix(ARTIFACTS)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The push hook's baselines stage (D414): passes when the change since `base` only lowers
/// or removes baseline rows, and says each row raised or added otherwise.
pub(crate) fn only_lowered(base: &str, out: &mut String) -> bool {
    match refused_since(base) {
        Ok(refused) if refused.is_empty() => {
            out.push_str("no baseline raised or added\n");
            true
        }
        Ok(refused) => {
            for line in refused {
                out.push_str(&line);
                out.push('\n');
            }
            false
        }
        Err(why) => {
            out.push_str(&why);
            out.push('\n');
            false
        }
    }
}

/// What a push does to the baselines against `base`, a line for each change no hand may make
/// (D414): a size row raised or added (D396: it never rises and nothing is added), or a
/// scorecard's or the artifacts' count raised or added without a why of its own. A row
/// lowered or removed passes. Empty when everything passes.
fn refused_since(base: &str) -> Result<Vec<String>, String> {
    let changed = crate::output("git", &["diff", "--name-only", "--no-renames", base])
        .ok_or_else(|| format!("git cannot say what changed since {base}"))?;
    let mut refused = Vec::new();
    for file in changed.lines().filter(|file| is_baseline(file)) {
        let old = crate::output("git", &["show", &format!("{base}:{file}")]).unwrap_or_default();
        let new = std::fs::read_to_string(crate::root().join(file)).unwrap_or_default();
        refused.extend(refused_rows(file, &old, &new));
    }
    Ok(refused)
}

/// The rows of `file` that `new` raises or adds against `old`, each a line saying what, the
/// rule and the fix (D556).
fn refused_rows(file: &str, old: &str, new: &str) -> Vec<String> {
    let (old, new) = (rows(file, old), rows(file, new));
    let mut refused = Vec::new();
    for (key, (value, why)) in &new {
        let held = old.get(key);
        if held.is_some_and(|(was, _)| value <= was) {
            continue;
        }
        let what = match held {
            Some((was, _)) => format!("{key} raised {was} -> {value}"),
            None => format!("{key} added at {value}"),
        };
        if file == SIZES {
            refused.push(format!(
                "{file}: {what} lines; a size row never rises and none is added, only nightly \
                 lowers one (D396, D414): undo it and split the file or function instead"
            ));
        } else if why.is_empty() || why == "-" || held.is_some_and(|(_, was)| was == why) {
            refused.push(format!(
                "{file}: {what} by hand, with no why of its own (D414): undo it and run `{}`",
                raise_command(file)
            ));
        }
    }
    refused
}

/// The command that raises a baseline's rows with a why (D414, D452, D457, D631).
fn raise_command(file: &str) -> &'static str {
    if file == COMBAT {
        "cargo xtask combat --raise \"<why>\""
    } else if file == CONTROLLER {
        "cargo xtask controller --raise \"<why>\""
    } else if file == ANIM {
        "cargo xtask anim --raise \"<why>\""
    } else if file.starts_with(&format!("{ARTIFACTS}/fuzz/")) {
        "cargo xtask fuzz --raise \"<why>\""
    } else {
        "cargo xtask artifacts [--quick] --raise \"<why>\""
    }
}

/// A baseline's rows, key to number and why. The size baseline's are path, function and
/// lines, with no why; the others' end in a number and a why, the key the columns before. A
/// comment, the header or a row with no number is no row.
fn rows(file: &str, text: &str) -> BTreeMap<String, (f64, String)> {
    let mut rows = BTreeMap::new();
    for line in text.lines().filter(|line| !line.starts_with('#')) {
        let columns: Vec<&str> = line.split('\t').collect();
        let (key, value, why) = match columns[..] {
            [path, "-", lines] if file == SIZES => (path.to_string(), lines, ""),
            [path, function, lines] if file == SIZES => {
                (format!("{path} fn {function}"), lines, "")
            }
            [ref key @ .., value, why] if file != SIZES && !key.is_empty() => {
                (key.join(" "), value, why)
            }
            _ => continue,
        };
        if let Ok(value) = value.trim().parse::<f64>() {
            rows.insert(key, (value, why.trim().to_string()));
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZES_OLD: &str = "# path\tfunction (- for the file)\tlines\n\
        crates/terra_world/src/planet.rs\t-\t1129\n\
        crates/a/src/lib.rs\tbuild\t200\n";

    #[test]
    fn a_size_row_raised_or_added_is_refused() {
        let raised = SIZES_OLD.replace("1129", "1130");
        let refused = refused_rows(SIZES, SIZES_OLD, &raised);
        assert_eq!(refused.len(), 1, "{refused:?}");
        assert!(
            refused[0].contains("planet.rs raised 1129 -> 1130"),
            "{refused:?}"
        );
        assert!(refused[0].contains("D396") && refused[0].contains("split"));
        let added = format!("{SIZES_OLD}crates/b/src/lib.rs\t-\t1001\n");
        let refused = refused_rows(SIZES, SIZES_OLD, &added);
        assert_eq!(refused.len(), 1, "{refused:?}");
        assert!(refused[0].contains("crates/b/src/lib.rs added at 1001"));
        let grown = SIZES_OLD.replace("\t200", "\t201");
        let refused = refused_rows(SIZES, SIZES_OLD, &grown);
        assert!(refused[0].contains("crates/a/src/lib.rs fn build raised 200 -> 201"));
    }

    #[test]
    fn a_size_row_lowered_or_removed_passes() {
        let lowered = SIZES_OLD.replace("1129", "1100");
        assert!(refused_rows(SIZES, SIZES_OLD, &lowered).is_empty());
        let removed = SIZES_OLD.replace("crates/a/src/lib.rs\tbuild\t200\n", "");
        assert!(refused_rows(SIZES, SIZES_OLD, &removed).is_empty());
        assert!(refused_rows(SIZES, SIZES_OLD, "").is_empty());
    }

    #[test]
    fn a_scorecard_raise_needs_a_why_of_its_own() {
        let old = "id\tvalue\twhy\nchain.blows\t3\tseeded from main\n";
        let by_hand = old.replace("\t3\t", "\t4\t");
        assert_eq!(refused_rows(COMBAT, old, &by_hand).len(), 1);
        let with_why = "id\tvalue\twhy\nchain.blows\t4\tthe new parry costs a blow\n";
        assert!(refused_rows(COMBAT, old, with_why).is_empty());
        let added = format!("{old}chain.new\t1\t-\n");
        let refused = refused_rows(CONTROLLER, old, &added);
        assert_eq!(refused.len(), 1, "{refused:?}");
        assert!(refused[0].contains("cargo xtask controller --raise"));
        let refused = refused_rows(ANIM, old, &by_hand);
        assert_eq!(refused.len(), 1, "{refused:?}");
        assert!(refused[0].contains("cargo xtask anim --raise"));
        let lowered = old.replace("\t3\t", "\t2\t");
        assert!(refused_rows(COMBAT, old, &lowered).is_empty());
    }

    #[test]
    fn an_artifacts_count_raised_without_a_why_is_refused() {
        let file = format!("{ARTIFACTS}/fuzz/seed-2.tsv");
        let old = "burg\tBuried\t32\t-\n";
        let refused = refused_rows(&file, old, "burg\tBuried\t33\t-\n");
        assert_eq!(refused.len(), 1, "{refused:?}");
        assert!(refused[0].contains("burg Buried raised 32 -> 33"));
        assert!(refused[0].contains("cargo xtask fuzz --raise"));
        assert!(refused_rows(&file, old, "burg\tBuried\t33\troofs overlap now\n").is_empty());
    }
}
