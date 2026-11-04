//! Space bodies, hab sites and mining profiles, with the game's formulas.
//!
//! Each formula cites the decompiled member it mirrors. The game evaluates the mining
//! formulas in `float`; these use `f64`, which only matters at exact rating thresholds.
//! Values the game randomizes when a template omits them (orbit angles, hab site
//! coordinates) are left `None`.

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

use serde::Deserialize;
use serde_json::Value;

use crate::dataset::EntityId;
use crate::model;
use crate::view::View;

pub const SPACE_BODY: &str = "TISpaceBodyTemplate";
pub const HAB_SITE: &str = "TIHabSiteTemplate";
pub const MINING_PROFILE: &str = "TIMiningProfileTemplate";
pub const ORBIT: &str = "TIOrbitTemplate";
pub const NAVIGABLE: &str = "TINavigableTemplate";
pub const GLOBAL_CONFIG: &str = "TIGlobalConfig";

/// The game's gravitational constant (`TISpaceObjectState.mu`); escape velocity uses `2G = 1.334768E-10`.
pub const G: f64 = 6.67384e-11;
pub const AU_M: f64 = 149_597_870_700.0;
/// `surfaceGravity_g` divisor.
pub const STANDARD_GRAVITY: f64 = 9.80665;
/// `TIHabSiteState.Get*Productivity_day`: month → day divisor.
pub const DAYS_PER_MONTH: f64 = 30.436874;
/// `TIDateTime.SetTime(double)`: seconds per Julian year.
pub const JULIAN_YEAR_S: f64 = 31_557_600.0;
/// `TISpaceObjectTemplate.Mass` when `mass_kg` is missing (`double.Epsilon`).
const NO_MASS: f64 = 5e-324;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Deserialize)]
pub enum BodyType {
    Star,
    Planet,
    DwarfPlanet,
    PlanetaryMoon,
    AsteroidalMoon,
    Asteroid,
    Comet,
    #[serde(other)]
    #[default]
    Unknown,
}

impl BodyType {
    /// lowerCamel name (`dwarfPlanet`, …).
    pub fn name(self) -> &'static str {
        match self {
            BodyType::Star => "star",
            BodyType::Planet => "planet",
            BodyType::DwarfPlanet => "dwarfPlanet",
            BodyType::PlanetaryMoon => "planetaryMoon",
            BodyType::AsteroidalMoon => "asteroidalMoon",
            BodyType::Asteroid => "asteroid",
            BodyType::Comet => "comet",
            BodyType::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deserialize)]
pub enum Atmosphere {
    #[default]
    None,
    Trace,
    Thin,
    Standard,
    Thick,
    Massive,
}

impl Atmosphere {
    /// `TIHabModuleState.AtmosphereSolarModifier`.
    pub fn solar_modifier(self) -> f64 {
        match self {
            Atmosphere::Massive => 0.0,
            Atmosphere::Thick => 0.25,
            Atmosphere::Standard => 0.5,
            Atmosphere::Thin => 0.75,
            _ => 1.0,
        }
    }
}

/// `TISpaceBodyTemplate` (with the `TISpaceObjectTemplate`/`TINaturalSpaceObjectTemplate` fields).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SpaceBody {
    #[serde(rename = "dataName")]
    pub data_name: String,
    #[serde(rename = "friendlyName")]
    pub friendly_name: String,
    #[serde(rename = "objectType")]
    pub object_type: BodyType,
    #[serde(rename = "barycenterName")]
    pub barycenter_name: Option<String>,
    #[serde(rename = "semiMajorAxis_AU")]
    pub semi_major_axis_au: Option<f64>,
    #[serde(rename = "semiMajorAxis_km")]
    pub semi_major_axis_km: Option<f64>,
    pub eccentricity: Option<f64>,
    #[serde(rename = "inclination_Deg")]
    pub inclination_deg: Option<f64>,
    #[serde(rename = "longAscendingNode_Deg")]
    pub long_ascending_node_deg: Option<f64>,
    #[serde(rename = "argPeriapsis_Deg")]
    pub arg_periapsis_deg: Option<f64>,
    #[serde(rename = "meanAnomalyAtEpoch_Deg")]
    pub mean_anomaly_at_epoch_deg: Option<f64>,
    #[serde(rename = "longPeriapsis_Deg")]
    pub long_periapsis_deg: Option<f64>,
    #[serde(rename = "meanLongitude_Deg")]
    pub mean_longitude_deg: Option<f64>,
    #[serde(rename = "epoch_floatJYears")]
    pub epoch_julian_years: Option<f64>,
    #[serde(rename = "nodalPrecession_Years")]
    pub nodal_precession_years: Option<f64>,
    #[serde(rename = "apsidalPrecession_Years")]
    pub apsidal_precession_years: Option<f64>,
    pub mass_kg: Option<f64>,
    pub density_gcm3: Option<f64>,
    #[serde(rename = "equatorialRadius_km")]
    pub equatorial_radius_km: Option<f64>,
    #[serde(rename = "meanRadius_km")]
    pub mean_radius_km: Option<f64>,
    #[serde(rename = "dimensionX_km")]
    pub dimension_x_km: Option<f64>,
    #[serde(rename = "dimensionY_km")]
    pub dimension_y_km: Option<f64>,
    #[serde(rename = "dimensionZ_km")]
    pub dimension_z_km: Option<f64>,
    pub oblateness: Option<f64>,
    #[serde(rename = "tilt_Deg")]
    pub tilt_deg: Option<f64>,
    #[serde(rename = "rotationPeriod_strHours")]
    pub rotation_period_hours: Option<String>,
    pub atmosphere: Atmosphere,
    #[serde(rename = "maxHabSize", default = "three")]
    pub max_hab_size: u32,
    pub orbits: Vec<String>,
    #[serde(rename = "habSites")]
    pub hab_sites: Vec<String>,
}

