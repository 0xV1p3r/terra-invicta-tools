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
        let Some(mil) = e.variants.iter().position(|v| tags(&v.record) == ["Millennium"]) else { continue };
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
            let Some(raw) = view.text_raw(&en, id, field::SUMMARY) else { continue };
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
