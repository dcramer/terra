//! Material instances (D290, docs/proposals/construction-002.md): a construction material's
//! file says only what makes it itself, and the rest comes from its pattern's file and its
//! reference pack, merged under it, as an Unreal material instance overrides a few
//! parameters of its parent material and a Minecraft block model names a `parent`.
//!
//! The merge is of RON as written, so an enum's variant is kept as named: two structs merge
//! field by field (a role's `Profile(extend: 0.22)` over `Profile(name: "lintel", ...)`
//! changes only its `extend`), two maps entry by entry, `Some(a)` over `Some(b)` as `a` over
//! `b`, and anything else is replaced whole. A tuple of numbers, such as a colour, is
//! replaced whole too: its parts mean nothing alone. A map's entry given as `None` takes the
//! one under it away, as cob leaves bare the corners its pattern quoins (`Arris: None`).

/// A RON value as written: enough of the grammar to merge kit files and write them back.
#[derive(Clone, Debug, PartialEq)]
enum Node {
    /// A number, a string as written with its quotes, or a name (`true`, `None`, `Masonry`).
    Atom(String),
    /// `Name(field: value, ...)` or `(field: value, ...)`.
    Struct(Option<String>, Vec<(String, Node)>),
    /// `Name(value, ...)` or `(value, ...)`.
    Tuple(Option<String>, Vec<Node>),
    List(Vec<Node>),
    /// `{key: value, ...}`, each key as written.
    Map(Vec<(String, Node)>),
}

struct Parser<'a> {
    text: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn error(&self, what: &str) -> String {
        let line = self.text[..self.at.min(self.text.len())]
            .iter()
            .filter(|&&c| c == b'\n')
            .count()
            + 1;
        format!("line {line}: {what}")
    }

    /// Skips white space and comments.
    fn skip(&mut self) {
        loop {
            while self.at < self.text.len() && self.text[self.at].is_ascii_whitespace() {
                self.at += 1;
            }
            if self.text[self.at..].starts_with(b"//") {
                while self.at < self.text.len() && self.text[self.at] != b'\n' {
                    self.at += 1;
                }
            } else if self.text[self.at..].starts_with(b"/*") {
                let end = self.text[self.at + 2..]
                    .windows(2)
                    .position(|w| w == b"*/")
                    .map_or(self.text.len(), |p| self.at + 2 + p + 2);
                self.at = end;
            } else {
                return;
            }
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip();
        self.text.get(self.at).copied()
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        if self.peek() == Some(c) {
            self.at += 1;
            Ok(())
        } else {
            Err(self.error(&format!("expected `{}`", c as char)))
        }
    }

    fn word(&mut self) -> Option<String> {
        self.skip();
        let start = self.at;
        while self.at < self.text.len()
            && (self.text[self.at].is_ascii_alphanumeric()
                || matches!(self.text[self.at], b'_' | b'.' | b'-' | b'+'))
        {
            self.at += 1;
        }
        (self.at > start).then(|| String::from_utf8_lossy(&self.text[start..self.at]).into_owned())
    }

    fn string(&mut self) -> Result<String, String> {
        let start = self.at;
        self.at += 1;
        while self.at < self.text.len() && self.text[self.at] != b'"' {
            if self.text[self.at] == b'\\' {
                self.at += 1;
            }
            self.at += 1;
        }
        if self.at >= self.text.len() {
            return Err(self.error("a string with no end"));
        }
        self.at += 1;
        Ok(String::from_utf8_lossy(&self.text[start..self.at]).into_owned())
    }

    fn value(&mut self) -> Result<Node, String> {
        match self.peek() {
            Some(b'"') => Ok(Node::Atom(self.string()?)),
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                while self.peek() != Some(b']') {
                    items.push(self.value()?);
                    if self.peek() == Some(b',') {
                        self.at += 1;
                    }
                }
                self.at += 1;
                Ok(Node::List(items))
            }
            Some(b'{') => {
                self.at += 1;
                let mut entries = Vec::new();
                while self.peek() != Some(b'}') {
                    let key = print(&self.value()?);
                    self.expect(b':')?;
                    entries.push((key, self.value()?));
                    if self.peek() == Some(b',') {
                        self.at += 1;
                    }
                }
                self.at += 1;
                Ok(Node::Map(entries))
            }
            Some(b'(') => self.parens(None),
            Some(_) => {
                let word = self.word().ok_or_else(|| self.error("expected a value"))?;
                if self.peek() == Some(b'(') {
                    self.parens(Some(word))
                } else {
                    Ok(Node::Atom(word))
                }
            }
            None => Err(self.error("expected a value, found the end")),
        }
    }

    /// What is in brackets after a name or none: fields if the first thing in it is a name
    /// and a colon, else values.
    fn parens(&mut self, name: Option<String>) -> Result<Node, String> {
        self.expect(b'(')?;
        let back = self.at;
        let fields = self.word().is_some() && self.peek() == Some(b':');
        self.at = back;
        if fields || self.peek() == Some(b')') {
            let mut out = Vec::new();
            while self.peek() != Some(b')') {
                let field = self.word().ok_or_else(|| self.error("expected a field"))?;
                self.expect(b':')?;
                out.push((field, self.value()?));
                if self.peek() == Some(b',') {
                    self.at += 1;
                }
            }
            self.at += 1;
            Ok(Node::Struct(name, out))
        } else {
            let mut out = Vec::new();
            while self.peek() != Some(b')') {
                out.push(self.value()?);
                if self.peek() == Some(b',') {
                    self.at += 1;
                }
            }
            self.at += 1;
            Ok(Node::Tuple(name, out))
        }
    }
}

