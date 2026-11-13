//! Command logic, independent of the Tauri runtime (the `#[tauri::command]`
//! wrappers in `lib.rs` only fetch the state and move work off the main thread).

use std::collections::{BTreeSet, HashMap};

use serde_json::Value;
use ti_data::graph::{Edge, ResearchGraph};
use ti_data::{EntityId, Traversal, UnlockKind, View, model, space};

use crate::dto::*;
use crate::effects::effect_line;
use crate::error::{CmdError, CmdResult};
use crate::layout::{self, LayoutEdge};
use crate::properties;
use crate::refs::{self, entity_ref};
use crate::scope;
use crate::state::{Backend, WikiRow};
use crate::stats::entity_stats;

/// Build, data folder, languages, and scenarios and factions labelled in `lang`.
pub fn get_meta(b: &Backend, lang: &str) -> CmdResult<Meta> {
    let ds = &*b.ds;
    let texts = b.texts(lang)?;
    let view = ds.overview();
    let scenarios = ds
        .scenarios()
        .iter()
        .map(|s| {
            let label = view.name(&texts, s.meta);
            ScenarioInfo {
                name: s.data_name.clone(),
                label: if label == s.data_name && !s.friendly_name.is_empty() {
                    s.friendly_name.clone()
                } else {
                    label
                },
                listed: s.listed,
                dlc: s.is_dlc(),
                tags: s.tags.clone(),
            }
        })
        .collect();
    let factions = view
        .entities(model::FACTION)
        .map(|id| FactionInfo {
            name: ds.entity(id).data_name.to_string(),
            label: view.name(&texts, id),
            icon: view
                .record(id)
                .and_then(|r| r.get("councilIcon64"))
                .and_then(Value::as_str)
                .and_then(|r| refs::asset_file(&view, r)),
        })
        .collect();
    Ok(Meta {
        build_id: ds.build_id().to_string(),
        game_version: ds.game_version().map(str::to_string),
        latest: b.latest_build_id() == Some(ds.build_id()),
        pinned_build: b.pinned_build.clone(),
        data_dir: b.data_dir.display().to_string(),
        data_dir_source: b.data_dir_source,
        languages: ds.languages().to_vec(),
        scenarios,
        factions,
    })
}

/// Builds of the data root, re-read from `index.json` so newly extracted builds show up.
pub fn list_builds(b: &Backend) -> CmdResult<Vec<BuildInfo>> {
    let root = ti_data::DataRoot::open(&b.data_dir).unwrap_or_else(|_| b.root.clone());
    let latest = root.index.latest.as_deref();
    Ok(root
        .index
        .builds
        .iter()
        .map(|entry| {
            let loaded = entry.build_id == b.ds.build_id();
            BuildInfo {
                dir: entry.dir.clone(),
                build_id: entry.build_id.clone(),
                last_updated: entry.last_updated,
                game_version: if loaded {
                    b.ds.game_version().map(str::to_string)
                } else {
                    root.manifest(entry).ok().and_then(|m| m.game.version)
                },
                loaded,
                latest: latest == Some(entry.dir.as_str()),
            }
        })
        .collect())
}

/// Entities of a template that exist in the scope. Unlike `View::entities`, projects of
/// other factions are kept: the wiki lists everything (annotations say who may research them).
fn wiki_entities<'v>(view: &'v View<'_>, template: &str) -> impl Iterator<Item = EntityId> + 'v {
    view.dataset().of_template(template).iter().copied().filter(|&id| view.exists(id))
}

/// All nonempty dataset templates, in wiki group and label order.
pub fn get_wiki_index(b: &Backend, scope: &ScopeArgs, show_spoilers: Option<bool>) -> CmdResult<Vec<WikiSection>> {
    let view = b.ds.view(scope::resolve(&b.ds, scope)?);
    let spoilers = &*b.spoilers(&view)?;
    let mut sections = Vec::new();
    for template in b.ds.templates() {
        let (_, group, label) = crate::wiki::section(template);
        for spoiler in [false, true] {
            if spoiler && show_spoilers != Some(true) {
                continue;
            }
            let count = wiki_entities(&view, template).filter(|id| spoilers.contains(id) == spoiler).count();
            if count > 0 {
                sections.push(WikiSection {
                    template: template.into(),
                    label: label.clone(),
                    group: if spoiler { "Spoilers".into() } else { group.into() },
                    count,
                    spoiler,
                });
            }
        }
    }
    sections.sort_by_cached_key(|s| (if s.spoiler { 6 } else { crate::wiki::section(&s.template).0 }, s.label.clone()));
    Ok(sections)
}

/// Every scoped entity of a known template, ordered by localized name, with its filter text.
/// Cached per (scope, language, template, spoiler), so paging and filtering stay cheap.
fn wiki_rows(b: &Backend, scope: &ScopeArgs, template: &str, spoiler: Option<bool>) -> CmdResult<std::sync::Arc<Vec<WikiRow>>> {
    if !b.ds.templates().any(|t| t == template) {
        return Err(CmdError::msg(format!("Unknown template {template:?}")));
    }
    let resolved = scope::resolve(&b.ds, scope)?;
    let spoiler = spoiler == Some(true);
    let key = (resolved, b.language(&scope.lang).to_string(), template.to_string(), spoiler);
    b.wiki_rows(key, || {
        let texts = b.texts(&scope.lang)?;
        let view = b.ds.view(resolved);
        let spoilers = &*b.spoilers(&view)?;
        let mut rows: Vec<_> = wiki_entities(&view, template)
            .filter(|id| spoilers.contains(id) == spoiler)
            .map(|id| {
                let entry = WikiEntry {
                    entity: entity_ref(&view, &texts, id, spoilers),
                    subtitle: crate::wiki::subtitle(&view, &texts, id),
                    effect: (template == model::EFFECT).then(|| effect_line(&view, &texts, id, entity_ref(&view, &texts, id, spoilers))),
                };
                // What the wiki list filter matches: name, dataName and subtitle.
                let haystack = format!(
                    "{} {} {}",
                    entry.entity.name,
                    entry.entity.data_name,
                    entry.subtitle.as_deref().unwrap_or_default()
                )
                .to_lowercase();
                WikiRow { haystack, entry }
            })
            .collect();
        rows.sort_by_cached_key(|r| (r.entry.entity.name.to_lowercase(), r.entry.entity.data_name.clone()));
        Ok(rows)
    })
}

/// A page of a wiki section: entries whose name, dataName or subtitle contain `filter`
/// (case-insensitive), skipping `offset`, at most `limit` (`None` = all).
pub fn get_wiki_entries(
    b: &Backend,
    scope: &ScopeArgs,
    template: &str,
    spoiler: Option<bool>,
    filter: &str,
    offset: usize,
    limit: Option<usize>,
) -> CmdResult<WikiPage> {
    let rows = wiki_rows(b, scope, template, spoiler)?;
    let needle = filter.to_lowercase();
    let mut total = 0;
    let mut entries = Vec::new();
    for row in rows.iter().filter(|r| r.haystack.contains(&needle)) {
        if total >= offset && limit.is_none_or(|l| entries.len() < l) {
            entries.push(row.entry.clone());
        }
        total += 1;
    }
    Ok(WikiPage { total, entries })
}

/// Every entry of a wiki section (tests and mock fixtures).
#[cfg(test)]
pub fn all_wiki_entries(b: &Backend, scope: &ScopeArgs, template: &str, spoiler: Option<bool>) -> CmdResult<Vec<WikiEntry>> {
    Ok(get_wiki_entries(b, scope, template, spoiler, "", 0, None)?.entries)
}

