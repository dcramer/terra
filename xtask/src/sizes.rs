//! The size ratchet (D396): no Rust file over [`FILE_LINES`] lines and no function over
//! [`FN_LINES`], except those already over when it began, kept in [`BASELINE`], which may
//! shrink and drop out but never grow. The full check (`cargo xtask check --all`) runs it, and
//! `cargo xtask sizes` runs it alone; `--lower`, the full run on main (`cargo xtask nightly`)'s alone (D414), lowers
//! the baseline to what is there now, dropping what is under the limits, and never adds to it
//! or raises it.
//!
//! Functions are found by reading the source, not parsing it: each `fn name` and its body's
//! braces, skipping strings, characters and comments. A function's lines run from the line
//! of its `fn` to its closing brace, a function written inside it included.

use crate::{Task, output, root};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The task this module runs (D410).
pub(crate) const TASKS: &[Task] = &[Task {
    name: "sizes",
    takes: "[--lower]",
    does: "the size ratchet the full check runs: no Rust file over 1,000 lines or function over 150 but those in xtask/sizes-baseline.tsv, which never grow; --lower, the full run on main (`cargo xtask nightly`)'s alone (D414), lowers the baseline to what is there",
    group: "Play and check",
    run: sizes,
}];

/// Over this many lines a file is doing more than one thing: a module per concern reads
/// better, builds in parallel and keeps two people's edits apart. Most of the code is under
/// half of it.
pub(crate) const FILE_LINES: usize = 1_000;

/// Over this many lines, about three screens, a function no longer reads at once and is
/// several steps that want their own names.
pub(crate) const FN_LINES: usize = 150;

/// What was over the limits when the ratchet began, as it is now, relative to the root.
const BASELINE: &str = "xtask/sizes-baseline.tsv";

/// The folders whose Rust is measured: the workspace and the tasks. Spikes are thrown away.
const MEASURED: [&str; 2] = ["crates", "xtask"];

const HEADER: &str = "# What was over the size limits (files over 1,000 lines, functions over 150) when the\n\
# ratchet began, as it is now (D396). Each may shrink, never grow, and nothing is added:\n\
# the full run on main (`cargo xtask nightly`) (`cargo xtask sizes --lower`, D414) lowers it and drops what is under.\n\
# path\tfunction (- for the file)\tlines\n";

/// A function as read: its name, the line its `fn` is on (from 1) and how many lines it spans.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Function {
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) lines: usize,
}

/// What a source holds: its functions, in the order they close, and whether its braces
/// balanced as read (they always do in code that compiles; false means the reading is wrong).
#[derive(Debug)]
pub(crate) struct Read {
    pub(crate) functions: Vec<Function>,
    pub(crate) balanced: bool,
    /// The lines each string written over several lines runs over, first and last.
    pub(crate) strings: Vec<(usize, usize)>,
}

fn ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Reads a Rust source's functions. A `fn` followed by a name starts one; its body is the
/// first `{` outside the signature's brackets, and a `;` there first makes it a declaration.
/// Strings (raw ones too), byte strings, character literals and comments (nested block
/// comments too) are skipped, and a lifetime is told from a character by what follows it.
pub(crate) fn read(text: &str) -> Read {
    let mut reader = Reader {
        chars: text.chars().collect(),
        i: 0,
        line: 1,
        depth: 0,
        open: Vec::new(),
        pending: None,
        read: Read {
            functions: Vec::new(),
            balanced: true,
            strings: Vec::new(),
        },
    };
    while reader.i < reader.chars.len() {
        reader.step();
    }
    let mut read = reader.read;
    read.balanced &= reader.depth == 0 && reader.open.is_empty();
    read
}

/// Where [`read`] is in a source, and what it has found.
struct Reader {
    chars: Vec<char>,
    i: usize,
    line: usize,
    /// Braces open.
    depth: usize,
    /// The functions whose bodies are open: name, first line, brace depth inside the body.
    open: Vec<(String, usize, usize)>,
    /// A `fn name` whose body has not begun: name, line, brackets open in its signature.
    pending: Option<(String, usize, usize)>,
    read: Read,
}

