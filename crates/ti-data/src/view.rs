//! Scoped access to a dataset: optionally one scenario and/or one faction.
//!
//! * `scenario: Some(s)` – exactly the records the game uses in scenario `s`.
//! * `scenario: None` – overview over all scenarios: every entity that exists in
//!   at least one scenario, shown with the record of the default scenario when
//!   it exists there (else of the first scenario that has it). Use
//!   [`View::annotations`] to point out scenario/faction specifics.
//! * `faction: Some(f)` – additionally hides projects locked to other factions
//!   (`factionPrereq`), as the game does.

use std::collections::HashSet;
use std::sync::OnceLock;

use serde_json::Value;

use crate::dataset::{Dataset, EntityId, Reference, Variant};
use crate::graph::ResearchGraph;
use crate::model::{self, Effect, HabModule, Kind, PartKind, Project, ShipPart, Tech};
use crate::scenario::{Scenario, ScenarioIdx, ScenarioSet};
use crate::text::{Texts, field, to_plain};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Scope {
    pub scenario: Option<ScenarioIdx>,
    /// A `TIFactionTemplate` entity.
    pub faction: Option<EntityId>,
}

impl Scope {
    pub const OVERVIEW: Scope = Scope { scenario: None, faction: None };

    pub fn scenario(s: ScenarioIdx) -> Scope {
        Scope { scenario: Some(s), faction: None }
    }

    pub fn with_faction(self, faction: EntityId) -> Scope {
        Scope { faction: Some(faction), ..self }
    }
}

/// What makes an entity scenario- or faction-specific.
#[derive(Debug, Clone, Default)]
pub struct Annotations {
    /// Scenarios in which the entity exists.
    pub scenarios: ScenarioSet,
    /// Exists in some scenarios only.
    pub scenario_specific: bool,
    /// Different scenarios use different records (stats/prereqs may differ).
    pub varies_by_scenario: bool,
    /// Record used here comes from a DLC folder.
    pub from_dlc: bool,
    /// Projects: factions allowed to research it (`factionPrereq`); empty = everyone.
    pub factions: Vec<String>,
    /// Projects: faction guaranteed to get it (`factionAlways`; may be `"Random"`).
    pub faction_always: Option<String>,
    /// Projects: nation that must exist (`requiresNation`).
    pub requires_nation: Option<String>,
}

/// Why something appears in [`View::unlocks`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UnlockKind {
    /// Tech/project that lists this one as a prereq (`alternative`: via `altPrereqN`).
    Research { alternative: bool },
    ShipPart(PartKind),
    HabModule,
    /// Org that becomes available on the market (`requiredTechName`).
    Org,
    /// Org granted on completion (`orgGranted`).
    GrantedOrg,
    /// Territorial claim (`TIBilateralTemplate.projectUnlockName`).
    Claim,
    /// Trait gained via the project (`TITraitTemplate.projectDataName`).
    Trait,
    /// Codex entry (`unlockTech`).
    Codex,
    /// Mission knowledge (`TIMissionTemplate.knowledgeProject`).
    Mission,
    /// Objective targeting it (`targetProjectTemplateName`/`targetTechTemplateName`).
    Objective,
}

#[derive(Debug, Clone)]
pub struct Unlock {
    pub kind: UnlockKind,
    pub entity: EntityId,
}

pub struct View<'d> {
    ds: &'d Dataset,
    scope: Scope,
    all_scenarios: ScenarioSet,
    graph: OnceLock<ResearchGraph>,
}

impl Dataset {
    pub fn view(&self, scope: Scope) -> View<'_> {
        let mut all = ScenarioSet::default();
        for s in self.scenarios() {
            all.insert(s.idx);
        }
        View { ds: self, scope, all_scenarios: all, graph: OnceLock::new() }
    }

    pub fn overview(&self) -> View<'_> {
        self.view(Scope::OVERVIEW)
    }
}