/// Use the referencing record's field path for both directions.
fn reference_groups(view: &View<'_>, texts: &ti_data::Texts, id: EntityId, incoming: bool, spoilers: &std::collections::HashSet<EntityId>) -> Vec<RefGroup> {
    let mut groups = std::collections::BTreeMap::<(String, String), BTreeSet<EntityId>>::new();
    let references = if incoming { view.references_to(id) } else { view.references_from(id) };
    for reference in references {
        let other = if incoming { reference.from } else { reference.to };
        groups
            .entry((view.dataset().entity(other).template.to_string(), reference.path.to_string()))
            .or_default()
            .insert(other);
    }
    groups
        .into_iter()
        .map(|((template, field), ids)| {
            let mut entities: Vec<_> = ids.into_iter().map(|id| entity_ref(view, texts, id, spoilers)).collect();
            entities.sort_by_cached_key(|e| (e.name.clone(), e.data_name.clone()));
            RefGroup { template, field, entities }
        })
        .collect()
}

pub fn get_tree(b: &Backend, scope: &ScopeArgs, filter: &TreeFilter, layout: FullLayout, merge_edges: bool) -> CmdResult<TreeGraph> {
    let texts = b.texts(&scope.lang)?;
    let view = b.ds.view(scope::resolve(&b.ds, scope)?);
    let graph = view.research_graph();
    let nodes: BTreeSet<EntityId> = graph
        .nodes()
        .iter()
        .copied()
        .filter(|&id| matches_kind_and_category(&view, id, filter))
        .filter(|&id| filter.properties.is_empty() || properties::of(&view, id).iter().any(|p| filter.properties.contains(p)))
        .collect();
    let edges: BTreeSet<Edge> = graph
        .full()
        .edges
        .into_iter()
        .filter(|e| nodes.contains(&e.from) && nodes.contains(&e.to))
        .collect();
    let unfiltered = filter.techs && filter.projects && filter.categories.is_empty() && filter.properties.is_empty();
    let mut tree = build_graph(
        &view,
        &texts,
        &nodes,
        &edges,
        None,
        GraphLayout {
            bands: layout == FullLayout::Bands,
            merge_edges,
        },
        unfiltered,
    );
    tree.total_cost = tree.nodes.iter().map(|n| n.cost).filter(|c| *c > 0.0).sum();
    Ok(tree)
}

/// The full-tree filter without its property part.
fn matches_kind_and_category(view: &View<'_>, id: EntityId, filter: &TreeFilter) -> bool {
    let kind = match view.kind(id) {
        model::Kind::Tech => filter.techs,
        model::Kind::Project => filter.projects,
        _ => false,
    };
    kind && (filter.categories.is_empty() || refs::category(view, id).is_some_and(|c| filter.categories.contains(&c)))
}

pub fn get_subtree(b: &Backend, scope: &ScopeArgs, args: &SubtreeArgs, merge_edges: bool) -> CmdResult<TreeGraph> {
    let texts = b.texts(&scope.lang)?;
    let view = b.ds.view(scope::resolve(&b.ds, scope)?);
    let root = refs::parse_key(&view, &args.root)?;
    let graph = view.research_graph();
    if !graph.contains(root) {
        return Err(CmdError::msg(format!("{} is not a tech or project in this scope", args.root)));
    }
    let t = Traversal {
        alternatives: args.alternatives,
        max_depth: args.max_depth,
    };
    let mut sub = match args.direction {
        Direction::Up => graph.ancestors(root, t),
        Direction::Down => graph.descendants(root, t),
        Direction::Both => graph.ancestors(root, t),
    };
    if args.direction == Direction::Both {
        let down = graph.descendants(root, t);
        sub.nodes.extend(down.nodes);
        sub.edges.extend(down.edges);
    }
    // Implying chains are complete only when the walk follows every edge without a depth limit.
    let complete = args.alternatives && args.max_depth.is_none();
    let mut tree = build_graph(
        &view,
        &texts,
        &sub.nodes,
        &sub.edges,
        Some(root),
        GraphLayout { bands: false, merge_edges },
        complete,
    );
    tree.total_cost = required_cost(graph, &view, root);
    Ok(tree)
}

/// Properties of the full-tree research that passes `filter`'s other parts (its own
/// `properties` are ignored, so the options don't shrink to the current selection).
pub fn get_properties(b: &Backend, scope: &ScopeArgs, filter: &TreeFilter) -> CmdResult<Vec<PropertyInfo>> {
    let texts = b.texts(&scope.lang)?;
    let view = b.ds.view(scope::resolve(&b.ds, scope)?);
    let nodes = view
        .research_graph()
        .nodes()
        .iter()
        .copied()
        .filter(|&id| matches_kind_and_category(&view, id, filter));
    Ok(properties::catalog(&view, &texts, nodes))
}

/// Research cost of everything that must be researched for `id`, including it (listed prereqs only).
fn required_cost(graph: &ResearchGraph, view: &View<'_>, id: EntityId) -> f64 {
    graph
        .ancestors(
            id,
            Traversal {
                alternatives: false,
                max_depth: None,
            },
        )
        .research_cost(view)
}

fn research_cost(view: &View<'_>, id: EntityId) -> f64 {
    view.record(id).and_then(|r| r.get("researchCost")).and_then(Value::as_f64).unwrap_or(0.0)
}

fn record_flag(view: &View<'_>, id: EntityId, key: &str) -> bool {
    view.record(id).and_then(|r| r.get(key)).and_then(Value::as_bool).unwrap_or(false)
}

/// How [`build_graph`] lays out its graph.
struct GraphLayout {
    /// One band per category.
    bands: bool,
    /// Long edges merged into shared lines.
    merge_edges: bool,
}

/// Lay out a node/edge set and fill in node data. `total_cost` is left at 0.
fn build_graph(
    view: &View<'_>,
    texts: &ti_data::Texts,
    nodes: &BTreeSet<EntityId>,
    edges: &BTreeSet<Edge>,
    root: Option<EntityId>,
    options: GraphLayout,
    mark_redundant: bool,
) -> TreeGraph {
    let ds = view.dataset();
    // Language-independent input order: category, then dataName.
    let mut order: Vec<EntityId> = nodes.iter().copied().collect();
    order.sort_by_cached_key(|&id| (refs::category(view, id).unwrap_or_default(), ds.entity(id).data_name.clone()));
    let index: HashMap<EntityId, usize> = order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let kept: Vec<&Edge> = edges.iter().filter(|e| index.contains_key(&e.from) && index.contains_key(&e.to)).collect();
    // A filtered tree may lack the chain implying an edge: then the edge is not redundant there.
    let none = BTreeSet::new();
    let redundant = if mark_redundant { view.research_graph().redundant_edges() } else { &none };
    let layout_edges: Vec<LayoutEdge> = kept
        .iter()
        .filter(|e| !redundant.contains(e))
        .map(|e| LayoutEdge {
            from: index[&e.from],
            to: index[&e.to],
            alternative: e.alternative,
        })
        .collect();
    let costs: Vec<f64> = order.iter().map(|&id| research_cost(view, id)).collect();
    // Band order: category name (matches the frontend's category order).
    let categories: Vec<String> = order.iter().map(|&id| refs::category(view, id).unwrap_or_default()).collect();
    let mut band_names: Vec<String> = if options.bands { categories.clone() } else { Vec::new() };
    band_names.sort_unstable();
    band_names.dedup();
    let groups: Vec<usize> = if options.bands {
        categories.iter().map(|c| band_names.binary_search(c).unwrap()).collect()
    } else {
        Vec::new()
    };
    let routed = layout::route_grouped(order.len(), &layout_edges, &costs, &groups, options.merge_edges);
    let placements = routed.nodes;
    let tree_bands = band_names
        .into_iter()
        .zip(routed.groups)
        .filter_map(|(category, span)| span.map(|(top, bottom)| Band { category, top, bottom }))
        .collect();
    let obtainable = view.research_graph().obtainable_set();
    let tree_nodes = order
        .iter()
        .zip(&placements)
        .map(|(&id, p)| tree_node(view, texts, id, p.column, p.x, p.y, obtainable.contains(&id)))
        .collect();
    let mut waypoints = routed.waypoints.into_iter();
    let tree_edges = kept
        .iter()
        .map(|e| TreeEdge {
            from: refs::key(view, e.from),
            to: refs::key(view, e.to),
            alternative: e.alternative,
            redundant: redundant.contains(e),
            points: if redundant.contains(e) {
                Vec::new()
            } else {
                waypoints.next().expect("routed edge")
            },
        })
        .collect();
    TreeGraph {
        nodes: tree_nodes,
        edges: tree_edges,
        node_width: layout::NODE_WIDTH,
        node_height: layout::NODE_HEIGHT,
        root: root.map(|r| refs::key(view, r)),
        total_cost: 0.0,
        bands: tree_bands,
        any_of: Vec::new(),
    }
}

