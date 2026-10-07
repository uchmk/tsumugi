//! Prompts kept to send again (the input box's **Prompts**): a name and the
//! text, in `prompts.toml` beside the settings. `{folder}`, `{project}` and
//! `{branch}` in a prompt become each receiving session's when it is sent.

use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prompt {
    pub name: String,
    pub text: String,
}

/// The file beside the settings.
pub fn file() -> Option<PathBuf> {
    tsumugi_mux::settings::default_path().map(|p| p.with_file_name("prompts.toml"))
}

/// The prompts written as TOML, `[[prompt]]` each.
pub fn to_toml(list: &[Prompt]) -> String {
    let items: Vec<toml::Value> = list
        .iter()
        .map(|p| {
            let mut t = toml::Table::new();
            t.insert("name".into(), toml::Value::String(p.name.clone()));
            t.insert("text".into(), toml::Value::String(p.text.clone()));
            toml::Value::Table(t)
        })
        .collect();
    let mut top = toml::Table::new();
    top.insert("prompt".into(), toml::Value::Array(items));
    format!("# The input box's saved prompts. {{folder}}, {{project}} and {{branch}} become the session's.\n{}", toml::to_string(&top).unwrap_or_default())
}

/// The prompts read back; what does not read is left out.
pub fn from_toml(text: &str) -> Vec<Prompt> {
    let Ok(top) = text.parse::<toml::Table>() else { return Vec::new() };
    let Some(items) = top.get("prompt").and_then(toml::Value::as_array) else { return Vec::new() };
    items
        .iter()
        .filter_map(|v| {
            let t = v.as_table()?;
            let s = |k: &str| t.get(k).and_then(toml::Value::as_str).map(str::to_owned);
            Some(Prompt { name: s("name")?, text: s("text")? })
        })
        .collect()
}

pub fn load() -> Vec<Prompt> {
    file().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| from_toml(&t)).unwrap_or_default()
}

/// Write them on a thread.
pub fn save(list: Vec<Prompt>) {
    let _ = std::thread::Builder::new().name("prompts".into()).spawn(move || {
        let Some(p) = file() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, to_toml(&list));
    });
}

/// `{folder}`, `{project}` and `{branch}` in `text` as the session's.
pub fn fill(text: &str, folder: &std::path::Path, project: &std::path::Path, branch: &str) -> String {
    let name = project.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    text.replace("{folder}", &folder.display().to_string()).replace("{project}", &name).replace("{branch}", branch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_come_back_and_fill_in() {
        let list = vec![Prompt { name: "Tests".into(), text: "Run the tests in {project} on {branch}\nand fix \"what\" fails".into() }, Prompt { name: "Review".into(), text: "Review {folder}".into() }];
        assert_eq!(from_toml(&to_toml(&list)), list);
        assert!(from_toml("not = [toml").is_empty());
        let p = std::path::Path::new("/home/u/dev/filer");
        assert_eq!(fill(&list[0].text, p, p, "fix/x"), "Run the tests in filer on fix/x\nand fix \"what\" fails");
        assert_eq!(fill("Review {folder}", p, p, ""), "Review /home/u/dev/filer");
    }
}
