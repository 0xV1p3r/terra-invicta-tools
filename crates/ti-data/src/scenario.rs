//! Scenarios and the game's per-scenario template resolution.
//!
//! Starting instances vs runtime templates: recursive scenario meta groups describe
//! starting instances, not template availability. Only entities listed in some
//! scenario are gated, and runtime creation paths remain available by scenarioTags.
//! Independent solar-system/faction options do not restrict a start-date scope.
//!
//! A scenario is a `TIMetaTemplate` with `newCampaignOptionCategory ==
//! "Scenario"`. At game start the game picks, for every `(template, dataName)`,
//! one whole record by `scenarioTags` (`TemplateManager.ResolveScenarioTemplates`,
//! see `docs/research/game-semantics.md` §1):
//!
//! 1. Disabled records are not registered; among registered records a later one
//!    with the same tag set as an earlier one is ignored.
//! 2. A record is eligible iff its tags ⊆ the scenario's tags. If the first
//!    registered record is ineligible it is replaced by the first eligible one;
//!    with none, the entity does not exist in that scenario.
//! 3. With several registered records, the winner is the one with the most tags
//!    in common with the scenario, then the earliest-listed matching scenario
//!    tag, then load order. (Like the game, this step does not re-check
//!    eligibility; no current data hits that edge case.)

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use serde_json::Value;

use crate::dataset::{Dataset, EntityId, Variant};
use crate::error::{Error, Result};

pub const META: &str = "TIMetaTemplate";
pub const START_TIME: &str = "TIStartTimeTemplate";
/// The game's default scenario (used for old saves and as the overview default).
pub const DEFAULT_SCENARIO: &str = "ModernScenario";

/// Index into [`Dataset::scenarios`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScenarioIdx(pub(crate) u8);

impl ScenarioIdx {
    /// Most scenarios a dataset can have (one bit each in [`ScenarioSet`]).
    pub const MAX_SCENARIOS: usize = u64::BITS as usize;

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// A set of scenarios (bit per [`ScenarioIdx`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ScenarioSet(pub u64);

impl ScenarioSet {
    pub fn contains(self, s: ScenarioIdx) -> bool {
        self.0 & (1 << s.0) != 0
    }