fn tree_node(view: &View<'_>, texts: &ti_data::Texts, id: EntityId, column: usize, x: f64, y: f64, obtainable: bool) -> TreeNode {
    let a = view.annotations(id);
    TreeNode {
        key: refs::key(view, id),
        data_name: view.dataset().entity(id).data_name.to_string(),
        kind: if view.kind(id) == model::Kind::Tech {
            NodeKind::Tech
        } else {
            NodeKind::Project
        },
        name: view.name(texts, id),
        category: refs::category(view, id).unwrap_or_default(),
        cost: research_cost(view, id),
        column,
        x,
        y,
        flags: NodeFlags {
            scenario_specific: a.scenario_specific,
            varies_by_scenario: a.varies_by_scenario,
            dlc: a.from_dlc,
            faction_locked: !a.factions.is_empty(),
            end_game: record_flag(view, id, "endGameTech"),
            repeatable: record_flag(view, id, "repeatable"),
            obtainable,
        },
        properties: properties::of(view, id),
    }
}

/// Space between requirement groups in the focus view (room for the "any of" bracket).
const FOCUS_GROUP_GAP: f64 = 2.0 * layout::GAP_Y;

/// One tech/project with its direct requirements (left, in requirement-group order) and
/// direct dependents (right, by category, cost and name). Columns are centered on the root.
pub fn get_focus(b: &Backend, scope: &ScopeArgs, root: &str) -> CmdResult<TreeGraph> {
    let texts = b.texts(&scope.lang)?;
    let view = b.ds.view(scope::resolve(&b.ds, scope)?);
    let root_id = refs::parse_key(&view, root)?;
    let graph = view.research_graph();
    if !graph.contains(root_id) {
        return Err(CmdError::msg(format!("{root} is not a tech or project in this scope")));
    }
    let ds = view.dataset();
    let mut placed: BTreeSet<EntityId> = BTreeSet::from([root_id]);
    // Requirement groups (options with their alternative flag). An option may satisfy several
    // groups: groups sharing options are placed next to each other with the shared cards at
    // their boundary, so each group's cards stay contiguous (one card per option).
    let mut pending: Vec<Vec<(EntityId, bool)>> = graph
        .requirements(root_id)
        .iter()
        .map(|g| g.options_with_kind().filter(|(o, _)| *o != root_id).collect::<Vec<_>>())
        .filter(|o| !o.is_empty())
        .collect();
    let shared = |a: &[(EntityId, bool)], b: &[(EntityId, bool)]| b.iter().filter(|(o, _)| a.iter().any(|(p, _)| p == o)).count();
    let mut groups: Vec<Vec<(EntityId, bool)>> = Vec::new();
    while !pending.is_empty() {
        let next = groups
            .last()
            .and_then(|last| {
                (0..pending.len())
                    .max_by_key(|&i| (shared(last, &pending[i]), std::cmp::Reverse(i)))
                    .filter(|&i| shared(last, &pending[i]) > 0)
            })
            .unwrap_or(0);
        groups.push(pending.remove(next));
    }
    let mut dependents: Vec<(EntityId, bool)> = graph
        .dependents(root_id)
        .iter()
        .filter(|d| graph.contains(d.node) && !groups.iter().flatten().any(|(o, _)| *o == d.node) && placed.insert(d.node))
        .map(|d| (d.node, d.alternative))
        .collect();
    dependents.sort_by_cached_key(|&(id, _)| {
        let cost = research_cost(&view, id);
        (
            refs::category(&view, id).unwrap_or_default(),
            cost.max(0.0) as u64,
            ds.entity(id).data_name.clone(),
        )
    });

    let (w, h, gap) = (layout::NODE_WIDTH, layout::NODE_HEIGHT, layout::GAP_Y);
    let column_x = |c: usize| c as f64 * (w + layout::GAP_X);
    // Left column, top at 0: each new option once, options shared with the next group last.
    let mut left: Vec<(EntityId, bool, f64)> = Vec::new();
    let mut card_y: HashMap<EntityId, f64> = HashMap::new();
    let mut y = 0.0;
    for (i, g) in groups.iter().enumerate() {
        if i > 0 && shared(&groups[i - 1], g) == 0 {
            y += FOCUS_GROUP_GAP - gap;
        }
        let next = groups.get(i + 1).map_or(&[][..], Vec::as_slice);
        let mut fresh: Vec<(EntityId, bool)> = g.iter().copied().filter(|(o, _)| !card_y.contains_key(o)).collect();
        fresh.sort_by_key(|(o, _)| next.iter().any(|(p, _)| p == o));
        for (id, alt) in fresh {
            card_y.insert(id, y);
            left.push((id, alt, y));
            y += h + gap;
        }
    }
    // "Any of" brackets over each group's cards; overlapping ones alternate levels.
    let mut spans: Vec<(f64, f64, usize)> = Vec::new();
    for g in groups.iter().filter(|g| g.len() > 1) {
        let ys = g.iter().map(|(o, _)| card_y[o]);
        let (top, bottom) = (ys.clone().fold(f64::INFINITY, f64::min), ys.fold(f64::NEG_INFINITY, f64::max) + h);
        let level = match spans.last() {
            Some(&(_, prev_bottom, prev_level)) if top < prev_bottom => 1 - prev_level,
            _ => 0,
        };
        spans.push((top, bottom, level));
    }
    let left_height = (y - gap).max(0.0);
    let right_height = (dependents.len() as f64 * (h + gap) - gap).max(0.0);
    // Center every column on the tallest one.
    let tallest = left_height.max(right_height).max(h);
    let (left_off, right_off, root_y) = ((tallest - left_height) / 2.0, (tallest - right_height) / 2.0, (tallest - h) / 2.0);

    let obtainable = graph.obtainable_set();
    let node = |id: EntityId, column: usize, y: f64| tree_node(&view, &texts, id, column, column_x(column), y, obtainable.contains(&id));
    let key = |id: EntityId| refs::key(&view, id);
    let edge = |from: EntityId, to: EntityId, alternative: bool| TreeEdge {
        from: key(from),
        to: key(to),
        alternative,
        redundant: false,
        points: Vec::new(),
    };
    let mut nodes = vec![node(root_id, 1, root_y)];
    let mut edges = Vec::new();
    for &(id, alt, y) in &left {
        nodes.push(node(id, 0, y + left_off));
        edges.push(edge(id, root_id, alt));
    }
    for (i, &(id, alt)) in dependents.iter().enumerate() {
        nodes.push(node(id, 2, right_off + i as f64 * (h + gap)));
        edges.push(edge(root_id, id, alt));
    }
    let any_of = spans
        .into_iter()
        .map(|(top, bottom, level)| Span {
            x: column_x(0),
            top: top + left_off,
            bottom: bottom + left_off,
            level,
        })
        .collect();
    Ok(TreeGraph {
        nodes,
        edges,
        node_width: w,
        node_height: h,
        root: Some(key(root_id)),
        total_cost: required_cost(graph, &view, root_id),
        bands: Vec::new(),
        any_of,
    })
}

