//! Effect descriptions and display metadata for the detail panel.

use ti_data::effect_text::format_small_number;
use ti_data::model::Effect;
use ti_data::{EffectContext, EffectSegment as Segment, EntityId, Texts, View};

use crate::dto::{EffectLine, EffectSegment, EntityRef};
use crate::properties::{Label, effect_label, raises};

/// Effect description as the game renders it in tech/project tooltips (placeholders filled;
/// state-dependent ones use neutral stand-ins), or `None` if the game hides it.
pub fn effect_text(view: &View<'_>, texts: &Texts, effect_id: EntityId) -> Option<String> {
    view.effect_description(texts, effect_id, &EffectContext::default())
}

/// Contexts where more of the labelled quantity is worse for the acting faction.
const LOWER_IS_BETTER: &[&str] = &[
    "AlienHateFromMCUsage",
    "ArmyNuclearHardening",
    "AtrocityMitigation",
    "AugmentationMoneyCost",
    "BilateralRelationsCooldownMultiplier",
    "BombardmentArmyDefenseBonus",
    "BombardmentHabDefenseBonus",
    "Corruption",
    "DamageReductionAgainstAlienShips",
    "DamageReductionAgainstAllShips",
    "DirectInvestGlobalDiscount_Influence_PCT",
    "DirectInvestGlobalDiscount_Money_PCT",
    "GenericModuleTransferTime",
    "GenericTransferEV_kps",
    "GenericTransfer_OffDate_PCT",
    "HabMissionControlReduction",
    "MegafaunaDamageMitigation",
    "NuclearStrikeDamageReduction",
    "OrgPurchaseCost",
    "PostRegimeChangeUnrestReduction",
    "ProbeTransferTime",
    "ShipConstructionTime",
    "ShipMissionControlReduction",
];

/// Contexts whose direction is a trade-off or a switch rather than a gain.
const NEUTRAL: &[&str] = &["BreakawayChance", "CanContactAliens", "CanTransferTerritoryToAliens", "LaserDefenseFreq", "LaserDefenseType"];

/// Research category of single-science contexts.
const SCIENCES: &[(&str, &str)] = &[
    ("EnergyScience", "Energy"),
    ("InformationScience", "InformationScience"),
    ("LifeScience", "LifeScience"),
    ("MaterialScience", "Materials"),
    ("MilitaryScience", "MilitaryScience"),
    ("SocialScience", "SocialScience"),
    ("SpaceScience", "SpaceScience"),
    ("Xenology", "Xenology"),
];

/// Icon group of an effect context (or of an instant effect's name).
fn domain(name: &str) -> &'static str {
    let has = |parts: &[&str]| parts.iter().any(|p| name.contains(p));
    let starts = |parts: &[&str]| parts.iter().any(|p| name.starts_with(p));
    if SCIENCES.iter().any(|(c, _)| *c == name)
        || has(&["Research", "ProjectUnlock", "ProjectTrigger", "RerollMissingProjects", "TechLevel", "Miltech"])
    {
        "research"
    } else if name.ends_with("Priority") {
        "priority"
    } else if starts(&["Mission_"]) {
        "mission"
    } else if starts(&["ControlPoint"]) {
        "control"
    } else if has(&["Recruit", "Councilor", "CouncilSize", "Lifespan", "Augmentation", "Interrogation", "MissionSlider"]) {
        "councilor"
    } else if has(&["MissionControl", "MCFree"]) {
        "missionControl"
    } else if has(&["Alien", "Megafauna", "Pherocyte", "Xenoform", "Hydra"]) {
        "alien"
    } else if starts(&["Army", "Bombardment", "NuclearStrike", "DefendInterests", "AdvancedAircraft"]) {
        "army"
    } else if starts(&["Ship", "DamageReduction", "Combat_", "TargetingComputer", "STOFighter", "LaserDefense", "HumanECM", "GlobalECM", "NationSetCanBuild"]) {
        "ship"
    } else if has(&["Mining", "MineSize", "ResourceMarket", "CanAmass", "Exotics", "SpaceResource"]) {
        "mining"
    } else if has(&["Exploration", "Explore", "ProbeTransfer", "SpaceScan"]) {
        "exploration"
    } else if starts(&["GenericTransfer", "GenericModuleTransfer", "HabNuclearFreighters"]) {
        "transfer"
    } else if starts(&["CanFound", "HabConstruction"]) {
        "hab"
    } else if starts(&["Welfare_", "Environment", "EndofOil", "NationSetCanDecontaminate"]) {
        "environment"
    } else if starts(&["Economy_", "Spoils", "DirectInvest", "OrgPurchase", "Corruption", "MaxAvailableOrgs", "MapRegionGDP", "GlobalIP"]) {
        "economy"
    } else if starts(&["PostRegime", "Breakaway", "PublicCampaign", "PublicOpinion", "Atrocity", "Bilateral", "Propaganda", "RandomFactionNationGainsRandomClaim"]) {
        "politics"
    } else {
        "other"
    }
}

