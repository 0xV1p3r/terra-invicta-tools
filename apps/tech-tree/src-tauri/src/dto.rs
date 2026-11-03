//! Data transfer types of the frontend API. Mirrored 1:1 in `src/lib/api.ts`;
//! change both together. All JSON is camelCase.
//!
//! Entities are addressed by a stable string key `"<Template>:<dataName>"`
//! (e.g. `"TITechTemplate:AdAstra"`); `ti_data::EntityId`s never leave the backend.

use serde::{Deserialize, Serialize};

/// Which slice of the data to show. `scenario`/`faction` are `TIMetaTemplate` /
/// `TIFactionTemplate` dataNames; `None` = overview over all scenarios / no faction filter.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeArgs {
    pub scenario: Option<String>,
    pub faction: Option<String>,
    /// Language code such as `en`, `deu`; falls back to English.
    pub lang: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub build_id: String,
    pub game_version: Option<String>,
    pub data_dir: String,
    pub languages: Vec<String>,
    /// Game menu order (listed ones first, by `listPriority`).
    pub scenarios: Vec<ScenarioInfo>,
    pub factions: Vec<FactionInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioInfo {
    /// dataName, e.g. `ModernScenario`; pass back in `ScopeArgs::scenario`.
    pub name: String,
    pub label: String,
    /// Shown in the game's new-game menu (others are skirmish/editor-only).
    pub listed: bool,
    pub dlc: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactionInfo {
    pub name: String,
    pub label: String,
    /// Asset path (relative to the build's assets dir) of the faction logo, if any.
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Tech,
    Project,
}

/// Options for the full tree.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeFilter {
    pub techs: bool,
    pub projects: bool,
    /// `TechCategory` names (`Energy`, `InformationScience`, …); empty = all.
    #[serde(default)]
    pub categories: Vec<String>,
    /// Property ids (see `properties.rs`); a node matches if it has any of them. Empty = all.
    #[serde(default)]
    pub properties: Vec<String>,
}

/// A filterable property of techs/projects (see `properties.rs` for the id scheme).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyInfo {
    pub id: String,
    /// English label, e.g. "Increases control point capacity".
    pub label: String,
    /// `effect` (changes a quantity), `ability` or `unlock`.
    pub group: String,
    /// Techs/projects in scope that have it.
    pub count: usize,
    /// Rendered description of one effect giving it (effects only).
    pub example: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    /// Requirements of the root.
    Up,
    /// Everything building on the root.
    Down,
    Both,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtreeArgs {
    pub root: String,
    pub direction: Direction,
    /// Follow `altPrereqN` alternatives too.
    pub alternatives: bool,
    pub max_depth: Option<usize>,
}

