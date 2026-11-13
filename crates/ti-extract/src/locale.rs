//! Game-compatible line parsing and one merged dictionary per language (port of `locale.py`).
//!
//! Base files load by name with first-key-wins diagnostics. All DLC language
//! files then load by relative path and replace earlier values. Mods are excluded.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::Result;
use crate::game::{file_name, files_recursive, read, rel_posix, subdirs};
use crate::jsonc::{Value, object, py_isspace, universal_newlines};
use crate::output::sha256_bytes;

#[derive(Debug, Clone, Default)]
pub struct LocaleSet {
    /// lang -> key -> text
    pub entries: BTreeMap<String, BTreeMap<String, String>>,
    /// relative source path -> sha256
    pub sources: BTreeMap<String, String>,
    pub issues: Vec<Value>,
}

pub fn parse_line(line: &str) -> Option<(String, String)> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.chars().all(py_isspace) {
        return None;
    }
    let separator = line.find('=').filter(|&i| i >= 1)?;
    let (key, value) = (&line[..separator], &line[separator + 1..]);
    let value = value
        .replace('\t', "")
        .replace("<br/>", "\n")
        .replace("<h>", "<color=#EC9933>")
        .replace("</h>", "</color>")
        .replace("<rcol>", "<line-height=0.01%>\n<align=\"right\">")
        .replace("</rcol>", "</align></line-height>");
    let value = value.split("//").next().unwrap_or_default().trim_end_matches(' ').replace("<sp>", " ");
    if value.is_empty() {
        return None;
    }
    Some((key.to_owned(), value.replace("<skip/>", "")))
}

/// Load `text` of one file into `entries`, preserving base keys across files.
pub fn parse_text(text: &str, issues: &mut Vec<Value>, file: &str, lang: &str, layer: &str, entries: &mut BTreeMap<String, String>) {
    // File.ReadLines splits CR/LF, not Unicode separators or vertical tabs.
    for (lineno, line) in text.split('\n').enumerate() {
        let Some((key, value)) = parse_line(line) else { continue };
        if layer == "base" && entries.contains_key(&key) {
            issues.push(object([
                ("kind", Value::from("locale_duplicate_key")),
                ("lang", lang.into()),
                ("layer", layer.into()),
                ("file", file.into()),
                ("line", (lineno + 1).into()),
                ("key", key.into()),
            ]));
            continue;
        }
        entries.insert(key, value);
    }
}

/// Python's `read_text(encoding="utf-8-sig", errors="replace")`.
fn decode_lossy(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    universal_newlines(&String::from_utf8_lossy(bytes))
}

pub fn parse_file(path: &Path, issues: &mut Vec<Value>, lang: &str, layer: &str, entries: &mut BTreeMap<String, String>) -> Result<String> {
    let bytes = read(path)?;
    parse_text(&decode_lossy(&bytes), issues, &file_name(path).unwrap_or_default(), lang, layer, entries);
    Ok(sha256_bytes(&bytes))
}

/// The files of each language, `(lang, [(layer, path)])`, in load order.
fn plan(game_root: &Path) -> Vec<(String, Vec<(&'static str, PathBuf)>)> {
    let loc_root = game_root.join("TerraInvicta_Data/StreamingAssets/Localization");
    let dlc_root = game_root.join("DLC_Content");
    subdirs(&loc_root)
        .into_iter()
        .map(|directory| {
            let lang = file_name(&directory).unwrap_or_default();
            let mut base: Vec<PathBuf> = std::fs::read_dir(&directory)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && !file_name(p).unwrap_or_default().ends_with("meta"))
                .collect();
            base.sort();
            let mut dlc = Vec::new();
            files_recursive(&dlc_root, &format!(".{lang}"), &mut dlc);
            dlc.sort_by_cached_key(|p| rel_posix(p, &dlc_root));
            let files = base.into_iter().map(|p| ("base", p)).chain(dlc.into_iter().map(|p| ("dlc", p))).collect();
            (lang, files)
        })
        .collect()
}

/// Load every language. `on_file(done, total)` runs after each file; an error from it aborts.
pub fn load_all_with(game_root: &Path, on_file: &mut dyn FnMut(u64, u64) -> Result<()>) -> Result<LocaleSet> {
    let plan = plan(game_root);
    let total = plan.iter().map(|(_, files)| files.len()).sum::<usize>() as u64;
    let mut ls = LocaleSet::default();
    let mut done = 0;
    for (lang, files) in plan {
        let mut merged = BTreeMap::new();
        for (layer, path) in files {
            let digest = parse_file(&path, &mut ls.issues, &lang, layer, &mut merged)?;
            ls.sources.insert(rel_posix(&path, game_root), digest);
            done += 1;
            on_file(done, total)?;
        }
        ls.entries.insert(lang, merged);
    }
    Ok(ls)
}

