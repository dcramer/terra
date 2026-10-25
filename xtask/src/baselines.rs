//! The ratchets' baselines (D414): a count is raised by a change with its why, and lowered
//! only by the full run on main (`cargo xtask nightly`), which needs to know which files they
//! are.

/// The artifacts ratchet's baseline, a file a set and group: `<set>/<group>.tsv`, rows
/// group, fault, count, why.
pub(crate) const ARTIFACTS: &str = "crates/terra_build_scenes/examples/artifacts-baseline";

/// The size ratchet's baseline (D396): path, function, lines.
pub(crate) const SIZES: &str = "xtask/sizes-baseline.tsv";

/// The combat scorecard's baseline (D452): id, value, why.
pub(crate) const COMBAT: &str = "tools/combat/baseline.tsv";

/// The controller scorecard's baseline (D457): id, value, why.
pub(crate) const CONTROLLER: &str = "tools/controller/baseline.tsv";

/// Whether a file, from the root, is a ratchet's baseline: a file of the artifacts folder, the
/// size baseline or a scorecard's baseline.
pub(crate) fn is_baseline(file: &str) -> bool {
    file == SIZES
        || file == COMBAT
        || file == CONTROLLER
        || file
            .strip_prefix(ARTIFACTS)
            .is_some_and(|rest| rest.starts_with('/'))
}
