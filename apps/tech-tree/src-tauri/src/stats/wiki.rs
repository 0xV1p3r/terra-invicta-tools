//! Wiki presentation for world, people, scenario and configuration templates.
//! Traverse only this record and its indexed references; never scan a template.
use super::*;

pub(super) fn stats(view: &View<'_>, texts: &Texts, id: EntityId, spoilers: &HashSet<EntityId>) -> Option<EntityStats> {
    let record = view.record(id)?.as_object()?;
    let template = view.dataset().entity(id).template.as_ref();
    let mut b = Builder { view, texts, spoilers, stats: EntityStats { tags: vec![], groups: vec![], notes: vec![], links: vec![] } };
    let mut linked = HashSet::new();
    for reference in view.references_from(id) {
        let path = reference.path.as_ref();
        linked.insert(path.to_string());
        let label = path_label(path);
        let entity = entity_ref(view, texts, reference.to, spoilers);
        if let Some(group) = b.stats.links.iter_mut().find(|g| g.label == label) {
            if !group.entities.iter().any(|e| e.key == entity.key) {
                group.entities.push(entity);
            }
        } else {
            b.stats.links.push(StatLinks { label, entities: vec![entity] });
        }
    }
    if template == "TIMiningProfileTemplate" {
        for (key, label, sprite) in [
            ("water", "Water", "water"),
            ("volatiles", "Volatiles", "volatiles"),
            ("metals", "Metals", "metal"),
            ("nobles", "Noble metals", "metal_noble"),
            ("fissiles", "Fissiles", "radioactive"),
        ] {
            let read = |suffix: &str| record.get(&format!("{key}_{suffix}")).and_then(Value::as_f64).unwrap_or(0.0);
            let min = read("min");
            let mean = read("mean");
            let width = read("width");
            b.push(
                "Resource distribution",
                Stat {
                    label: label.into(),
                    value: format!("{} mean", num(mean)),
                    detail: Some(format!("Minimum {}; distribution width {}; jump {}", num(min), num(width), num(read("jump")))),
                    sentiment: "neutral".into(),
                    icon: b.icon(&format!("sprite:{sprite}")),
                    field: format!("{key}_mean"),
                },
            );
        }
    }
    if template == "TIStartTimeTemplate"
        && let (Some(year), Some(month), Some(day)) = (record.get("year"), record.get("month"), record.get("day"))
    {
        b.stats.notes.push(format!("Start date: {year}-{month}-{day}"));
    }
    if template == "TIRegionTemplate" {
        for reference in view.references_to(id) {
            let Some(claim) = view.record(reference.from) else {
                continue;
            };
            if view.dataset().entity(reference.from).template.as_ref() == "TIBilateralTemplate"
                && claim.get("initialOwner").and_then(Value::as_bool) == Some(true)
                && let Some(nation) = claim.get("nation1").and_then(Value::as_str).and_then(|n| view.find("TINationTemplate", n))
            {
                b.stats.links.push(StatLinks { label: "Nation".into(), entities: vec![entity_ref(view, texts, nation, spoilers)] });
            }
        }
    }
    for (field, value) in record {
        if !hidden(field) && !(template == "TIMiningProfileTemplate" && ["_min", "_mean", "_width", "_jump"].iter().any(|suffix| field.ends_with(suffix))) {
            present(&mut b, field, value, group(template, field), &linked, 0);
        }
    }
    if template == "TINationTemplate" {
        // Region membership is indexed by the reference graph, including scenario variants.
        let mut regions = HashSet::new();
        let mut population = 0.0;
        for reference in view.references_to(id) {
            let Some(claim) = view.record(reference.from) else {
                continue;
            };
            if view.dataset().entity(reference.from).template.as_ref() != "TIBilateralTemplate"
                || claim.get("initialOwner").and_then(Value::as_bool) != Some(true)
            {
                continue;
            }
            let Some(region) = claim.get("region1").and_then(Value::as_str).and_then(|n| view.find("TIRegionTemplate", n)) else {
                continue;
            };
            if regions.insert(region) {
                population += view.record(region).and_then(|r| r.get("population_Millions")).and_then(Value::as_f64).unwrap_or(0.0);
                if claim.get("capitalClaim").and_then(Value::as_bool) == Some(true) {
                    b.stats.links.push(StatLinks { label: "Capital".into(), entities: vec![entity_ref(view, texts, region, spoilers)] });
                }
            }
        }
        if !regions.is_empty() {
            tile(&mut b, "Economy & population", "population_Millions", "Population", format!("{} million", num(population)), Tone::Neutral, population);
            let mut ids: Vec<_> = regions.into_iter().collect();
            ids.sort_by_key(|id| view.dataset().entity(*id).data_name.clone());
            b.stats.links.push(StatLinks { label: "Regions".into(), entities: ids.into_iter().map(|id| entity_ref(view, texts, id, spoilers)).collect() });
            if population > 0.0
                && let Some(gdp) = record.get("initialGDP").and_then(Value::as_f64)
            {
                tile(
                    &mut b,
                    "Economy & population",
                    "perCapitaGDP",
                    "GDP per capita",
                    format!("${}", num((gdp / (population * 1e6)).round())),
                    Tone::Neutral,
                    gdp,
                );
            }
        }
    }
    if template == "TISpaceBodyTemplate"
        && let (Some(mass), Some(radius)) =
            (record.get("mass_kg").and_then(Value::as_f64), record.get("equatorialRadius_km").or_else(|| record.get("meanRadius_km")).and_then(Value::as_f64))
        && radius > 0.0
    {
        let gravity = 6.67430e-11 * mass / (radius * 1000.0).powi(2);
        tile(&mut b, "Physical properties", "surfaceGravity", "Surface gravity", format!("{} m/s²", amount(gravity)), Tone::Neutral, gravity);
    }
    if b.stats.groups.is_empty() && b.stats.notes.is_empty() && b.stats.links.is_empty() {
        b.stats.notes.push(format!("{} definition", humanize(template.trim_start_matches("TI").trim_end_matches("Template"))));
    }
    Some(b.stats)
}

