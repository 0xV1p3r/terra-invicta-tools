//! Filterable "properties" of techs/projects: what their effects change (by effect
//! context and direction) and what kinds of things they unlock.
//!
//! Property ids (stable, language-independent):
//! * `effect:<context>:up` / `effect:<context>:down` – changes a quantity (e.g.
//!   `effect:ControlPointMaintenance:up` = more control point capacity). Up/down is the
//!   direction of the *labelled* quantity, which may be the inverse of the raw context.
//! * `effect:<context>`  – grants an ability (`CanFoundTier1Base`, …).
//! * `unlock:<kind>`     – unlocks hab modules, orgs, … (`unlock:shipPart:drive` per part kind).

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use ti_data::model::{Effect, PartKind};
use ti_data::{EntityId, Texts, UnlockKind, View};

use crate::dto::PropertyInfo;
use crate::effects::{changes, context_style, effect_text};
use crate::refs;

#[derive(Clone, Copy)]
pub(crate) enum Label {
    /// A quantity (lowercase noun phrase); "Increases …"/"Decreases …".
    Quantity(&'static str),
    /// A quantity that is labelled as the inverse of the raw context value
    /// (`ControlPointMaintenance` −40 = +40 control point capacity).
    Inverted(&'static str),
    /// A one-off ability or switch; the value direction is meaningless.
    Ability(&'static str),
}

use Label::*;

/// Player-facing labels of effect contexts. Unknown contexts get a humanized fallback.
const EFFECT_LABELS: &[(&str, Label)] = &[
    ("AdvancedAircraft", Ability("Advanced aircraft")),
    ("AlienHateFromMCUsage", Quantity("alien hostility from Mission Control usage")),
    ("AlienRelationsEstablished", Ability("Establishes contact with the aliens")),
    ("AllRecruitStats", Quantity("councilor recruit attributes")),
    ("AllRecruitTraits", Ability("Councilor recruits gain a trait")),
    ("ArmyDamageBonustoAllArmies", Quantity("army damage")),
    ("ArmyDamageBonustoHumanArmy", Quantity("army damage vs. human armies")),
    ("ArmyDamageBonustoInvaderArmy", Quantity("army damage vs. alien armies")),
    ("ArmyDamageBonustoMegafauna", Quantity("army damage vs. megafauna")),
    ("ArmyHealRate", Quantity("army repair rate")),
    ("ArmyNuclearHardening", Quantity("army damage taken from nuclear attacks")),
    ("ArmyRuggedWarfare", Quantity("army combat value in rugged regions")),
    ("ArmyUrbanWarfare", Quantity("army combat value in core economic regions")),
    ("AugmentationMoneyCost", Quantity("money cost of councilor augmentations")),
    ("AtrocityMitigation", Quantity("propaganda impact of atrocities")),
    ("BilateralRelationsCooldownMultiplier", Quantity("cooldowns on improving relations")),
    ("BombardmentArmyDefenseBonus", Quantity("army damage taken from orbital bombardment")),
    ("BombardmentHabDefenseBonus", Quantity("hab damage taken from orbital bombardment")),
    ("BreakawayChance", Quantity("breakaway chance after unrest missions")),
    ("BuildArmyPriority", Quantity("Build Army investment")),
    ("BuildNuclearWeaponsPriority", Quantity("nuclear weapons investment")),
    ("BuildSpaceDefensesPriority", Quantity("Space Defenses investment")),
    ("BuildSTOSquadronPriority", Quantity("STO Squadron investment")),
    ("CanAmassAntimatter", Ability("Can stockpile antimatter")),
    ("CanAmassExotics", Ability("Can stockpile exotics")),
    ("CanCaptureHydra", Ability("Can capture live aliens")),
    ("CanContactAliens", Ability("Changes contact with the aliens")),
    ("CanFoundAutomatedT1Base", Ability("Can found automated outposts")),
    ("CanFoundAutomatedT1Station", Ability("Can found automated platforms")),
    ("CanFoundTier1Base", Ability("Can found outposts")),
    ("CanFoundTier1Station", Ability("Can found platforms")),
    ("CanFoundTier2Base", Ability("Can found settlements")),
    ("CanFoundTier2Station", Ability("Can found orbitals")),
    ("CanFoundTier3Base", Ability("Can found colonies")),
    ("CanFoundTier3Station", Ability("Can found rings")),
    ("CanTransferTerritoryToAliens", Ability("Can transfer territory to the aliens")),
    ("Combat_ShipRepairSpeed", Quantity("ship repair speed")),
    ("ControlPointMaintenance", Inverted("control point capacity")),
    ("ControlPointResearch", Quantity("research from control points")),
    ("Corruption", Quantity("corruption")),
    ("CouncilSize", Quantity("council size")),
    ("DamageReductionAgainstAlienShips", Quantity("ship damage taken from alien weapons")),
    ("DamageReductionAgainstAllShips", Quantity("ship damage taken")),
    ("DefendInterestsValue", Quantity("Defend Interests defense bonus")),
    ("DetectAlienActivity", Ability("Detects alien activity")),
    ("DetectAlienSpaceAssetsRange", Quantity("alien space asset detection range")),
    ("DetectAliensOnEarth", Quantity("investigation when detecting aliens")),
    ("DirectInvestGlobalDiscount_Influence_PCT", Inverted("influence cost of direct investment")),
    ("DirectInvestGlobalDiscount_Money_PCT", Inverted("money cost of direct investment")),
    ("EnergyScience", Quantity("Energy research")),
    ("EconomyPriority", Quantity("Economy investment")),
    ("Economy_BasePCGDPIncrease", Quantity("GDP growth from the Economy priority")),
    ("EnvironmentPriority", Quantity("Environment investment")),
    ("Environment_BestSustainabilityValue", Inverted("best achievable sustainability")),
    ("ExploreEarthLagrangePoints", Ability("Opens Earth's Lagrange points and asteroids")),
    ("ExploreLuna", Ability("Opens the Moon")),
    ("GenericModuleTransferTime", Quantity("hab module delivery time")),
    ("GenericTransferEV_kps", Inverted("boost cost of launches beyond LEO")),
    ("GenericTransfer_OffDate_PCT", Quantity("delivery time outside launch windows")),
    ("GlobalFissionTechLevel", Quantity("global fission tech level")),
    ("GlobalFusionTechLevel", Quantity("global fusion tech level")),
    ("GovernmentPriority", Quantity("Government investment")),
    ("HabMissionControlReduction", Quantity("Mission Control cost of habs")),
    ("HabNuclearFreighters", Ability("Nuclear freighters")),
    ("HabResearchProduction", Quantity("research from habs")),
    ("HumanECMAgainstAliens", Ability("ECM works against alien weapons")),
    ("HumanLifespan", Quantity("councilor lifespan")),
    ("InformationScience", Quantity("Information Science research")),
    ("InnerExplorationRange_AU", Inverted("inner system exploration range")),
    ("InterrogationBonus", Quantity("intelligence from interrogations")),
    ("KnowledgePriority", Quantity("Knowledge investment")),
    ("LaserDefenseFreq", Ability("Upgrades surface defense laser frequency")),
    ("LaserDefenseType", Ability("Upgrades surface defense laser type")),
    ("LifeScience", Quantity("Life Science research")),
    ("LaunchFacilitiesPriority", Quantity("Boost investment")),
    ("MCFreeSpaceMineNetwork", Quantity("mining modules free of Mission Control cost")),
    ("MaterialScience", Quantity("Materials research")),
    ("MaxAvailableOrgs", Quantity("orgs available each month")),
    ("MaxMissionSliderSteps", Quantity("max resources spendable on missions")),
    ("MegafaunaDamageMitigation", Quantity("megafauna damage to our regions")),
    ("MegafaunaMastery", Quantity("chance to control megafauna")),
    ("MegafaunaRepellent", Ability("Megafauna avoid our regions")),
    ("MilitaryPriority", Quantity("Military investment")),
    ("MiningFissilesBonus", Quantity("fissiles mining output")),
    ("MiningMetalsBonus", Quantity("metals mining output")),
    ("MiningNoblesBonus", Quantity("noble metals mining output")),
    ("MiningVolatilesBonus", Quantity("volatiles mining output")),
    ("MiningWaterBonus", Quantity("water mining output")),
    ("MilitaryScience", Quantity("Military Science research")),
    ("MineSizeModifier", Quantity("space mine size")),
    ("MissionControlPriority", Quantity("Mission Control investment")),
    ("MonthlyProjectTriggerChance", Quantity("monthly project availability chance")),
    ("NewCouncilorRecruitXP", Quantity("XP of new councilors")),
    ("NuclearStrikeDamageReduction", Quantity("damage from nuclear strikes")),
    ("OppressionPriority", Quantity("Oppression investment")),
    ("OrgPurchaseCost", Quantity("org purchase cost")),
    ("OuterExplorationRange_AU", Quantity("outer system exploration range")),
    ("PherocyteResistance", Quantity("pherocyte resistance")),
    ("PostRegimeChangeUnrestReduction", Quantity("unrest in nations we conquer")),
    ("PostRegimeLossUnrestIncrease", Quantity("unrest when our nations are conquered")),
    ("ProbeTransferTime", Quantity("probe travel time")),
    ("ProjectUnlockChance", Quantity("project unlock chance")),
    ("PublicCampaignStrength", Quantity("Public Campaign impact")),
    ("PublicOpinionInfluence", Quantity("influence from public opinion")),
    ("ResourceMarketSales", Quantity("space resource market revenue")),
    ("STOFighterECM", Quantity("STO fighter ECM")),
    ("SemiMajorAxisExplorationRange_AU", Quantity("orbit-based exploration range")),
    ("ShipConstructionTime", Quantity("ship construction time")),
    ("ShipConvMissileDamage", Quantity("missile damage")),
    ("ShipLaserDamage", Quantity("laser weapon damage")),
    ("ShipMagDamage", Quantity("magnetic weapon damage")),
    ("ShipMissionControlReduction", Quantity("Mission Control cost of ships")),
    ("ShipOfficerPromotion", Quantity("officer promotion chance")),
    ("Ship_MaxSurvivableCombatAcceleration_Bonus", Quantity("max ship combat acceleration")),
    ("Ship_MaxSurvivableCruiseAcceleration_Bonus", Quantity("max ship cruise acceleration")),
    ("SocialScience", Quantity("Social Science research")),
    ("SpaceScience", Quantity("Space Science research")),
    ("SpaceMiningBonus", Quantity("off-Earth mining output")),
    ("SpaceflightPriority", Quantity("Space Program investment")),
    ("SpoilsOutput", Quantity("money from Spoils")),
    ("SpoilsPriority", Quantity("Spoils investment")),
    ("TargetingComputerBonus", Quantity("ECM penetration of habs and surface defenses")),
    ("UnityPriority", Quantity("Unity investment")),
    ("UpgradeArmyPriority", Quantity("Build Navy investment")),
    ("WelfarePriority", Quantity("Welfare investment")),
    ("Welfare_CH4_ppm", Quantity("methane removal")),
    ("Welfare_CO2_ppm", Quantity("CO₂ removal")),
    ("Welfare_N2O_ppm", Quantity("N₂O removal")),
    ("Xenology", Quantity("Xenology research")),
    ("XenoformingDestructionStrength", Quantity("damage to xenoforming")),
];

/// Display names of `Mission_<name>_Att/_Def` contexts that differ from the context name.
const MISSION_NAMES: &[(&str, &str)] = &[
    ("ControlAsset", "Control Space Asset"),
    ("GainInfluence", "Control Nation"),
    ("GainControlPoint", "gain control point"),
    ("SeizeSpaceAsset", "Assault Enemy Space Asset"),
    ("AssaultAlienAsset", "Assault Alien Asset"),
    ("SabotageHabModule", "Sabotage Hab Module"),
];

/// Label of an effect context and its text (a noun phrase for quantities).
pub(crate) fn effect_label(context: &str) -> (Label, Cow<'static, str>) {
    if let Some((_, label)) = EFFECT_LABELS.iter().find(|(c, _)| *c == context) {
        let (Quantity(text) | Inverted(text) | Ability(text)) = *label;
        return (*label, text.into());
    }
    if let Some(rest) = context.strip_prefix("Mission_") {
        let (name, suffix) = rest.rsplit_once('_').unwrap_or((rest, "Att"));
        let name = MISSION_NAMES.iter().find(|(n, _)| *n == name).map_or_else(|| humanize(name), |(_, d)| d.to_string());
        return match suffix {
            "Def" => (Quantity(""), format!("defense against {name} missions").into()),
            _ => (Quantity(""), format!("{name} mission bonus").into()),
        };
    }
    (Quantity(""), humanize(context).to_lowercase().into())
}

/// `Ship_MaxCruiseBonus` → `Ship max cruise bonus` (keeps acronyms).
fn humanize(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            continue;
        }
        let prev = i.checked_sub(1).map(|p| chars[p]);
        let next = chars.get(i + 1);
        let boundary = c.is_uppercase()
            && prev.is_some_and(|p| p.is_lowercase() || (p.is_uppercase() && next.is_some_and(|n| n.is_lowercase())));
        if boundary && !out.ends_with(' ') {
            out.push(' ');
        }
        out.push(c);
    }
    out.trim().to_string()
}

/// Raw direction of an effect's value: does it raise the context value?
pub(crate) fn raises(effect: &Effect) -> bool {
    match effect.operation.as_deref() {
        Some("Multiplicative") => effect.value >= 1.0,
        Some("SetToFixedValue" | "IncreaseToValue") => true,
        Some("DecreaseToValue") => false,
        _ => effect.value >= 0.0,
    }
}

/// Property id, label and labelled direction (None for abilities) for one context of an effect.
fn effect_property(context: &str, effect: &Effect) -> (String, String, Option<bool>) {
    match effect_label(context) {
        (Ability(_), label) => (format!("effect:{context}"), label.into_owned(), None),
        (kind, noun) => {
            let up = raises(effect) != matches!(kind, Inverted(_));
            let (dir, verb) = if up { ("up", "Increases") } else { ("down", "Decreases") };
            (format!("effect:{context}:{dir}"), format!("{verb} {noun}"), Some(up))
        }
    }
}

fn unlock_property(kind: UnlockKind) -> Option<(String, String)> {
    let (id, label) = match kind {
        UnlockKind::HabModule => ("habModule".to_string(), "Unlocks hab modules".to_string()),
        UnlockKind::ShipPart(k) => (format!("shipPart:{}", refs::part_kind_name(k)), format!("Unlocks {}", part_plural(k))),
        UnlockKind::Org => ("org".into(), "Unlocks orgs".into()),
        UnlockKind::GrantedOrg => ("grantedOrg".into(), "Grants an org".into()),
        UnlockKind::Trait => ("trait".into(), "Unlocks traits".into()),
        UnlockKind::Mission => ("mission".into(), "Unlocks missions".into()),
        UnlockKind::Claim => ("claim".into(), "Unlocks territorial claims".into()),
        UnlockKind::Research { .. } | UnlockKind::Codex | UnlockKind::Objective => return None,
    };
    Some((format!("unlock:{id}"), label))
}

fn part_plural(kind: PartKind) -> &'static str {
    match kind {
        PartKind::Hull => "hulls",
        PartKind::Drive => "drives",
        PartKind::PowerPlant => "power plants",
        PartKind::Radiator => "radiators",
        PartKind::HeatSink => "heat sinks",
        PartKind::Battery => "batteries",
        PartKind::Armor => "armor",
        PartKind::Utility => "utility modules",
        PartKind::Laser => "lasers",
        PartKind::MagneticGun => "magnetic guns",
        PartKind::Gun => "guns",
        PartKind::Missile => "missiles",
        PartKind::ParticleWeapon => "particle weapons",
        PartKind::PlasmaWeapon => "plasma weapons",
    }
}

