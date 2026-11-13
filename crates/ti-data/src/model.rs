//! Typed views of the most important templates.
//!
//! Every record stays available as raw JSON (`Variant::record`); these structs
//! cover the fields the domain logic relies on. Unknown/missing fields default.

use serde::Deserialize;
use serde_json::Value;

pub const TECH: &str = "TITechTemplate";
pub const PROJECT: &str = "TIProjectTemplate";
pub const EFFECT: &str = "TIEffectTemplate";
pub const HAB_MODULE: &str = "TIHabModuleTemplate";
pub const ORG: &str = "TIOrgTemplate";
pub const TRAIT: &str = "TITraitTemplate";
pub const FACTION: &str = "TIFactionTemplate";
pub const OBJECTIVE: &str = "TIObjectiveTemplate";
pub const MISSION: &str = "TIMissionTemplate";
pub const CODEX: &str = "TICodexEntryTemplate";
pub const START_TIME: &str = "TIStartTimeTemplate";

/// Broad classification of a template, for grouping in UIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Tech,
    Project,
    Effect,
    HabModule,
    ShipPart(PartKind),
    Org,
    Trait,
    Faction,
    Objective,
    Mission,
    Codex,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PartKind {
    Hull,
    Drive,
    PowerPlant,
    Radiator,
    HeatSink,
    Battery,
    Armor,
    Utility,
    Laser,
    MagneticGun,
    Gun,
    Missile,
    ParticleWeapon,
    PlasmaWeapon,
}

impl PartKind {
    pub const ALL: [PartKind; 14] = [
        PartKind::Hull,
        PartKind::Drive,
        PartKind::PowerPlant,
        PartKind::Radiator,
        PartKind::HeatSink,
        PartKind::Battery,
        PartKind::Armor,
        PartKind::Utility,
        PartKind::Laser,
        PartKind::MagneticGun,
        PartKind::Gun,
        PartKind::Missile,
        PartKind::ParticleWeapon,
        PartKind::PlasmaWeapon,
    ];

    pub fn template(self) -> &'static str {
        match self {
            PartKind::Hull => "TIShipHullTemplate",
            PartKind::Drive => "TIDriveTemplate",
            PartKind::PowerPlant => "TIPowerPlantTemplate",
            PartKind::Radiator => "TIRadiatorTemplate",
            PartKind::HeatSink => "TIHeatSinkTemplate",
            PartKind::Battery => "TIBatteryTemplate",
            PartKind::Armor => "TIShipArmorTemplate",
            PartKind::Utility => "TIUtilityModuleTemplate",
            PartKind::Laser => "TILaserWeaponTemplate",
            PartKind::MagneticGun => "TIMagneticGunTemplate",
            PartKind::Gun => "TIGunTemplate",
            PartKind::Missile => "TIMissileTemplate",
            PartKind::ParticleWeapon => "TIParticleWeaponTemplate",
            PartKind::PlasmaWeapon => "TIPlasmaWeaponTemplate",
        }
    }

    pub fn is_weapon(self) -> bool {
        matches!(
            self,
            PartKind::Laser | PartKind::MagneticGun | PartKind::Gun | PartKind::Missile | PartKind::ParticleWeapon | PartKind::PlasmaWeapon
        )
    }
}

impl Kind {
    pub fn of_template(template: &str) -> Kind {
        match template {
            TECH => Kind::Tech,
            PROJECT => Kind::Project,
            EFFECT => Kind::Effect,
            HAB_MODULE => Kind::HabModule,
            ORG => Kind::Org,
            TRAIT => Kind::Trait,
            FACTION => Kind::Faction,
            OBJECTIVE => Kind::Objective,
            MISSION => Kind::Mission,
            CODEX => Kind::Codex,
            t => PartKind::ALL.into_iter().find(|p| p.template() == t).map(Kind::ShipPart).unwrap_or(Kind::Other),
        }
    }