fn target_label(target: &str) -> Option<&'static str> {
    Some(match target {
        "SourceFaction" | "" => return None,
        "AllHumanFactions" => "All human factions",
        "AllFactions" => "All factions",
        "AllNations" | "AllExtantNations" => "All nations",
        "InputState" => "Selected nation or region",
        "GlobalEffectSet" => "Global",
        _ => "Other target",
    })
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// Signed change of the labelled quantity, e.g. `+100%`; `percent` says how additive values read.
fn modifier(effect: &Effect, inverted: bool, percent: bool) -> Option<String> {
    let v = effect.value;
    let (delta, percent) = match effect.operation.as_deref() {
        // The value is a new level of the raw context, which inverted labels don't show.
        Some("SetToFixedValue" | "IncreaseToValue" | "DecreaseToValue") => {
            return (!inverted).then(|| format!("→ {}", format_small_number(v)));
        }
        Some("Multiplicative") => (v - 1.0, true),
        _ => (v, percent),
    };
    if delta == 0.0 || !delta.is_finite() {
        return None;
    }
    let up = raises(effect) != inverted;
    let magnitude = if percent {
        format!("{}%", format_small_number((delta.abs() * 1000.0).round() / 10.0))
    } else {
        format_small_number(delta.abs())
    };
    Some(format!("{}{magnitude}", if up { '+' } else { '−' }))
}

