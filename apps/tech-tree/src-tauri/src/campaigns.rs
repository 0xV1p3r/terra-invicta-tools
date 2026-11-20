//! Campaigns (research planning state): one JSON file per campaign in
//! `<app data dir>/campaigns/`, apart from the extracted data, and the plan commands.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use ti_data::{EntityId, Picks, Planner, Progress, Status, View, model};

use crate::dto::{Campaign, PlanChoice, PlanResult, PlanStatus, PlanStep, ScopeArgs};
use crate::error::{CmdError, CmdResult};
use crate::refs::{self, entity_ref};
use crate::scope;
use crate::state::Backend;

pub const CAMPAIGNS_SUBDIR: &str = "campaigns";

/// The campaigns folder; `None` when the app has no data dir. Writes are serialized.
#[derive(Debug)]
pub struct CampaignStore {
    dir: Option<PathBuf>,
    lock: Mutex<()>,
}

impl CampaignStore {
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self { dir, lock: Mutex::new(()) }
    }

    fn dir(&self) -> CmdResult<&Path> {
        self.dir.as_deref().ok_or_else(|| CmdError::msg("No app data folder to keep campaigns in."))
    }

    /// Every readable campaign, most recently saved first. Unreadable files are skipped.
    pub fn list(&self) -> CmdResult<Vec<Campaign>> {
        let dir = self.dir()?;
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(CmdError::msg(format!("Could not read {}: {e}", dir.display()))),
        };
        let mut out: Vec<Campaign> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .filter_map(|p| match std::fs::read(&p).map(|b| serde_json::from_slice::<Campaign>(&b)) {
                Ok(Ok(c)) if valid_id(&c.id) && p.file_stem().is_some_and(|s| *s == *c.id) => Some(c),
                Ok(Err(e)) => {
                    eprintln!("ti-tech-tree: ignoring invalid campaign {}: {e}", p.display());
                    None
                }
                _ => None,
            })
            .collect();
        out.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.name.cmp(&b.name)));
        Ok(out)
    }

    /// Write `campaign` (a new id when it has none) and return it as saved.
    pub fn save(&self, mut campaign: Campaign) -> CmdResult<Campaign> {
        let dir = self.dir()?;
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        if campaign.id.is_empty() {
            campaign.id = new_id(dir);
        } else if !valid_id(&campaign.id) {
            return Err(CmdError::msg(format!("Invalid campaign id {:?}", campaign.id)));
        }
        campaign.name = campaign.name.trim().to_string();
        if campaign.name.is_empty() {
            return Err(CmdError::msg("A campaign needs a name."));
        }
        campaign.updated = now();
        let fail = |e: std::io::Error| CmdError::msg(format!("Could not save the campaign to {}: {e}", dir.display()));
        std::fs::create_dir_all(dir).map_err(fail)?;
        let path = dir.join(format!("{}.json", campaign.id));
        // Via a temporary file, so a crash cannot leave half a campaign.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&campaign).expect("campaign serializes")).map_err(fail)?;
        std::fs::rename(&tmp, &path).map_err(fail)?;
        Ok(campaign)
    }

    pub fn delete(&self, id: &str) -> CmdResult<()> {
        if !valid_id(id) {
            return Err(CmdError::msg(format!("Invalid campaign id {id:?}")));
        }
        let path = self.dir()?.join(format!("{id}.json"));
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CmdError::msg(format!("Could not delete {}: {e}", path.display()))),
        }
    }
}

/// Ids are file names: ASCII letters, digits, `-` and `_` only.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// A time-based id not yet used in `dir`.
fn new_id(dir: &Path) -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    (0u32..)
        .map(|n| format!("c{:x}{}", nanos, if n == 0 { String::new() } else { format!("-{n}") }))
        .find(|id| !dir.join(format!("{id}.json")).exists())
        .expect("an unused id")
}

// ---- planning ---------------------------------------------------------------

/// The campaign's scope (its scenario and faction) and its state resolved against it.
struct Resolved {
    progress: Progress,
    goals: Vec<EntityId>,
    picks: Picks,
    unknown: Vec<String>,
}

fn campaign_scope(campaign: &Campaign, lang: &str) -> ScopeArgs {
    ScopeArgs {
        scenario: Some(campaign.scenario.clone()),
        faction: Some(campaign.faction.clone()),
        lang: lang.to_string(),
    }
}