fn unlock_kind_name(kind: UnlockKind) -> &'static str {
    match kind {
        UnlockKind::Research { alternative: false } => "research",
        UnlockKind::Research { alternative: true } => "researchAlternative",
        UnlockKind::ShipPart(_) => "shipPart",
        UnlockKind::HabModule => "habModule",
        UnlockKind::Org => "org",
        UnlockKind::GrantedOrg => "grantedOrg",
        UnlockKind::Claim => "claim",
        UnlockKind::Trait => "trait",
        UnlockKind::Codex => "codex",
        UnlockKind::Mission => "mission",
        UnlockKind::Objective => "objective",
    }
}

pub fn get_entity(b: &Backend, scope: &ScopeArgs, key: &str) -> CmdResult<EntityDetail> {
    use ti_data::text::field;
    let texts = b.texts(&scope.lang)?;
    let view = b.ds.view(scope::resolve(&b.ds, scope)?);
    let spoilers = &*b.spoilers(&view)?;
    let id = refs::parse_key(&view, key)?;
    // Faction-hidden entities are still shown (the annotations say who may research them).
    let record = view.record(id).ok_or_else(|| CmdError::msg(format!("{key} does not exist in this scenario")))?;
    let graph = view.research_graph();
    let a = view.annotations(id);
    let is_research = graph.contains(id);
    let r = |id| entity_ref(&view, &texts, id, spoilers);
    Ok(EntityDetail {
        image: view.image_resource(id).and_then(|r| refs::asset_file(&view, &r)),
        effect: (&*b.ds.entity(id).template == model::EFFECT).then(|| effect_line(&view, &texts, id, r(id))),
        entity: r(id),
        summary: view.text(&texts, id, field::SUMMARY),
        description: view.text(&texts, id, field::DESCRIPTION),
        quote: view.text(&texts, id, field::QUOTE),
        annotations: AnnotationsDto {
            scenarios: a.scenarios.iter().map(|s| b.ds.scenario(s).data_name.clone()).collect(),
            scenario_specific: a.scenario_specific,
            varies_by_scenario: a.varies_by_scenario,
            dlc: a.from_dlc,
            factions: a.factions,
            faction_always: a.faction_always,
            requires_nation: a.requires_nation,
        },
        requirements: graph
            .requirements(id)
            .iter()
            .map(|g| RequirementGroup {
                options: g.options.iter().map(|&o| r(o)).collect(),
                alternative: g.alternative.clone(),
                hidden: g.hidden.iter().map(|&h| r(h)).collect(),
                unresolved: g.unresolved.clone(),
            })
            .collect(),
        dropped_prereqs: graph.dropped_prereqs(id).to_vec(),
        effects: view.effects(id).into_iter().map(|e| effect_line(&view, &texts, e, r(e))).collect(),
        unlocks: view
            .unlocks(id)
            .into_iter()
            .map(|u| UnlockItem {
                kind: unlock_kind_name(u.kind).into(),
                entity: r(u.entity),
                // Unlock cards show stats of modules, parts and orgs only (a project can unlock 1,000+ claims).
                stats: matches!(
                    u.kind,
                    UnlockKind::ShipPart(_) | UnlockKind::HabModule | UnlockKind::Org | UnlockKind::GrantedOrg
                )
                .then(|| entity_stats(&view, &texts, u.entity, spoilers))
                .flatten(),
            })
            .collect(),
        unlocked_by: view.unlocked_by(id).into_iter().map(r).collect(),
        references: reference_groups(&view, &texts, id, false, spoilers),
        referenced_by: reference_groups(&view, &texts, id, true, spoilers),
        stats: entity_stats(&view, &texts, id, spoilers),
        research_cost: is_research.then(|| research_cost(&view, id)),
        total_cost: is_research.then(|| required_cost(graph, &view, id)),
        record: record.clone(),
    })
}

/// `all`: also hits other factions' projects (the wiki); otherwise only what the faction sees.
/// `kinds`: only entities of these `EntityRef::kind`s (empty = any), applied before `limit`.
pub fn search(b: &Backend, scope: &ScopeArgs, query: &str, limit: usize, all: bool, kinds: &[String]) -> CmdResult<Vec<SearchHit>> {
    let texts = b.texts(&scope.lang)?;
    let index = b.search_index(&scope.lang)?;
    let view = b.ds.view(scope::resolve(&b.ds, scope)?);
    let spoilers = &*b.spoilers(&view)?;
    let hits = index.search(query, limit, &|id| {
        (if all { view.exists(id) } else { view.contains(id) }) && (kinds.is_empty() || kinds.iter().any(|k| k == refs::kind_name(view.kind(id))))
    });
    Ok(hits
        .into_iter()
        .map(|h| SearchHit {
            entity: entity_ref(&view, &texts, h.id, spoilers),
            score: h.score,
            matched: match h.field {
                ti_data::MatchField::Name => "name",
                ti_data::MatchField::DataName => "dataName",
                ti_data::MatchField::Text => "text",
                ti_data::MatchField::Value => "value",
            }
            .into(),
        })
        .collect())
}

/// Start date of the scope's scenario (overview: the default scenario's), `YYYY-MM-DD`.
fn start_date(view: &View<'_>) -> Option<String> {
    let ds = view.dataset();
    let scenario = view.scenario().or_else(|| ds.default_scenario())?;
    let record = &ds.selected_variant(scenario.idx, scenario.start_time?)?.record;
    let field = |key| record.get(key).and_then(Value::as_i64);
    Some(format!("{:04}-{:02}-{:02}", field("year")?, field("month")?, field("day")?))
}

