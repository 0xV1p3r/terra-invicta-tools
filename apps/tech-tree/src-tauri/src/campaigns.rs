//! Campaigns (research planning state): one JSON file per campaign in
//! `<app data dir>/campaigns/`, apart from the extracted data, and the plan commands.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use ti_data::{EntityId, Picks, Planner, Progress, Status, View, model};

use crate::commands::{GraphLayout, build_graph};
use crate::dto::{Campaign, Completion, PlanChoice, PlanResult, PlanStatus, PlanStep, SaveLink, SaveProgress, ScopeArgs, TreeGraph};
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
    /// From an imported save: points put in (and whether being researched) by research.
    points: HashMap<EntityId, (f64, bool)>,
    /// From an imported save: the projects it has unlocked.
    unlocked: Option<HashSet<EntityId>>,
    research_speed: f64,
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
    let save = campaign.save.as_ref();
    let points = save
        .into_iter()
        .flat_map(|s| &s.progress)
        .filter_map(|p| Some((lookup(&p.key)?, (p.points, p.active))))
        .collect();
    // Unlocked projects the game data lacks are only noise: not reported.
    let unlocked = save.map(|s| s.available.iter().filter_map(|k| research(view, k)).collect());
    Resolved {
        progress,
        goals,
        picks,
        points,
        unlocked,
        research_speed: save.map_or(1.0, |s| s.research_speed).max(f64::MIN_POSITIVE),
        unknown,
    }
}

