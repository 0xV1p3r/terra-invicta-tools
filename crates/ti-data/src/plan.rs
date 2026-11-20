//! Research planning against what a campaign has already researched.
//!
//! [`Progress`] holds completion counts (repeatable projects and end-game techs can be
//! completed more than once). It is campaign state, kept apart from the [`Dataset`](crate::Dataset):
//! callers persist it by stable `(template, dataName)` keys and rebuild it per loaded dataset.
//!
//! Game rule (`TechPrereqsSatisfied`): a requirement group is met once any of its options
//! is completed. Project availability is also gated by unlock rolls, objectives and
//! nations (see `docs/research/game-semantics.md`); [`Status::Available`] only means the
//! prerequisites are met.

use std::collections::{BTreeSet, HashMap, HashSet};

use serde_json::Value;

use crate::dataset::EntityId;
use crate::graph::ResearchGraph;
use crate::view::View;

/// Completed research of one campaign: entity → times completed (at least 1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Progress {
    done: HashMap<EntityId, u32>,
}

impl Progress {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set how often `id` was completed; 0 removes it.
    pub fn set(&mut self, id: EntityId, count: u32) {
        if count == 0 {
            self.done.remove(&id);
        } else {
            self.done.insert(id, count);
        }
    }

    pub fn count(&self, id: EntityId) -> u32 {
        self.done.get(&id).copied().unwrap_or(0)
    }

    pub fn is_done(&self, id: EntityId) -> bool {
        self.done.contains_key(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (EntityId, u32)> + '_ {
        self.done.iter().map(|(id, n)| (*id, *n))
    }

    pub fn len(&self) -> usize {
        self.done.len()
    }

    pub fn is_empty(&self) -> bool {
        self.done.is_empty()
    }
}

/// Where a tech or project stands in a campaign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    Done,
    /// Every requirement group has a completed option (projects may still need an unlock roll).
    Available,
    /// Obtainable, but some requirement is not completed yet.
    Locked,
    /// Can never be researched in this scope (see [`ResearchGraph::is_obtainable`]).
    Unobtainable,
}

/// An OR-group the plan had to decide: none of its options is completed yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub node: EntityId,
    /// Index into [`ResearchGraph::requirements`] of `node`.
    pub group: usize,
    pub chosen: EntityId,
    /// `chosen` comes from the caller's picks rather than the cheapest option.
    pub picked: bool,
}

/// Everything still to research for some goals, prerequisites first.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// Research order: every step comes after the steps it needs. Goals are handled in the
    /// given order, so the first goal's chain comes first.
    pub steps: Vec<EntityId>,
    pub choices: Vec<Choice>,
    /// Goals that can never be researched in this scope (they and their chains are not in `steps`).
    pub unreachable: Vec<EntityId>,
    /// Step → the steps it needs directly (one option per open group).
    needs: HashMap<EntityId, Vec<EntityId>>,
}