/// One property of a node, with the effect that gave it (for example texts).
struct Found {
    id: String,
    label: String,
    effect: Option<EntityId>,
    /// Effect context, labelled direction (None for abilities) and whether the effect moves
    /// the value in that direction (effects only; for display metadata).
    context: Option<(String, Option<bool>, bool)>,
}

fn find(view: &View<'_>, id: EntityId) -> Vec<Found> {
    let mut out = Vec::new();
    for effect_id in view.effects(id) {
        let Some(effect) = view.effect(effect_id) else { continue };
        for context in &effect.contexts {
            let (id, label, up) = effect_property(context, &effect);
            out.push(Found { id, label, effect: Some(effect_id), context: Some((context.clone(), up, changes(&effect))) });
        }
    }
    for unlock in view.unlocks(id) {
        if let Some((id, label)) = unlock_property(unlock.kind) {
            out.push(Found { id, label, effect: None, context: None });
        }
    }
    out
}

/// Property ids of a tech/project, sorted and deduplicated.
pub fn of(view: &View<'_>, id: EntityId) -> Vec<String> {
    let ids: BTreeSet<String> = find(view, id).into_iter().map(|f| f.id).collect();
    ids.into_iter().collect()
}

/// Every property of `nodes`, with counts; effects first, then unlocks, by label.
pub fn catalog(view: &View<'_>, texts: &Texts, nodes: impl IntoIterator<Item = EntityId>) -> Vec<PropertyInfo> {
    let mut by_id: BTreeMap<String, (PropertyInfo, BTreeSet<EntityId>)> = BTreeMap::new();
    for node in nodes {
        for f in find(view, node) {
            let (info, nodes) = by_id.entry(f.id.clone()).or_insert_with(|| {
                let group = if f.id.starts_with("unlock:") {
                    "unlock"
                } else if f.id.matches(':').count() == 1 {
                    "ability"
                } else {
                    "effect"
                };
                let style = f.context.as_ref().map(|(context, up, moves)| context_style(view, context, up.filter(|_| *moves), up.is_none()));
                let info = PropertyInfo {
                    id: f.id,
                    label: f.label,
                    group: group.into(),
                    count: 0,
                    example: None,
                    sentiment: style.as_ref().map_or("neutral", |s| s.sentiment).into(),
                    domain: style.as_ref().map(|s| s.domain.into()),
                    category: style.as_ref().and_then(|s| s.category.clone()),
                    icon: style.and_then(|s| s.icon),
                };
                (info, BTreeSet::new())
            });
            // Directional sentiment once any effect moves the value (not just fixes a level).
            if let Some((context, up @ Some(_), true)) = &f.context
                && info.sentiment == "neutral"
            {
                info.sentiment = context_style(view, context, *up, false).sentiment.into();
            }
            nodes.insert(node);
            info.count = nodes.len();
            if info.example.is_none() {
                // Drop the game's list dash and gaps left by stripped inline sprites ("The total  cost").
                info.example = f.effect.and_then(|e| effect_text(view, texts, e)).map(|t| t.trim_start_matches('-').split_whitespace().collect::<Vec<_>>().join(" "));
            }
        }
    }
    let group_order = |g: &str| match g {
        "effect" => 0,
        "ability" => 1,
        _ => 2,
    };
    let mut out: Vec<PropertyInfo> = by_id.into_values().map(|(info, _)| info).collect();
    out.sort_by(|a, b| group_order(&a.group).cmp(&group_order(&b.group)).then_with(|| a.label.cmp(&b.label)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanizes_context_names() {
        assert_eq!(humanize("Ship_MaxSurvivableCruiseAcceleration_Bonus"), "Ship Max Survivable Cruise Acceleration Bonus");
        assert_eq!(humanize("HabMCReduction"), "Hab MC Reduction");
        assert_eq!(humanize("EnthrallElites"), "Enthrall Elites");
    }

    #[test]
    fn mission_contexts() {
        assert_eq!(effect_label("Mission_Terrorize_Def").1, "defense against Terrorize missions");
        assert_eq!(effect_label("Mission_GainInfluence_Att").1, "Control Nation mission bonus");
    }
}