fn three() -> u32 {
    3
}

impl SpaceBody {
    /// `TISpaceObjectTemplate.Mass` (`double.Epsilon` when missing).
    pub fn mass(&self) -> f64 {
        self.mass_kg.unwrap_or(NO_MASS)
    }

    /// `TISpaceObjectState.mu`, m³/s².
    pub fn mu(&self) -> f64 {
        G * self.mass()
    }

    /// `TISpaceBodyState.density_gcm3` (2 when missing).
    pub fn density(&self) -> f64 {
        self.density_gcm3.unwrap_or(2.0)
    }

    /// `TISpaceObjectTemplate.SemiMajorAxis_m`: km first, then AU; `None` where the game logs an error and uses 0.
    pub fn semi_major_axis_m(&self) -> Option<f64> {
        self.semi_major_axis_km.map(|km| km * 1000.0).or(self.semi_major_axis_au.map(|au| au * AU_M))
    }

    /// `TISpaceObjectTemplate.Eccentricity` (`double.Epsilon`, i.e. 0, when missing).
    pub fn eccentricity(&self) -> f64 {
        self.eccentricity.unwrap_or(0.0)
    }

    /// `TISpaceObjectTemplate.ArgumentPeriapsis_Rad` in degrees; `None` where the game picks a random angle.
    pub fn arg_periapsis(&self) -> Option<f64> {
        self.arg_periapsis_deg.or_else(|| Some(self.long_periapsis_deg? - self.long_ascending_node_deg?))
    }

    /// `TISpaceObjectTemplate.MeanAnomalyAtEpoch_Rad` in degrees; `None` where the game picks a random angle.
    pub fn mean_anomaly_at_epoch(&self) -> Option<f64> {
        self.mean_anomaly_at_epoch_deg.or_else(|| Some(self.mean_longitude_deg? - self.long_periapsis_deg?))
    }

    /// `TISpaceObjectTemplate.Epoch_floatJYears`.
    pub fn epoch(&self) -> f64 {
        self.epoch_julian_years.unwrap_or(2000.0)
    }

    /// `TISpaceBodyState.dimensionX_km`.
    pub fn dimension_x(&self) -> f64 {
        self.dimension_x_km.or(self.equatorial_radius_km.map(|r| r * 2.0)).unwrap_or_else(|| self.mean_radius_km.unwrap_or(0.0) * 2.0)
    }

    /// `TISpaceBodyState.dimensionY_km`.
    pub fn dimension_y(&self) -> f64 {
        self.dimension_y_km.or(self.dimension_x_km).unwrap_or_else(|| self.dimension_x())
    }

    /// `TISpaceBodyState.dimensionZ_km`.
    pub fn dimension_z(&self) -> f64 {
        self.dimension_z_km.or(self.dimension_y_km).unwrap_or_else(|| self.dimension_y())
    }

    /// `TISpaceBodyState.oblateness`.
    pub fn oblateness(&self) -> f64 {
        self.oblateness.unwrap_or_else(|| (self.dimension_x() - self.dimension_z()) / self.dimension_x())
    }

    /// `TISpaceBodyState.GetPolarRadius_m` in km. (Like the game, the full `dimensionZ_km`.)
    pub fn polar_radius(&self) -> f64 {
        match (self.equatorial_radius_km, self.dimension_z_km) {
            (Some(eq), _) => eq * (1.0 - self.oblateness()),
            (None, Some(z)) => z,
            (None, None) => self.mean_radius(),
        }
    }