impl Reader {
    fn at(&self, i: usize) -> char {
        self.chars.get(i).copied().unwrap_or('\0')
    }

    /// Moves on by one character, counting the line it ends.
    fn next(&mut self) {
        if self.at(self.i) == '\n' {
            self.line += 1;
        }
        self.i += 1;
    }

    /// Reads one token, or one character that is none.
    fn step(&mut self) {
        let c = self.at(self.i);
        match c {
            '/' if self.at(self.i + 1) == '/' => {
                while self.i < self.chars.len() && self.at(self.i) != '\n' {
                    self.i += 1;
                }
            }
            '/' if self.at(self.i + 1) == '*' => self.block_comment(),
            '"' => {
                self.i += 1;
                self.string(0, false);
            }
            '\'' => self.character(),
            c if ident(c) => self.word(),
            '(' | '[' | ')' | ']' => {
                if let Some((_, _, brackets)) = &mut self.pending {
                    *brackets = if matches!(c, '(' | '[') {
                        *brackets + 1
                    } else {
                        brackets.saturating_sub(1)
                    };
                }
                self.i += 1;
            }
            ';' => {
                if matches!(self.pending, Some((_, _, 0))) {
                    self.pending = None;
                }
                self.i += 1;
            }
            '{' => self.open_brace(),
            '}' => self.close_brace(),
            _ => self.next(),
        }
    }

    /// A block comment, its nested ones within it.
    fn block_comment(&mut self) {
        let mut nested = 1;
        self.i += 2;
        while self.i < self.chars.len() && nested > 0 {
            if self.at(self.i) == '/' && self.at(self.i + 1) == '*' {
                nested += 1;
                self.i += 2;
            } else if self.at(self.i) == '*' && self.at(self.i + 1) == '/' {
                nested -= 1;
                self.i += 2;
            } else {
                self.next();
            }
        }
    }

    /// The rest of a string whose opening quote is just behind: to its closing quote and
    /// `hashes` hashes. A raw string (`hashes` > 0, or `raw`) has no escapes.
    fn string(&mut self, hashes: usize, raw: bool) {
        let first = self.line;
        while self.i < self.chars.len() {
            let c = self.at(self.i);
            if c == '\\' && !raw {
                self.i += 1;
                self.next();
            } else if c == '"' && (1..=hashes).all(|k| self.at(self.i + k) == '#') {
                self.i += hashes + 1;
                break;
            } else {
                self.next();
            }
        }
        if self.line > first {
            self.read.strings.push((first, self.line));
        }
    }

    /// A character literal ('x', '\n', '\u{7f}'), or a lifetime or a label, which is left.
    fn character(&mut self) {
        if self.at(self.i + 1) == '\\' {
            let mut j = self.i + 3;
            while j < self.chars.len() && self.chars[j] != '\'' {
                j += 1;
            }
            self.i = j + 1;
        } else if self.at(self.i + 2) == '\'' {
            self.i += 3;
        } else {
            self.i += 1;
        }
    }

    /// A word: a raw string's prefix, a function's `fn`, or any other name.
    fn word(&mut self) {
        let start = self.i;
        while ident(self.at(self.i)) {
            self.i += 1;
        }
        if start > 0 && ident(self.chars[start - 1]) {
            return;
        }
        let word: String = self.chars[start..self.i].iter().collect();
        if matches!(word.as_str(), "r" | "br" | "cr") && matches!(self.at(self.i), '"' | '#') {
            let mut hashes = 0;
            while self.at(self.i + hashes) == '#' {
                hashes += 1;
            }
            if self.at(self.i + hashes) == '"' {
                self.i += hashes + 1;
                self.string(hashes, true);
            }
        } else if word == "fn" {
            let mut j = self.i;
            while matches!(self.at(j), ' ' | '\t' | '\n' | '\r') {
                j += 1;
            }
            if self.at(j) == 'r' && self.at(j + 1) == '#' {
                j += 2;
            }
            let name_start = j;
            while ident(self.at(j)) {
                j += 1;
            }
            if j > name_start && !self.at(name_start).is_ascii_digit() {
                let name: String = self.chars[name_start..j].iter().collect();
                self.pending = Some((name, self.line, 0));
            }
        }
    }