fn hidden(field: &str) -> bool {
    let lower = field.to_ascii_lowercase();
    matches!(
        field,
        "dataName"
            | "friendlyName"
            | "displayName"
            | "locPath"
            | "scenarioTags"
            | "disable"
            | "$type"
            | "localizationAlias"
            | "referenceAlias"
            | "color"
            | "color32"
    ) || ["path", "texture", "icon", "portrait", "video", "sound", "skin", "illustration", "resource", "model"].iter().any(|s| lower.contains(s))
        && !matches!(field, "resourcesGranted" | "startingResources" | "resource" | "resourceType" | "resourceAmount" | "modelValue" | "oilResource")
}

fn path_label(path: &str) -> String {
    path.split('.').map(|p| humanize(p.trim_end_matches("[]"))).collect::<Vec<_>>().join(" · ")
}

fn group<'a>(template: &str, field: &'a str) -> &'a str {
    if field.starts_with("base") && template == "TICouncilorTypeTemplate" || field.starts_with("statMods") {
        return "Attributes";
    }
    if field.ends_with("Setting") || template == "TIPriorityPresetTemplate" {
        return "Priorities";
    }
    if field.starts_with("income") || field == "baseAnnualIncomes" {
        return "Income";
    }
    if field.starts_with("bonus") || field == "startingResources" {
        return "Starting bonuses";
    }
    match template {
        "TINationTemplate" => {
            if matches!(field, "initialGDP" | "highestPerCapitaGDP" | "popGrowthModifier" | "greenEconomy") {
                "Economy & population"
            } else {
                "Society & capabilities"
            }
        }
        "TIRegionTemplate" => "Population & facilities",
        "TISpaceBodyTemplate" => "Physical properties",
        "TIOrbitTemplate" | "TINavigableTemplate" => "Orbit",
        "TIMiningProfileTemplate" => "Resource distribution",
        "TIHabSiteTemplate" => "Site",
        "TIArmyTemplate" => "Deployment",
        "TISpaceShipTemplate" => "Ship design",
        "TISpaceFleetTemplate" => "Fleet",
        "TIFormationTemplate" => "Formation",
        "TIMissionTemplate" => "Mission rules",
        "TITraitTemplate" => "Trait",
        "TIObjectiveTemplate" => "Objective",
        "TINarrativeEventTemplate" => "Event",
        "TIVictoryTemplate" => "Victory",
        "TIStartTimeTemplate" => "Scenario",
        "TIFactionTemplate" | "TIFactionIdeologyTemplate" => "Faction",
        _ => "Properties",
    }
}

