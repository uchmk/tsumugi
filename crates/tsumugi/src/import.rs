//! Colour schemes from other terminals, put in the `themes` folder as they
//! come (v1-scope 1k): Windows Terminal's (`*.json`: one scheme, a list of
//! them, or a whole `settings.json` with its `schemes`) and iTerm2's
//! (`*.itermcolors`). Each becomes the table a `themes/*.toml` gives, the
//! window's colours mixed from the scheme's and its sixteen kept as they are.

use std::collections::BTreeMap;

use eframe::egui::Color32;

use crate::theme::mix;

/// A scheme as read: its name, background, foreground, and sixteen.
#[derive(Clone, Debug, PartialEq)]
pub struct Scheme {
    pub name: String,
    pub background: Color32,
    pub foreground: Color32,
    pub ansi: [Color32; 16],
}

/// The table a theme file would have for `s`.
pub fn table(s: &Scheme) -> BTreeMap<String, toml::Value> {
    let hex = |c: Color32| toml::Value::String(format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b()));
    let (bg, fg) = (s.background, s.foreground);
    let light = luminance(bg) > 0.4;
    // The window's own parts a step darker than the panes, as the built-in
    // themes have them.
    let toward = Color32::BLACK;
    let a = &s.ansi;
    // The dim text: the bright black when it reads on the background, else
    // half way from the background to the text.
    let dim = if crate::theme::contrast(a[8], bg) >= 3.0 { a[8] } else { mix(bg, fg, 0.55) };
    let mut t = BTreeMap::new();
    t.insert("name".into(), toml::Value::String(s.name.clone()));
    t.insert("light".into(), toml::Value::Boolean(light));
    for (k, c) in [
        ("bg", mix(bg, toward, if light { 0.03 } else { 0.12 })),
        ("side", mix(bg, toward, if light { 0.06 } else { 0.2 })),
        // The panes are the scheme's own background.
        ("panel", bg),
        ("border", mix(bg, fg, 0.15)),
        ("fg", fg),
        ("dim", dim),
        ("wait", a[3]),
        ("run", a[6]),
        ("err", a[1]),
        ("done", a[2]),
        ("blue", a[4]),
        ("magenta", a[5]),
    ] {
        t.insert(k.into(), hex(c));
    }
    t.insert("ansi".into(), toml::Value::Array(a.iter().map(|c| hex(*c)).collect()));
    t
}

fn luminance(c: Color32) -> f32 {
    (0.2126 * f32::from(c.r()) + 0.7152 * f32::from(c.g()) + 0.0722 * f32::from(c.b())) / 255.0
}

