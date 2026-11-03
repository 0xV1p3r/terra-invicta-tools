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

/// Game icon (file under the build's `assets/`) for effect contexts and instant effects.
const GAME_ICONS: &[(&str, &str)] = &[
    // National priorities
    ("BuildArmyPriority", "icons_2d/ICO_buildArmy_priority.png"),
    ("BuildNuclearWeaponsPriority", "icons_2d/ICO_buildNuclearWeapons_priority.png"),
    ("BuildSTOSquadronPriority", "icons_2d/ICO_buildSTOSquadron_priority.png"),
    ("BuildSpaceDefensesPriority", "icons_2d/ICO_buildSpaceDefenses_priority.png"),
    ("EconomyPriority", "icons_2d/ICO_economy_priority.png"),
    ("EnvironmentPriority", "icons_2d/ICO_environment_priority.png"),
    ("GovernmentPriority", "icons_2d/ICO_government_priority.png"),
    ("KnowledgePriority", "icons_2d/ICO_knowledge_priority.png"),
    ("LaunchFacilitiesPriority", "icons_2d/ICO_launchFacilities_Priority.png"),
    ("MilitaryPriority", "icons_2d/ICO_military_priority.png"),
    ("MissionControlPriority", "icons_2d/ICO_missionControl_priority.png"),
    ("OppressionPriority", "icons_2d/ICO_oppression_priority.png"),
    ("SpaceflightPriority", "icons_2d/ICO_spaceflightProgram_priority.png"),
    ("SpoilsPriority", "icons_2d/ICO_spoils_priority.png"),
    ("UnityPriority", "icons_2d/ICO_unity_priority.png"),
    ("UpgradeArmyPriority", "icons_2d/ICO_buildNavy_priority.png"),
    ("WelfarePriority", "icons_2d/ICO_welfare_priority.png"),
    // Resources
    ("CanAmassAntimatter", "icons_2d/ICO_antimatter.png"),
    ("CanAmassExotics", "icons_2d/ICO_exotics.png"),
    ("MiningFissilesBonus", "icons_2d/ICO_fissile.png"),
    ("MiningMetalsBonus", "icons_2d/ICO_metal.png"),
    ("MiningNoblesBonus", "icons_2d/ICO_metal_noble.png"),
    ("MiningVolatilesBonus", "icons_2d/ICO_volatiles.png"),
    ("MiningWaterBonus", "icons_2d/ICO_water.png"),
    ("SpaceMiningBonus", "sprite:mining"),
    ("MineSizeModifier", "sprite:mining"),
    ("GainExoticsFromSpaceIndustry", "icons_2d/ICO_exotics.png"),
    ("NationSetCanDecontaminateRegion", "icons_2d/ICO_environment_priority.png"),
    ("CanCaptureHydra", "councilor_missions/ICO_detain_on.png"),
    ("GenericTransferEV_kps", "icons_2d/ICO_boost.png"),
    ("HabMissionControlReduction", "icons_2d/ICO_mission_control.png"),
    ("ShipMissionControlReduction", "icons_2d/ICO_mission_control.png"),
    ("MCFreeSpaceMineNetwork", "icons_2d/ICO_mission_control.png"),
    ("ControlPointResearch", "icons_2d/ICO_research.png"),
    ("HabResearchProduction", "icons_2d/ICO_research.png"),
    ("GlobalIPProduction", "icons_2d/ICO_investments.png"),
    ("ProjectUnlockChance", "icons_2d/ICO_projects.png"),
    ("MonthlyProjectTriggerChance", "icons_2d/ICO_projects.png"),
    ("RerollMissingProjects", "icons_2d/ICO_projects.png"),
    ("DirectInvestGlobalDiscount_Money_PCT", "icons_2d/ICO_currency.png"),
    ("DirectInvestGlobalDiscount_Influence_PCT", "icons_2d/ICO_influence.png"),
    ("ResourceMarketSales", "icons_2d/ICO_currency.png"),
    ("OrgPurchaseCost", "icons_2d/ICO_currency.png"),
    ("AugmentationMoneyCost", "icons_2d/ICO_currency.png"),
    ("PublicOpinionInfluence", "icons_2d/ICO_influence.png"),
    ("SpoilsOutput", "icons_2d/ICO_spoils_priority.png"),
    ("Corruption", "icons_2d/ICO_spoils_priority.png"),
    ("Economy_BasePCGDPIncrease", "icons_2d/ICO_economy_priority.png"),
    ("MapRegionGDPPctChange_StrValue", "icons_2d/ICO_economy_priority.png"),
    ("Welfare_CO2_ppm", "icons_2d/ICO_environment_priority.png"),
    ("Welfare_CH4_ppm", "icons_2d/ICO_environment_priority.png"),
    ("Welfare_N2O_ppm", "icons_2d/ICO_environment_priority.png"),
    ("Environment_BestSustainabilityValue", "icons_2d/ICO_environment_priority.png"),
    ("NationSetCanBuildSpaceDefenses", "icons_2d/ICO_buildSpaceDefenses_priority.png"),
    ("NationSetCanBuildSTOFighters", "icons_2d/ICO_buildSTOSquadron_priority.png"),
    ("NationMaxMiltechChange", "icons_2d/ICO_military_priority.png"),
    // Inline text sprites
    ("ControlPointMaintenance", "sprite:control_point"),
    ("CouncilSize", "sprite:attribute_administration"),
    ("NewCouncilorRecruitXP", "sprite:star"),
    ("MaxMissionSliderSteps", "sprite:ops"),
    ("ArmyDamageBonustoAllArmies", "sprite:army_battle"),
    ("ArmyDamageBonustoHumanArmy", "sprite:army_battle"),
    ("ArmyDamageBonustoInvaderArmy", "sprite:army_battle"),
    ("ArmyDamageBonustoMegafauna", "sprite:army_battle"),
    ("ArmyHealRate", "sprite:army"),
    ("ArmyRuggedWarfare", "sprite:rugged_terrain"),
    ("ArmyUrbanWarfare", "sprite:eco_region"),
    ("AdvancedAircraft", "sprite:build_sto"),
    ("ArmyNuclearHardening", "sprite:nukes"),
    ("NuclearStrikeDamageReduction", "sprite:nukes"),
    ("BombardmentArmyDefenseBonus", "sprite:laser_orbit"),
    ("BombardmentHabDefenseBonus", "sprite:hab_defense"),
    ("DamageReductionAgainstAllShips", "sprite:armor"),
    ("DamageReductionAgainstAlienShips", "sprite:armor"),
    ("LaserDefenseType", "sprite:laser_orbit"),
    ("LaserDefenseFreq", "sprite:laser_orbit"),
    ("STOFighterECM", "sprite:build_sto"),
    ("GenericModuleTransferTime", "sprite:construction_module"),
    ("GenericTransfer_OffDate_PCT", "sprite:construction_module"),
    ("HabNuclearFreighters", "sprite:construction_module"),
    ("HabConstructionSpeed", "sprite:construction_module"),
    ("ProbeTransferTime", "sprite:probe"),
    ("DetectAlienSpaceAssetsRange", "sprite:alien_marker"),
    ("SemiMajorAxisExplorationRange_AU", "sprite:radius_of_orbit"),
    ("InnerExplorationRange_AU", "sprite:sun"),
    ("GlobalFissionTechLevel", "sprite:tech_Energy"),
    ("GlobalFusionTechLevel", "sprite:tech_Energy"),
    ("BilateralRelationsCooldownMultiplier", "sprite:nation_at_peace"),
    ("SpaceScan", "sprite:show"),
    // Councilor missions
    ("DetectAliensOnEarth", "sprite:attribute_investigation"),
    ("DetectAlienActivity", "councilor_missions/ICO_investigatealienactivity_on.png"),
    ("InterrogationBonus", "councilor_missions/ICO_detain_on.png"),
    ("DefendInterestsValue", "councilor_missions/ICO_defendinterest_on.png"),
    ("CanContactAliens", "councilor_missions/ICO_contact_on.png"),
    ("AlienRelationsEstablished", "councilor_missions/ICO_contact_on.png"),
    ("PublicCampaignStrength", "councilor_missions/ICO_propaganda_on.png"),
    ("AtrocityMitigation", "councilor_missions/ICO_propaganda_on.png"),
    ("Propaganda", "councilor_missions/ICO_propaganda_on.png"),
    ("PherocyteResistance", "councilor_missions/ICO_enthrallpublic_on.png"),
    ("BreakawayChance", "sprite:nation_unrest"),
    ("PostRegimeChangeUnrestReduction", "sprite:nation_unrest"),
    ("PostRegimeLossUnrestIncrease", "sprite:nation_unrest"),
    ("XenoformingDestructionStrength", "councilor_missions/ICO_Xenoform_on.png"),
    ("ShipConstructionTime", "sprite:construction_shipyard"),
    ("CanFoundTier1Station", "habmodules/station_T1_PlatformCore.png"),
    ("CanFoundTier2Station", "habmodules/station_T2_OrbitalCore.png"),
    ("CanFoundTier3Station", "habmodules/station_T3_RingCore.png"),
    ("CanFoundTier1Base", "habmodules/base_T1_OutpostCore.png"),
    ("CanFoundTier2Base", "habmodules/base_T2_SettlementCore.png"),
    ("CanFoundTier3Base", "habmodules/base_T3_ColonyCore.png"),
    ("CanFoundAutomatedT1Station", "habmodules/station_T1_AutomatedPlatformCore.png"),
    ("CanFoundAutomatedT1Base", "habmodules/base_T1_AutomatedOutpostCore.png"),
    // Ship officers
    ("ShipOfficerPromotion", "icons_2d/Ship_Officer_Admiral_1.png"),
    ("Combat_ShipRepairSpeed", "icons_2d/Ship_Officer_DamageControl_1.png"),
    ("ShipLaserDamage", "icons_2d/Ship_Officer_DirectedEnergy_1.png"),
    ("ShipMagDamage", "icons_2d/Ship_Officer_Gunnery_1.png"),
    ("ShipConvMissileDamage", "icons_2d/Ship_Officer_FireControl_1.png"),
    ("TargetingComputerBonus", "icons_2d/Ship_Officer_CIC_1.png"),
    ("Ship_MaxSurvivableCombatAcceleration_Bonus", "icons_2d/Ship_Officer_Propulsion_1.png"),
    ("Ship_MaxSurvivableCruiseAcceleration_Bonus", "icons_2d/Ship_Officer_Propulsion_1.png"),
];

