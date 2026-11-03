//! A reader of the part of RON (Rusty Object Notation, the game's data files' format) that
//! xtask's own data uses, so xtask stays without dependencies: structs `( key: value, … )`,
//! maps `{ "key": value, … }`, lists `[ … ]`, strings, numbers, `true` and `false`, and `//`
//! comments. A struct's name before its `(`, an enum's variant, `Some(…)` and tuples are not
//! read: the data that needs them is the game's, read by `ron` and serde there.

/// One value of the file.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    /// A struct's fields or a map's entries, in the file's order.
    Fields(Vec<(String, Value)>),
    List(Vec<Value>),
    Text(String),
    Number(f64),
    Bool(bool),
}

impl Value {
    /// The field or entry named, if this has one.
    pub(crate) fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Fields(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub(crate) fn number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub(crate) fn fields(&self) -> &[(String, Value)] {
        match self {
            Value::Fields(fields) => fields,
            _ => &[],
        }
    }
}

/// Reads a whole file's one value; the error names the line and what was wanted there.
pub(crate) fn parse(text: &str) -> Result<Value, String> {
    let mut reader = Reader { text, at: 0 };
    let value = reader.value()?;
    reader.blank();
    if reader.at < text.len() {
        return Err(reader.wanted("the end of the file"));
    }
    Ok(value)
}

struct Reader<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Reader<'a> {
    fn rest(&self) -> &'a str {
        &self.text[self.at..]
    }

    /// Skips spaces, newlines and `//` comments.
    fn blank(&mut self) {
        loop {
            let rest = self.rest();
            let trimmed = rest.trim_start();
            self.at += rest.len() - trimmed.len();
            if trimmed.starts_with("//") {
                self.at += trimmed.find('\n').unwrap_or(trimmed.len());
            } else {
                return;
            }
        }
    }

    fn wanted(&self, what: &str) -> String {
        let line = self.text[..self.at].matches('\n').count() + 1;
        let found: String = self.rest().chars().take(20).collect();
        format!("line {line}: {what} wanted, found `{found}`")
    }

    fn eat(&mut self, token: char) -> bool {
        self.blank();
        let ate = self.rest().starts_with(token);
        if ate {
            self.at += token.len_utf8();
        }
        ate
    }

    fn value(&mut self) -> Result<Value, String> {
        self.blank();
        let rest = self.rest();
        if rest.starts_with('(') {
            self.fields('(', ')', |r| r.word())
        } else if rest.starts_with('{') {
            self.fields('{', '}', |r| r.text())
        } else if self.eat('[') {
            let mut list = Vec::new();
            while !self.eat(']') {
                list.push(self.value()?);
                if !self.eat(',') && !self.rest().starts_with(']') {
                    return Err(self.wanted("`,` or `]`"));
                }
            }
            Ok(Value::List(list))
        } else if rest.starts_with('"') {
            self.text().map(Value::Text)
        } else if rest.starts_with("true") || rest.starts_with("false") {
            let word = self.word()?;
            Ok(Value::Bool(word == "true"))
        } else {
            let end = rest
                .find(|c: char| !(c.is_ascii_digit() || "+-.eE_".contains(c)))
                .unwrap_or(rest.len());
            let number = rest[..end].replace('_', "").parse::<f64>();
            match number {
                Ok(n) if end > 0 => {
                    self.at += end;
                    Ok(Value::Number(n))
                }
                _ => {
                    Err(self.wanted("a value (a number, string, `true`, `false`, `(`, `{` or `[`)"))
                }
            }
        }
    }

    /// A struct's `(key: value, …)` or a map's `{"key": value, …}`, by how a key is read.
    fn fields(
        &mut self,
        open: char,
        close: char,
        key: fn(&mut Self) -> Result<String, String>,
    ) -> Result<Value, String> {
        self.eat(open);
        let mut fields = Vec::new();
        while !self.eat(close) {
            let name = key(self)?;
            if !self.eat(':') {
                return Err(self.wanted(&format!("`:` after `{name}`")));
            }
            fields.push((name, self.value()?));
            if !self.eat(',') && !self.rest().starts_with(close) {
                return Err(self.wanted(&format!("`,` or `{close}`")));
            }
        }
        Ok(Value::Fields(fields))
    }

    fn word(&mut self) -> Result<String, String> {
        self.blank();
        let rest = self.rest();
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        if end == 0 {
            return Err(self.wanted("a field's name"));
        }
        self.at += end;
        Ok(rest[..end].to_string())
    }

    fn text(&mut self) -> Result<String, String> {
        self.blank();
        let Some(body) = self.rest().strip_prefix('"') else {
            return Err(self.wanted("a string in quotes"));
        };
        let Some(end) = body.find('"') else {
            return Err(self.wanted("a closing `\"`"));
        };
        let text = body[..end].to_string();
        self.at += end + 2;
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_structs_maps_lists_and_comments() {
        let value = parse(
            "// a comment\n(\n  a: 1.5, // more\n  b: { \"x-y\": -2, \"z\": 3e1 },\n  c: [true, false,],\n  d: \"t\",\n)",
        )
        .unwrap();
        assert_eq!(value.get("a").and_then(Value::number), Some(1.5));
        let b = value.get("b").unwrap();
        assert_eq!(b.get("x-y").and_then(Value::number), Some(-2.0));
        assert_eq!(b.get("z").and_then(Value::number), Some(30.0));
        assert_eq!(
            value.get("c"),
            Some(&Value::List(vec![Value::Bool(true), Value::Bool(false)]))
        );
        assert_eq!(value.get("d"), Some(&Value::Text("t".into())));
    }

    #[test]
    fn an_error_names_the_line_and_what_was_wanted() {
        let error = parse("(\n  a: 1,\n  b 2,\n)").unwrap_err();
        assert!(error.starts_with("line 3: `:` after `b` wanted"), "{error}");
        assert!(
            parse("(a: 1) x")
                .unwrap_err()
                .contains("the end of the file")
        );
    }
}
