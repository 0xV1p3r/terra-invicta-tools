//! The tech + project prerequisite graph of a [`View`].
//!
//! Game rule (`TechPrereqs`, `TechPrereqsSatisfied`): `prereqs` is first filtered —
//! empty names, `"PLACEHOLDER"` and names that do not exist in the scope's
//! scenario(s) are dropped (the game logs and drops them). Every remaining
//! entry is required; entry `0` *of the filtered list* may instead be satisfied
//! by `altPrereq0` and entry `1` by `altPrereq1`. Names resolve to a tech or a
//! project. Techs never depend on projects.
//!
//! A prereq that exists but is hidden from the selected faction (`factionPrereq`)
//! stays in the list – the faction can never complete it, so it only counts if
//! the group has another option.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::OnceLock;

use serde_json::Value;

use crate::dataset::EntityId;
use crate::model;
use crate::view::View;

/// One AND-term of a node's requirements, satisfied by any of `options`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrereqGroup {
    /// Prereqs that can satisfy this term in this scope.
    pub options: Vec<EntityId>,
    /// Parallel to `options`: `true` if the option comes from `altPrereqN`
    /// rather than the listed prereq.
    pub alternative: Vec<bool>,
    /// Prereqs that exist in this scope but can never be completed here: hidden
    /// from the selected faction (or the node itself). They do not count as options.
    pub hidden: Vec<EntityId>,
    /// `altPrereqN` names that do not exist in this scope (ignored, as in the game).
    pub unresolved: Vec<String>,
}

impl PrereqGroup {
    /// No usable option in this scope: the requirement cannot be met here.
    pub fn is_unsatisfiable(&self) -> bool {
        self.options.is_empty()
    }

    /// Whether `options[k]` comes from `altPrereqN`.
    pub fn is_alternative(&self, k: usize) -> bool {
        self.alternative.get(k).copied().unwrap_or(false)
    }

    /// Options with their provenance (`true` = alternative).
    pub fn options_with_kind(&self) -> impl Iterator<Item = (EntityId, bool)> + '_ {
        self.options.iter().enumerate().map(|(k, p)| (*p, self.is_alternative(k)))
    }

    /// The listed prereq if usable, else the first alternative (used for layering).
    pub fn primary(&self) -> Option<EntityId> {
        self.options_with_kind()
            .find(|(_, alt)| !alt)
            .or_else(|| self.options_with_kind().next())
            .map(|(p, _)| p)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dependent {
    pub node: EntityId,
    /// Depends via an `altPrereqN` rather than the listed prereq.
    pub alternative: bool,
}

/// Edge from a prerequisite to the node requiring it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Edge {
    pub from: EntityId,
    pub to: EntityId,
    pub alternative: bool,
}

/// A connected part of the graph, e.g. everything required for one project.
#[derive(Debug, Clone, Default)]
pub struct Subgraph {
    pub nodes: BTreeSet<EntityId>,
    pub edges: BTreeSet<Edge>,
}

#[derive(Debug, Clone, Copy)]
pub struct Traversal {
    /// Follow `altPrereqN` edges too (otherwise only the listed prereqs).
    pub alternatives: bool,
    /// Stop after this many steps (`None` = unlimited).
    pub max_depth: Option<usize>,
}