    /// `TISpaceBodyState.GetMeanRadius_km`: `meanRadius_km`, else `(2·equatorial + polar)/3`, else
    /// the mean half-dimension (0 without any size).
    pub fn mean_radius(&self) -> f64 {
        if let Some(r) = self.mean_radius_km {
            return r;
        }
        if let Some(eq) = self.equatorial_radius_km {
            return (2.0 * eq + eq * (1.0 - self.oblateness())) / 3.0;
        }
        (self.dimension_x() + self.dimension_y() + self.dimension_z()) / 6.0
    }

    /// `TISpaceBodyState.maxRadiusDimension_km`.
    pub fn max_radius_dimension(&self) -> f64 {
        self.dimension_x().max(self.dimension_y()).max(self.dimension_z()) / 2.0
    }

    /// `TISpaceBodyState.surfaceGravity_mps2`.
    pub fn surface_gravity_mps2(&self) -> f64 {
        let r = self.mean_radius() * 1000.0;
        self.mu() / (r * r)
    }

    /// `TISpaceBodyState.surfaceGravity_g`.
    pub fn surface_gravity_g(&self) -> f64 {
        self.surface_gravity_mps2() / STANDARD_GRAVITY
    }

    /// `TISpaceBodyState.escapeVelocity_mps`.
    pub fn escape_velocity_mps(&self) -> f64 {
        (2.0 * G * self.mass() / (self.mean_radius() * 1000.0)).sqrt()
    }

    /// `TISpaceObjectState.orbitalPeriod_s` around a parent of the given mass.
    pub fn orbital_period_s(&self, parent_mass_kg: f64) -> Option<f64> {
        Some(orbital_period_s(self.semi_major_axis_m()?, parent_mass_kg))
    }

    /// `TISpaceBodyState.GetRotationPeriod_Hours`: `"Lock"` means the orbital period.
    pub fn rotation_period_hours(&self, parent_mass_kg: Option<f64>) -> Option<f64> {
        match self.rotation_period_hours.as_deref()?.trim() {
            "Lock" => Some(self.orbital_period_s(parent_mass_kg?)? / 3600.0),
            s => s.parse().ok(),
        }
    }

    /// `TIHabModuleState.SetLocationSolarPowerMultiplier` for the body (`site_latitude: None`)
    /// or one of its hab sites. `sun_distance_au`: semi-major axis of the body's
    /// sun-orbiting ancestor (`GetSunOrbitingRelatedObject`); `orbits_sun`: its parent is a star.
    pub fn solar_multiplier(&self, sun_distance_au: f64, orbits_sun: bool, site_latitude: Option<f64>) -> f64 {
        let (atmosphere, exposure) = match self.object_type {
            BodyType::Star => return 1.0,
            BodyType::Asteroid | BodyType::AsteroidalMoon => (1.0, 0.6),
            BodyType::Comet => (0.5, 0.6),
            _ => {
                // Polar sites of untilted planets get near-constant sunlight.
                let polar = site_latitude.filter(|lat| orbits_sun && self.tilt_deg.unwrap_or(0.0) < 5.0 && lat.abs() > 85.0);
                (self.atmosphere.solar_modifier(), polar.map_or(0.5, |lat| 0.5 + lat.abs() / 360.0))
            }
        };
        atmosphere * exposure / (sun_distance_au * sun_distance_au)
    }
}

/// `TISpaceObjectState.orbitalPeriod_s`: `2π·√(a³ / (G·M))`.
pub fn orbital_period_s(semi_major_axis_m: f64, parent_mass_kg: f64) -> f64 {
    TAU * (semi_major_axis_m.powi(3) / (G * parent_mass_kg)).sqrt()
}

/// `Mathd.Normalize_Rad`: wraps into `[-π, π)`.
pub fn normalize_rad(angle: f64) -> f64 {
    angle - TAU * ((angle + PI) / TAU).floor()
}

/// Days since 1970-01-01 of a proleptic Gregorian date (`days_from_civil`, H. Hinnant).
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = month as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// UTC seconds since 1970 of a Julian-year epoch, as `TIDateTime.SetTime(double)` converts it:
/// January 1st of year ⌊J⌋, plus ⌊frac(J) · 31 557 600⌋ s, plus 12 h (so 2000.0 is J2000).
pub fn epoch_unix_seconds(julian_year: f64) -> f64 {
    let year = julian_year.trunc();
    let seconds = ((julian_year - year) * JULIAN_YEAR_S).trunc();
    days_from_civil(year as i64, 1, 1) as f64 * 86_400.0 + seconds + 43_200.0
}

