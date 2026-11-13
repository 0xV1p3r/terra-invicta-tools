//! Source of truth for frontend DTOs. Regenerate checked-in TypeScript with
//! `cargo test -p ti-tech-tree export_bindings` from the workspace root.
//! `src/lib/api.ts` re-exports these bindings. All JSON is camelCase.
//!
//! Entities are addressed by a stable string key `"<Template>:<dataName>"`
//! (e.g. `"TITechTemplate:AdAstra"`); `ti_data::EntityId`s never leave the backend.

use serde::{Deserialize, Serialize};

/// Which slice of the data to show. `scenario`/`faction` are `TIMetaTemplate` /
/// `TIFactionTemplate` dataNames; `None` = overview over all scenarios / no faction filter.
#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct ScopeArgs {
    pub scenario: Option<String>,
    pub faction: Option<String>,
    /// Language code such as `en`, `deu`; falls back to English.
    pub lang: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub build_id: String,
    pub game_version: Option<String>,
    /// The loaded build is the data root's latest one.
    pub latest: bool,
    /// Build id pinned in the settings; `None` = always the latest.
    pub pinned_build: Option<String>,
    pub data_dir: String,
    pub data_dir_source: DataDirSource,
    pub languages: Vec<String>,
    /// Game menu order (listed ones first, by `listPriority`).
    pub scenarios: Vec<ScenarioInfo>,
    pub factions: Vec<FactionInfo>,
}

/// Where the data directory came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub enum DataDirSource {
    /// The `TI_DATA_DIR` environment variable.
    Env,
    /// Chosen in the app (persisted settings).
    Settings,
    /// The app's own data root (`<app data dir>/data`), where in-app extraction writes.
    Default,
    /// A `data/` directory above the working directory or the executable.
    Found,
}

/// A build listed in the data root's `index.json` (`list_builds`).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    /// Directory name inside the data root.
    pub dir: String,
    pub build_id: String,
    /// Steam update time, Unix seconds.
    #[cfg_attr(test, ts(type = "number | null"))]
    pub last_updated: Option<i64>,
    /// Version shown by the game, from the build's manifest.
    pub game_version: Option<String>,
    /// The build currently loaded.
    pub loaded: bool,
    /// The data root's latest build.
    pub latest: bool,
}

/// Disk usage of a build directory (`get_build_sizes`).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct BuildSize {
    pub dir: String,
    #[cfg_attr(test, ts(type = "number"))]
    pub bytes: u64,
}

/// A Terra Invicta install (`detect_game`, `inspect_game`).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    pub root: String,
    /// Version shown by the game, e.g. `1.0.53b`.
    pub version: Option<String>,
    /// Steam build id; `None` outside Steam.
    pub build_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub enum ImageFormat {
    #[default]
    Png,
    /// Lossless WebP: smaller files, same pixels.
    Webp,
}

/// What `extract_game_data` extracts. Icons, logos and flags are always included.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct ExtractArgs {
    /// Root of the install (from `detect_game` / `inspect_game`).
    pub game_dir: String,
    /// Also the large artwork: illustrations, councilor portraits, hab modules (about 1.5 GB).
    pub large_art: bool,
    pub image_format: ImageFormat,
    /// Replace an existing extraction of the same build.
    pub force: bool,
}

/// Extraction phases, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub enum ExtractStage {
    Templates,
    Locale,
    Schema,
    Images,
    Finish,
}

/// Progress of `extract_game_data`, streamed over its channel.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExtractEvent {
    /// A phase started.
    Stage { stage: ExtractStage },
    /// `done` of `total` units of the current phase, with an optional label (e.g. the bundle).
    Step {
        #[cfg_attr(test, ts(type = "number"))]
        done: u64,
        #[cfg_attr(test, ts(type = "number"))]
        total: u64,
        label: Option<String>,
    },
    /// A log line.
    Log { message: String },
}

/// Outcome of `extract_game_data`; the new build is loaded by then.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct ExtractResult {
    pub build_id: String,
    /// `false` when the build was already extracted (nothing written).
    pub extracted: bool,
    /// Data issues found (see the build's `reports/issues.json`).
    #[cfg_attr(test, ts(type = "number"))]
    pub issues: usize,
    #[cfg_attr(test, ts(type = "number"))]
    pub parse_errors: usize,
    pub seconds: f64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
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
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct FactionInfo {
    pub name: String,
    pub label: String,
    /// Asset path (relative to the build's assets dir) of the faction logo, if any.
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Tech,
    Project,
}