pub fn load_all(game_root: &Path) -> Result<LocaleSet> {
    load_all_with(game_root, &mut |_, _| Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::sha256_file;
    use crate::tests::{TempDir, write};

    #[test]
    fn lines() {
        let cases: &[(&str, Option<(&str, &str)>)] = &[
            ("A.displayName.X=Hello", Some(("A.displayName.X", "Hello"))),
            (
                "A.description.X=see http://a.b/c\t\t\t// translator note",
                Some(("A.description.X", "see http:")),
            ),
            ("A.b.X=1 = 2\r\n", Some(("A.b.X", "1 = 2"))),
            ("// full comment", None),
            ("   ", None),
            ("", None),
            ("A.displayName.NoValue", None),
            ("=NoKey", None),
            (" Key =  leading and trailing   ", Some((" Key ", "  leading and trailing"))),
            ("A=one\ttwo\t // comment", Some(("A", "onetwo"))),
            ("A=one//comment//more", Some(("A", "one"))),
            ("A=<br/>next<br/>", Some(("A", "\nnext\n"))),
            ("A=<h>title</h>", Some(("A", "<color=#EC9933>title</color>"))),
            (
                "A=<rcol>right</rcol>",
                Some(("A", "<line-height=0.01%>\n<align=\"right\">right</align></line-height>")),
            ),
            ("A=<sp>hello<sp>   ", Some(("A", " hello "))),
            ("A=value\x0b\u{a0}   ", Some(("A", "value\x0b\u{a0}"))),
            ("A=", None),
            ("A=\t  //comment", None),
            ("A=<skip/>", Some(("A", ""))),
            ("A= <skip/>   ", Some(("A", " "))),
            ("A=a<skip/>b", Some(("A", "ab"))),
            ("A=literal \\n \\\" {0} <i>text</i>", Some(("A", "literal \\n \\\" {0} <i>text</i>"))),
        ];
        for (line, expected) in cases {
            assert_eq!(parse_line(line), expected.map(|(k, v)| (k.to_owned(), v.to_owned())), "{line:?}");
        }
    }

    #[test]
    fn file_line_endings_and_other_whitespace() {
        let dir = TempDir::new();
        let path = dir.path().join("test.en");
        std::fs::write(&path, "\u{feff}a=first\r\nb=second\rc=value\x0b\u{2028}\n").unwrap();
        let mut entries = BTreeMap::new();
        parse_file(&path, &mut Vec::new(), "en", "base", &mut entries).unwrap();
        let expected: BTreeMap<String, String> = [("a", "first"), ("b", "second"), ("c", "value\x0b\u{2028}")]
            .map(|(k, v)| (k.into(), v.into()))
            .into();
        assert_eq!(entries, expected);
    }

    #[test]
    fn merge_base_first_wins_then_all_dlc_override() {
        let dir = TempDir::new();
        let root = dir.path();
        let loc = "TerraInvicta_Data/StreamingAssets/Localization/en";
        // Create in reverse lexical order to exercise sorting.
        let b = write(root, &format!("{loc}/b.en"), "shared=second\nbase=second\nsame=same\n");
        let a = write(
            root,
            &format!("{loc}/a.anything"),
            "\u{feff}shared=first\nbase=first\nsame=same\nwithin=first\nwithin=last\nhidden=<skip/>\nhidden=shown\n",
        );
        write(root, &format!("{loc}/ignoredmeta"), "ignored=meta\n");
        write(root, &format!("{loc}/sub/nested.en"), "ignored=nested\n");
        let z = write(root, "DLC_Content/Z/odd/folder/en/z.en", "shared=dlc-z\nshared=last\nhidden=\n");
        let d = write(root, "DLC_Content/A/outside-localization/a.en", "shared=dlc-a\nnew=dlc\n");
        write(root, "Mods/Enabled/mod/a.en", "shared=mod\n");
        let deu = write(root, "TerraInvicta_Data/StreamingAssets/Localization/deu/a.deu", "base=Deutsch\n");
        let ls = load_all(root).unwrap();
        let map = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<BTreeMap<_, _>>();
        let expected: BTreeMap<String, BTreeMap<String, String>> = [
            (
                "en".to_owned(),
                map(&[
                    ("shared", "last"),
                    ("base", "first"),
                    ("same", "same"),
                    ("within", "first"),
                    ("hidden", ""),
                    ("new", "dlc"),
                ]),
            ),
            ("deu".to_owned(), map(&[("base", "Deutsch")])),
        ]
        .into();
        assert_eq!(ls.entries, expected);
        assert_eq!(ls.issues.len(), 5);
        assert!(ls.issues.iter().all(|i| i.get("kind") == Some(&"locale_duplicate_key".into())));
        let mut keys: Vec<_> = ls.issues.iter().map(|i| i.get("key").unwrap().as_str().unwrap()).collect();
        keys.sort();
        assert_eq!(keys, ["base", "hidden", "same", "shared", "within"]);
        let expected: BTreeMap<String, String> = [a, b, z, d, deu].iter().map(|p| (rel_posix(p, root), sha256_file(p).unwrap())).collect();
        assert_eq!(ls.sources, expected);
    }
}
