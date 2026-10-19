//! The lines between the crates `terra_game` is to be cut into (D407, step 7), held here
//! until step 8 makes them crates and Cargo holds them: every module is placed in the crate
//! it is to move to (research/2026-10-18-crate-layout.md, "On Bevy"), and a module naming
//! one in a crate above its own, or beside it (building play beside creatures and
//! townsfolk), fails, unless the crossing is one step 7 has not cut yet, listed with its
//! seam in [`NOT_YET_CUT`]. A new module is placed here; a seam cut takes its rows out.
//! Tests and test-only items may name anything. The check reads the sources; it builds
//! nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The crates `terra_game` is to be cut into, and the app left on top.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Crate {
    /// `terra_bases`: the base on screen.
    Bases,
    /// `terra_walker`: the walker layer (D198).
    Walker,
    /// `terra_survival`: what a player carries, makes and needs.
    Survival,
    /// `terra_creatures`: creatures and fighting.
    Creatures,
    /// `terra_folk`: townsfolk.
    Folk,
    /// `terra_building`: building play.
    Building,
    /// `terra_game`: the app, bundling the rest.
    App,
}

impl Crate {
    /// Whether this crate may name `other`: itself, or one under it in the research's graph,
    /// where building play forks from creatures and townsfolk above survival.
    fn may_name(self, other: Crate) -> bool {
        use Crate::*;
        self == other
            || match self {
                Bases => false,
                Walker => other == Bases,
                Survival => matches!(other, Bases | Walker),
                Creatures | Building => matches!(other, Bases | Walker | Survival),
                Folk => matches!(other, Bases | Walker | Survival | Creatures),
                App => true,
            }
    }
}

/// Where each module goes, by its path under `src/` (`play/claim`). The crate's root,
/// `play` and `build` hold modules of every crate, so each of their children is placed by
/// name; a deeper module goes with its parent.
const PLACES: &[(&str, Crate)] = {
    use Crate::*;
    &[
        // The app: the plugins bundling the rest, saving, the probe, the map, sound.
        ("lib", App),
        ("play", App),
        ("play/map", App),
        ("play/music", App),
        ("play/noises", App),
        ("play/probe", App),
        ("play/save", App),
        ("play/sounds", App),
        ("play/soundscape", App),
        // The base on screen.
        ("build", Bases),
        ("build/arrays", Bases),
        ("build/atlases", Bases),
        ("build/draw", Bases),
        ("build/footing", Bases),
        ("build/hammer", Bases),
        ("build/hlod", Bases),
        ("build/layout", Bases),
        ("build/light", Bases),
        ("build/maps", Bases),
        ("build/memory", Bases),
        ("build/store", Bases),
        ("build/views", Bases),
        // The walker.
        ("appearance", Walker),
        ("character", Walker),
        ("lift", Walker),
        ("model_files", Walker),
        ("pace", Walker),
        ("skinning", Walker),
        ("swing", Walker),
        ("play/bodies", Walker),
        ("play/button", Walker),
        ("play/camera", Walker),
        ("play/controls", Walker),
        ("play/figure", Walker),
        ("play/intent", Walker),
        ("play/movement", Walker),
        ("play/options", Walker),
        ("play/walker", Walker),
        // Survival.
        ("icons", Survival),
        ("play/bag", Survival),
        ("play/catalogue", Survival),
        ("play/claim", Survival),
        ("play/contract", Survival),
        ("play/crafting", Survival),
        ("play/death", Survival),
        ("play/dig", Survival),
        ("play/gains", Survival),
        ("play/harvest", Survival),
        ("play/held", Survival),
        ("play/interact", Survival),
        ("play/inventory", Survival),
        ("play/kinds", Survival),
        ("play/noise", Survival),
        ("play/pile", Survival),
        ("play/props", Survival),
        ("play/registries", Survival),
        ("play/skills", Survival),
        ("play/station_screen", Survival),
        ("play/storage", Survival),
        ("play/storage_screen", Survival),
        ("play/tiles", Survival),
        ("play/told", Survival),
        ("play/torchlight", Survival),
        ("play/vitals", Survival),
        ("play/warmth", Survival),
        ("play/wear", Survival),
        ("play/work", Survival),
        ("play/worn", Survival),
        // Creatures and fighting.
        ("play/archery", Creatures),
        ("play/coats", Creatures),
        ("play/combat", Creatures),
        ("play/creatures", Creatures),
        ("play/fliers", Creatures),
        ("play/hits", Creatures),
        ("play/melee", Creatures),
        ("play/sighted", Creatures),
        ("play/species", Creatures),
        ("play/staged", Creatures),
        ("play/wildlife", Creatures),
        ("play/zones", Creatures),
        // Townsfolk.
        ("play/people", Folk),
        ("play/pooling", Folk),
        ("play/rising", Folk),
        ("play/scaffold", Folk),
        ("play/settlement", Folk),
        ("play/stall_goods", Folk),
        ("play/town", Folk),
        ("play/town_animals", Folk),
        ("play/town_fences", Folk),
        ("play/town_gardens", Folk),
        ("play/town_sound", Folk),
        ("play/townsfolk", Folk),
        // Building play.
        ("build/act", Building),
        ("build/fire", Building),
        ("build/going_up", Building),
        ("build/loss", Building),
        ("build/props", Building),
        ("build/remote", Building),
        ("build/sheet", Building),
        ("build/village", Building),
        ("play/build_menu", Building),
        ("play/dial", Building),
        ("play/tool", Building),
    ]
};

