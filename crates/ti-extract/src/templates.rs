//! Reading template files of every layer into plain records.

use std::path::{Path, PathBuf};

use crate::Result;
use crate::game::{Layer, files_in, files_recursive, read, rel_posix};
use crate::jsonc::{self, Context, Value, object};
use crate::output::sha256_bytes;

#[derive(Debug, Clone)]
pub struct TemplateFile {
    pub layer: String,
    /// e.g. `TITechTemplate`.
    pub name: String,
    /// The first file of this name in the layer.
    pub source: PathBuf,
    pub sha256: String,
    pub records: Vec<Value>,
}

#[derive(Debug, Clone, Default)]
pub struct TemplateSet {
    /// Per layer (in load order), the template files by name (in first-seen order).
    pub files: Vec<(String, Vec<TemplateFile>)>,
    pub issues: Vec<Value>,
    /// Every file read (also those failing to parse), with its sha256.
    pub sources: Vec<(PathBuf, String)>,
}

impl TemplateSet {
    pub fn layer(&self, id: &str) -> Option<&[TemplateFile]> {
        self.files.iter().find(|(l, _)| l == id).map(|(_, files)| &files[..])
    }

    pub fn file(&self, layer: &str, name: &str) -> Option<&TemplateFile> {
        self.layer(layer)?.iter().find(|f| f.name == name)
    }

    /// Template names over all layers, sorted.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.files.iter().flat_map(|(_, files)| files.iter().map(|f| f.name.clone())).collect();
        names.sort();
        names.dedup();
        names
    }

    /// `(layer, record)` for one template over all layers.
    pub fn records<'a>(&'a self, template: &'a str) -> impl Iterator<Item = (&'a str, &'a Value)> + 'a {
        self.files
            .iter()
            .filter_map(move |(layer, files)| files.iter().find(|f| f.name == template).map(|f| (layer.as_str(), f)))
            .flat_map(|(layer, f)| f.records.iter().map(move |r| (layer, r)))
    }
}

fn issue(issues: &mut Vec<Value>, kind: &str, details: Vec<(&str, Value)>) {
    issues.push(object(std::iter::once(("kind", kind.into())).chain(details)));
}

/// The template files of a layer, in load order.
pub fn layer_files(layer: &Layer) -> Vec<PathBuf> {
    // DLC directories are already individually discovered, including nested ones.
    let mut paths = if layer.kind == "base" {
        let mut paths = Vec::new();
        files_recursive(&layer.templates_dir, ".json", &mut paths);
        paths
    } else {
        files_in(&layer.templates_dir, ".json")
    };
    paths.sort_by_cached_key(|p| rel_posix(p, &layer.templates_dir));
    paths
}

fn file_stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Read one file of `layer` into `by_name`.
fn load_file(layer: &Layer, path: &Path, by_name: &mut Vec<TemplateFile>, out: &mut TemplateSet) -> Result<()> {
    let bytes = read(path)?;
    let sha256 = sha256_bytes(&bytes);
    out.sources.push((path.to_path_buf(), sha256.clone()));
    let name = file_stem(path);
    let lid = layer.id.as_str();
    let data = match jsonc::load_bytes(
        &bytes,
        &mut out.issues,
        Context {
            template: Some(&name),
            layer: Some(lid),
        },
    ) {
        Ok(data) => data,
        Err(e) => {
            issue(
                &mut out.issues,
                "parse_error",
                vec![("layer", lid.into()), ("template", name.as_str().into()), ("error", e.0.into())],
            );
            return Ok(());
        }
    };
    let data = match data {
        Value::Array(items) => items,
        other => vec![other],
    };
    let mut records = Vec::with_capacity(data.len());
    let mut seen = std::collections::HashSet::new();
    for (i, rec) in data.into_iter().enumerate() {
        if !matches!(rec, Value::Object(_)) {
            issue(
                &mut out.issues,
                "non_object_record",
                vec![("layer", lid.into()), ("template", name.as_str().into()), ("index", i.into())],
            );
            continue;
        }
        match rec.get("dataName").and_then(Value::as_str).filter(|dn| !dn.is_empty()) {
            None => issue(
                &mut out.issues,
                "missing_data_name",
                vec![("layer", lid.into()), ("template", name.as_str().into()), ("index", i.into())],
            ),
            Some(dn) if seen.contains(dn) => {
                issue(
                    &mut out.issues,
                    "duplicate_data_name",
                    vec![("layer", lid.into()), ("template", name.as_str().into()), ("dataName", dn.into())],
                );
            }
            Some(dn) => {
                seen.insert(dn.to_owned());
            }
        }
        records.push(rec);
    }
    match by_name.iter_mut().find(|f| f.name == name) {
        Some(f) => f.records.extend(records),
        None => by_name.push(TemplateFile {
            layer: layer.id.clone(),
            name,
            source: path.to_path_buf(),
            sha256,
            records,
        }),
    }
    Ok(())
}

