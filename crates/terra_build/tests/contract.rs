//! The voxel contract proved row by row (docs/building-voxels.md, D348): its junction matrix
//! is data (docs/building-voxels.json), and every closed row names the code that closes it
//! and what proves it, a test or a case of the scene registry the mesh checker's ratchet
//! holds at no fault. A row whose code is not there, whose test is missing or ignored, or
//! whose case is not registered or has faults recorded against it, fails here; a closed row
//! with nothing to prove it fails too. The docs check renders the page from the same data.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use terra_build::scenes::registry::{Materials, registry};

#[derive(Deserialize)]
struct Contract {
    sections: Vec<Section>,
}

#[derive(Deserialize)]
struct Section {
    title: String,
    rows: Vec<Row>,
}

#[derive(Deserialize)]
struct Row {
    junction: String,
    code: Vec<String>,
    tests: Vec<String>,
    cases: Vec<String>,
    status: String,
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// The source files of a module by its path under `src/` (`layout::roofs`), or of an
/// integration test (`tests::rules`): its own file, and with `inner`, every file under its
/// folder too; its tests' files (`tests.rs`, `tests/`) either way.
fn files(module: &str, inner: bool) -> Vec<PathBuf> {
    if let Some(test) = module.strip_prefix("tests::") {
        return vec![root().join("tests").join(format!("{test}.rs"))];
    }
    let path = root().join("src").join(module.replace("::", "/"));
    let mut out: Vec<PathBuf> = [
        path.with_extension("rs"),
        path.join("mod.rs"),
        path.join("tests.rs"),
    ]
    .into_iter()
    .filter(|file| file.is_file())
    .collect();
    let mut folders = vec![path.join("tests")];
    if inner {
        folders.push(path.clone());
    }
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
            let file = entry.path();
            if file.is_dir() && inner {
                folders.push(file);
            } else if file.extension().is_some_and(|e| e == "rs") && !out.contains(&file) {
                out.push(file);
            }
        }
    }
    out
}

fn read(files: &[PathBuf]) -> String {
    files
        .iter()
        .map(|file| std::fs::read_to_string(file).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether `text` holds `word` as a whole identifier.
fn holds(text: &str, word: &str) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back().is_none_or(|c| !ident(c));
        let after = text[at + word.len()..]
            .chars()
            .next()
            .is_none_or(|c| !ident(c));
        before && after
    })
}

/// What is wrong with a test named `module::name`, if anything: not there, not a test, or
/// ignored, so not run by `cargo xtask check`.
fn test_fault(test: &str) -> Option<String> {
    let (module, name) = test.rsplit_once("::")?;
    unrun(&read(&files(module, false)), test, name)
}

/// What is wrong with the test `name` in a module's `text`, if anything.
fn unrun(text: &str, test: &str, name: &str) -> Option<String> {
    let at = text.find(&format!("fn {name}("));
    let Some(at) = at else {
        return Some(format!("{test}: no such test there"));
    };
    // The attributes and comments over it, up to the code before.
    let over: Vec<&str> = text[..at]
        .lines()
        .rev()
        .skip(1)
        .map(str::trim)
        .take_while(|line| line.starts_with("#[") || line.starts_with("//") || line.is_empty())
        .collect();
    let line = text[..at].lines().next_back().unwrap_or("").trim();
    let attributes: Vec<&str> = over.into_iter().chain([line]).collect();
    if !attributes.iter().any(|line| line.starts_with("#[test]")) {
        Some(format!("{test}: not a #[test]"))
    } else if attributes.iter().any(|line| line.starts_with("#[ignore")) {
        Some(format!("{test}: ignored, so never run by the check"))
    } else {
        None
    }
}