/// The crossings step 7 has not cut yet: the module that names, the module it names, and
/// the seam that cuts it, numbered as the research's "Where the shared types go" lists
/// them from `Village` (1), then those found since (10 on).
const NOT_YET_CUT: &[(&str, &str, &str)] = &[
    (
        "play/crafting",
        "play/people",
        "5: a station's user, owned by survival",
    ),
    (
        "play/warmth",
        "play/people",
        "5: a fire's user, owned by survival",
    ),
    (
        "play/harvest",
        "play/combat",
        "6: combat::Swing into survival",
    ),
    (
        "play/registries",
        "play/species",
        "7: creatures read their own bestiary",
    ),
    (
        "play/station_screen",
        "play/archery",
        "8: the bow as an item into survival",
    ),
    (
        "play/sighted",
        "play/town_animals",
        "9: a tame marker the creatures own",
    ),
    (
        "build",
        "build/act",
        "10: the base's plugin adds building play's",
    ),
    (
        "build",
        "build/fire",
        "10: the base's plugin adds building play's",
    ),
    (
        "build",
        "build/going_up",
        "10: the base's plugin adds building play's",
    ),
    (
        "build",
        "build/loss",
        "10: the base's plugin adds building play's; Built::fallen",
    ),
    (
        "build",
        "build/props",
        "10: the base's plugin adds building play's",
    ),
    (
        "build",
        "build/remote",
        "10: the base's plugin adds building play's",
    ),
    (
        "build",
        "build/sheet",
        "10: the base's plugin adds building play's",
    ),
    (
        "build",
        "build/village",
        "10: the base's plugin adds building play's",
    ),
    (
        "build/fire",
        "play/creatures",
        "11: a fire frightens creatures (Frightful)",
    ),
    (
        "play/fliers",
        "build/fire",
        "11: a flier breathes fire (BreathesFire)",
    ),
    (
        "build/loss",
        "play/hits",
        "12: what falls shakes the view (hits::Shake)",
    ),
    (
        "play/rising",
        "build/props",
        "13: a rising house hangs its doors (fit_all)",
    ),
    (
        "play/storage",
        "build/act",
        "14: storage acts on a base (act::Building)",
    ),
    (
        "play/wildlife",
        "play/rising",
        "15: wildlife and a rising town (Rising)",
    ),
];