/// Icons of single effects whose context is too generic (exploration range → the body opened).
const EFFECT_ICONS: &[(&str, &str)] = &[
    ("Effect_ExploreVenus", "icons_2d/ICO_Venus.png"),
    ("Effect_ExploreInnerPlanets", "icons_2d/ICO_Mercury.png"),
    ("Effect_ExploreLuna", "icons_2d/ICO_Luna.png"),
    ("Effect_ExploreMars", "icons_2d/ICO_Mars.png"),
    ("Effect_ExploreEarthLagrangePoints", "icons_2d/ICO_Earth.png"),
    ("Effect_ExploreAsteroids", "icons_2d/ICO_Ceres.png"),
    ("Effect_BSBE_ExploreAsteroids", "icons_2d/ICO_Ceres.png"),
    ("Effect_ExploreJupiter", "icons_2d/ICO_Jupiter.png"),
    ("Effect_ExploreSaturn", "icons_2d/ICO_Saturn.png"),
    ("Effect_ExploreOuterPlanets", "icons_2d/ICO_Uranus.png"),
    ("Effect_ExploreKuiperBelt", "icons_2d/ICO_Pluto.png"),
];

/// `Mission_<name>_Att/_Def` → councilor mission icon name, where it differs from the lowercased name.
const MISSION_ICONS: &[(&str, &str)] = &[
    ("Assassination", "assassinate"),
    ("ControlAsset", "controlspaceasset"),
    ("GainInfluence", "gain_influence"),
    ("GainControlPoint", "assumecontrol"),
    ("SabotageHabModule", "sabotagehab"),
    ("AssaultAlienAsset", "assaultfacility"),
    ("Xenoform", "Xenoform"),
];

