//! Localization tables and the game's rich-text markup.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

/// All localization strings of one language (base game, then all DLC files, as the game merges them).
#[derive(Debug)]
pub struct Locale {
    lang: String,
    strings: HashMap<String, String>,
}

impl Locale {
    pub(crate) fn new(lang: &str, strings: HashMap<String, String>) -> Self {
        Self { lang: lang.into(), strings }
    }

    pub fn lang(&self) -> &str {
        &self.lang
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.strings.get(key).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.strings.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Marker for hidden strings. The extractor already removes it (as the game does at load),
/// leaving an empty value; kept here for robustness with raw text.
pub const SKIP_MARKER: &str = "<skip/>";

/// True if the game would show nothing for this text.
pub fn is_skipped(text: &str) -> bool {
    text.trim().is_empty() || text.trim_start().starts_with(SKIP_MARKER)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Span {
    Text { text: String, style: Style, color: Option<String> },
    LineBreak,
    /// `{0}`, `{resistLeader}`, … – filled in by the game at runtime.
    Placeholder(String),
    /// `<align=right>` etc. (alignment is reset by `<align=left>`).
    Align(String),
    /// `<sprite …>` and other inline graphics, kept verbatim.
    Sprite(String),
}

/// Parse Unity rich text (`<b>`, `<i>`, `<u>`, `<color=…>`, `<br/>`, `<align=…>`, `{tokens}`).
/// Unknown tags are dropped; `<skip/>` yields no spans.
pub fn parse_rich(text: &str) -> Vec<Span> {
    if is_skipped(text) {
        return Vec::new();
    }
    let mut spans = Vec::new();
    let mut style = Style::default();
    let mut colors: Vec<String> = Vec::new();
    let mut buf = String::new();
    let flush = |buf: &mut String, spans: &mut Vec<Span>, style: Style, colors: &[String]| {
        if !buf.is_empty() {
            spans.push(Span::Text { text: std::mem::take(buf), style, color: colors.last().cloned() });
        }
    };
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        match c {
            '<' if rest[1..].find('>').is_some_and(|end| !rest[1..1 + end].contains('<')) => {
                let end = rest.find('>').unwrap();
                let tag = &rest[1..end];
                rest = &rest[end + 1..];
                let lower = tag.to_ascii_lowercase();
                let (name, arg) = match lower.split_once('=') {
                    Some((n, _)) => (n.trim().to_string(), Some(tag[n.len() + 1..].trim().trim_matches('"').to_string())),
                    None => (lower.trim().trim_end_matches('/').trim().to_string(), None),
                };
                flush(&mut buf, &mut spans, style, &colors);
                match (name.as_str(), arg) {
                    ("br", _) => spans.push(Span::LineBreak),
                    ("b", _) => style.bold = true,
                    ("/b", _) => style.bold = false,
                    ("i", _) => style.italic = true,
                    ("/i", _) => style.italic = false,
                    ("u", _) => style.underline = true,
                    ("/u", _) => style.underline = false,
                    ("color", Some(c)) => colors.push(c),
                    ("/color", _) => {
                        colors.pop();
                    }
                    ("align", Some(a)) => spans.push(Span::Align(a)),
                    (n, _) if n == "sprite" || n.starts_with("sprite ") => spans.push(Span::Sprite(tag.to_string())),
                    _ => {}
                }
            }
            '{' if rest.find('}').is_some_and(|end| end > 1 && !rest[1..end].contains(['{', ' '])) => {
                let end = rest.find('}').unwrap();
                flush(&mut buf, &mut spans, style, &colors);
                spans.push(Span::Placeholder(rest[1..end].to_string()));
                rest = &rest[end + 1..];
            }
            '\n' => {
                flush(&mut buf, &mut spans, style, &colors);
                spans.push(Span::LineBreak);
                rest = &rest[1..];
            }
            _ => {
                buf.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    flush(&mut buf, &mut spans, style, &colors);
    spans
}

/// Plain text without markup; placeholders are rendered by `placeholder`.
pub fn to_plain_with(text: &str, placeholder: &mut dyn FnMut(&str) -> Option<String>) -> String {
    let mut out = String::new();
    for span in parse_rich(text) {
        match span {
            Span::Text { text, .. } => out.push_str(&text),
            Span::LineBreak => out.push('\n'),
            Span::Placeholder(p) => match placeholder(&p) {
                Some(v) => out.push_str(&v),
                None => {
                    out.push('{');
                    out.push_str(&p);
                    out.push('}');
                }
            },
            Span::Align(_) | Span::Sprite(_) => {}
        }
    }
    out
}

/// Plain text without markup, placeholders kept as `{name}`.
pub fn to_plain(text: &str) -> String {
    to_plain_with(text, &mut |_| None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rich_text() {
        let spans = parse_rich("a <i>b</i><br/>{x} <color=#FF0000>c</color> 1 < 2");
        assert_eq!(
            spans,
            vec![
                Span::Text { text: "a ".into(), style: Style::default(), color: None },
                Span::Text { text: "b".into(), style: Style { italic: true, ..Default::default() }, color: None },
                Span::LineBreak,
                Span::Placeholder("x".into()),
                Span::Text { text: " ".into(), style: Style::default(), color: None },
                Span::Text { text: "c".into(), style: Style::default(), color: Some("#FF0000".into()) },
                Span::Text { text: " 1 < 2".into(), style: Style::default(), color: None },
            ]
        );
    }

    #[test]
    fn plain_and_skip() {
        assert_eq!(to_plain("<b>Hi</b><br/><align=right>{who}<align=left>"), "Hi\n{who}");
        assert!(parse_rich("<skip/> hidden").is_empty());
        assert_eq!(to_plain_with("by {0}%", &mut |p| (p == "0").then(|| "5".into())), "by 5%");
    }
}

/// A language plus English fallback, for looking up entity texts.
#[derive(Debug, Clone)]
pub struct Texts {
    pub primary: Arc<Locale>,
    pub fallback: Option<Arc<Locale>>,
}

/// Standard per-entity text fields (`<Template>.<field>.<name>`).
pub mod field {
    pub const DISPLAY_NAME: &str = "displayName";
    pub const SUMMARY: &str = "summary";
    pub const DESCRIPTION: &str = "description";
    pub const QUOTE: &str = "quote";
}

/// Templates whose names the game looks up with the scenario postfix first (`Loc.T_Scenario`).
const SCENARIO_POSTFIX_TEMPLATES: [&str; 4] = ["TINationTemplate", "TIRegionTemplate", "TIOrgTemplate", "TIFactionTemplate"];
/// Templates whose keys all use `localizationName` (`localizationAlias ?? dataName`). For other
/// templates only `displayName` (inherited from `TIDataTemplate`) does; their other fields use `dataName`.
const ALIASED_TEMPLATES: [&str; 7] = [
    "TITechTemplate",
    "TIProjectTemplate",
    "TIEffectTemplate",
    "TIObjectiveTemplate",
    "TINationTemplate",
    "TIRegionTemplate",
    "TIOrgTemplate",
];
/// Individual aliased fields of templates not in [`ALIASED_TEMPLATES`].
const ALIASED_FIELDS: [(&str, &str); 1] = [("TIFactionTemplate", "leader.description")];
/// Leader tokens in tech quotes, replaced by `TIFactionTemplate.fullLeader.<X>Council`.
const LEADER_TOKENS: [(&str, &str); 7] = [
    ("{resistLeader}", "ResistCouncil"),
    ("{destroyLeader}", "DestroyCouncil"),
    ("{exploitLeader}", "ExploitCouncil"),
    ("{submitLeader}", "SubmitCouncil"),
    ("{appeaseLeader}", "AppeaseCouncil"),
    ("{cooperateLeader}", "CooperateCouncil"),
    ("{escapeLeader}", "EscapeCouncil"),
];

impl Texts {
    /// Load `lang` with English fallback (the game's only "core" language).
    pub fn load(ds: &crate::Dataset, lang: &str) -> crate::Result<Self> {
        let primary = ds.locale(lang)?;
        let fallback = if lang == "en" { None } else { ds.locale("en").ok() };
        Ok(Self { primary, fallback })
    }

    pub fn lang(&self) -> &str {
        self.primary.lang()
    }

    /// Value for a key in the language, else English.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.primary.get(key).or_else(|| self.fallback.as_ref()?.get(key))
    }

    /// The game's `localizationName` of a record: `localizationAlias`, else `dataName`.
    pub fn key_name<'r>(_template: &str, data_name: &'r str, record: Option<&'r Value>) -> &'r str {
        record
            .and_then(|r| r.get("localizationAlias"))
            .and_then(Value::as_str)
            .filter(|a| !a.is_empty())
            .unwrap_or(data_name)
    }

    /// Name used in the localization key of `field`: [`Texts::key_name`] for `displayName`
    /// and for every field of techs, projects, effects, objectives, nations, regions and orgs;
    /// `dataName` otherwise (ship parts, hab modules, traits, most faction texts, …).
    pub fn key_name_for<'r>(template: &str, field: &str, data_name: &'r str, record: Option<&'r Value>) -> &'r str {
        if field == field::DISPLAY_NAME || ALIASED_TEMPLATES.contains(&template) || ALIASED_FIELDS.contains(&(template, field)) {
            Self::key_name(template, data_name, record)
        } else {
            data_name
        }
    }

    /// Raw localized text of an entity field (`<Template>.<field>.<name>`), with the game's
    /// lookup rules: `localizationAlias` per [`Texts::key_name_for`], scenario postfix first for
    /// nation/region/org/faction texts, the drive description shared by all thruster counts.
    /// `record` is the variant in use; `postfix` the scenario's localization postfix.
    pub fn entity_field(&self, template: &str, data_name: &str, record: Option<&Value>, field: &str, postfix: Option<&str>) -> Option<&str> {
        let name = Self::key_name_for(template, field, data_name, record);
        let key = if template == "TIDriveTemplate" && field == field::DESCRIPTION {
            // `TIDriveTemplate.description.<dataName with last char -> '1'>`
            let mut n = name.to_string();
            n.pop();
            format!("{template}.{field}.{n}1")
        } else {
            format!("{template}.{field}.{name}")
        };
        if let Some(p) = postfix.filter(|_| SCENARIO_POSTFIX_TEMPLATES.contains(&template))
            && let Some(v) = self.get(&format!("{key}{p}")) {
                return Some(v);
            }
        self.get(&key)
    }

    /// Display name: localized `displayName`, else `friendlyName`/`displayName` of the record, else `dataName`.
    pub fn display_name(&self, template: &str, data_name: &str, record: Option<&Value>, postfix: Option<&str>) -> String {
        if let Some(t) = self.entity_field(template, data_name, record, field::DISPLAY_NAME, postfix) {
            let plain = to_plain(t);
            if !plain.trim().is_empty() {
                return plain;
            }
        }
        record
            .and_then(|r| r.get("friendlyName").or_else(|| r.get("displayName")))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| data_name.to_string())
    }

    /// Replace the faction-leader tokens used in tech quotes.
    pub fn replace_leader_tokens(&self, text: &str) -> String {
        let mut out = text.to_string();
        for (token, faction) in LEADER_TOKENS {
            if out.contains(token)
                && let Some(name) = self.get(&format!("TIFactionTemplate.fullLeader.{faction}")) {
                    out = out.replace(token, &to_plain(name));
                }
        }
        out
    }
}

#[cfg(test)]
mod lookup_tests {
    use serde_json::json;

    use super::*;
    use crate::scenario::META;
    use crate::test_support::{dataset, id, scenario};

    #[test]
    fn localization_alias_applies_per_template_and_field() {
        let ds = dataset(
            &[
                (META, vec![scenario("S", 1, &[], Some(".2003"))]),
                ("TINationTemplate", vec![json!({"dataName": "2003_RUS", "localizationAlias": "RUS"})]),
                ("TITraitTemplate", vec![json!({"dataName": "2003_Trait", "localizationAlias": "Trait"})]),
                (crate::model::FACTION, vec![json!({"dataName": "F_2003", "localizationAlias": "F"})]),
            ],
            json!({
                "TINationTemplate.displayName.RUS": "Russia",
                "TINationTemplate.displayName.RUS.2003": "Russian Federation",
                "TINationTemplate.displayName.2003_RUS": "2003_Russia",
                "TITraitTemplate.displayName.Trait": "Trait name",
                "TITraitTemplate.description.Trait": "wrong",
                "TITraitTemplate.description.2003_Trait": "Trait description",
                "TIFactionTemplate.displayName.F": "Faction",
                "TIFactionTemplate.leader.description.F": "Leader description",
                "TIFactionTemplate.Goal.F_2003": "Goal",
            }),
        );
        let texts = Texts::load(&ds, "en").unwrap();
        let s = ds.scenarios()[0].idx;
        let overview = ds.overview();
        let scenario = ds.view(crate::Scope::scenario(s));
        let nation = id(&ds, "TINationTemplate", "2003_RUS");
        assert_eq!(overview.name(&texts, nation), "Russia");
        // alias first, then the scenario postfix on the aliased key
        assert_eq!(scenario.name(&texts, nation), "Russian Federation");
        let t = id(&ds, "TITraitTemplate", "2003_Trait");
        assert_eq!(overview.name(&texts, t), "Trait name");
        assert_eq!(overview.text(&texts, t, field::DESCRIPTION).as_deref(), Some("Trait description"));
        let f = id(&ds, crate::model::FACTION, "F_2003");
        assert_eq!(overview.name(&texts, f), "Faction");
        assert_eq!(overview.text(&texts, f, "leader.description").as_deref(), Some("Leader description"));
        assert_eq!(overview.text(&texts, f, "Goal").as_deref(), Some("Goal"));
    }
}