fn parse(text: &str) -> Result<Node, String> {
    let mut parser = Parser {
        text: text.as_bytes(),
        at: 0,
    };
    let node = parser.value()?;
    if parser.peek().is_some() {
        return Err(parser.error("more after the value"));
    }
    Ok(node)
}

fn print(node: &Node) -> String {
    let name = |n: &Option<String>| n.clone().unwrap_or_default();
    match node {
        Node::Atom(a) => a.clone(),
        Node::Struct(n, fields) => format!(
            "{}({})",
            name(n),
            fields
                .iter()
                .map(|(f, v)| format!("{f}: {}", print(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Node::Tuple(n, items) => format!(
            "{}({}{})",
            name(n),
            items.iter().map(print).collect::<Vec<_>>().join(", "),
            // A one-tuple with no name is written with a comma, so it is not read as brackets.
            if n.is_none() && items.len() == 1 {
                ","
            } else {
                ""
            }
        ),
        Node::List(items) => format!(
            "[{}]",
            items.iter().map(print).collect::<Vec<_>>().join(", ")
        ),
        Node::Map(entries) => format!(
            "{{{}}}",
            entries
                .iter()
                .map(|(k, v)| format!("{k}: {}", print(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// `over` merged onto `base`.
fn merge(base: Node, over: Node) -> Node {
    match (base, over) {
        (Node::Struct(bn, mut fields), Node::Struct(on, more)) if on.is_none() || on == bn => {
            for (field, value) in more {
                match fields.iter_mut().find(|(f, _)| *f == field) {
                    Some((_, old)) => *old = merge(old.clone(), value),
                    None => fields.push((field, value)),
                }
            }
            Node::Struct(bn.or(on), fields)
        }
        (Node::Map(mut entries), Node::Map(more)) => {
            for (key, value) in more {
                // An entry of `None` takes the one under it away: a role the pattern dresses
                // that this material leaves bare.
                if value == Node::Atom("None".into()) {
                    entries.retain(|(k, _)| *k != key);
                    continue;
                }
                match entries.iter_mut().find(|(k, _)| *k == key) {
                    Some((_, old)) => *old = merge(old.clone(), value),
                    None => entries.push((key, value)),
                }
            }
            Node::Map(entries)
        }
        (Node::Tuple(Some(bn), b), Node::Tuple(Some(on), o))
            if bn == on && bn == "Some" && b.len() == 1 && o.len() == 1 =>
        {
            let inner = merge(b.into_iter().next().unwrap_or(Node::Atom("()".into())), {
                o.into_iter().next().unwrap_or(Node::Atom("()".into()))
            });
            Node::Tuple(Some(bn), vec![inner])
        }
        (_, over) => over,
    }
}

/// Where a material's pattern and pack files are, under `assets/`.
pub const PATTERNS: &str = "data/kits/patterns";
pub const PACKS: &str = "data/packs";

/// A material's file resolved into a whole kit's, as RON: its pattern's file
/// (`pattern: "block"`, from [`PATTERNS`]), then its pack (`pack: "fieldstone"`, from
/// [`PACKS`]), then its own lines, `weight` being how heavy it reads where kinds meet
/// (`meets.weight`). A pattern or pack may itself name a pack under it, as lime-washed earth
/// is packed earth washed. A file naming none of them is a whole kit already, and is given
/// back as it is. `read` gives a file under `assets/` as text.
pub fn resolve(
    text: &str,
    read: &dyn Fn(&str) -> Result<String, String>,
) -> Result<String, String> {
    match layered(parse(text)?, read, 0)? {
        Some(node) => Ok(print(&node)),
        None => Ok(text.to_string()),
    }
}

/// The texture sets a material's own lines and its packs give, its pattern's left out:
/// the roles it fills itself, or by the pack it names under it, rather than leaving them to
/// its pattern's defaults (`packs`, D349). Each as its `textures` entry names it.
pub fn own_textures(
    text: &str,
    read: &dyn Fn(&str) -> Result<String, String>,
) -> Result<Vec<String>, String> {
    let Node::Struct(name, mut fields) = parse(text)? else {
        return Ok(Vec::new());
    };
    fields.retain(|(field, value)| {
        !(field == "pattern" && matches!(value, Node::Atom(a) if a.starts_with('"')))
    });
    let node = Node::Struct(name, fields);
    let whole = layered(node.clone(), read, 0)?.unwrap_or(node);
    let Node::Struct(_, fields) = whole else {
        return Ok(Vec::new());
    };
    Ok(fields
        .into_iter()
        .filter(|(field, _)| field == "textures")
        .flat_map(|(_, value)| match value {
            Node::Map(entries) => entries
                .into_iter()
                .filter(|(_, value)| !matches!(value, Node::Atom(a) if a == "None"))
                .map(|(key, _)| key)
                .collect(),
            _ => Vec::new(),
        })
        .collect())
}

/// Every field a material's files set, each with the file under `assets/` that sets it: its
/// own (`own`), its pattern's and its packs', as `timber` or, a level down, `colours.light`.
/// So the checker can refuse a field nothing reads for its pattern, wherever it was written.
pub fn fields_set(
    own: &str,
    text: &str,
    read: &dyn Fn(&str) -> Result<String, String>,
) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    let mut files = vec![(own.to_string(), text.to_string())];
    let mut depth = 0;
    while let Some((file, text)) = files.pop() {
        depth += 1;
        if depth > 16 {
            return Err("patterns and packs name one another in a loop".into());
        }
        let Node::Struct(_, fields) = parse(&text).map_err(|why| format!("{file}: {why}"))? else {
            continue;
        };
        for (field, value) in fields {
            let quoted = matches!(&value, Node::Atom(a) if a.starts_with('"'));
            match (field.as_str(), &value) {
                ("pattern", Node::Atom(name)) if quoted => {
                    let path = format!("{PATTERNS}/{}.ron", name.trim_matches('"'));
                    files.push((path.clone(), read(&path)?));
                }
                ("pack", Node::Atom(name)) if quoted => {
                    let path = format!("{PACKS}/{}.ron", name.trim_matches('"'));
                    files.push((path.clone(), read(&path)?));
                }
                (_, Node::Struct(None, inner)) => {
                    out.push((file.clone(), field.clone()));
                    for (each, _) in inner {
                        out.push((file.clone(), format!("{field}.{each}")));
                    }
                }
                _ => out.push((file.clone(), field)),
            }
        }
    }
    Ok(out)
}

/// A file's value over what it names under it, if it names anything.
fn layered(
    node: Node,
    read: &dyn Fn(&str) -> Result<String, String>,
    depth: u32,
) -> Result<Option<Node>, String> {
    let Node::Struct(name, mut fields) = node else {
        return Ok(None);
    };
    if depth > 8 {
        return Err("patterns and packs name one another in a loop".into());
    }
    let mut take = |field: &str, quoted: bool| -> Option<String> {
        let at = fields.iter().position(|(f, v)| {
            f == field && matches!(v, Node::Atom(a) if a.starts_with('"') == quoted)
        })?;
        let Node::Atom(atom) = fields.remove(at).1 else {
            return None;
        };
        Some(atom.trim_matches('"').to_string())
    };
    let pattern = take("pattern", true);
    let pack = take("pack", true);
    let weight = take("weight", false);
    if pattern.is_none() && pack.is_none() && weight.is_none() {
        return Ok(None);
    }
    let mut layers = Vec::new();
    for path in [
        pattern.map(|p| format!("{PATTERNS}/{p}.ron")),
        pack.map(|p| format!("{PACKS}/{p}.ron")),
    ]
    .into_iter()
    .flatten()
    {
        let under = parse(&read(&path)?).map_err(|why| format!("{path}: {why}"))?;
        let under = layered(under.clone(), read, depth + 1)
            .map_err(|why| format!("{path}: {why}"))?
            .unwrap_or(under);
        layers.push(under);
    }
    if let Some(weight) = weight {
        fields.push((
            "meets".into(),
            Node::Tuple(
                Some("Some".into()),
                vec![Node::Struct(
                    None,
                    vec![("weight".into(), Node::Atom(weight))],
                )],
            ),
        ));
    }
    layers.push(Node::Struct(name, fields));
    let mut layers = layers.into_iter();
    let first = layers.next().unwrap_or(Node::Struct(None, Vec::new()));
    Ok(Some(layers.fold(first, merge)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(name: &str) -> Result<String, String> {
        match name {
            "data/kits/patterns/block.ron" => Ok("// the block pattern\n(pattern: Masonry, bevel: 0.04, \
                 colours: (light: (1, 1, 1), chinking: (0, 0, 0)), \
                 dress: (roles: {Head: Profile(name: \"lintel\", extend: 0.2), Arris: Corners(set: \"quoin\")}), \
                 meets: Some((weight: 3, end: None, carries: Cap)))"
                .into()),
            "data/packs/clay.ron" => Ok("(bevel: 0.12, colours: (light: (0.5, 0.4, 0.3)))".into()),
            "data/packs/limed.ron" => Ok("(pack: \"clay\", bevel: 0.1)".into()),
            other => Err(format!("no {other}")),
        }
    }

    #[test]
    fn a_materials_own_textures_are_its_lines_and_packs_not_its_patterns() {
        let read = |name: &str| match name {
            "data/kits/patterns/block.ron" => Ok(
                "(pattern: Masonry, textures: {Dressed: (path: \"d\"), Floor: (path: \"f\")})"
                    .to_string(),
            ),
            "data/packs/earth.ron" => Ok("(textures: {Masonry: (path: \"e\")})".into()),
            "data/packs/limed.ron" => {
                Ok("(pack: \"earth\", textures: {Floor: (path: \"l\")})".into())
            }
            other => Err(format!("no {other}")),
        };
        let own = |text: &str| own_textures(text, &read).unwrap();
        assert_eq!(own("(pattern: \"block\", pack: \"earth\")"), ["Masonry"]);
        // Laid over another pack, it is given what that one gives.
        assert_eq!(
            own("(pattern: \"block\", pack: \"limed\", textures: {Hewn: (path: \"h\")})"),
            ["Masonry", "Floor", "Hewn"]
        );
    }

    #[test]
    fn a_material_is_its_pattern_then_its_pack_then_itself() {
        let text = "(id: 9, pattern: \"block\", pack: \"clay\", weight: 2, \
                    dress: (roles: {Head: Profile(extend: 0.3), Post: Pillar(width: 0.4)}))";
        let resolved = resolve(text, &files).unwrap();
        assert_eq!(
            parse(&resolved).unwrap(),
            parse(
                "(pattern: Masonry, bevel: 0.12, \
                 colours: (light: (0.5, 0.4, 0.3), chinking: (0, 0, 0)), \
                 dress: (roles: {Head: Profile(name: \"lintel\", extend: 0.3), Arris: Corners(set: \"quoin\"), Post: Pillar(width: 0.4)}), \
                 meets: Some((weight: 2, end: None, carries: Cap)), id: 9)"
            )
            .unwrap()
        );
    }

    #[test]
    fn a_variant_of_another_name_replaces_the_one_under_it() {
        let text = "(pattern: \"block\", dress: (roles: {Arris: Profile(name: \"edge\")}))";
        let resolved = parse(&resolve(text, &files).unwrap()).unwrap();
        let printed = print(&resolved);
        assert!(
            printed.contains("Arris: Profile(name: \"edge\")"),
            "{printed}"
        );
        assert!(!printed.contains("quoin"), "{printed}");
    }

    #[test]
    fn an_entry_of_none_takes_the_one_under_it_away() {
        let text = "(pattern: \"block\", dress: (roles: {Arris: None, Jamb: None}))";
        let printed = resolve(text, &files).unwrap();
        assert!(
            !printed.contains("Arris") && !printed.contains("Jamb"),
            "{printed}"
        );
        assert!(printed.contains("Head: Profile"), "{printed}");
    }

    #[test]
    fn a_pack_may_name_a_pack_under_it() {
        let printed = resolve("(pattern: \"block\", pack: \"limed\")", &files).unwrap();
        assert!(printed.contains("bevel: 0.1,"), "{printed}");
        assert!(printed.contains("light: (0.5, 0.4, 0.3)"), "{printed}");
    }

    #[test]
    fn a_whole_kit_is_given_back_as_it_is() {
        let text = "(id: 1, pattern: Logs)";
        assert_eq!(resolve(text, &files).unwrap(), text);
    }

    #[test]
    fn what_it_prints_reads_back_the_same() {
        let text = "(a: (1, 2), b: Some((x: \"q\\\"\")), c: [(\"Stone\", 1, 4)], d: {1: (t: Some(true))}, e: (0.5,), f: ())";
        let node = parse(text).unwrap();
        assert_eq!(parse(&print(&node)).unwrap(), node);
    }

    #[test]
    fn a_missing_pattern_names_its_file() {
        let why = resolve("(pattern: \"nothing\")", &files).unwrap_err();
        assert!(why.contains("nothing"), "{why}");
    }
}