/// Mean anomaly (rad, in `[-π, π)`) at `unix_seconds`, as `OrbitalElementsState.ToCartesianStateAtTime`
/// propagates an elliptic orbit: `Normalize_Rad(M₀ + 2π·Δt/P)`, Δt in seconds since the epoch.
/// The game never applies the templates' precession periods.
pub fn mean_anomaly_at(mean_anomaly_at_epoch_rad: f64, epoch_julian_year: f64, period_s: f64, unix_seconds: f64) -> f64 {
    let dt = unix_seconds - epoch_unix_seconds(epoch_julian_year);
    normalize_rad(mean_anomaly_at_epoch_rad + TAU * (dt / period_s))
}

/// `TIHabSiteTemplate`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HabSite {
    #[serde(rename = "dataName")]
    pub data_name: String,
    #[serde(rename = "parentBodyName")]
    pub parent_body: String,
    #[serde(rename = "miningProfileName")]
    pub mining_profile: String,
    /// Random in ±60° when missing (`TIHabSiteState.InitWithTemplate`).
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Resource {
    Water,
    Volatiles,
    Metals,
    Nobles,
    Fissiles,
}

impl Resource {
    pub const ALL: [Resource; 5] = [Resource::Water, Resource::Volatiles, Resource::Metals, Resource::Nobles, Resource::Fissiles];

    /// Prefix of the mining profile fields (`water`, `nobles`, …).
    pub fn key(self) -> &'static str {
        match self {
            Resource::Water => "water",
            Resource::Volatiles => "volatiles",
            Resource::Metals => "metals",
            Resource::Nobles => "nobles",
            Resource::Fissiles => "fissiles",
        }
    }

    /// Metals, nobles and fissiles scale with density (`densitySensitive`).
    fn density_sensitive(self) -> bool {
        matches!(self, Resource::Metals | Resource::Nobles | Resource::Fissiles)
    }
}

/// One resource of a mining profile (per month).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ResourceProfile {
    pub mean: f64,
    pub width: f64,
    pub min: f64,
    pub jump: f64,
}

/// `TIMiningProfileTemplate`.
#[derive(Debug, Clone, Default)]
pub struct MiningProfile {
    pub data_name: String,
    pub modify_by_size: bool,
    pub resources: [ResourceProfile; 5],
}

impl MiningProfile {
    pub fn from_record(record: &Value) -> MiningProfile {
        let num = |key: String| record.get(&key).and_then(Value::as_f64).unwrap_or(0.0);
        MiningProfile {
            data_name: record.get("dataName").and_then(Value::as_str).unwrap_or_default().to_string(),
            modify_by_size: record.get("modifyBySize").and_then(Value::as_bool).unwrap_or(false),
            resources: Resource::ALL.map(|r| ResourceProfile {
                mean: num(format!("{}_mean", r.key())),
                width: num(format!("{}_width", r.key())),
                min: num(format!("{}_min", r.key())),
                jump: num(format!("{}_jump", r.key())),
            }),
        }
    }

    pub fn get(&self, r: Resource) -> ResourceProfile {
        self.resources[r as usize]
    }
}

/// `TIOrbitTemplate` (fields used for display).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Orbit {
    #[serde(rename = "dataName")]
    pub data_name: String,
    #[serde(rename = "barycenterName")]
    pub barycenter_name: String,
    #[serde(rename = "stationCapacity", default = "one")]
    pub station_capacity: u32,
    #[serde(rename = "irradiatedMultiplier")]
    pub irradiated_multiplier: f64,
    #[serde(rename = "interfaceOrbit")]
    pub interface_orbit: bool,
}

fn one() -> u32 {
    1
}

/// `TINavigableTemplate`: a Lagrange point of `related_object` and its parent.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LagrangePoint {
    #[serde(rename = "dataName")]
    pub data_name: String,
    /// `L1` … `L5`.
    #[serde(rename = "lagrangeValue")]
    pub lagrange_value: String,
    #[serde(rename = "relatedObject")]
    pub related_object: String,
    #[serde(rename = "maxHabSize", default = "three")]
    pub max_hab_size: u32,
}

/// `TIGlobalConfig` values behind hab site output (game-code defaults where the config omits them).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MiningConstants {
    pub max_mass_for_malus: f64,
    pub metals_malus_density_cut: f64,
    pub metals_bonus_density_cut: f64,
    /// `TIGlobalValuesState.GetGlobalMineProductivityModifier`: the campaign slider
    /// (`defaultMiningProductivity` 20 × 0.05 = 1 with default settings).
    pub productivity: f64,
}