/// Load every layer. `on_file(done, total)` runs after each file; an error from it aborts.
pub fn load_all_with(layers: &[Layer], on_file: &mut dyn FnMut(u64, u64) -> Result<()>) -> Result<TemplateSet> {
    let files: Vec<Vec<PathBuf>> = layers.iter().map(layer_files).collect();
    let total = files.iter().map(Vec::len).sum::<usize>() as u64;
    let mut ts = TemplateSet::default();
    let mut done = 0;
    for (layer, paths) in layers.iter().zip(&files) {
        let mut by_name = Vec::new();
        for path in paths {
            load_file(layer, path, &mut by_name, &mut ts)?;
            done += 1;
            on_file(done, total)?;
        }
        ts.files.push((layer.id.clone(), by_name));
    }
    for layer in layers.iter().skip(1) {
        let names: Vec<String> = ts.layer(&layer.id).unwrap_or_default().iter().map(|f| f.name.clone()).collect();
        for name in names {
            if ts.file(crate::game::BASE_LAYER, &name).is_none() {
                issue(
                    &mut ts.issues,
                    "dlc_only_template",
                    vec![("layer", layer.id.as_str().into()), ("template", name.into())],
                );
            }
        }
    }
    Ok(ts)
}

pub fn load_all(layers: &[Layer]) -> Result<TemplateSet> {
    load_all_with(layers, &mut |_, _| Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonc::parse;
    use crate::output::sha256_file;
    use crate::tests::{TempDir, write};

    #[test]
    fn duplicate_keys_reject_file_with_record_context() {
        let dir = TempDir::new();
        write(
            dir.path(),
            "TITestTemplate.json",
            "[\n  {\"value\": 1, \"value\": 2, \"nested\": {\"x\": 3, \"x\": 4}, \"dataName\": \"a\"},\n  {\"dataName\": \"a\", \"value\": 5}\n]",
        );
        let layer = Layer {
            id: "base".into(),
            kind: "base".into(),
            templates_dir: dir.path().to_path_buf(),
            dlc: None,
            folder: None,
        };
        let ts = load_all(&[layer]).unwrap();
        assert!(ts.file("base", "TITestTemplate").is_none());
        assert!(ts.issues.iter().any(|i| i.get("kind") == Some(&"parse_error".into())));
        let duplicates: Vec<_> = ts
            .issues
            .iter()
            .filter(|i| i.get("kind") == Some(&"duplicate_json_key".into()))
            .cloned()
            .collect();
        assert_eq!(
            Value::Array(duplicates),
            parse(
                r#"[
                {"kind": "duplicate_json_key", "severity": "error", "template": "TITestTemplate", "layer": "base", "key": "value", "path": "$[0]", "dataName": "a"},
                {"kind": "duplicate_json_key", "severity": "error", "template": "TITestTemplate", "layer": "base", "key": "x", "path": "$[0].nested", "dataName": "a"}
            ]"#
            )
            .unwrap()
        );
    }

    #[test]
    fn discover_recursive_directory_layers_in_load_order() {
        let dir = TempDir::new();
        let root = dir.path();
        write(root, "TerraInvicta_Data/StreamingAssets/Templates/TIBase.json", "[]");
        for folder in ["Z/Extras", "A/Scenario/Templates", "A/Scenario/Templates/Nested"] {
            write(root, &format!("DLC_Content/{folder}/TITest.json"), "[]");
        }
        write(root, "DLC_Content/A/Scenario/Templates/Another.json", "[]");
        let install = crate::discover(root).unwrap();
        let layers: Vec<_> = install
            .layers
            .iter()
            .map(|l| (l.id.as_str(), l.kind.as_str(), l.dlc.as_deref(), l.folder.as_deref()))
            .collect();
        assert_eq!(
            layers,
            [
                ("base", "base", None, None),
                ("dlc.a.scenario.templates", "dlc", Some("A"), Some("A/Scenario/Templates")),
                ("dlc.a.scenario.templates.nested", "dlc", Some("A"), Some("A/Scenario/Templates/Nested")),
                ("dlc.z.extras", "dlc", Some("Z"), Some("Z/Extras")),
            ]
        );
        let ts = load_all(&install.layers).unwrap();
        let mut names: Vec<_> = ts.layer("dlc.a.scenario.templates").unwrap().iter().map(|f| f.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["Another", "TITest"]);
        assert_eq!(install.version, None);
    }

    #[test]
    fn recursive_base_repeated_basenames_and_nested_dlc() {
        let dir = TempDir::new();
        let root = dir.path();
        let prefix = "TerraInvicta_Data/StreamingAssets/Templates";
        let paths: Vec<_> = [("z/", "last"), ("", "first"), ("a/", "middle")]
            .into_iter()
            .map(|(folder, name)| write(root, &format!("{prefix}/{folder}Test.json"), &format!(r#"[{{"dataName":"{name}"}}]"#)))
            .collect();
        write(root, "DLC_Content/A/Templates/Test.json", r#"[{"dataName":"dlc"}]"#);
        write(root, "DLC_Content/A/Templates/Nested/Test.json", r#"[{"dataName":"nested"}]"#);
        let ts = load_all(&crate::discover(root).unwrap().layers).unwrap();
        let names = |it: Vec<&Value>| {
            it.into_iter()
                .map(|r| r.get("dataName").unwrap().as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(ts.file("base", "Test").unwrap().records.iter().collect()), ["first", "middle", "last"]);
        assert_eq!(
            names(ts.records("Test").map(|(_, r)| r).collect()),
            ["first", "middle", "last", "dlc", "nested"]
        );
        assert_eq!(ts.sources.len(), 5);
        for p in &paths {
            assert_eq!(ts.sources.iter().find(|(s, _)| s == p).unwrap().1, sha256_file(p).unwrap());
        }
    }
}
