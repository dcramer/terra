//! What a change touches (D398): the crates the hook before a push formats and lints, and what
//! `cargo xtask nightly` and the continuous checks look at. The change is everything that
//! differs from where HEAD left origin/main (`git merge-base`), the working
//! tree's changes included. Each file is placed: a crate's own folder is that crate, files outside
//! the crates (`assets/` among them) are read by the crates in [`READ_BY`],
//! `tools/`, the docs and the agents' folders by their own checks, a ratchet's baseline by the
//! nightly (D411). A file the rules cannot
//! place, or a change to the build itself (the workspace's manifest, its lock, the compiler,
//! cargo's or clippy's settings), runs everything, as `--all` does.

use crate::{output, root};
use std::collections::BTreeSet;
use std::process::Command;

/// Files outside a crate's folder that a crate's code or tests read, by path prefix: a change
/// there tests the crate, and before a push every crate depending on it. A test
/// (`every_file_a_crate_reads_is_in_read_by`) finds each such path a crate names, under
/// `assets/` too, and fails when it is not here. A file may match several prefixes.
pub(crate) const READ_BY: &[(&str, &[&str])] = &[
    // The demos' places, `include_str!`ed by the site queries.
    ("tools/sandbox/places.ts", &["terra_land"]),
    // The voxel contract, proved row by row by building's contract test.
    ("docs/building-voxels.json", &["terra_build_scenes"]),
    // The kits' models and references, which the kit checker and the material bench read.
    ("art/", &["terra_build", "terra_build_mesh"]),
    // The game's files (D133): the crates on Bevy load any of them through its asset server.
    (
        "assets/",
        &[
            "terra_world",
            "terra_bases",
            "terra_walker",
            "terra_survival",
            "terra_creatures",
            "terra_game",
            "terra_sandbox",
        ],
    ),
    // The texture bake (D188): every model's and material's pictures and the fire's
    // flipbooks, and the builds, for the materials they name.
    ("assets/models/", &["terra_bake"]),
    ("assets/materials/", &["terra_bake"]),
    ("assets/textures/fire/", &["terra_bake"]),
    // Building's kits, packs, items and settlement, and the materials its kits name, which the
    // kit checker and the packs' tests read.
    (
        "assets/data/kits/",
        &[
            "terra_build",
            "terra_build_layout",
            "terra_build_mesh",
            "terra_build_scenes",
        ],
    ),
    ("assets/data/packs/", &["terra_build", "terra_build_mesh"]),
    (
        "assets/data/items.ron",
        &["terra_build", "terra_build_mesh"],
    ),
    (
        "assets/data/settlement.ron",
        &["terra_town", "terra_build_mesh", "terra_build_scenes"],
    ),
    (
        "assets/materials/",
        &[
            "terra_build",
            "terra_build_layout",
            "terra_build_mesh",
            "terra_build_scenes",
        ],
    ),
    // The town's market and the catalogue its stalls are sized from, and its people's day.
    ("assets/data/market.ron", &["terra_town"]),
    ("assets/data/builds.ron", &["terra_town", "terra_bake"]),
    ("assets/data/life.ron", &["terra_town"]),
    // The climate the land's places are found in.
    ("assets/data/climate.ron", &["terra_land"]),
];

/// Files whose change can alter how everything builds or what the gate is: these run
/// everything. The check's own code is the xtask crate, tested like any other.
const EVERYTHING: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "clippy.toml",
    "rustfmt.toml",
    ".cargo/",
];

/// Folders and files only the docs check reads, or nothing in the gate: their change runs the
/// stages that are always cheap and nothing else.
const DOCS: &[&str] = &[
    "docs/",
    "research/",
    ".agents/",
    ".claude/",
    ".github/",
    // The hooks only start the check; nothing reads them.
    ".githooks/",
    "spike/",
    "AGENTS.md",
    "CLAUDE.md",
    "README.md",
    "THIRD_PARTY.md",
    "LICENSE",
    "Makefile",
    ".gitignore",
    ".gitattributes",
    ".editorconfig",
    ".env.example",
    "crates/README.md",
];