/// Options for the full tree.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct TreeFilter {
    pub techs: bool,
    pub projects: bool,
    /// `TechCategory` names (`Energy`, `InformationScience`, …); empty = all.
    #[serde(default)]
    #[cfg_attr(test, ts(as = "Vec<TechCategory>"))]
    pub categories: Vec<String>,
    /// Property ids (see `properties.rs`); a node matches if it has any of them. Empty = all.
    #[serde(default)]
    pub properties: Vec<String>,
}

/// A filterable property of techs/projects (see `properties.rs` for the id scheme).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct PropertyInfo {
    pub id: String,
    /// English label, e.g. "Increases control point capacity".
    pub label: String,
    /// `effect` (changes a quantity), `ability` or `unlock`.
    #[cfg_attr(test, ts(as = "PropertyGroup"))]
    pub group: String,
    /// Techs/projects in scope that have it.
    pub count: usize,
    /// Rendered description of one effect giving it (effects only).
    pub example: Option<String>,
    /// `good`/`bad`/`neutral` for the acting faction (see `EffectLine::sentiment`).
    #[cfg_attr(test, ts(as = "EffectSentiment"))]
    pub sentiment: String,
    /// Icon group (see `EffectLine::domain`); None for unlocks.
    #[cfg_attr(test, ts(as = "Option<EffectDomain>"))]
    pub domain: Option<String>,
    /// Research category for effects on one science.
    #[cfg_attr(test, ts(as = "Option<TechCategory>"))]
    pub category: Option<String>,
    /// Game icon (asset path), if one matches.
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    /// Requirements of the root.
    Up,
    /// Everything building on the root.
    Down,
    Both,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct SubtreeArgs {
    #[cfg_attr(test, ts(as = "EntityKey"))]
    pub root: String,
    pub direction: Direction,
    /// Follow `altPrereqN` alternatives too.
    pub alternatives: bool,
    pub max_depth: Option<usize>,
}

/// Layout of the full tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub enum FullLayout {
    /// One layered drawing.
    #[default]
    Layered,
    /// One horizontal band per research category.
    Bands,
}

/// Vertical extent of a category band (cards and edge routes), in px.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct Band {
    #[cfg_attr(test, ts(as = "TechCategory"))]
    pub category: String,
    pub top: f64,
    pub bottom: f64,
}

/// A laid-out graph, ready to render. Positions are top-left corners in pixels for
/// nodes of `nodeWidth` × `nodeHeight`; columns run left → right (prereqs first, cheaper first).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct TreeGraph {
    pub nodes: Vec<TreeNode>,
    pub edges: Vec<TreeEdge>,
    pub node_width: f64,
    pub node_height: f64,
    /// Subtree root, if any.
    #[cfg_attr(test, ts(as = "Option<EntityKey>"))]
    pub root: Option<String>,
    /// Sum of research cost over all nodes (subtrees: everything needed).
    pub total_cost: f64,
    /// Category bands, top to bottom (`FullLayout::Bands` only).
    pub bands: Vec<Band>,
    /// Focus view: requirement groups satisfied by any one of several cards (left column).
    pub any_of: Vec<Span>,
}

