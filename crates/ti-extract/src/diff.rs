//! Changelog between two extracted builds, keyed by layer/template/dataName (port of `diff.py`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::Result;
use crate::game::{file_name, files_in, read, subdirs};
use crate::jsonc::{self, Value};
use crate::output::scalar;

fn load_json(path: &Path) -> Result<Value> {
    let text = String::from_utf8_lossy(&read(path)?).into_owned();
    jsonc::parse(&text).map_err(|e| crate::Error::Other(format!("{}: {e}", path.display())))
}

fn stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// A dataName or tag as text (they are strings in practice).
fn text(v: &Value) -> String {
    v.as_str().map_or_else(|| dumps(v), str::to_owned)
}

fn dumps(v: &Value) -> String {
    let mut s = String::new();
    match v {
        Value::Array(_) | Value::Object(_) => s = crate::output::dumps(v, true).unwrap_or_default().trim_end().to_owned(),
        _ => {
            if scalar(&mut s, v).is_err() {
                s = crate::output::float_repr(if let Value::Float(f) = v { *f } else { 0.0 });
            }
        }
    }
    s
}

fn tags(record: &Value) -> Vec<String> {
    let mut tags: Vec<String> = record
        .get("scenarioTags")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .map(text)
        .collect();
    tags.sort();
    tags
}

/// `record.get(field)`, with a missing field read as `null` like Python's `dict.get`.
fn field<'a>(record: &'a Value, f: &str) -> &'a Value {
    record.get(f).unwrap_or(&Value::Null)
}

type ByName = BTreeMap<String, Vec<Value>>;

/// Per template file: dataName -> every record with that name (scenario variants), in file order.
fn load_templates(build: &Path) -> Result<BTreeMap<(String, String), ByName>> {
    let mut out = BTreeMap::new();
    for layer_dir in subdirs(&build.join("templates")) {
        let layer = file_name(&layer_dir).unwrap_or_default();
        for f in files_in(&layer_dir, ".json") {
            let mut by_name = ByName::new();
            if let Value::Array(records) = load_json(&f)? {
                for (i, record) in records.into_iter().enumerate() {
                    let dn = record.get("dataName").map_or_else(|| format!("#{i}"), text);
                    by_name.entry(dn).or_default().push(record);
                }
            }
            out.insert((layer.clone(), stem(&f)), by_name);
        }
    }
    Ok(out)
}

/// Pair the variants of one dataName: `(added, removed, changed)`.
///
/// Identical records cancel out first (so reordering is not a change), then records with
/// the same tag set pair up; a single leftover on each side is one record whose tags changed.
fn match_variants(old: &[Value], new: &[Value]) -> (Vec<Value>, Vec<Value>, Vec<(Value, Value)>) {
    let mut old = old.to_vec();
    let mut new = new.to_vec();
    old.retain(|record| match new.iter().position(|r| r == record) {
        Some(i) => {
            new.remove(i);
            false
        }
        None => true,
    });
    let mut changed = Vec::new();
    let mut rest = Vec::new();
    for record in old {
        match new.iter().position(|r| tags(r) == tags(&record)) {
            Some(i) => changed.push((record, new.remove(i))),
            None => rest.push(record),
        }
    }
    if rest.len() == 1 && new.len() == 1 {
        changed.push((rest.pop().unwrap(), new.pop().unwrap()));
    }
    (new, rest, changed)
}

fn load_locale(build: &Path, lang: &str) -> Result<BTreeMap<String, String>> {
    let mut merged = BTreeMap::new();
    for f in files_in(&build.join("locale").join(lang), ".json") {
        if let Value::Object(pairs) = load_json(&f)? {
            let stem = stem(&f);
            for (k, v) in pairs {
                merged.insert(format!("{stem}:{k}"), text(&v));
            }
        }
    }
    Ok(merged)
}

fn build_id(build: &Path) -> Result<String> {
    let manifest = load_json(&build.join("manifest.json"))?;
    Ok(manifest.get("game").and_then(|g| g.get("buildId")).map(text).unwrap_or_default())
}