pub fn effect_line(view: &View<'_>, texts: &Texts, id: EntityId, entity: EntityRef) -> EffectLine {
    let segments = view.effect_segments(texts, id, &EffectContext::default()).unwrap_or_default();
    let effect = view.effect(id).unwrap_or_default();
    let first_value = segments.iter().find_map(|s| match s {
        Segment::Value(v) => Some(v.as_str()),
        _ => None,
    });
    let percent = first_value.is_some_and(|v| v.ends_with('%'));
    let (mut label, mut modifier_text, mut sentiment, mut dom, mut category) = (None, None, "neutral", "other", None);
    if let Some(context) = effect.contexts.first() {
        let (kind, text) = effect_label(context);
        label = (!text.is_empty()).then(|| capitalize(&text));
        dom = domain(context);
        category = SCIENCES.iter().find(|(c, _)| c == context).map(|(_, cat)| cat.to_string());
        match kind {
            Label::Ability(_) => {
                if dom == "other" {
                    dom = "ability";
                }
                if !NEUTRAL.contains(&context.as_str()) {
                    sentiment = "good";
                }
            }
            Label::Quantity(_) | Label::Inverted(_) => {
                let inverted = matches!(kind, Label::Inverted(_));
                modifier_text = modifier(&effect, inverted, percent);
                let changes = match effect.operation.as_deref() {
                    Some("SetToFixedValue") => false,
                    Some("IncreaseToValue" | "DecreaseToValue") => true,
                    Some("Multiplicative") => effect.value != 1.0,
                    _ => effect.value != 0.0,
                };
                if changes && !NEUTRAL.contains(&context.as_str()) {
                    let up = raises(&effect) != inverted;
                    sentiment = if up != LOWER_IS_BETTER.contains(&context.as_str()) { "good" } else { "bad" };
                }
            }
        }
    } else if let Some(instant) = &effect.instant_effect {
        dom = domain(instant);
    }
    let text = (!segments.is_empty()).then(|| effect_text(view, texts, id)).flatten();
    EffectLine {
        entity,
        text,
        segments: segments
            .into_iter()
            .map(|s| {
                let (kind, text) = match s {
                    Segment::Text(t) => ("text", t),
                    Segment::Value(t) => ("value", t),
                    Segment::Sprite(t) => ("sprite", t),
                };
                EffectSegment { kind: kind.into(), text }
            })
            .collect(),
        label,
        modifier: modifier_text,
        sentiment: sentiment.into(),
        domain: dom.into(),
        category,
        target: target_label(&effect.target).map(Into::into),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::commands::tests::backend;
    use crate::refs::entity_ref;

    #[test]
    fn research_effect_lines() {
        let Some(b) = backend() else { return };
        let texts = b.texts("en").unwrap();
        let view = b.ds.overview();
        let effects: BTreeSet<EntityId> = view.research_graph().nodes().iter().flat_map(|&n| view.effects(n)).collect();
        let mut dump = String::new();
        for &e in &effects {
            let l = effect_line(&view, &texts, e, entity_ref(&view, &texts, e));
            let text: String = l.segments.iter().map(|s| if s.kind == "value" { format!("[{}]", s.text) } else { s.text.clone() }).collect();
            assert!(!text.starts_with('-'), "{text}");
            dump += &format!(
                "{:<45} {:<14} {:<8} {:<7} {:<45} {:<9} {}\n",
                l.entity.data_name,
                l.domain,
                l.sentiment,
                l.modifier.as_deref().unwrap_or(""),
                l.label.as_deref().unwrap_or(""),
                l.category.as_deref().unwrap_or(""),
                text.replace('\n', " / ")
            );
        }
        if let Ok(path) = std::env::var("EFFECT_DUMP") {
            std::fs::write(path, dump).unwrap();
        }
        let line = |name: &str| {
            let id = view.find(ti_data::model::EFFECT, name).unwrap();
            effect_line(&view, &texts, id, entity_ref(&view, &texts, id))
        };
        let ship = line("Effect_BSBE_ShipConstructionTimePenalty");
        assert_eq!((ship.modifier.as_deref(), ship.sentiment.as_str()), (Some("+50%"), "bad"));
        let cp = line("Effect_BSBE_CPMaintenanceRepair");
        assert_eq!((cp.modifier.as_deref(), cp.sentiment.as_str(), cp.domain.as_str()), (Some("+70"), "good", "control"));
        let mats = line("Effect_BS_MaterialScienceBonusA");
        assert_eq!((mats.modifier.as_deref(), mats.category.as_deref()), (Some("+100%"), Some("Materials")));
        assert_eq!(line("Effect_BS_LifeSciencePenalty").modifier.as_deref(), Some("−50%"));
        let armor = line("Effect_BSBE_SpaceshipSurvivabilityBonus");
        assert_eq!((armor.modifier.as_deref(), armor.sentiment.as_str()), (Some("−15%"), "good"));
        // Unrest in our nations once an enemy conquers them hurts the conqueror.
        assert_eq!(line("Effect_PostRegimeLossUnrestIncrease").sentiment, "good");
        assert!(line("Effect_MiningMetalsBonus").segments.iter().any(|s| s.kind == "sprite" && s.text == "metal"));
    }
}