/// What the fire sheet reads (docs/fire.md, "The automatic check"; D415): fire's code and
/// shaders, its textures and presets, the building fire, the tonemapping patches it is drawn
/// through, and the check's own tools. A change here runs the sheet before a push.
pub(crate) const FIRE: &[&str] = &[
    "crates/terra_world/src/fire.rs",
    "crates/terra_world/src/fire/",
    "crates/terra_world/src/smoke.rs",
    "crates/terra_world/src/grading.rs",
    "crates/terra_world/src/dither.rs",
    "crates/terra_world/src/scene.rs",
    "crates/terra_game/src/build/fire.rs",
    "assets/shaders/fire.wesl",
    "assets/shaders/fire_",
    "assets/textures/fire/",
    "assets/data/fire.ron",
    "tools/fire/",
];

/// What a change touches.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Scope {
    /// Why everything runs, when it does: asked for, or a change the rules cannot place.
    pub(crate) everything: Option<String>,
    /// How many files the change holds.
    pub(crate) files: usize,
    /// The workspace's packages (crates and xtask) whose own folders changed.
    pub(crate) changed: BTreeSet<String>,
    /// Packages that read a changed file outside their folder ([`READ_BY`]).
    pub(crate) readers: BTreeSet<String>,
    /// A Rust source changed somewhere.
    pub(crate) rust: bool,
    /// A crate's manifest changed.
    pub(crate) manifests: bool,
    /// Something under `assets/` or `art/` changed.
    pub(crate) assets: bool,
    /// The folders under `tools/` that changed.
    pub(crate) tools: BTreeSet<String>,
    /// A Python script changed (the Blender scripts' check).
    pub(crate) python: bool,
    /// Something the fire sheet reads changed ([`FIRE`]).
    pub(crate) fire: bool,
    /// A ratchet's baseline changed (`baselines::is_baseline`), which only the baselines
    /// stage reads.
    pub(crate) baselines: bool,
}

impl Scope {
    /// Everything, for the reason given.
    pub(crate) fn everything(why: &str) -> Self {
        Self {
            everything: Some(why.to_string()),
            ..Self::default()
        }
    }

    pub(crate) fn all(&self) -> bool {
        self.everything.is_some()
    }

    /// One line saying what the change is, for the check's first line.
    pub(crate) fn summary(&self) -> String {
        if let Some(why) = &self.everything {
            return format!("everything ({why})");
        }
        let mut parts = Vec::new();
        if !self.changed.is_empty() {
            parts.push(join(&self.changed));
        }
        if !self.readers.is_empty() && self.readers != self.changed {
            parts.push(format!("read by {}", join(&self.readers)));
        }
        if self.assets {
            parts.push("assets".into());
        }
        if !self.tools.is_empty() {
            parts.push(format!(
                "tools/{{{}}}",
                self.tools.iter().cloned().collect::<Vec<_>>().join(",")
            ));
        }
        if self.fire {
            parts.push("fire".into());
        }
        if self.baselines {
            parts.push("baselines".into());
        }
        if parts.is_empty() {
            parts.push("docs only".into());
        }
        format!("{} files: {}", self.files, parts.join("; "))
    }
}

fn join(names: &BTreeSet<String>) -> String {
    names.iter().cloned().collect::<Vec<_>>().join(", ")
}