/// Bodies of the scope (parents first) with their orbits and resource ratings, plus Lagrange points.
pub fn get_solar_system(b: &Backend, scope: &ScopeArgs) -> CmdResult<SolarSystem> {
    let texts = b.texts(&scope.lang)?;
    let view = b.ds.view(scope::resolve(&b.ds, scope)?);
    let spoilers = &*b.spoilers(&view)?;
    let system = space::System::load(&view);
    let key = |i: usize| refs::key(&view, system.bodies[i].id);
    let bodies = system
        .bodies
        .iter()
        .map(|body| {
            let d = &body.data;
            // Angles the game would randomize are shown as 0. A massless parent gives no usable period.
            let period_s = system.orbital_period_s(body).filter(|p| p.is_finite() && *p > 0.0);
            let orbit = d.semi_major_axis_m().zip(period_s).map(|(a, period_s)| OrbitElements {
                semi_major_axis_km: a / 1000.0,
                eccentricity: d.eccentricity(),
                inclination_deg: d.inclination_deg.unwrap_or(0.0),
                long_ascending_node_deg: d.long_ascending_node_deg.unwrap_or(0.0),
                arg_periapsis_deg: d.arg_periapsis().unwrap_or(0.0),
                mean_anomaly_at_epoch_deg: d.mean_anomaly_at_epoch().unwrap_or(0.0),
                epoch_julian_year: d.epoch(),
                period_days: period_s / 86_400.0,
                nodal_precession_years: d.nodal_precession_years,
                apsidal_precession_years: d.apsidal_precession_years,
            });
            SpaceBodyInfo {
                entity: entity_ref(&view, &texts, body.id, spoilers),
                body_type: d.object_type.name().into(),
                parent: body.parent.map(key),
                orbit,
                radius_km: d.mean_radius(),
                mass_kg: d.mass_kg,
                max_hab_size: d.max_hab_size,
                hab_site_count: body.sites.len() as u32,
                resources: space::Resource::ALL
                    .into_iter()
                    .filter_map(|r| Some((r.key().to_string(), system.body_rating(body, r)?.name().to_string())))
                    .collect(),
            }
        })
        .collect();
    let lagrange_points = system
        .lagrange_points
        .iter()
        .filter_map(|(id, point)| {
            Some(LagrangePointInfo {
                entity: entity_ref(&view, &texts, *id, spoilers),
                point: point.lagrange_value.clone(),
                body: key(system.index(&point.related_object)?),
                max_hab_size: point.max_hab_size,
            })
        })
        .collect();
    Ok(SolarSystem {
        start_date: start_date(&view).unwrap_or_default(),
        bodies,
        lagrange_points,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, OnceLock};

    use super::*;

    /// The workspace's extracted data, or `None` (tests skip) when it is missing.
    pub fn backend() -> Option<Arc<Backend>> {
        static B: OnceLock<Option<Arc<Backend>>> = OnceLock::new();
        B.get_or_init(|| {
            let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data");
            if !dir.join("index.json").is_file() {
                eprintln!("skipping: no data at {}", dir.display());
                return None;
            }
            Some(Arc::new(Backend::open(dir).expect("data loads")))
        })
        .clone()
    }

    pub fn en() -> ScopeArgs {
        ScopeArgs {
            scenario: None,
            faction: None,
            lang: "en".into(),
        }
    }

    #[test]
    fn wiki_images_effects_and_context() {
        let Some(b) = backend() else { return };
        let effects = all_wiki_entries(&b, &en(), model::EFFECT, None).unwrap();
        assert!(effects.iter().all(|e| e.effect.is_some() && e.subtitle.is_none()));
        let effect = get_entity(&b, &en(), &effects[0].entity.key).unwrap();
        assert_eq!(effect.effect.unwrap().entity.key, effects[0].entity.key);
        let region = get_entity(&b, &en(), "TIRegionTemplate:Afghanistan").unwrap();
        assert!(region.image.is_some());
        assert!(region.effect.is_none());
        let entries = all_wiki_entries(&b, &en(), "TIRegionTemplate", None).unwrap();
        assert!(entries.iter().find(|e| e.entity.data_name == "Afghanistan").unwrap().subtitle.is_some());
    }

    #[test]
    fn wiki_templates_and_entries() {
        let Some(b) = backend() else { return };
        for scope in std::iter::once(en()).chain(get_meta(&b, "en").unwrap().scenarios.into_iter().map(|s| ScopeArgs {
            scenario: Some(s.name),
            ..en()
        })) {
            let view = b.ds.view(scope::resolve(&b.ds, &scope).unwrap());
            let index = get_wiki_index(&b, &scope, Some(true)).unwrap();
            let expected: BTreeSet<_> = b.ds.templates().filter(|t| view.entities(t).next().is_some()).collect();
            assert_eq!(index.iter().map(|s| s.template.as_str()).collect::<BTreeSet<_>>(), expected);

            for section in index {
                let entries = all_wiki_entries(&b, &scope, &section.template, Some(section.spoiler)).unwrap();
                assert_eq!(section.count, entries.len());
                assert!(
                    entries
                        .windows(2)
                        .all(|w| (w[0].entity.name.to_lowercase(), &w[0].entity.data_name) <= (w[1].entity.name.to_lowercase(), &w[1].entity.data_name))
                );
            }
        }
        assert!(all_wiki_entries(&b, &en(), "TIUnknownTemplate", None).is_err());
    }

    #[test]
    fn wiki_pages_and_filters() {
        let Some(b) = backend() else { return };
        let all = all_wiki_entries(&b, &en(), model::TECH, None).unwrap();
        let keys = |entries: &[WikiEntry]| entries.iter().map(|e| e.entity.key.clone()).collect::<Vec<_>>();
        let page = get_wiki_entries(&b, &en(), model::TECH, None, "", 10, Some(5)).unwrap();
        assert_eq!(page.total, all.len());
        assert_eq!(keys(&page.entries), keys(&all[10..15]));

        // Same rule as the old client-side filter: name, dataName, subtitle; case-insensitive substring.
        for filter in ["ENERGY", "adastra", "a f", "zzzz-no-match"] {
            let needle = filter.to_lowercase();
            let expected: Vec<_> = all
                .iter()
                .filter(|e| {
                    format!("{} {} {}", e.entity.name, e.entity.data_name, e.subtitle.as_deref().unwrap_or_default())
                        .to_lowercase()
                        .contains(&needle)
                })
                .cloned()
                .collect();
            let page = get_wiki_entries(&b, &en(), model::TECH, None, filter, 0, Some(200)).unwrap();
            assert_eq!(page.total, expected.len(), "{filter}");
            assert_eq!(keys(&page.entries), keys(&expected[..expected.len().min(200)]), "{filter}");
        }
        let past = get_wiki_entries(&b, &en(), model::TECH, None, "", all.len() + 5, Some(200)).unwrap();
        assert_eq!((past.total, past.entries.len()), (all.len(), 0));
    }

    /// Prints the time of 200-row pages of the largest wiki section (run with `--release --nocapture`).
    #[test]
    fn wiki_page_timing() {
        let Some(b) = backend() else { return };
        let template = "TIBilateralTemplate";
        let time = |offset: usize, filter: &str| {
            let started = std::time::Instant::now();
            let page = get_wiki_entries(&b, &en(), template, None, filter, offset, Some(200)).unwrap();
            (started.elapsed(), page)
        };
        let (cold, page) = time(0, "");
        let (warm, _) = time(200, "");
        let (filtered, filtered_page) = time(0, "united");
        let bytes = serde_json::to_vec(&page).unwrap().len();
        eprintln!(
            "{template}: {} rows; first page {cold:?} (builds the list), next page {warm:?}, filtered ({} rows) {filtered:?}; page JSON {bytes} bytes",
            page.total, filtered_page.total
        );
        assert_eq!(page.entries.len(), 200.min(page.total));
    }

    #[test]
    fn meta_follows_the_language() {
        let Some(b) = backend() else { return };
        let en = get_meta(&b, "en").unwrap();
        assert_eq!(
            get_meta(&b, "no-such-language").unwrap().factions.iter().map(|f| &f.label).collect::<Vec<_>>(),
            en.factions.iter().map(|f| &f.label).collect::<Vec<_>>()
        );
        let Some(lang) = en.languages.iter().find(|l| *l != "en") else { return };
        let other = get_meta(&b, lang).unwrap();
        assert!(
            other.factions.iter().zip(&en.factions).any(|(o, e)| o.name == e.name && o.label != e.label),
            "{lang} faction labels match English"
        );
    }

    #[test]
    fn builds_listed() {
        let Some(b) = backend() else { return };
        let builds = list_builds(&b).unwrap();
        let loaded: Vec<_> = builds.iter().filter(|x| x.loaded).collect();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].build_id, b.ds.build_id());
        assert_eq!(loaded[0].game_version.as_deref(), b.ds.game_version());
        assert_eq!(builds.iter().filter(|x| x.latest).count(), 1);
        let meta = get_meta(&b, "en").unwrap();
        assert_eq!(meta.latest, loaded[0].latest);
        assert_eq!(meta.pinned_build, None);
    }

    #[test]
    fn overview_armies_name_their_scenarios() {
        let Some(b) = backend() else { return };
        let rows = all_wiki_entries(&b, &en(), "TIArmyTemplate", None).unwrap();
        let labels = get_meta(&b, "en").unwrap().scenarios;
        let mut subtitles = BTreeSet::new();
        for name in ["USAArmy4", "2026_USAArmy4", "2070_USAArmy2"] {
            let row = rows.iter().find(|r| r.entity.data_name == name).unwrap();
            assert_eq!(row.entity.name, "1st Armored");
            let subtitle = row.subtitle.as_ref().unwrap();
            let id = b.ds.find("TIArmyTemplate", name).unwrap();
            for idx in b.ds.presence(id).iter() {
                let scenario = b.ds.scenario(idx);
                assert!(subtitle.contains(&labels.iter().find(|s| s.name == scenario.data_name).unwrap().label));
            }
            assert!(subtitles.insert(subtitle));
        }
    }

    #[test]
    fn wiki_nation_capitals() {
        let Some(b) = backend() else { return };
        let entries = all_wiki_entries(&b, &en(), "TINationTemplate", None).unwrap();
        let usa = entries.iter().find(|e| e.entity.data_name == "USA").unwrap();
        assert!(usa.subtitle.as_deref().is_some_and(|s| s.contains("Capital: ")), "{:?}", usa.subtitle);
    }

    #[test]
    fn wiki_keeps_other_factions_projects() {
        let Some(b) = backend() else { return };
        let scenario = ScopeArgs {
            scenario: Some("ModernScenario".into()),
            ..en()
        };
        let faction = ScopeArgs {
            faction: Some("ResistCouncil".into()),
            ..scenario.clone()
        };
        let projects = |scope: &ScopeArgs| {
            [None, Some(true)]
                .into_iter()
                .map(|spoiler| all_wiki_entries(&b, scope, model::PROJECT, spoiler).unwrap().len())
                .sum::<usize>()
        };
        assert_eq!(projects(&faction), projects(&scenario));
        // The tree search still hides them; the wiki search does not.
        let view = b.ds.view(scope::resolve(&b.ds, &faction).unwrap());
        let hidden = view.entities(model::PROJECT).count();
        assert!(hidden < projects(&faction), "ResistCouncil should not see every project");
    }

    #[test]
    fn wiki_references_both_directions() {
        let Some(b) = backend() else { return };
        let view = b.ds.overview();
        let id = view.entities(model::TECH).find(|&id| !view.effects(id).is_empty()).expect("tech with effects");
        let tech = get_entity(&b, &en(), &refs::key(&view, id)).unwrap();
        let effects = tech
            .references
            .iter()
            .find(|g| g.template == model::EFFECT && g.field == "effects[]")
            .expect("tech effects group");
        assert!(!effects.entities.is_empty());
        for effect in &effects.entities {
            let detail = get_entity(&b, &en(), &effect.key).unwrap();
            assert!(
                detail
                    .referenced_by
                    .iter()
                    .any(|g| g.template == model::TECH && g.field == "effects[]" && g.entities.iter().any(|e| e.key == tech.entity.key))
            );
        }
    }

    #[test]
    fn meta() {
        let Some(b) = backend() else { return };
        let m = get_meta(&b, "en").unwrap();
        // Game menu order (by listPriority), not "default first".
        assert!(m.scenarios.iter().take_while(|s| s.listed).any(|s| s.name == "ModernScenario"));
        assert!(m.scenarios.iter().skip_while(|s| s.listed).all(|s| !s.listed));
        assert!(m.scenarios.iter().any(|s| s.dlc));
        assert!(m.scenarios.iter().all(|s| !s.label.is_empty()));
        assert!(m.languages.contains(&"en".to_string()));
        let resist = m.factions.iter().find(|f| f.name == "ResistCouncil").unwrap();
        assert_eq!(resist.label, "the Resistance");
        let icon = resist.icon.as_ref().expect("faction icon");
        assert!(b.assets_dir.as_ref().unwrap().join(icon).is_file());
    }

    #[test]
    fn full_tree() {
        let Some(b) = backend() else { return };
        let filter = TreeFilter {
            techs: true,
            projects: true,
            categories: vec![],
            properties: vec![],
        };
        let t = std::time::Instant::now();
        let tree = get_tree(&b, &en(), &filter, FullLayout::Layered, true).unwrap();
        eprintln!(
            "get_tree (overview, all): {} nodes, {} edges in {:?}",
            tree.nodes.len(),
            tree.edges.len(),
            t.elapsed()
        );
        assert!(tree.nodes.len() > 800);
        // The frontend types promise one of the known categories (`dto.rs` `TechCategory`).
        let unknown: Vec<_> = tree
            .nodes
            .iter()
            .filter(|n| model::TechCategory::parse(&n.category).is_none_or(|c| c == model::TechCategory::Unknown))
            .map(|n| &n.key)
            .collect();
        assert!(unknown.is_empty(), "research without a known category: {unknown:?}");
        assert!(tree.total_cost > 0.0);
        let keys: std::collections::HashSet<&str> = tree.nodes.iter().map(|n| n.key.as_str()).collect();
        assert!(tree.edges.iter().all(|e| keys.contains(e.from.as_str()) && keys.contains(e.to.as_str())));
        assert!(tree.edges.iter().any(|e| e.alternative));
        let redundant = tree.edges.iter().filter(|e| e.redundant).count();
        eprintln!("redundant edges: {redundant}");
        assert!(redundant > 0);
        assert!(tree.edges.iter().all(|e| !e.alternative || !e.redundant));
        assert!(tree.edges.iter().filter(|e| e.redundant).all(|e| e.points.is_empty()));
        let by_key: HashMap<&str, &TreeNode> = tree.nodes.iter().map(|n| (n.key.as_str(), n)).collect();
        for e in tree.edges.iter().filter(|e| !e.alternative) {
            assert!(by_key[e.from.as_str()].column < by_key[e.to.as_str()].column, "{} -> {}", e.from, e.to);
        }
        assert!(tree.nodes.iter().any(|n| n.flags.end_game));
        assert!(tree.nodes.iter().any(|n| n.flags.repeatable));
        assert!(tree.nodes.iter().any(|n| n.flags.faction_locked));
        assert!(tree.nodes.iter().filter(|n| n.flags.obtainable).count() > 800);

        let techs = get_tree(
            &b,
            &en(),
            &TreeFilter {
                techs: true,
                projects: false,
                categories: vec![],
                properties: vec![],
            },
            FullLayout::Layered,
            true,
        )
        .unwrap();
        assert!(techs.nodes.iter().all(|n| n.kind == NodeKind::Tech));
        let energy = get_tree(
            &b,
            &en(),
            &TreeFilter {
                techs: true,
                projects: true,
                categories: vec!["Energy".into()],
                properties: vec![],
            },
            FullLayout::Layered,
            true,
        )
        .unwrap();
        assert!(!energy.nodes.is_empty() && energy.nodes.iter().all(|n| n.category == "Energy"));
    }

    /// Every option of every requirement group has a card, and multi-option groups get a
    /// bracket spanning exactly their cards, also when groups share an option.
    #[test]
    fn focus_groups_keep_shared_options() {
        let Some(b) = backend() else { return };
        let root = "TIProjectTemplate:Project_AutomatedOutpostCore";
        let mut shared_seen = false;
        for scenario in get_meta(&b, "en").unwrap().scenarios {
            let scope = ScopeArgs {
                scenario: Some(scenario.name.clone()),
                ..en()
            };
            let Ok(f) = get_focus(&b, &scope, root) else {
                continue;
            };
            let detail = get_entity(&b, &scope, root).unwrap();
            let y: HashMap<&str, f64> = f.nodes.iter().filter(|n| n.column == 0).map(|n| (n.key.as_str(), n.y)).collect();
            let groups: Vec<Vec<&str>> = detail
                .requirements
                .iter()
                .map(|g| g.options.iter().map(|o| o.key.as_str()).collect())
                .filter(|g: &Vec<&str>| !g.is_empty())
                .collect();
            for g in &groups {
                assert!(g.iter().all(|k| y.contains_key(k)), "{}: {g:?} missing", scenario.name);
                if g.len() > 1 {
                    let top = g.iter().map(|k| y[k]).fold(f64::INFINITY, f64::min);
                    let bottom = g.iter().map(|k| y[k]).fold(f64::NEG_INFINITY, f64::max) + f.node_height;
                    assert!(f.any_of.iter().any(|s| s.top == top && s.bottom == bottom), "{}: {g:?}", scenario.name);
                    // Contiguous: no other card inside the bracket.
                    assert_eq!(y.values().filter(|&&v| v >= top && v < bottom).count(), g.len(), "{}: {g:?}", scenario.name);
                }
            }
            shared_seen |= groups
                .iter()
                .enumerate()
                .any(|(i, g)| groups[i + 1..].iter().any(|h| h.iter().any(|k| g.contains(k))));
        }
        assert!(shared_seen, "no scenario with a shared requirement option");
    }

    #[test]
    fn focus_view() {
        let Some(b) = backend() else { return };
        let f = get_focus(&b, &en(), "TIProjectTemplate:Project_Pherocytes").unwrap();
        let root = f.root.as_deref().unwrap();
        assert_eq!(f.nodes[0].key, root);
        assert_eq!(f.nodes[0].column, 1);
        // Pherocytes has one requirement with two options: an "any of" bracket.
        assert_eq!(f.any_of.len(), 1);
        assert!(f.nodes.iter().filter(|n| n.column == 0).count() >= 2);
        assert!(f.edges.iter().all(|e| e.from == root || e.to == root));
        let keys: std::collections::HashSet<&str> = f.nodes.iter().map(|n| n.key.as_str()).collect();
        assert_eq!(keys.len(), f.nodes.len());

        let d = get_focus(&b, &en(), "TITechTemplate:AdAstra").unwrap();
        let deps: Vec<&TreeNode> = d.nodes.iter().filter(|n| n.column == 2).collect();
        assert!(deps.len() > 3);
        assert!(deps.windows(2).all(|w| w[0].category <= w[1].category && w[0].y < w[1].y));
        let detail = get_entity(&b, &en(), "TITechTemplate:AdAstra").unwrap();
        let research_unlocks = detail.unlocks.iter().filter(|u| u.kind.starts_with("research")).count();
        assert_eq!(deps.len(), research_unlocks);
        assert!(get_focus(&b, &en(), "TITechTemplate:Nope").is_err());
    }

    #[test]
    fn filtered_trees_keep_redundant_edges() {
        let Some(b) = backend() else { return };
        let filter = TreeFilter {
            techs: true,
            projects: true,
            categories: vec!["Energy".into()],
            properties: vec![],
        };
        let tree = get_tree(&b, &en(), &filter, FullLayout::Layered, true).unwrap();
        assert!(tree.edges.iter().all(|e| !e.redundant));
    }

    #[test]
    fn category_bands() {
        let Some(b) = backend() else { return };
        let filter = TreeFilter {
            techs: true,
            projects: true,
            categories: vec![],
            properties: vec![],
        };
        let tree = get_tree(&b, &en(), &filter, FullLayout::Bands, true).unwrap();
        assert_eq!(tree.bands.len(), 8);
        assert!(tree.bands.windows(2).all(|w| w[0].bottom < w[1].top && w[0].category < w[1].category));
        for n in &tree.nodes {
            let band = tree.bands.iter().find(|band| band.category == n.category).unwrap();
            assert!(n.y >= band.top - 1e-6 && n.y + tree.node_height <= band.bottom + 1e-6, "{}", n.key);
        }
        assert!(get_tree(&b, &en(), &filter, FullLayout::Layered, true).unwrap().bands.is_empty());
    }

    #[test]
    fn properties() {
        let Some(b) = backend() else { return };
        let all = TreeFilter {
            techs: true,
            projects: true,
            categories: vec![],
            properties: vec![],
        };
        let catalog = get_properties(&b, &en(), &all).unwrap();
        let find = |id: &str| catalog.iter().find(|p| p.id == id).unwrap_or_else(|| panic!("{id} missing"));
        let cp = find("effect:ControlPointMaintenance:up");
        assert_eq!(cp.label, "Increases control point capacity");
        assert!(cp.count > 20, "{}", cp.count);
        assert!(cp.example.as_deref().is_some_and(|e| e.contains("control point")), "{:?}", cp.example);
        assert_eq!(
            find("effect:MCFreeSpaceMineNetwork:up").label,
            "Increases mining modules free of Mission Control cost"
        );
        assert_eq!(find("effect:CanFoundTier1Base").group, "ability");
        assert!(find("unlock:habModule").count > 10);
        assert!(find("unlock:shipPart:drive").count > 10);
        assert!(catalog.iter().all(|p| !p.label.is_empty() && p.count > 0));

        let filter = TreeFilter {
            techs: true,
            projects: true,
            categories: vec![],
            properties: vec!["effect:ControlPointMaintenance:up".into(), "effect:MCFreeSpaceMineNetwork:up".into()],
        };
        let tree = get_tree(&b, &en(), &filter, FullLayout::Layered, true).unwrap();
        let mine = find("effect:MCFreeSpaceMineNetwork:up").count;
        assert!(tree.nodes.len() >= cp.count.max(mine) && tree.nodes.len() <= cp.count + mine);
        assert!(tree.nodes.iter().all(|n| n.properties.iter().any(|p| filter.properties.contains(p))));

        // The options follow the other filters but not the selected properties.
        assert_eq!(cp.sentiment, "good");
        assert_eq!(cp.icon.as_deref(), Some("sprites/control_point.png"));
        let energy = TreeFilter {
            categories: vec!["Energy".into()],
            ..filter.clone()
        };
        let faceted = get_properties(&b, &en(), &energy).unwrap();
        let energy_tree = get_tree(
            &b,
            &en(),
            &TreeFilter {
                properties: vec![],
                ..energy.clone()
            },
            FullLayout::Layered,
            true,
        )
        .unwrap();
        assert!(faceted.len() < catalog.len());
        for p in &faceted {
            let on_screen = energy_tree.nodes.iter().filter(|n| n.properties.contains(&p.id)).count();
            assert_eq!(p.count, on_screen, "{}", p.id);
        }
    }

    #[test]
    fn full_tree_layout_timing() {
        let Some(b) = backend() else { return };
        let view = b.ds.overview();
        let full = view.research_graph().full();
        let index: HashMap<EntityId, usize> = full.nodes.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        let edges: Vec<LayoutEdge> = full
            .edges
            .iter()
            .map(|e| LayoutEdge {
                from: index[&e.from],
                to: index[&e.to],
                alternative: e.alternative,
            })
            .collect();
        let costs: Vec<f64> = full.nodes.iter().map(|&id| research_cost(&view, id)).collect();
        let t = std::time::Instant::now();
        let routed = layout::route(full.nodes.len(), &edges, &costs);
        let elapsed = t.elapsed();
        let p = routed.nodes;
        eprintln!(
            "routed: {} waypoints, crossings of the drawn (routed) edges {}",
            routed.waypoints.iter().map(Vec::len).sum::<usize>(),
            routed.crossings
        );
        let before = layout::count_crossings(&layout::unordered(full.nodes.len(), &edges), &edges);
        let after = layout::count_crossings(&p, &edges);
        let ncols = p.iter().map(|q| q.column + 1).max().unwrap_or(0);
        let mut per_col = vec![0usize; ncols];
        for q in &p {
            per_col[q.column] += 1;
        }
        eprintln!("cost-ranked columns: {ncols}, tallest {}", per_col.iter().max().unwrap_or(&0));
        let stats = |p: &[layout::Placement]| {
            let height = p.iter().map(|q| q.y).fold(0.0, f64::max) - p.iter().map(|q| q.y).fold(0.0, f64::min);
            let dy: Vec<f64> = edges.iter().map(|e| (p[e.from].y - p[e.to].y).abs()).collect();
            let near = dy.iter().filter(|&&d| d < layout::NODE_HEIGHT / 2.0).count();
            format!(
                "height {:.0}, crossings {}, edges within half a card {near}, mean |dy| {:.0}",
                height + layout::NODE_HEIGHT,
                layout::count_crossings(p, &edges),
                dy.iter().sum::<f64>() / dy.len() as f64
            )
        };
        let packed = layout::packed(full.nodes.len(), &edges);
        let tallest = packed.iter().map(|q| q.row + 1).max().unwrap_or(0) as f64;
        eprintln!(
            "layout of real tree ({} nodes, {} edges): {elapsed:?}, crossings {before} (input order) -> {after}\n  \
             tallest column packed: {:.0}\n  packed columns: {}\n  layout: {}",
            full.nodes.len(),
            edges.len(),
            tallest * (layout::NODE_HEIGHT + layout::GAP_Y) - layout::GAP_Y,
            stats(&packed),
            stats(&p),
        );
        assert!(after < before);
        if !cfg!(debug_assertions) {
            assert!(elapsed.as_millis() < 50, "{elapsed:?}");
        }
    }

    #[test]
    fn subtrees() {
        let Some(b) = backend() else { return };
        let args = |root: &str, direction| SubtreeArgs {
            root: root.into(),
            direction,
            alternatives: true,
            max_depth: None,
        };
        let up = get_subtree(&b, &en(), &args("TIProjectTemplate:Project_ProtiumConverterTorch", Direction::Up), true).unwrap();
        assert_eq!(up.root.as_deref(), Some("TIProjectTemplate:Project_ProtiumConverterTorch"));
        assert!(up.nodes.len() > 10);
        assert!(up.total_cost > 0.0);
        let down = get_subtree(&b, &en(), &args("TITechTemplate:AdAstra", Direction::Down), true).unwrap();
        let both = get_subtree(&b, &en(), &args("TITechTemplate:AdAstra", Direction::Both), true).unwrap();
        assert!(both.nodes.len() >= down.nodes.len());
        assert_eq!(down.total_cost, both.total_cost);
        let shallow = get_subtree(
            &b,
            &en(),
            &SubtreeArgs {
                max_depth: Some(1),
                ..args("TITechTemplate:AdAstra", Direction::Down)
            },
            true,
        )
        .unwrap();
        let root = shallow.root.as_deref().unwrap();
        assert!(shallow.nodes.len() > 1);
        assert!(
            shallow
                .nodes
                .iter()
                .all(|n| { n.key == root || shallow.edges.iter().any(|e| e.from == root && e.to == n.key) })
        );
        assert!(get_subtree(&b, &en(), &args("TITechTemplate:Nope", Direction::Up), true).is_err());
    }

    #[test]
    fn entity_detail() {
        let Some(b) = backend() else { return };
        let d = get_entity(&b, &en(), "TITechTemplate:AdAstra").unwrap();
        assert_eq!(d.entity.kind, "tech");
        assert!(d.entity.icon.is_some());
        assert!(d.summary.is_some() || d.description.is_some());
        assert!(!d.unlocks.is_empty());
        assert!(d.research_cost.unwrap() > 0.0);
        assert!(d.total_cost.unwrap() >= d.research_cost.unwrap());
        assert_eq!(d.record["dataName"], "AdAstra");

        let p = get_entity(&b, &en(), "TIProjectTemplate:Project_Pherocytes").unwrap();
        assert_eq!(p.requirements[0].options.len(), 2);

        let drive = b.ds.overview().entities("TIDriveTemplate").next().unwrap();
        let key = refs::key(&b.ds.overview(), drive);
        let d = get_entity(&b, &en(), &key).unwrap();
        assert_eq!(d.entity.kind, "shipPart");
        assert_eq!(d.entity.subtype.as_deref(), Some("drive"));
        assert!(d.research_cost.is_none());

        let with_effects = b.ds.overview().entities(model::TECH).find(|t| !b.ds.overview().effects(*t).is_empty()).unwrap();
        let d = get_entity(&b, &en(), &refs::key(&b.ds.overview(), with_effects)).unwrap();
        assert!(d.effects.iter().all(|e| e.entity.kind == "effect"));

        assert!(get_entity(&b, &en(), "garbage").is_err());
        assert!(get_entity(&b, &en(), "TITechTemplate:NoSuchTech").is_err());
        // Unknown languages fall back to English.
        let unknown = ScopeArgs { lang: "xx".into(), ..en() };
        let name = |scope: &ScopeArgs| get_entity(&b, scope, "TITechTemplate:AdAstra").unwrap().entity.name;
        assert_eq!(name(&unknown), name(&en()));
    }

    #[test]
    fn scoped_queries() {
        let Some(b) = backend() else { return };
        let scope = ScopeArgs {
            scenario: Some("ModernScenario".into()),
            faction: Some("ResistCouncil".into()),
            lang: "en".into(),
        };
        let tree = get_tree(
            &b,
            &scope,
            &TreeFilter {
                techs: true,
                projects: true,
                categories: vec![],
                properties: vec![],
            },
            FullLayout::Layered,
            true,
        )
        .unwrap();
        assert!(!tree.nodes.is_empty());
        let bad = ScopeArgs {
            scenario: Some("NoSuchScenario".into()),
            ..en()
        };
        assert!(
            get_tree(
                &b,
                &bad,
                &TreeFilter {
                    techs: true,
                    projects: true,
                    categories: vec![],
                    properties: vec![]
                },
                FullLayout::Layered,
                true
            )
            .is_err()
        );
        let bad = ScopeArgs {
            faction: Some("NoSuchFaction".into()),
            ..en()
        };
        assert!(search(&b, &bad, "laser", 5, false, &[]).is_err());
    }

    #[test]
    fn search_hits() {
        let Some(b) = backend() else { return };
        let hits = search(&b, &en(), "ad astra", 10, false, &[]).unwrap();
        assert_eq!(hits[0].entity.key, "TITechTemplate:AdAstra");
        assert!(["name", "dataName", "text", "value"].contains(&hits[0].matched.as_str()));
        let lasers = search(&b, &en(), "laser", 30, false, &[]).unwrap();
        assert!(lasers.len() > 5 && lasers.len() <= 30);
        assert!(search(&b, &en(), "   ", 10, false, &[]).unwrap().is_empty());
        // The kind filter applies before the limit: a small limit still fills up with parts.
        let parts = search(&b, &en(), "laser", 5, false, &["shipPart".into()]).unwrap();
        assert_eq!(parts.len(), 5);
        assert!(parts.iter().all(|h| h.entity.kind == "shipPart"));
    }

    #[test]
    fn solar_system() {
        let Some(b) = backend() else { return };
        let s = get_solar_system(&b, &en()).unwrap();
        assert_eq!(s.start_date, "2022-09-30");
        assert!(s.bodies.len() > 450, "{}", s.bodies.len());
        assert_eq!((s.bodies[0].entity.data_name.as_str(), s.bodies[0].body_type.as_str()), ("Sol", "star"));
        assert!(s.bodies[0].parent.is_none() && s.bodies[0].orbit.is_none());
        let index: HashMap<&str, usize> = s.bodies.iter().enumerate().map(|(i, b)| (b.entity.key.as_str(), i)).collect();
        for (i, body) in s.bodies.iter().enumerate().skip(1) {
            let parent = body.parent.as_deref().unwrap_or_else(|| panic!("{} has no parent", body.entity.key));
            assert!(index[parent] < i, "{} before its parent", body.entity.key);
        }
        let earth = s.bodies.iter().find(|b| b.entity.data_name == "Earth").unwrap();
        assert_eq!(earth.parent.as_deref(), Some("TISpaceBodyTemplate:Sol"));
        assert!((earth.orbit.as_ref().unwrap().period_days - 365.25).abs() < 0.2);
        assert!(earth.entity.icon.is_some());
        let luna = s.bodies.iter().find(|b| b.entity.data_name == "Luna").unwrap();
        assert!(luna.hab_site_count > 0 && luna.resources.len() == 5, "{:?}", luna.resources);
        assert!(luna.orbit.as_ref().unwrap().nodal_precession_years.is_some());
        let ratings = ["empty", "possible", "low", "medium", "high", "max"];
        assert!(s.bodies.iter().flat_map(|b| b.resources.values()).all(|r| ratings.contains(&r.as_str())));
        assert!(s.bodies.iter().any(|b| b.resources.values().any(|r| r == "max")));
        assert!(s.lagrange_points.len() > 100);
        assert!(
            s.lagrange_points
                .iter()
                .all(|l| index.contains_key(l.body.as_str()) && l.point.starts_with('L'))
        );

        let scenario = ScopeArgs {
            scenario: Some("2003Scenario".into()),
            ..en()
        };
        assert_eq!(get_solar_system(&b, &scenario).unwrap().start_date, "2003-03-31");
    }
}