    /// A brace opening: a function's body when one's signature is complete.
    fn open_brace(&mut self) {
        self.depth += 1;
        // A brace inside the signature, as in a const generic's `{ N }`, leaves it pending.
        if matches!(self.pending, Some((_, _, 0)))
            && let Some((name, first, _)) = self.pending.take()
        {
            self.open.push((name, first, self.depth));
        }
        self.i += 1;
    }

    /// A brace closing: a function's end when it closes that function's body.
    fn close_brace(&mut self) {
        if self
            .open
            .last()
            .is_some_and(|(_, _, inside)| *inside == self.depth)
            && let Some((name, first, _)) = self.open.pop()
        {
            self.read.functions.push(Function {
                name,
                line: first,
                lines: self.line - first + 1,
            });
        }
        if self.depth == 0 {
            self.read.balanced = false;
        }
        self.depth = self.depth.saturating_sub(1);
        self.i += 1;
    }
}

/// What is over the limits, by path and function (`-` for the file), at its most lines.
type Sizes = BTreeMap<(String, String), usize>;

/// Where each function over the limit starts, by the same keys: the line of its `fn`.
type Places = BTreeMap<(String, String), usize>;

/// What is over the limits in some sources and where, and the paths that could not be read.
fn measure(sources: &[(String, String)]) -> (Sizes, Places, Vec<String>) {
    let mut over = Sizes::new();
    let mut places = Places::new();
    let mut unread = Vec::new();
    for (path, text) in sources {
        let lines = text.lines().count();
        if lines > FILE_LINES {
            over.insert((path.clone(), "-".into()), lines);
        }
        let read = read(text);
        if !read.balanced {
            unread.push(path.clone());
        }
        for function in read.functions {
            if function.lines > FN_LINES {
                let key = (path.clone(), function.name);
                let size = over.entry(key.clone()).or_default();
                if function.lines > *size {
                    *size = function.lines;
                    places.insert(key, function.line);
                }
            }
        }
    }
    (over, places, unread)
}

fn parse(text: &str) -> Sizes {
    let mut sizes = Sizes::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        if let [path, item, lines] = line.split('\t').collect::<Vec<_>>()[..]
            && let Ok(lines) = lines.parse()
        {
            sizes.insert((path.to_string(), item.to_string()), lines);
        }
    }
    sizes
}

fn render(sizes: &Sizes) -> String {
    let mut text = String::from(HEADER);
    for ((path, item), lines) in sizes {
        let _ = writeln!(text, "{path}\t{item}\t{lines}");
    }
    text
}

/// What the ratchet finds: what is new over a limit, what grew, and what shrank or went.
#[derive(Debug, Default, PartialEq)]
struct Verdict {
    new: Vec<String>,
    grew: Vec<String>,
    shrank: usize,
}

fn judge(now: &Sizes, places: &Places, recorded: &Sizes) -> Verdict {
    let mut verdict = Verdict::default();
    for ((path, item), &lines) in now {
        let limit = if item == "-" { FILE_LINES } else { FN_LINES };
        let what = || match places.get(&(path.clone(), item.clone())) {
            _ if item == "-" => path.clone(),
            Some(line) => format!("{path}:{line}: fn {item}"),
            None => format!("{path}: fn {item}"),
        };
        match recorded.get(&(path.clone(), item.clone())) {
            None => verdict
                .new
                .push(format!("{} is {lines} lines (limit {limit})", what())),
            Some(&was) if lines > was => verdict.grew.push(format!(
                "{} grew to {lines} lines (it was {was}; limit {limit})",
                what()
            )),
            Some(&was) if lines < was => verdict.shrank += 1,
            Some(_) => {}
        }
    }
    verdict.shrank += recorded
        .keys()
        .filter(|key| !now.contains_key(*key))
        .count();
    verdict
}