/// This test's own module.
const OWN: &str = "seams";

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Every module of the crate by its path under `src/` (`lib` the root), and its file; a
/// module of tests (`tests.rs`, `tests/`) is left out.
fn modules() -> BTreeMap<String, PathBuf> {
    fn walk(dir: &Path, out: &mut BTreeMap<String, PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if stem == "tests" {
                continue;
            }
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let relative = path.strip_prefix(src()).unwrap_or(&path);
                let name = relative
                    .with_extension("")
                    .to_string_lossy()
                    .replace('\\', "/");
                let name = name.strip_suffix("/mod").unwrap_or(&name).to_string();
                out.insert(name, path);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(&src(), &mut out);
    out
}

/// The module's path as segments from the crate's root.
fn split(module: &str) -> Vec<String> {
    if module == "lib" {
        Vec::new()
    } else {
        module.split('/').map(str::to_string).collect()
    }
}

fn module_name(segments: &[String]) -> String {
    if segments.is_empty() {
        "lib".to_string()
    } else {
        segments.join("/")
    }
}

/// Where a module goes: its own place, or its nearest placed parent's.
fn place(module: &str) -> Option<Crate> {
    let mut at = split(module);
    loop {
        let name = module_name(&at);
        if let Some(&(_, crate_)) = PLACES.iter().find(|(m, _)| *m == name) {
            return Some(crate_);
        }
        at.pop()?;
    }
}

/// Code with its comments, strings and character literals blanked (newlines kept, so lines
/// still count), and every `#[cfg(test)]` item blanked with them.
fn code_of(text: &str) -> Vec<u8> {
    let b = text.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    while i < b.len() {
        let rest = &b[i..];
        let after_ident = i > 0 && ident(b[i - 1]);
        let end = if rest.starts_with(b"//") {
            rest.iter()
                .position(|&c| c == b'\n')
                .map_or(b.len(), |n| i + n)
        } else if rest.starts_with(b"/*") {
            let (mut depth, mut j) = (0usize, i);
            while j < b.len() {
                if b[j..].starts_with(b"/*") {
                    depth += 1;
                    j += 2;
                } else if b[j..].starts_with(b"*/") {
                    depth -= 1;
                    j += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    j += 1;
                }
            }
            j
        } else if !after_ident && b[i] == b'r' && raw_string_hashes(rest).is_some() {
            let hashes = raw_string_hashes(rest).unwrap_or(0);
            let open = i + 2 + hashes;
            let mut close = vec![b'"'];
            close.extend(std::iter::repeat_n(b'#', hashes));
            b[open..]
                .windows(close.len())
                .position(|w| w == close.as_slice())
                .map_or(b.len(), |n| open + n + close.len())
        } else if b[i] == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            j + 1
        } else if b[i] == b'\'' && (rest.get(1) == Some(&b'\\') || rest.get(2) == Some(&b'\'')) {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'\'' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            j + 1
        } else {
            i += 1;
            continue;
        };
        blank(&mut out, i, end);
        i = end;
    }
    blank_test_items(&mut out);
    out
}

/// The number of `#` of a raw string starting here (`r"`, `r#"`), if one does.
fn raw_string_hashes(rest: &[u8]) -> Option<usize> {
    let hashes = rest[1..].iter().take_while(|&&c| c == b'#').count();
    (rest.get(1 + hashes) == Some(&b'"')).then_some(hashes)
}

fn blank(out: &mut [u8], from: usize, to: usize) {
    let to = to.min(out.len());
    for c in &mut out[from..to] {
        if *c != b'\n' {
            *c = b' ';
        }
    }
}

/// Blanks each test-only item: to the end of its statement, or its block.
fn blank_test_items(out: &mut [u8]) {
    let attribute = b"#[cfg(test)]";
    let mut from = 0;
    while let Some(at) = out[from..]
        .windows(attribute.len())
        .position(|w| w == attribute)
        .map(|n| from + n)
    {
        let mut j = at + attribute.len();
        while j < out.len() && out[j] != b';' && out[j] != b'{' {
            j += 1;
        }
        if j < out.len() && out[j] == b'{' {
            let mut depth = 0usize;
            while j < out.len() {
                match out[j] {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
        }
        let end = (j + 1).min(out.len());
        blank(out, at, end);
        from = end;
    }
}

fn ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// A token of code, as far as paths need: a word, `::`, a brace, a comma, a star.
#[derive(Clone, PartialEq, Debug)]
enum Tok {
    Word(String),
    Sep,
    Open,
    Close,
    Comma,
    Star,
    Other,
}

/// The code's tokens, each with its line.
fn tokens(code: &[u8]) -> Vec<(Tok, usize)> {
    let mut out = Vec::new();
    let (mut i, mut line) = (0, 1);
    while i < code.len() {
        let c = code[i];
        let (tok, len) = if c == b'\n' {
            line += 1;
            i += 1;
            continue;
        } else if c.is_ascii_whitespace() {
            i += 1;
            continue;
        } else if ident(c) {
            let n = code[i..].iter().take_while(|&&c| ident(c)).count();
            let word = String::from_utf8_lossy(&code[i..i + n]).into_owned();
            (Tok::Word(word), n)
        } else if code[i..].starts_with(b"::") {
            (Tok::Sep, 2)
        } else {
            let tok = match c {
                b'{' => Tok::Open,
                b'}' => Tok::Close,
                b',' => Tok::Comma,
                b'*' => Tok::Star,
                _ => Tok::Other,
            };
            (tok, 1)
        };
        out.push((tok, line));
        i += len;
    }
    out
}

/// A path the code names: its segments, the name a `use` gives it, its line, and whether a
/// `use` brings it in.
#[derive(Debug)]
struct Named {
    segments: Vec<String>,
    alias: Option<String>,
    line: usize,
    used: bool,
    glob: bool,
}

/// Every path in the code, with `use` groups (`a::{b, c::d}`) spread into one each.
fn paths(tokens: &[(Tok, usize)]) -> Vec<Named> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let before = i.checked_sub(1).map(|b| &tokens[b].0);
        // A visibility's path (`pub(in crate::play)`) names no module's contents.
        let starts = matches!(tokens[i].0, Tok::Word(_))
            && before != Some(&Tok::Sep)
            && before != Some(&Tok::Word("in".into()));
        if starts {
            let used = i > 0 && tokens[i - 1].0 == Tok::Word("use".into());
            i = path(tokens, i, Vec::new(), used, &mut out);
        } else {
            i += 1;
        }
    }
    out
}