/// The planner's status, with projects the imported save has not unlocked set apart.
fn status_dto(view: &View<'_>, r: &Resolved, id: EntityId, s: Status) -> PlanStatus {
    match s {
        Status::Done => PlanStatus::Done,
        Status::Available if view.kind(id) == model::Kind::Project && r.unlocked.as_ref().is_some_and(|u| !u.contains(&id)) => PlanStatus::NotUnlocked,
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
    let status: BTreeMap<String, PlanStatus> = graph
        .nodes()
        .iter()
        .map(|&id| (refs::key(&view, id), status_dto(&view, &r, id, planner.status(id))))
        .collect();
    let plan = planner.plan(&r.goals, &r.picks);
    let goals: HashSet<EntityId> = r.goals.iter().copied().collect();
    let unblocks = plan.unblocks();
    let (mut tech_cost, mut project_cost) = (0.0, 0.0);
    let steps: Vec<PlanStep> = plan
        .steps
        .iter()
        .zip(&unblocks)
        .map(|(&id, &unblocks)| {
            let (progress, active) = r.points.get(&id).copied().unwrap_or((0.0, false));
            let cost = (planner.cost(id) / r.research_speed - progress).max(0.0);
            if view.kind(id) == model::Kind::Tech {
                tech_cost += cost;
            } else {
                project_cost += cost;
            }
            PlanStep {
                entity: entity_ref(&view, &texts, id, spoilers),
                cost,
                progress,
                active,
                status: status_dto(&view, &r, id, planner.status(id)),
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
        research_speed: r.research_speed,
        unreachable: plan.unreachable.iter().map(|&id| entity_ref(&view, &texts, id, spoilers)).collect(),
        unknown: r.unknown,
    })
}

/// The plan's path as a tree of its own: the research still needed for the goals (the plan's
/// steps) and the requirements between them, laid out left to right. `total_cost` is the
/// plan's remaining cost.
pub fn get_plan_graph(b: &Backend, lang: &str, campaign: &Campaign, merge_edges: bool) -> CmdResult<TreeGraph> {
    let texts = b.texts(lang)?;
    let view = b.ds.view(scope::resolve(&b.ds, &campaign_scope(campaign, lang))?);
    let r = resolve(&view, campaign);
    let planner = Planner::new(&view, &r.progress);
    let plan = planner.plan(&r.goals, &r.picks);
    let nodes: BTreeSet<EntityId> = plan.steps.iter().copied().collect();
    let edges: BTreeSet<_> = view
        .research_graph()
        .full()
        .edges
        .into_iter()
        .filter(|e| nodes.contains(&e.from) && nodes.contains(&e.to))
        .collect();
    let mut tree = build_graph(&view, &texts, &nodes, &edges, None, GraphLayout { bands: false, merge_edges }, false);
    tree.total_cost = plan.steps.iter().map(|&id| planner.cost(id)).sum();
    Ok(tree)
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

// ---- save import ---------------------------------------------------------------

/// The game's save folder, if found (Windows Documents or a Steam Proton prefix).
pub fn default_save_dir() -> Option<String> {
    ti_extract::find_save_dir().map(|d| d.to_string_lossy().into_owned())
}

/// The campaign as of the save at `path`: `into` (when given) updated with the save's research
/// state, keeping its name, goals and picks, or a new campaign named after the faction and date.
/// The result is not saved yet.
pub fn import_save(b: &Backend, lang: &str, path: &str, into: Option<Campaign>) -> CmdResult<Campaign> {
    let save = ti_save::read(Path::new(path)).map_err(|e| CmdError::msg(e.to_string()))?;
    if b.ds.scenario_by_name(&save.scenario).is_none() {
        return Err(CmdError::msg(format!(
            "The save's scenario {:?} is not in the loaded game data.",
            save.scenario
        )));
    }
    let faction =
        b.ds.find(model::FACTION, &save.faction)
            .ok_or_else(|| CmdError::msg(format!("The save's faction {:?} is not in the loaded game data.", save.faction)))?;
    if let Some(c) = &into
        && (c.scenario != save.scenario || c.faction != save.faction)
    {
        return Err(CmdError::msg(format!(
            "This save is a {} campaign as {}, not the campaign's {} as {}.",
            save.scenario, save.faction, c.scenario, c.faction
        )));
    }
    let view = b.ds.view(scope::resolve(
        &b.ds,
        &ScopeArgs {
            scenario: Some(save.scenario.clone()),
            faction: Some(save.faction.clone()),
            lang: lang.into(),
        },
    )?);
    let tech = |name: &str| format!("{}:{name}", model::TECH);
    let project = |name: &str| format!("{}:{name}", model::PROJECT);

    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for name in &save.finished_techs {
        let key = tech(name);
        // End-game techs repeat; the save counts their completions per category.
        let repeats = research(&view, &key)
            .filter(|&id| view.record(id).and_then(|r| r.get("endGameTech")).and_then(serde_json::Value::as_bool) == Some(true))
            .and_then(|id| refs::category(&view, id))
            .and_then(|cat| save.end_game_completions.as_ref()?.get(&cat).copied())
            .unwrap_or(1);
        counts.insert(key, repeats.max(1));
    }
    // A repeatable project is listed once per completion; one-time projects someone else
    // completed satisfy prerequisites too.
    for name in save
        .finished_projects
        .iter()
        .chain(save.finished_one_time_projects.iter().filter(|n| !save.finished_projects.contains(n)))
    {
        *counts.entry(project(name)).or_default() += 1;
    }
    let progress = save
        .tech_slots
        .iter()
        .map(|t| SaveProgress {
            key: tech(&t.tech),
            points: t.accumulated,
            active: true,
        })
        .chain(save.project_slots.iter().map(|p| SaveProgress {
            key: project(&p.project),
            points: p.accumulated,
            active: p.active,
        }))
        .collect();
    let d = save.game_date;
    let link = SaveLink {
        path: path.to_string(),
        game_date: format!("{:04}-{:02}-{:02}", d.year, d.month, d.day),
        save_version: save.save_version.clone(),
        research_speed: save.research_speed.unwrap_or(1.0),
        scenario_inferred: save.scenario_inferred,
        modded: save.modded,
        imported: now(),
        progress,
        available: save.available_projects.iter().map(|n| project(n)).collect(),
    };
    let mut campaign = into.unwrap_or_else(|| {
        let texts = b.texts(lang).ok();
        let faction_name = texts.map_or_else(|| save.faction.clone(), |t| view.name(&t, faction));
        Campaign {
            name: format!("{faction_name} {}", link.game_date),
            scenario: save.scenario.clone(),
            faction: save.faction.clone(),
            ..Campaign::default()
        }
    });
    campaign.completed = counts.into_iter().map(|(key, count)| Completion { key, count }).collect();
    campaign.save = Some(link);
    Ok(campaign)
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

        // the path holds exactly the plan's steps, linked by their requirements
        let path = get_plan_graph(&b, "en", &c, true).unwrap();
        let mut path_keys: Vec<&str> = path.nodes.iter().map(|n| n.key.as_str()).collect();
        let mut step_keys = keys.clone();
        path_keys.sort_unstable();
        step_keys.sort_unstable();
        assert_eq!(path_keys, step_keys);
        assert!(!path.edges.is_empty());
        assert!(path.edges.iter().all(|e| keys.contains(&e.from.as_str()) && keys.contains(&e.to.as_str())));
        let goal_node = path.nodes.iter().find(|n| n.key == goal).unwrap();
        assert!(path.nodes.iter().all(|n| n.column <= goal_node.column));

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

    /// Imports the research notes' acceptance save when the developer's saves are present.
    #[test]
    fn import_local_save() {
        let Some(b) = backend() else { return };
        let Some(path) = std::env::var_os("HOME")
            .map(|h| {
                std::path::PathBuf::from(h)
                    .join(".local/share/Steam/steamapps/compatdata/1176470/pfx/drive_c/users/steamuser/Documents/My Games/TerraInvicta/Saves/RE_ASIA_EU_1-3.gz")
            })
            .filter(|p| p.is_file())
        else {
            return;
        };
        let path = path.to_string_lossy();
        let mut c = import_save(&b, "en", &path, None).unwrap();
        assert_eq!((c.scenario.as_str(), c.faction.as_str()), ("2026Scenario", "ResistCouncil"));
        let link = c.save.clone().unwrap();
        assert_eq!(link.progress.len(), 8);
        assert_eq!(link.available.len(), 113);
        assert_eq!(c.completed.iter().filter(|x| x.key.starts_with(model::TECH)).count(), 58);

        // the tech slot's points come off its cost
        let fusion = "TITechTemplate:NuclearFusioninSpace";
        c.goals = vec![fusion.into()];
        let plan = get_plan(&b, "en", &c).unwrap();
        let step = plan.steps.iter().find(|s| s.entity.key == fusion).unwrap();
        assert!(step.active && (step.progress - 28349.14).abs() < 0.01);
        assert!((step.cost - (50000.0 / link.research_speed - step.progress)).abs() < 0.01);
        assert!(plan.status.values().any(|s| *s == PlanStatus::NotUnlocked));

        // re-importing into the campaign keeps its goals; another faction's campaign is refused
        let again = import_save(&b, "en", &path, Some(c.clone())).unwrap();
        assert_eq!(again.goals, c.goals);
        let other = Campaign {
            faction: "HumanityFirstCouncil".into(),
            ..c
        };
        assert!(import_save(&b, "en", &path, Some(other)).is_err());
    }
}