impl Default for MiningConstants {
    fn default() -> Self {
        MiningConstants { max_mass_for_malus: 5e16, metals_malus_density_cut: 1.25, metals_bonus_density_cut: 3.0, productivity: 1.0 }
    }
}

impl MiningConstants {
    pub fn from_global_config(record: Option<&Value>) -> MiningConstants {
        let d = MiningConstants::default();
        let num = |key: &str, default: f64| record.and_then(|r| r.get(key)).and_then(Value::as_f64).unwrap_or(default);
        MiningConstants {
            max_mass_for_malus: num("maxMassforMiningResourceMalus", d.max_mass_for_malus),
            metals_malus_density_cut: num("metalsMalusDensityCutPoint", d.metals_malus_density_cut),
            metals_bonus_density_cut: num("metalsBonusDensityCutPoint", d.metals_bonus_density_cut),
            productivity: d.productivity,
        }
    }
}

/// Monthly output of one resource at a hab site.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SiteOutput {
    pub expected: f64,
    pub min: f64,
    pub max: f64,
}

/// Mining at a site of a body with the given mass and density.
#[derive(Debug, Clone, Copy)]
pub struct SiteMining<'a> {
    pub profile: &'a MiningProfile,
    pub body_mass_kg: f64,
    pub body_density: f64,
    pub constants: MiningConstants,
}

impl SiteMining<'_> {
    /// `TIHabSiteState.ModifyBaseValueForConditions`: small or (for metals etc.) light bodies
    /// lose output, dense ones gain; the factor is clamped to 0.75–1.25.
    pub fn modify(&self, base: f64, density_sensitive: bool) -> f64 {
        let c = &self.constants;
        let mut value = base;
        if self.profile.modify_by_size && value > 0.0 {
            let mut factor = 1.0;
            if self.body_mass_kg <= c.max_mass_for_malus {
                factor *= self.body_mass_kg / c.max_mass_for_malus;
            }
            if density_sensitive {
                if self.body_density <= c.metals_malus_density_cut {
                    factor *= self.body_density / c.metals_malus_density_cut;
                } else if self.body_density >= c.metals_bonus_density_cut {
                    factor *= self.body_density / c.metals_bonus_density_cut;
                }
            }
            value *= factor.clamp(0.75, 1.25);
        }
        value * c.productivity
    }

    /// `TIHabSiteState.GetHabSiteExpectedProductivity_month` (nobles capped at half the metals).
    pub fn expected_month(&self, r: Resource) -> f64 {
        let value = self.modify(self.profile.get(r).mean, r.density_sensitive());
        if r == Resource::Nobles { value.min(self.expected_month(Resource::Metals) / 2.0) } else { value }
    }

    /// `GetHabSiteExpectedProductivity_month`, `…Min…`, `…Max…`: expected ± half the modified width,
    /// the minimum floored at the profile's (unmodified) `min`.
    pub fn output_month(&self, r: Resource) -> SiteOutput {
        let p = self.profile.get(r);
        let expected = self.expected_month(r);
        let half_width = self.modify(p.width, false) / 2.0;
        SiteOutput { expected, min: p.min.max(expected - half_width), max: expected + half_width }
    }
}

/// `SiteProfileRating`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SiteRating {
    Empty,
    Possible,
    Low,
    Medium,
    High,
    Max,
}

impl SiteRating {
    pub fn name(self) -> &'static str {
        match self {
            SiteRating::Empty => "empty",
            SiteRating::Possible => "possible",
            SiteRating::Low => "low",
            SiteRating::Medium => "medium",
            SiteRating::High => "high",
            SiteRating::Max => "max",
        }
    }

    /// `TIMiningProfileTemplate.GetSiteProfileRating`. `global_cap`: the best expected daily
    /// output of any hab site (`maxGlobalExpectedHabSiteProduction_day`).
    pub fn rate(mean: f64, width: f64, min: f64, global_cap: f64, prospected: bool) -> SiteRating {
        if mean <= 0.0 && (prospected || (min <= 0.0 && width <= mean.abs())) {
            SiteRating::Empty
        } else if min <= 0.0 && !prospected {
            SiteRating::Possible
        } else if mean <= global_cap * 0.333 {
            SiteRating::Low
        } else if mean <= global_cap * 0.667 {
            SiteRating::Medium
        } else if mean <= global_cap * 0.95 {
            SiteRating::High
        } else {
            SiteRating::Max
        }
    }
}

/// A space body in scope with its resolved parent.
#[derive(Debug, Clone)]
pub struct Body {
    pub id: EntityId,
    pub data: SpaceBody,
    /// Index into [`System::bodies`].
    pub parent: Option<usize>,
    /// Hab sites (`habSites`) that exist in scope, with their mining profiles.
    pub sites: Vec<(EntityId, HabSite, Option<MiningProfile>)>,
}