/// Markdown changelog from build dir `old` to `new`, with localization changes in `lang`.
pub fn diff_builds(old: &Path, new: &Path, lang: &str) -> Result<String> {
    let (a, b) = (load_templates(old)?, load_templates(new)?);
    let mut lines = vec![format!("# Changes: build {} → {}", build_id(old)?, build_id(new)?), String::new()];

    let empty = ByName::new();
    let keys: BTreeSet<_> = a.keys().chain(b.keys()).collect();
    for key in keys {
        let (layer, template) = key;
        let (ra, rb) = (a.get(key).unwrap_or(&empty), b.get(key).unwrap_or(&empty));
        let (mut added, mut removed, mut changed) = (Vec::new(), Vec::new(), Vec::new());
        let names: BTreeSet<&String> = ra.keys().chain(rb.keys()).collect();
        for dn in names {
            let (olds, news) = (ra.get(dn).map_or(&[][..], Vec::as_slice), rb.get(dn).map_or(&[][..], Vec::as_slice));
            // Variants are named by their tags only where a dataName has several records.
            let several = olds.len().max(news.len()) > 1;
            let label = |record: &Value| {
                let tags = tags(record);
                let tags = if tags.is_empty() { "none".to_owned() } else { tags.join(", ") };
                if several { format!("`{dn}` [tags: {tags}]") } else { format!("`{dn}`") }
            };
            let (more, fewer, mut pairs) = match_variants(olds, news);
            let mut more: Vec<String> = more.iter().map(label).collect();
            more.sort();
            added.extend(more);
            let mut fewer: Vec<String> = fewer.iter().map(label).collect();
            fewer.sort();
            removed.extend(fewer);
            pairs.sort_by_cached_key(|(x, _)| tags(x));
            for (x, y) in pairs {
                let fields: BTreeSet<&String> = x
                    .as_object()
                    .unwrap_or_default()
                    .iter()
                    .chain(y.as_object().unwrap_or_default())
                    .map(|(k, _)| k)
                    .collect();
                let fields: Vec<&String> = fields.into_iter().filter(|f| field(&x, f) != field(&y, f)).collect();
                let details: Vec<String> = fields
                    .iter()
                    .map(|f| {
                        let (xv, yv) = (field(&x, f), field(&y, f));
                        if xv.is_container() || yv.is_container() {
                            format!("`{f}`")
                        } else {
                            format!("`{f}`: {} → {}", dumps(xv), dumps(yv))
                        }
                    })
                    .collect();
                changed.push(format!("- changed {}: {}", label(&x), details.join(", ")));
            }
        }
        if added.is_empty() && removed.is_empty() && changed.is_empty() {
            continue;
        }
        lines.push(format!("## {template} ({layer})"));
        lines.extend(added.iter().map(|n| format!("- added {n}")));
        lines.extend(removed.iter().map(|n| format!("- removed {n}")));
        lines.extend(changed);
        lines.push(String::new());
    }

    let (la, lb) = (load_locale(old, lang)?, load_locale(new, lang)?);
    let keys: BTreeSet<&String> = la.keys().chain(lb.keys()).collect();
    let text_changes: Vec<&String> = keys.into_iter().filter(|k| la.get(*k) != lb.get(*k)).collect();
    if !text_changes.is_empty() {
        lines.push(format!("## Localization ({lang}): {} keys added/removed/changed", text_changes.len()));
        lines.extend(text_changes.iter().take(500).map(|k| format!("- `{k}`")));
        if text_changes.len() > 500 {
            lines.push(format!("- … {} more", text_changes.len() - 500));
        }
        lines.push(String::new());
    }
    if lines.len() == 2 {
        lines.push("No data changes.".into());
    }
    Ok(lines.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{TempDir, write};

    fn build(dir: &TempDir, name: &str, records: &str) -> std::path::PathBuf {
        write(dir.path(), &format!("{name}/templates/base/Test.json"), records);
        write(dir.path(), &format!("{name}/manifest.json"), &format!(r#"{{"game": {{"buildId": "{name}"}}}}"#));
        dir.path().join(name)
    }

    fn diff(old: &Path, new: &Path) -> String {
        diff_builds(old, new, "en").unwrap()
    }

    #[test]
    fn changed_non_last_variant() {
        let dir = TempDir::new();
        let old = build(
            &dir,
            "old",
            r#"[{"dataName": "Foo", "scenarioTags": ["B", "A"], "value": 1}, {"dataName": "Foo", "scenarioTags": ["C"], "value": 3}]"#,
        );
        let new = build(
            &dir,
            "new",
            r#"[{"dataName": "Foo", "scenarioTags": ["B", "A"], "value": 2}, {"dataName": "Foo", "scenarioTags": ["C"], "value": 3}]"#,
        );
        assert!(diff(&old, &new).contains("- changed `Foo` [tags: A, B]: `value`: 1 → 2"));
    }

    #[test]
    fn added_removed_variants() {
        let dir = TempDir::new();
        let old = build(
            &dir,
            "old",
            r#"[{"dataName": "Foo", "scenarioTags": ["A"]}, {"dataName": "Foo", "scenarioTags": ["B"]}]"#,
        );
        let new = build(
            &dir,
            "new",
            r#"[{"dataName": "Foo", "scenarioTags": ["A"]}, {"dataName": "Foo", "scenarioTags": ["C"]}, {"dataName": "Foo", "scenarioTags": ["D"]}]"#,
        );
        let text = diff(&old, &new);
        assert!(text.contains("- added `Foo` [tags: C]") && text.contains("- added `Foo` [tags: D]"));
        assert!(text.contains("- removed `Foo` [tags: B]"));
        assert!(!text.contains("- changed"));
    }

    #[test]
    fn tag_edit_of_a_sole_record_is_a_change() {
        let dir = TempDir::new();
        let old = build(&dir, "old", r#"[{"dataName": "Foo", "scenarioTags": ["A"]}]"#);
        let new = build(&dir, "new", r#"[{"dataName": "Foo", "scenarioTags": ["A", "B"]}]"#);
        assert!(diff(&old, &new).contains("- changed `Foo`: `scenarioTags`\n"));
    }

    #[test]
    fn reordered_and_removed_duplicates() {
        let dir = TempDir::new();
        let (one, two) = (
            r#"{"dataName": "Foo", "scenarioTags": ["A"], "value": 1}"#,
            r#"{"dataName": "Foo", "scenarioTags": ["A"], "value": 2}"#,
        );
        let old = build(&dir, "old", &format!("[{one}, {two}]"));
        assert!(diff(&old, &build(&dir, "reordered", &format!("[{two}, {one}]"))).ends_with("No data changes.\n"));
        let text = diff(&old, &build(&dir, "new", &format!("[{two}]")));
        assert!(text.contains("- removed `Foo` [tags: A]") && !text.contains("- changed"));
    }

    #[test]
    fn single_record_output_unchanged() {
        let dir = TempDir::new();
        let old = build(&dir, "old", r#"[{"dataName": "Foo", "value": 1}, {"dataName": "Removed"}]"#);
        let new = build(&dir, "new", r#"[{"dataName": "Foo", "value": 2}, {"dataName": "Added"}]"#);
        assert_eq!(
            diff(&old, &new),
            "# Changes: build old → new\n\n## Test (base)\n- added `Added`\n- removed `Removed`\n- changed `Foo`: `value`: 1 → 2\n\n"
        );
    }

    #[test]
    fn duplicate_tag_identity_preserves_each_occurrence() {
        let dir = TempDir::new();
        let one = r#"{"dataName": "Foo", "scenarioTags": ["A"], "value": 1}"#;
        let old = build(&dir, "old", &format!("[{one}, {one}]"));
        let new = build(&dir, "new", &format!(r#"[{{"dataName": "Foo", "scenarioTags": ["A"], "value": 2}}, {one}]"#));
        assert_eq!(diff(&old, &new).matches("- changed `Foo` [tags: A]: `value`: 1 → 2").count(), 1);
    }

    #[test]
    fn tag_order_does_not_change_variant_identity() {
        let dir = TempDir::new();
        let old = build(&dir, "old", r#"[{"dataName": "Foo", "scenarioTags": ["B", "A"]}]"#);
        let new = build(&dir, "new", r#"[{"dataName": "Foo", "scenarioTags": ["A", "B"]}]"#);
        let text = diff(&old, &new);
        assert!(text.contains("- changed `Foo`: `scenarioTags`"));
        assert!(!text.contains("- added") && !text.contains("- removed"));
    }
}
