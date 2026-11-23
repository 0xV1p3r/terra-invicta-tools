//! Read-only reader for Terra Invicta campaign saves: the human player's research state.
//!
//! A save is a gzip (or, with compression off, plain) UTF-8 FullSerializer document:
//! `{"currentID": …, "gamestates": {"<C# type>": [{"Key": {"value": id}, "Value": {…}}, …]}}`.
//! FullSerializer writes bare `Infinity`/`-Infinity`/`NaN`, so those tokens are turned into
//! `null` (outside strings only) before JSON decoding. Only the few state records needed for
//! research are decoded; everything else is skipped unparsed.
//!
//! See `docs/research/save-files.md` for the format and the meaning of each field.

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::value::RawValue;

/// Largest decompressed save accepted (observed saves are up to about 80 MB).
pub const MAX_DECOMPRESSED: u64 = 512 * 1024 * 1024;

/// Scenario of saves that predate `scenarioMetaTemplateName` (the game's own fallback).
pub const LEGACY_SCENARIO: &str = "ModernScenario";

const TYPE_PREFIX: &str = "PavonisInteractive.TerraInvicta.";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not read {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("Could not decompress the save: {0}")]
    Gzip(std::io::Error),
    #[error("The save is larger than {} MB when decompressed.", MAX_DECOMPRESSED / (1024 * 1024))]
    TooLarge,
    #[error("The save is not UTF-8 text.")]
    NotText,
    #[error("The save is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Unsupported save: {0}")]
    Shape(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A date in game time or real time (`TIDateTime`/`DateTime` as serialized).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    #[serde(default)]
    pub hour: u32,
    #[serde(default)]
    pub minute: u32,
    #[serde(default)]
    pub second: u32,
}

/// Points accumulated in one public tech slot (techs are shared by all factions).
#[derive(Debug, Clone, PartialEq)]
pub struct TechSlot {
    pub tech: String,
    pub accumulated: f64,
}

/// A selected or paused faction project.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSlot {
    pub project: String,
    pub accumulated: f64,
    /// 3 = base slot, 4 = org slot, 5 = hab slot, 6 and up = paused.
    pub slot: i32,
    /// Researching now (an enabled slot below 6), rather than paused.
    pub active: bool,
}

/// The human player's research state in one save.
#[derive(Debug, Clone, PartialEq)]
pub struct Save {
    /// The player's `TIFactionTemplate` name, e.g. `ResistCouncil`.
    pub faction: String,
    /// `TIMetaTemplate` name of the scenario.
    pub scenario: String,
    /// The save has no scenario field; `scenario` is the game's fallback [`LEGACY_SCENARIO`].
    pub scenario_inferred: bool,
    pub game_date: Date,
    pub campaign_start_version: Option<String>,
    pub save_version: Option<String>,
    /// Real time the campaign was started: with the scenario, tells campaigns apart.
    pub campaign_started: Option<Date>,
    /// Campaign research speed (`researchSpeedMultiplier`); costs are divided by it. `None` if absent.
    pub research_speed: Option<f64>,
    pub modded: bool,
    /// Completed techs (global).
    pub finished_techs: Vec<String>,
    /// Completed faction projects; a repeatable project appears once per completion.
    pub finished_projects: Vec<String>,
    /// `oneTimeGlobally` projects completed by any faction.
    pub finished_one_time_projects: Vec<String>,
    /// Projects unlocked for the faction (the authority on project availability).
    pub available_projects: Vec<String>,
    /// Projects whose unlock roll failed.
    pub missed_projects: Vec<String>,
    /// Projects waiting on an unlock roll.
    pub pending_projects: Vec<String>,
    pub tech_slots: Vec<TechSlot>,
    pub project_slots: Vec<ProjectSlot>,
    /// End-game techs completed per category (`None` in old saves).
    pub end_game_completions: Option<BTreeMap<String, u32>>,
}

/// Read and decode the save file at `path`.
pub fn read(path: &Path) -> Result<Save> {
    let bytes = std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse(&bytes)
}

/// Decode a save from its file contents (gzip or plain text).
pub fn parse(bytes: &[u8]) -> Result<Save> {
    let text = decompress(bytes)?;
    let text = String::from_utf8(text).map_err(|_| Error::NotText)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let json = normalize_non_finite(text);
    let root: Root<'_> = serde_json::from_str(&json)?;
    extract(&root)
}