/// The bodies, hab sites and Lagrange points of a scope.
#[derive(Debug, Clone)]
pub struct System {
    /// Parents before children (stars first), siblings in load order.
    pub bodies: Vec<Body>,
    pub lagrange_points: Vec<(EntityId, LagrangePoint)>,
    pub mining: MiningConstants,
    by_name: HashMap<String, usize>,
    /// `maxGlobalExpectedHabSiteProduction_day`, per resource.
    global_cap: [f64; 5],
}

impl System {
    pub fn load(view: &View<'_>) -> System {
        let mining = MiningConstants::from_global_config(view.entities(GLOBAL_CONFIG).next().and_then(|id| view.record(id)));
        let mut loaded: Vec<Body> = view
            .entities(SPACE_BODY)
            .filter_map(|id| {
                let data: SpaceBody = model::parse(view.record(id)?);
                let sites = data
                    .hab_sites
                    .iter()
                    .filter_map(|name| {
                        let site_id = view.find(HAB_SITE, name)?;
                        let site: HabSite = model::parse(view.record(site_id)?);
                        let profile = view.find(MINING_PROFILE, &site.mining_profile).and_then(|p| view.record(p)).map(MiningProfile::from_record);
                        Some((site_id, site, profile))
                    })
                    .collect();
                Some(Body { id, data, parent: None, sites })
            })
            .collect();

        // Depth-first from the roots, so moons follow their planet.
        let names: HashMap<String, usize> = loaded.iter().enumerate().map(|(i, b)| (view.dataset().entity(b.id).data_name.to_string(), i)).collect();
        let parent_of = |b: &Body| b.data.barycenter_name.as_deref().and_then(|n| names.get(n).copied());
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); loaded.len()];
        let mut roots = Vec::new();
        for (i, b) in loaded.iter().enumerate() {
            match parent_of(b) {
                Some(p) if p != i => children[p].push(i),
                _ => roots.push(i),
            }
        }
        roots.sort_by_key(|&i| loaded[i].data.object_type != BodyType::Star);
        let mut order = Vec::with_capacity(loaded.len());
        let mut stack: Vec<usize> = roots.into_iter().rev().collect();
        while let Some(i) = stack.pop() {
            order.push(i);
            stack.extend(children[i].iter().rev());
        }
        // Parent cycles never reach a root; keep those bodies, parentless.
        let mut seen = vec![false; loaded.len()];
        order.iter().for_each(|&i| seen[i] = true);
        order.extend((0..loaded.len()).filter(|&i| !seen[i]));

        let position: HashMap<usize, usize> = order.iter().enumerate().map(|(pos, &i)| (i, pos)).collect();
        let parents: Vec<Option<usize>> = order.iter().map(|&i| parent_of(&loaded[i]).filter(|p| position[p] < position[&i]).map(|p| position[&p])).collect();
        let mut slots: Vec<Option<Body>> = loaded.drain(..).map(Some).collect();
        let bodies: Vec<Body> = order
            .iter()
            .zip(parents)
            .map(|(&i, parent)| Body { parent, ..slots[i].take().expect("each body once") })
            .collect();
        let by_name = bodies.iter().enumerate().map(|(i, b)| (view.dataset().entity(b.id).data_name.to_string(), i)).collect();

        let lagrange_points = view.entities(NAVIGABLE).filter_map(|id| Some((id, model::parse(view.record(id)?)))).collect();
        let mut system = System { bodies, lagrange_points, mining, by_name, global_cap: [0.0; 5] };
        for r in Resource::ALL {
            system.global_cap[r as usize] = system
                .bodies
                .iter()
                .flat_map(|b| b.sites.iter().filter_map(|(_, _, p)| Some(system.site_mining(b, p.as_ref()?).expected_month(r) / DAYS_PER_MONTH)))
                .fold(0.0, f64::max);
        }
        system
    }

    pub fn index(&self, data_name: &str) -> Option<usize> {
        self.by_name.get(data_name).copied()
    }

    pub fn body(&self, data_name: &str) -> Option<&Body> {
        self.index(data_name).map(|i| &self.bodies[i])
    }

    pub fn parent(&self, body: &Body) -> Option<&Body> {
        body.parent.map(|p| &self.bodies[p])
    }

    /// `TISpaceObjectState.orbitalPeriod_s` around the parent.
    pub fn orbital_period_s(&self, body: &Body) -> Option<f64> {
        body.data.orbital_period_s(self.parent(body)?.data.mass())
    }

    /// `TISpaceBodyState.rotationPeriod_Hours`.
    pub fn rotation_period_hours(&self, body: &Body) -> Option<f64> {
        body.data.rotation_period_hours(self.parent(body).map(|p| p.data.mass()))
    }

    /// `TISpaceObjectState.GetSunOrbitingRelatedObject`: the body itself or the ancestor orbiting a star.
    pub fn sun_orbiting<'s>(&'s self, body: &'s Body) -> &'s Body {
        let mut b = body;
        while let Some(p) = self.parent(b) {
            if p.data.object_type == BodyType::Star {
                break;
            }
            b = p;
        }
        b
    }

    /// Solar power multiplier of a body, or of a hab site on it (see [`SpaceBody::solar_multiplier`]).
    pub fn solar_multiplier(&self, body: &Body, site: Option<&HabSite>) -> f64 {
        let au = self.sun_orbiting(body).data.semi_major_axis_m().unwrap_or(0.0) / AU_M;
        let orbits_sun = self.parent(body).is_some_and(|p| p.data.object_type == BodyType::Star);
        body.data.solar_multiplier(au, orbits_sun, site.and_then(|s| s.latitude))
    }

    pub fn site_mining<'p>(&self, body: &Body, profile: &'p MiningProfile) -> SiteMining<'p> {
        SiteMining { profile, body_mass_kg: body.data.mass(), body_density: body.data.density(), constants: self.mining }
    }

    /// `TIGlobalValuesState.maxGlobalExpectedHabSiteProduction_day` over the hab sites in scope.
    pub fn global_cap_per_day(&self, r: Resource) -> f64 {
        self.global_cap[r as usize]
    }

    /// `TISpaceBodyState.GetSiteProfileRating(resource, prospected: false)`: the rating of the
    /// sites' average expected and minimum daily output and average profile width.
    /// `None` without hab sites (the game's average would throw).
    pub fn body_rating(&self, body: &Body, r: Resource) -> Option<SiteRating> {
        let sites: Vec<(SiteMining<'_>, f64)> =
            body.sites.iter().filter_map(|(_, _, p)| p.as_ref()).map(|p| (self.site_mining(body, p), p.get(r).width)).collect();
        if sites.is_empty() {
            return None;
        }
        let n = sites.len() as f64;
        let mean = sites.iter().map(|(m, _)| m.expected_month(r) / DAYS_PER_MONTH).sum::<f64>() / n;
        let width = sites.iter().map(|(_, w)| w).sum::<f64>() / n;
        let min = sites.iter().map(|(m, _)| m.output_month(r).min / DAYS_PER_MONTH).sum::<f64>() / n;
        Some(SiteRating::rate(mean, width, min, self.global_cap_per_day(r), false))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn body(v: Value) -> SpaceBody {
        model::parse(&v)
    }

    #[test]
    fn radius_fallbacks() {
        // equatorial radius and oblateness: (2·eq + polar) / 3
        let b = body(json!({"equatorialRadius_km": 3000.0, "oblateness": 0.1}));
        assert!((b.mean_radius() - (6000.0 + 2700.0) / 3.0).abs() < 1e-9);
        assert_eq!(b.polar_radius(), 2700.0);
        // dimensions only: mean of the half-dimensions; missing Y/Z repeat the previous one
        let b = body(json!({"dimensionX_km": 30.0, "dimensionY_km": 20.0}));
        assert_eq!((b.dimension_z(), b.mean_radius(), b.max_radius_dimension()), (20.0, 70.0 / 6.0, 15.0));
        assert_eq!(body(json!({"meanRadius_km": 5.0})).mean_radius(), 5.0);
        assert_eq!(body(json!({})).mean_radius(), 0.0);
    }

    #[test]
    fn earth_like_physics() {
        let b = body(json!({"mass_kg": 5.972e24, "meanRadius_km": 6371.0, "semiMajorAxis_AU": 1.0, "rotationPeriod_strHours": "Lock"}));
        assert!((b.surface_gravity_g() - 1.0).abs() < 0.01);
        assert!((b.escape_velocity_mps() - 11_186.0).abs() < 20.0);
        let year = b.orbital_period_s(1.98855e30).unwrap() / 86_400.0;
        assert!((year - 365.25).abs() < 0.2, "{year}");
        assert!((b.rotation_period_hours(Some(1.98855e30)).unwrap() / 24.0 - year).abs() < 1e-9);
        assert_eq!(b.rotation_period_hours(None), None);
        assert_eq!(body(json!({"rotationPeriod_strHours": "9.5"})).rotation_period_hours(None), Some(9.5));
    }

    #[test]
    fn orbit_angle_fallbacks() {
        let b = body(json!({"longAscendingNode_Deg": 10.0, "longPeriapsis_Deg": 50.0, "meanLongitude_Deg": 80.0}));
        assert_eq!((b.arg_periapsis(), b.mean_anomaly_at_epoch()), (Some(40.0), Some(30.0)));
        assert_eq!(body(json!({})).arg_periapsis(), None);
        assert_eq!(body(json!({})).epoch(), 2000.0);
    }

    #[test]
    fn epochs_and_mean_anomaly() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        // J2000 = 2000-01-01 12:00 UTC
        assert_eq!(epoch_unix_seconds(2000.0), 946_728_000.0);
        // fractional years count Julian-year seconds from January 1st (truncated)
        assert_eq!(epoch_unix_seconds(2019.5), days_from_civil(2019, 1, 1) as f64 * 86_400.0 + 15_778_800.0 + 43_200.0);
        let period = 100.0;
        let t0 = epoch_unix_seconds(2000.0);
        assert!((mean_anomaly_at(0.5, 2000.0, period, t0 + 25.0) - (0.5 + PI / 2.0)).abs() < 1e-12);
        // wraps into [-π, π)
        assert!((mean_anomaly_at(3.0, 2000.0, period, t0 + 50.0) - (3.0 + PI - TAU)).abs() < 1e-12);
        assert_eq!(normalize_rad(PI), -PI);
    }

    #[test]
    fn solar_multiplier_cases() {
        let planet = body(json!({"objectType": "Planet", "atmosphere": "Thin", "tilt_Deg": 2.0}));
        assert_eq!(planet.solar_multiplier(2.0, true, None), 0.75 * 0.5 / 4.0);
        assert_eq!(planet.solar_multiplier(1.0, true, Some(-88.0)), 0.75 * (0.5 + 88.0 / 360.0));
        // no polar bonus for moons or tilted bodies
        assert_eq!(planet.solar_multiplier(1.0, false, Some(88.0)), 0.375);
        let comet = body(json!({"objectType": "Comet", "atmosphere": "Massive"}));
        assert_eq!(comet.solar_multiplier(1.0, true, None), 0.3);
        assert_eq!(body(json!({"objectType": "Asteroid", "atmosphere": "Massive"})).solar_multiplier(1.0, true, None), 0.6);
        assert_eq!(body(json!({"objectType": "Star"})).solar_multiplier(0.0, false, None), 1.0);
    }

    #[test]
    fn site_output_scaling() {
        let profile = MiningProfile::from_record(&json!({
            "dataName": "P", "modifyBySize": true,
            "water_mean": 40.0, "water_width": 20.0, "water_min": 35.0,
            "metals_mean": 30.0, "metals_width": 10.0, "nobles_mean": 40.0, "nobles_width": 4.0
        }));
        let c = MiningConstants::default();
        let small = SiteMining { profile: &profile, body_mass_kg: 1e16, body_density: 1.0, constants: c };
        // mass 0.2 of the cap → clamped to 0.75; metals also lose density (×0.8) → still 0.75
        let water = small.output_month(Resource::Water);
        assert_eq!(water, SiteOutput { expected: 30.0, min: 35.0, max: 37.5 });
        assert_eq!(small.expected_month(Resource::Metals), 22.5);
        // nobles: at most half the metals
        assert_eq!(small.expected_month(Resource::Nobles), 11.25);
        let dense = SiteMining { body_mass_kg: 1e20, body_density: 4.5, ..small };
        assert_eq!(dense.expected_month(Resource::Metals), 37.5);
        assert_eq!(dense.expected_month(Resource::Water), 40.0);
        let fixed = MiningProfile { modify_by_size: false, ..profile.clone() };
        assert_eq!(SiteMining { profile: &fixed, ..small }.expected_month(Resource::Water), 40.0);
    }

    #[test]
    fn ratings() {
        use SiteRating::*;
        assert_eq!(SiteRating::rate(0.0, 0.0, 0.0, 10.0, false), Empty);
        assert_eq!(SiteRating::rate(0.0, 5.0, 0.0, 10.0, false), Possible);
        assert_eq!(SiteRating::rate(3.0, 5.0, 1.0, 10.0, false), Low);
        assert_eq!(SiteRating::rate(6.0, 5.0, 1.0, 10.0, false), Medium);
        assert_eq!(SiteRating::rate(9.0, 5.0, 1.0, 10.0, false), High);
        assert_eq!(SiteRating::rate(10.0, 5.0, 1.0, 10.0, false), Max);
        assert_eq!(SiteRating::rate(0.0, 5.0, 0.0, 10.0, true), Empty);
    }
}