impl Default for Traversal {
    fn default() -> Self {
        Self {
            alternatives: true,
            max_depth: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct ResearchGraph {
    nodes: Vec<EntityId>,
    index: HashMap<EntityId, usize>,
    requirements: Vec<Vec<PrereqGroup>>,
    /// Listed prereq names dropped because they do not exist in this scope.
    dropped: Vec<Vec<String>>,
    dependents: Vec<Vec<Dependent>>,
    depth: Vec<usize>,
    cyclic: BTreeSet<EntityId>,
    obtainable: OnceLock<BTreeSet<EntityId>>,
    redundant: OnceLock<BTreeSet<Edge>>,
}

const IGNORED_NAMES: [&str; 2] = ["", "PLACEHOLDER"];

impl ResearchGraph {
    pub(crate) fn build(view: &View<'_>) -> Self {
        let mut g = ResearchGraph::default();
        for template in [model::TECH, model::PROJECT] {
            for id in view.entities(template) {
                g.index.insert(id, g.nodes.len());
                g.nodes.push(id);
            }
        }
        g.dependents = vec![Vec::new(); g.nodes.len()];
        for &id in &g.nodes {
            let record = view.record(id).expect("node exists in view");
            let prereqs = record.get("prereqs").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str);
            // The game's `TechPrereqs`: ignored and non-existent names are dropped first;
            // `altPrereqN` pairs with entry N of the filtered list.
            let mut dropped = Vec::new();
            let mut listed = Vec::new();
            for name in prereqs.filter(|n| !IGNORED_NAMES.contains(n)) {
                match view.resolve_research(name) {
                    Some(p) => listed.push(p),
                    None => dropped.push(name.to_string()),
                }
            }
            let usable = |p: EntityId| p != id && view.faction_allows(p);
            let mut groups = Vec::with_capacity(listed.len());
            for (i, &p) in listed.iter().enumerate() {
                let mut group = PrereqGroup {
                    options: Vec::new(),
                    alternative: Vec::new(),
                    hidden: Vec::new(),
                    unresolved: Vec::new(),
                };
                let alt = (i < 2)
                    .then(|| record.get(format!("altPrereq{i}")).and_then(Value::as_str))
                    .flatten()
                    .filter(|a| !IGNORED_NAMES.contains(a));
                let alt = match alt {
                    Some(name) => match view.resolve_research(name) {
                        Some(a) => Some(a),
                        None => {
                            group.unresolved.push(name.to_string());
                            None
                        }
                    },
                    None => None,
                };
                for (o, is_alt) in std::iter::once((p, false)).chain(alt.map(|a| (a, true))) {
                    if usable(o) && !group.options.contains(&o) {
                        group.options.push(o);
                        group.alternative.push(is_alt);
                        g.dependents[g.index[&o]].push(Dependent { node: id, alternative: is_alt });
                    } else if !usable(o) {
                        group.hidden.push(o);
                    }
                }
                groups.push(group);
            }
            g.requirements.push(groups);
            g.dropped.push(dropped);
        }
        g.compute_depth();
        g.cyclic = g.compute_cyclic();
        g
    }

    /// Longest-path layering via each group's [`PrereqGroup::primary`] option (roots = 0);
    /// back edges of cycles are ignored.
    fn compute_depth(&mut self) {
        const UNSEEN: usize = usize::MAX;
        const ON_STACK: usize = usize::MAX - 1;
        let n = self.nodes.len();
        let mut depth = vec![UNSEEN; n];
        let primary: Vec<Vec<usize>> = self
            .requirements
            .iter()
            .map(|groups| groups.iter().filter_map(PrereqGroup::primary).map(|p| self.index[&p]).collect())
            .collect();
        for root in 0..n {
            if depth[root] != UNSEEN {
                continue;
            }
            // iterative DFS: (node, next prereq index)
            let mut stack = vec![(root, 0usize)];
            depth[root] = ON_STACK;
            while let Some(&mut (i, ref mut k)) = stack.last_mut() {
                if let Some(&j) = primary[i].get(*k) {
                    *k += 1;
                    if depth[j] == UNSEEN {
                        depth[j] = ON_STACK;
                        stack.push((j, 0));
                    }
                } else {
                    stack.pop();
                    depth[i] = primary[i].iter().map(|&j| depth[j]).filter(|d| *d < ON_STACK).map(|d| d + 1).max().unwrap_or(0);
                }
            }
        }
        self.depth = depth;
    }

    /// Nodes in a strongly connected component of size > 1 (Tarjan, iterative), over all
    /// prereq edges including alternatives.
    fn compute_cyclic(&self) -> BTreeSet<EntityId> {
        const UNVISITED: usize = usize::MAX;
        let n = self.nodes.len();
        let succ: Vec<Vec<usize>> = self
            .requirements
            .iter()
            .map(|groups| groups.iter().flat_map(|g| g.options.iter()).map(|p| self.index[p]).collect())
            .collect();
        let mut index = vec![UNVISITED; n];
        let mut low = vec![0usize; n];
        let mut on_stack = vec![false; n];
        let mut stack: Vec<usize> = Vec::new();
        let mut next = 0usize;
        let mut cyclic = BTreeSet::new();
        for root in 0..n {
            if index[root] != UNVISITED {
                continue;
            }
            let mut call: Vec<(usize, usize)> = vec![(root, 0)];
            index[root] = next;
            low[root] = next;
            next += 1;
            stack.push(root);
            on_stack[root] = true;
            while let Some(&mut (v, ref mut k)) = call.last_mut() {
                if let Some(&w) = succ[v].get(*k) {
                    *k += 1;
                    if index[w] == UNVISITED {
                        index[w] = next;
                        low[w] = next;
                        next += 1;
                        stack.push(w);
                        on_stack[w] = true;
                        call.push((w, 0));
                    } else if on_stack[w] {
                        low[v] = low[v].min(index[w]);
                    }
                } else {
                    call.pop();
                    if let Some(&(parent, _)) = call.last() {
                        low[parent] = low[parent].min(low[v]);
                    }
                    if low[v] == index[v] {
                        let mut component = Vec::new();
                        loop {
                            let w = stack.pop().expect("tarjan stack");
                            on_stack[w] = false;
                            component.push(w);
                            if w == v {
                                break;
                            }
                        }
                        if component.len() > 1 || succ[v].contains(&v) {
                            cyclic.extend(component.into_iter().map(|w| self.nodes[w]));
                        }
                    }
                }
            }
        }
        cyclic
    }

    /// Tech and project nodes of the scope.
    pub fn nodes(&self) -> &[EntityId] {
        &self.nodes
    }

    pub fn contains(&self, id: EntityId) -> bool {
        self.index.contains_key(&id)
    }

    /// AND-list of OR-groups that must be completed before `id`.
    pub fn requirements(&self, id: EntityId) -> &[PrereqGroup] {
        self.index.get(&id).map(|&i| self.requirements[i].as_slice()).unwrap_or(&[])
    }

    /// Listed prereq names of `id` that do not exist in this scope and were dropped
    /// (diagnostics; they impose no requirement).
    pub fn dropped_prereqs(&self, id: EntityId) -> &[String] {
        self.index.get(&id).map(|&i| self.dropped[i].as_slice()).unwrap_or(&[])
    }

    /// Nodes that list `id` as a prereq or alternative.
    pub fn dependents(&self, id: EntityId) -> &[Dependent] {
        self.index.get(&id).map(|&i| self.dependents[i].as_slice()).unwrap_or(&[])
    }

    /// Layer for drawing: length of the longest chain of primary prereqs (the listed
    /// prereq, else its alternative) leading to `id`.
    pub fn depth(&self, id: EntityId) -> Option<usize> {
        self.index.get(&id).map(|&i| self.depth[i])
    }

    /// Nodes without prerequisites.
    pub fn roots(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.nodes.iter().copied().filter(|id| self.requirements(*id).is_empty())
    }

    /// Nodes on prereq cycles, i.e. in a strongly connected component of the prereq
    /// graph (alternatives included); empty for valid data.
    pub fn cyclic_nodes(&self) -> &BTreeSet<EntityId> {
        &self.cyclic
    }

    /// Everything needed for `id` (transitively), including `id`.
    pub fn ancestors(&self, id: EntityId, t: Traversal) -> Subgraph {
        self.walk(id, t, |g, node| {
            g.requirements(node)
                .iter()
                .flat_map(PrereqGroup::options_with_kind)
                .map(|(p, alt)| {
                    (
                        p,
                        Edge {
                            from: p,
                            to: node,
                            alternative: alt,
                        },
                    )
                })
                .collect()
        })
    }

    /// Everything that (transitively) builds on `id`, including `id`.
    pub fn descendants(&self, id: EntityId, t: Traversal) -> Subgraph {
        self.walk(id, t, |g, node| {
            g.dependents(node)
                .iter()
                .map(|d| {
                    (
                        d.node,
                        Edge {
                            from: node,
                            to: d.node,
                            alternative: d.alternative,
                        },
                    )
                })
                .collect()
        })
    }

    fn walk(&self, start: EntityId, t: Traversal, next: impl Fn(&Self, EntityId) -> Vec<(EntityId, Edge)>) -> Subgraph {
        let mut sub = Subgraph::default();
        if !self.contains(start) {
            return sub;
        }
        sub.nodes.insert(start);
        let mut queue = VecDeque::from([(start, 0usize)]);
        while let Some((node, d)) = queue.pop_front() {
            if t.max_depth.is_some_and(|m| d >= m) {
                continue;
            }
            for (other, edge) in next(self, node) {
                if edge.alternative && !t.alternatives {
                    continue;
                }
                sub.edges.insert(edge);
                if sub.nodes.insert(other) {
                    queue.push_back((other, d + 1));
                }
            }
        }
        sub
    }

    /// The whole graph as a subgraph (for drawing the full tree).
    pub fn full(&self) -> Subgraph {
        let mut sub = Subgraph {
            nodes: self.nodes.iter().copied().collect(),
            edges: BTreeSet::new(),
        };
        for (i, groups) in self.requirements.iter().enumerate() {
            for (p, alt) in groups.iter().flat_map(PrereqGroup::options_with_kind) {
                sub.edges.insert(Edge {
                    from: p,
                    to: self.nodes[i],
                    alternative: alt,
                });
            }
        }
        sub
    }

    /// Edges implied by a singleton requirement in another group. Alternatives are never redundant.
    pub fn redundant_edges(&self) -> &BTreeSet<Edge> {
        self.redundant.get_or_init(|| {
            let mandatory: Vec<Vec<usize>> = self
                .requirements
                .iter()
                .map(|groups| groups.iter().filter(|g| g.options.len() == 1).map(|g| self.index[&g.options[0]]).collect())
                .collect();
            // Separate bounded traversals keep ancestor sets complete even across cycles.
            let ancestors: Vec<BTreeSet<usize>> = (0..self.nodes.len())
                .map(|root| {
                    let mut seen = BTreeSet::new();
                    let mut stack = mandatory[root].clone();
                    while let Some(i) = stack.pop() {
                        if seen.insert(i) {
                            stack.extend(&mandatory[i]);
                        }
                    }
                    seen
                })
                .collect();
            let mut redundant = BTreeSet::new();
            for (i, groups) in self.requirements.iter().enumerate() {
                for (k, group) in groups.iter().enumerate() {
                    for (a, alternative) in group.options_with_kind() {
                        if alternative {
                            continue;
                        }
                        if groups.iter().enumerate().any(|(j, other)| {
                            j != k && other.options.len() == 1 && other.options[0] != a && ancestors[self.index[&other.options[0]]].contains(&self.index[&a])
                        }) {
                            redundant.insert(Edge {
                                from: a,
                                to: self.nodes[i],
                                alternative: false,
                            });
                        }
                    }
                }
            }
            // Repeated singleton requirements collapse to the same Edge; retain that edge.
            redundant
        })
    }

    /// Whether `id` can ever be researched in this scope: every group has an option
    /// that is itself obtainable. Faction filters make foreign projects unusable.
    pub fn is_obtainable(&self, id: EntityId) -> bool {
        self.obtainable_set().contains(&id)
    }

    /// All obtainable nodes: the least fixed point of "every group has an obtainable
    /// option" (cycles never justify themselves). Computed once per graph.
    pub fn obtainable_set(&self) -> &BTreeSet<EntityId> {
        self.obtainable.get_or_init(|| {
            let mut ok = vec![false; self.nodes.len()];
            loop {
                let mut changed = false;
                for i in 0..self.nodes.len() {
                    if !ok[i] && self.requirements[i].iter().all(|g| g.options.iter().any(|p| ok[self.index[p]])) {
                        ok[i] = true;
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
            self.nodes.iter().zip(&ok).filter(|(_, ok)| **ok).map(|(id, _)| *id).collect()
        })
    }
}

impl Subgraph {
    /// Sum of `researchCost` over nodes (base cost; repeatable/end-game scaling not applied).
    pub fn research_cost(&self, view: &View<'_>) -> f64 {
        self.nodes
            .iter()
            .filter_map(|id| view.record(*id)?.get("researchCost")?.as_f64())
            .filter(|c| *c > 0.0)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::scenario::META;
    use crate::test_support::{dataset, id, scenario};
    use crate::view::Scope;

    fn tech(name: &str, prereqs: &[&str]) -> Value {
        json!({"dataName": name, "prereqs": prereqs})
    }

    fn ds_with(techs: Vec<Value>, projects: Vec<Value>) -> crate::Dataset {
        let factions = vec![json!({"dataName": "F1"}), json!({"dataName": "F2"})];
        dataset(
            &[
                (META, vec![scenario("S", 1, &[], None)]),
                (model::TECH, techs),
                (model::PROJECT, projects),
                (model::FACTION, factions),
            ],
            json!({}),
        )
    }

    #[test]
    fn unresolved_prereqs_are_dropped_and_alternatives_follow_the_filtered_list() {
        let ds = ds_with(
            vec![
                tech("X", &[]),
                tech("Y", &[]),
                tech("Z", &[]),
                // filtered list is [X, Y]: altPrereq0 pairs with X, altPrereq1 with Y
                json!({"dataName": "T", "prereqs": ["", "Missing", "X", "PLACEHOLDER", "Y"], "altPrereq0": "Z", "altPrereq1": "Gone"}),
            ],
            vec![],
        );
        let view = ds.overview();
        let g = view.research_graph();
        let t = id(&ds, model::TECH, "T");
        let [x, y, z] = ["X", "Y", "Z"].map(|n| id(&ds, model::TECH, n));
        let req = g.requirements(t);
        assert_eq!(req.len(), 2);
        assert_eq!(req[0].options, vec![x, z]);
        assert_eq!(req[0].alternative, vec![false, true]);
        assert_eq!(req[1].options, vec![y]);
        assert_eq!(req[1].unresolved, vec!["Gone".to_string()]);
        assert_eq!(g.dropped_prereqs(t), ["Missing".to_string()]);
        assert!(g.is_obtainable(t));
    }

    #[test]
    fn faction_hidden_prereqs_make_the_group_unsatisfiable() {
        let ds = ds_with(
            vec![tech("A", &[])],
            vec![
                json!({"dataName": "Locked", "factionPrereq": ["F1"]}),
                json!({"dataName": "Alt"}),
                json!({"dataName": "P", "prereqs": ["A", "Locked"]}),
                json!({"dataName": "Q", "prereqs": ["Locked"], "altPrereq0": "Alt"}),
            ],
        );
        let s = ds.scenarios()[0].idx;
        let f2 = id(&ds, model::FACTION, "F2");
        let view = ds.view(Scope::scenario(s).with_faction(f2));
        let g = view.research_graph();
        let [locked, alt, p, q] = ["Locked", "Alt", "P", "Q"].map(|n| id(&ds, model::PROJECT, n));
        assert!(!g.contains(locked));
        let req = g.requirements(p);
        assert_eq!(req.len(), 2);
        assert!(req[1].is_unsatisfiable());
        assert_eq!(req[1].hidden, vec![locked]);
        assert!(g.dropped_prereqs(p).is_empty());
        assert!(!g.is_obtainable(p));

        // provenance survives the hidden primary: the alternative is the only option
        let req = g.requirements(q);
        assert_eq!(req[0].options, vec![alt]);
        assert_eq!(req[0].alternative, vec![true]);
        assert_eq!(req[0].primary(), Some(alt));
        assert!(g.is_obtainable(q));
        assert_eq!(g.depth(q), Some(1));
        let edge = Edge {
            from: alt,
            to: q,
            alternative: true,
        };
        assert!(g.ancestors(q, Traversal::default()).edges.contains(&edge));
        assert!(g.full().edges.contains(&edge));
        assert!(g.descendants(alt, Traversal::default()).edges.contains(&edge));
        assert!(
            g.ancestors(
                q,
                Traversal {
                    alternatives: false,
                    max_depth: None
                }
            )
            .edges
            .is_empty()
        );

        // F1 sees it
        let f1 = id(&ds, model::FACTION, "F1");
        let view = ds.view(Scope::scenario(s).with_faction(f1));
        assert!(view.research_graph().is_obtainable(p));
    }

    #[test]
    fn obtainability_is_a_fixed_point() {
        // R root; A needs B or R; B needs A; T needs A and B.
        let ds = ds_with(
            vec![
                tech("R", &[]),
                json!({"dataName": "A", "prereqs": ["B"], "altPrereq0": "R"}),
                tech("B", &["A"]),
                tech("T", &["A", "B"]),
                // a pure cycle is never obtainable
                tech("C1", &["C2"]),
                tech("C2", &["C1"]),
            ],
            vec![],
        );
        let view = ds.overview();
        let g = view.research_graph();
        let t = |n| id(&ds, model::TECH, n);
        // query T first: the old memoized DFS cached B/A as failures inside the cycle
        assert!(g.is_obtainable(t("T")));
        for n in ["R", "A", "B"] {
            assert!(g.is_obtainable(t(n)), "{n}");
        }
        assert!(!g.is_obtainable(t("C1")));
        assert!(!g.is_obtainable(t("C2")));
        assert_eq!(g.obtainable_set().len(), 4);
    }

    #[test]
    fn real_data_redundant_edges() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let Ok(root) = crate::DataRoot::open(path) else {
            eprintln!("no extracted data, skipping");
            return;
        };
        let ds = root.latest().expect("latest build loads");
        let view = ds.overview();
        let g = view.research_graph();
        let start = std::time::Instant::now();
        let redundant = g.redundant_edges();
        eprintln!("real data: {} redundant edges in {:?}", redundant.len(), start.elapsed());
        assert!(!redundant.is_empty());
        assert!(redundant.iter().all(|e| !e.alternative));
    }

    #[test]
    fn redundant_edges_follow_only_mandatory_requirements() {
        for alternatives in [false, true] {
            let c = if alternatives {
                json!({"dataName": "C", "prereqs": ["A", "B"], "altPrereq1": "D"})
            } else {
                tech("C", &["A", "B"])
            };
            let ds = ds_with(vec![tech("A", &[]), tech("B", &["A"]), c, tech("D", &[])], vec![]);
            let view = ds.overview();
            let g = view.research_graph();
            let edge = Edge {
                from: id(&ds, model::TECH, "A"),
                to: id(&ds, model::TECH, "C"),
                alternative: false,
            };
            assert_eq!(g.redundant_edges().contains(&edge), !alternatives);
            assert_eq!(g.redundant_edges().len(), usize::from(!alternatives));
        }
    }

    #[test]
    fn alternative_edges_are_never_redundant() {
        let ds = ds_with(
            vec![
                tech("A", &[]),
                tech("B", &["A"]),
                tech("D", &[]),
                json!({"dataName": "C", "prereqs": ["D", "B"], "altPrereq0": "A"}),
            ],
            vec![],
        );
        let view = ds.overview();
        assert!(view.research_graph().redundant_edges().is_empty());
    }

    #[test]
    fn redundant_edges_terminate_on_cycles() {
        let ds = ds_with(vec![tech("A", &["B"]), tech("B", &["A"]), tech("C", &["A", "B"])], vec![]);
        let view = ds.overview();
        let g = view.research_graph();
        let c = id(&ds, model::TECH, "C");
        assert_eq!(g.redundant_edges().len(), 2);
        assert!(g.redundant_edges().iter().all(|e| e.to == c));
    }

    #[test]
    fn duplicate_singletons_keep_the_collapsed_edge() {
        let ds = ds_with(vec![tech("A", &[]), tech("C", &["A", "A"])], vec![]);
        let view = ds.overview();
        let g = view.research_graph();
        assert_eq!(g.full().edges.len(), 1);
        assert!(g.redundant_edges().is_empty());
    }

    #[test]
    fn cycles_are_strongly_connected_components() {
        let ds = ds_with(
            vec![
                tech("C1", &["C2"]),
                tech("C2", &["C3"]),
                tech("C3", &["C1"]),
                tech("D", &["C1"]),
                tech("E", &[]),
            ],
            vec![],
        );
        let view = ds.overview();
        let g = view.research_graph();
        let cyclic: Vec<&str> = g.cyclic_nodes().iter().map(|i| &*ds.entity(*i).data_name).collect();
        assert_eq!(cyclic.len(), 3);
        for n in ["C1", "C2", "C3"] {
            assert!(cyclic.contains(&n), "{n} in {cyclic:?}");
        }
    }
}