fn hex(s: &str) -> Option<Color32> {
    let h = s.trim().strip_prefix('#')?;
    let v = u32::from_str_radix(h.get(..6)?, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// Windows Terminal's names for the sixteen, in order.
const WT_NAMES: [&str; 16] = [
    "black", "red", "green", "yellow", "blue", "purple", "cyan", "white", "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightPurple", "brightCyan", "brightWhite",
];

/// The schemes in a Windows Terminal file (`name` names one without a name).
pub fn windows_terminal(text: &str, name: &str) -> Result<Vec<Scheme>, String> {
    let v = Json::parse(text)?;
    let list: Vec<&Json> = match &v {
        Json::Object(_) if v.get("schemes").is_some() => v.get("schemes").and_then(Json::array).map(|a| a.iter().collect()).unwrap_or_default(),
        Json::Object(_) => vec![&v],
        Json::Array(a) => a.iter().collect(),
        _ => Vec::new(),
    };
    let mut out = Vec::new();
    for s in list {
        let color = |k: &str| s.get(k).and_then(Json::string).and_then(hex);
        let (Some(background), Some(foreground)) = (color("background"), color("foreground")) else { continue };
        let mut ansi = [Color32::BLACK; 16];
        for (k, n) in WT_NAMES.iter().enumerate() {
            ansi[k] = color(n).ok_or_else(|| format!("the scheme has no `{n}`"))?;
        }
        let name = s.get("name").and_then(Json::string).map_or_else(|| name.to_owned(), str::to_owned);
        out.push(Scheme { name, background, foreground, ansi });
    }
    if out.is_empty() {
        return Err("no colour scheme in it (one with background, foreground and the sixteen)".into());
    }
    Ok(out)
}

/// An iTerm2 `.itermcolors` file: a property list of colours, each a
/// dictionary of `Red`, `Green` and `Blue Component` from 0 to 1.
pub fn iterm(text: &str, name: &str) -> Result<Scheme, String> {
    let color = |key: &str| -> Option<Color32> {
        let at = text.find(&format!("<key>{key}</key>"))?;
        let rest = &text[at..];
        let dict = &rest[rest.find("<dict>")?..rest.find("</dict>")?];
        let part = |c: &str| -> Option<u8> {
            let at = dict.find(&format!("<key>{c} Component</key>"))?;
            let r = &dict[at..];
            let v = &r[r.find("<real>")? + 6..r.find("</real>")?];
            Some((v.trim().parse::<f32>().ok()?.clamp(0.0, 1.0) * 255.0).round() as u8)
        };
        Some(Color32::from_rgb(part("Red")?, part("Green")?, part("Blue")?))
    };
    let mut ansi = [Color32::BLACK; 16];
    for (k, c) in ansi.iter_mut().enumerate() {
        *c = color(&format!("Ansi {k} Color")).ok_or_else(|| format!("no `Ansi {k} Color`"))?;
    }
    let background = color("Background Color").ok_or("no `Background Color`")?;
    let foreground = color("Foreground Color").ok_or("no `Foreground Color`")?;
    Ok(Scheme { name: name.to_owned(), background, foreground, ansi })
}

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

    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(f) => f.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn string(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    fn array(&self) -> Option<&Vec<Json>> {
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

    const CAMPBELL: &str = r##"{
        // Windows Terminal's own
        "name": "Campbell",
        "background": "#0C0C0C", "foreground": "#CCCCCC",
        "black": "#0C0C0C", "red": "#C50F1F", "green": "#13A10E", "yellow": "#C19C00",
        "blue": "#0037DA", "purple": "#881798", "cyan": "#3A96DD", "white": "#CCCCCC",
        "brightBlack": "#767676", "brightRed": "#E74856", "brightGreen": "#16C60C", "brightYellow": "#F9F1A5",
        "brightBlue": "#3B78FF", "brightPurple": "#B4009E", "brightCyan": "#61D6D6", "brightWhite": "#F2F2F2",
        "cursorColor": "#FFFFFF",
    }"##;

    #[test]
    fn a_windows_terminal_scheme_is_read() {
        let s = &windows_terminal(CAMPBELL, "file").unwrap()[0];
        assert_eq!(s.name, "Campbell");
        assert_eq!(s.ansi[1], Color32::from_rgb(0xc5, 0x0f, 0x1f));
        assert_eq!(s.ansi[15], Color32::from_rgb(0xf2, 0xf2, 0xf2));
        let t = table(s);
        assert_eq!(t["panel"].as_str(), Some("#0c0c0c"), "the panes on the scheme's background");
        assert_eq!(t["err"].as_str(), Some("#c50f1f"));
        assert_eq!(t["light"].as_bool(), Some(false));
        assert_eq!(t["ansi"].as_array().map(Vec::len), Some(16));
        // A settings.json: its schemes, comments and trailing commas aside.
        let settings = format!("{{ /* mine */ \"profiles\": {{}}, \"schemes\": [ {CAMPBELL}, ], }}");
        assert_eq!(windows_terminal(&settings, "x").unwrap().len(), 1);
        assert!(windows_terminal("{\"profiles\": {}}", "x").is_err());
    }

    #[test]
    fn an_iterm_scheme_is_read() {
        let mut text = String::from("<plist><dict>");
        let entry = |key: &str, r: f32, g: f32, b: f32| format!("<key>{key}</key><dict><key>Blue Component</key><real>{b}</real><key>Color Space</key><string>sRGB</string><key>Green Component</key><real>{g}</real><key>Red Component</key><real>{r}</real></dict>");
        for k in 0..16 {
            text.push_str(&entry(&format!("Ansi {k} Color"), k as f32 / 15.0, 0.0, 0.0));
        }
        text.push_str(&entry("Background Color", 1.0, 1.0, 1.0));
        text.push_str(&entry("Foreground Color", 0.0, 0.0, 0.0));
        text.push_str("</dict></plist>");
        let s = iterm(&text, "Paper").unwrap();
        assert_eq!((s.background, s.foreground), (Color32::WHITE, Color32::BLACK));
        assert_eq!(s.ansi[15], Color32::from_rgb(255, 0, 0));
        assert_eq!(table(&s)["light"].as_bool(), Some(true));
        assert!(iterm("<plist/>", "x").is_err());
    }

    #[test]
    fn json_reads_what_a_settings_file_has() {
        let v = Json::parse(r#"[1, -2.5e1, "a\"bé", true, null, {"k": []}]"#).unwrap();
        assert_eq!(v, Json::Array(vec![Json::Number(1.0), Json::Number(-25.0), Json::String("a\"bé".into()), Json::Bool(true), Json::Null, Json::Object(vec![("k".into(), Json::Array(vec![]))])]));
        assert!(Json::parse("{\"a\" 1}").is_err());
        assert!(Json::parse("[1] 2").is_err());
    }
}
