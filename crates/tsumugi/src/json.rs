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
    Bool(bool),
    Null,
}

impl Json {
    pub fn parse(text: &str) -> Result<Json, String> {
        let mut p = Parser { s: text.as_bytes(), at: 0 };
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
            Json::Number(n) => out.push_str(&n.to_string()),
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

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
}

impl Parser<'_> {
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

    fn value(&mut self) -> Result<Json, String> {
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
                let n = std::str::from_utf8(&self.s[start..self.at]).ok().and_then(|t| t.parse().ok());
                n.map(Json::Number).ok_or_else(|| format!("a bad number at byte {start}"))
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
                        b'u' => {
                            let code = std::str::from_utf8(self.s.get(self.at..self.at + 4).unwrap_or_default()).ok().and_then(|h| u32::from_str_radix(h, 16).ok());
                            self.at += 4;
                            let c = code.and_then(char::from_u32).unwrap_or('\u{fffd}');
                            out.extend(c.to_string().bytes());
                        }
                        other => out.push(other),
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