/// Councilor mission icon files that exist (`councilor_missions/ICO_<name>_on.png`).
const MISSION_FILES: &[&str] = &[
    "abductions", "assassinate", "assaultfacility", "assumecontrol", "contact", "controlspaceasset", "coup",
    "crackdown", "defendinterest", "enthrallelites", "enthrallpublic", "gain_influence", "hostiletakeover",
    "inspire", "propaganda", "purge", "sabotagefacilities", "sabotagehab", "sabotageproject", "seizespaceasset",
    "stabilize", "stealproject", "turn", "unrest", "Xenoform",
];

/// Asset file of an icon table entry: a path under `assets/`, or `sprite:<name>` for an
/// inline text sprite (None if this build lacks it).
fn resolve(view: &View<'_>, icon: &str) -> Option<String> {
    match icon.strip_prefix("sprite:") {
        Some(name) => view.dataset().sprite(name).map(|a| a.file.clone()),
        None => Some(icon.to_string()),
    }
}

/// Game icon file for an effect context or instant effect name.
fn game_icon(name: &str) -> Option<String> {
    if let Some((_, file)) = GAME_ICONS.iter().find(|(n, _)| *n == name) {
        return Some((*file).to_string());
    }
    let rest = name.strip_prefix("Mission_")?;
    let mission = rest.rsplit_once('_').map_or(rest, |(m, _)| m);
    let icon = MISSION_ICONS.iter().find(|(m, _)| *m == mission).map_or_else(|| mission.to_lowercase(), |(_, i)| (*i).to_string());
    MISSION_FILES.contains(&icon.as_str()).then(|| format!("councilor_missions/ICO_{icon}_on.png"))
}

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
    let icon = EFFECT_ICONS.iter().find(|(n, _)| *n == effect.data_name).map(|(_, f)| (*f).to_string()).or_else(|| {
        // Councilor attribute changes name the attribute in strValue.
        let attribute = effect.str_value.as_ref().and_then(|v| v.as_str()).map(|a| format!("sprite:attribute_{}", a.to_lowercase()));
        attribute
            .filter(|a| resolve(view, a).is_some())
            .or_else(|| effect.contexts.first().or(effect.instant_effect.as_ref()).and_then(|n| game_icon(n)))
    });
    let icon = icon.and_then(|i| resolve(view, &i));
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
    let icon = icon.or_else(|| (dom == "alien").then(|| "mapicons/ICO_geoscape_councilor_alien_generic.png".to_string()));
    let sprite_file = |name: &str| view.dataset().sprite(name).map(|a| a.file.clone());
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
                let icon = (kind == "sprite").then(|| sprite_file(&text)).flatten();
                EffectSegment { kind: kind.into(), text, icon }
            })
            .collect(),
        label,
        modifier: modifier_text,
        sentiment: sentiment.into(),
        domain: dom.into(),
        category,
        target: target_label(&effect.target).map(Into::into),
        icon,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::commands::tests::backend;
    use crate::refs::entity_ref;

    #[test]
    fn game_icons_exist() {
        let Some(b) = backend() else { return };
        let view = b.ds.overview();
        let missing: Vec<String> = GAME_ICONS
            .iter()
            .chain(EFFECT_ICONS)
            .map(|(_, f)| (*f).to_string())
            .chain(MISSION_FILES.iter().map(|m| format!("councilor_missions/ICO_{m}_on.png")))
            .filter(|f| !resolve(&view, f).and_then(|f| b.ds.asset_path(&f)).is_some_and(|p| p.is_file()))
            .collect();
        assert!(missing.is_empty(), "{missing:?}");
        assert_eq!(game_icon("Mission_Purge_Att").as_deref(), Some("councilor_missions/ICO_purge_on.png"));
        assert_eq!(game_icon("Mission_GainControlPoint").as_deref(), Some("councilor_missions/ICO_assumecontrol_on.png"));
        assert_eq!(game_icon("Mission_Terrorize_Def"), None);
    }

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
                "{:<45} {:<14} {:<55} {:<8} {:<7} {:<45} {:<9} {}\n",
                l.entity.data_name,
                l.domain.clone() + if l.icon.is_some() { "*" } else { "" },
                l.icon.as_deref().unwrap_or(""),
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
        assert_eq!(line("Effect_MiningMetalsBonus").icon.as_deref(), Some("icons_2d/ICO_metal.png"));
        assert_eq!(line("Effect_AllHumanFactionsGainExotics").icon.as_deref(), Some("icons_2d/ICO_exotics.png"));
        assert_eq!(line("Effect_AllRecruitsInvestigation2").icon.as_deref(), Some("sprites/attribute_investigation.png"));
        assert_eq!(cp.icon.as_deref(), Some("sprites/control_point.png"));
        let metal = line("Effect_MiningMetalsBonus");
        assert!(metal.segments.iter().any(|s| s.kind == "sprite" && s.icon.as_deref() == Some("sprites/metal.png")));
        assert!(line("Effect_MiningMetalsBonus").segments.iter().any(|s| s.kind == "sprite" && s.text == "metal"));
    }
}