/// What a failure says to do: split, and how.
const SPLIT: [&str; 6] = [
    "Split it rather than raise the limit (there is no option to):",
    "- a long function: name its steps and make each its own function, with a struct for what",
    "  they share; a long match: a function per arm; a long test: a helper for its setup",
    "- a long file: a folder module, `name/mod.rs` with a file per concern beside it, its tests",
    "  in `name/tests.rs`; see how `terra_build::layout` and `xtask/src` are split",
    "An offender in the baseline may shrink; leave its entry, the full run on main (`cargo xtask nightly`) lowers it (D414).",
];

/// The tracked Rust sources measured, as path and text, or `None` when git cannot list them.
fn sources() -> Option<Vec<(String, String)>> {
    let mut args = vec!["ls-files", "--"];
    args.extend(MEASURED);
    let listed = output("git", &args)?;
    Some(
        listed
            .lines()
            .filter(|path| path.ends_with(".rs"))
            .filter_map(|path| {
                let text = std::fs::read_to_string(root().join(path)).ok()?;
                Some((path.to_string(), text))
            })
            .collect(),
    )
}

/// The check's stage: what fixes it when something is new over a limit or grew.
pub(crate) fn sizes_hold(out: &mut String) -> bool {
    let Some(sources) = sources() else {
        let _ = writeln!(out, "git cannot list the Rust sources");
        return false;
    };
    let recorded = parse(&std::fs::read_to_string(root().join(BASELINE)).unwrap_or_default());
    let (now, places, unread) = measure(&sources);
    report(&judge(&now, &places, &recorded), &unread, out)
}

fn report(verdict: &Verdict, unread: &[String], out: &mut String) -> bool {
    let mut ok = true;
    if !unread.is_empty() {
        let _ = writeln!(
            out,
            "cannot measure, its braces do not balance as read (fix the reading in xtask/src/sizes.rs):"
        );
        for path in unread {
            let _ = writeln!(out, "  {path}");
        }
        ok = false;
    }
    if !verdict.new.is_empty() {
        let _ = writeln!(
            out,
            "over the size limits (files {FILE_LINES} lines, functions {FN_LINES}):"
        );
        for line in &verdict.new {
            let _ = writeln!(out, "  {line}");
        }
        ok = false;
    }
    if !verdict.grew.is_empty() {
        let _ = writeln!(out, "over the limits already, and grew ({BASELINE}):");
        for line in &verdict.grew {
            let _ = writeln!(out, "  {line}");
        }
        ok = false;
    }
    if !verdict.new.is_empty() || !verdict.grew.is_empty() {
        for line in SPLIT {
            let _ = writeln!(out, "{line}");
        }
    }
    ok
}

/// `cargo xtask sizes [--lower]`: the ratchet alone, saying what shrank; `--lower` lowers
/// the baseline to what is there now.
pub(crate) fn sizes(args: &[String]) -> bool {
    let lowering = match args {
        [] => false,
        [flag] if flag == "--lower" => true,
        _ => {
            eprintln!("usage: cargo xtask sizes [--lower]");
            return false;
        }
    };
    let Some(sources) = sources() else {
        eprintln!("git cannot list the Rust sources");
        return false;
    };
    let path = root().join(BASELINE);
    let recorded = parse(&std::fs::read_to_string(&path).unwrap_or_default());
    let (now, places, unread) = measure(&sources);
    let verdict = judge(&now, &places, &recorded);
    let mut out = String::new();
    let ok = report(&verdict, &unread, &mut out);
    print!("{out}");
    if lowering {
        let lowered = lower(&recorded, &now);
        if lowered != recorded {
            if let Err(error) = std::fs::write(&path, render(&lowered)) {
                eprintln!("cannot write {BASELINE}: {error}");
                return false;
            }
            println!(
                "{BASELINE}: lowered to what is there, {} entries",
                lowered.len()
            );
        }
    } else if verdict.shrank > 0 {
        println!(
            "{} offenders shrank or went; leave the baseline, the full run on main (`cargo xtask nightly`) lowers it (D414)",
            verdict.shrank
        );
    }
    if ok {
        println!(
            "✓ sizes: nothing new over {FILE_LINES} lines a file or {FN_LINES} a function, and nothing in the baseline grew ({} entries)",
            lower(&recorded, &now).len()
        );
    }
    ok
}

