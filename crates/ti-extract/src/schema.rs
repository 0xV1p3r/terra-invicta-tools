//! Data-driven schema discovery.
//!
//! For every template, records each field path (`a`, `a[]`, `a[].b`), the JSON
//! types seen and, for string values, which templates own a record with that
//! `dataName`. Consumers use this to build a generic reference graph without
//! hard-coding every field, and to notice schema drift between game builds.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::jsonc::{Value, object};
use crate::output::{round4, scalar};
use crate::templates::TemplateSet;

/// A field counts as a reference when at least this share of its string values resolve.
pub const REF_THRESHOLD: f64 = 0.5;
pub const MAX_EXAMPLES: usize = 3;
/// Fields that hold display text or the record's own key, never references.
pub const NON_REF_FIELDS: &[&str] = &["dataName", "friendlyName", "displayName", "notes", "Notes"];

fn json_type(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Int(_) | Value::BigInt(_) => "int",
        Value::Float(_) => "float",
        Value::Str(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Every `(path, value)` of `value`, pre-order; the root has path `""`.
pub fn walk<'a>(value: &'a Value, f: &mut dyn FnMut(&str, &'a Value)) {
    fn go<'a>(value: &'a Value, path: &mut String, f: &mut dyn FnMut(&str, &'a Value)) {
        f(path, value);
        let len = path.len();
        match value {
            Value::Object(pairs) => {
                for (k, v) in pairs {
                    if len > 0 {
                        path.push('.');
                    }
                    path.push_str(k);
                    go(v, path, f);
                    path.truncate(len);
                }
            }
            Value::Array(items) => {
                for v in items {
                    path.push_str("[]");
                    go(v, path, f);
                    path.truncate(len);
                }
            }
            _ => {}
        }
    }
    go(value, &mut String::new(), f);
}

/// Every string value in `value`, recursively.
pub fn strings<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
    match value {
        Value::Str(s) => out.push(s),
        Value::Array(items) => items.iter().for_each(|v| strings(v, out)),
        Value::Object(pairs) => pairs.iter().for_each(|(_, v)| strings(v, out)),
        _ => {}
    }
}

pub fn data_name_index(ts: &TemplateSet) -> HashMap<String, BTreeSet<String>> {
    let mut index: HashMap<String, BTreeSet<String>> = HashMap::new();
    for name in ts.names() {
        for (_, rec) in ts.records(&name) {
            if let Some(dn) = rec.get("dataName").and_then(Value::as_str) {
                index.entry(dn.to_owned()).or_default().insert(name.clone());
            }
        }
    }
    index
}

/// Python equality classes of scalars, so `examples` dedupes like `v not in examples`.
#[derive(PartialEq, Eq, Hash)]
enum ExampleKey {
    Null,
    Int(i128),
    Big(String),
    Float(u64),
    Str(String),
}

fn example_key(v: &Value) -> Option<ExampleKey> {
    Some(match v {
        Value::Null => ExampleKey::Null,
        Value::Bool(b) => ExampleKey::Int(*b as i128),
        Value::Int(i) => ExampleKey::Int(*i),
        Value::BigInt(s) => ExampleKey::Big(s.clone()),
        Value::Float(f) if f.is_nan() => return None, // NaN != NaN
        Value::Float(f) if f.fract() == 0.0 && f.abs() < 1e38 => ExampleKey::Int(*f as i128),
        Value::Float(f) => ExampleKey::Float(f.to_bits()),
        Value::Str(s) => ExampleKey::Str(s.clone()),
        Value::Array(_) | Value::Object(_) => return None,
    })
}

#[derive(Default)]
struct FieldStats {
    types: BTreeMap<&'static str, u64>,
    present: u64,
    last_record: usize,
    strings: u64,
    refs: BTreeMap<String, u64>,
    unresolved: BTreeMap<String, u64>,
    examples: Vec<Value>,
    example_keys: HashSet<ExampleKey>,
}

fn dumps_scalar(v: &Value) -> String {
    let mut s = String::new();
    // Records are canonical (finite) by now; a non-finite float sorts as its repr.
    if scalar(&mut s, v).is_err() {
        s = crate::output::float_repr(match v {
            Value::Float(f) => *f,
            _ => 0.0,
        });
    }
    s
}

pub fn discover(ts: &TemplateSet) -> Value {
    let index = data_name_index(ts);
    let mut templates = Vec::new();
    let mut ref_fields = Vec::new();
    let mut dangling = Vec::new();

    for name in ts.names() {
        let mut counts: Vec<(String, Value)> = ts
            .files
            .iter()
            .filter_map(|(layer, files)| files.iter().find(|f| f.name == name).map(|f| (layer.clone(), f.records.len().into())))
            .collect();
        counts.sort_by(|a, b| a.0.cmp(&b.0));
        let mut fields: HashMap<String, FieldStats> = HashMap::new();

        for (n, (_, rec)) in ts.records(&name).enumerate() {
            walk(rec, &mut |path, v| {
                if path.is_empty() {
                    return;
                }
                let stats = match fields.get_mut(path) {
                    Some(stats) => stats,
                    None => fields.entry(path.to_owned()).or_default(),
                };
                *stats.types.entry(json_type(v)).or_default() += 1;
                if stats.present == 0 || stats.last_record != n {
                    stats.present += 1;
                    stats.last_record = n;
                }
                if !v.is_container() {
                    match example_key(v) {
                        Some(key) => {
                            if stats.example_keys.insert(key) {
                                stats.examples.push(v.clone());
                            }
                        }
                        None => stats.examples.push(v.clone()),
                    }
                }
                let leaf = path.rsplit('.').next().unwrap_or(path);
                let leaf = leaf.strip_suffix("[]").unwrap_or(leaf);
                if let Value::Str(s) = v
                    && !s.is_empty()
                    && !NON_REF_FIELDS.contains(&leaf)
                {
                    stats.strings += 1;
                    match index.get(s) {
                        Some(targets) => {
                            for t in targets {
                                *stats.refs.entry(t.clone()).or_default() += 1;
                            }
                        }
                        None => *stats.unresolved.entry(s.clone()).or_default() += 1,
                    }
                }
            });
        }

        let mut paths: Vec<_> = fields.into_iter().collect();
        paths.sort_by(|a, b| a.0.cmp(&b.0));
        let mut out_fields = Vec::with_capacity(paths.len());
        for (path, mut stats) in paths {
            let mut examples: Vec<(&'static str, String, Value)> = stats.examples.drain(..).map(|v| (v.py_type_name(), dumps_scalar(&v), v)).collect();
            examples.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
            let examples: Vec<Value> = examples.into_iter().take(MAX_EXAMPLES).map(|(_, _, v)| v).collect();
            let mut entry = vec![
                ("present", stats.present.into()),
                ("types", object(stats.types.iter().map(|(t, n)| (*t, (*n).into())))),
                ("examples", Value::Array(examples)),
            ];
            if !stats.refs.is_empty() {
                entry.push(("refs", object(stats.refs.iter().map(|(t, n)| (t.clone(), (*n).into())))));
                let resolved = stats.strings - stats.unresolved.values().sum::<u64>();
                let ratio = resolved as f64 / stats.strings as f64;
                let rounded = round4(ratio);
                entry.push(("refRatio", rounded.into()));
                if ratio >= REF_THRESHOLD {
                    let mut targets: Vec<(&String, &u64)> = stats.refs.iter().collect();
                    targets.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
                    ref_fields.push(object([
                        ("template", Value::from(&name)),
                        ("path", path.as_str().into()),
                        ("targets", Value::Array(targets.into_iter().map(|(t, _)| t.into()).collect())),
                        ("ratio", rounded.into()),
                    ]));
                    if !stats.unresolved.is_empty() {
                        dangling.push(object([
                            ("template", Value::from(&name)),
                            ("path", path.as_str().into()),
                            ("values", object(stats.unresolved.iter().map(|(v, n)| (v.clone(), (*n).into())))),
                        ]));
                    }
                }
            }
            out_fields.push((path, object(entry)));
        }
        templates.push((name, object([("records", Value::Object(counts)), ("fields", Value::Object(out_fields))])));
    }

    let mut ambiguous: Vec<(&String, &BTreeSet<String>)> = index.iter().filter(|(_, t)| t.len() > 1).collect();
    ambiguous.sort_by(|a, b| a.0.cmp(b.0));
    object([
        ("templates", Value::Object(templates)),
        ("refFields", Value::Array(ref_fields)),
        (
            "ambiguousDataNames",
            object(
                ambiguous
                    .into_iter()
                    .map(|(dn, t)| (dn.clone(), Value::Array(t.iter().map(Into::into).collect()))),
            ),
        ),
        ("dangling", Value::Array(dangling)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::dumps;
    use crate::templates::TemplateFile;

    #[test]
    fn independent_of_input_order() {
        let discover_with = |reverse: bool| {
            let mut ts = TemplateSet::default();
            let layers = if reverse { ["z", "base"] } else { ["base", "z"] };
            for layer in layers {
                let names = if reverse { ["Z", "A"] } else { ["A", "Z"] };
                let files = names
                    .iter()
                    .map(|name| {
                        let mut records: Vec<Value> = (0..5)
                            .map(|v| object([("dataName", Value::from("shared")), ("ref", "shared".into()), ("value", v.into())]))
                            .collect();
                        if reverse {
                            records.reverse();
                        }
                        TemplateFile {
                            layer: layer.into(),
                            name: (*name).into(),
                            source: Default::default(),
                            sha256: String::new(),
                            records,
                        }
                    })
                    .collect();
                ts.files.push((layer.into(), files));
            }
            discover(&ts)
        };
        assert_eq!(dumps(&discover_with(false), false).unwrap(), dumps(&discover_with(true), false).unwrap());
        let forward = discover_with(false);
        for r in forward.get("refFields").unwrap().as_array().unwrap() {
            assert_eq!(r.get("targets").unwrap(), &Value::Array(vec!["A".into(), "Z".into()]));
        }
    }

    #[test]
    fn examples_dedupe_with_python_equality() {
        let rec = |v: Value| object([("dataName", Value::from("x")), ("v", v)]);
        let ts = TemplateSet {
            files: vec![(
                "base".into(),
                vec![TemplateFile {
                    layer: "base".into(),
                    name: "T".into(),
                    source: Default::default(),
                    sha256: String::new(),
                    records: vec![rec(1.into()), rec(true.into()), rec(1.0.into()), rec(0.5.into()), rec("a".into())],
                }],
            )],
            ..Default::default()
        };
        let schema = discover(&ts);
        let v = schema.get("templates").unwrap().get("T").unwrap().get("fields").unwrap().get("v").unwrap();
        assert_eq!(dumps(v.get("examples").unwrap(), true).unwrap(), "[0.5,1,\"a\"]\n");
    }
}
