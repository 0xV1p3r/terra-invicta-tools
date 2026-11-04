//! Stats of hab modules, ship parts and orgs for the detail panel: every field of the
//! record, labelled, formatted and grouped. Known fields come from `FIELDS`; anything
//! else that isn't presentation data (models, sounds, UI paths) lands in "Other".

mod space;
mod wiki;

use std::collections::HashSet;

use serde_json::{Map, Value};
use ti_data::effect_text::format_small_number;
use ti_data::model::{self, Kind, PartKind};
use ti_data::text::to_plain_with;
use ti_data::{EntityId, Texts, View};

use crate::dto::{EntityRef, EntityStats, Stat, StatGroup, StatLinks};
use crate::refs::entity_ref;

#[derive(Clone, Copy, PartialEq)]
enum Fmt {
    /// Number with a unit suffix ("" for none).
    Num(&'static str),
    /// Number with sign, unit suffix.
    Signed(&'static str),
    /// Fraction shown as a percentage.
    Pct,
    /// Fraction shown as a signed percentage.
    SignedPct,
    /// Enum value: localized via `<prefix><value>` when the game has the string.
    Enum(&'static str),
    /// Enum value shown as a tag (localized like `Enum`).
    TagEnum(&'static str),
    /// `true` adds this tag.
    Tag(&'static str),
    /// `true` adds this game string (key) as a note.
    Note(&'static str),
    /// Data that the special handling in `entity_stats` consumes.
    Skip,
}

/// Which direction is good for the owning faction.
#[derive(Clone, Copy, PartialEq)]
enum Tone {
    Neutral,
    Up,
    Down,
}

struct Field {
    field: &'static str,
    label: &'static str,
    fmt: Fmt,
    group: &'static str,
    tone: Tone,
    /// `sprite:<name>` or an asset path.
    icon: Option<&'static str>,
    /// Shown even when zero.
    always: bool,
}

const fn f(field: &'static str, label: &'static str, fmt: Fmt, group: &'static str) -> Field {
    Field { field, label, fmt, group, tone: Tone::Neutral, icon: None, always: false }
}

impl Field {
    const fn up(mut self) -> Self {
        self.tone = Tone::Up;
        self
    }
    const fn down(mut self) -> Self {
        self.tone = Tone::Down;
        self
    }
    const fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }
    const fn always(mut self) -> Self {
        self.always = true;
        self
    }
}

const GENERAL: &str = "General";
const COMBAT: &str = "Combat";
const PROPULSION: &str = "Propulsion";
const POWER: &str = "Power & heat";
const STRUCTURE: &str = "Structure";
const INCOME: &str = "Income / month";
const ATTRIBUTES: &str = "Councilor attributes";
const PRIORITIES: &str = "Bonuses";
const ACQUIRE: &str = "Acquisition cost";
const MISSILE: &str = "Missile";
const OTHER: &str = "Other";

/// Known fields of all three kinds, in display order (a group appears where its first field is).
const FIELDS: &[Field] = &[
    // Tags
    f("tier", "Tier", Fmt::Skip, GENERAL),
    f("habType", "", Fmt::Skip, GENERAL),
    f("orgType", "", Fmt::TagEnum(""), GENERAL),
    f("driveClassification", "", Fmt::TagEnum("TIDriveTemplate.Class."), GENERAL),
    f("powerPlantClass", "", Fmt::TagEnum("TIPowerPlantTemplate.PowerPlantRequirement."), GENERAL),
    f("radiatorType", "", Fmt::TagEnum(""), GENERAL),
    f("warheadClass", "", Fmt::TagEnum("TIMissileTemplate.WarheadType."), GENERAL),
    f("dispersionModel", "", Fmt::TagEnum(""), GENERAL),
    f("coreModule", "", Fmt::Tag("Core module"), GENERAL),
    f("automated", "", Fmt::Tag("Automated"), GENERAL),
    f("mine", "", Fmt::Tag("Mine"), GENERAL),
    f("allowsShipConstruction", "", Fmt::Tag("Shipyard"), GENERAL),
    f("allowsResupply", "", Fmt::Tag("Resupply"), GENERAL),
    f("spaceCombatModule", "", Fmt::Tag("Combat module"), GENERAL),
    f("objectiveModule", "", Fmt::Tag("Objective"), GENERAL),
    f("alienModule", "", Fmt::Tag("Alien"), GENERAL),
    f("alien", "", Fmt::Tag("Alien"), GENERAL),
    f("destroyed", "", Fmt::Tag("Ruined"), GENERAL),
    f("noBuild", "", Fmt::Tag("Not buildable"), GENERAL),
    f("disable", "", Fmt::Tag("Disabled"), GENERAL),
    f("simpleHull", "", Fmt::Tag("Simple hull"), GENERAL),
    f("noShipyardBuild", "", Fmt::Tag("No shipyard build"), GENERAL),
    f("attackMode", "", Fmt::Tag("Offensive"), GENERAL),
    f("defenseMode", "", Fmt::Tag("Point defense"), GENERAL),
    f("isPointDefenseTargetable", "", Fmt::Tag("Interceptable"), GENERAL),
    f("collector", "", Fmt::Tag("Solar collector"), GENERAL),
    f("noCombatRepair", "", Fmt::Tag("No combat repair"), GENERAL),
    f("randomized", "", Fmt::Tag("Randomized"), GENERAL),
    f("requiresNationality", "", Fmt::Tag("Requires nationality"), GENERAL),
    // Notes
    f("onePerHab", "", Fmt::Note("TIHabModuleTemplate.OnePerHab"), GENERAL),
    f("freeISRU", "", Fmt::Note("TIDriveTemplate.ISRU"), GENERAL),
    f("helium3Fuel", "", Fmt::Note("TIDriveTemplate.UsesHelium3"), GENERAL),
    // General
    f("baseMass_tons", "Mass", Fmt::Num(" t"), GENERAL).icon("sprite:mass").always(),
    f("mass_tons", "Mass", Fmt::Num(" t"), GENERAL).icon("sprite:mass").always(),
    f("flatMass_tons", "Mass", Fmt::Num(" t"), GENERAL).icon("sprite:mass"),
    f("baseWeaponMass_tons", "Mass", Fmt::Num(" t"), GENERAL).icon("sprite:mass").always(),
    f("crew", "Crew", Fmt::Num(""), GENERAL).icon("sprite:population").always(),
    f("power", "Power", Fmt::Signed(" MW"), GENERAL).up().icon("sprite:hab_power").always(),
    f("powerRequirement_MW", "Power required", Fmt::Num(" MW"), GENERAL).down().icon("sprite:hab_power"),
    f("missionControl", "Mission control", Fmt::Signed(""), GENERAL).up().icon("sprite:mission_control"),
    f("controlPointCapacity", "Control point capacity", Fmt::Signed(""), GENERAL).up().icon("sprite:control_point"),
    f("buildTime_Days", "Build time", Fmt::Num(" days"), GENERAL).down().icon("sprite:alarmClock"),
    f("baseConstructionTime_days", "Build time", Fmt::Num(" days"), GENERAL).down().icon("sprite:alarmClock"),
    f("constructionTimeModifier", "Construction time", Fmt::Pct, GENERAL).down(),
    f("miningModifier", "Mining output", Fmt::Pct, GENERAL).up().icon("sprite:mining"),
    f("consTier", "Construction tier", Fmt::Num(""), GENERAL),
    f("minConsTier", "Min. hull construction tier", Fmt::Num(""), GENERAL),
    f("maxOfficers", "Officers", Fmt::Num(""), GENERAL),
    f("monthlyIncome_Money", "Upkeep / month", Fmt::Signed(""), GENERAL).up().icon("sprite:currency"),
    f("specialModuleValue", "Rule value", Fmt::Num(""), GENERAL),
    f("specialRulesValue", "", Fmt::Skip, GENERAL),
    f("hp", "Hit points", Fmt::Num(""), GENERAL).up(),
    f("minScenarioYear", "Available from", Fmt::Num(""), GENERAL),
    f("takeoverDefense", "Takeover defense", Fmt::Signed(""), GENERAL).up(),
    f("projectsGranted", "Projects granted", Fmt::Num(""), GENERAL).up().icon("sprite:projects"),
    // Hab income
    f("incomeMoney_month", "Money", Fmt::Signed(""), INCOME).up().icon("sprite:currency"),
    f("incomeInfluence_month", "Influence", Fmt::Signed(""), INCOME).up().icon("sprite:influence"),
    f("incomeOps_month", "Operations", Fmt::Signed(""), INCOME).up().icon("sprite:ops"),
    f("incomeResearch_month", "Research", Fmt::Signed(""), INCOME).up().icon("sprite:research"),
    f("incomeProjects", "Project slots", Fmt::Signed(""), INCOME).up().icon("sprite:projects"),
    f("incomeMetals_month", "Metals", Fmt::Signed(""), INCOME).up().icon("sprite:metal"),
    f("incomeNobles_month", "Noble metals", Fmt::Signed(""), INCOME).up().icon("sprite:metal_noble"),
    f("incomeVolatiles_month", "Volatiles", Fmt::Signed(""), INCOME).up().icon("sprite:volatiles"),
    f("incomeFissiles_month", "Fissiles", Fmt::Signed(""), INCOME).up().icon("sprite:radioactive"),
    f("incomeAntimatter_month", "Antimatter", Fmt::Signed(""), INCOME).up().icon("sprite:antimatter"),
    f("incomeExotics_month", "Exotics", Fmt::Signed(""), INCOME).up().icon("sprite:exotics"),
    // Org income
    f("incomeMoney", "Money", Fmt::Signed(""), INCOME).up().icon("sprite:currency"),
    f("incomeInfluence", "Influence", Fmt::Signed(""), INCOME).up().icon("sprite:influence"),
    f("incomeOps", "Operations", Fmt::Signed(""), INCOME).up().icon("sprite:ops"),
    f("incomeBoost", "Boost", Fmt::Signed(""), INCOME).up().icon("sprite:boost"),
    f("incomeMissionControl", "Mission control", Fmt::Signed(""), INCOME).up().icon("sprite:mission_control"),
    f("incomeResearch", "Research", Fmt::Signed(""), INCOME).up().icon("sprite:research"),
    // Org attributes
    f("persuasion", "Persuasion", Fmt::Signed(""), ATTRIBUTES).up().icon("sprite:attribute_persuasion"),
    f("investigation", "Investigation", Fmt::Signed(""), ATTRIBUTES).up().icon("sprite:attribute_investigation"),
    f("espionage", "Espionage", Fmt::Signed(""), ATTRIBUTES).up().icon("sprite:attribute_espionage"),
    f("command", "Command", Fmt::Signed(""), ATTRIBUTES).up().icon("sprite:attribute_command"),
    f("administration", "Administration", Fmt::Signed(""), ATTRIBUTES).up().icon("sprite:attribute_administration"),
    f("science", "Science", Fmt::Signed(""), ATTRIBUTES).up().icon("sprite:attribute_science"),
    f("security", "Security", Fmt::Signed(""), ATTRIBUTES).up().icon("sprite:attribute_security"),
    // Org bonuses
    f("economyBonus", "Economy priority", Fmt::SignedPct, PRIORITIES).up().icon("sprite:economy"),
    f("welfareBonus", "Welfare priority", Fmt::SignedPct, PRIORITIES).up().icon("sprite:welfare"),
    f("environmentBonus", "Environment priority", Fmt::SignedPct, PRIORITIES).up().icon("sprite:environment"),
    f("knowledgeBonus", "Knowledge priority", Fmt::SignedPct, PRIORITIES).up().icon("sprite:knowledge"),
    f("governmentBonus", "Government priority", Fmt::SignedPct, PRIORITIES).up().icon("sprite:gov_type"),
    f("unityBonus", "Unity priority", Fmt::SignedPct, PRIORITIES).up().icon("sprite:unity"),
    f("militaryBonus", "Military priority", Fmt::SignedPct, PRIORITIES).up().icon("sprite:build_military"),
    f("oppressionBonus", "Oppression priority", Fmt::SignedPct, PRIORITIES).up().icon("sprite:oppression"),
    f("spaceDevBonus", "Space development priority", Fmt::SignedPct, PRIORITIES).up().icon("sprite:space_launch"),
    f("spaceflightBonus", "Spaceflight priority", Fmt::SignedPct, PRIORITIES).up().icon("sprite:spaceflight_program"),
    f("MCBonus", "Mission control priority", Fmt::SignedPct, PRIORITIES).up().icon("sprite:mission_control"),
    f("miningBonus", "Space mining output", Fmt::SignedPct, PRIORITIES).up().icon("sprite:mining"),
    f("spoilsBonus", "Spoils", Fmt::SignedPct, PRIORITIES).up().icon("sprite:spoils"),
    // Org cost
    f("costMoney", "Money", Fmt::Num(""), ACQUIRE).icon("sprite:currency"),
    f("costInfluence", "Influence", Fmt::Num(""), ACQUIRE).icon("sprite:influence"),
    f("costOps", "Operations", Fmt::Num(""), ACQUIRE).icon("sprite:ops"),
    f("costBoost", "Boost", Fmt::Num(""), ACQUIRE).icon("sprite:boost"),
    // Weapons
    f("mount", "Mount", Fmt::Enum(""), COMBAT),
    f("damage_MJ", "Damage", Fmt::Num(" MJ"), COMBAT).up().icon("sprite:ship_damage"),
    f("expectedDamage_MJ", "Expected damage", Fmt::Num(" MJ"), COMBAT).up().icon("sprite:ship_damage"),
    f("flatDamage_MJ", "Damage", Fmt::Num(" MJ"), COMBAT).up().icon("sprite:ship_damage"),
    f("shotPower_MJ", "Shot power", Fmt::Num(" MJ"), COMBAT).up().icon("sprite:ship_damage"),
    f("chargingEnergy_GJ", "Charging energy", Fmt::Num(" GJ"), COMBAT).down(),
    f("targetingRange_km", "Targeting range", Fmt::Num(" km"), COMBAT).up(),
    f("cooldown_s", "Cooldown", Fmt::Num(" s"), COMBAT).down(),
    f("salvo_shots", "Salvo", Fmt::Num(" shots"), COMBAT).up(),
    f("intraSalvoCooldown_s", "Time between shots", Fmt::Num(" s"), COMBAT).down(),
    f("magazine", "Magazine", Fmt::Num(""), COMBAT).up(),
    f("muzzleVelocity_kps", "Muzzle velocity", Fmt::Num(" km/s"), COMBAT).up(),
    f("warheadMass_kg", "Warhead mass", Fmt::Num(" kg"), COMBAT),
    f("ammoMass_kg", "Ammo mass", Fmt::Num(" kg"), COMBAT),
    f("flatChipping", "Armor chipping", Fmt::Pct, COMBAT).up(),
    f("pivotRange_deg", "Pivot range", Fmt::Num("°"), COMBAT).up(),
    f("bombardmentValue", "Bombardment", Fmt::Num(""), COMBAT).up(),
    f("wavelength_nm", "Wavelength", Fmt::Num(" nm"), COMBAT),
    f("mirrorRadius_cm", "Mirror radius", Fmt::Num(" cm"), COMBAT),
    f("beam_quality", "Beam quality (M²)", Fmt::Num(""), COMBAT).down(),
    f("jitter_Rad", "Jitter", Fmt::Num(" rad"), COMBAT).down(),
    f("lensRadius_cm", "Lens radius", Fmt::Num(" cm"), COMBAT),
    f("emittance_mrad", "Emittance", Fmt::Num(" mrad"), COMBAT).down(),
    f("doublingRange_km", "Spot doubling range", Fmt::Num(" km"), COMBAT).up(),
    f("heatFraction", "Thermal damage", Fmt::Num("×"), COMBAT),
    f("xRayFraction", "X-ray damage", Fmt::Num("×"), COMBAT),
    f("baryonFraction", "Baryonic damage", Fmt::Num("×"), COMBAT),
    // Missiles
    f("deltaV_kps", "Delta-v", Fmt::Num(" km/s"), MISSILE).up().icon("sprite:delta_v"),
    f("acceleration_g", "Acceleration", Fmt::Num(" g"), MISSILE).up(),
    f("EV_kps", "Exhaust velocity", Fmt::Num(" km/s"), PROPULSION).up(),
    f("Rocket Thrust", "Thrust", Fmt::Num(" N"), MISSILE).up(),
    f("fuelMass_kg", "Fuel mass", Fmt::Num(" kg"), MISSILE),
    f("systemMass_kg", "System mass", Fmt::Num(" kg"), MISSILE),
    f("rotation_degps", "Turn rate", Fmt::Num("°/s"), MISSILE).up(),
    f("maneuver_angle", "Maneuver angle", Fmt::Num("°"), MISSILE).up(),
    f("thrustRamp_s", "Thrust ramp-up", Fmt::Num(" s"), MISSILE).down(),
    f("turnRamp_s", "Turn ramp-up", Fmt::Num(" s"), MISSILE).down(),
    f("shapedChargeAngle", "Destruction cone", Fmt::Num("°"), MISSILE),
    // Drives
    f("thrust_N", "Thrust", Fmt::Num(" N"), PROPULSION).up(),
    f("thrustRating_GW", "Thrust rating", Fmt::Num(" GW"), PROPULSION),
    f("thrusters", "Thrusters", Fmt::Num(""), PROPULSION),
    f("thrustCap", "Combat thrust", Fmt::Num("×"), PROPULSION).up(),
    f("propellant", "Propellant", Fmt::Enum("TIDriveTemplate."), PROPULSION),
    f("requiredPowerPlant", "Required power plant", Fmt::Enum("TIPowerPlantTemplate.PowerPlantRequirement."), PROPULSION),
    f("req power", "Required power", Fmt::Num(" GW"), PROPULSION),
    f("cooling", "Cooling", Fmt::Skip, PROPULSION),
    f("powerGen", "Backup power", Fmt::Enum("TIDriveTemplate.PowerGenerationType."), PROPULSION),
    // Power & heat
    f("maxOutput_GW", "Max output", Fmt::Num(" GW"), POWER).up(),
    f("specificPower_tGW", "Specific power", Fmt::Num(" t/GW"), POWER).down(),
    f("specificPower_kgMW", "Specific power", Fmt::Num(" kg/MW"), POWER).down(),
    f("efficiency", "Efficiency", Fmt::Pct, POWER).up(),
    f("energyCapacity_GJ", "Capacity", Fmt::Num(" GJ"), POWER).up(),
    f("rechargeRate_GJs", "Recharge rate", Fmt::Num(" GJ/s"), POWER).up(),
    f("heatCapacity_GJ", "Heat capacity", Fmt::Num(" GJ"), POWER).up(),
    f("operatingTemp_K", "Operating temperature", Fmt::Num(" K"), POWER),
    f("emissivity", "Emissivity", Fmt::Num(""), POWER).up(),
    f("specificMass_2s_kgm2", "Specific mass", Fmt::Num(" kg/m²"), POWER).down(),
    f("specificPower_2s_KWkg", "Specific power", Fmt::Num(" kW/kg"), POWER).up(),
    f("vulnerability", "Vulnerability when deployed", Fmt::Num(""), POWER).down(),
    // Hull / armor
    f("structuralIntegrity", "Structural integrity", Fmt::Num(""), STRUCTURE).up(),
    f("noseHardpoints", "Nose hardpoints", Fmt::Num(""), STRUCTURE).always(),
    f("hullHardpoints", "Hull hardpoints", Fmt::Num(""), STRUCTURE).always(),
    f("internalModules", "Utility slots", Fmt::Num(""), STRUCTURE).always(),
    f("thrusterMultiplier", "Thruster multiplier", Fmt::Num("×"), STRUCTURE),
    f("length_m", "Length", Fmt::Num(" m"), STRUCTURE),
    f("width_m", "Width", Fmt::Num(" m"), STRUCTURE),
    f("volume", "Volume", Fmt::Num(" m³"), STRUCTURE),
    f("density_kgm3", "Density", Fmt::Num(" kg/m³"), STRUCTURE),
    f("heatofVaporization_MJkg", "Heat of vaporization", Fmt::Num(" MJ/kg"), STRUCTURE).up(),
    f("xRayHalfValue_cm", "X-ray half-value thickness", Fmt::Num(" cm"), STRUCTURE).down(),
    f("baryonicHalfValue_cm", "Baryonic half-value thickness", Fmt::Num(" cm"), STRUCTURE).down(),
    // Consumed by special handling
    f("weightedBuildMaterials", "", Fmt::Skip, GENERAL),
    f("weightBuildMaterials", "", Fmt::Skip, GENERAL),
    f("supportMaterials_month", "", Fmt::Skip, GENERAL),
    f("perTankPropellantMaterials", "", Fmt::Skip, GENERAL),
    f("ammoMaterials", "", Fmt::Skip, GENERAL),
    f("techBonuses", "", Fmt::Skip, GENERAL),
    f("specialRules", "", Fmt::Skip, GENERAL),
    f("specialModuleRules", "", Fmt::Skip, GENERAL),
    f("specialties", "", Fmt::Skip, GENERAL),
    f("shipModuleSlots", "", Fmt::Skip, GENERAL),
    f("missionsGrantedNames", "", Fmt::Skip, GENERAL),
    f("requiredTechName", "", Fmt::Skip, GENERAL),
    f("upgradesFromName", "", Fmt::Skip, GENERAL),
    f("unlocksProjectName", "", Fmt::Skip, GENERAL),
    f("projectGrantedName", "", Fmt::Skip, GENERAL),
    f("requiredOwnerTraits", "", Fmt::Skip, GENERAL),
    f("prohibitedOwnerTraits", "", Fmt::Skip, GENERAL),
    f("affinities", "", Fmt::Skip, GENERAL),
    f("restricted", "", Fmt::Skip, GENERAL),
    f("homeRegionMapTemplateName", "", Fmt::Skip, GENERAL),
    f("grantsMarked", "", Fmt::Skip, GENERAL),
    f("allowedOnMarket", "", Fmt::Skip, GENERAL),
];

/// Hulls consume mission control; hab modules provide (+) or consume (−) it.
const HULL_MISSION_CONTROL: Field = f("missionControl", "Mission control use", Fmt::Num(""), GENERAL).down().icon("sprite:mission_control");

/// Identity, unlock and presentation fields: never stats.
const IGNORED: &[&str] = &[
    "dataName",
    "friendlyName",
    "displayName",
    "requiredProjectName",
    "notes",
    "sort",
    "sortOrder",
    "grouping",
    "alertWorthy",
    "combatUIpath",
    "path1",
    "path2",
    "shipyardyOffset",
    "toylength_cm",
];

fn is_presentation(field: &str) -> bool {
    IGNORED.contains(&field) || ["Resource", "FX", "Model"].iter().any(|p| field.contains(p))
}

/// Material keys of `weightedBuildMaterials` & co.: label and sprite.
const MATERIALS: &[(&str, &str, &str)] = &[
    ("money", "Money", "currency"),
    ("influence", "Influence", "influence"),
    ("ops", "Operations", "ops"),
    ("boost", "Boost", "boost"),
    ("water", "Water", "water"),
    ("volatiles", "Volatiles", "volatiles"),
    ("metals", "Metals", "metal"),
    ("nobleMetals", "Noble metals", "metal_noble"),
    ("fissiles", "Fissiles", "radioactive"),
    ("antimatter", "Antimatter", "antimatter"),
    ("exotics", "Exotics", "exotics"),
];

/// Game constants behind costs: `TIGlobalConfig` defaults in the game code (this build's
/// config file doesn't override them).
const RESOURCE_PER_TON: f64 = 0.1; // spaceResourceToTons
const CREW_WATER_TONS_YEAR: f64 = 3.5;
const CREW_VOLATILES_TONS_YEAR: f64 = 3.5;
const CREW_SALARY_YEAR: f64 = 0.1;
/// Propellant tanks hold 100 t.
const TANK_TONS: f64 = 100.0;

/// Structured stats for wiki entities; research and effects have dedicated panels.
pub fn entity_stats(view: &View<'_>, texts: &Texts, id: EntityId, spoilers: &HashSet<EntityId>) -> Option<EntityStats> {
    if matches!(
        view.dataset().entity(id).template.as_ref(),
        ti_data::space::SPACE_BODY | ti_data::space::HAB_SITE | ti_data::space::ORBIT | ti_data::space::MINING_PROFILE
    ) {
        return space::stats(view, texts, id, spoilers);
    }
    let kind = view.kind(id);
    if matches!(kind, Kind::Tech | Kind::Project | Kind::Effect) {
        return None;
    }
    if !matches!(kind, Kind::HabModule | Kind::ShipPart(_) | Kind::Org) {
        return wiki::stats(view, texts, id, spoilers);
    }
    let record = view.record(id)?.as_object()?;
    let mut b = Builder { view, texts, spoilers, stats: EntityStats { tags: Vec::new(), groups: Vec::new(), notes: Vec::new(), links: Vec::new() } };
    let mut consumed: HashSet<String> = HashSet::new();

    if let Some(tier) = record.get("tier").and_then(Value::as_f64) {
        b.stats.tags.push(format!("Tier {}", num(tier)));
    }
    if let Some(hab_type) = record.get("habType").and_then(Value::as_str) {
        b.stats.tags.push(match hab_type {
            "Station" => "Station only".into(),
            "Base" => "Base only".into(),
            "Any" => "Station or base".into(),
            t => humanize(t),
        });
    }

    for spec in FIELDS {
        consumed.insert(spec.field.into());
        let Some(value) = record.get(spec.field) else {
            continue;
        };
        let spec = if spec.field == "missionControl" && matches!(kind, Kind::ShipPart(_)) { &HULL_MISSION_CONTROL } else { spec };
        b.field(spec, value, record);
        if !matches!(spec.fmt, Fmt::Skip) {
            consumed.insert(prefixed("rand", spec.field));
            consumed.insert(prefixed("chance", spec.field));
        }
    }

    b.costs(kind, record);
    b.tech_bonuses(record.get("techBonuses"));
    b.hab_rules(record);
    b.module_rules(record);
    b.armor(record.get("specialties"));
    b.slots(record.get("shipModuleSlots"));
    b.drive_notes(record);
    b.org_notes(record);
    b.links(record);

    // Anything else is a stat too.
    for (field, value) in record {
        if !consumed.contains(field.as_str()) && !is_presentation(field) {
            b.other(field, value);
        }
    }
    Some(b.stats)
}

/// `rand`/`chance` companion field name of an org stat (`incomeMoney` → `randIncomeMoney`).
fn prefixed(prefix: &str, field: &str) -> String {
    let mut chars = field.chars();
    let first = chars.next().map(|c| c.to_ascii_uppercase().to_string()).unwrap_or_default();
    format!("{prefix}{first}{}", chars.as_str())
}

struct Builder<'a, 'd> {
    view: &'a View<'d>,
    texts: &'a Texts,
    spoilers: &'a HashSet<EntityId>,
    stats: EntityStats,
}

impl Builder<'_, '_> {
    fn push(&mut self, group: &str, stat: Stat) {
        match self.stats.groups.iter_mut().find(|g| g.label == group) {
            Some(g) => g.stats.push(stat),
            None => self.stats.groups.push(StatGroup { label: group.into(), stats: vec![stat] }),
        }
    }

    fn icon(&self, icon: &str) -> Option<String> {
        match icon.strip_prefix("sprite:") {
            Some(name) => self.view.dataset().sprite(name).map(|a| a.file.clone()),
            None => Some(icon.to_string()),
        }
    }

    /// Localized enum value: `<prefix><value>`, else the humanized value.
    fn enum_text(&self, prefix: &str, value: &str) -> String {
        if !prefix.is_empty()
            && let Some(t) = self.texts.get(&format!("{prefix}{value}")).filter(|t| !t.trim().is_empty())
        {
            return plain(t);
        }
        humanize(value)
    }

    fn field(&mut self, spec: &Field, value: &Value, record: &Map<String, Value>) {
        match spec.fmt {
            Fmt::Skip => {}
            Fmt::Tag(label) => {
                if value.as_bool() == Some(true) && !self.stats.tags.iter().any(|t| t == label) {
                    self.stats.tags.push(label.into());
                }
            }
            Fmt::Note(key) => {
                if value.as_bool() == Some(true)
                    && let Some(t) = self.texts.get(key)
                {
                    self.stats.notes.push(plain(t));
                }
            }
            Fmt::TagEnum(prefix) => {
                if let Some(s) = value.as_str().filter(|s| !s.is_empty()) {
                    self.stats.tags.push(self.enum_text(prefix, s));
                }
            }
            Fmt::Enum(prefix) => {
                let Some(s) = value.as_str().filter(|s| !s.is_empty()) else {
                    return;
                };
                // Some numbers are stored as strings ("req power": "0.290").
                let text = match s.parse::<f64>() {
                    Ok(_) => return self.number(spec, value, record),
                    Err(_) => self.enum_text(prefix, s),
                };
                let stat = self.stat(spec, text, Tone::Neutral, 0.0, None);
                self.push(spec.group, stat);
            }
            Fmt::Num(_) | Fmt::Signed(_) | Fmt::Pct | Fmt::SignedPct => self.number(spec, value, record),
        }
    }

    fn number(&mut self, spec: &Field, value: &Value, record: &Map<String, Value>) {
        let Some(v) = value.as_f64().or_else(|| value.as_str().and_then(|s| s.trim().parse().ok())) else {
            return;
        };
        let rand = record.get(&prefixed("rand", spec.field)).and_then(Value::as_f64).filter(|r| *r != 0.0);
        let chance = record.get(&prefixed("chance", spec.field)).and_then(Value::as_f64).filter(|c| *c < 100.0);
        if v == 0.0 && !spec.always && rand.is_none() {
            return;
        }
        let detail = match (rand, chance) {
            (None, None) => None,
            (rand, chance) => Some(
                [rand.map(|r| format!("+0–{} random", format_value(spec.fmt, r).trim_start_matches('+'))), chance.map(|c| format!("{}% chance", num(c)))]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · "),
            ),
        };
        let stat = self.stat(spec, format_value(spec.fmt, v), spec.tone, v, detail);
        self.push(spec.group, stat);
    }

    fn stat(&self, spec: &Field, value: String, tone: Tone, v: f64, detail: Option<String>) -> Stat {
        Stat {
            label: spec.label.into(),
            value,
            detail,
            sentiment: sentiment(tone, v).into(),
            icon: spec.icon.and_then(|i| self.icon(i)),
            field: spec.field.into(),
        }
    }

    /// A field outside `FIELDS`: numbers as stats, `true` as a tag, strings as text.
    fn other(&mut self, field: &str, value: &Value) {
        let label = humanize(field);
        let text = match value {
            Value::Number(n) => match n.as_f64() {
                Some(v) if v != 0.0 => num(v),
                _ => return,
            },
            Value::String(s) if !s.is_empty() => humanize(s),
            Value::Bool(true) => {
                if !self.stats.tags.contains(&label) {
                    self.stats.tags.push(label);
                }
                return;
            }
            _ => return,
        };
        let stat = Stat { label, value: text, detail: None, sentiment: "neutral".into(), icon: None, field: field.into() };
        self.push(OTHER, stat);
    }

    /// Resource costs as the game computes them: `mass × share × spaceResourceToTons`
    /// (TIHabModuleTemplate.BuildMaterials, TIShipModuleTemplate.buildCost & co.). Parts sized
    /// by the ship design cost per GW or per ton; shares remain visible in each stat's detail.
    fn costs(&mut self, kind: Kind, record: &Map<String, Value>) {
        let n = |f: &str| record.get(f).and_then(|v| v.as_f64().or_else(|| v.as_str()?.trim().parse().ok()));
        let map = |f: &str| record.get(f).and_then(Value::as_object);
        let rules: Vec<&str> = record.get("specialRules").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
        // With an active helium-3 mine, fissiles in these materials are replaced by water
        // (TIHabModuleTemplate.BuildMaterials/MonthlySupportCost, TIDriveTemplate.GetPerTankPropellantMaterials).
        let he3 = rules.contains(&"UsesHelium3") || record.get("helium3Fuel").and_then(Value::as_bool) == Some(true);
        let tons = match kind {
            Kind::HabModule => n("baseMass_tons").map(|t| (t, "Base build cost (in space)")),
            Kind::ShipPart(PartKind::PowerPlant) => n("specificPower_tGW").map(|t| (t, "Build cost per GW")),
            Kind::ShipPart(PartKind::Radiator) => n("specificPower_2s_KWkg").filter(|p| *p > 0.0).map(|p| (1000.0 / p, "Build cost per GW waste heat")),
            Kind::ShipPart(PartKind::Armor) => Some((1.0, "Build cost per ton")),
            Kind::ShipPart(PartKind::Drive) => {
                // TIDriveTemplate.buildMass_tons: flatMass + thrustPower_GW × specificPower_kgMW.
                let thrust_power_gw = n("thrust_N").unwrap_or(0.0) * n("EV_kps").unwrap_or(0.0) * 0.5 / 1e6;
                Some((n("flatMass_tons").unwrap_or(0.0) + thrust_power_gw * n("specificPower_kgMW").unwrap_or(0.0), "Build cost"))
            }
            Kind::ShipPart(_) => n("mass_tons").or_else(|| n("baseWeaponMass_tons")).map(|t| (t, "Build cost")),
            _ => None,
        };
        let build_field = if record.contains_key("weightedBuildMaterials") { "weightedBuildMaterials" } else { "weightBuildMaterials" };
        if let Some(materials) = map(build_field) {
            match tons {
                Some((t, label)) if t > 0.0 => self.cost_group(label, build_field, materials, t, kind == Kind::HabModule && he3),
                _ => self.share_group("Build materials (share)", build_field, materials),
            }
        }
        if kind == Kind::HabModule {
            self.hab_cost_notes(&rules, record.get("mine").and_then(Value::as_bool) == Some(true));
        }
        if let (Some(materials), Some(magazine), Some(ammo_kg)) = (map("ammoMaterials"), n("magazine"), n("ammoMass_kg")) {
            let t = magazine * ammo_kg / 1000.0;
            if t > 0.0 {
                self.cost_group("Full magazine cost", "ammoMaterials", materials, t, false);
            }
        }
        if let Some(materials) = map("perTankPropellantMaterials") {
            self.cost_group("Propellant cost per tank (100 t)", "perTankPropellantMaterials", materials, TANK_TONS, he3);
        }
        // TIHabModuleTemplate.MonthlySupportCost: listed support plus crew upkeep. Crew pay no
        // salary in `Stability` modules or for alien owners (MonthlyCrewSupportCost).
        if kind == Kind::HabModule {
            let crew = n("crew").unwrap_or(0.0);
            let alien = record.get("alienModule").and_then(Value::as_bool) == Some(true);
            let paid = !rules.contains(&"Stability") && !alien;
            let crew_cost = [
                ("money", if paid { crew * CREW_SALARY_YEAR / 12.0 } else { 0.0 }),
                ("water", crew * CREW_WATER_TONS_YEAR * RESOURCE_PER_TON / 12.0),
                ("volatiles", crew * CREW_VOLATILES_TONS_YEAR * RESOURCE_PER_TON / 12.0),
            ];
            let empty = Map::new();
            let support = map("supportMaterials_month").unwrap_or(&empty);
            self.upkeep(support, &crew_cost, he3);
        }
    }

    /// What `TIHabModuleTemplate.Mass_tons` scales the base build cost by at a real site.
    fn hab_cost_notes(&mut self, rules: &[&str], mine: bool) {
        let mut factors = vec!["radiation at irradiated sites"];
        if rules.contains(&"Cost_Scales_With_Gravity") {
            factors.push("the body's gravity and distance (50–100%+ of base)");
        }
        if rules.contains(&"SolarMirror") {
            factors.push("the square of the orbit's distance from the Sun in AU");
        }
        if mine {
            factors.push("the faction's mine size modifier");
        }
        self.stats.notes.push(format!(
            "Base build cost uses the listed mass; at a real site it scales with {}. Upgrading a module costs 2/3.",
            factors.join(", ")
        ));
    }

    /// One stat per material: `share × tons × spaceResourceToTons`.
    fn cost_group(&mut self, group: &str, field: &str, materials: &Map<String, Value>, tons: f64, he3: bool) {
        let amounts: Vec<(String, f64, Option<String>)> = materials
            .iter()
            .filter_map(|(key, share)| {
                let share = share.as_f64().filter(|v| *v > 0.0)?;
                Some((key.clone(), share * tons * RESOURCE_PER_TON, Some(format!("{} of {} t", pct(share), amount(tons)))))
            })
            .collect();
        self.render_amounts(group, field, amounts, he3, Tone::Neutral);
    }

    fn share_group(&mut self, group: &str, field: &str, materials: &Map<String, Value>) {
        for (key, share) in materials {
            let Some(share) = share.as_f64().filter(|v| *v > 0.0) else {
                continue;
            };
            let (label, icon) = self.material(key);
            let stat = Stat { label, value: pct(share), detail: None, sentiment: "neutral".into(), icon, field: format!("{field}.{key}") };
            self.push(group, stat);
        }
    }

    fn upkeep(&mut self, support: &Map<String, Value>, crew: &[(&str, f64)], he3: bool) {
        let mut keys: Vec<&str> = support.keys().map(String::as_str).collect();
        keys.extend(crew.iter().map(|(k, _)| *k).filter(|k| !support.contains_key(*k)));
        let amounts = keys
            .into_iter()
            .filter_map(|key| {
                let listed = support.get(key).and_then(Value::as_f64).unwrap_or(0.0);
                let from_crew = crew.iter().find(|(k, _)| *k == key).map_or(0.0, |(_, v)| *v);
                let total = listed + from_crew;
                (total != 0.0).then(|| (key.to_string(), total, (from_crew > 0.0).then(|| format!("incl. {} for crew", amount(from_crew)))))
            })
            .collect();
        self.render_amounts("Upkeep / month", "supportMaterials_month", amounts, he3, Tone::Down);
    }

    /// Resource amounts as stats; with `he3`, water and fissiles also show the helium-3 variant.
    fn render_amounts(&mut self, group: &str, field: &str, mut amounts: Vec<(String, f64, Option<String>)>, he3: bool, tone: Tone) {
        let get = |a: &[(String, f64, Option<String>)], k: &str| a.iter().find(|(key, ..)| key == k).map_or(0.0, |(_, v, _)| *v);
        let fissiles = get(&amounts, "fissiles");
        let he3 = he3 && fissiles > 0.0;
        if he3 && !amounts.iter().any(|(k, ..)| k == "water") {
            amounts.push(("water".into(), 0.0, None));
        }
        let water = get(&amounts, "water");
        for (key, v, detail) in amounts {
            let alt = match key.as_str() {
                "water" if he3 => Some(water + fissiles),
                "fissiles" if he3 => Some(0.0),
                _ => None,
            };
            let detail = [detail, alt.map(|a| format!("{} with helium-3", amount(a)))].into_iter().flatten().collect::<Vec<_>>().join(" · ");
            let (label, icon) = self.material(&key);
            let stat = Stat {
                label,
                value: amount(v),
                detail: (!detail.is_empty()).then_some(detail),
                sentiment: sentiment(tone, v).into(),
                icon,
                field: format!("{field}.{key}"),
            };
            self.push(group, stat);
        }
    }

    fn material(&self, key: &str) -> (String, Option<String>) {
        match MATERIALS.iter().find(|(k, ..)| *k == key) {
            Some((_, label, sprite)) => ((*label).to_string(), self.icon(&format!("sprite:{sprite}"))),
            None => (humanize(key), None),
        }
    }

    fn tech_bonuses(&mut self, bonuses: Option<&Value>) {
        for b in bonuses.and_then(Value::as_array).into_iter().flatten() {
            let (Some(category), Some(bonus)) = (b.get("category").and_then(Value::as_str), b.get("bonus").and_then(Value::as_f64)) else {
                continue;
            };
            let name = model::TechCategory::parse(category).map_or_else(|| humanize(category), |_| category_label(category));
            let stat = Stat {
                label: format!("{name} research"),
                value: signed_pct(bonus),
                detail: None,
                sentiment: sentiment(Tone::Up, bonus).into(),
                icon: self.icon(&format!("sprite:tech_{category}")),
                field: format!("techBonuses.{category}"),
            };
            self.push("Research bonuses", stat);
        }
    }

    /// `HabModuleSpecialRule.*` sentences. Placeholders: `{0}`/`{2}` are the value, `{1}`
    /// the value as a percentage; the others are game constants or runtime state.
    fn hab_rules(&mut self, record: &Map<String, Value>) {
        let value = record.get("specialRulesValue").and_then(Value::as_f64);
        for rule in record.get("specialRules").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
            let text = self
                .texts
                .get(&format!("HabModuleSpecialRule.{rule}"))
                .filter(|t| !t.trim().is_empty())
                .map(|t| {
                    to_plain_with(t, &mut |p| match (p, value) {
                        ("1", Some(v)) => Some(pct(v)),
                        ("0" | "2", Some(v)) => Some(num(v)),
                        _ => Some("(varies)".into()),
                    })
                })
                .map(|t| t.trim().trim_start_matches('-').trim().to_string());
            match text {
                Some(t) if !t.is_empty() => self.stats.notes.push(t),
                _ => self.stats.tags.push(humanize(rule)),
            }
        }
    }

    fn module_rules(&mut self, record: &Map<String, Value>) {
        for rule in record.get("specialModuleRules").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
            self.stats.tags.push(humanize(rule));
        }
    }

    /// Armor specialties, labelled with the game's tooltip lines.
    fn armor(&mut self, specialties: Option<&Value>) {
        for s in specialties.and_then(Value::as_array).into_iter().flatten() {
            let (Some(name), Some(v)) = (s.get("armorSpecialty").and_then(Value::as_str), s.get("value").and_then(Value::as_f64)) else {
                continue;
            };
            let label = self
                .texts
                .get(&format!("TIShipArmorTemplate.{name}"))
                .map(|t| to_plain_with(t, &mut |_| Some(String::new())).trim().trim_end_matches(':').trim().to_string())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| humanize(name));
            let tone = if name.ends_with("Vulnerability") { Tone::Down } else { Tone::Up };
            // X-ray/baryonic entries are absolute armor points; the others fractional bonuses.
            let points = matches!(name, "XRayResistance" | "BaryonicResistance");
            let stat = Stat {
                label,
                value: if points { num(v) } else { pct(v) },
                detail: None,
                sentiment: sentiment(tone, v).into(),
                icon: self.icon("sprite:armor"),
                field: format!("specialties.{name}"),
            };
            self.push("Armor specialties", stat);
        }
    }

    /// Hull module slots, counted by type.
    fn slots(&mut self, slots: Option<&Value>) {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for t in slots.and_then(Value::as_array).into_iter().flatten().filter_map(|s| s.get("moduleSlotType")?.as_str()) {
            match counts.iter_mut().find(|(n, _)| n == t) {
                Some((_, c)) => *c += 1,
                None => counts.push((t.to_string(), 1)),
            }
        }
        for (t, c) in counts {
            let stat = Stat {
                label: humanize(&t),
                value: c.to_string(),
                detail: None,
                sentiment: "neutral".into(),
                icon: None,
                field: format!("shipModuleSlots.{t}"),
            };
            self.push("Module slots", stat);
        }
    }

    fn drive_notes(&mut self, record: &Map<String, Value>) {
        let key = match record.get("cooling").and_then(Value::as_str) {
            Some("Closed") => "TIDriveTemplate.ClosedCycle",
            Some("Open") => "TIDriveTemplate.OpenCycle",
            Some(other) => {
                let stat = Stat {
                    label: "Cooling".into(),
                    value: if other == "Calc" { "Depends on exhaust velocity".into() } else { humanize(other) },
                    detail: None,
                    sentiment: "neutral".into(),
                    icon: None,
                    field: "cooling".into(),
                };
                self.push(PROPULSION, stat);
                return;
            }
            None => return,
        };
        if let Some(t) = self.texts.get(key) {
            self.stats.notes.push(plain(t));
        }
    }

    fn org_notes(&mut self, record: &Map<String, Value>) {
        let names = |field: &str, template: &str| -> Vec<String> {
            record
                .get(field)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|n| self.view.find(template, n).map_or_else(|| humanize(n), |id| self.view.name(self.texts, id)))
                .collect()
        };
        let fill = |key: &str, fallback: &str, arg: &str| -> String {
            let t = self.texts.get(key).unwrap_or(fallback);
            to_plain_with(t, &mut |_| Some(arg.to_string()))
        };
        let required = names("requiredOwnerTraits", model::TRAIT);
        if !required.is_empty() {
            self.stats.notes.push(fill("UI.Orgs.RequiresTraits", "Councilor must have these trait(s): {0}", &required.join(", ")));
        }
        let prohibited = names("prohibitedOwnerTraits", model::TRAIT);
        if !prohibited.is_empty() {
            self.stats.notes.push(fill("UI.Orgs.ProhibitedTraits", "Councilor cannot have these trait(s): {0}", &prohibited.join(", ")));
        }
        if let Some(region) = record.get("homeRegionMapTemplateName").and_then(Value::as_str) {
            let name = humanize(region.strip_prefix("map_").unwrap_or(region));
            self.stats.notes.push(fill("UI.Orgs.HomeRegion", "Headquartered in {0} region", &name));
        }
        let affinities = names("affinities", model::FACTION);
        if !affinities.is_empty() {
            self.stats.notes.push(format!("Ideological affinity: {}", affinities.join(", ")));
        }
        let restricted = names("restricted", model::FACTION);
        if !restricted.is_empty() {
            self.stats.notes.push(format!("Not available to: {}", restricted.join(", ")));
        }
        if record.get("allowedOnMarket").and_then(Value::as_bool) == Some(false) {
            self.stats.notes.push("Never offered in the org market".into());
        }
        if record.get("grantsMarked").and_then(Value::as_bool) == Some(true)
            && let Some(t) = self.texts.get("UI.Orgs.GrantsMarked")
        {
            let marked = self.view.find(model::TRAIT, "Marked").map_or_else(|| "Marked".into(), |id| self.view.name(self.texts, id));
            self.stats.notes.push(to_plain_with(t, &mut |_| Some(marked.clone())));
        }
    }

    fn links(&mut self, record: &Map<String, Value>) {
        let r = |id| entity_ref(self.view, self.texts, id, self.spoilers);
        let mut add = |label: &str, refs: Vec<EntityRef>| {
            if !refs.is_empty() {
                self.stats.links.push(StatLinks { label: label.into(), entities: refs });
            }
        };
        let str_field = |f: &str| record.get(f).and_then(Value::as_str);
        let missions = record
            .get("missionsGrantedNames")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter_map(|n| self.view.find(model::MISSION, n))
            .map(r)
            .collect();
        add("Missions granted", missions);
        add("Requires tech", str_field("requiredTechName").and_then(|n| self.view.find_research(n)).map(r).into_iter().collect());
        add("Upgrades from", str_field("upgradesFromName").and_then(|n| self.view.find(model::HAB_MODULE, n)).map(r).into_iter().collect());
        add("Project granted on acquisition", str_field("projectGrantedName").and_then(|n| self.view.find_research(n)).map(r).into_iter().collect());
        add("Unlocks project", str_field("unlocksProjectName").and_then(|n| self.view.find_research(n)).map(r).into_iter().collect());
        if let Some(own) = record.get("dataName").and_then(Value::as_str) {
            let upgrades = self
                .view
                .entities(model::HAB_MODULE)
                .filter(|&id| self.view.record(id).and_then(|r| r.get("upgradesFromName")).and_then(Value::as_str) == Some(own))
                .map(r)
                .collect();
            add("Upgrades to", upgrades);
        }
    }
}

fn sentiment(tone: Tone, v: f64) -> &'static str {
    match tone {
        Tone::Neutral => "neutral",
        _ if v == 0.0 => "neutral",
        Tone::Up if v > 0.0 => "good",
        Tone::Down if v < 0.0 => "good",
        _ => "bad",
    }
}

