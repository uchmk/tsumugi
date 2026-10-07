//! Just enough JSON for the files tsumugi reads and writes beside other
//! programs: Windows Terminal's colour schemes and Claude Code's settings.
//! No crate for it (CLAUDE.md: new dependencies wait on the owner).

/// Just enough JSON for a settings file: objects, arrays, strings, numbers,
/// true, false and null, with the comments and trailing commas Windows
/// Terminal allows.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Object(Vec<(String, Json)>),
    Array(Vec<Json>),
    String(String),
    Number(f64),
    /// A number an `f64` would change (an integer past 2^53, `1e400`),
    /// kept as it was written so it is written back the same.
    RawNumber(String),
    Bool(bool),
    Null,
}

impl Json {
    pub fn parse(text: &str) -> Result<Json, String> {
        // A UTF-8 byte order mark, as some editors save one, is no value.
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut p = Parser { s: text.as_bytes(), at: 0, depth: 0 };
        let v = p.value()?;
        p.space();
        if p.at < p.s.len() {
            return Err(format!("something after the end at byte {}", p.at));
        }
        Ok(v)
    }

    /// Written out, two spaces a level, as Claude Code writes its settings.
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out.push('\n');
        out
    }

    fn write(&self, out: &mut String, depth: usize) {
        let pad = |n: usize| "  ".repeat(n);
        match self {
            Json::Object(f) if f.is_empty() => out.push_str("{}"),
            Json::Array(a) if a.is_empty() => out.push_str("[]"),
            Json::Object(f) => {
                out.push_str("{\n");
                for (k, (key, v)) in f.iter().enumerate() {
                    out.push_str(&pad(depth + 1));
                    Json::String(key.clone()).write(out, depth + 1);
                    out.push_str(": ");
                    v.write(out, depth + 1);
                    out.push_str(if k + 1 < f.len() { ",\n" } else { "\n" });
                }
                out.push_str(&pad(depth));
                out.push('}');
            }
            Json::Array(a) => {
                out.push_str("[\n");
                for (k, v) in a.iter().enumerate() {
                    out.push_str(&pad(depth + 1));
                    v.write(out, depth + 1);
                    out.push_str(if k + 1 < a.len() { ",\n" } else { "\n" });
                }
                out.push_str(&pad(depth));
                out.push(']');
            }
            Json::String(s) => {
                out.push('"');
                for c in s.chars() {
                    match c {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            Json::Number(n) if n.fract() == 0.0 && n.abs() < 1e15 => out.push_str(&format!("{}", *n as i64)),
            Json::Number(n) if n.is_finite() => out.push_str(&n.to_string()),
            // JSON has no infinity: `null`, not `inf`, which would not read.
            Json::Number(_) => out.push_str("null"),
            Json::RawNumber(t) => out.push_str(t),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Null => out.push_str("null"),
        }
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(f) => f.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn string(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn array(&self) -> Option<&Vec<Json>> {
        match self {
            Json::Array(a) => Some(a),
            _ => None,
        }
    }
}

/// How deep arrays and objects may go.
const MAX_DEPTH: usize = 256;

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
    depth: usize,
}

impl Parser<'_> {
    /// Four hex digits at the cursor, as a number; the cursor past them.
    fn hex4(&mut self) -> Option<u32> {
        let code = std::str::from_utf8(self.s.get(self.at..self.at + 4)?).ok().and_then(|h| u32::from_str_radix(h, 16).ok());
        self.at += 4;
        code
    }

    fn err<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("{what} at byte {}", self.at))
    }

    /// Spaces and comments.
    fn space(&mut self) {
        loop {
            while self.at < self.s.len() && self.s[self.at].is_ascii_whitespace() {
                self.at += 1;
            }
            if self.s[self.at..].starts_with(b"//") {
                while self.at < self.s.len() && self.s[self.at] != b'\n' {
                    self.at += 1;
                }
            } else if self.s[self.at..].starts_with(b"/*") {
                self.at = self.s[self.at + 2..].windows(2).position(|w| w == b"*/").map_or(self.s.len(), |p| self.at + 2 + p + 2);
            } else {
                return;
            }
        }
    }

    fn eat(&mut self, b: u8) -> bool {
        self.space();
        if self.s.get(self.at) == Some(&b) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    /// A value, no deeper than `MAX_DEPTH` (a file of a million `[` would
    /// otherwise run the stack out).
    fn value(&mut self) -> Result<Json, String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return self.err("nested too deep");
        }
        let v = self.value_here();
        self.depth -= 1;
        v
    }

    fn value_here(&mut self) -> Result<Json, String> {
        self.space();
        match self.s.get(self.at) {
            Some(b'{') => {
                self.at += 1;
                let mut fields = Vec::new();
                loop {
                    if self.eat(b'}') {
                        return Ok(Json::Object(fields));
                    }
                    self.space();
                    let Json::String(k) = self.string()? else { unreachable!() };
                    if !self.eat(b':') {
                        return self.err("a `:` expected");
                    }
                    fields.push((k, self.value()?));
                    if !self.eat(b',') && !self.eat(b'}') {
                        return self.err("a `,` or `}` expected");
                    } else if self.s.get(self.at - 1) == Some(&b'}') {
                        return Ok(Json::Object(fields));
                    }
                }
            }
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                loop {
                    if self.eat(b']') {
                        return Ok(Json::Array(items));
                    }
                    items.push(self.value()?);
                    if !self.eat(b',') && !self.eat(b']') {
                        return self.err("a `,` or `]` expected");
                    } else if self.s.get(self.at - 1) == Some(&b']') {
                        return Ok(Json::Array(items));
                    }
                }
            }
            Some(b'"') => self.string(),
            Some(b't') if self.s[self.at..].starts_with(b"true") => {
                self.at += 4;
                Ok(Json::Bool(true))
            }
            Some(b'f') if self.s[self.at..].starts_with(b"false") => {
                self.at += 5;
                Ok(Json::Bool(false))
            }
            Some(b'n') if self.s[self.at..].starts_with(b"null") => {
                self.at += 4;
                Ok(Json::Null)
            }
            Some(c) if c.is_ascii_digit() || *c == b'-' => {
                let start = self.at;
                while self.at < self.s.len() && matches!(self.s[self.at], b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E') {
                    self.at += 1;
                }
                let text = std::str::from_utf8(&self.s[start..self.at]).unwrap_or_default();
                let n: f64 = text.parse().map_err(|_| format!("a bad number at byte {start}"))?;
                let integer = !text.contains(['.', 'e', 'E']);
                if !n.is_finite() || (integer && n.abs() > 9_007_199_254_740_992.0) {
                    Ok(Json::RawNumber(text.to_owned()))
                } else {
                    Ok(Json::Number(n))
                }
            }
            _ => self.err("a value expected"),
        }
    }

    fn string(&mut self) -> Result<Json, String> {
        if self.s.get(self.at) != Some(&b'"') {
            return self.err("a string expected");
        }
        self.at += 1;
        let mut out = Vec::new();
        while let Some(&b) = self.s.get(self.at) {
            self.at += 1;
            match b {
                b'"' => return Ok(Json::String(String::from_utf8_lossy(&out).into_owned())),
                b'\\' => {
                    let Some(&e) = self.s.get(self.at) else { break };
                    self.at += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0c),
                        b'"' | b'\\' | b'/' => out.push(e),
                        b'u' => {
                            let mut code = self.hex4();
                            // A pair (an emoji written escaped) is one
                            // character, not two replacement marks.
                            if let Some(high @ 0xD800..=0xDBFF) = code {
                                if self.s.get(self.at..self.at + 2) == Some(b"\\u") {
                                    self.at += 2;
                                    code = self.hex4().filter(|low| (0xDC00..=0xDFFF).contains(low)).map(|low| 0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00));
                                }
                            }
                            let c = code.and_then(char::from_u32).unwrap_or('\u{fffd}');
                            out.extend(c.to_string().bytes());
                        }
                        _ => return self.err("an unknown escape"),
                    }
                }
                other => out.push(other),
            }
        }
        self.err("a string not closed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the source review found lost on a round trip (2026-10-07).
    #[test]
    fn awkward_json_comes_back_as_it_was() {
        let v = Json::parse("\u{feff}{\"e\": \"\\uD83D\\uDE00 \\b\\f\\/\", \"big\": 12345678901234567890, \"huge\": 1e400}").unwrap();
        assert_eq!(v.get("e"), Some(&Json::String("\u{1F600} \u{8}\u{c}/".into())));
        let back = v.pretty();
        assert!(back.contains("12345678901234567890") && back.contains("1e400"), "{back}");
        assert_eq!(Json::parse(&back).unwrap(), v, "the same again");
        assert!(Json::parse("\"\\q\"").is_err(), "an unknown escape");
        assert!(Json::parse(&"[".repeat(100_000)).is_err(), "too deep: an error, not a stack overflow");
        assert_eq!(Json::Number(f64::INFINITY).pretty(), "null\n");
    }

    #[test]
    fn json_reads_what_a_settings_file_has() {
        let v = Json::parse(r#"[1, -2.5e1, "a\"bé", true, null, {"k": []}]"#).unwrap();
        assert_eq!(v, Json::Array(vec![Json::Number(1.0), Json::Number(-25.0), Json::String("a\"bé".into()), Json::Bool(true), Json::Null, Json::Object(vec![("k".into(), Json::Array(vec![]))])]));
        assert!(Json::parse("{\"a\" 1}").is_err());
        assert!(Json::parse("[1] 2").is_err());
        let text = "{\n  \"a\": [\n    1,\n    \"q\\\"\\\\\"\n  ],\n  \"b\": {},\n  \"c\": 2.5\n}\n";
        assert_eq!(Json::parse(text).unwrap().pretty(), text, "written back as read");
    }
}