/// Reads one path from `i`, under the group's `prefix`; returns where it ends.
fn path(
    tokens: &[(Tok, usize)],
    mut i: usize,
    mut segments: Vec<String>,
    used: bool,
    out: &mut Vec<Named>,
) -> usize {
    let line = tokens[i].1;
    let word = |w: &str| Tok::Word(w.to_string());
    loop {
        match tokens.get(i).map(|t| &t.0) {
            Some(Tok::Word(w)) => {
                if w != "self" || segments.is_empty() {
                    segments.push(w.clone());
                }
                i += 1;
            }
            Some(Tok::Open) if !segments.is_empty() => {
                i += 1;
                while let Some((tok, _)) = tokens.get(i) {
                    match tok {
                        Tok::Close => return i + 1,
                        Tok::Word(_) => i = path(tokens, i, segments.clone(), used, out),
                        _ => i += 1,
                    }
                }
                return i;
            }
            Some(Tok::Star) if !segments.is_empty() => {
                let (alias, glob) = (None, true);
                out.push(Named {
                    segments,
                    alias,
                    line,
                    used,
                    glob,
                });
                return i + 1;
            }
            _ => break,
        }
        if tokens.get(i).map(|t| &t.0) == Some(&Tok::Sep) {
            i += 1;
        } else {
            break;
        }
    }
    let mut alias = None;
    if tokens.get(i).map(|t| &t.0) == Some(&word("as"))
        && let Some((Tok::Word(name), _)) = tokens.get(i + 1)
    {
        alias = Some(name.clone());
        i += 2;
    }
    let glob = false;
    out.push(Named {
        segments,
        alias,
        line,
        used,
        glob,
    });
    i
}

/// The crate as the check sees it: its modules, and what each one's `use` lines bring in.
struct Seen {
    modules: BTreeSet<String>,
    uses: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

impl Seen {
    fn is_module(&self, segments: &[String]) -> bool {
        self.modules.contains(&module_name(segments))
    }