/// Vertical extent of a group of cards in the column at `x`, in px.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub x: f64,
    pub top: f64,
    pub bottom: f64,
    /// Nesting level of the bracket (0 next to the cards); overlapping groups alternate.
    pub level: usize,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    #[cfg_attr(test, ts(as = "EntityKey"))]
    pub key: String,
    pub data_name: String,
    pub kind: NodeKind,
    pub name: String,
    /// `TechCategory` name.
    #[cfg_attr(test, ts(as = "TechCategory"))]
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
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
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
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct TreeEdge {
    /// Prerequisite.
    #[cfg_attr(test, ts(as = "EntityKey"))]
    pub from: String,
    /// Node requiring it.
    #[cfg_attr(test, ts(as = "EntityKey"))]
    pub to: String,
    /// Via `altPrereqN` (render dashed).
    pub alternative: bool,
    /// Implied by another chain of listed prereqs: the UI may hide it
    pub redundant: bool,
    /// Points the edge passes between its endpoints, left to right (empty = direct curve).
    pub points: Vec<[f64; 2]>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct EntityRef {
    pub spoiler: bool,
    #[cfg_attr(test, ts(as = "EntityKey"))]
    pub key: String,
    pub template: String,
    pub data_name: String,
    pub name: String,
    /// Coarse kind for grouping/icons: `tech`, `project`, `effect`, `habModule`, `shipPart`,
    /// `org`, `trait`, `faction`, `objective`, `mission`, `codex`, `other`.
    #[cfg_attr(test, ts(as = "EntityKind"))]
    pub kind: String,
    /// Ship part subtype (`drive`, `laser`, …) when `kind == "shipPart"`.
    pub subtype: Option<String>,
    #[cfg_attr(test, ts(as = "Option<TechCategory>"))]
    pub category: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub entity: EntityRef,
    pub score: u32,
    /// `name`, `dataName`, `text` or `value`.
    #[cfg_attr(test, ts(type = "'name' | 'dataName' | 'text' | 'value'"))]
    pub matched: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct EntityDetail {
    pub image: Option<String>,
    pub effect: Option<EffectLine>,
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
    /// Outgoing references grouped by target template and field.
    pub references: Vec<RefGroup>,
    /// Incoming references grouped by source template and field.
    pub referenced_by: Vec<RefGroup>,
    /// Stats of a hab module, ship part or org (None for other kinds).
    pub stats: Option<EntityStats>,
    pub research_cost: Option<f64>,
    /// Research cost of everything needed, including this (listed prereqs only).
    pub total_cost: Option<f64>,
    /// The raw record in use (for a "data" tab).
    #[cfg_attr(test, ts(type = "Record<string, unknown>"))]
    pub record: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
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
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
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
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
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
    #[cfg_attr(test, ts(as = "EffectSentiment"))]
    pub sentiment: String,
    /// Icon group: `research`, `priority`, `mission`, `control`, `councilor`, `army`, `ship`,
    /// `mining`, `missionControl`, `exploration`, `transfer`, `hab`, `alien`, `environment`,
    /// `economy`, `politics`, `ability`, `other`.
    #[cfg_attr(test, ts(as = "EffectDomain"))]
    pub domain: String,
    /// Research category for `research` effects on one science (`Materials`, …).
    #[cfg_attr(test, ts(as = "Option<TechCategory>"))]
    pub category: Option<String>,
    /// Who it applies to, if not just the acting faction ("All human factions").
    pub target: Option<String>,
    /// Matching game icon (asset path, like `EntityRef::icon`); `domain` is the fallback.
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct EffectSegment {
    /// `text`, `value` (substituted number) or `sprite` (inline icon name such as `metal`).
    #[cfg_attr(test, ts(type = "'text' | 'value' | 'sprite'"))]
    pub kind: String,
    pub text: String,
    /// Image of a `sprite` (asset path, like `EntityRef::icon`).
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct UnlockItem {
    /// `research`, `researchAlternative`, `shipPart`, `habModule`, `org`, `grantedOrg`,
    /// `claim`, `trait`, `codex`, `mission`, `objective`.
    #[cfg_attr(test, ts(as = "UnlockKind"))]
    pub kind: String,
    pub entity: EntityRef,
    /// Stats of unlocked hab modules, ship parts and orgs.
    pub stats: Option<EntityStats>,
}

/// Every stat of a hab module, ship part or org, grouped for display (see `stats.rs`).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct EntityStats {
    /// Short classifications ("Tier 2", "Station", "Fusion Thermal", "Criminal").
    pub tags: Vec<String>,
    pub groups: Vec<StatGroup>,
    /// Rules and conditions as sentences (special rules, eligibility, drive notes).
    pub notes: Vec<String>,
    /// Related entities (missions granted, upgrade chain, required tech).
    pub links: Vec<StatLinks>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct StatGroup {
    /// "Combat", "Income / month", "Build materials", …
    pub label: String,
    pub stats: Vec<Stat>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct Stat {
    pub label: String,
    /// Formatted value with unit ("1,200 t", "+45", "35%").
    pub value: String,
    /// Randomized orgs: roll range and chance, e.g. "+0–4 · 50% chance".
    pub detail: Option<String>,
    /// For the owning faction: `good`, `bad` or `neutral`.
    #[cfg_attr(test, ts(as = "EffectSentiment"))]
    pub sentiment: String,
    /// Game icon (asset path, like `EntityRef::icon`).
    pub icon: Option<String>,
    /// Record field the value comes from.
    pub field: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct StatLinks {
    pub label: String,
    pub entities: Vec<EntityRef>,
}

/// A nonempty template in the current wiki scope.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct WikiSection {
    pub spoiler: bool,
    pub template: String,
    pub label: String,
    pub group: String,
    pub count: usize,
}

/// References sharing an opposite-side template and referencing field.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct RefGroup {
    /// Template of the entities on the other side.
    pub template: String,
    /// Field path in the referencing record, e.g. `effects[]`.
    pub field: String,
    pub entities: Vec<EntityRef>,
}

/// One wiki row, with cheap context and an optional effect presentation.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct WikiEntry {
    pub entity: EntityRef,
    pub subtitle: Option<String>,
    pub effect: Option<EffectLine>,
}

/// A page of a wiki section's entries (`get_wiki_entries`).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct WikiPage {
    /// Entries matching the filter, in all pages.
    pub total: usize,
    pub entries: Vec<WikiEntry>,
}

// --- Solar system map (`get_solar_system`) ------------------------------------

/// Everything the solar system map draws, in one payload. Positions are not included:
/// the frontend propagates `OrbitElements` to any date (see `src/lib/components/map/kepler.ts`).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct SolarSystem {
    /// Start date of the scenario (overview: the base campaign's), `YYYY-MM-DD`.
    pub start_date: String,
    /// Every space body in scope, parents before children (the Sun first).
    pub bodies: Vec<SpaceBodyInfo>,
    pub lagrange_points: Vec<LagrangePointInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct SpaceBodyInfo {
    pub entity: EntityRef,
    /// `star`, `planet`, `dwarfPlanet`, `planetaryMoon`, `asteroidalMoon`, `asteroid`, `comet`.
    #[cfg_attr(test, ts(as = "BodyType"))]
    pub body_type: String,
    /// Key of the body it orbits (`barycenterName`); None for the Sun.
    #[cfg_attr(test, ts(as = "Option<EntityKey>"))]
    pub parent: Option<String>,
    /// None for the Sun (and bodies without usable elements).
    pub orbit: Option<OrbitElements>,
    /// The game's mean radius in km (mean half-dimension for irregular bodies; 0 without size data).
    pub radius_km: f64,
    pub mass_kg: Option<f64>,
    pub max_hab_size: u32,
    pub hab_site_count: u32,
    /// The game's rating of the unprospected body, per resource (keys `water`, `volatiles`,
    /// `metals`, `nobles`, `fissiles`): averaged over its hab sites; empty without hab sites.
    #[cfg_attr(
        test,
        ts(type = "Partial<Record<('water' | 'volatiles' | 'metals' | 'nobles' | 'fissiles'), ('empty' | 'possible' | 'low' | 'medium' | 'high' | 'max')>>")
    )]
    pub resources: std::collections::BTreeMap<String, ResourceRating>,
}

