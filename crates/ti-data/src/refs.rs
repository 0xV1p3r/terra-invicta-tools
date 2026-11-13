//! Generic reference graph: every string value at a field path that the
//! extractor identified as a reference field (`schema.json` → `refFields`) and
//! that names an existing record of one of the field's target templates.

use std::sync::Arc;

use serde_json::Value;

use crate::dataset::{Dataset, Reference};
use crate::manifest::{RefField, Schema};

/// Visit every value at `path` (`a`, `a[]`, `a[].b`, `a[][]`) inside `value`.
pub fn visit_path<'v>(value: &'v Value, path: &str, f: &mut dyn FnMut(&'v Value)) {
    fn rec<'v>(value: &'v Value, segments: &[&str], f: &mut dyn FnMut(&'v Value)) {
        let Some((first, rest)) = segments.split_first() else {
            f(value);
            return;
        };
        let name = first.trim_end_matches("[]");
        let depth = (first.len() - name.len()) / 2;
        if let Some(v) = value.get(name) {
            flatten(v, depth, rest, f);
        }
    }
    fn flatten<'v>(value: &'v Value, depth: usize, rest: &[&str], f: &mut dyn FnMut(&'v Value)) {
        if depth == 0 {
            rec(value, rest, f);
        } else if let Value::Array(items) = value {
            for item in items {
                flatten(item, depth - 1, rest, f);
            }
        }
    }
    let segments: Vec<&str> = path.split('.').collect();
    rec(value, &segments, f);
}

/// Reference fields confirmed from the game code (`docs/research/game-semantics.md` §6),
/// added even if the schema heuristic misses them.
const CURATED: &[(&str, &str, &[&str])] = &[
    ("TITechTemplate", "prereqs[]", &["TITechTemplate", "TIProjectTemplate"]),
    ("TITechTemplate", "effects[]", &["TIEffectTemplate"]),
    ("TIProjectTemplate", "prereqs[]", &["TITechTemplate", "TIProjectTemplate"]),
    ("TIProjectTemplate", "altPrereq0", &["TITechTemplate", "TIProjectTemplate"]),
    ("TIProjectTemplate", "altPrereq1", &["TITechTemplate", "TIProjectTemplate"]),
    ("TIProjectTemplate", "effects[]", &["TIEffectTemplate"]),
    ("TIProjectTemplate", "factionPrereq[]", &["TIFactionTemplate"]),
    ("TIProjectTemplate", "factionAlways", &["TIFactionTemplate"]),
    ("TIProjectTemplate", "requiredObjectiveName", &["TIObjectiveTemplate"]),
    ("TIProjectTemplate", "altRequiredObjectiveName", &["TIObjectiveTemplate"]),
    ("TIProjectTemplate", "orgGranted", &["TIOrgTemplate"]),
    ("TIHabModuleTemplate", "requiredProjectName", &["TIProjectTemplate"]),
    ("TIHabModuleTemplate", "unlocksProjectName", &["TIProjectTemplate"]),
    ("TIHabModuleTemplate", "upgradesFromName", &["TIHabModuleTemplate"]),
    ("TIOrgTemplate", "requiredTechName", &["TITechTemplate", "TIProjectTemplate"]),
    ("TICodexEntryTemplate", "unlockTech", &["TITechTemplate", "TIProjectTemplate"]),
    ("TIMissionTemplate", "knowledgeProject", &["TITechTemplate", "TIProjectTemplate"]),
    ("TIBilateralTemplate", "projectUnlockName", &["TIProjectTemplate"]),
    ("TITraitTemplate", "projectDataName", &["TIProjectTemplate"]),
    ("TIObjectiveTemplate", "targetProjectTemplateName", &["TIProjectTemplate"]),
    ("TIObjectiveTemplate", "targetTechTemplateName", &["TITechTemplate"]),
    ("TIEffectTemplate", "initialFactionsStr[]", &["TIFactionTemplate"]),
];

pub(crate) fn build(ds: &mut Dataset, schema: &Schema) {
    let mut fields: Vec<RefField> = schema.ref_fields.clone();
    for (template, path, targets) in CURATED {
        let targets: Vec<String> = targets.iter().map(|t| t.to_string()).collect();
        match fields.iter_mut().find(|f| f.template == *template && f.path == *path) {
            Some(f) => f.targets = targets,
            None => fields.push(RefField {
                template: template.to_string(),
                path: path.to_string(),
                targets,
            }),
        }
    }
    // Ship parts: `requiredProjectName` is their only reference.
    for part in crate::model::PartKind::ALL {
        if !fields.iter().any(|f| f.template == part.template() && f.path == "requiredProjectName") {
            fields.push(RefField {
                template: part.template().into(),
                path: "requiredProjectName".into(),
                targets: vec!["TIProjectTemplate".into()],
            });
        }
    }
    let mut refs: Vec<Reference> = Vec::new();
    for field in &fields {
        let path: Arc<str> = Arc::from(field.path.as_str());
        let targets: Vec<Arc<str>> = field.targets.iter().filter_map(|t| ds.intern_template(t)).collect();
        for &from in ds.of_template(&field.template) {
            let entity = ds.entity(from);
            for (vi, variant) in entity.variants.iter().enumerate() {
                visit_path(&variant.record, &field.path, &mut |v| {
                    let Some(name) = v.as_str() else { return };
                    for t in &targets {
                        if let Some(to) = ds.find(t, name).filter(|to| *to != from) {
                            refs.push(Reference {
                                from,
                                variant: vi as u16,
                                path: path.clone(),
                                to,
                            });
                        }
                    }
                });
            }
        }
    }
    for (i, r) in refs.iter().enumerate() {
        ds.refs_from.entry(r.from).or_default().push(i);
        ds.refs_to.entry(r.to).or_default().push(i);
    }
    ds.references = refs;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn collect(v: &Value, path: &str) -> Vec<Value> {
        let mut out = vec![];
        visit_path(v, path, &mut |x| out.push(x.clone()));
        out
    }

    #[test]
    fn paths() {
        let v = json!({"a": "x", "b": ["y", "z"], "c": [{"d": ["p"]}, {"d": ["q", "r"]}], "e": [["m"], ["n"]]});
        assert_eq!(collect(&v, "a"), vec![json!("x")]);
        assert_eq!(collect(&v, "b[]"), vec![json!("y"), json!("z")]);
        assert_eq!(collect(&v, "c[].d[]"), vec![json!("p"), json!("q"), json!("r")]);
        assert_eq!(collect(&v, "e[][]"), vec![json!("m"), json!("n")]);
        assert!(collect(&v, "missing[]").is_empty());
    }
}