fn decompress(bytes: &[u8]) -> Result<Vec<u8>> {
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        if bytes.len() as u64 > MAX_DECOMPRESSED {
            return Err(Error::TooLarge);
        }
        return Ok(bytes.to_vec());
    }
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .take(MAX_DECOMPRESSED + 1)
        .read_to_end(&mut out)
        .map_err(Error::Gzip)?;
    if out.len() as u64 > MAX_DECOMPRESSED {
        return Err(Error::TooLarge);
    }
    Ok(out)
}

/// Replace FullSerializer's bare `Infinity`, `-Infinity` and `NaN` (outside strings, as whole
/// tokens) with `null`. Borrows the input when there are none.
fn normalize_non_finite(text: &str) -> std::borrow::Cow<'_, str> {
    const TOKENS: [(&str, &str); 3] = [("-Infinity", "null     "), ("Infinity", "null    "), ("NaN", "null")];
    let bytes = text.as_bytes();
    let mut out: Option<String> = None;
    let mut copied = 0;
    let mut in_string = false;
    let mut i = 0;
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'.';
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            match b {
                b'\\' => i += 1,
                b'"' => in_string = false,
                _ => {}
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        if b == b'-' || b == b'I' || b == b'N' {
            let boundary_before = i == 0 || !word(bytes[i - 1]);
            let hit = TOKENS
                .iter()
                .find(|(t, _)| bytes[i..].starts_with(t.as_bytes()) && bytes.get(i + t.len()).is_none_or(|&n| !word(n)));
            if let (true, Some((token, null))) = (boundary_before, hit) {
                let s = out.get_or_insert_with(|| String::with_capacity(text.len()));
                s.push_str(&text[copied..i]);
                s.push_str(null);
                i += token.len();
                copied = i;
                continue;
            }
        }
        i += 1;
    }
    match out {
        Some(mut s) => {
            s.push_str(&text[copied..]);
            s.into()
        }
        None => text.into(),
    }
}

// ---- document shape -----------------------------------------------------------

#[derive(Deserialize)]
struct Root<'a> {
    #[serde(borrow)]
    gamestates: HashMap<String, &'a RawValue>,
}