/// Rating the game shows for an unprospected site: `empty`, `possible`, `low`, `medium`, `high`, `max`.
pub type ResourceRating = String;

/// Keplerian elements as the game uses them (angles in degrees).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct OrbitElements {
    pub semi_major_axis_km: f64,
    pub eccentricity: f64,
    pub inclination_deg: f64,
    pub long_ascending_node_deg: f64,
    pub arg_periapsis_deg: f64,
    pub mean_anomaly_at_epoch_deg: f64,
    /// Epoch as a Julian year (2000.0 = J2000).
    pub epoch_julian_year: f64,
    /// Orbital period around the parent, in days (from the game's gravitational parameter).
    pub period_days: f64,
    /// Full precession cycles, in years (moons only). The game never applies them.
    pub nodal_precession_years: Option<f64>,
    pub apsidal_precession_years: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "camelCase")]
pub struct LagrangePointInfo {
    pub entity: EntityRef,
    /// `L1` … `L5`.
    #[cfg_attr(test, ts(type = "'L1' | 'L2' | 'L3' | 'L4' | 'L5'"))]
    pub point: String,
    /// Body the point belongs to (`relatedObject`, e.g. Mercury); its parent is the primary.
    #[cfg_attr(test, ts(as = "EntityKey"))]
    pub body: String,
    pub max_hab_size: u32,
}