/// A research key of the view's graph, or `None`.
fn research(view: &View<'_>, key: &str) -> Option<EntityId> {
    let id = refs::parse_key(view, key).ok()?;
    view.research_graph().contains(id).then_some(id)
}

fn resolve(view: &View<'_>, campaign: &Campaign) -> Resolved {
    let mut unknown = Vec::new();
    let mut lookup = |key: &str| {
        let id = research(view, key);
        if id.is_none() && !unknown.iter().any(|k| k == key) {
            unknown.push(key.to_string());
        }
        id
    };
    let mut progress = Progress::new();
    for c in &campaign.completed {
        if let Some(id) = lookup(&c.key) {
            progress.set(id, c.count.max(progress.count(id)));
        }
    }
    let goals = campaign.goals.iter().filter_map(|k| lookup(k)).collect();
    let mut picks = Picks::new();
    for p in &campaign.picks {
        if let (Some(node), Some(option)) = (lookup(&p.node), lookup(&p.option)) {
            picks.insert((node, p.group), option);
        }
    }
    Resolved {
        progress,
        goals,
        picks,
        unknown,
    }
}

fn status_dto(s: Status) -> PlanStatus {
    match s {
        Status::Done => PlanStatus::Done,
        Status::Available => PlanStatus::Available,
        Status::Locked => PlanStatus::Locked,
        Status::Unobtainable => PlanStatus::Unobtainable,
    }
}

pub fn get_plan(b: &Backend, lang: &str, campaign: &Campaign) -> CmdResult<PlanResult> {
    let scope_args = campaign_scope(campaign, lang);
    let texts = b.texts(lang)?;
    let view = b.ds.view(scope::resolve(&b.ds, &scope_args)?);
    let spoilers = &*b.spoilers(&view)?;
    let r = resolve(&view, campaign);
    let planner = Planner::new(&view, &r.progress);
    let graph = view.research_graph();
    let status: BTreeMap<String, PlanStatus> = graph.nodes().iter().map(|&id| (refs::key(&view, id), status_dto(planner.status(id)))).collect();
    let plan = planner.plan(&r.goals, &r.picks);
    let goals: HashSet<EntityId> = r.goals.iter().copied().collect();
    let unblocks = plan.unblocks();
    let (mut tech_cost, mut project_cost) = (0.0, 0.0);
    let steps: Vec<PlanStep> = plan
        .steps
        .iter()
        .zip(&unblocks)
        .map(|(&id, &unblocks)| {
            let cost = planner.cost(id);
            if view.kind(id) == model::Kind::Tech {
                tech_cost += cost;
            } else {
                project_cost += cost;
            }
            PlanStep {
                entity: entity_ref(&view, &texts, id, spoilers),
                cost,
                status: status_dto(planner.status(id)),
                unblocks,
                goal: goals.contains(&id),
            }
        })
        .collect();
    let mut next: Vec<(usize, usize, String)> = steps
        .iter()
        .enumerate()
        .filter(|(_, s)| s.status == PlanStatus::Available)
        .map(|(i, s)| (s.unblocks, i, s.entity.key.clone()))
        .collect();
    next.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let choices = plan
        .choices
        .iter()
        .map(|c| PlanChoice {
            node: entity_ref(&view, &texts, c.node, spoilers),
            group: c.group,
            options: graph.requirements(c.node)[c.group]
                .options
                .iter()
                .filter(|o| graph.is_obtainable(**o))
                .map(|&o| entity_ref(&view, &texts, o, spoilers))
                .collect(),
            chosen: refs::key(&view, c.chosen),
            picked: c.picked,
        })
        .collect();
    Ok(PlanResult {
        status,
        steps,
        next: next.into_iter().map(|(_, _, k)| k).collect(),
        choices,
        tech_cost,
        project_cost,
        unreachable: plan.unreachable.iter().map(|&id| entity_ref(&view, &texts, id, spoilers)).collect(),
        unknown: r.unknown,
    })
}