    pub fn insert(&mut self, s: ScenarioIdx) {
        self.0 |= 1 << s.0;
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    pub fn iter(self) -> impl Iterator<Item = ScenarioIdx> {
        (0..64u8).filter(move |i| self.0 & (1 << i) != 0).map(ScenarioIdx)
    }
}

#[derive(Debug, Clone)]
pub struct Scenario {
    pub idx: ScenarioIdx,
    /// The scenario's `TIMetaTemplate` entity.
    pub meta: EntityId,
    pub data_name: String,
    pub friendly_name: String,
    pub tags: Vec<String>,
    /// Shown in the new-game menu (`isNewCampaignOption` and `listPriority != 999`).
    pub listed: bool,
    pub list_priority: i64,
    pub required_dlc: Vec<String>,
    /// Suffix tried first for scenario-specific names (`.2003`, `.BrokenEarth`).
    pub localization_postfix: Option<String>,
    /// The `TIStartTimeTemplate` record (start date, starting techs, completed projects).
    pub start_time: Option<EntityId>,
    /// Records instantiated through the scenario's meta templates, per template type
    /// (nations, regions, armies, orgs, habs, …).
    pub instantiated: BTreeMap<String, Vec<String>>,
}

impl Scenario {
    pub fn is_dlc(&self) -> bool {
        !self.required_dlc.is_empty()
    }
}

/// `selected[scenario][entity]` = index into `Entity::variants`, or `u16::MAX` if absent.
#[derive(Debug, Default)]
pub(crate) struct Resolution {
    pub selected: Vec<Vec<u16>>,
    /// Union of presence per entity.
    pub presence: Vec<ScenarioSet>,
}

const ABSENT: u16 = u16::MAX;

fn str_list(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

/// The record the game would register for a meta template (metas themselves are
/// rarely tagged; use the first registered variant).
fn meta_record(ds: &Dataset, id: EntityId) -> Option<&Value> {
    ds.entity(id).variants.iter().find(|v| !v.disabled && !v.shadowed).map(|v| &v.record)
}

/// Match TIMetaTemplate.LoadMetaTemplate: names are resolved within the group's
/// template type, and meta groups recurse. Guard cycles in modded data.
fn collect_meta(ds: &Dataset, id: EntityId, visited: &mut HashSet<EntityId>, out: &mut BTreeMap<String, Vec<String>>) {
    if !visited.insert(id) {
        return;
    }
    let Some(r) = meta_record(ds, id) else { return };
    let Some(kind) = r.get("templateType").and_then(Value::as_str) else {
        return;
    };
    let names = str_list(r.get("templateNames"));
    if kind == META {
        for name in names {
            if let Some(child) = ds.find(META, &name) {
                collect_meta(ds, child, visited, out);
            }
        }
    } else {
        let entries = out.entry(kind.to_string()).or_default();
        entries.extend(names);
        entries.sort();
        entries.dedup();
    }
}

pub(crate) fn build(ds: &mut Dataset) -> Result<()> {
    let metas: Vec<EntityId> = ds.of_template(META).to_vec();
    // Disabled records listed by a meta template are registered anyway.
    let force_enabled: HashSet<String> = metas
        .iter()
        .filter_map(|&m| meta_record(ds, m))
        .flat_map(|r| str_list(r.get("templateNames")))
        .collect();

    let mut scenarios = Vec::new();
    for &m in &metas {
        let Some(r) = meta_record(ds, m) else {
            continue;
        };
        if r.get("newCampaignOptionCategory").and_then(Value::as_str) != Some("Scenario") {
            continue;
        }
        let list_priority = r.get("listPriority").and_then(Value::as_i64).unwrap_or(0);
        let mut instantiated: BTreeMap<String, Vec<String>> = BTreeMap::new();
        collect_meta(ds, m, &mut HashSet::new(), &mut instantiated);
        let start_time = instantiated
            .get(START_TIME)
            .and_then(|names| names.first())
            .and_then(|n| ds.find(START_TIME, n));
        scenarios.push(Scenario {
            idx: ScenarioIdx(0),
            meta: m,
            data_name: ds.entity(m).data_name.to_string(),
            friendly_name: r.get("friendlyName").and_then(Value::as_str).unwrap_or_default().to_string(),
            tags: str_list(r.get("scenarioTags")),
            listed: r.get("isNewCampaignOption").and_then(Value::as_bool).unwrap_or(false) && list_priority != 999,
            list_priority,
            required_dlc: str_list(r.get("requiredDLC")),
            localization_postfix: r.get("scenarioLocalizationPostfix").and_then(Value::as_str).map(str::to_string),
            start_time,
            instantiated,
        });
    }
    // Menu order: listed first, by priority; the default scenario wins ties.
    scenarios.sort_by_key(|s| (!s.listed, s.list_priority, s.data_name != DEFAULT_SCENARIO, s.data_name.clone()));
    if scenarios.len() > ScenarioIdx::MAX_SCENARIOS {
        return Err(Error::TooManyScenarios {
            count: scenarios.len(),
            max: ScenarioIdx::MAX_SCENARIOS,
        });
    }
    for (i, s) in scenarios.iter_mut().enumerate() {
        s.idx = ScenarioIdx(i as u8);
    }

    // Gate starting instances by (template, name), never an entire template type.
    let starting: HashSet<(&str, &str)> = scenarios
        .iter()
        .flat_map(|s| {
            s.instantiated
                .iter()
                .flat_map(|(kind, names)| names.iter().map(move |name| (kind.as_str(), name.as_str())))
        })
        .collect();
    // TIProjectTemplate.OrgGranted / TIFactionState.OnProjectCompleted can create
    // unique orgs too. Market CanSpawn only applies to randomized orgs; a
    // requiredTechName on a unique starting org does not make it spawnable.
    let granted_orgs: HashSet<&str> = ds
        .entities()
        .iter()
        .filter(|e| matches!(&*e.template, "TITechTemplate" | "TIProjectTemplate"))
        .flat_map(|e| &e.variants)
        .filter_map(|v| v.record.get("orgGranted").and_then(Value::as_str))
        .collect();
    // Start-date copies carry their scenario's `scenarioPrefix` (`2026_ALN`). A copy that no
    // scenario lists is never instantiated: it is not a runtime template either.
    let prefixes: Vec<String> = metas
        .iter()
        .filter_map(|&m| meta_record(ds, m))
        .filter_map(|r| r.get("scenarioPrefix").and_then(Value::as_str).filter(|p| !p.is_empty()).map(str::to_string))
        .collect();
    let unused_copy = |template: &str, name: &str| {
        matches!(template, "TINationTemplate" | "TIRegionTemplate" | "TIArmyTemplate" | START_TIME)
            && !starting.contains(&(template, name))
            && prefixes.iter().any(|p| name.starts_with(p.as_str()))
    };
    let n = ds.entities().len();
    let mut selected = Vec::with_capacity(scenarios.len());
    let mut presence = vec![ScenarioSet::default(); n];
    for s in &scenarios {
        let tags: Vec<Arc<str>> = s.tags.iter().map(|t| Arc::from(t.as_str())).collect();
        let row: Vec<u16> = ds
            .entities()
            .iter()
            .map(|e| {
                let listed = s
                    .instantiated
                    .get(&*e.template)
                    .is_some_and(|names| names.binary_search_by(|n| n.as_str().cmp(&e.data_name)).is_ok());
                // Check the resolved variant: randomized flags can vary by scenario.
                let forced = force_enabled.contains(&*e.data_name);
                let Some(index) = resolve(&e.variants, &tags, forced) else {
                    return ABSENT;
                };
                let record = &e.variants[index].record;
                let runtime = match &*e.template {
                    // TIFactionState market/tech completion create randomized orgs.
                    "TIOrgTemplate" => record.get("randomized").and_then(Value::as_bool) == Some(true) || granted_orgs.contains(&*e.data_name),
                    "TICouncilorTemplate" => record.get("randomized").and_then(Value::as_bool) == Some(true),
                    // TISpaceFleetState.CreateNewFleet / TITimeEvent creation.
                    "TISpaceFleetTemplate" => &*e.data_name == "GenericFleetTemplate",
                    "TITimeEventTemplate" => &*e.data_name == "GenericTimeEvent",
                    _ => false,
                };
                if (starting.contains(&(&*e.template, &*e.data_name)) && !listed && !runtime) || unused_copy(&e.template, &e.data_name) {
                    return ABSENT;
                }
                index as u16
            })
            .collect();
        for (i, v) in row.iter().enumerate() {
            if *v != ABSENT {
                presence[i].insert(s.idx);
            }
        }
        selected.push(row);
    }
    ds.scenarios = scenarios;
    ds.resolution = Resolution { selected, presence };
    Ok(())
}

/// Index of the variant the game would use for `scenario_tags`, if any.
pub fn resolve(variants: &[Variant], scenario_tags: &[Arc<str>], force_enabled: bool) -> Option<usize> {
    let registered: Vec<usize> = (0..variants.len())
        .filter(|&i| (!variants[i].disabled || force_enabled) && !variants[i].shadowed)
        .collect();
    let eligible = |i: usize| variants[i].tags.iter().all(|t| scenario_tags.contains(t));
    let &primary = registered.first()?;
    if !eligible(primary) && !registered.iter().any(|&i| eligible(i)) {
        return None;
    }
    if registered.len() == 1 {
        return Some(primary);
    }
    let overlap = |i: usize| variants[i].tags.iter().filter(|t| scenario_tags.contains(t)).count();
    let first_tag = |i: usize| scenario_tags.iter().position(|st| variants[i].tags.contains(st)).unwrap_or(usize::MAX);
    registered
        .iter()
        .copied()
        .min_by_key(|&i| (std::cmp::Reverse(overlap(i)), first_tag(i), variants[i].load_order))
}

impl Dataset {
    /// Scenarios in menu order (listed ones first; the default scenario is first among equals).
    pub fn scenarios(&self) -> &[Scenario] {
        &self.scenarios
    }

    pub fn scenario(&self, idx: ScenarioIdx) -> &Scenario {
        &self.scenarios[idx.index()]
    }

    pub fn scenario_by_name(&self, data_name: &str) -> Option<&Scenario> {
        self.scenarios.iter().find(|s| s.data_name == data_name)
    }

    pub fn default_scenario(&self) -> Option<&Scenario> {
        self.scenario_by_name(DEFAULT_SCENARIO).or_else(|| self.scenarios.first())
    }

    /// Variant used in a scenario, or `None` if the entity does not exist there.
    pub fn selected_variant(&self, scenario: ScenarioIdx, id: EntityId) -> Option<&Variant> {
        let v = *self.resolution.selected.get(scenario.index())?.get(id.index())?;
        (v != ABSENT).then(|| &self.entity(id).variants[v as usize])
    }

    pub(crate) fn selected_index(&self, scenario: ScenarioIdx, id: EntityId) -> Option<u16> {
        let v = *self.resolution.selected.get(scenario.index())?.get(id.index())?;
        (v != ABSENT).then_some(v)
    }

    /// Scenarios in which the entity exists.
    pub fn presence(&self, id: EntityId) -> ScenarioSet {
        self.resolution.presence.get(id.index()).copied().unwrap_or_default()
    }

    /// True if different scenarios use different records for this entity.
    pub fn varies_by_scenario(&self, id: EntityId) -> bool {
        let mut seen = None;
        for s in self.presence(id).iter() {
            let v = self.selected_index(s, id);
            if seen.is_some_and(|x| x != v) {
                return true;
            }
            seen = Some(v);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::LayerIdx;
    use serde_json::json;

    fn v(order: u32, tags: &[&str]) -> Variant {
        Variant {
            layer: LayerIdx::BASE,
            load_order: order,
            tags: tags.iter().map(|t| Arc::from(*t)).collect(),
            disabled: false,
            shadowed: false,
            record: json!({}),
        }
    }

    fn tags(t: &[&str]) -> Vec<Arc<str>> {
        t.iter().map(|x| Arc::from(*x)).collect()
    }

    #[test]
    fn resolution_rules() {
        // base untagged + 2003 [Millennium] + base [NotPostApoc]-only record elsewhere
        let vs = vec![v(0, &[]), v(1, &["Millennium"])];
        assert_eq!(resolve(&vs, &tags(&["NotPostApoc"]), false), Some(0));
        assert_eq!(resolve(&vs, &tags(&["Millennium", "NotPostApoc"]), false), Some(1));
        assert_eq!(resolve(&vs, &tags(&["PostApoc"]), false), Some(0));

        let not_post_apoc = vec![v(0, &["NotPostApoc"])];
        assert_eq!(resolve(&not_post_apoc, &tags(&["PostApoc"]), false), None);
        assert_eq!(resolve(&not_post_apoc, &tags(&["NotPostApoc"]), false), Some(0));

        let mut disabled = vec![v(0, &[])];
        disabled[0].disabled = true;
        assert_eq!(resolve(&disabled, &tags(&[]), false), None);
        assert_eq!(resolve(&disabled, &tags(&[]), true), Some(0));

        // tie on overlap -> earliest matching scenario tag wins, then load order
        let both = vec![v(0, &["B"]), v(1, &["A"])];
        assert_eq!(resolve(&both, &tags(&["A", "B"]), false), Some(1));
    }

    #[test]
    fn too_many_scenarios_is_an_error() {
        use crate::test_support::{scenario, try_dataset};
        let metas = |n: usize| (0..n).map(|i| scenario(&format!("S{i}"), i as i64, &[], None)).collect::<Vec<_>>();
        let max = ScenarioIdx::MAX_SCENARIOS;
        let err = try_dataset(&[("base", &[(META, metas(max + 1))])], json!({})).unwrap_err();
        assert!(matches!(err, Error::TooManyScenarios { count: 65, max: 64 }), "{err}");
        let ds = try_dataset(&[("base", &[(META, metas(max))])], json!({})).unwrap();
        assert_eq!(ds.scenarios().len(), max);
        assert!(ds.scenarios().iter().all(|s| ds.presence(s.meta).contains(s.idx)));
    }
}