/// Frontend contract for game-data strings.
#[cfg(test)]
#[allow(dead_code)]
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/lib/generated/", rename = "EntityKey", type = "string")]
struct EntityKey(String);

/// The game's research categories. The values are raw `techCategory` strings of the
/// records: a build adding a category would send a name outside this union (the UI falls
/// back to neutral styling). `commands::tests::full_tree` fails on such a build.
#[cfg(test)]
#[allow(dead_code)]
#[derive(ts_rs::TS)]
#[ts(
    export,
    export_to = "../../src/lib/generated/",
    rename = "TechCategory",
    type = "| 'Energy' | 'InformationScience' | 'LifeScience' | 'Materials' | 'MilitaryScience' | 'SocialScience' | 'SpaceScience' | 'Xenology'"
)]
struct TechCategory(String);

/// Frontend contract for game-data strings.
#[cfg(test)]
#[allow(dead_code)]
#[derive(ts_rs::TS)]
#[ts(
    export,
    export_to = "../../src/lib/generated/",
    rename = "PropertyGroup",
    type = "'effect' | 'ability' | 'unlock'"
)]
struct PropertyGroup(String);

/// Frontend contract for game-data strings.
#[cfg(test)]
#[allow(dead_code)]
#[derive(ts_rs::TS)]
#[ts(
    export,
    export_to = "../../src/lib/generated/",
    rename = "EntityKind",
    type = "| 'tech' | 'project' | 'effect' | 'habModule' | 'shipPart' | 'org' | 'trait' | 'faction' | 'objective' | 'mission' | 'codex' | 'other'"
)]
struct EntityKind(String);

/// Frontend contract for game-data strings.
#[cfg(test)]
#[allow(dead_code)]
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/lib/generated/", rename = "EffectSentiment", type = "'good' | 'bad' | 'neutral'")]
struct EffectSentiment(String);

/// Frontend contract for game-data strings.
#[cfg(test)]
#[allow(dead_code)]
#[derive(ts_rs::TS)]
#[ts(
    export,
    export_to = "../../src/lib/generated/",
    rename = "EffectDomain",
    type = "| 'research' | 'priority' | 'mission' | 'control' | 'councilor' | 'army' | 'ship' | 'mining' | 'missionControl' | 'exploration' | 'transfer' | 'hab' | 'alien' | 'environment' | 'economy' | 'politics' | 'ability' | 'other'"
)]
struct EffectDomain(String);

/// Frontend contract for game-data strings.
#[cfg(test)]
#[allow(dead_code)]
#[derive(ts_rs::TS)]
#[ts(
    export,
    export_to = "../../src/lib/generated/",
    rename = "UnlockKind",
    type = "| 'research' | 'researchAlternative' | 'shipPart' | 'habModule' | 'org' | 'grantedOrg' | 'claim' | 'trait' | 'codex' | 'mission' | 'objective'"
)]
struct UnlockKind(String);

/// Frontend contract for game-data strings.
#[cfg(test)]
#[allow(dead_code)]
#[derive(ts_rs::TS)]
#[ts(
    export,
    export_to = "../../src/lib/generated/",
    rename = "BodyType",
    type = "'star' | 'planet' | 'dwarfPlanet' | 'planetaryMoon' | 'asteroidalMoon' | 'asteroid' | 'comet' | 'unknown'"
)]
struct BodyType(String);

/// Frontend contract for game-data strings.
#[cfg(test)]
#[allow(dead_code)]
#[derive(ts_rs::TS)]
#[ts(
    export,
    export_to = "../../src/lib/generated/",
    rename = "SpaceResource",
    type = "'water' | 'volatiles' | 'metals' | 'nobles' | 'fissiles'"
)]
struct SpaceResource(String);

/// Frontend contract for game-data strings.
#[cfg(test)]
#[allow(dead_code)]
#[derive(ts_rs::TS)]
#[ts(
    export,
    export_to = "../../src/lib/generated/",
    rename = "ResourceRating",
    type = "'empty' | 'possible' | 'low' | 'medium' | 'high' | 'max'"
)]
struct ResourceRatingBinding(String);
