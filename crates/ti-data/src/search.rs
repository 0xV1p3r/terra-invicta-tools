//! Full-text search over every record of a build.
//!
//! The corpus is small (tens of thousands of records), so the index is a list
//! of pre-normalized strings scanned per query: substring matching works for
//! every language (including CJK, which has no word boundaries) and ranks by
//! which field matched. Normalization folds case and diacritics.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::dataset::{Dataset, EntityId};
use crate::text::{Texts, field, is_skipped, to_plain};

/// Which part of a record matched a query term.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MatchField {
    Name,
    DataName,
    Text,
    Value,
}

#[derive(Debug, Clone)]
pub struct Hit {
    pub id: EntityId,
    pub score: u32,
    /// Best-matching field (over all query terms).
    pub field: MatchField,
}

#[derive(Debug)]
struct Doc {
    id: EntityId,
    /// Display names over all variants, normalized and joined by `\n`.
    name: String,
    data_name: String,
    text: String,
    values: String,
}

#[derive(Debug)]
pub struct SearchIndex {
    lang: String,
    docs: Vec<Doc>,
}

pub fn normalize(s: &str) -> String {
    s.nfkd().filter(|c| !is_combining_mark(*c)).flat_map(char::to_lowercase).collect()
}

/// Split `CamelCase_names` into words so `heat sink` finds `HeatSink`.
fn split_identifier(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    let mut prev: Option<char> = None;
    for c in s.chars() {
        if c == '_' {
            out.push(' ');
        } else {
            if let Some(p) = prev
                && ((c.is_uppercase() && p.is_lowercase()) || (c.is_ascii_digit() != p.is_ascii_digit() && p != '_')) {
                    out.push(' ');
                }
            out.push(c);
        }
        prev = Some(c);
    }
    out
}

fn collect_values(record: &serde_json::Value, out: &mut String) {
    use serde_json::Value;
    match record {
        Value::String(s) if !s.is_empty() => {
            out.push_str(s);
            out.push('\n');
        }
        Value::Number(n) => {
            out.push_str(&n.to_string());
            out.push('\n');
        }
        Value::Array(items) => items.iter().for_each(|v| collect_values(v, out)),
        Value::Object(map) => map.values().for_each(|v| collect_values(v, out)),
        _ => {}
    }
}

impl SearchIndex {
    /// Index every entity (all layers) with texts from `texts`.
    pub fn build(ds: &Dataset, texts: &Texts) -> Self {
        let mut docs = Vec::with_capacity(ds.entities().len());
        let overview = ds.overview();
        let postfixes: Vec<Option<String>> =
            std::iter::once(None).chain(ds.scenarios().iter().filter_map(|s| s.localization_postfix.clone()).map(Some)).collect();
        for e in ds.entities() {
            let mut names = vec![normalize(&overview.name(texts, e.id))];
            let mut text = String::new();
            // Names/texts of every variant and every scenario postfix, so any scope finds it.
            for v in &e.variants {
                for postfix in &postfixes {
                    let n = normalize(&texts.display_name(&e.template, &e.data_name, Some(&v.record), postfix.as_deref()));
                    if !names.contains(&n) {
                        names.push(n);
                    }
                }
                for f in [field::SUMMARY, field::DESCRIPTION, field::QUOTE] {
                    if let Some(t) = texts.entity_field(&e.template, &e.data_name, Some(&v.record), f, None).filter(|t| !is_skipped(t)) {
                        let t = normalize(&to_plain(t));
                        if !text.contains(&t) {
                            text.push_str(&t);
                            text.push('\n');
                        }
                    }
                }
            }
            if let Some(body) = overview.text(texts, e.id, field::DESCRIPTION) {
                text.push_str(&normalize(&body));
                text.push('\n');
            }
            // Raw values of every variant, each distinct value once.
            let mut values = String::new();
            let mut seen = std::collections::HashSet::new();
            for v in &e.variants {
                let mut raw = String::new();
                collect_values(&v.record, &mut raw);
                for line in raw.lines().filter(|l| seen.insert(l.to_string())) {
                    values.push_str(line);
                    values.push('\n');
                }
            }
            docs.push(Doc {
                id: e.id,
                name: names.join("\n"),
                data_name: normalize(&format!("{}\n{}", e.data_name, split_identifier(&e.data_name))),
                text,
                values: normalize(&values),
            });
        }
        Self { lang: texts.lang().into(), docs }
    }