/// The baseline lowered to what is there now: each entry at the lesser of the two, and those
/// now under the limits dropped. Nothing is added or raised.
fn lower(recorded: &Sizes, now: &Sizes) -> Sizes {
    recorded
        .iter()
        .filter_map(|(key, &was)| Some((key.clone(), now.get(key)?.min(&was).to_owned())))
        .collect()
}

/// A size baseline lowered at one commit (`at` to `lowered`) carried onto a later commit's
/// (`now`), for the full run on main's push after main moved (D411): an entry lowered or dropped
/// is lowered or dropped there too while its file did not change in between (`since`) and
/// the later baseline has it as it was; everything else is the later baseline's.
pub(crate) fn carry_lowered(
    at: &str,
    lowered: &str,
    now: &str,
    since: &std::collections::BTreeSet<String>,
) -> String {
    let (at, lowered) = (parse(at), parse(lowered));
    let mut now = parse(now);
    for (key, was) in &at {
        if since.contains(&key.0) || now.get(key) != Some(was) {
            continue;
        }
        match lowered.get(key) {
            Some(&less) => now.insert(key.clone(), less.min(*was)),
            None => now.remove(key),
        };
    }
    render(&now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lowering_is_carried_onto_a_later_baseline_only_where_its_file_is_unchanged() {
        let rows = |list: &[(&str, usize)]| -> String {
            list.iter()
                .map(|(path, lines)| format!("{path}\tf\t{lines}\n"))
                .collect()
        };
        let at = rows(&[("a.rs", 200), ("b.rs", 190), ("c.rs", 180), ("d.rs", 170)]);
        // Measured there: a fell, b went under the limit, c and d fell.
        let lowered = rows(&[("a.rs", 160), ("c.rs", 175), ("d.rs", 165)]);
        // Later: c's file changed, d's entry was lowered by another run, e is new.
        let now = rows(&[
            ("a.rs", 200),
            ("b.rs", 190),
            ("c.rs", 180),
            ("d.rs", 168),
            ("e.rs", 155),
        ]);
        let since = ["c.rs".to_string()].into_iter().collect();
        let carried = carry_lowered(&at, &lowered, &now, &since);
        let expect = rows(&[("a.rs", 160), ("c.rs", 180), ("d.rs", 168), ("e.rs", 155)]);
        assert_eq!(parse(&carried), parse(&expect));
        assert!(carried.starts_with(HEADER));
    }

    /// A source as lines, so no line of it stands alone in this file to be misread.
    fn source(lines: &[&str]) -> String {
        lines.join("\n")
    }

    fn sizes(text: &str) -> Vec<(String, usize, usize)> {
        let read = read(text);
        assert!(read.balanced, "{text}");
        let mut found: Vec<_> = read
            .functions
            .into_iter()
            .map(|f| (f.name, f.line, f.lines))
            .collect();
        found.sort_by_key(|f| f.1);
        found
    }

    #[test]
    fn a_nested_function_is_its_own_and_its_outers_too() {
        let text = source(&[
            "fn outer() {",          // 1
            "    let a = 1;",        // 2
            "    fn inner(x: u8) {", // 3
            "        let _ = x;",    // 4
            "    }",                 // 5
            "    let b = 2;",        // 6
            "}",                     // 7
            "fn after() {}",         // 8
        ]);
        assert_eq!(
            sizes(&text),
            vec![
                ("outer".into(), 1, 7),
                ("inner".into(), 3, 3),
                ("after".into(), 8, 1)
            ]
        );
    }

    #[test]
    fn braces_in_closures_strings_characters_and_comments_do_not_end_a_function() {
        let text = source(&[
            "fn tricky<'a>(s: &'a str) -> impl Fn(u8) -> u8 + 'a {",
            "    let open = '{';",
            "    let close = '}';",
            "    let quote = '\\'';",
            "    let escaped = \"a } \\\" { fn fake() {\";",
            "    let raw = r#\"} \" fn nope() {\"#;",
            "    let bytes = b\"}}\";",
            "    // } fn commented() {",
            "    /* } /* nested } */ still a comment { */",
            "    let f = |x: u8| { if x > 1 { x } else { 0 } };",
            "    'outer: loop { break 'outer; }",
            "    move |x| f(x)",
            "}",
            "fn next() {",
            "}",
        ]);
        assert_eq!(
            sizes(&text),
            vec![("tricky".into(), 1, 13), ("next".into(), 14, 2)]
        );
    }

    #[test]
    fn declarations_and_function_types_are_not_functions() {
        let text = source(&[
            "trait Shape {",
            "    fn area(&self, sides: [u8; 4]) -> f32;",
            "    fn named(&self) -> &str {",
            "        \"shape\"",
            "    }",
            "}",
            "struct Stage { check: fn(&mut String) -> bool }",
            "fn takes(f: fn(u8) -> u8, n: [u8; 2]) -> u8",
            "where",
            "    u8: Copy,",
            "{",
            "    f(n[0])",
            "}",
            "fn sized() -> [u8; { 1 + 1 }] {",
            "    [0; 2]",
            "}",
        ]);
        assert_eq!(
            sizes(&text),
            vec![
                ("named".into(), 3, 3),
                ("takes".into(), 8, 6),
                ("sized".into(), 14, 3)
            ]
        );
    }

    /// Checked another way: rustfmt starts a function at its `fn` line and closes it with a
    /// brace alone at the same indent. Every function so written in the repository's own Rust
    /// must measure the same by the reader.
    #[test]
    fn the_reader_agrees_with_rustfmts_layout_over_the_whole_repository() {
        let sources = super::sources().expect("git lists the sources");
        assert!(sources.len() > 100, "{} sources", sources.len());
        let mut compared = 0;
        for (path, text) in &sources {
            let read = read(text);
            assert!(read.balanced, "{path} reads unbalanced");
            let measured: BTreeMap<usize, usize> =
                read.functions.iter().map(|f| (f.line, f.lines)).collect();
            let lines: Vec<&str> = text.lines().collect();
            for (index, line) in lines.iter().enumerate() {
                let indent = line.len() - line.trim_start().len();
                let words: Vec<&str> = line.split_whitespace().collect();
                let at = words.iter().position(|w| *w == "fn");
                let is_item = at.is_some_and(|at| {
                    words[..at].iter().all(|w| {
                        ["pub", "const", "async", "unsafe", "extern"].contains(w)
                            || w.starts_with("pub(")
                            || w.starts_with('"')
                    }) && words
                        .get(at + 1)
                        .is_some_and(|name| name.starts_with(char::is_alphabetic))
                });
                let in_string = read
                    .strings
                    .iter()
                    .any(|&(first, last)| (first..=last).contains(&(index + 1)));
                if !is_item || line.contains('}') || in_string {
                    continue;
                }
                // The signature ends at the first line at its indent ending in `{` (a body) or
                // `;` (a declaration); the body at the next brace alone at that indent.
                let mut opened = None;
                for (j, next) in lines.iter().enumerate().skip(index).take(40) {
                    let level = next.len() - next.trim_start().len();
                    if j > index && level != indent {
                        continue;
                    }
                    if next.trim_end().ends_with('{') {
                        opened = Some(j);
                        break;
                    }
                    if next.trim_end().ends_with(';') {
                        break;
                    }
                }
                let Some(opened) = opened else { continue };
                let close = format!("{}}}", " ".repeat(indent));
                let Some(end) = lines[opened + 1..].iter().position(|l| *l == close) else {
                    continue;
                };
                let expected = opened + 1 + end - index + 1;
                assert_eq!(
                    measured.get(&(index + 1)),
                    Some(&expected),
                    "{path}:{}",
                    index + 1
                );
                compared += 1;
            }
        }
        assert!(compared > 1_000, "only {compared} functions compared");
    }

    fn sized(entries: &[(&str, &str, usize)]) -> Sizes {
        entries
            .iter()
            .map(|(p, i, n)| ((p.to_string(), i.to_string()), *n))
            .collect()
    }

    #[test]
    fn a_new_offender_or_one_that_grew_fails_and_one_that_shrank_passes() {
        let recorded = sized(&[
            ("a.rs", "-", 1_400),
            ("a.rs", "big", 300),
            ("b.rs", "gone", 200),
        ]);
        let now = sized(&[
            ("a.rs", "-", 1_300),
            ("a.rs", "big", 301),
            ("c.rs", "new", 151),
        ]);
        let places = Places::from([(("a.rs".into(), "big".into()), 40)]);
        let verdict = judge(&now, &places, &recorded);
        assert_eq!(verdict.new, vec!["c.rs: fn new is 151 lines (limit 150)"]);
        assert_eq!(
            verdict.grew,
            vec!["a.rs:40: fn big grew to 301 lines (it was 300; limit 150)"]
        );
        assert_eq!(verdict.shrank, 2);
        let mut out = String::new();
        assert!(!report(&verdict, &[], &mut out));
        assert!(out.contains("Split it"), "{out}");
        let shrunk = sized(&[("a.rs", "-", 1_300), ("a.rs", "big", 299)]);
        let mut out = String::new();
        assert!(report(&judge(&shrunk, &places, &recorded), &[], &mut out));
        assert!(out.is_empty(), "{out}");
    }

    #[test]
    fn recording_lowers_and_drops_but_never_adds_or_raises() {
        let recorded = sized(&[
            ("a.rs", "-", 1_400),
            ("a.rs", "big", 300),
            ("b.rs", "gone", 200),
        ]);
        let now = sized(&[
            ("a.rs", "-", 1_500),
            ("a.rs", "big", 250),
            ("c.rs", "new", 151),
        ]);
        assert_eq!(
            lower(&recorded, &now),
            sized(&[("a.rs", "-", 1_400), ("a.rs", "big", 250)])
        );
        assert_eq!(parse(&render(&recorded)), recorded);
    }

    #[test]
    fn a_planted_long_function_and_file_are_found() {
        let mut lines = vec!["fn long() {".to_string()];
        lines.extend((0..FN_LINES).map(|i| format!("    let _x{i} = {i};")));
        lines.push("}".into());
        lines.extend((0..FILE_LINES).map(|_| "// more".to_string()));
        let (over, places, unread) = measure(&[("planted.rs".into(), lines.join("\n"))]);
        assert!(unread.is_empty());
        assert_eq!(places.get(&("planted.rs".into(), "long".into())), Some(&1));
        assert_eq!(
            over,
            sized(&[
                ("planted.rs", "-", FILE_LINES + FN_LINES + 2),
                ("planted.rs", "long", FN_LINES + 2)
            ])
        );
    }

    /// The repository as it is passes: nothing is over the limits that the baseline does not
    /// hold at its size or more.
    #[test]
    fn the_repository_holds_to_its_baseline() {
        let mut out = String::new();
        assert!(sizes_hold(&mut out), "{out}");
    }
}
