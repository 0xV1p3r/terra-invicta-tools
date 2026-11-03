//! Command logic, independent of the Tauri runtime (the `#[tauri::command]`
//! wrappers in `lib.rs` only fetch the state and move work off the main thread).

use std::collections::{BTreeSet, HashMap};

use serde_json::Value;
use ti_data::graph::{Edge, ResearchGraph};
use ti_data::{EntityId, Traversal, UnlockKind, View, model};

use crate::dto::*;
use crate::effects::effect_line;
use crate::error::{CmdError, CmdResult};
use crate::layout::{self, LayoutEdge};
use crate::properties;
use crate::refs::{self, entity_ref};
use crate::scope;
use crate::state::Backend;

pub fn get_meta(b: &Backend) -> CmdResult<Meta> {
    let ds = &*b.ds;
    let texts = b.texts("en")?;
    let view = ds.overview();
    let scenarios = ds
        .scenarios()
        .iter()
        .map(|s| {
            let label = view.name(&texts, s.meta);
            ScenarioInfo {
                name: s.data_name.clone(),
                label: if label == s.data_name && !s.friendly_name.is_empty() { s.friendly_name.clone() } else { label },
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
        data_dir: b.data_dir.display().to_string(),
        languages: ds.languages().to_vec(),
        scenarios,
        factions,
    })
}

pub fn get_tree(b: &Backend, scope: &ScopeArgs, filter: &TreeFilter) -> CmdResult<TreeGraph> {
    let texts = b.texts(&scope.lang)?;
    let view = b.ds.view(scope::resolve(&b.ds, scope)?);
    let graph = view.research_graph();
    let nodes: BTreeSet<EntityId> = graph
        .nodes()
        .iter()
        .copied()
        .filter(|&id| match view.kind(id) {
            model::Kind::Tech => filter.techs,
            model::Kind::Project => filter.projects,
            _ => false,
        })
        .filter(|&id| {
            filter.categories.is_empty()
                || refs::category(&view, id).is_some_and(|c| filter.categories.contains(&c))
        })
        .filter(|&id| {
            filter.properties.is_empty() || properties::of(&view, id).iter().any(|p| filter.properties.contains(p))
        })
        .collect();
    let edges: BTreeSet<Edge> =
        graph.full().edges.into_iter().filter(|e| nodes.contains(&e.from) && nodes.contains(&e.to)).collect();
    let mut tree = build_graph(&view, &texts, &nodes, &edges, None);
    tree.total_cost = tree.nodes.iter().map(|n| n.cost).filter(|c| *c > 0.0).sum();
    Ok(tree)
}

pub fn get_subtree(b: &Backend, scope: &ScopeArgs, args: &SubtreeArgs) -> CmdResult<TreeGraph> {
    let texts = b.texts(&scope.lang)?;
    let view = b.ds.view(scope::resolve(&b.ds, scope)?);
    let root = refs::parse_key(&view, &args.root)?;
    let graph = view.research_graph();
    if !graph.contains(root) {
        return Err(CmdError::msg(format!("{} is not a tech or project in this scope", args.root)));
    }
    let t = Traversal { alternatives: args.alternatives, max_depth: args.max_depth };
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
    let mut tree = build_graph(&view, &texts, &sub.nodes, &sub.edges, Some(root));
    tree.total_cost = required_cost(graph, &view, root);
    Ok(tree)
}

pub fn get_properties(b: &Backend, scope: &ScopeArgs) -> CmdResult<Vec<PropertyInfo>> {
    let texts = b.texts(&scope.lang)?;
    let view = b.ds.view(scope::resolve(&b.ds, scope)?);
    Ok(properties::catalog(&view, &texts))
}

/// Research cost of everything that must be researched for `id`, including it (listed prereqs only).
fn required_cost(graph: &ResearchGraph, view: &View<'_>, id: EntityId) -> f64 {
    graph.ancestors(id, Traversal { alternatives: false, max_depth: None }).research_cost(view)
}

fn research_cost(view: &View<'_>, id: EntityId) -> f64 {
    view.record(id).and_then(|r| r.get("researchCost")).and_then(Value::as_f64).unwrap_or(0.0)
}

fn record_flag(view: &View<'_>, id: EntityId, key: &str) -> bool {
    view.record(id).and_then(|r| r.get(key)).and_then(Value::as_bool).unwrap_or(false)
}

/// Lay out a node/edge set and fill in node data. `total_cost` is left at 0.
fn build_graph(
    view: &View<'_>,
    texts: &ti_data::Texts,
    nodes: &BTreeSet<EntityId>,
    edges: &BTreeSet<Edge>,
    root: Option<EntityId>,
) -> TreeGraph {
    let ds = view.dataset();
    // Language-independent input order: category, then dataName.
    let mut order: Vec<EntityId> = nodes.iter().copied().collect();
    order.sort_by_cached_key(|&id| (refs::category(view, id).unwrap_or_default(), ds.entity(id).data_name.clone()));
    let index: HashMap<EntityId, usize> = order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let layout_edges: Vec<LayoutEdge> = edges
        .iter()
        .filter_map(|e| Some(LayoutEdge { from: *index.get(&e.from)?, to: *index.get(&e.to)?, alternative: e.alternative }))
        .collect();
    let costs: Vec<f64> = order.iter().map(|&id| research_cost(view, id)).collect();
    let placements = layout::layout_by_cost(order.len(), &layout_edges, &costs);
    let obtainable = view.research_graph().obtainable_set();
    let tree_nodes = order
        .iter()
        .zip(&placements)
        .map(|(&id, p)| {
            let a = view.annotations(id);
            TreeNode {
                key: refs::key(view, id),
                data_name: ds.entity(id).data_name.to_string(),
                kind: if view.kind(id) == model::Kind::Tech { NodeKind::Tech } else { NodeKind::Project },
                name: view.name(texts, id),
                category: refs::category(view, id).unwrap_or_default(),
                cost: research_cost(view, id),
                column: p.column,
                x: p.x,
                y: p.y,
                flags: NodeFlags {
                    scenario_specific: a.scenario_specific,
                    varies_by_scenario: a.varies_by_scenario,
                    dlc: a.from_dlc,
                    faction_locked: !a.factions.is_empty(),
                    end_game: record_flag(view, id, "endGameTech"),
                    repeatable: record_flag(view, id, "repeatable"),
                    obtainable: obtainable.contains(&id),
                },
                properties: properties::of(view, id),
            }
        })
        .collect();
    let tree_edges = edges
        .iter()
        .filter(|e| index.contains_key(&e.from) && index.contains_key(&e.to))
        .map(|e| TreeEdge { from: refs::key(view, e.from), to: refs::key(view, e.to), alternative: e.alternative })
        .collect();
    TreeGraph {
        nodes: tree_nodes,
        edges: tree_edges,
        node_width: layout::NODE_WIDTH,
        node_height: layout::NODE_HEIGHT,
        root: root.map(|r| refs::key(view, r)),
        total_cost: 0.0,
    }
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
    let id = refs::parse_key(&view, key)?;
    // Faction-hidden entities are still shown (the annotations say who may research them).
    let record = view.record(id).ok_or_else(|| CmdError::msg(format!("{key} does not exist in this scenario")))?;
    let graph = view.research_graph();
    let a = view.annotations(id);
    let is_research = graph.contains(id);
    let r = |id| entity_ref(&view, &texts, id);
    Ok(EntityDetail {
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
        effects: view
            .effects(id)
            .into_iter()
            .map(|e| effect_line(&view, &texts, e, r(e)))
            .collect(),
        unlocks: view
            .unlocks(id)
            .into_iter()
            .map(|u| UnlockItem { kind: unlock_kind_name(u.kind).into(), entity: r(u.entity) })
            .collect(),
        unlocked_by: view.unlocked_by(id).into_iter().map(r).collect(),
        research_cost: is_research.then(|| research_cost(&view, id)),
        total_cost: is_research.then(|| required_cost(graph, &view, id)),
        record: record.clone(),
    })
}

pub fn search(b: &Backend, scope: &ScopeArgs, query: &str, limit: usize) -> CmdResult<Vec<SearchHit>> {
    let texts = b.texts(&scope.lang)?;
    let index = b.search_index(&scope.lang)?;
    let view = b.ds.view(scope::resolve(&b.ds, scope)?);
    let hits = index.search(query, limit, &|id| view.contains(id));
    Ok(hits
        .into_iter()
        .map(|h| SearchHit {
            entity: entity_ref(&view, &texts, h.id),
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
        ScopeArgs { scenario: None, faction: None, lang: "en".into() }
    }

    #[test]
    fn meta() {
        let Some(b) = backend() else { return };
        let m = get_meta(&b).unwrap();
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
        let filter = TreeFilter { techs: true, projects: true, categories: vec![], properties: vec![] };
        let t = std::time::Instant::now();
        let tree = get_tree(&b, &en(), &filter).unwrap();
        eprintln!("get_tree (overview, all): {} nodes, {} edges in {:?}", tree.nodes.len(), tree.edges.len(), t.elapsed());
        assert!(tree.nodes.len() > 800);
        assert!(tree.total_cost > 0.0);
        let keys: std::collections::HashSet<&str> = tree.nodes.iter().map(|n| n.key.as_str()).collect();
        assert!(tree.edges.iter().all(|e| keys.contains(e.from.as_str()) && keys.contains(e.to.as_str())));
        assert!(tree.edges.iter().any(|e| e.alternative));
        let by_key: HashMap<&str, &TreeNode> = tree.nodes.iter().map(|n| (n.key.as_str(), n)).collect();
        for e in tree.edges.iter().filter(|e| !e.alternative) {
            assert!(by_key[e.from.as_str()].column < by_key[e.to.as_str()].column, "{} -> {}", e.from, e.to);
        }
        assert!(tree.nodes.iter().any(|n| n.flags.end_game));
        assert!(tree.nodes.iter().any(|n| n.flags.repeatable));
        assert!(tree.nodes.iter().any(|n| n.flags.faction_locked));
        assert!(tree.nodes.iter().filter(|n| n.flags.obtainable).count() > 800);

        let techs = get_tree(&b, &en(), &TreeFilter { techs: true, projects: false, categories: vec![], properties: vec![] }).unwrap();
        assert!(techs.nodes.iter().all(|n| n.kind == NodeKind::Tech));
        let energy =
            get_tree(&b, &en(), &TreeFilter { techs: true, projects: true, categories: vec!["Energy".into()], properties: vec![] }).unwrap();
        assert!(!energy.nodes.is_empty() && energy.nodes.iter().all(|n| n.category == "Energy"));
    }

    #[test]
    fn properties() {
        let Some(b) = backend() else { return };
        let catalog = get_properties(&b, &en()).unwrap();
        let find = |id: &str| catalog.iter().find(|p| p.id == id).unwrap_or_else(|| panic!("{id} missing"));
        let cp = find("effect:ControlPointMaintenance:up");
        assert_eq!(cp.label, "Increases control point capacity");
        assert!(cp.count > 20, "{}", cp.count);
        assert!(cp.example.as_deref().is_some_and(|e| e.contains("control point")), "{:?}", cp.example);
        assert_eq!(find("effect:MCFreeSpaceMineNetwork:up").label, "Increases mining modules free of Mission Control cost");
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
        let tree = get_tree(&b, &en(), &filter).unwrap();
        let mine = find("effect:MCFreeSpaceMineNetwork:up").count;
        assert!(tree.nodes.len() >= cp.count.max(mine) && tree.nodes.len() <= cp.count + mine);
        assert!(tree.nodes.iter().all(|n| n.properties.iter().any(|p| filter.properties.contains(p))));
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
            .map(|e| LayoutEdge { from: index[&e.from], to: index[&e.to], alternative: e.alternative })
            .collect();
        let costs: Vec<f64> = full.nodes.iter().map(|&id| research_cost(&view, id)).collect();
        let t = std::time::Instant::now();
        let p = layout::layout_by_cost(full.nodes.len(), &edges, &costs);
        let elapsed = t.elapsed();
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
        let args = |root: &str, direction| SubtreeArgs { root: root.into(), direction, alternatives: true, max_depth: None };
        let up = get_subtree(&b, &en(), &args("TIProjectTemplate:Project_ProtiumConverterTorch", Direction::Up)).unwrap();
        assert_eq!(up.root.as_deref(), Some("TIProjectTemplate:Project_ProtiumConverterTorch"));
        assert!(up.nodes.len() > 10);
        assert!(up.total_cost > 0.0);
        let down = get_subtree(&b, &en(), &args("TITechTemplate:AdAstra", Direction::Down)).unwrap();
        let both = get_subtree(&b, &en(), &args("TITechTemplate:AdAstra", Direction::Both)).unwrap();
        assert!(both.nodes.len() >= down.nodes.len());
        assert_eq!(down.total_cost, both.total_cost);
        let shallow = get_subtree(
            &b,
            &en(),
            &SubtreeArgs { max_depth: Some(1), ..args("TITechTemplate:AdAstra", Direction::Down) },
        )
        .unwrap();
        let root = shallow.root.as_deref().unwrap();
        assert!(shallow.nodes.len() > 1);
        assert!(shallow.nodes.iter().all(|n| n.key == root || shallow.edges.iter().any(|e| e.from == root && e.to == n.key)));
        assert!(get_subtree(&b, &en(), &args("TITechTemplate:Nope", Direction::Up)).is_err());
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

        let with_effects = b
            .ds
            .overview()
            .entities(model::TECH)
            .find(|t| !b.ds.overview().effects(*t).is_empty())
            .unwrap();
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
        let scope = ScopeArgs { scenario: Some("ModernScenario".into()), faction: Some("ResistCouncil".into()), lang: "en".into() };
        let tree = get_tree(&b, &scope, &TreeFilter { techs: true, projects: true, categories: vec![], properties: vec![] }).unwrap();
        assert!(!tree.nodes.is_empty());
        let bad = ScopeArgs { scenario: Some("NoSuchScenario".into()), ..en() };
        assert!(get_tree(&b, &bad, &TreeFilter { techs: true, projects: true, categories: vec![], properties: vec![] }).is_err());
        let bad = ScopeArgs { faction: Some("NoSuchFaction".into()), ..en() };
        assert!(search(&b, &bad, "laser", 5).is_err());
    }

    #[test]
    fn search_hits() {
        let Some(b) = backend() else { return };
        let hits = search(&b, &en(), "ad astra", 10).unwrap();
        assert_eq!(hits[0].entity.key, "TITechTemplate:AdAstra");
        assert!(["name", "dataName", "text", "value"].contains(&hits[0].matched.as_str()));
        let lasers = search(&b, &en(), "laser", 30).unwrap();
        assert!(lasers.len() > 5 && lasers.len() <= 30);
        assert!(search(&b, &en(), "   ", 10).unwrap().is_empty());
    }
}