impl Plan {
    /// The steps that `id` needs directly.
    pub fn needs(&self, id: EntityId) -> &[EntityId] {
        self.needs.get(&id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// For each step, how many later steps need it (transitively); parallel to `steps`.
    pub fn unblocks(&self) -> Vec<usize> {
        let mut needed_by: HashMap<EntityId, Vec<EntityId>> = HashMap::new();
        for (node, needs) in &self.needs {
            for n in needs {
                needed_by.entry(*n).or_default().push(*node);
            }
        }
        self.steps
            .iter()
            .map(|&start| {
                let mut seen = HashSet::new();
                let mut stack = vec![start];
                while let Some(n) = stack.pop() {
                    for &d in needed_by.get(&n).into_iter().flatten() {
                        if seen.insert(d) {
                            stack.push(d);
                        }
                    }
                }
                seen.len()
            })
            .collect()
    }
}

/// Picks for OR-groups: `(node, group index)` → option to research.
pub type Picks = HashMap<(EntityId, usize), EntityId>;

/// Planning queries over one scoped view and one campaign's progress.
pub struct Planner<'a, 'd> {
    view: &'a View<'d>,
    graph: &'a ResearchGraph,
    progress: &'a Progress,
}

impl<'a, 'd> Planner<'a, 'd> {
    pub fn new(view: &'a View<'d>, progress: &'a Progress) -> Self {
        Self {
            view,
            graph: view.research_graph(),
            progress,
        }
    }

    pub fn status(&self, id: EntityId) -> Status {
        if self.progress.is_done(id) {
            Status::Done
        } else if !self.graph.is_obtainable(id) {
            Status::Unobtainable
        } else if self.requirements_met(id, self.progress) {
            Status::Available
        } else {
            Status::Locked
        }
    }

    fn requirements_met(&self, id: EntityId, progress: &Progress) -> bool {
        self.graph.requirements(id).iter().all(|g| g.options.iter().any(|o| progress.is_done(*o)))
    }

    /// Cost of researching `id` once more: `researchCost × (1 + completions)` for repeatable
    /// projects and end-game techs (the latter repeat per category, one tech each), else `researchCost`.
    /// Campaign speed settings are not applied.
    pub fn cost(&self, id: EntityId) -> f64 {
        let record = self.view.record(id);
        let base = record.and_then(|r| r.get("researchCost")).and_then(Value::as_f64).unwrap_or(0.0).max(0.0);
        let flag = |key: &str| record.and_then(|r| r.get(key)).and_then(Value::as_bool).unwrap_or(false);
        if flag("repeatable") || flag("endGameTech") {
            base * f64::from(1 + self.progress.count(id))
        } else {
            base
        }
    }

    /// What is still missing for `goals`. Open OR-groups take the option in `picks` when it is
    /// usable, else the one with the cheapest missing chain.
    pub fn plan(&self, goals: &[EntityId], picks: &Picks) -> Plan {
        let mut plan = Plan::default();
        let mut placed = HashSet::new();
        let mut estimates = HashMap::new();
        for &goal in goals {
            if !self.graph.contains(goal) || self.progress.is_done(goal) || placed.contains(&goal) {
                continue;
            }
            if !self.graph.is_obtainable(goal) {
                if !plan.unreachable.contains(&goal) {
                    plan.unreachable.push(goal);
                }
                continue;
            }
            self.place(goal, picks, &mut plan, &mut placed, &mut estimates);
        }
        plan
    }

    /// Post-order DFS (iterative): a node is placed after the options chosen for its open groups.
    fn place(&self, root: EntityId, picks: &Picks, plan: &mut Plan, placed: &mut HashSet<EntityId>, estimates: &mut HashMap<EntityId, f64>) {
        let mut on_path = HashSet::from([root]);
        // (node, its chosen prereqs, next index into them)
        let first = self.open_needs(root, picks, plan, estimates);
        let mut stack = vec![(root, first, 0usize)];
        while let Some(top) = stack.last_mut() {
            if let Some(&next) = top.1.get(top.2) {
                top.2 += 1;
                // Valid data has no cycles; a cyclic edge is skipped rather than followed forever.
                if placed.contains(&next) || !on_path.insert(next) {
                    continue;
                }
                let needs = self.open_needs(next, picks, plan, estimates);
                stack.push((next, needs, 0));
            } else {
                let (node, needs, _) = stack.pop().expect("nonempty stack");
                on_path.remove(&node);
                placed.insert(node);
                plan.steps.push(node);
                plan.needs.insert(node, needs);
            }
        }
    }

    /// The option to research for each requirement group of `id` that has no completed option.
    fn open_needs(&self, id: EntityId, picks: &Picks, plan: &mut Plan, estimates: &mut HashMap<EntityId, f64>) -> Vec<EntityId> {
        let mut needs = Vec::new();
        for (group, g) in self.graph.requirements(id).iter().enumerate() {
            if g.options.iter().any(|o| self.progress.is_done(*o)) {
                continue;
            }
            let usable: Vec<EntityId> = g.options.iter().copied().filter(|o| self.graph.is_obtainable(*o)).collect();
            let picked = picks.get(&(id, group)).copied().filter(|p| usable.contains(p));
            let chosen = picked.or_else(|| {
                usable
                    .iter()
                    .copied()
                    .min_by(|a, b| self.estimate(*a, estimates).total_cmp(&self.estimate(*b, estimates)))
            });
            // `id` is obtainable, so every group has a usable option.
            let Some(chosen) = chosen else { continue };
            if g.options.len() > 1 {
                plan.choices.push(Choice {
                    node: id,
                    group,
                    chosen,
                    picked: picked.is_some(),
                });
            }
            if !needs.contains(&chosen) {
                needs.push(chosen);
            }
        }
        needs
    }

    /// Rough missing cost of `id` with the cheapest option per open group (shared ancestors
    /// count once per path); only used to compare options.
    fn estimate(&self, id: EntityId, memo: &mut HashMap<EntityId, f64>) -> f64 {
        if let Some(v) = memo.get(&id) {
            return *v;
        }
        // Guards cycles: a node on the current path counts as unreachable.
        memo.insert(id, f64::INFINITY);
        let value = if self.progress.is_done(id) {
            0.0
        } else {
            let mut total = self.cost(id);
            for g in self.graph.requirements(id) {
                if g.options.iter().any(|o| self.progress.is_done(*o)) {
                    continue;
                }
                let best = g
                    .options
                    .iter()
                    .filter(|o| self.graph.is_obtainable(**o))
                    .map(|o| self.estimate(*o, memo))
                    .fold(f64::INFINITY, f64::min);
                total += best;
            }
            total
        };
        memo.insert(id, value);
        value
    }

    /// Everything to mark completed when `id` is marked "researched up to here": `id` plus the
    /// steps of its plan (empty if `id` is already done or can never be researched).
    pub fn up_to(&self, id: EntityId, picks: &Picks) -> Vec<EntityId> {
        self.plan(&[id], picks).steps
    }

    /// Completed research that would lose a requirement if `id` were not completed
    /// (transitively), in no particular order; never contains `id`.
    pub fn dependents_lost(&self, id: EntityId) -> BTreeSet<EntityId> {
        let mut progress = self.progress.clone();
        progress.set(id, 0);
        let mut lost = BTreeSet::new();
        let mut frontier = vec![id];
        while let Some(n) = frontier.pop() {
            for d in self.graph.dependents(n) {
                if progress.is_done(d.node) && !self.requirements_met(d.node, &progress) {
                    progress.set(d.node, 0);
                    lost.insert(d.node);
                    frontier.push(d.node);
                }
            }
        }
        lost
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::model;
    use crate::scenario::META;
    use crate::test_support::{dataset, id};
    use crate::view::Scope;

    fn tech(name: &str, cost: f64, prereqs: &[&str]) -> Value {
        json!({"dataName": name, "researchCost": cost, "prereqs": prereqs})
    }

    fn ds_with(techs: Vec<Value>, projects: Vec<Value>) -> crate::Dataset {
        let factions = vec![json!({"dataName": "F1"}), json!({"dataName": "F2"})];
        dataset(
            &[
                (META, vec![crate::test_support::scenario("S", 1, &[], None)]),
                (model::TECH, techs),
                (model::PROJECT, projects),
                (model::FACTION, factions),
            ],
            json!({}),
        )
    }

    fn done(ids: &[EntityId]) -> Progress {
        let mut p = Progress::new();
        for &i in ids {
            p.set(i, 1);
        }
        p
    }

    #[test]
    fn status_follows_completed_options() {
        let ds = ds_with(
            vec![
                tech("A", 10.0, &[]),
                tech("B", 10.0, &[]),
                json!({"dataName": "C", "researchCost": 10.0, "prereqs": ["A"], "altPrereq0": "B"}),
                tech("D", 10.0, &["C", "A"]),
                tech("X1", 1.0, &["X2"]),
                tech("X2", 1.0, &["X1"]),
            ],
            vec![],
        );
        let view = ds.overview();
        let t = |n| id(&ds, model::TECH, n);
        let progress = done(&[t("B")]);
        let p = Planner::new(&view, &progress);
        assert_eq!(p.status(t("B")), Status::Done);
        assert_eq!(p.status(t("A")), Status::Available);
        // the alternative B satisfies C's only group
        assert_eq!(p.status(t("C")), Status::Available);
        assert_eq!(p.status(t("D")), Status::Locked);
        assert_eq!(p.status(t("X1")), Status::Unobtainable);
    }

    #[test]
    fn plan_orders_prerequisites_first_and_takes_the_cheaper_option() {
        // G needs (P or Q) and R; P's chain is expensive, Q's cheap.
        let ds = ds_with(
            vec![
                tech("Base", 5.0, &[]),
                tech("Big", 1000.0, &["Base"]),
                tech("P", 10.0, &["Big"]),
                tech("Q", 50.0, &["Base"]),
                tech("R", 1.0, &["Base"]),
                json!({"dataName": "G", "researchCost": 1.0, "prereqs": ["P", "R"], "altPrereq0": "Q"}),
            ],
            vec![],
        );
        let view = ds.overview();
        let t = |n| id(&ds, model::TECH, n);
        let progress = Progress::new();
        let p = Planner::new(&view, &progress);
        let plan = p.plan(&[t("G")], &Picks::new());
        assert_eq!(plan.steps, vec![t("Base"), t("Q"), t("R"), t("G")]);
        assert_eq!(
            plan.choices,
            vec![Choice {
                node: t("G"),
                group: 0,
                chosen: t("Q"),
                picked: false
            }]
        );
        assert_eq!(plan.unblocks(), vec![3, 1, 1, 0]);

        // a pick overrides the estimate
        let picks = Picks::from([((t("G"), 0), t("P"))]);
        let plan = p.plan(&[t("G")], &picks);
        assert_eq!(plan.steps, vec![t("Base"), t("Big"), t("P"), t("R"), t("G")]);
        assert!(plan.choices[0].picked);

        // completed options close the group; completed steps are skipped
        let progress = done(&[t("Base"), t("P")]);
        let p = Planner::new(&view, &progress);
        let plan = p.plan(&[t("G")], &Picks::new());
        assert_eq!(plan.steps, vec![t("R"), t("G")]);
        assert!(plan.choices.is_empty());
    }

    #[test]
    fn goals_share_steps_and_unreachable_goals_are_reported() {
        let ds = ds_with(
            vec![tech("A", 1.0, &[]), tech("B", 1.0, &["A"]), tech("C", 1.0, &["A"])],
            vec![
                json!({"dataName": "Mine", "researchCost": 1.0, "prereqs": ["B"]}),
                json!({"dataName": "Theirs", "factionPrereq": ["F1"]}),
                json!({"dataName": "NeedsTheirs", "prereqs": ["Theirs"]}),
            ],
        );
        let s = ds.scenarios()[0].idx;
        let f2 = id(&ds, model::FACTION, "F2");
        let view = ds.view(Scope::scenario(s).with_faction(f2));
        let t = |n| id(&ds, model::TECH, n);
        let pr = |n| id(&ds, model::PROJECT, n);
        let progress = Progress::new();
        let p = Planner::new(&view, &progress);
        let plan = p.plan(&[pr("Mine"), t("C"), pr("NeedsTheirs"), t("A")], &Picks::new());
        assert_eq!(plan.steps, vec![t("A"), t("B"), pr("Mine"), t("C")]);
        assert_eq!(plan.unreachable, vec![pr("NeedsTheirs")]);
        assert_eq!(plan.needs(pr("Mine")), [t("B")]);
    }

    #[test]
    fn repeatable_costs_scale_with_completions() {
        let ds = ds_with(
            vec![json!({"dataName": "E", "researchCost": 100.0, "endGameTech": true}), tech("T", 100.0, &[])],
            vec![json!({"dataName": "R", "researchCost": 10.0, "repeatable": true})],
        );
        let view = ds.overview();
        let (e, t, r) = (id(&ds, model::TECH, "E"), id(&ds, model::TECH, "T"), id(&ds, model::PROJECT, "R"));
        let mut progress = Progress::new();
        progress.set(e, 2);
        progress.set(t, 1);
        progress.set(r, 3);
        let p = Planner::new(&view, &progress);
        assert_eq!(p.cost(e), 300.0);
        assert_eq!(p.cost(t), 100.0);
        assert_eq!(p.cost(r), 40.0);
    }

    #[test]
    fn up_to_and_dependents_lost() {
        let ds = ds_with(
            vec![
                tech("A", 1.0, &[]),
                tech("B", 1.0, &[]),
                json!({"dataName": "C", "researchCost": 1.0, "prereqs": ["A"], "altPrereq0": "B"}),
                tech("D", 1.0, &["C"]),
                tech("E", 1.0, &["A"]),
            ],
            vec![],
        );
        let view = ds.overview();
        let t = |n| id(&ds, model::TECH, n);
        let progress = Progress::new();
        let p = Planner::new(&view, &progress);
        assert_eq!(p.up_to(t("D"), &Picks::new()), vec![t("A"), t("C"), t("D")]);

        // C keeps B as its other option, so only E loses its requirement
        let progress = done(&[t("A"), t("B"), t("C"), t("D"), t("E")]);
        let p = Planner::new(&view, &progress);
        assert_eq!(p.dependents_lost(t("A")), BTreeSet::from([t("E")]));
        let progress = done(&[t("A"), t("C"), t("D"), t("E")]);
        let p = Planner::new(&view, &progress);
        assert_eq!(p.dependents_lost(t("A")), BTreeSet::from([t("C"), t("D"), t("E")]));
        assert!(p.up_to(t("D"), &Picks::new()).is_empty());
    }
}