/// Reference-field form of a presentation path: `statMods[1].stat` → `statMods[].stat`.
fn ref_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut in_index = false;
    for c in path.chars() {
        match c {
            '[' => {
                in_index = true;
                out.push('[');
            }
            ']' => {
                in_index = false;
                out.push(']');
            }
            _ if in_index => {}
            _ => out.push(c),
        }
    }
    out
}

/// `path` keeps array indices (`statMods[1].stat`) so every stat's `field` is unique.
fn present(b: &mut Builder<'_, '_>, path: &str, value: &Value, group: &str, linked: &HashSet<String>, depth: usize) {
    let normalized = ref_path(path);
    if linked.contains(&normalized) || linked.contains(&format!("{normalized}[]")) {
        return;
    }
    let field = normalized.rsplit('.').next().unwrap_or(&normalized).trim_end_matches("[]");
    let label = label(field);
    match value {
        Value::Null => {}
        Value::Bool(v) => tile(b, group, path, &label, if *v { "Yes" } else { "No" }.into(), Tone::Neutral, 0.0),
        Value::Number(n) => {
            if let Some(v) = n.as_f64() {
                let (fmt, tone) = numeric(field);
                let formatted = if field == "mass_kg" && v.abs() >= 1e15 { format!("{v:.4e} kg") } else { format_value(fmt, v) };
                tile(b, group, path, &label, formatted, tone, v);
            }
        }
        Value::String(s) if !s.is_empty() && s.len() <= 160 && !s.contains('/') && !s.contains('\\') => {
            if matches!(
                field,
                "objectType" | "relationType" | "armyType" | "deploymentType" | "habType" | "objectiveType" | "ideology" | "role" | "terrain" | "environment"
            ) {
                b.stats.tags.push(humanize(s));
            }
            tile(b, group, path, &label, b.enum_text("", s), Tone::Neutral, 0.0);
        }
        Value::Array(items) if !items.is_empty() => {
            if depth < 3 && items.len() <= 32 {
                for (i, item) in items.iter().enumerate() {
                    let child = format!("{path}[{i}]");
                    if item.is_object() {
                        present(b, &child, item, &format!("{label} {}", i + 1), linked, depth + 1);
                    } else if !linked.contains(&format!("{normalized}[]")) {
                        let text = match item {
                            Value::String(s) => humanize(s),
                            _ => item.to_string(),
                        };
                        if text.len() <= 160 {
                            b.stats.notes.push(format!("{label} {}: {text}", i + 1));
                        }
                    }
                }
            } else {
                tile(b, group, path, &label, format!("{} entries", items.len()), Tone::Neutral, 0.0);
            }
        }
        Value::Object(map) if !map.is_empty() && depth < 5 => {
            if let Some(t) = map.get("$type").and_then(Value::as_str) {
                b.stats.notes.push(format!("{label}: {}", humanize(t.trim_start_matches("TI"))));
            }
            // Array members keep their numbered group ("Stat mods 2").
            let group = if path.ends_with(']') { group } else { &label };
            for (key, v) in map {
                if !hidden(key) {
                    present(b, &format!("{path}.{key}"), v, group, linked, depth + 1);
                }
            }
        }
        _ => {}
    }
}

fn label(field: &str) -> String {
    match field {
        "initialGDP" => "GDP".into(),
        "population_Millions" => "Population".into(),
        "XPCost" => "XP cost".into(),
        "equatorialRadius_km" => "Equatorial radius".into(),
        "mass_kg" => "Mass".into(),
        _ => humanize(field),
    }
}

fn numeric(field: &str) -> (Fmt, Tone) {
    let fmt = match field {
        "initialGDP" => Fmt::Num(" USD"),
        "highestPerCapitaGDP" => Fmt::Num(" USD/person"),
        "population_Millions" => Fmt::Num(" million"),
        "baseChance" | "chance" | "spawnChance" => Fmt::Num("%"),
        "XPModifier" => Fmt::SignedPct,
        _ => {
            let unit = field.rsplit('_').next().unwrap_or("");
            Fmt::Num(match unit {
                "km" => " km",
                "km2" => " km²",
                "kg" => " kg",
                "AU" => " AU",
                "Deg" | "deg" => "°",
                "years" => " years",
                "months" => " months",
                "days" => " days",
                "ppm" => " ppm",
                "cm" => " cm",
                "gcm3" => " g/cm³",
                "tons" => " t/year",
                _ => "",
            })
        }
    };
    let tone = match field {
        "unrest" | "inequality" | "XPCost" | "XPModifier" => Tone::Down,
        "education" | "startingStrength" => Tone::Up,
        _ => Tone::Neutral,
    };
    (fmt, tone)
}