/// Keys to mark completed for "researched up to here": `key` and what it still needs.
pub fn plan_up_to(b: &Backend, campaign: &Campaign, key: &str) -> CmdResult<Vec<String>> {
    let view = b.ds.view(scope::resolve(&b.ds, &campaign_scope(campaign, "en"))?);
    let id = research(&view, key).ok_or_else(|| CmdError::msg(format!("{key} is not research of this campaign")))?;
    let r = resolve(&view, campaign);
    let planner = Planner::new(&view, &r.progress);
    Ok(planner.up_to(id, &r.picks).into_iter().map(|id| refs::key(&view, id)).collect())
}

/// Completed research that loses a requirement when `key` is no longer completed (not `key`).
pub fn plan_dependents_lost(b: &Backend, campaign: &Campaign, key: &str) -> CmdResult<Vec<String>> {
    let view = b.ds.view(scope::resolve(&b.ds, &campaign_scope(campaign, "en"))?);
    let id = research(&view, key).ok_or_else(|| CmdError::msg(format!("{key} is not research of this campaign")))?;
    let r = resolve(&view, campaign);
    let lost: BTreeSet<EntityId> = Planner::new(&view, &r.progress).dependents_lost(id);
    Ok(lost.into_iter().map(|id| refs::key(&view, id)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::tests::backend;
    use crate::dto::{Completion, PlanPick};

    fn campaign(b: &Backend) -> Campaign {
        let s = b.ds.scenarios().iter().find(|s| s.data_name == "2026Scenario").expect("2026 scenario");
        Campaign {
            name: "Test".into(),
            scenario: s.data_name.clone(),
            faction: "ResistCouncil".into(),
            ..Campaign::default()
        }
    }

    #[test]
    fn store_round_trips_and_rejects_bad_ids() {
        let dir = std::env::temp_dir().join(format!("ti-campaigns-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = CampaignStore::new(Some(dir.clone()));
        assert!(store.list().unwrap().is_empty());
        let saved = store
            .save(Campaign {
                name: "  Run  ".into(),
                scenario: "S".into(),
                faction: "F".into(),
                ..Campaign::default()
            })
            .unwrap();
        assert!(valid_id(&saved.id));
        assert_eq!(saved.name, "Run");
        assert_eq!(store.list().unwrap(), vec![saved.clone()]);
        assert!(
            store
                .save(Campaign {
                    id: "../x".into(),
                    ..saved.clone()
                })
                .is_err()
        );
        assert!(
            store
                .save(Campaign {
                    name: " ".into(),
                    ..saved.clone()
                })
                .is_err()
        );
        assert!(store.delete("../x").is_err());
        store.delete(&saved.id).unwrap();
        assert!(store.list().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plan_on_real_data() {
        let Some(b) = backend() else { return };
        let mut c = campaign(&b);
        let goal = "TITechTemplate:NuclearFusioninSpace";
        c.goals = vec![goal.into(), "TITechTemplate:NoSuchTech".into()];
        let up_to = plan_up_to(&b, &c, goal).unwrap();
        assert_eq!(up_to.last().map(String::as_str), Some(goal));
        let plan = get_plan(&b, "en", &c).unwrap();
        assert_eq!(plan.unknown, vec!["TITechTemplate:NoSuchTech".to_string()]);
        let keys: Vec<&str> = plan.steps.iter().map(|s| s.entity.key.as_str()).collect();
        assert_eq!(keys, up_to.iter().map(String::as_str).collect::<Vec<_>>());
        assert!(plan.steps.last().unwrap().goal);
        assert!(!plan.next.is_empty());
        assert!(plan.tech_cost > 0.0);

        // marking everything up to the goal empties the plan
        c.completed = up_to.iter().map(|k| Completion { key: k.clone(), count: 1 }).collect();
        let plan = get_plan(&b, "en", &c).unwrap();
        assert!(plan.steps.is_empty());
        assert_eq!(plan.status[goal], PlanStatus::Done);
        let first = &up_to[0];
        let lost = plan_dependents_lost(&b, &c, first).unwrap();
        assert!(lost.iter().any(|k| k == goal));

        // a pick for an unknown node is reported, not fatal
        c.picks = vec![PlanPick {
            node: "TITechTemplate:Gone".into(),
            group: 0,
            option: first.clone(),
        }];
        assert!(get_plan(&b, "en", &c).unwrap().unknown.iter().any(|k| k == "TITechTemplate:Gone"));
    }
}