impl<'d> View<'d> {
    pub fn dataset(&self) -> &'d Dataset {
        self.ds
    }

    pub fn scope(&self) -> Scope {
        self.scope
    }

    pub fn scenario(&self) -> Option<&'d Scenario> {
        self.scope.scenario.map(|s| self.ds.scenario(s))
    }

    /// Localization postfix of the selected scenario (`.2003`, …).
    pub fn localization_postfix(&self) -> Option<&'d str> {
        self.scenario().and_then(|s| s.localization_postfix.as_deref())
    }

    /// Index of the variant in use, ignoring the faction filter.
    fn variant_index(&self, id: EntityId) -> Option<u16> {
        match self.scope.scenario {
            Some(s) => self.ds.selected_index(s, id),
            None => {
                let presence = self.ds.presence(id);
                let preferred = self.ds.default_scenario().map(|s| s.idx).filter(|s| presence.contains(*s));
                preferred.or_else(|| presence.iter().next()).and_then(|s| self.ds.selected_index(s, id))
            }
        }
    }

    /// The record in use for this entity, if it exists in this scope (ignores the faction filter).
    pub fn variant(&self, id: EntityId) -> Option<&'d Variant> {
        let i = self.variant_index(id)?;
        Some(&self.ds.entity(id).variants[i as usize])
    }

    pub fn record(&self, id: EntityId) -> Option<&'d Value> {
        self.variant(id).map(|v| &v.record)
    }

    /// Does the selected faction get to see this entity? (Only projects are faction-locked.)
    pub fn faction_allows(&self, id: EntityId) -> bool {
        self.scope.faction.is_none() || self.record(id).is_some_and(|r| self.faction_allows_record(id, r))
    }

    fn faction_allows_record(&self, id: EntityId, record: &Value) -> bool {
        let Some(faction) = self.scope.faction else { return true };
        if &*self.ds.entity(id).template != model::PROJECT {
            return true;
        }
        let allowed = record.get("factionPrereq").and_then(Value::as_array);
        match allowed {
            Some(list) if !list.is_empty() => {
                let name = &*self.ds.entity(faction).data_name;
                list.iter().any(|v| v.as_str() == Some(name))
            }
            _ => true,
        }
    }

    /// Exists in this scope, ignoring the faction filter.
    pub fn exists(&self, id: EntityId) -> bool {
        self.variant_index(id).is_some()
    }

    /// Exists in this scope and is visible to the selected faction.
    pub fn contains(&self, id: EntityId) -> bool {
        self.variant_index(id).is_some() && self.faction_allows(id)
    }

    pub fn find(&self, template: &str, data_name: &str) -> Option<EntityId> {
        self.ds.find(template, data_name).filter(|id| self.contains(*id))
    }

    /// Resolve a prereq name the way the game does: a tech or a project.
    pub fn find_research(&self, data_name: &str) -> Option<EntityId> {
        self.find(model::TECH, data_name).or_else(|| self.find(model::PROJECT, data_name))
    }

    /// Like [`View::find_research`], but ignoring the faction filter (the game's
    /// template lookup; faction-hidden projects still exist).
    pub fn resolve_research(&self, data_name: &str) -> Option<EntityId> {
        [model::TECH, model::PROJECT]
            .into_iter()
            .filter_map(|t| self.ds.find(t, data_name))
            .find(|id| self.exists(*id))
    }

    /// Entities of a template that exist in this scope.
    pub fn entities(&self, template: &str) -> impl Iterator<Item = EntityId> + '_ {
        self.ds.of_template(template).iter().copied().filter(|id| self.contains(*id))
    }

    /// All entities in this scope, across templates.
    pub fn all_entities(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.ds.entities().iter().map(|e| e.id).filter(|id| self.contains(*id))
    }

    pub fn annotations(&self, id: EntityId) -> Annotations {
        let scenarios = self.ds.presence(id);
        let mut a = Annotations {
            scenarios,
            scenario_specific: !scenarios.is_empty() && scenarios != self.all_scenarios,
            varies_by_scenario: self.ds.varies_by_scenario(id),
            from_dlc: self.variant(id).is_some_and(|v| self.ds.layer(v.layer).is_dlc),
            ..Default::default()
        };
        if &*self.ds.entity(id).template == model::PROJECT
            && let Some(p) = self.project(id) {
                a.factions = p.faction_prereq;
                a.faction_always = p.faction_always;
                a.requires_nation = p.requires_nation;
            }
        a
    }

    pub fn kind(&self, id: EntityId) -> Kind {
        Kind::of_template(&self.ds.entity(id).template)
    }

    // ---- typed records --------------------------------------------------

    pub fn tech(&self, id: EntityId) -> Option<Tech> {
        (&*self.ds.entity(id).template == model::TECH).then(|| model::parse(self.record(id)?))?
    }

    pub fn project(&self, id: EntityId) -> Option<Project> {
        (&*self.ds.entity(id).template == model::PROJECT).then(|| Some(Project::from_record(self.record(id)?)))?
    }

    pub fn effect(&self, id: EntityId) -> Option<Effect> {
        (&*self.ds.entity(id).template == model::EFFECT).then(|| model::parse(self.record(id)?))?
    }

    pub fn ship_part(&self, id: EntityId) -> Option<(PartKind, ShipPart)> {
        match self.kind(id) {
            Kind::ShipPart(k) => Some((k, model::parse(self.record(id)?))),
            _ => None,
        }
    }

    pub fn hab_module(&self, id: EntityId) -> Option<HabModule> {
        (&*self.ds.entity(id).template == model::HAB_MODULE).then(|| model::parse(self.record(id)?))?
    }

    // ---- texts ----------------------------------------------------------

    pub fn name(&self, texts: &Texts, id: EntityId) -> String {
        let e = self.ds.entity(id);
        let record = self.record(id).or_else(|| e.variants.first().map(|v| &v.record));
        texts.display_name(&e.template, &e.data_name, record, self.localization_postfix())
    }

    /// Raw localized text (rich-text markup kept) of a field such as [`field::SUMMARY`].
    pub fn text_raw<'t>(&self, texts: &'t Texts, id: EntityId, field: &str) -> Option<&'t str> {
        let e = self.ds.entity(id);
        let record = self.record(id).or_else(|| e.variants.first().map(|v| &v.record));
        texts
            .entity_field(&e.template, &e.data_name, record, field, self.localization_postfix())
            .filter(|t| !t.trim().is_empty())
    }

    /// Plain text of a field, with tech-quote leader tokens resolved and the project
    /// summary sentinels replaced (see [`View::text_source`]).
    pub fn text(&self, texts: &Texts, id: EntityId, field: &str) -> Option<String> {
        let (source, field) = self.text_source(texts, id, field);
        let raw = self.text_raw(texts, source, field)?;
        let raw = if field == field::QUOTE { texts.replace_leader_tokens(raw) } else { raw.to_string() };
        Some(to_plain(&raw))
    }

    /// Entity and field whose text the game actually shows for `(id, field)`.
    ///
    /// `TIProjectTemplate.summary`: the sentinel `<habmodule>` is replaced by the
    /// description of the hab module unlocked by the project (only if there is exactly
    /// one), `<shipmodule>` by the description of the first unlocked ship part.
    /// Otherwise `(id, field)` itself.
    pub fn text_source<'f>(&self, texts: &Texts, id: EntityId, field: &'f str) -> (EntityId, &'f str) {
        if field != field::SUMMARY || &*self.ds.entity(id).template != model::PROJECT {
            return (id, field);
        }
        let e = self.ds.entity(id);
        let record = self.record(id).or_else(|| e.variants.first().map(|v| &v.record));
        let Some(raw) = texts.entity_field(&e.template, &e.data_name, record, field, self.localization_postfix()) else {
            return (id, field);
        };
        let modules: Vec<EntityId> = match raw {
            "<habmodule>" => {
                let habs = self.modules_unlocked_by(id, |k| k == Kind::HabModule);
                if habs.len() == 1 { habs } else { Vec::new() }
            }
            "<shipmodule>" => self.modules_unlocked_by(id, |k| matches!(k, Kind::ShipPart(_))),
            _ => Vec::new(),
        };
        match modules.first() {
            Some(&m) => (m, field::DESCRIPTION),
            None => (id, field),
        }
    }

    /// Entities of the given kinds whose `requiredProjectName` is `project`, in the
    /// game's iteration order (template type load order, then entity load order).
    fn modules_unlocked_by(&self, project: EntityId, kind: impl Fn(Kind) -> bool) -> Vec<EntityId> {
        let first_load = |id: EntityId| self.ds.entity(id).variants.first().map_or(u32::MAX, |v| v.load_order);
        let template_load = |id: EntityId| {
            let t = &*self.ds.entity(id).template;
            self.ds.of_template(t).first().map_or(u32::MAX, |f| first_load(*f))
        };
        let mut out: Vec<EntityId> = self
            .references_to(project)
            .into_iter()
            .filter(|r| &*r.path == "requiredProjectName" && kind(self.kind(r.from)))
            .map(|r| r.from)
            .collect();
        out.sort_by_key(|id| (template_load(*id), first_load(*id)));
        out.dedup();
        out
    }

    // ---- assets ---------------------------------------------------------

    /// Icon resource string: the record's own icon, or the tech category icon for research.
    pub fn icon_resource(&self, id: EntityId) -> Option<String> {
        let record = self.record(id)?;
        for key in ["iconResource", "stationIconResource", "baseIconResource", "missionIconImagePath", "flagResource", "councilIcon64"] {
            if let Some(r) = record.get(key).and_then(Value::as_str).filter(|r| !r.is_empty()) {
                return Some(r.to_string());
            }
        }
        if self.kind(id).is_research() {
            let category: model::TechCategory = record
                .get("techCategory")
                .and_then(|c| model::TechCategory::parse(c.as_str()?))
                .unwrap_or_default();
            return category.icon_resource().map(str::to_string);
        }
        None
    }

    // ---- relations ------------------------------------------------------

    /// The tech/project prerequisite graph of this scope (built on first use).
    pub fn research_graph(&self) -> &ResearchGraph {
        self.graph.get_or_init(|| ResearchGraph::build(self))
    }

    /// The referencing record is in use and both ends exist (and pass the faction
    /// filter) in the same scenario: the selected one, or – in the overview – any.
    fn reference_in_scope(&self, r: &Reference) -> bool {
        if !self.contains(r.from) || !self.contains(r.to) {
            return false;
        }
        let in_scenario = |s: ScenarioIdx| {
            if self.ds.selected_index(s, r.from) != Some(r.variant) {
                return false;
            }
            if self.scope.faction.is_none() {
                return self.ds.selected_index(s, r.to).is_some();
            }
            let record = |id| self.ds.selected_variant(s, id).map(|v| &v.record);
            record(r.from).is_some_and(|v| self.faction_allows_record(r.from, v))
                && record(r.to).is_some_and(|v| self.faction_allows_record(r.to, v))
        };
        match self.scope.scenario {
            Some(s) => in_scenario(s),
            None => {
                let both = ScenarioSet(self.ds.presence(r.from).0 & self.ds.presence(r.to).0);
                both.iter().any(in_scenario)
            }
        }
    }

    /// Incoming references from records in use in this scope (one per referencing entity and path).
    pub fn references_to(&self, id: EntityId) -> Vec<&'d Reference> {
        let mut seen = HashSet::new();
        self.ds
            .references_to(id)
            .filter(|r| self.reference_in_scope(r))
            .filter(|r| seen.insert((r.from, r.path.clone())))
            .collect()
    }

    /// Outgoing references of the record(s) in use in this scope (one per target and path).
    pub fn references_from(&self, id: EntityId) -> Vec<&'d Reference> {
        let mut seen = HashSet::new();
        self.ds
            .references_from(id)
            .filter(|r| self.reference_in_scope(r))
            .filter(|r| seen.insert((r.to, r.path.clone())))
            .collect()
    }

    /// Effects (`effects[]`) of a tech or project, in order.
    pub fn effects(&self, id: EntityId) -> Vec<EntityId> {
        self.string_list(id, "effects").filter_map(|n| self.find(model::EFFECT, n)).collect()
    }

    fn string_list(&self, id: EntityId, key: &str) -> impl Iterator<Item = &'d str> {
        self.record(id)
            .and_then(|r| r.get(key))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
    }

    /// Everything that researching `id` makes available or leads to.
    pub fn unlocks(&self, id: EntityId) -> Vec<Unlock> {
        let mut out: Vec<Unlock> = self
            .research_graph()
            .dependents(id)
            .iter()
            .map(|d| Unlock { kind: UnlockKind::Research { alternative: d.alternative }, entity: d.node })
            .collect();
        for r in self.references_to(id) {
            let from_template = &*self.ds.entity(r.from).template;
            let kind = match (Kind::of_template(from_template), &*r.path) {
                (Kind::ShipPart(k), "requiredProjectName") => UnlockKind::ShipPart(k),
                (Kind::HabModule, "requiredProjectName") => UnlockKind::HabModule,
                (Kind::Org, "requiredTechName") => UnlockKind::Org,
                (Kind::Trait, "projectDataName") => UnlockKind::Trait,
                (Kind::Codex, "unlockTech") => UnlockKind::Codex,
                (Kind::Mission, "knowledgeProject") => UnlockKind::Mission,
                (Kind::Objective, "targetProjectTemplateName" | "targetTechTemplateName") => UnlockKind::Objective,
                (Kind::Other, "projectUnlockName") if from_template == "TIBilateralTemplate" => UnlockKind::Claim,
                _ => continue,
            };
            out.push(Unlock { kind, entity: r.from });
        }
        if let Some(org) = self.record(id).and_then(|r| r.get("orgGranted")).and_then(Value::as_str)
            && let Some(o) = self.find(model::ORG, org) {
                out.push(Unlock { kind: UnlockKind::GrantedOrg, entity: o });
            }
        out.sort_by_key(|u| (u.kind, u.entity));
        out.dedup_by(|a, b| a.kind == b.kind && a.entity == b.entity);
        out
    }

    /// What unlocks this entity: the required project of a ship part/hab module, the
    /// required tech of an org, …
    pub fn unlocked_by(&self, id: EntityId) -> Vec<EntityId> {
        let Some(record) = self.record(id) else { return Vec::new() };
        let mut out = Vec::new();
        for key in ["requiredProjectName", "requiredTechName", "projectDataName", "unlockTech", "knowledgeProject", "projectUnlockName"] {
            if let Some(name) = record.get(key).and_then(Value::as_str) {
                out.extend(self.find_research(name));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::scenario::META;
    use crate::test_support::{dataset, id, scenario};
    use crate::text::Texts;

    #[test]
    fn project_summary_sentinels_use_the_unlocked_module() {
        let ds = dataset(
            &[
                (META, vec![scenario("S", 1, &[], None)]),
                (
                    model::PROJECT,
                    vec![json!({"dataName": "Ship"}), json!({"dataName": "Hab"}), json!({"dataName": "Habs"}), json!({"dataName": "Plain"})],
                ),
                // template load order decides which ship part comes first
                ("TIShipArmorTemplate", vec![json!({"dataName": "Armor", "requiredProjectName": "Ship"})]),
                (
                    "TIUtilityModuleTemplate",
                    vec![
                        json!({"dataName": "Util2", "requiredProjectName": "Ship"}),
                        json!({"dataName": "Util1", "requiredProjectName": "Ship"}),
                    ],
                ),
                (
                    model::HAB_MODULE,
                    vec![
                        json!({"dataName": "Mod", "requiredProjectName": "Hab"}),
                        json!({"dataName": "ModA", "requiredProjectName": "Habs"}),
                        json!({"dataName": "ModB", "requiredProjectName": "Habs"}),
                    ],
                ),
            ],
            json!({
                "TIProjectTemplate.summary.Ship": "<shipmodule>",
                "TIProjectTemplate.summary.Hab": "<habmodule>",
                "TIProjectTemplate.summary.Habs": "<habmodule>",
                "TIProjectTemplate.summary.Plain": "Plain <b>text</b>",
                "TIShipArmorTemplate.description.Armor": "Armor description",
                "TIUtilityModuleTemplate.description.Util2": "Util2 description",
                "TIHabModuleTemplate.description.Mod": "Module description",
                "TIHabModuleTemplate.description.ModA": "A",
            }),
        );
        let texts = Texts::load(&ds, "en").unwrap();
        let view = ds.overview();
        let p = |n| id(&ds, model::PROJECT, n);
        let summary = |n| view.text(&texts, p(n), field::SUMMARY);
        assert_eq!(summary("Ship").as_deref(), Some("Armor description"));
        assert_eq!(view.text_source(&texts, p("Ship"), field::SUMMARY), (id(&ds, "TIShipArmorTemplate", "Armor"), field::DESCRIPTION));
        assert_eq!(summary("Hab").as_deref(), Some("Module description"));
        // several hab modules: the game keeps the sentinel
        assert_eq!(view.text_source(&texts, p("Habs"), field::SUMMARY), (p("Habs"), field::SUMMARY));
        assert_eq!(summary("Plain").as_deref(), Some("Plain text"));
    }

    #[test]
    fn overview_references_need_one_scenario_with_both_ends() {
        let ds = dataset(
            &[
                (META, vec![scenario("SA", 1, &["A"], None), scenario("SB", 2, &["B"], None)]),
                (
                    model::PROJECT,
                    vec![
                        json!({"dataName": "OnlyB", "scenarioTags": ["B"]}),
                        json!({"dataName": "Both"}),
                        json!({"dataName": "Locked", "scenarioTags": ["A"]}),
                        json!({"dataName": "Locked", "factionPrereq": ["F1"], "scenarioTags": ["B"]}),
                    ],
                ),
                (
                    model::HAB_MODULE,
                    vec![
                        json!({"dataName": "ModA", "scenarioTags": ["A"], "requiredProjectName": "OnlyB"}),
                        json!({"dataName": "ModBoth", "scenarioTags": ["A"], "requiredProjectName": "Both"}),
                        json!({"dataName": "ModLocked", "scenarioTags": ["B"], "requiredProjectName": "Locked"}),
                    ],
                ),
                (model::FACTION, vec![json!({"dataName": "F1"}), json!({"dataName": "F2"})]),
            ],
            json!({}),
        );
        let view = ds.overview();
        let p = |n| id(&ds, model::PROJECT, n);
        let from = |v: &View<'_>, n| v.references_to(p(n)).iter().map(|r| r.from).collect::<Vec<_>>();
        // ModA exists only in A, OnlyB only in B: never together
        assert!(from(&view, "OnlyB").is_empty());
        assert_eq!(from(&view, "Both"), vec![id(&ds, model::HAB_MODULE, "ModBoth")]);
        // F2 sees Locked (in A) and ModLocked (in B), but never both in the same scenario
        let f2 = view.dataset().find(model::FACTION, "F2").unwrap();
        let f2_view = ds.view(Scope::OVERVIEW.with_faction(f2));
        assert!(f2_view.contains(p("Locked")) && f2_view.contains(id(&ds, model::HAB_MODULE, "ModLocked")));
        assert!(from(&f2_view, "Locked").is_empty());
        let f1 = view.dataset().find(model::FACTION, "F1").unwrap();
        assert_eq!(from(&ds.view(Scope::OVERVIEW.with_faction(f1)), "Locked").len(), 1);
    }

    #[test]
    fn references_are_deduplicated_by_entity_and_path() {
        let ds = dataset(
            &[
                (META, vec![scenario("S", 1, &[], None)]),
                (model::TECH, vec![json!({"dataName": "T", "effects": ["E1", "E2", "E1"]})]),
                (model::EFFECT, vec![json!({"dataName": "E1"}), json!({"dataName": "E2"})]),
            ],
            json!({}),
        );
        let view = ds.overview();
        let t = id(&ds, model::TECH, "T");
        let to: Vec<&str> = view.references_from(t).iter().map(|r| &*ds.entity(r.to).data_name).collect();
        assert_eq!(to, ["E1", "E2"]);
        assert_eq!(view.references_to(id(&ds, model::EFFECT, "E1")).len(), 1);
    }
}
