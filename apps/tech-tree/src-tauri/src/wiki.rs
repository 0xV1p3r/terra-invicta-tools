//! Wiki section labels and ordering. Unknown templates remain visible in Other.

/// Ordered groups and their hand-written template labels.
const GROUPS: &[(&str, &[(&str, &str)])] = &[
    (
        "Research",
        &[
            ("TITechTemplate", "Technologies"),
            ("TIProjectTemplate", "Projects"),
            ("TIEffectTemplate", "Effects"),
        ],
    ),
    (
        "Space",
        &[
            ("TIHabModuleTemplate", "Hab Modules"),
            ("TIHabSchematicTemplate", "Hab Schematics"),
            ("TIHabTemplate", "Habs"),
            ("TIHabSiteTemplate", "Hab Sites"),
            ("TISpaceBodyTemplate", "Space Bodies"),
            ("TIOrbitTemplate", "Orbits"),
            ("TIMiningProfileTemplate", "Mining Profiles"),
            ("TINavigableTemplate", "Navigables"),
        ],
    ),
    (
        "Ships & Combat",
        &[
            ("TIShipHullTemplate", "Hulls"),
            ("TIDriveTemplate", "Drives"),
            ("TIPowerPlantTemplate", "Power Plants"),
            ("TIRadiatorTemplate", "Radiators"),
            ("TIHeatSinkTemplate", "Heat Sinks"),
            ("TIBatteryTemplate", "Batteries"),
            ("TIShipArmorTemplate", "Armor"),
            ("TIUtilityModuleTemplate", "Utility Modules"),
            ("TILaserWeaponTemplate", "Laser Weapons"),
            ("TIMagneticGunTemplate", "Magnetic Guns"),
            ("TIGunTemplate", "Guns"),
            ("TIMissileTemplate", "Missiles"),
            ("TIParticleWeaponTemplate", "Particle Weapons"),
            ("TIPlasmaWeaponTemplate", "Plasma Weapons"),
            ("TISpaceShipTemplate", "Ship Designs"),
            ("TISpaceFleetTemplate", "Fleets"),
            ("TIFormationTemplate", "Formations"),
        ],
    ),
    (
        "Earth",
        &[
            ("TINationTemplate", "Nations"),
            ("TIRegionTemplate", "Regions"),
            ("TIBilateralTemplate", "Bilateral Relations"),
            ("TIArmyTemplate", "Armies"),
        ],
    ),
    (
        "Factions & People",
        &[
            ("TIFactionTemplate", "Factions"),
            ("TIFactionIdeologyTemplate", "Faction Ideologies"),
            ("TIPlayerTemplate", "Players"),
            ("TICouncilorTemplate", "Councilors"),
            ("TICouncilorTypeTemplate", "Councilor Types"),
            ("TITraitTemplate", "Traits"),
            ("TIOrgTemplate", "Orgs"),
            ("TIOfficerTemplate", "Officers"),
            ("TIMissionTemplate", "Missions"),
        ],
    ),
    (
        "Spoilers",
        &[
            ("TIObjectiveTemplate", "Objectives"),
            ("TINarrativeEventTemplate", "Narrative Events"),
            ("TITimeEventTemplate", "Time Events"),
            ("TICodexEntryTemplate", "Codex"),
            ("TIVictoryTemplate", "Victory Conditions"),
        ],
    ),
    ("Setup", &[("TIStartTimeTemplate", "Scenarios")]),
    (
        "Technical",
        &[
            ("TIGlobalConfig", "Global Config"),
            ("TIMetaTemplate", "Meta"),
            ("TILocalizationTemplate", "Localization"),
            ("TI2DCinematicTemplate", "Cinematics"),
            ("TICouncilorAppearanceTemplate", "Councilor Appearances"),
            ("TICouncilorVoiceTemplate", "Councilor Voices"),
            ("TIOrgIconTemplate", "Org Icons"),
            ("TIMapGroupVisualizerTemplate", "Map Visualizers"),
            ("TINotificationTemplate", "Notifications"),
            ("TIPriorityPresetTemplate", "Priority Presets"),
            ("TIMapRegionTemplate", "Map Regions"),
            ("TISpaceCombatTemplate", "Space Combat"),
        ],
    ),
];

/// Group position, group name, and display label for any dataset template.
pub fn section(template: &str) -> (usize, &'static str, String) {
    for (order, (group, templates)) in GROUPS.iter().enumerate() {
        if let Some((_, label)) = templates.iter().find(|(t, _)| *t == template) {
            let order = match *group {
                "Setup" => 5,
                "Spoilers" => 6,
                _ => order,
            };
            return (order, group, (*label).into());
        }
    }
    (GROUPS.len(), "Other", derived_label(template))
}