/// Places each changed file. `packages` are the workspace's crates by folder name (each
/// crate's name is its folder's).
pub(crate) fn classify(files: &[String], packages: &BTreeSet<String>) -> Scope {
    let mut scope = Scope {
        files: files.len(),
        ..Scope::default()
    };
    for file in files {
        if EVERYTHING
            .iter()
            .any(|p| file == p || (p.ends_with('/') && file.starts_with(p)))
        {
            return Scope::everything(&format!("{file} changed"));
        }
        // A ratchet's baseline is its own class (D411), not a change to the crate or the
        // tasks holding it: a raise
        // only loosens a ratchet and only the full run on main (`cargo xtask nightly`) lowers one, having measured
        // it.
        if crate::baselines::is_baseline(file) {
            scope.baselines = true;
            continue;
        }
        scope.rust |= file.ends_with(".rs");
        scope.fire |= FIRE.iter().any(|p| file.starts_with(p));
        for (prefix, readers) in READ_BY {
            if file.starts_with(prefix) {
                scope.readers.extend(readers.iter().map(|r| r.to_string()));
            }
        }
        if DOCS
            .iter()
            .any(|p| file == p || (p.ends_with('/') && file.starts_with(p)))
        {
            continue;
        }
        let mut parts = file.split('/');
        match (parts.next(), parts.next(), parts.next()) {
            (Some("crates"), Some(name), Some(rest)) if packages.contains(name) => {
                scope.changed.insert(name.to_string());
                scope.manifests |= rest == "Cargo.toml";
            }
            (Some("xtask"), Some(_), _) => {
                scope.changed.insert("xtask".into());
            }
            (Some("assets" | "art"), Some(_), _) => scope.assets = true,
            (Some("tools"), Some(dir), Some(_)) => {
                scope.tools.insert(dir.to_string());
                scope.python |= file.ends_with(".py");
            }
            // The tools' own index.
            (Some("tools"), Some(name), None) if name.ends_with(".md") => {}
            _ if file.ends_with(".md") && !file.contains('/') => {}
            _ => return Scope::everything(&format!("{file} is not placed by the scoping")),
        }
    }
    scope
}

/// The workspace's crates, by folder.
pub(crate) fn packages() -> BTreeSet<String> {
    std::fs::read_dir(root().join("crates"))
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().join("Cargo.toml").exists())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn lines(text: Option<String>) -> Option<Vec<String>> {
    text.map(|t| {
        t.lines()
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect()
    })
}

/// What a push holds: everything that differs from where HEAD left origin/main, committed or
/// not.
pub(crate) fn push() -> Scope {
    let Some(base) = output("git", &["merge-base", "HEAD", "origin/main"]) else {
        return Scope::everything("no merge-base with origin/main");
    };
    match lines(output(
        "git",
        &["diff", "--name-only", "--no-renames", &base],
    )) {
        Some(files) => classify(&files, &packages()),
        None => Scope::everything("git cannot say what changed"),
    }
}

/// The packages given and every workspace package depending on them, through normal, build
/// and dev dependencies on any target, as cargo resolves them (`cargo tree --invert`). `None`
/// when cargo gives no answer.
pub(crate) fn with_dependents(names: &BTreeSet<String>) -> Option<BTreeSet<String>> {
    if names.is_empty() {
        return Some(BTreeSet::new());
    }
    let mut tree = Command::new("cargo");
    tree.current_dir(root()).args([
        "tree",
        "--workspace",
        "-e",
        "normal,build,dev",
        "--target",
        "all",
        "--prefix",
        "none",
    ]);
    for name in names {
        tree.args(["-i", name]);
    }
    let out = tree.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let mut found = dependents(&String::from_utf8_lossy(&out.stdout), &root());
    found.extend(names.iter().cloned());
    Some(found)
}

/// The workspace's packages in what `cargo tree --prefix none` printed: those whose folder is
/// under the root.
fn dependents(tree: &str, root: &std::path::Path) -> BTreeSet<String> {
    let inside = format!("({}", root.display());
    tree.lines()
        .filter(|line| line.contains(&inside))
        .filter_map(|line| line.split(' ').next())
        .map(String::from)
        .collect()
}