    /// The module a path from `from` names: the deepest module along it, or, where it
    /// meets a name a module brings in by `use`, the module that name is from. `None` for
    /// a path out of the crate.
    fn target(&self, from: &str, segments: &[String], depth: usize) -> Option<String> {
        if depth > 16 {
            return None;
        }
        let mut at = split(from);
        let mut rest = segments;
        match rest.first()?.as_str() {
            "crate" => {
                at.clear();
                rest = &rest[1..];
            }
            "self" => rest = &rest[1..],
            "super" => {
                while rest.first().is_some_and(|s| s == "super") {
                    at.pop()?;
                    rest = &rest[1..];
                }
            }
            first => {
                let mut child = at.clone();
                child.push(first.to_string());
                if !self.is_module(&child) {
                    return None;
                }
            }
        }
        for segment in rest {
            at.push(segment.clone());
            if self.is_module(&at) {
                continue;
            }
            at.pop();
            let here = module_name(&at);
            if let Some(to) = self.uses.get(&here).and_then(|u| u.get(segment)) {
                return self.target(&here, to, depth + 1);
            }
            return Some(here);
        }
        Some(module_name(&at))
    }
}

/// Every module's paths, and the crate as the check sees it.
fn read() -> (Seen, BTreeMap<String, Vec<Named>>) {
    let mut named = BTreeMap::new();
    let mut uses = BTreeMap::new();
    for (module, file) in modules() {
        let text = std::fs::read_to_string(&file).expect("read");
        let found = paths(&tokens(&code_of(&text)));
        let brought: BTreeMap<String, Vec<String>> = found
            .iter()
            .filter(|n| n.used && !n.glob)
            .filter_map(|n| {
                let name = n.alias.clone().or_else(|| n.segments.last().cloned())?;
                Some((name, n.segments.clone()))
            })
            .collect();
        uses.insert(module.clone(), brought);
        named.insert(module, found);
    }
    let modules = named.keys().cloned().collect();
    (Seen { modules, uses }, named)
}

/// Every place a module names one in a crate it may not: (asker, named, `file:line`).
fn crossings() -> Vec<(String, String, String)> {
    let (crate_, named) = read();
    let mut out = Vec::new();
    for (module, paths) in &named {
        if module == OWN {
            continue;
        }
        let Some(asker) = place(module) else {
            continue;
        };
        for n in paths.iter().filter(|n| n.segments.len() > 1) {
            let Some(target) = crate_.target(module, &n.segments, 0) else {
                continue;
            };
            let Some(owner) = place(&target) else {
                continue;
            };
            if target != *module && !asker.may_name(owner) {
                let path = n.segments.join("::");
                let at = format!("{module}:{} names `{path}` ({target})", n.line);
                out.push((module.clone(), target, at));
            }
        }
    }
    out
}

#[test]
fn every_module_is_placed() {
    let unplaced: Vec<String> = modules()
        .into_keys()
        .filter(|m| m != OWN)
        .filter(|m| {
            let parent = m.rsplit_once('/').map_or("", |(p, _)| p);
            matches!(parent, "" | "play" | "build") && !PLACES.iter().any(|(p, _)| p == m)
        })
        .collect();
    assert!(
        unplaced.is_empty(),
        "place each module in the crate it is to move to, in src/seams.rs's PLACES (D407): \
         {unplaced:?}"
    );
}

#[test]
fn no_module_names_one_above_or_beside_it() {
    let found = crossings();
    let mut new = Vec::new();
    for (asker, target, at) in &found {
        if !NOT_YET_CUT
            .iter()
            .any(|(a, t, _)| a == asker && t == target)
        {
            new.push(at.clone());
        }
    }
    let cut: Vec<String> = NOT_YET_CUT
        .iter()
        .filter(|(a, t, _)| !found.iter().any(|(fa, ft, _)| fa == a && ft == t))
        .map(|(a, t, seam)| format!("{a} -> {t} ({seam})"))
        .collect();
    assert!(
        new.is_empty(),
        "a module names one in a crate above or beside its own (D407): move the type it needs \
         down to the side that asks, or the code up beside what it names:\n  {}",
        new.join("\n  ")
    );
    assert!(
        cut.is_empty(),
        "these seams are cut: take their rows out of src/seams.rs's NOT_YET_CUT:\n  {}",
        cut.join("\n  ")
    );
}

/// The check sees a module named every way, and none of what only looks like it.
#[test]
fn it_sees_a_module_named_every_way() {
    let code = code_of(
        "use super::{a, b::{self, C as D}};\nuse crate::e::*;\n// super::f in a comment\n\
         const S: &str = \"super::g\";\nfn h() { x::<T>::y(); super::i::J::new() }\n\
         #[cfg(test)]\nmod tests { use super::k; }\n",
    );
    let found: Vec<(String, usize, bool)> = paths(&tokens(&code))
        .into_iter()
        .filter(|n| n.segments.len() > 1)
        .map(|n| (n.segments.join("::"), n.line, n.used))
        .collect();
    let expected = [
        ("super::a", 1, true),
        ("super::b", 1, true),
        ("super::b::C", 1, true),
        ("crate::e", 2, true),
        ("super::i::J::new", 5, false),
    ];
    let expected: Vec<(String, usize, bool)> = expected
        .iter()
        .map(|&(p, l, u)| (p.to_string(), l, u))
        .collect();
    assert_eq!(found, expected);
}

/// A name brought in by `use` is followed to the module it is from.
#[test]
fn a_name_brought_in_is_followed() {
    let map = |pairs: &[(&str, &[&str])]| -> BTreeMap<String, Vec<String>> {
        pairs
            .iter()
            .map(|(n, p)| (n.to_string(), p.iter().map(|s| s.to_string()).collect()))
            .collect()
    };
    let crate_ = Seen {
        modules: ["lib", "play", "play/walker", "play/claim"]
            .map(String::from)
            .into(),
        uses: [
            (
                "play".to_string(),
                map(&[("Player", &["walker", "Player"])]),
            ),
            (
                "lib".to_string(),
                map(&[("planet", &["terra_world", "planet"])]),
            ),
        ]
        .into(),
    };
    let path = |p: &str| p.split("::").map(String::from).collect::<Vec<_>>();
    let to = |p: &str| crate_.target("play/claim", &path(p), 0);
    assert_eq!(to("super::Player"), Some("play/walker".into()));
    assert_eq!(to("super::walker::Stance"), Some("play/walker".into()));
    assert_eq!(to("crate::play::GamePlugins"), Some("play".into()));
    assert_eq!(to("crate::planet::Planet"), None);
    assert_eq!(to("bevy::prelude"), None);
}