    pub fn lang(&self) -> &str {
        &self.lang
    }

    /// All terms must match somewhere; results are sorted by score (best first).
    /// `filter` restricts the candidate set (scope, templates, …).
    pub fn search(&self, query: &str, limit: usize, filter: &dyn Fn(EntityId) -> bool) -> Vec<Hit> {
        let q = normalize(query.trim());
        if q.is_empty() {
            return Vec::new();
        }
        let terms: Vec<&str> = q.split_whitespace().collect();
        let mut hits: Vec<Hit> = Vec::new();
        'docs: for doc in &self.docs {
            if !filter(doc.id) {
                continue;
            }
            let mut score = 0u32;
            let mut best = MatchField::Value;
            for term in &terms {
                let Some((s, f)) = score_term(doc, term) else {
                    continue 'docs;
                };
                score += s;
                best = best.min(f);
            }
            if q.len() > 1 && doc.name.split('\n').any(|n| n == q) {
                score += 1000; // exact full-name match
            } else if doc.data_name.split('\n').next() == Some(q.as_str()) {
                score += 800; // exact dataName match
            }
            hits.push(Hit { id: doc.id, score, field: best });
        }
        hits.sort_by(|a, b| b.score.cmp(&a.score).then(a.id.cmp(&b.id)));
        hits.truncate(limit);
        hits
    }
}

fn word_start(haystack: &str, pos: usize) -> bool {
    pos == 0 || haystack[..pos].chars().next_back().is_some_and(|c| !c.is_alphanumeric())
}

fn score_term(doc: &Doc, term: &str) -> Option<(u32, MatchField)> {
    let in_field = |h: &str| -> Option<bool> {
        let mut found = None;
        for (pos, _) in h.match_indices(term) {
            if word_start(h, pos) {
                return Some(true);
            }
            found = Some(false);
        }
        found
    };
    if let Some(ws) = in_field(&doc.name) {
        return Some((if ws { 100 } else { 60 }, MatchField::Name));
    }
    if let Some(ws) = in_field(&doc.data_name) {
        return Some((if ws { 50 } else { 30 }, MatchField::DataName));
    }
    if let Some(ws) = in_field(&doc.text) {
        return Some((if ws { 12 } else { 6 }, MatchField::Text));
    }
    if let Some(ws) = in_field(&doc.values) {
        return Some((if ws { 4 } else { 2 }, MatchField::Value));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_of_all_variants_are_indexed() {
        use serde_json::json;
        let ds = crate::test_support::dataset(
            &[
                (crate::scenario::META, vec![crate::test_support::scenario("S", 1, &["A"], None)]),
                (
                    crate::model::TECH,
                    vec![
                        json!({"dataName": "T", "researchCost": 10, "flavor": "common"}),
                        json!({"dataName": "T", "scenarioTags": ["A"], "flavor": "common", "extra": "zanzibar"}),
                    ],
                ),
            ],
            json!({}),
        );
        let texts = Texts::load(&ds, "en").unwrap();
        let index = SearchIndex::build(&ds, &texts);
        let hits = index.search("zanzibar", 10, &|_| true);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].field, MatchField::Value);
        let doc = index.docs.iter().find(|d| d.id == hits[0].id).unwrap();
        assert_eq!(doc.values.matches("common").count(), 1);
    }

    #[test]
    fn normalization() {
        assert_eq!(normalize("Énergie ÜBER"), "energie uber");
        assert_eq!(split_identifier("Project_HeatSinkMk2"), "Project Heat Sink Mk 2");
    }
}