#[derive(Deserialize)]
struct Id {
    value: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Entry<T> {
    key: Id,
    value: T,
}

/// Bucket entries of a state type, by game-state id. Absent and `{}` buckets are empty.
fn bucket<T: DeserializeOwned>(root: &Root<'_>, name: &str) -> Result<Vec<(i64, T)>> {
    let raw = root.gamestates.get(&format!("{TYPE_PREFIX}{name}")).or_else(|| root.gamestates.get(name));
    let Some(raw) = raw else { return Ok(Vec::new()) };
    if raw.get().trim_start().starts_with('{') {
        let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(raw.get())?;
        if map.is_empty() {
            return Ok(Vec::new());
        }
        return Err(Error::Shape(format!("{name} is not a list of records")));
    }
    let entries: Vec<Entry<T>> = serde_json::from_str(raw.get()).map_err(|e| Error::Shape(format!("{name}: {e}")))?;
    Ok(entries.into_iter().map(|e| (e.key.value, e.value)).collect())
}

fn singleton<T: DeserializeOwned>(root: &Root<'_>, name: &str) -> Result<T> {
    let mut records = bucket::<T>(root, name)?;
    match records.len() {
        1 => Ok(records.pop().expect("one record").1),
        0 => Err(Error::Shape(format!("no {name} record"))),
        n => Err(Error::Shape(format!("{n} {name} records, expected one"))),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayerState {
    #[serde(rename = "isAI")]
    is_ai: bool,
    faction: Option<Id>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct FactionState {
    template_name: String,
    finished_project_names: Vec<String>,
    available_project_names: Vec<String>,
    missed_projects: Vec<String>,
    active_project_triggers: Vec<Trigger>,
    current_project_progress: Vec<ProjectProgress>,
    org_project_slot_unlocked: bool,
    hab_project_slot_unlocked: bool,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Trigger {
    project_template_name: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ProjectProgress {
    project_template_name: String,
    accumulated_research: Option<f64>,
    slot: i32,
    completed: bool,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct GlobalResearch {
    finished_techs_names: Vec<String>,
    finished_one_time_only_project_names: Vec<String>,
    tech_progress: Vec<TechProgress>,
    end_game_techs_completed_by_category: Option<BTreeMap<String, u32>>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct TechProgress {
    tech_template_name: Option<String>,
    accumulated_research: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimeState {
    current_date_time: Date,
    #[serde(default)]
    scenario_meta_template_name: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct GlobalValues {
    campaign_start_version: Option<String>,
    latest_save_version: Option<String>,
    real_world_campaign_start: Option<Date>,
    scenario_customizations: Option<Customizations>,
    modding_active: bool,
    modding_used_anytime: bool,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Customizations {
    research_speed_multiplier: Option<f64>,
}

fn extract(root: &Root<'_>) -> Result<Save> {
    let humans: Vec<PlayerState> = bucket::<PlayerState>(root, "TIPlayerState")?
        .into_iter()
        .map(|(_, p)| p)
        .filter(|p| !p.is_ai)
        .collect();
    let faction_id = match humans.as_slice() {
        [p] => p
            .faction
            .as_ref()
            .map(|f| f.value)
            .ok_or_else(|| Error::Shape("the player has no faction".into()))?,
        [] => return Err(Error::Shape("no human player".into())),
        _ => return Err(Error::Shape(format!("{} human players; only single-player saves are supported", humans.len()))),
    };
    let faction = bucket::<FactionState>(root, "TIFactionState")?
        .into_iter()
        .find(|(id, _)| *id == faction_id)
        .map(|(_, f)| f)
        .ok_or_else(|| Error::Shape(format!("the player's faction {faction_id} is missing")))?;
    if faction.template_name.is_empty() {
        return Err(Error::Shape("the player's faction has no template".into()));
    }
    let research: GlobalResearch = singleton(root, "TIGlobalResearchState")?;
    let time: TimeState = singleton(root, "TITimeState")?;
    let values: GlobalValues = singleton(root, "TIGlobalValuesState")?;

    let tech_slots = research
        .tech_progress
        .into_iter()
        .filter_map(|t| {
            let tech = t.tech_template_name.filter(|n| !n.is_empty())?;
            Some((tech, t.accumulated_research))
        })
        .map(|(tech, points)| {
            let accumulated = points.ok_or_else(|| Error::Shape(format!("research points of {tech} are not a number")))?;
            Ok(TechSlot { tech, accumulated })
        })
        .collect::<Result<Vec<_>>>()?;
    let enabled = |slot: i32| match slot {
        3 => true,
        4 => faction.org_project_slot_unlocked,
        5 => faction.hab_project_slot_unlocked,
        _ => false,
    };
    let project_slots = faction
        .current_project_progress
        .iter()
        .filter(|p| !p.completed && !p.project_template_name.is_empty())
        .map(|p| {
            let accumulated = p
                .accumulated_research
                .ok_or_else(|| Error::Shape(format!("research points of {} are not a number", p.project_template_name)))?;
            Ok(ProjectSlot {
                project: p.project_template_name.clone(),
                accumulated,
                slot: p.slot,
                active: enabled(p.slot),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let (scenario, scenario_inferred) = match time.scenario_meta_template_name.filter(|s| !s.is_empty()) {
        Some(s) => (s, false),
        None => (LEGACY_SCENARIO.to_string(), true),
    };
    Ok(Save {
        faction: faction.template_name,
        scenario,
        scenario_inferred,
        game_date: time.current_date_time,
        campaign_start_version: values.campaign_start_version,
        save_version: values.latest_save_version,
        campaign_started: values.real_world_campaign_start,
        research_speed: values
            .scenario_customizations
            .and_then(|c| c.research_speed_multiplier)
            .filter(|s| s.is_finite() && *s > 0.0),
        modded: values.modding_active || values.modding_used_anytime,
        finished_techs: research.finished_techs_names,
        finished_projects: faction.finished_project_names,
        finished_one_time_projects: research.finished_one_time_only_project_names,
        available_projects: faction.available_project_names,
        missed_projects: faction.missed_projects,
        pending_projects: faction
            .active_project_triggers
            .into_iter()
            .map(|t| t.project_template_name)
            .filter(|n| !n.is_empty())
            .collect(),
        tech_slots,
        project_slots,
        end_game_completions: research.end_game_techs_completed_by_category,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn doc(extra_buckets: &str, scenario: &str) -> String {
        format!(
            r#"{{"currentID": {{"value": 99}}, "gamestates": {{
            "PavonisInteractive.TerraInvicta.TIPlayerState": [
                {{"Key": {{"value": 1}}, "Value": {{"isAI": true, "faction": {{"value": 10}}}}}},
                {{"Key": {{"value": 2}}, "Value": {{"isAI": false, "faction": {{"value": 11}}, "templateName": "P"}}}}
            ],
            "PavonisInteractive.TerraInvicta.TIFactionState": [
                {{"Key": {{"value": 10}}, "Value": {{"templateName": "AlienCouncil", "finishedProjectNames": ["X"]}}}},
                {{"Key": {{"value": 11}}, "Value": {{
                    "templateName": "ResistCouncil",
                    "finishedProjectNames": ["Project_A", "Project_R", "Project_R"],
                    "availableProjectNames": ["Project_B", "Project_R"],
                    "missedProjects": ["Project_M"],
                    "activeProjectTriggers": [{{"projectTemplateName": "Project_T", "monthlyTriggerValue": 0.5}}],
                    "currentProjectProgress": [
                        {{"projectTemplateName": "Project_B", "accumulatedResearch": 10.5, "slot": 3, "completed": false}},
                        {{"projectTemplateName": "Project_C", "accumulatedResearch": 4, "slot": 4, "completed": false}},
                        {{"projectTemplateName": "Project_P", "accumulatedResearch": 7, "slot": 6, "completed": false}},
                        {{"projectTemplateName": "Project_A", "accumulatedResearch": 1, "slot": 5, "completed": true}}
                    ],
                    "orgProjectSlotUnlocked": false,
                    "habProjectSlotUnlocked": true,
                    "unrelated": [NaN, "Infinity"],
                    "ratio": -Infinity
                }}}}
            ],
            "PavonisInteractive.TerraInvicta.TIGlobalResearchState": [
                {{"Key": {{"value": 3}}, "Value": {{
                    "finishedTechsNames": ["TechA", "TechB"],
                    "finishedOneTimeOnlyProjectNames": [],
                    "techProgress": [
                        {{"techTemplateName": "TechC", "accumulatedResearch": 123.25}},
                        {{"techTemplateName": "", "accumulatedResearch": 0}}
                    ],
                    "endGameTechsCompletedByCategory": {{"Energy": 2}}
                }}}}
            ],
            "PavonisInteractive.TerraInvicta.TITimeState": [
                {{"Key": {{"value": 4}}, "Value": {{"currentDateTime": {{"year": 2030, "month": 2, "day": 3, "hour": 12}}{scenario}}}}}
            ],
            "PavonisInteractive.TerraInvicta.TIGlobalValuesState": [
                {{"Key": {{"value": 5}}, "Value": {{"latestSaveVersion": "1.0.65", "scenarioCustomizations": {{"researchSpeedMultiplier": 2.0, "note": "NaN in text"}}}}}}
            ],
            "TIHistoricalData": {{}},
            "PavonisInteractive.TerraInvicta.TINationState": {{}}{extra_buckets}
        }}}}"#
        )
    }

    fn gzip(text: &str) -> Vec<u8> {
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(text.as_bytes()).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn reads_the_players_research_state() {
        let text = format!("\u{feff}{}", doc("", r#", "scenarioMetaTemplateName": "2026Scenario""#));
        for bytes in [gzip(&text), text.clone().into_bytes()] {
            let s = parse(&bytes).unwrap();
            assert_eq!(s.faction, "ResistCouncil");
            assert_eq!((s.scenario.as_str(), s.scenario_inferred), ("2026Scenario", false));
            assert_eq!((s.game_date.year, s.game_date.month, s.game_date.day), (2030, 2, 3));
            assert_eq!(s.save_version.as_deref(), Some("1.0.65"));
            assert_eq!(s.research_speed, Some(2.0));
            assert_eq!(s.finished_techs, ["TechA", "TechB"]);
            assert_eq!(s.finished_projects, ["Project_A", "Project_R", "Project_R"]);
            assert_eq!(s.available_projects, ["Project_B", "Project_R"]);
            assert_eq!(s.missed_projects, ["Project_M"]);
            assert_eq!(s.pending_projects, ["Project_T"]);
            assert_eq!(
                s.tech_slots,
                [TechSlot {
                    tech: "TechC".into(),
                    accumulated: 123.25
                }]
            );
            let slots: Vec<(&str, i32, bool)> = s.project_slots.iter().map(|p| (p.project.as_str(), p.slot, p.active)).collect();
            // the org slot is locked, slot 6 is paused, completed entries are dropped
            assert_eq!(slots, [("Project_B", 3, true), ("Project_C", 4, false), ("Project_P", 6, false)]);
            assert_eq!(s.end_game_completions.unwrap()["Energy"], 2);
        }
    }

    #[test]
    fn old_saves_fall_back_to_the_legacy_scenario() {
        let s = parse(doc("", "").as_bytes()).unwrap();
        assert_eq!((s.scenario.as_str(), s.scenario_inferred), (LEGACY_SCENARIO, true));
    }

    #[test]
    fn non_finite_tokens_only_outside_strings() {
        let src = r#"{"a": Infinity, "b": [-Infinity,NaN], "c": "Infinity \"NaN\" -Infinity", "d": NaNo, "e": 1}"#;
        let out = normalize_non_finite(src);
        assert_eq!(
            out,
            r#"{"a": null    , "b": [null     ,null], "c": "Infinity \"NaN\" -Infinity", "d": NaNo, "e": 1}"#
        );
        assert!(matches!(normalize_non_finite("{\"x\": 1}"), std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn rejects_unsupported_shapes() {
        let two_humans = doc("", "").replace(r#""isAI": true"#, r#""isAI": false"#);
        assert!(matches!(parse(two_humans.as_bytes()), Err(Error::Shape(_))));
        let infinite_progress = doc("", "").replace("123.25", "Infinity");
        assert!(matches!(parse(infinite_progress.as_bytes()), Err(Error::Shape(m)) if m.contains("TechC")));
        assert!(matches!(parse(&gzip("{\"gamestates\": {")[..20]), Err(Error::Gzip(_))));
        assert!(matches!(parse(b"\xff\xfe"), Err(Error::NotText)));
    }

    /// The developer's local save folder (`TI_SAVE_DIR`, else the Proton default), if any.
    fn local_saves() -> Option<PathBuf> {
        let dir = std::env::var_os("TI_SAVE_DIR").map(PathBuf::from).or_else(|| {
            let home = std::env::var_os("HOME")?;
            Some(PathBuf::from(home).join(".local/share/Steam/steamapps/compatdata/1176470/pfx/drive_c/users/steamuser/Documents/My Games/TerraInvicta/Saves"))
        })?;
        dir.is_dir().then_some(dir)
    }

    /// Every local save decodes (skipped without saves; never required in CI).
    #[test]
    fn reads_local_saves() {
        let Some(dir) = local_saves() else { return };
        let Ok(entries) = std::fs::read_dir(&dir) else { return };
        for path in entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "gz"))
        {
            let s = read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert!(!s.faction.is_empty(), "{}", path.display());
            assert!(s.game_date.year >= 2022, "{}", path.display());
        }
    }

    /// The acceptance case of docs/research/save-files.md (that `ExitSave.gz` is `RE_ASIA_EU_1-3.gz`).
    #[test]
    fn matches_the_research_notes() {
        let Some(path) = local_saves().map(|d| d.join("RE_ASIA_EU_1-3.gz")).filter(|p| p.is_file()) else {
            return;
        };
        let s = read(&path).unwrap();
        assert_eq!((s.faction.as_str(), s.scenario.as_str()), ("ResistCouncil", "2026Scenario"));
        assert_eq!(s.finished_techs.len(), 58);
        assert_eq!(s.finished_projects.len(), 74);
        assert_eq!(s.available_projects.len(), 113);
        let techs: Vec<&str> = s.tech_slots.iter().map(|t| t.tech.as_str()).collect();
        assert_eq!(techs, ["NuclearFusioninSpace", "IndustrializationofSpace", "QuantumEncryption"]);
        assert!((s.tech_slots[0].accumulated - 28349.14).abs() < 0.01);
        let mut slots: Vec<(i32, bool)> = s.project_slots.iter().map(|p| (p.slot, p.active)).collect();
        slots.sort();
        assert_eq!(slots, [(3, true), (4, true), (5, true), (6, false), (7, false)]);
    }
}