fn tile(b: &mut Builder<'_, '_>, group: &str, path: &str, label: &str, value: String, tone: Tone, number: f64) {
    let lower = path.to_ascii_lowercase();
    let icon = MATERIALS.iter().find(|(key, _, _)| lower.contains(&key.to_ascii_lowercase())).and_then(|(_, _, sprite)| b.icon(&format!("sprite:{sprite}")));
    b.push(group, Stat { label: label.into(), value, detail: None, sentiment: sentiment(tone, number).into(), icon, field: path.into() });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::tests::backend;

    #[test]
    fn ref_paths_drop_indices() {
        assert_eq!(ref_path("statMods[1].stat"), "statMods[].stat");
        assert_eq!(ref_path("a[0][12].b"), "a[][].b");
    }

    /// The frontend keys stat tiles by `field` within a group.
    #[test]
    fn stat_fields_unique_per_group() {
        let Some(b) = backend() else { return };
        let texts = b.texts("en").unwrap();
        let view = b.ds.overview();
        for template in ["TITraitTemplate", "TIMissionTemplate", "TINarrativeEventTemplate", "TISpaceShipTemplate", "TIVictoryTemplate"] {
            for id in view.entities(template) {
                let Some(stats) = crate::stats::entity_stats(&view, &texts, id, &b.spoilers(&view).unwrap()) else {
                    continue;
                };
                for group in &stats.groups {
                    let mut seen = HashSet::new();
                    for stat in &group.stats {
                        assert!(seen.insert(&stat.field), "{template}:{} repeats {} in {}", view.dataset().entity(id).data_name, stat.field, group.label);
                    }
                }
            }
        }
    }

    #[test]
    fn all_templates_have_presentation() {
        let Some(b) = backend() else { return };
        let texts = b.texts("en").unwrap();
        let view = b.ds.overview();
        for template in b.ds.templates() {
            let Some(id) = view.entities(template).next() else {
                continue;
            };
            let stats = entity_stats(&view, &texts, id, &b.spoilers(&view).unwrap());
            if matches!(view.kind(id), Kind::Tech | Kind::Project | Kind::Effect) {
                assert!(stats.is_none(), "{template}");
            } else {
                let stats = stats.unwrap_or_else(|| panic!("{template}"));
                assert!(!stats.groups.is_empty() || !stats.notes.is_empty() || !stats.links.is_empty(), "{template}");
            }
        }
    }

    #[test]
    fn world_and_people_stats() {
        let Some(b) = backend() else { return };
        let texts = b.texts("en").unwrap();
        let view = b.ds.overview();
        let get = |template, name| entity_stats(&view, &texts, view.find(template, name).unwrap(), &b.spoilers(&view).unwrap()).unwrap();
        let nation = get("TINationTemplate", "AFG");
        assert!(nation.groups.iter().flat_map(|g| &g.stats).any(|s| s.field == "population_Millions"));
        assert!(nation.links.iter().any(|l| l.label == "Capital"));
        let body = get("TISpaceBodyTemplate", "Sol");
        assert!(body.groups.iter().flat_map(|g| &g.stats).any(|s| s.field == "mass_kg" && s.value.ends_with(" kg")));
        let trait_stats = get("TITraitTemplate", "Wealthy");
        assert!(trait_stats.groups.iter().flat_map(|g| &g.stats).any(|s| s.field == "XPCost"));
        let bilateral =
            view.entities("TIBilateralTemplate").find(|&id| view.record(id).is_some_and(|r| r.get("nation1").is_some() && r.get("nation2").is_some())).unwrap();
        let stats = entity_stats(&view, &texts, bilateral, &b.spoilers(&view).unwrap()).unwrap();
        assert_eq!(stats.links.iter().flat_map(|l| &l.entities).filter(|e| e.template == "TINationTemplate").count(), 2);
    }
}
