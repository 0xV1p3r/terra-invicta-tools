//! Integration tests against the extracted build in `<workspace>/data`.
//! Skipped (pass trivially) when no extracted data is present.

use std::path::PathBuf;
use std::sync::OnceLock;

use serde_json::Value;
use ti_data::graph::Traversal;
use ti_data::text::field;
use ti_data::view::UnlockKind;
use ti_data::{DataRoot, Dataset, Scope, SearchIndex, Texts, model};

fn dataset() -> Option<&'static Dataset> {
    static DS: OnceLock<Option<Dataset>> = OnceLock::new();
    DS.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let root = DataRoot::open(root).ok()?;
        Some(root.latest().expect("latest build loads"))
    })
    .as_ref()
}

macro_rules! data_or_skip {
    () => {
        match dataset() {
            Some(ds) => ds,
            None => {
                eprintln!("no extracted data, skipping");
                return;
            }
        }
    };
}

fn tags(v: &Value) -> Vec<&str> {
    v.get("scenarioTags").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect()
}

#[test]
fn scenarios_come_from_meta_templates() {
    let ds = data_or_skip!();
    let names: Vec<&str> = ds.scenarios().iter().map(|s| s.data_name.as_str()).collect();
    for expected in ["ModernScenario", "2026Scenario", "2070Scenario", "2003Scenario", "BrokenEarthScenario"] {
        assert!(names.contains(&expected), "missing {expected} in {names:?}");
    }
    assert_eq!(ds.default_scenario().unwrap().data_name, "ModernScenario");
    let s2003 = ds.scenario_by_name("2003Scenario").unwrap();
    assert!(s2003.tags.iter().any(|t| t == "Millennium"));
    assert_eq!(s2003.localization_postfix.as_deref(), Some(".2003"));
    assert!(s2003.start_time.is_some());
    assert!(s2003.is_dlc());
    // every listed scenario has a start time
    for s in ds.scenarios().iter().filter(|s| s.listed) {
        assert!(s.start_time.is_some(), "{} has no start time", s.data_name);
    }
}