fn format_value(fmt: Fmt, v: f64) -> String {
    match fmt {
        Fmt::Num(unit) => format!("{}{unit}", num(v)),
        Fmt::Signed(unit) => format!("{}{unit}", signed(v)),
        Fmt::Pct => pct(v),
        Fmt::SignedPct => signed_pct(v),
        _ => num(v),
    }
}

/// `1234567` → `1,234,567`; fractions as the game prints small numbers.
fn num(v: f64) -> String {
    if v.fract() != 0.0 || v.abs() < 1000.0 {
        return format_small_number(v);
    }
    let digits = format!("{}", v.abs() as i64);
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if v < 0.0 { format!("−{out}") } else { out }
}

fn signed(v: f64) -> String {
    let n = num(v.abs());
    if v < 0.0 { format!("−{n}") } else { format!("+{n}") }
}

fn pct(v: f64) -> String {
    format!("{}%", num(round_pct(v)))
}

fn signed_pct(v: f64) -> String {
    format!("{}%", signed(round_pct(v)))
}

/// Fraction → percent, rounding away float noise (0.07 * 100 = 7.000000000000001).
fn round_pct(v: f64) -> f64 {
    (v * 100.0 * 1e6).round() / 1e6
}

/// Resource amount: one decimal from 1 up, two significant digits below, scientific
/// notation for traces (antimatter per tank is ~1e-15).
fn amount(v: f64) -> String {
    if v == 0.0 {
        "0".into()
    } else if v.abs() >= 1.0 {
        num((v * 10.0).round() / 10.0)
    } else if v.abs() >= 1e-4 {
        let decimals = (1 - v.abs().log10().floor() as i32).max(0) as usize;
        let s = format!("{v:.decimals$}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        format!("{v:.1e}")
    }
}

fn plain(text: &str) -> String {
    ti_data::text::to_plain(text).trim().to_string()
}

fn category_label(category: &str) -> String {
    match category {
        "InformationScience" => "Information science".into(),
        "LifeScience" => "Life science".into(),
        "MilitaryScience" => "Military science".into(),
        "SocialScience" => "Social science".into(),
        "SpaceScience" => "Space science".into(),
        c => c.into(),
    }
}

/// `TwoHullHoriz` → `Two hull horiz`, `Cost_Scales_With_Gravity` → `Cost scales with gravity`,
/// `EV_kps` → `EV kps`.
fn humanize(s: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' || c == ' ' || c == '-' {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            continue;
        }
        let prev = i.checked_sub(1).map(|j| chars[j]);
        let next = chars.get(i + 1);
        let boundary = c.is_uppercase()
            && prev.is_some_and(|p| p.is_lowercase() || p.is_ascii_digit() || (p.is_uppercase() && next.is_some_and(|n| n.is_lowercase())));
        if boundary && !cur.is_empty() {
            words.push(std::mem::take(&mut cur));
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    let mut out = words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let acronym = w.len() > 1 && w.chars().all(|c| c.is_uppercase() || c.is_ascii_digit());
            if i == 0 || acronym { w.clone() } else { w.to_lowercase() }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if let Some(first) = out.get(0..1) {
        out = first.to_uppercase() + &out[1..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::tests::backend;

    #[test]
    fn formatting() {
        assert_eq!(humanize("TwoHullHoriz"), "Two hull horiz");
        assert_eq!(humanize("Cost_Scales_With_Gravity"), "Cost scales with gravity");
        assert_eq!(humanize("EVMultiplier"), "EV multiplier");
        assert_eq!(humanize("map_England"), "Map england");
        assert_eq!(num(1_234_567.0), "1,234,567");
        assert_eq!(num(-2500.0), "−2,500");
        assert_eq!(signed(-5.0), "−5");
        assert_eq!(pct(0.07), "7%");
        assert_eq!(signed_pct(0.025), "+2.5%");
        assert_eq!(amount(19.75), "19.8");
        assert_eq!(amount(0.002), "0.002");
        assert_eq!(amount(0.35), "0.35");
        assert_eq!(amount(0.8192), "0.82");
        assert_eq!(amount(0.0021), "0.0021");
        assert_eq!(amount(3e-15), "3.0e-15");
        assert_eq!(prefixed("rand", "incomeMoney"), "randIncomeMoney");
        assert_eq!(prefixed("chance", "MCBonus"), "chanceMCBonus");
    }

    fn stats(template: &str, data_name: &str) -> EntityStats {
        let b = backend().expect("dataset");
        let texts = b.texts("en").unwrap();
        let view = b.ds.overview();
        let id = view.find(template, data_name).unwrap_or_else(|| panic!("{template}:{data_name}"));
        entity_stats(&view, &texts, id, &b.spoilers(&view).unwrap()).unwrap()
    }

    fn value<'s>(s: &'s EntityStats, field: &str) -> &'s Stat {
        s.groups.iter().flat_map(|g| &g.stats).find(|st| st.field == field).unwrap_or_else(|| panic!("no {field} in {s:#?}"))
    }

    #[test]
    fn space_entity_stats() {
        let Some(b) = backend() else { return };
        let earth = stats(ti_data::space::SPACE_BODY, "Earth");
        let gravity = value(&earth, "surfaceGravity_g").value.strip_suffix(" g").unwrap().parse::<f64>().unwrap();
        assert!((gravity - 1.0).abs() < 0.02, "Earth gravity: {gravity}");
        assert!(earth.links.iter().any(|l| l.label == "Parent body"));
        let has = |s: &EntityStats, field: &str| s.groups.iter().flat_map(|g| &g.stats).any(|st| st.field == field);
        // no size data / no mass: unknown values are left out rather than shown as 0
        assert!(!has(&stats(ti_data::space::SPACE_BODY, "Skrymir"), "meanRadius_km"));
        let comet = stats(ti_data::space::SPACE_BODY, "PonsWinnecke");
        assert!(!has(&comet, "surfaceGravity_g") && !has(&comet, "escapeVelocity_kps"));
        let view = b.ds.overview();
        let mars_id = view.find(ti_data::space::SPACE_BODY, "Mars").unwrap();
        let mars: ti_data::space::SpaceBody = model::parse(view.record(mars_id).unwrap());
        let site = stats(ti_data::space::HAB_SITE, &mars.hab_sites[0]);
        let output = site.groups.iter().find(|g| g.label == "Expected output / month").unwrap();
        assert_eq!(output.stats.len(), 5);
        assert!(output.stats.iter().all(|s| s.detail.is_some() && s.value.contains('–')));
        let orbit = stats(ti_data::space::ORBIT, &mars.orbits[0]);
        assert!(!value(&orbit, "stationCapacity").value.is_empty());
        let site_id = view.find(ti_data::space::HAB_SITE, &mars.hab_sites[0]).unwrap();
        let data: ti_data::space::HabSite = model::parse(view.record(site_id).unwrap());
        let profile = stats(ti_data::space::MINING_PROFILE, &data.mining_profile);
        assert_eq!(profile.groups[0].stats.len(), 5);
    }

    #[test]
    fn hab_module_stats() {
        if backend().is_none() {
            return;
        }
        let s = stats(model::HAB_MODULE, "OutpostMiningComplex");
        assert!(s.tags.contains(&"Tier 1".into()) && s.tags.contains(&"Base only".into()) && s.tags.contains(&"Mine".into()));
        assert_eq!(value(&s, "power").value, "−10 MW");
        assert_eq!(value(&s, "power").sentiment, "bad");
        // 12 crew: +3.5 t water/volatiles × 0.1 per crew-year, 0.1 money per crew-year.
        assert_eq!(value(&s, "supportMaterials_month.water").value, "2.4");
        assert_eq!(value(&s, "supportMaterials_month.money").value, "6.1");
        // 250 t × 79% metals × 0.1
        assert_eq!(value(&s, "weightedBuildMaterials.metals").value, "19.8");
        assert_eq!(value(&s, "weightedBuildMaterials.metals").detail.as_deref(), Some("79% of 250 t"));
        assert!(s.notes.iter().any(|n| n.starts_with("Construction cost and power consumption scale")), "{:?}", s.notes);
        assert!(s.notes.iter().any(|n| n.contains("only have one")), "{:?}", s.notes);
        assert!(s.notes.iter().any(|n| n.contains("mine size")), "{:?}", s.notes);
        // Helium-3 swaps fissiles for water: 2,400 t × (10% water + 2% fissiles) × 0.1.
        let he3 = stats(model::HAB_MODULE, "HeavyFusionReactorFarm");
        let fissiles = value(&he3, "weightedBuildMaterials.fissiles");
        assert!(fissiles.detail.as_deref().is_some_and(|d| d.ends_with("0 with helium-3")), "{fissiles:?}");
        let water = value(&he3, "weightedBuildMaterials.water");
        assert_eq!((water.value.as_str(), water.detail.as_deref()), ("24", Some("10% of 2,400 t · 28.8 with helium-3")));
        // Alien owners pay their crew nothing.
        let alien = stats(model::HAB_MODULE, "AlienSpaceworks");
        assert!(alien.groups.iter().flat_map(|g| &g.stats).all(|st| st.field != "supportMaterials_month.money"));
    }

    #[test]
    fn ship_part_stats() {
        if backend().is_none() {
            return;
        }
        let s = stats("TIDriveTemplate", "PlasmaWaveDrivex5");
        assert!(s.tags.contains(&"Electromagnetic".into()), "{:?}", s.tags);
        assert_eq!(value(&s, "thrust_N").value, "6,000 N");
        assert_eq!(value(&s, "EV_kps").value, "78.4 km/s");
        assert_eq!(value(&s, "req power").value, "0.29 GW");
        assert_eq!(value(&s, "requiredPowerPlant").value, "Any Reactor");
        let missile = stats("TIMissileTemplate", "LanceheadMissileBay");
        assert_eq!(value(&missile, "salvo_shots").value, "8 shots");
        // 10 t × 50% metals × 0.1; magazine 16 × 1600 kg × 32% metals × 0.1
        assert_eq!(value(&missile, "weightedBuildMaterials.metals").value, "0.5");
        assert_eq!(value(&missile, "ammoMaterials.metals").value, "0.82");
        assert!(missile.tags.contains(&"Penetrator".into()));
        let armor = stats("TIShipArmorTemplate", "BoronCarbideArmor");
        assert_eq!(value(&armor, "specialties.XRayResistance").value, "1");
        assert_eq!(value(&armor, "specialties.KineticsResistance").value, "75%");
        assert_eq!(value(&stats("TIShipArmorTemplate", "SteelArmor"), "weightedBuildMaterials.volatiles").value, "0.002");
        let hull = stats("TIShipHullTemplate", "Destroyer");
        assert_eq!((value(&hull, "missionControl").value.as_str(), value(&hull, "missionControl").sentiment.as_str()), ("2", "bad"));
        assert_eq!(value(&hull, "shipModuleSlots.Utility").value, "5");
        // Nothing presentational leaks into "Other".
        for s in [&s, &missile, &hull] {
            for g in s.groups.iter().filter(|g| g.label == OTHER) {
                assert!(g.stats.iter().all(|st| !is_presentation(&st.field)), "{:?}", g.stats);
            }
        }
    }

    #[test]
    fn org_stats() {
        let Some(b) = backend() else { return };
        let texts = b.texts("en").unwrap();
        let view = b.ds.view(ti_data::Scope::scenario(b.ds.scenario_by_name("ModernScenario").unwrap().idx));
        let id = view.find(model::ORG, "CooperateSpecial").unwrap();
        let s = entity_stats(&view, &texts, id, &b.spoilers(&view).unwrap()).unwrap();
        assert_eq!(value(&s, "administration").value, "+6");
        assert_eq!(value(&s, "incomeResearch").value, "+10");
        assert_eq!(value(&s, "techBonuses.Xenology").value, "+10%");
        assert!(s.links.iter().any(|l| l.label == "Missions granted" && l.entities.len() == 4), "{:?}", s.links);
        // `rand*`/`chance*` companions fold into their stat.
        assert!(s.groups.iter().flat_map(|g| &g.stats).all(|st| !st.field.starts_with("chance") && !st.field.starts_with("rand")));
    }
}
