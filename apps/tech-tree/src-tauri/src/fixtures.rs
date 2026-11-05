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
//! * `wiki-index.json` – `get_wiki_index`
//! * `wiki-entries-<template>.json` – `get_wiki_entries` for representative templates
//! * `meta.json`                     – `get_meta`
//! * `tree.json`, `tree-techs.json`  – `get_tree` (techs+projects / techs only, all categories)
//! * `tree-bands.json`               – `get_tree` with the category-band layout (all research)
//! * `properties.json`               – `get_properties`
//! * `subtree-<up|down|both>-<dataName>.json` – `get_subtree` (alternatives on, no depth limit)
//! * `focus-<dataName>.json`        – `get_focus`
//! * `entity-<key with ':' → '__'>.json`        – `get_entity`, e.g. `entity-TITechTemplate__AdAstra.json`
//! * `search-<query, spaces → '_'>.json`         – `search` (limit 30), e.g. `search-ad_astra.json`
//! * `index.json` – lists every entity/subtree/search fixture written.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Serialize;
use ti_data::{EntityId, model};

use crate::commands::{
    self,
    tests::{backend, en},
};
use crate::dto::*;
use crate::refs;

#[test]
#[ignore = "writes static/mock fixtures; run explicitly"]
fn write_fixtures() {
    let Some(b) = backend() else { return };
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../static/mock");
    std::fs::create_dir_all(&out).unwrap();
    let written = std::cell::RefCell::new(BTreeSet::<String>::new());
    let write = |name: &str, value: &dyn erased::Json| {
        written.borrow_mut().insert(name.to_string());
        let path = out.join(name);
        std::fs::write(&path, value.to_json()).unwrap();
        eprintln!("{:>9} {}", std::fs::metadata(&path).unwrap().len(), name);
    };
    let scope = en();
    write("meta.json", &commands::get_meta(&b).unwrap());
    let all = TreeFilter { techs: true, projects: true, categories: vec![], properties: vec![] };
    write("tree.json", &commands::get_tree(&b, &scope, &all, FullLayout::Layered, true).unwrap());
    let techs = TreeFilter { projects: false, ..all.clone() };
    write("tree-techs.json", &commands::get_tree(&b, &scope, &techs, FullLayout::Layered, true).unwrap());
    write("tree-bands.json", &commands::get_tree(&b, &scope, &all, FullLayout::Bands, true).unwrap());
    write("properties.json", &commands::get_properties(&b, &scope, &all).unwrap());

    write("wiki-index.json", &commands::get_wiki_index(&b, &scope, Some(true)).unwrap());
    let mut wiki_keys = Vec::new();
    for template in [
        "TITechTemplate",
        "TIProjectTemplate",
        "TIEffectTemplate",
        "TIDriveTemplate",
        "TINationTemplate",
        "TIFactionTemplate",
        "TICodexEntryTemplate",
        "TIGlobalConfig",
    ] {
        let entries = commands::get_wiki_entries(&b, &scope, template, None).unwrap();
        wiki_keys.extend(entries.iter().take(3).map(|e| e.entity.key.clone()));
        write(&format!("wiki-entries-{template}.json"), &entries);
    }
    for template in ["TIProjectTemplate", "TIObjectiveTemplate"] {
        let entries = commands::get_wiki_entries(&b, &scope, template, Some(true)).unwrap();
        wiki_keys.extend(entries.iter().take(3).map(|e| e.entity.key.clone()));
        write(&format!("wiki-entries-spoiler-{template}.json"), &entries);
    }
    let mut index = Index::default();
    for (dir, direction, root) in [
        ("up", Direction::Up, "TIProjectTemplate:Project_ProtiumConverterTorch"),
        ("down", Direction::Down, "TITechTemplate:AdAstra"),
        ("both", Direction::Both, "TITechTemplate:OrbitalShipbuilding"),
    ] {
        let args = SubtreeArgs { root: root.into(), direction, alternatives: true, max_depth: None };
        let tree = commands::get_subtree(&b, &scope, &args, true).unwrap();
        let name = format!("subtree-{dir}-{}.json", root.split_once(':').unwrap().1);
        write(&name, &tree);
        index.subtrees.push(name);
    }
    for root in ["TITechTemplate:AdAstra", "TIProjectTemplate:Project_Pherocytes", "TITechTemplate:OrbitalShipbuilding"] {
        let name = format!("focus-{}.json", root.split_once(':').unwrap().1);
        write(&name, &commands::get_focus(&b, &scope, root).unwrap());
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
        first(model::PROJECT, &|id| !view.annotations(id).factions.is_empty() && !b.ds.entity(id).data_name.contains("Alien")),
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
    keys.extend(wiki_keys);
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
        write(&name, &commands::search(&b, &scope, q, 30, false, &[]).unwrap());
        index.searches.push(q.into());
    }
    write("index.json", &index);
    // Drop fixtures of earlier runs (renamed or no longer written) so the directory matches `index.json`.
    let written = written.into_inner();
    for entry in std::fs::read_dir(&out).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name.ends_with(".json") && !written.contains(&name) {
            std::fs::remove_file(&path).unwrap();
        }
    }
    let total: u64 = written.iter().map(|name| std::fs::metadata(out.join(name)).unwrap().len()).sum();
    eprintln!("total {total} bytes in {}", out.display());
    // Full wiki entry lists and bidirectional references bring this to about 6 MB.
    assert!(total < 10_000_000);
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
