//! The ratchets' baselines (D414): a count is raised by a change with its why, and lowered
//! only by the full run on main (`cargo xtask nightly`), which needs to know which files they
//! are.

/// The artifacts ratchet's baseline, a file a set and group: `<set>/<group>.tsv`, rows
/// group, fault, count, why.
pub(crate) const ARTIFACTS: &str = "crates/terra_build_scenes/examples/artifacts-baseline";

/// The size ratchet's baseline (D396): path, function, lines.
pub(crate) const SIZES: &str = "xtask/sizes-baseline.tsv";

/// Whether a file, from the root, is a ratchet's baseline: a file of the artifacts folder or
/// the size baseline.
pub(crate) fn is_baseline(file: &str) -> bool {
    file == SIZES
        || file
            .strip_prefix(ARTIFACTS)
            .is_some_and(|rest| rest.starts_with('/'))
}