/// What is wrong with a piece of code named by its path (`layout::roles::End::Shared`): the
/// longest start of it that is a module, and every name after that in its files.
fn code_fault(code: &str) -> Option<String> {
    let parts: Vec<&str> = code.split("::").collect();
    for split in (1..=parts.len()).rev() {
        let module = parts[..split].join("::");
        let found = files(&module, true);
        if found.is_empty() {
            continue;
        }
        let text = read(&found);
        let missing: Vec<&&str> = parts[split..]
            .iter()
            .filter(|name| !holds(&text, name))
            .collect();
        return (!missing.is_empty()).then(|| format!("{code}: {module} has no {missing:?}"));
    }
    Some(format!("{code}: no such module"))
}

/// The groups of scenes the ratchet's baseline has faults recorded for, in any set.
fn faulty_groups() -> BTreeSet<String> {
    let baseline = root().join("examples/artifacts-baseline.tsv");
    std::fs::read_to_string(baseline)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split('\t').nth(1).map(str::to_string))
        .collect()
}

#[test]
fn every_row_of_the_voxel_contract_is_proved() {
    let text = std::fs::read_to_string(root().join("../../docs/building-voxels.json"))
        .expect("docs/building-voxels.json");
    let contract: Contract = serde_json::from_str(&text).expect("the contract reads");
    let assets = root().join("../../assets");
    let materials = Materials::read(&assets, true).expect("the baseline's materials");
    let scenes: Vec<String> = registry(&materials).into_iter().map(|s| s.name).collect();
    let faulty = faulty_groups();
    let mut faults = Vec::new();
    let mut rows = 0;
    for section in &contract.sections {
        for row in &section.rows {
            rows += 1;
            let at = format!("{}: {}", section.title, row.junction);
            if row.status == "Closed" && row.tests.is_empty() && row.cases.is_empty() {
                faults.push(format!("{at}: closed with no test or case to prove it"));
            }
            if row.status == "Closed" && row.code.is_empty() {
                faults.push(format!("{at}: closed with no code named that closes it"));
            }
            faults.extend(row.code.iter().filter_map(|code| code_fault(code)));
            faults.extend(row.tests.iter().filter_map(|test| test_fault(test)));
            for case in &row.cases {
                let matching: Vec<&String> = scenes
                    .iter()
                    .filter(|name| name.contains(case.as_str()))
                    .collect();
                let group = |name: &str| name.split(':').next().unwrap_or(name).trim().to_string();
                if matching.is_empty() {
                    faults.push(format!("{at}: no registered scene is named \"{case}\""));
                } else if matching.iter().all(|name| faulty.contains(&group(name))) {
                    faults.push(format!(
                        "{at}: \"{case}\" is only in scenes whose groups have faults recorded in the ratchet's baseline: {matching:?}"
                    ));
                }
            }
        }
    }
    assert!(rows > 40, "the contract has {rows} rows");
    assert!(faults.is_empty(), "{faults:#?}");
}

#[test]
fn a_contract_fault_is_seen_for_what_is_not_there() {
    // Known answers from the code itself: a test that is there and run, one that is not
    // there, a type and its variant, and a name no module has.
    assert_eq!(
        test_fault("tests::rules::a_roof_is_cut_only_to_the_shapes_a_roof_has"),
        None
    );
    assert!(test_fault("layout::roles::no_such_test_anywhere_at_all").is_some());
    assert!(test_fault("layout::roles::how_kinds_meet").is_some());
    let text = "    #[test]\n    fn a() {}\n\n#[test]\n// Fails today.\n#[ignore = \"why\"]\nfn b() {}\n\nfn c() {}\n";
    assert_eq!(unrun(text, "a", "a"), None);
    assert!(unrun(text, "b", "b").is_some_and(|why| why.contains("ignored")));
    assert!(unrun(text, "c", "c").is_some_and(|why| why.contains("not a #[test]")));
    assert_eq!(code_fault("layout::roles::End::Shared"), None);
    assert_eq!(code_fault("layout::roofs::Gutter"), None);
    assert!(code_fault("layout::roles::End::Nowhere").is_some());
    assert!(code_fault("nowhere::at_all").is_some());
}