/// A laid-out graph, ready to render. Positions are top-left corners in pixels for
/// nodes of `nodeWidth` × `nodeHeight`; columns run left → right (prereqs first, cheaper first).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeGraph {
    pub nodes: Vec<TreeNode>,
    pub edges: Vec<TreeEdge>,
    pub node_width: f64,
    pub node_height: f64,
    /// Subtree root, if any.
    pub root: Option<String>,
    /// Sum of research cost over all nodes (subtrees: everything needed).
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    pub key: String,
    pub data_name: String,
    pub kind: NodeKind,
    pub name: String,
    /// `TechCategory` name.
    pub category: String,
    pub cost: f64,
    /// Layout column (0 = leftmost).
    pub column: usize,
    pub x: f64,
    pub y: f64,
    pub flags: NodeFlags,
    /// Property ids (see `PropertyInfo`), sorted.
    pub properties: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeFlags {
    /// Exists only in some scenarios.
    pub scenario_specific: bool,
    /// Differs between scenarios.
    pub varies_by_scenario: bool,
    /// Record comes from DLC.
    pub dlc: bool,
    /// Project restricted to some factions (`factionPrereq`).
    pub faction_locked: bool,
    pub end_game: bool,
    pub repeatable: bool,
    /// All requirement chains can be met in this scope.
    pub obtainable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeEdge {
    /// Prerequisite.
    pub from: String,
    /// Node requiring it.
    pub to: String,
    /// Via `altPrereqN` (render dashed).
    pub alternative: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityRef {
    pub key: String,
    pub template: String,
    pub data_name: String,
    pub name: String,
    /// Coarse kind for grouping/icons: `tech`, `project`, `effect`, `habModule`, `shipPart`,
    /// `org`, `trait`, `faction`, `objective`, `mission`, `codex`, `other`.
    pub kind: String,
    /// Ship part subtype (`drive`, `laser`, …) when `kind == "shipPart"`.
    pub subtype: Option<String>,
    pub category: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub entity: EntityRef,
    pub score: u32,
    /// `name`, `dataName`, `text` or `value`.
    pub matched: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityDetail {
    pub entity: EntityRef,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub quote: Option<String>,
    pub annotations: AnnotationsDto,
    /// AND-list of OR-groups (techs/projects only).
    pub requirements: Vec<RequirementGroup>,
    /// Listed prereq names that don't exist in this scope (the game ignores them).
    pub dropped_prereqs: Vec<String>,
    pub effects: Vec<EffectLine>,
    pub unlocks: Vec<UnlockItem>,
    pub unlocked_by: Vec<EntityRef>,
    pub research_cost: Option<f64>,
    /// Research cost of everything needed, including this (listed prereqs only).
    pub total_cost: Option<f64>,
    /// The raw record in use (for a "data" tab).
    pub record: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationsDto {
    /// Scenario dataNames the entity exists in.
    pub scenarios: Vec<String>,
    pub scenario_specific: bool,
    pub varies_by_scenario: bool,
    pub dlc: bool,
    pub factions: Vec<String>,
    pub faction_always: Option<String>,
    pub requires_nation: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementGroup {
    /// Any one option satisfies the group.
    pub options: Vec<EntityRef>,
    /// Parallel to `options`: option comes from `altPrereqN`.
    pub alternative: Vec<bool>,
    /// Prereqs that exist but can't be completed in this scope (other faction's project).
    pub hidden: Vec<EntityRef>,
    /// `altPrereqN` names that don't exist in this scope.
    pub unresolved: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectLine {
    pub entity: EntityRef,
    /// Rendered, plain-text description (None if the game hides it).
    pub text: Option<String>,
    /// The description split into text, numbers and inline sprites (empty if hidden).
    pub segments: Vec<EffectSegment>,
    /// What the effect changes ("Materials research") or grants ("Can found outposts").
    pub label: Option<String>,
    /// Change of `label` in its own terms: `+100%`, `−15%`, `+70`, or a new level `→ 12`.
    /// None for abilities and when the label can't express it.
    pub modifier: Option<String>,
    /// For the acting faction: `good`, `bad` or `neutral`.
    pub sentiment: String,
    /// Icon group: `research`, `priority`, `mission`, `control`, `councilor`, `army`, `ship`,
    /// `mining`, `missionControl`, `exploration`, `transfer`, `hab`, `alien`, `environment`,
    /// `economy`, `politics`, `ability`, `other`.
    pub domain: String,
    /// Research category for `research` effects on one science (`Materials`, …).
    pub category: Option<String>,
    /// Who it applies to, if not just the acting faction ("All human factions").
    pub target: Option<String>,
    /// Matching game icon (asset path, like `EntityRef::icon`); `domain` is the fallback.
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectSegment {
    /// `text`, `value` (substituted number) or `sprite` (inline icon name such as `metal`).
    pub kind: String,
    pub text: String,
    /// Image of a `sprite` (asset path, like `EntityRef::icon`).
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockItem {
    /// `research`, `researchAlternative`, `shipPart`, `habModule`, `org`, `grantedOrg`,
    /// `claim`, `trait`, `codex`, `mission`, `objective`.
    pub kind: String,
    pub entity: EntityRef,
}
