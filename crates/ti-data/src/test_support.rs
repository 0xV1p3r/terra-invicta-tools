//! In-memory datasets for unit tests.

use std::collections::HashMap;

use serde_json::{Value, json};

use crate::dataset::{Dataset, EntityId};
use crate::error::{Error, Result};
use crate::source::DataSource;

struct MemSource(HashMap<String, Vec<u8>>);

impl DataSource for MemSource {
    fn read(&self, rel: &str) -> Result<Vec<u8>> {
        self.0.get(rel).cloned().ok_or_else(|| Error::Io {
            path: rel.into(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, rel.to_string()),
        })
    }

    fn list(&self, dir: &str) -> Result<Vec<String>> {
        let prefix = format!("{dir}/");
        let mut out: Vec<String> = self
            .0
            .keys()
            .filter(|k| k.strip_prefix(&prefix).is_some_and(|rest| !rest.contains('/')))
            .cloned()
            .collect();
        out.sort();
        Ok(out)
    }
}

/// Scenario meta template with the given tags (listed in the menu, in argument order).
pub fn scenario(name: &str, priority: i64, tags: &[&str], postfix: Option<&str>) -> Value {
    json!({
        "dataName": name,
        "newCampaignOptionCategory": "Scenario",
        "isNewCampaignOption": true,
        "listPriority": priority,
        "scenarioTags": tags,
        "scenarioLocalizationPostfix": postfix,
        "templateNames": [],
    })
}

/// A one-layer (base game) dataset. `templates` are loaded in the given order;
/// `strings` is the English localization table.
pub fn dataset(templates: &[(&str, Vec<Value>)], strings: Value) -> Dataset {
    try_dataset(&[("base", templates)], strings).expect("test dataset loads")
}

pub type TestLayer<'a> = (&'a str, &'a [(&'a str, Vec<Value>)]);

/// A dataset with one layer per `(id, templates)` (the first is the base game), or the load error.
pub fn try_dataset(layers: &[TestLayer<'_>], strings: Value) -> Result<Dataset> {
    let mut files = HashMap::new();
    let mut put = |path: String, v: &Value| files.insert(path, serde_json::to_vec(v).unwrap());
    let manifest_layers: Vec<Value> = layers
        .iter()
        .enumerate()
        .map(|(i, (layer, templates))| {
            let names: Vec<&str> = templates.iter().map(|(t, _)| *t).collect();
            let kind = if i == 0 { "base" } else { "dlc" };
            json!({"id": layer, "kind": kind, "dlc": null, "folder": null, "templates": names})
        })
        .collect();
    put(
        "manifest.json".into(),
        &json!({
            "formatVersion": crate::manifest::FORMAT_VERSION,
            "extractor": {"name": "test", "version": "0"},
            "extractedAt": "",
            "game": {"appId": "0", "buildId": "test", "lastUpdated": null},
            "layers": manifest_layers,
            "languages": ["en"],
            "hasAssets": false,
        }),
    );
    put("schema.json".into(), &json!({"refFields": []}));
    put("locale/en/strings.json".into(), &strings);
    for (layer, templates) in layers {
        for (t, records) in *templates {
            put(format!("templates/{layer}/{t}.json"), &Value::Array(records.clone()));
        }
    }
    Dataset::from_source(Box::new(MemSource(files)))
}

pub fn id(ds: &Dataset, template: &str, name: &str) -> EntityId {
    ds.find(template, name).unwrap_or_else(|| panic!("no {template} {name}"))
}