    pub fn is_research(self) -> bool {
        matches!(self, Kind::Tech | Kind::Project)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Default)]
pub enum TechCategory {
    Energy,
    InformationScience,
    LifeScience,
    Materials,
    MilitaryScience,
    SocialScience,
    SpaceScience,
    Xenology,
    #[serde(other)]
    #[default]
    Unknown,
}

impl TechCategory {
    pub const ALL: [TechCategory; 8] = [
        TechCategory::Energy,
        TechCategory::InformationScience,
        TechCategory::LifeScience,
        TechCategory::Materials,
        TechCategory::MilitaryScience,
        TechCategory::SocialScience,
        TechCategory::SpaceScience,
        TechCategory::Xenology,
    ];

    pub fn parse(s: &str) -> Option<TechCategory> {
        TechCategory::deserialize(serde::de::value::StrDeserializer::<serde::de::value::Error>::new(s)).ok()
    }

    /// Resource string of the category icon (`icons_2d` bundle).
    pub fn icon_resource(self) -> Option<&'static str> {
        Some(match self {
            TechCategory::Energy => "icons_2d/tech_energy_icon",
            TechCategory::InformationScience => "icons_2d/tech_info_icon",
            TechCategory::LifeScience => "icons_2d/tech_life_icon",
            TechCategory::Materials => "icons_2d/tech_material_icon",
            TechCategory::MilitaryScience => "icons_2d/tech_military_icon",
            TechCategory::SocialScience => "icons_2d/tech_social_icon",
            TechCategory::SpaceScience => "icons_2d/tech_space_icon",
            TechCategory::Xenology => "icons_2d/tech_xeno_icon",
            TechCategory::Unknown => return None,
        })
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Tech {
    #[serde(rename = "dataName")]
    pub data_name: String,
    #[serde(rename = "friendlyName")]
    pub friendly_name: String,
    #[serde(rename = "techCategory")]
    pub category: TechCategory,
    #[serde(rename = "researchCost")]
    pub research_cost: f64,
    pub prereqs: Vec<String>,
    pub effects: Vec<String>,
    #[serde(rename = "endGameTech")]
    pub end_game_tech: bool,
    #[serde(rename = "AI_techRole")]
    pub ai_tech_role: String,
    #[serde(rename = "AI_criticalTech")]
    pub ai_critical_tech: bool,
    #[serde(rename = "scenarioTags")]
    pub scenario_tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ResourceGrant {
    pub resource: String,
    pub value: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Project {
    #[serde(rename = "dataName")]
    pub data_name: String,
    #[serde(rename = "friendlyName")]
    pub friendly_name: String,
    #[serde(rename = "techCategory")]
    pub category: TechCategory,
    #[serde(rename = "researchCost")]
    pub research_cost: f64,
    pub prereqs: Vec<String>,
    /// `altPrereqN` fields as `(N, dataName)`, sorted by N.
    #[serde(skip)]
    pub alt_prereqs: Vec<(usize, String)>,
    pub effects: Vec<String>,
    pub disable: bool,
    #[serde(rename = "factionPrereq")]
    pub faction_prereq: Vec<String>,
    #[serde(rename = "factionAlways")]
    pub faction_always: Option<String>,
    #[serde(rename = "requiresNation")]
    pub requires_nation: Option<String>,
    #[serde(rename = "requiredObjectiveName")]
    pub required_objective: Option<String>,
    #[serde(rename = "altRequiredObjectiveName")]
    pub alt_required_objective: Option<String>,
    #[serde(rename = "requiredMilestone")]
    pub required_milestone: Option<String>,
    #[serde(rename = "factionAvailableChance")]
    #[serde(default = "hundred")]
    pub faction_available_chance: f64,
    #[serde(rename = "initialUnlockChance")]
    #[serde(default = "hundred")]
    pub initial_unlock_chance: f64,
    #[serde(rename = "deltaUnlockChance")]
    pub delta_unlock_chance: f64,
    #[serde(rename = "maxUnlockChance")]
    #[serde(default = "hundred")]
    pub max_unlock_chance: f64,
    #[serde(rename = "oneTimeGlobally")]
    pub one_time_globally: bool,
    pub repeatable: bool,
    #[serde(rename = "resourcesGranted")]
    pub resources_granted: Vec<ResourceGrant>,
    #[serde(rename = "orgGranted")]
    pub org_granted: Option<String>,
    #[serde(rename = "AI_techRole")]
    pub ai_tech_role: String,
    #[serde(rename = "AI_projectRole")]
    pub ai_project_role: String,
    #[serde(rename = "AI_criticalTech")]
    pub ai_critical_tech: bool,
    #[serde(rename = "scenarioTags")]
    pub scenario_tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Effect {
    #[serde(rename = "dataName")]
    pub data_name: String,
    #[serde(rename = "effectTarget")]
    pub target: String,
    #[serde(rename = "effectSecondaryTarget")]
    pub secondary_target: Option<String>,
    #[serde(rename = "effectDuration")]
    pub duration: String,
    pub duration_months: i64,
    pub operation: Option<String>,
    pub value: f64,
    #[serde(rename = "strValue")]
    pub str_value: Option<Value>,
    pub contexts: Vec<String>,
    #[serde(rename = "instantEffect")]
    pub instant_effect: Option<String>,
    pub stackable: bool,
    #[serde(rename = "localizationAlias")]
    pub localization_alias: Option<String>,
    #[serde(rename = "scenarioTags")]
    pub scenario_tags: Vec<String>,
}

/// Fields shared by all ship parts (hulls, drives, weapons, …).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShipPart {
    #[serde(rename = "dataName")]
    pub data_name: String,
    #[serde(rename = "friendlyName", alias = "displayName")]
    pub friendly_name: String,
    #[serde(rename = "requiredProjectName")]
    pub required_project: Option<String>,
    #[serde(rename = "iconResource")]
    pub icon_resource: Option<String>,
    pub mass_tons: Option<f64>,
    #[serde(rename = "baseWeaponMass_tons")]
    pub base_weapon_mass_tons: Option<f64>,
    pub crew: Option<f64>,
    pub mount: Option<String>,
    #[serde(rename = "weightedBuildMaterials")]
    pub build_materials: Option<serde_json::Map<String, Value>>,
    pub disable: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HabModule {
    #[serde(rename = "dataName")]
    pub data_name: String,
    #[serde(rename = "friendlyName")]
    pub friendly_name: String,
    #[serde(rename = "requiredProjectName")]
    pub required_project: Option<String>,
    #[serde(rename = "upgradesFromName")]
    pub upgrades_from: Option<String>,
    #[serde(rename = "habType")]
    pub hab_type: String,
    pub tier: i64,
    #[serde(rename = "coreModule")]
    pub core_module: bool,
    #[serde(rename = "baseIconResource")]
    pub base_icon_resource: Option<String>,
    #[serde(rename = "stationIconResource")]
    pub station_icon_resource: Option<String>,
    pub disable: bool,
}

fn hundred() -> f64 {
    100.0
}

/// Deserialize a typed view from a raw record. Missing fields default; a record whose
/// fields have unexpected types yields the all-default value rather than an error.
pub fn parse<T: serde::de::DeserializeOwned + Default>(record: &Value) -> T {
    T::deserialize(record).unwrap_or_default()
}

impl Project {
    pub fn from_record(record: &Value) -> Project {
        let mut p: Project = parse(record);
        if let Some(obj) = record.as_object() {
            p.alt_prereqs = obj
                .iter()
                .filter_map(|(k, v)| {
                    let n = k.strip_prefix("altPrereq")?.parse().ok()?;
                    Some((n, v.as_str()?.to_string()))
                })
                .filter(|(_, v)| !v.is_empty())
                .collect();
            p.alt_prereqs.sort();
        }
        p
    }
}
