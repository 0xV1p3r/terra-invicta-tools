//! Mock data for developing the UI in a plain browser (no Tauri runtime).
//!
//! Writes the JSON outputs of the commands (overview scope, English) to
//! `apps/tech-tree/static/mock/`. Ignored by default; regenerate with
//!
//! ```text
//! cargo test -p ti-tech-tree --release -- --ignored write_fixtures
//! ```
//!
//! File names:
//! * `meta.json`                     – `get_meta`
//! * `tree.json`, `tree-techs.json`  – `get_tree` (techs+projects / techs only, all categories)
//! * `properties.json`               – `get_properties`
//! * `subtree-<up|down|both>-<dataName>.json` – `get_subtree` (alternatives on, no depth limit)
//! * `entity-<key with ':' → '__'>.json`        – `get_entity`, e.g. `entity-TITechTemplate__AdAstra.json`
//! * `search-<query, spaces → '_'>.json`         – `search` (limit 30), e.g. `search-ad_astra.json`
//! * `index.json` – lists every entity/subtree/search fixture written.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Serialize;
use ti_data::{EntityId, model};

use crate::commands::{self, tests::{backend, en}};
use crate::dto::*;
use crate::refs;

#[test]
#[ignore = "writes static/mock fixtures; run explicitly"]
fn write_fixtures() {
    let Some(b) = backend() else { return };
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../static/mock");
    std::fs::create_dir_all(&out).unwrap();
    let write = |name: &str, value: &dyn erased::Json| {
        let path = out.join(name);
        std::fs::write(&path, value.to_json()).unwrap();
        eprintln!("{:>9} {}", std::fs::metadata(&path).unwrap().len(), name);
    };
    let scope = en();
    write("meta.json", &commands::get_meta(&b).unwrap());
    let all = TreeFilter { techs: true, projects: true, categories: vec![], properties: vec![] };
    write("tree.json", &commands::get_tree(&b, &scope, &all).unwrap());
    let techs = TreeFilter { projects: false, ..all };
    write("tree-techs.json", &commands::get_tree(&b, &scope, &techs).unwrap());
    write("properties.json", &commands::get_properties(&b, &scope).unwrap());

    let mut index = Index::default();
    for (dir, direction, root) in [
        ("up", Direction::Up, "TIProjectTemplate:Project_ProtiumConverterTorch"),
        ("down", Direction::Down, "TITechTemplate:AdAstra"),
        ("both", Direction::Both, "TITechTemplate:OrbitalShipbuilding"),
    ] {
        let args = SubtreeArgs { root: root.into(), direction, alternatives: true, max_depth: None };
        let tree = commands::get_subtree(&b, &scope, &args).unwrap();
        let name = format!("subtree-{dir}-{}.json", root.split_once(':').unwrap().1);
        write(&name, &tree);
        index.subtrees.push(name);
    }

    // Interesting entities, picked by name where stable and by property otherwise.
    let view = b.ds.overview();
    let first = |template: &str, pred: &dyn Fn(EntityId) -> bool| view.entities(template).find(|id| pred(*id));
    let mut keys: BTreeSet<String> = [
        "TITechTemplate:AdAstra",
        "TIProjectTemplate:Project_ProtiumConverterTorch",
        "TIProjectTemplate:Project_Pherocytes",
        // Effects with good/bad modifiers and an inline resource sprite.
        "TIProjectTemplate:Project_BSBE_PlayingFromBehind_Resist",
        "TIProjectTemplate:Project_DeepSpaceMetallurgy",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let picks: Vec<Option<EntityId>> = vec![
        first(model::TECH, &|id| view.record(id).is_some_and(|r| r["endGameTech"] == true)),
        first(model::TECH, &|id| view.annotations(id).scenario_specific),
        // (not an alien one: Project_AlienMasterProject unlocks ~1900 ship parts)
        first(model::PROJECT, &|id| {
            !view.annotations(id).factions.is_empty() && !b.ds.entity(id).data_name.contains("Alien")
        }),
        first(model::PROJECT, &|id| view.annotations(id).from_dlc),
        first(model::PROJECT, &|id| view.annotations(id).varies_by_scenario),
        first(model::PROJECT, &|id| view.record(id).is_some_and(|r| r["repeatable"] == true)),
        first(model::PROJECT, &|id| view.annotations(id).faction_always.is_some()),
        first("TIDriveTemplate", &|id| !view.unlocked_by(id).is_empty()),
        first(model::HAB_MODULE, &|id| !view.unlocked_by(id).is_empty()),
        first(model::ORG, &|id| !view.unlocked_by(id).is_empty()),
        first("TILaserWeaponTemplate", &|_| true),
        first(model::TRAIT, &|id| !view.unlocked_by(id).is_empty()),
        view.effects(view.find(model::TECH, "AdAstra").unwrap()).first().copied(),
        first(model::FACTION, &|_| true),
    ];
    keys.extend(picks.into_iter().flatten().map(|id| refs::key(&view, id)));
    // Everything reachable in one click from three roots, so detail navigation works.
    let mut related = BTreeSet::new();
    for root in ["TITechTemplate:AdAstra", "TIProjectTemplate:Project_ProtiumConverterTorch", "TIProjectTemplate:Project_Pherocytes"] {
        let d = commands::get_entity(&b, &scope, root).unwrap();
        related.extend(d.requirements.iter().flat_map(|g| g.options.iter().map(|o| o.key.clone())));
        related.extend(d.unlocks.iter().map(|u| u.entity.key.clone()));
        related.extend(d.unlocked_by.iter().map(|u| u.key.clone()));
        related.extend(d.effects.iter().map(|e| e.entity.key.clone()));
    }
    keys.extend(related);
    for key in &keys {
        let d = commands::get_entity(&b, &scope, key).unwrap();
        let name = format!("entity-{}.json", key.replace(':', "__"));
        write(&name, &d);
        index.entities.push(key.clone());
    }

    for q in ["laser", "fusion", "ad astra"] {
        let name = format!("search-{}.json", q.replace(' ', "_"));
        write(&name, &commands::search(&b, &scope, q, 30).unwrap());
        index.searches.push(q.into());
    }
    write("index.json", &index);
    let total: u64 = std::fs::read_dir(&out).unwrap().map(|e| e.unwrap().metadata().unwrap().len()).sum();
    eprintln!("total {total} bytes in {}", out.display());
    assert!(total < 5_000_000);
}

#[derive(Default, Serialize)]
struct Index {
    /// Entity keys with an `entity-*.json` file.
    entities: Vec<String>,
    subtrees: Vec<String>,
    /// Queries with a `search-*.json` file.
    searches: Vec<String>,
}

mod erased {
    /// Object-safe serialization for the `write` helper.
    pub trait Json {
        fn to_json(&self) -> String;
    }

    impl<T: serde::Serialize> Json for T {
        fn to_json(&self) -> String {
            serde_json::to_string_pretty(self).unwrap()
        }
    }
}