/// Strip the template wrapper, split CamelCase words, and append a plural s.
fn derived_label(template: &str) -> String {
    let bare = template.strip_prefix("TI").unwrap_or(template);
    let bare = bare.strip_suffix("Template").unwrap_or(bare);
    let chars: Vec<char> = bare.chars().collect();
    let mut label = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_uppercase() && (chars[i - 1].is_lowercase() || chars.get(i + 1).is_some_and(|c| c.is_lowercase())) {
            label.push(' ');
        }
        label.push(c);
    }
    label.push('s');
    label
}

/// Cheap row context; relationship lookups use dataset indexes or the view's ownership cache.
pub fn subtitle(view: &ti_data::View<'_>, texts: &ti_data::Texts, id: ti_data::EntityId) -> Option<String> {
    use serde_json::Value;
    let r = view.record(id)?;
    let template = &*view.dataset().entity(id).template;
    let text = |key| r.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(ti_data::text::humanize);
    let related = |template, key| view.find(template, r.get(key)?.as_str()?).map(|id| view.name(texts, id));
    let tier = || r.get("tier").or_else(|| r.get("consTier")).and_then(Value::as_u64).map(|t| format!("Tier {t}"));
    let join = |a: Option<String>, b: Option<String>| {
        let parts: Vec<_> = a.into_iter().chain(b).collect();
        (!parts.is_empty()).then(|| parts.join(" · "))
    };
    let context = match template {
        "TIEffectTemplate" => None,
        "TIRegionTemplate" => view.region_nation(id).map(|id| view.name(texts, id)),
        // Capital: the region of the nation's initial-owner capital claim (indexed incoming references).
        "TINationTemplate" => view
            .references_to(id)
            .into_iter()
            .filter(|r| &*r.path == "nation1" && &*view.dataset().entity(r.from).template == "TIBilateralTemplate")
            .filter_map(|r| view.record(r.from))
            .find(|claim| claim.get("initialOwner").and_then(Value::as_bool) == Some(true) && claim.get("capitalClaim").and_then(Value::as_bool) == Some(true))
            .and_then(|claim| view.find("TIRegionTemplate", claim.get("region1")?.as_str()?))
            .and_then(|region| view.record(region)?.get("primaryCity")?.as_str().map(|c| format!("Capital: {c}"))),
        "TISpaceBodyTemplate" => join(
            text("objectType"),
            related("TISpaceBodyTemplate", "orbits")
                .or_else(|| related("TISpaceBodyTemplate", "parentBodyName"))
                .or_else(|| related("TISpaceBodyTemplate", "barycenterName")),
        ),
        "TIOrbitTemplate" => related("TISpaceBodyTemplate", "barycenterName"),
        "TIHabSiteTemplate" => related("TISpaceBodyTemplate", "parentBodyName"),
        "TIBilateralTemplate" => text("relationType"),
        "TIArmyTemplate" => join(text("deploymentType"), related("TIRegionTemplate", "homeRegionStr")),
        "TIHabTemplate" | "TIHabModuleTemplate" => join(text("habType"), tier()),
        "TIOrgTemplate" => join(text("orgType"), tier()),
        "TITraitTemplate" => text("traitClass").or_else(|| text("specialTraitRule")),
        "TICouncilorTypeTemplate" => r
            .get("keyStat")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" · ")),
        "TIObjectiveTemplate" => text("objectiveType"),
        "TICodexEntryTemplate" => Some(
            if r.get("mainTopic").and_then(Value::as_bool) == Some(true) {
                "Main topic"
            } else {
                "Subtopic"
            }
            .into(),
        ),
        "TICouncilorAppearanceTemplate" => text("string"),
        "TIMissionTemplate" => r
            .pointer("/resolutionMethod/attackingModifiers")
            .and_then(Value::as_array)
            .and_then(|modifiers| modifiers.iter().find_map(|m| m.get("attackerAttribute").and_then(Value::as_str)))
            .map(ti_data::text::humanize),
        _ if matches!(view.kind(id), ti_data::model::Kind::ShipPart(_)) || template == "TIShipHullTemplate" => join(
            text("partClass").or_else(|| text("driveClassification").or_else(|| text("powerPlantClass"))),
            tier(),
        ),
        _ => None,
    };
    let ds = view.dataset();
    let presence = ds.presence(id);
    if view.scenario().is_none() && presence.len() < ds.scenarios().len() && ds.scenarios().iter().any(|s| s.instantiated.contains_key(template)) {
        let overview = ds.overview();
        let labels = presence
            .iter()
            .map(|idx| {
                let s = ds.scenario(idx);
                let label = overview.name(texts, s.meta);
                if label == s.data_name && !s.friendly_name.is_empty() {
                    s.friendly_name.clone()
                } else {
                    label
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        join(Some(labels), context)
    } else {
        context
    }
    .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    #[test]
    fn fallback_label() {
        assert_eq!(super::section("TIFooBarTemplate"), (8, "Other", "Foo Bars".into()));
    }
}
