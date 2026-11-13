//! Scope-specific gameplay spoilers. Story is always hidden; alien records propagate
//! through unlocks, effects, and ship hulls. Human research stays visible.
//! Classification uses the active record, never localized text or other scenarios.
use serde_json::Value;
use std::collections::HashSet;
use ti_data::{EntityId, View, model::Kind};

pub const STORY: &[&str] = &[
    "TIObjectiveTemplate",
    "TINarrativeEventTemplate",
    "TITimeEventTemplate",
    "TICodexEntryTemplate",
    "TIVictoryTemplate",
];

fn alien_field(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(key, value)| {
            (key == "alien" && value == true)
                || (key == "allowedIdeologies" && value.as_array().is_some_and(|a| a.iter().any(|v| v == "Alien")))
                || (value == "AlienCouncil" && (key.to_ascii_lowercase().contains("faction") || matches!(key.as_str(), "council" | "player")))
                || (key.to_ascii_lowercase().contains("faction") && value.as_array().is_some_and(|a| !a.is_empty() && a.iter().all(|v| v == "AlienCouncil")))
                || alien_field(value)
        }),
        Value::Array(values) => values.iter().any(alien_field),
        _ => false,
    }
}

pub fn classify(view: &View<'_>) -> HashSet<EntityId> {
    let ids: Vec<_> = view
        .dataset()
        .templates()
        .flat_map(|t| view.dataset().of_template(t).iter().copied())
        .filter(|&id| view.exists(id))
        .collect();
    let mut spoilers = HashSet::new();
    for &id in &ids {
        let e = view.dataset().entity(id);
        let Some(r) = view.record(id) else { continue };
        // Research eligibility follows the faction annotations, not its subject or prerequisites.
        if matches!(view.kind(id), Kind::Tech | Kind::Project) {
            let factions = view.annotations(id).factions;
            if !factions.is_empty() && factions.iter().all(|f| f == "AlienCouncil") {
                spoilers.insert(id);
            }
            continue;
        }
        // A human research unlock overrides alien names and fields on obtainable entities.
        if !STORY.contains(&e.template.as_ref())
            && view.unlocked_by(id).iter().any(|&research| {
                let factions = view.annotations(research).factions;
                factions.is_empty() || factions.iter().any(|f| f != "AlienCouncil")
            })
        {
            continue;
        }
        let alien_name = e.data_name.to_ascii_lowercase().contains("alien");
        if STORY.contains(&e.template.as_ref())
            || alien_name
            || e.data_name.as_ref() == "AlienCouncil"
            || alien_field(r)
            || (e.template.as_ref() == "TIFactionIdeologyTemplate" && (e.data_name.as_ref() == "alien" || r["ideology"] == "Alien"))
        {
            spoilers.insert(id);
        }
    }
    // A bounded monotone closure; synchronous passes make results independent of entity order.
    for _ in 0..=ids.len() {
        let mut added = Vec::new();
        for &id in &ids {
            if spoilers.contains(&id) {
                continue;
            }
            let all = |ids: Vec<EntityId>| !ids.is_empty() && ids.iter().all(|id| spoilers.contains(id));
            let inherited = match view.kind(id) {
                Kind::Effect => false,
                Kind::HabModule | Kind::ShipPart(_) | Kind::Org | Kind::Trait | Kind::Mission => all(view.unlocked_by(id)),
                _ => false,
            };
            let ship = matches!(view.dataset().entity(id).template.as_ref(), "TISpaceShipTemplate" | "TISpaceFleetTemplate")
                && view
                    .references_from(id)
                    .iter()
                    .any(|r| spoilers.contains(&r.to) && matches!(view.dataset().entity(r.to).template.as_ref(), "TIShipHullTemplate" | "TIFactionTemplate"));
            if inherited || ship {
                added.push(id);
            }
        }
        if added.is_empty() {
            break;
        }
        spoilers.extend(added);
    }
    // Shared effects must stay visible to every ordinary source. Resolve them after
    // research and unlocks so an initial grant cannot seed a false positive.
    let intrinsic_effects: HashSet<_> = spoilers.iter().copied().filter(|&id| matches!(view.kind(id), Kind::Effect)).collect();
    spoilers.retain(|&id| !matches!(view.kind(id), Kind::Effect));
    for _ in 0..=ids.len() {
        let added: Vec<_> = ids
            .iter()
            .copied()
            .filter(|&id| {
                if !matches!(view.kind(id), Kind::Effect) || spoilers.contains(&id) {
                    return false;
                }
                let refs = view.references_to(id);
                (intrinsic_effects.contains(&id) || !refs.is_empty()) && refs.iter().all(|r| spoilers.contains(&r.from))
            })
            .collect();
        if added.is_empty() {
            break;
        }
        spoilers.extend(added);
    }
    spoilers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        commands::{
            self,
            tests::{backend, en},
        },
        dto::ScopeArgs,
        scope,
    };

    #[test]
    fn faction_grants_require_exclusively_alien_factions() {
        assert!(alien_field(&serde_json::json!({"factions": ["AlienCouncil"]})));
        assert!(!alien_field(&serde_json::json!({"factions": ["AlienCouncil", "ResistanceCouncil"]})));
        assert!(!alien_field(&serde_json::json!({"factions": []})));
        assert!(alien_field(&serde_json::json!({"faction": "AlienCouncil"})));
    }

    /// Selecting a faction must not hide what another human faction can obtain.
    #[test]
    fn classification_ignores_selected_faction() {
        let Some(b) = backend() else { return };
        for faction in ["ResistCouncil", "SubmitCouncil"] {
            let args = ScopeArgs {
                scenario: Some("ModernScenario".into()),
                faction: Some(faction.into()),
                ..en()
            };
            let view = b.ds.view(scope::resolve(&b.ds, &args).unwrap());
            let spoilers = b.spoilers(&view).unwrap();
            for name in ["Effect_BlockAlienContact", "Effect_AlienRelationsEstablished", "Effect_AlienECM"] {
                if let Some(id) = b.ds.find("TIEffectTemplate", name).filter(|&id| view.exists(id)) {
                    assert!(!spoilers.contains(&id), "{name} hidden for {faction}");
                }
            }
        }
    }

    #[test]
    fn real_data_spoilers() {
        let Some(b) = backend() else { return };
        let args = en();
        let view = b.ds.view(scope::resolve(&b.ds, &args).unwrap());
        let spoilers = b.spoilers(&view).unwrap();
        assert!(std::sync::Arc::ptr_eq(&spoilers, &b.spoilers(&view).unwrap()));
        let index = commands::get_wiki_index(&b, &args, Some(true)).unwrap();
        for template in b.ds.templates() {
            let ids: Vec<_> = b.ds.of_template(template).iter().copied().filter(|&id| view.exists(id)).collect();
            let count = ids.iter().filter(|id| spoilers.contains(id)).count();
            eprintln!("{template}: {count}/{} spoilers", ids.len());
            assert_eq!(index.iter().filter(|s| s.template == template).map(|s| s.count).sum::<usize>(), ids.len());
            let normal = commands::all_wiki_entries(&b, &args, template, None).unwrap();
            assert!(normal.iter().all(|e| !e.entity.spoiler));
            assert_eq!(normal.len() + count, ids.len());
            if STORY.contains(&template) {
                assert_eq!(count, ids.len());
            }
        }
        for (template, name) in [
            ("TIDriveTemplate", "AlienFusionLanternx4"),
            ("TIFactionTemplate", "AlienCouncil"),
            ("TIProjectTemplate", "Project_AlienMasterProject"),
            ("TIProjectTemplate", "Project_AlienAdvancedMasterProject"),
        ] {
            let id = view.find(template, name).unwrap();
            assert!(spoilers.contains(&id), "{template}:{name}");
        }
        let xenology = view
            .entities("TIProjectTemplate")
            .find(|&id| view.record(id).unwrap()["techCategory"] == "Xenology" && view.annotations(id).factions.is_empty())
            .unwrap();
        assert!(!spoilers.contains(&xenology));
        assert!(!spoilers.contains(&view.find("TITechTemplate", "AdAstra").unwrap()));
        for name in [
            "Project_Exotics",
            "Project_ProtiumConverterTorch",
            "Project_FusionTokamakV",
            "Project_ResistanceTalentDevelopment",
            "Project_GoldRush",
            "Project_OperationalSecurity",
            "Project_InvasiveSpeciesContainment",
            "Project_UrbanArmoring",
            "Project_CounteralienOperationsTeams",
        ] {
            assert!(!spoilers.contains(&view.find("TIProjectTemplate", name).unwrap()), "{name}");
        }
        for template in ["TITechTemplate", "TIProjectTemplate"] {
            for id in view.entities(template) {
                let factions = view.annotations(id).factions;
                let alien_only = !factions.is_empty() && factions.iter().all(|f| f == "AlienCouncil");
                assert_eq!(spoilers.contains(&id), alien_only, "{}", b.ds.entity(id).data_name);
                if spoilers.contains(&id) {
                    eprintln!("Spoiler research: {}", b.ds.entity(id).data_name);
                }
                if view.record(id).unwrap()["endGameTech"] == true {
                    assert!(!spoilers.contains(&id));
                }
            }
        }
        for name in ["Effect_ExploreMars", "Effect_CanFoundTier1Station", "Effect_IncreaseCouncilSize"] {
            assert!(!spoilers.contains(&view.find("TIEffectTemplate", name).unwrap()), "{name}");
        }
        for template in b.ds.templates() {
            for id in view.entities(template) {
                if spoilers.contains(&id)
                    || !matches!(
                        view.kind(id),
                        Kind::Tech | Kind::Project | Kind::HabModule | Kind::ShipPart(_) | Kind::Org | Kind::Trait
                    )
                {
                    continue;
                }
                for effect in view.references_from(id).iter().map(|r| r.to).chain(view.effects(id)) {
                    if matches!(view.kind(effect), Kind::Effect) {
                        assert!(
                            !spoilers.contains(&effect),
                            "{} references {}",
                            b.ds.entity(id).data_name,
                            b.ds.entity(effect).data_name
                        );
                    }
                }
            }
        }
        for template in b.ds.templates() {
            for id in view.entities(template) {
                if !STORY.contains(&template) && view.unlocked_by(id).iter().any(|research| !spoilers.contains(research)) {
                    assert!(!spoilers.contains(&id), "Human research unlock: {}", b.ds.entity(id).data_name);
                }
            }
        }
        let mars = view.find("TIEffectTemplate", "Effect_ExploreMars").unwrap();
        let mars_research: Vec<_> = view
            .references_to(mars)
            .iter()
            .map(|r| r.from)
            .filter(|&id| matches!(view.kind(id), Kind::Tech | Kind::Project))
            .collect();
        assert!(!mars_research.is_empty());
        for id in mars_research {
            assert!(!spoilers.contains(&id), "{}", b.ds.entity(id).data_name);
        }
        assert!(view.entities("TINationTemplate").any(|id| !spoilers.contains(&id)));
        let human_drive = view
            .entities("TIDriveTemplate")
            .find(|&id| b.ds.entity(id).data_name.as_ref() == "ApexSolidRocketx1")
            .unwrap();
        assert!(!spoilers.contains(&human_drive));
    }
}