#[test]
fn not_post_apoc_records_are_absent_in_broken_earth() {
    let ds = data_or_skip!();
    let be = ds.scenario_by_name("BrokenEarthScenario").unwrap().idx;
    let modern = ds.scenario_by_name("ModernScenario").unwrap().idx;
    let mut checked = 0;
    for &id in ds.of_template(model::PROJECT) {
        let e = ds.entity(id);
        if e.variants.len() == 1 && tags(&e.variants[0].record) == ["NotPostApoc"] && !e.variants[0].disabled {
            assert!(ds.selected_variant(be, id).is_none(), "{} present in BE", e.data_name);
            assert!(ds.selected_variant(modern, id).is_some(), "{} missing in modern", e.data_name);
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn tagged_dlc_variants_win_in_their_scenario() {
    let ds = data_or_skip!();
    let s2003 = ds.scenario_by_name("2003Scenario").unwrap().idx;
    let modern = ds.scenario_by_name("ModernScenario").unwrap().idx;
    let mut checked = 0;
    for &id in ds.of_template(model::TECH) {
        let e = ds.entity(id);
        let Some(mil) = e.variants.iter().position(|v| tags(&v.record) == ["Millennium"]) else {
            continue;
        };
        if !e.variants.iter().any(|v| tags(&v.record).is_empty()) {
            continue;
        }
        let in_2003 = ds.selected_variant(s2003, id).unwrap();
        assert!(std::ptr::eq(in_2003, &e.variants[mil]), "{}: 2003 should use the Millennium record", e.data_name);
        let in_modern = ds.selected_variant(modern, id).unwrap();
        assert!(tags(&in_modern.record).is_empty(), "{}: modern should use the untagged record", e.data_name);
        assert!(ds.varies_by_scenario(id));
        checked += 1;
    }
    assert!(checked > 10, "only {checked} overridden techs");
}

#[test]
fn research_graph_is_sound_in_every_scenario() {
    let ds = data_or_skip!();
    for s in ds.scenarios() {
        let view = ds.view(Scope::scenario(s.idx));
        let g = view.research_graph();
        assert!(g.nodes().len() > 500, "{}: {} nodes", s.data_name, g.nodes().len());
        assert!(g.cyclic_nodes().is_empty(), "{}: cycles {:?}", s.data_name, g.cyclic_nodes());
        // techs never depend on projects
        for &n in g.nodes() {
            if &*ds.entity(n).template == model::TECH {
                for grp in g.requirements(n) {
                    for o in &grp.options {
                        assert_eq!(&*ds.entity(*o).template, model::TECH);
                    }
                }
            }
        }
    }
}

#[test]
fn alt_prereqs_form_or_groups() {
    let ds = data_or_skip!();
    let view = ds.overview();
    let g = view.research_graph();
    let with_alt = g.nodes().iter().copied().filter(|n| g.requirements(*n).iter().any(|grp| grp.options.len() == 2)).count();
    assert!(with_alt > 50, "only {with_alt} nodes with alternatives");
    assert!(g.requirements(view.find(model::PROJECT, "Project_Pherocytes").unwrap())[0].options.len() == 2);
}

#[test]
fn faction_filter_hides_foreign_projects() {
    let ds = data_or_skip!();
    let resist = ds.find(model::FACTION, "ResistCouncil").unwrap();
    let all = ds.overview();
    let view = ds.view(Scope::OVERVIEW.with_faction(resist));
    let locked: Vec<_> = all
        .entities(model::PROJECT)
        .filter(|id| {
            let p = all.project(*id).unwrap();
            !p.faction_prereq.is_empty() && !p.faction_prereq.iter().any(|f| f == "ResistCouncil")
        })
        .collect();
    assert!(!locked.is_empty());
    for id in locked {
        assert!(!view.contains(id));
        assert!(!view.research_graph().contains(id));
    }
}

#[test]
fn unlocks_and_subtrees() {
    let ds = data_or_skip!();
    let view = ds.overview();
    // every drive's required project lists it as an unlock
    let drive = view.entities("TIDriveTemplate").next().unwrap();
    let project = view.unlocked_by(drive)[0];
    assert!(view.unlocks(project).iter().any(|u| u.entity == drive && matches!(u.kind, UnlockKind::ShipPart(_))));

    let g = view.research_graph();
    let project = g.nodes().iter().copied().find(|n| !g.requirements(*n).is_empty()).unwrap();
    let up = g.ancestors(project, Traversal::default());
    assert!(up.nodes.contains(&project) && up.nodes.len() > 1);
    for e in &up.edges {
        assert!(up.nodes.contains(&e.from) && up.nodes.contains(&e.to));
    }
    let root = g.roots().next().unwrap();
    assert!(!g.descendants(root, Traversal::default()).nodes.is_empty());
    assert!(g.is_obtainable(project));
}

#[test]
fn texts_with_fallback_and_scenario_postfix() {
    let ds = data_or_skip!();
    let en = Texts::load(ds, "en").unwrap();
    let view = ds.overview();
    let ad_astra = view.find(model::TECH, "AdAstra").unwrap();
    assert_eq!(view.name(&en, ad_astra), "Ad Astra");
    assert!(view.text(&en, ad_astra, field::SUMMARY).is_some());
    let quote = view.text(&en, ad_astra, field::QUOTE).unwrap();
    assert!(!quote.contains("Leader}"), "leader token not replaced: {quote}");

    // every language loads, and names fall back to English instead of failing
    for lang in ds.languages() {
        let t = Texts::load(ds, lang).unwrap();
        assert!(!view.name(&t, ad_astra).is_empty());
    }
    // no raw <skip/> survives extraction
    assert!(en.primary.entries().all(|(_, v)| !v.contains("<skip/>")));
}

#[test]
fn search_finds_names_ids_and_text() {
    let ds = data_or_skip!();
    let en = Texts::load(ds, "en").unwrap();
    let view = ds.overview();
    let index = SearchIndex::build(ds, &en);
    let all = |id| view.contains(id);
    let ad_astra = view.find(model::TECH, "AdAstra").unwrap();
    assert_eq!(index.search("ad astra", 5, &all)[0].id, ad_astra);
    assert_eq!(index.search("AdAstra", 5, &all)[0].id, ad_astra);
    let lasers = index.search("laser", 50, &|id| all(id) && &*ds.entity(id).template == "TILaserWeaponTemplate");
    assert!(lasers.len() > 10);
    assert!(index.search("zzzxxyq", 5, &all).is_empty());
}

#[test]
fn icons_resolve_to_files() {
    let ds = data_or_skip!();
    if !ds.manifest().has_assets {
        return;
    }
    let view = ds.overview();
    let mut found = 0;
    for id in view.entities("TIDriveTemplate").chain(view.entities(model::TECH)) {
        if let Some(asset) = view.icon_resource(id).and_then(|r| ds.asset(&r)) {
            assert!(ds.asset_path(&asset.file).unwrap().is_file());
            found += 1;
        }
    }
    assert!(found > 100);
}

#[test]
fn effect_descriptions_render() {
    let ds = data_or_skip!();
    let en = Texts::load(ds, "en").unwrap();
    let view = ds.overview();
    let ctx = ti_data::EffectContext::default();
    let mut rendered = 0;
    for id in view.entities(model::EFFECT) {
        if let Some(t) = view.effect_description(&en, id, &ctx) {
            assert!(!t.contains("{0}"), "unformatted: {t}");
            rendered += 1;
        }
    }
    assert!(rendered > 500, "only {rendered} effects rendered");
    let ad_astra = view.find(model::TECH, "Accelerando").unwrap();
    let e = view.effects(ad_astra)[0];
    assert!(view.effect_description(&en, e, &ctx).unwrap().contains('%'));
    let segs = view.effect_segments(&en, e, &ctx).unwrap();
    assert!(segs.iter().any(|s| matches!(s, ti_data::EffectSegment::Value(v) if v.ends_with('%'))), "{segs:?}");
    let mining = view.find(model::EFFECT, "Effect_MiningMetalsBonus").unwrap();
    let segs = view.effect_segments(&en, mining, &ctx).unwrap();
    assert!(segs.contains(&ti_data::EffectSegment::Sprite("metal".into())), "{segs:?}");
}

#[test]
fn aliased_nation_and_region_names() {
    let ds = data_or_skip!();
    let en = Texts::load(ds, "en").unwrap();
    let s2003 = ds.scenario_by_name("2003Scenario").unwrap().idx;
    let view = ds.view(Scope::scenario(s2003));
    let rus = view.find("TINationTemplate", "2003_RUS").unwrap();
    assert_eq!(view.name(&en, rus), "Russia");
    // no nation/region of any scenario shows its raw key name
    for s in ds.scenarios() {
        let view = ds.view(Scope::scenario(s.idx));
        for template in ["TINationTemplate", "TIRegionTemplate"] {
            for id in view.entities(template) {
                let e = ds.entity(id);
                let name = view.name(&en, id);
                assert!(!name.starts_with("2003_"), "{}: {} -> {name}", s.data_name, e.data_name);
            }
        }
    }
}

#[test]
fn module_sentinel_summaries_are_replaced() {
    let ds = data_or_skip!();
    let en = Texts::load(ds, "en").unwrap();
    for s in ds.scenarios() {
        let view = ds.view(Scope::scenario(s.idx));
        let mut replaced = 0;
        for id in view.entities(model::PROJECT) {
            let Some(raw) = view.text_raw(&en, id, field::SUMMARY) else {
                continue;
            };
            if raw != "<shipmodule>" && raw != "<habmodule>" {
                continue;
            }
            let (source, _) = view.text_source(&en, id, field::SUMMARY);
            if source != id {
                replaced += 1;
                let text = view.text(&en, id, field::SUMMARY).unwrap();
                assert!(!text.trim().is_empty(), "{}: empty summary for {}", s.data_name, ds.entity(id).data_name);
            }
        }
        assert!(replaced > 150, "{}: only {replaced} sentinel summaries replaced", s.data_name);
    }
}

#[test]
fn edge_provenance_is_consistent() {
    let ds = data_or_skip!();
    let be = ds.scenario_by_name("BrokenEarthScenario").unwrap().idx;
    let exploit = ds.find(model::FACTION, "ExploitCouncil").unwrap();
    for scope in [Scope::OVERVIEW, Scope::scenario(be).with_faction(exploit)] {
        let view = ds.view(scope);
        let g = view.research_graph();
        let full = g.full();
        for e in &full.edges {
            let d = g.dependents(e.from).iter().find(|d| d.node == e.to).expect("edge has a dependent");
            assert_eq!(d.alternative, e.alternative, "{} -> {}", ds.entity(e.from).data_name, ds.entity(e.to).data_name);
        }
    }
    let view = ds.view(Scope::scenario(be).with_faction(exploit));
    let g = view.research_graph();
    let neural = view.find(model::PROJECT, "Project_NeuralEngineering").unwrap();
    let anc = g.ancestors(neural, Traversal { alternatives: true, max_depth: Some(1) });
    let hidden: Vec<_> = g.requirements(neural).iter().flat_map(|grp| grp.hidden.iter()).collect();
    assert!(!hidden.is_empty());
    assert!(anc.edges.iter().any(|e| e.alternative));
    assert!(g.is_obtainable(neural));
}

#[test]
fn references_have_no_duplicates() {
    let ds = data_or_skip!();
    let view = ds.overview();
    for e in ds.entities() {
        let refs = view.references_to(e.id);
        let mut seen = std::collections::HashSet::new();
        for r in refs {
            assert!(seen.insert((r.from, r.path.clone())), "duplicate reference to {}", e.data_name);
        }
    }
}

#[test]
fn wiki_names_and_coverage() {
    let ds = data_or_skip!();
    let texts = Texts::load(ds, "en").unwrap();
    let view = ds.overview();
    let welcome = ds.find("TICodexEntryTemplate", "codex_welcome").unwrap();
    assert_eq!(view.name(&texts, welcome), "Introduction");
    assert!(!view.text(&texts, welcome, field::DESCRIPTION).unwrap().trim().is_empty());
    let objective = ds.find("TIObjectiveTemplate", "BS_ResearchTheirSignatures_Submit").unwrap();
    assert_eq!(
        view.name(&texts, objective),
        ti_data::text::to_plain(texts.get("TIObjectiveTemplate.displayName.ResearchTheirSignatures.SubmitCouncil").unwrap())
    );
    let bilateral = ds.of_template("TIBilateralTemplate")[0];
    let record = view.record(bilateral).unwrap();
    for field in ["nation1", "nation2"] {
        let nation = ds.find("TINationTemplate", record[field].as_str().unwrap()).unwrap();
        assert!(view.name(&texts, bilateral).contains(&view.name(&texts, nation)));
    }
    for template in [
        "TICodexEntryTemplate",
        "TIObjectiveTemplate",
        "TIBilateralTemplate",
        "TIMapRegionTemplate",
        "TIStartTimeTemplate",
        "TIPlayerTemplate",
        "TIFactionIdeologyTemplate",
        "TICouncilorTemplate",
        "TIVictoryTemplate",
        "TIHabSchematicTemplate",
    ] {
        let ids = ds.of_template(template);
        let named = ids.iter().filter(|&&id| view.name(&texts, id) != *ds.entity(id).data_name).count();
        let localized = ids.iter().filter(|&&id| view.text_raw(&texts, id, field::DISPLAY_NAME).is_some()).count();
        println!("{template}: {named}/{} labels, {localized}/{} localized", ids.len(), ids.len());
        if ["TICodexEntryTemplate", "TIObjectiveTemplate"].contains(&template) {
            assert!(localized * 100 >= ids.len() * 95);
        }
    }
    let index = SearchIndex::build(ds, &texts);
    assert!(index.search("Introduction", 20, &|_| true).iter().any(|h| h.id == welcome));
}

#[test]
fn wiki_icon_coverage() {
    let ds = data_or_skip!();
    let view = ds.overview();
    println!("Template | Before | After | Total");
    for template in ds.templates() {
        let ids: Vec<_> = view.entities(template).collect();
        let before = ids
            .iter()
            .filter(|&&id| {
                let r = view.record(id).unwrap();
                let own = ["iconResource", "stationIconResource", "baseIconResource", "missionIconImagePath", "flagResource", "councilIcon64"]
                    .iter()
                    .find_map(|key| r.get(*key).and_then(Value::as_str).filter(|s| !s.is_empty()));
                let category = || r.get("techCategory").and_then(Value::as_str).and_then(model::TechCategory::parse).unwrap_or_default().icon_resource();
                own.or_else(|| view.kind(id).is_research().then(category).flatten()).is_some_and(|r| ds.asset(r).is_some())
            })
            .count();
        let after = ids.iter().filter(|&&id| view.icon_resource(id).is_some()).count();
        println!("{template} | {before} | {after} | {}", ids.len());
        for id in ids {
            if let Some(resource) = view.icon_resource(id) {
                assert!(ds.asset(&resource).is_some(), "{template}: {resource}");
            }
            if let Some(resource) = view.image_resource(id) {
                assert!(ds.asset(&resource).is_some(), "{template}: {resource}");
            }
        }
    }
    for (template, name) in
        [("TISpaceBodyTemplate", "Sol"), ("TIOfficerTemplate", "Officer_Admiral"), ("TIRegionTemplate", "Afghanistan"), ("TIShipHullTemplate", "Gunship")]
    {
        let id = view.find(template, name).unwrap();
        assert!(view.icon_resource(id).is_some(), "{template}/{name}");
    }
}

#[test]
fn scenario_meta_membership() {
    let ds = data_or_skip!();
    let texts = Texts::load(ds, "en").unwrap();
    let forced: std::collections::HashSet<_> = ds
        .of_template("TIMetaTemplate")
        .iter()
        .filter_map(|&id| ds.overview().record(id))
        .flat_map(|r| r["templateNames"].as_array().into_iter().flatten().filter_map(Value::as_str))
        .collect();
    for scenario in ds.scenarios() {
        let view = ds.view(Scope { scenario: Some(scenario.idx), ..Scope::default() });
        for template in [
            "TINationTemplate",
            "TIRegionTemplate",
            "TIArmyTemplate",
            "TIOrgTemplate",
            "TICouncilorTemplate",
            "TISpaceFleetTemplate",
            "TIHabTemplate",
            "TITimeEventTemplate",
        ] {
            let tags = scenario.tags.iter().map(|t| std::sync::Arc::from(t.as_str())).collect::<Vec<_>>();
            let before = ds
                .of_template(template)
                .iter()
                .filter(|&&id| ti_data::scenario::resolve(&ds.entity(id).variants, &tags, forced.contains(&*ds.entity(id).data_name)).is_some())
                .count();
            eprintln!("{} {template}: {before} -> {}", scenario.data_name, view.entities(template).count());
        }
    }
    // Overview must retain exactly the entity set from plain scenarioTags resolution, minus
    // start-date copies that no scenario lists (e.g. `2026_ALN`, never instantiated).
    let prefixes: Vec<String> = ds
        .scenarios()
        .iter()
        .filter_map(|s| ds.overview().record(s.meta)?.get("scenarioPrefix")?.as_str().map(str::to_string))
        .filter(|p| !p.is_empty())
        .collect();
    let listed: std::collections::HashSet<(&str, &str)> =
        ds.scenarios().iter().flat_map(|s| s.instantiated.iter().flat_map(|(t, names)| names.iter().map(move |n| (t.as_str(), n.as_str())))).collect();
    for entity in ds.entities() {
        let unused_copy = matches!(&*entity.template, "TINationTemplate" | "TIRegionTemplate" | "TIArmyTemplate" | "TIStartTimeTemplate")
            && !listed.contains(&(&*entity.template, &*entity.data_name))
            && prefixes.iter().any(|p| entity.data_name.starts_with(p.as_str()));
        let expected = !unused_copy && ds.scenarios().iter().any(|scenario| {
            let tags = scenario.tags.iter().map(|t| std::sync::Arc::from(t.as_str())).collect::<Vec<_>>();
            ti_data::scenario::resolve(&entity.variants, &tags, forced.contains(&*entity.data_name)).is_some()
        });
        assert_eq!(ds.overview().find(&entity.template, &entity.data_name).is_some(), expected, "{}:{}", entity.template, entity.data_name);
    }
    for scenario in ds.scenarios() {
        let view = ds.view(Scope::scenario(scenario.idx));
        for (kind, name) in
            [("TICouncilorTemplate", "randomizedCouncilor1"), ("TISpaceFleetTemplate", "GenericFleetTemplate"), ("TITimeEventTemplate", "GenericTimeEvent")]
        {
            // GenericFleetTemplate is synthesized by CreateAtRunTime when absent
            // from the shipped data. Do not invent a dataset record for it.
            if ds.find(kind, name).is_some() {
                assert!(view.find(kind, name).is_some(), "{}: {name}", scenario.data_name);
            } else {
                assert_eq!(name, "GenericFleetTemplate");
            }
        }
    }
    let modern = ds.view(Scope::scenario(ds.scenario_by_name("ModernScenario").unwrap().idx));
    assert!(modern.find("TIOrgTemplate", "CooperateSpecial").is_some());
    let arrival = modern.find("TITechTemplate", "ArrivalSecurity").unwrap();
    let random_security = modern.find("TIOrgTemplate", "RandomSecurity12").unwrap();
    assert!(modern.unlocks(arrival).iter().any(|u| u.entity == random_security));
    for (scenario, included, excluded) in [("ModernScenario", "USAArmy4", "2026_USAArmy4"), ("2026Scenario", "2026_USAArmy4", "USAArmy4")] {
        let view = ds.view(Scope { scenario: Some(ds.scenario_by_name(scenario).unwrap().idx), ..Scope::default() });
        assert!(view.find("TIArmyTemplate", included).is_some());
        assert!(view.find("TIArmyTemplate", excluded).is_none());
        assert!(view.find("TIArmyTemplate", "2070_USAArmy2").is_none());
        let nations: Vec<String> = view.entities("TINationTemplate").map(|id| view.name(&texts, id)).collect();
        let unique: std::collections::HashSet<&String> = nations.iter().collect();
        assert_eq!(unique.len(), nations.len(), "duplicate nation names in {scenario}");
        assert_eq!(view.entities("TIRegionTemplate").count(), 363);
        let mut names = std::collections::HashSet::new();
        for id in view.entities("TIArmyTemplate") {
            let r = view.record(id).unwrap();
            let nation = view.find("TIRegionTemplate", r["homeRegionStr"].as_str().unwrap()).and_then(|r| view.region_nation(r));
            assert!(names.insert((nation, view.name(&texts, id))), "duplicate army {}", view.name(&texts, id));
        }
    }
}