/// The command-line options naming packages: `-p a -p b`.
pub(crate) fn package_args(names: &BTreeSet<String>) -> Vec<String> {
    let mut args = Vec::new();
    for name in names {
        args.push("-p".to_string());
        args.push(name.clone());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crates() -> BTreeSet<String> {
        [
            "terra_build",
            "terra_build_mesh",
            "terra_build_scenes",
            "terra_core",
            "terra_land",
            "terra_game",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    }

    fn files(list: &[&str]) -> Vec<String> {
        list.iter().map(|f| f.to_string()).collect()
    }

    fn set(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|f| f.to_string()).collect()
    }

    #[test]
    fn docs_and_settings_touch_no_crate() {
        let scope = classify(
            &files(&[
                ".claude/settings.json",
                "docs/development.md",
                "docs/decisions/D1.md",
                "README.md",
                "crates/README.md",
            ]),
            &crates(),
        );
        assert!(!scope.all());
        assert!(scope.changed.is_empty() && scope.readers.is_empty());
        assert!(!scope.rust && !scope.assets && scope.tools.is_empty());
    }

    #[test]
    fn a_crates_file_is_that_crate() {
        let scope = classify(
            &files(&[
                "crates/terra_core/src/lib.rs",
                "crates/terra_land/README.md",
            ]),
            &crates(),
        );
        assert_eq!(scope.changed, set(&["terra_core", "terra_land"]));
        assert!(scope.rust && !scope.manifests);
        let manifest = classify(&files(&["crates/terra_core/Cargo.toml"]), &crates());
        assert!(manifest.manifests && !manifest.all());
    }

    #[test]
    fn a_file_read_from_outside_a_crate_tests_its_reader() {
        let scope = classify(
            &files(&["tools/sandbox/places.ts", "docs/building-voxels.json"]),
            &crates(),
        );
        assert!(scope.changed.is_empty());
        assert_eq!(scope.readers, set(&["terra_land", "terra_build_scenes"]));
        assert_eq!(scope.tools, set(&["sandbox"]));
    }

    #[test]
    fn fires_code_shaders_textures_and_tonemapping_run_the_fire_sheet_and_nothing_else_does() {
        for file in [
            "crates/terra_world/src/fire.rs",
            "crates/terra_world/src/fire/seen.rs",
            "assets/shaders/fire_smoke.wesl",
            "assets/textures/fire/flames.ktx2",
            "crates/terra_world/src/grading.rs",
            "tools/fire/measure.ts",
        ] {
            assert!(
                classify(&files(&[file]), &set(&["terra_world", "terra_game"])).fire,
                "{file}"
            );
        }
        for file in [
            "crates/terra_world/src/firefly.rs",
            "assets/shaders/water.wesl",
            "tools/sandbox/places.ts",
            "docs/fire.md",
        ] {
            assert!(
                !classify(&files(&[file]), &set(&["terra_world", "terra_game"])).fire,
                "{file}"
            );
        }
    }

    #[test]
    fn an_asset_is_read_by_the_crates_on_bevy_and_those_reading_it_alone() {
        let bevy = [
            "terra_world",
            "terra_bases",
            "terra_walker",
            "terra_survival",
            "terra_creatures",
            "terra_game",
            "terra_sandbox",
        ];
        let scope = classify(&files(&["assets/data/kits/log.ron"]), &crates());
        assert!(scope.assets);
        assert_eq!(
            scope.readers,
            set(&[
                &bevy[..],
                &[
                    "terra_build",
                    "terra_build_layout",
                    "terra_build_mesh",
                    "terra_build_scenes"
                ]
            ]
            .concat())
        );
        let scope = classify(&files(&["assets/models/tree/pine.glb"]), &crates());
        assert_eq!(scope.readers, set(&[&bevy[..], &["terra_bake"]].concat()));
        let scope = classify(&files(&["assets/data/life.ron"]), &crates());
        assert_eq!(scope.readers, set(&[&bevy[..], &["terra_town"]].concat()));
    }

    #[test]
    fn the_build_itself_or_an_unknown_file_runs_everything() {
        for file in [
            "Cargo.lock",
            "Cargo.toml",
            ".cargo/config.toml",
            "rust-toolchain.toml",
            "somewhere/new.txt",
            "crates/terra_new/src/lib.rs",
        ] {
            let scope = classify(&files(&["docs/a.md", file]), &crates());
            assert!(scope.all(), "{file} should run everything");
        }
    }

    #[test]
    fn a_baseline_is_its_own_class_and_tests_no_crate() {
        // The scheduled run's lowering commit: the check's cheap stages and nothing else.
        let scope = classify(
            &files(&[
                "crates/terra_build_scenes/examples/artifacts-baseline/fuzz/seed-2.tsv",
                "crates/terra_build_scenes/examples/artifacts-baseline/all/sheet.tsv",
                "xtask/sizes-baseline.tsv",
                "docs/decisions.md",
            ]),
            &crates(),
        );
        assert!(scope.baselines && !scope.all());
        assert!(scope.changed.is_empty() && scope.readers.is_empty());
        assert!(!scope.rust && !scope.manifests && !scope.assets);
        assert!(
            scope.summary().ends_with("baselines"),
            "{}",
            scope.summary()
        );
        // Beside them, the checker and the tasks are still their crates, and a file only
        // named like the folder is not in it.
        for (file, crate_) in [
            (
                "crates/terra_build_scenes/examples/artifacts.rs",
                "terra_build_scenes",
            ),
            (
                "crates/terra_build_scenes/examples/artifacts-baseline.tsv",
                "terra_build_scenes",
            ),
            ("xtask/src/sizes.rs", "xtask"),
        ] {
            let scope = classify(&files(&[file]), &crates());
            assert!(!scope.baselines, "{file}");
            assert_eq!(scope.changed, set(&[crate_]), "{file}");
        }
    }

    #[test]
    fn python_is_seen_for_the_blender_check() {
        let scope = classify(&files(&["tools/blender/fit.py"]), &crates());
        assert!(scope.python);
        assert_eq!(scope.tools, set(&["blender"]));
    }

    #[test]
    fn dependents_are_the_workspaces_packages_in_the_tree() {
        let root = std::path::Path::new("/r");
        let tree = "terra_build v0.1.0 (/r/crates/terra_build)\n\
                    terra_game v0.1.0 (/r/crates/terra_game)\n\
                    terra_game v0.1.0 (/r/crates/terra_game) (*)\n\
                    [dev-dependencies]\n\
                    bevy v0.20.0\n";
        assert_eq!(dependents(tree, root), set(&["terra_build", "terra_game"]));
    }

    #[test]
    fn cargo_finds_the_crates_depending_on_one() {
        let found = with_dependents(&set(&["terra_build"])).expect("cargo tree answers");
        // Known from the manifests: the game draws buildings and the sandbox stands on the
        // game; the world and the generator are below building (D407).
        assert!(found.is_superset(&set(&["terra_build", "terra_game", "terra_sandbox"])));
        assert!(!found.contains("terra_world"));
        assert!(!found.contains("terra_gen"));
    }

    /// A crate that starts reading a file outside its folder without a line in `READ_BY` would
    /// not be tested when that file changes.
    #[test]
    fn every_file_a_crate_reads_is_in_read_by() {
        let root = root();
        let mut sources = Vec::new();
        assert!(crate::check::sources(&root.join("crates"), &mut sources));
        let (mut checked, mut assets) = (0, 0);
        for file in sources {
            let text = std::fs::read_to_string(&file).unwrap();
            let package = file
                .strip_prefix(root.join("crates"))
                .unwrap()
                .components()
                .next()
                .unwrap()
                .as_os_str()
                .to_string_lossy()
                .to_string();
            for literal in text.split('"').skip(1).step_by(2) {
                let Some(path) = outside_path(&root, literal) else {
                    continue;
                };
                checked += 1;
                assets += usize::from(path.starts_with("assets/"));
                // A folder named is under its prefix too.
                let under = |prefix: &str| {
                    path.starts_with(prefix) || prefix.strip_suffix('/') == Some(path.as_str())
                };
                assert!(
                    READ_BY
                        .iter()
                        .any(|(prefix, readers)| under(prefix)
                            && readers.contains(&package.as_str())),
                    "{} reads {path}: add it to READ_BY in xtask/src/scope.rs",
                    file.display()
                );
            }
        }
        assert!(
            checked > 0 && assets > 0,
            "no path outside the crates, or none under assets/, found: the search is wrong"
        );
    }

    /// The file outside the crates a string in a crate names, from the root: a path to a
    /// folder outside them, or a file under `assets/` named from there (`data/life.ron`, as a
    /// crate joins it to the folder or Bevy's asset server loads it). The folder alone is
    /// not one: what is read in it is named after it.
    fn outside_path(root: &std::path::Path, literal: &str) -> Option<String> {
        let path = literal.trim_start_matches('/').trim_start_matches("../");
        let named = ["tools/", "docs/", "art/", "research/", "xtask/", "assets/"]
            .iter()
            .any(|p| path.starts_with(p));
        let path = if named {
            path.to_string()
        } else if path.contains('/') && !path.contains(['{', '\\']) {
            format!("assets/{path}")
        } else {
            return None;
        };
        root.join(&path).exists().then_some(path)
    }
}
