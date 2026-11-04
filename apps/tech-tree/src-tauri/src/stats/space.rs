//! Space detail panels; physics and mining calculations live in ti-data.
use super::*;
use ti_data::space::{self as game, Resource, System};

pub(super) fn stats(view: &View<'_>, texts: &Texts, id: EntityId, spoilers: &HashSet<EntityId>) -> Option<EntityStats> {
    let record = view.record(id)?;
    let map = record.as_object()?;
    let mut b = Builder { view, texts, spoilers, stats: EntityStats { tags: vec![], groups: vec![], notes: vec![], links: vec![] } };
    let template = view.dataset().entity(id).template.as_ref();
    match template {
        game::SPACE_BODY => {
            let system = System::load(view);
            let body = system.body(&view.dataset().entity(id).data_name)?;
            let d = &body.data;
            for (key, prefix) in [("objectType", "TISpaceBodyTemplate.ObjectType."), ("atmosphere", "TISpaceBodyTemplate.Atmosphere.")] {
                if let Some(v) = map.get(key) {
                    b.field(&f(key, "", Fmt::TagEnum(prefix), ""), v, map);
                }
            }
            if d.dimension_x_km.is_some() {
                b.space_text(
                    "Physical",
                    "dimensionX_km",
                    "Dimensions",
                    format!("{} × {} × {} km", num(d.dimension_x()), num(d.dimension_y()), num(d.dimension_z())),
                );
            } else if d.mean_radius() > 0.0 {
                b.space_number("Physical", "meanRadius_km", "Diameter", d.mean_radius() * 2.0, " km");
            }
            if let Some(mass) = d.mass_kg {
                b.space_text("Physical", "mass_kg", "Mass", format!("{mass:.3e} kg"));
            }
            b.space_number("Physical", "density_gcm3", "Density", d.density(), " g/cm³");
            // Without size or mass the game's fallbacks (0 km, ε kg) would show as measured zeros.
            if d.mean_radius() > 0.0 && d.mass_kg.is_some_and(|m| m > 0.0) {
                b.space_text("Physical", "surfaceGravity_g", "Surface gravity", format!("{:.2} g", d.surface_gravity_g()));
                b.space_number("Physical", "escapeVelocity_kps", "Escape velocity", d.escape_velocity_mps() / 1000.0, " km/s");
            }
            if let Some(hours) = system.rotation_period_hours(body) {
                let locked = if d.rotation_period_hours.as_deref() == Some("Lock") { " (tidally locked)" } else { "" };
                b.space_text("Physical", "rotationPeriod_strHours", "Rotation period", format!("{} hours{locked}", num(hours)));
            } else if d.rotation_period_hours.as_deref() == Some("Lock") {
                b.space_text("Physical", "rotationPeriod_strHours", "Rotation period", "Tidally locked".into());
            }
            if let Some(tilt) = d.tilt_deg {
                b.space_number("Physical", "tilt_Deg", "Axial tilt", tilt, "°");
            }
            if let Some(parent) = system.parent(body) {
                b.space_text("Orbit", "barycenterName", "Parent", view.name(texts, parent.id));
                b.space_links("Parent body", vec![parent.id]);
                if let Some(axis) = d.semi_major_axis_m() {
                    let sun = parent.data.object_type == game::BodyType::Star;
                    b.space_number("Orbit", "semiMajorAxis", "Semi-major axis", axis / if sun { game::AU_M } else { 1000.0 }, if sun { " AU" } else { " km" });
                }
            }
            if let Some(seconds) = system.orbital_period_s(body) {
                let years = seconds >= game::JULIAN_YEAR_S;
                b.space_number(
                    "Orbit",
                    "orbitalPeriod",
                    "Orbital period",
                    seconds / if years { game::JULIAN_YEAR_S } else { 86400.0 },
                    if years { " years" } else { " days" },
                );
            }
            b.space_number("Orbit", "eccentricity", "Eccentricity", d.eccentricity(), "");
            if let Some(v) = d.inclination_deg {
                b.space_number("Orbit", "inclination_Deg", "Inclination", v, "°");
            }
            let size = match d.max_hab_size {
                1 => "Outpost",
                2 => "Settlement",
                3 => "Colony",
                _ => "",
            };
            let size = if size.is_empty() { d.max_hab_size.to_string() } else { b.texts.get(&format!("UI.Habs.Tier{}Base", d.max_hab_size)).map(plain).unwrap_or_else(|| size.into()) };
            b.space_text("Habitation", "maxHabSize", "Max hab size", size);
            b.space_number("Habitation", "habSites", "Hab sites", d.hab_sites.len() as f64, "");
            b.space_text("Habitation", "solarMultiplier", "Solar power", pct(system.solar_multiplier(body, None)));
            for r in Resource::ALL {
                if let Some(rating) = system.body_rating(body, r) {
                    b.space_resource("Resources", r, humanize(rating.name()), None);
                }
            }
            b.space_links("Moons", system.bodies.iter().filter(|m| system.parent(m).is_some_and(|p| p.id == id)).map(|m| m.id).collect());
            b.space_names("Hab sites", game::HAB_SITE, &d.hab_sites);
            b.space_names("Orbits", game::ORBIT, &d.orbits);
            b.space_links("Lagrange points", system.lagrange_points.iter().filter(|(_, p)| p.related_object == d.data_name).map(|(id, _)| *id).collect());
            if let Some(name) = record.get("effectToExplore").and_then(Value::as_str) {
                b.space_names("Explore effect", model::EFFECT, &[name.into()]);
            }
        }
        game::HAB_SITE => {
            let site: game::HabSite = model::parse(record);
            let system = System::load(view);
            if let Some(profile_id) = view.find(game::MINING_PROFILE, &site.mining_profile) {
                b.stats.tags.push(view.name(texts, profile_id));
                b.space_links("Mining profile", vec![profile_id]);
                if let (Some(body), Some(profile_record)) = (system.body(&site.parent_body), view.record(profile_id)) {
                    let profile = game::MiningProfile::from_record(profile_record);
                    let mining = system.site_mining(body, &profile);
                    for r in Resource::ALL {
                        let out = mining.output_month(r);
                        let rating = game::SiteRating::rate(
                            out.expected / game::DAYS_PER_MONTH,
                            profile.get(r).width,
                            out.min / game::DAYS_PER_MONTH,
                            system.global_cap_per_day(r),
                            false,
                        );
                        b.space_resource(
                            "Expected output / month",
                            r,
                            format!("{} ({}–{})", amount(out.expected), amount(out.min), amount(out.max)),
                            Some(humanize(rating.name())),
                        );
                    }
                }
            }
            if let Some(body) = system.body(&site.parent_body) {
                b.space_text("Location", "parentBodyName", "Body", view.name(texts, body.id));
                b.space_text("Location", "solarMultiplier", "Solar power", pct(system.solar_multiplier(body, Some(&site))));
                b.space_links("Body", vec![body.id]);
            }
            for (key, label, v) in [("latitude", "Latitude", site.latitude), ("longitude", "Longitude", site.longitude)] {
                match v {
                    Some(v) => b.space_number("Location", key, label, v, "°"),
                    None => b.space_text("Location", key, label, "Random".into()),
                }
            }
        }
        game::ORBIT => {
            let orbit: game::Orbit = model::parse(record);
            b.space_number("Orbit", "stationCapacity", "Station capacity", orbit.station_capacity as f64, "");
            b.space_number("Orbit", "irradiatedMultiplier", "Irradiation multiplier", orbit.irradiated_multiplier, "×");
            for spec in
                [f("eccentricity", "Eccentricity", Fmt::Num(""), "Orbit").always(), f("inclination_Deg", "Inclination", Fmt::Num("°"), "Orbit").always()]
            {
                if let Some(v) = map.get(spec.field) {
                    b.field(&spec, v, map);
                }
            }
            if let Some(parent) = view.find(game::SPACE_BODY, &orbit.barycenter_name) {
                b.space_text("Orbit", "barycenterName", "Parent", view.name(texts, parent));
                b.space_links("Parent body", vec![parent]);
            }
        }
        game::MINING_PROFILE => {
            let profile = game::MiningProfile::from_record(record);
            for r in Resource::ALL {
                let p = profile.get(r);
                b.space_resource(
                    "Mining profile / month",
                    r,
                    format!("{} / {} / {} / {}", num(p.mean), num(p.width), num(p.min), num(p.jump)),
                    Some("Mean / width / min / jump".into()),
                );
            }
            if profile.modify_by_size {
                b.stats.notes.push("Output scales with body size and density.".into());
            }
            b.space_links(
                "Hab sites",
                view.entities(game::HAB_SITE).filter(|&site| view.record(site).is_some_and(|r| r["miningProfileName"] == profile.data_name)).collect(),
            );
        }
        _ => return None,
    }
    Some(b.stats)
}

impl Builder<'_, '_> {
    fn space_text(&mut self, group: &str, field: &str, label: &str, value: String) {
        self.push(group, Stat { label: label.into(), value, detail: None, sentiment: "neutral".into(), icon: None, field: field.into() });
    }

    fn space_number(&mut self, group: &str, field: &str, label: &str, value: f64, unit: &str) {
        if value.is_finite() {
            self.space_text(group, field, label, format!("{}{unit}", num(value)));
        }
    }

    fn space_resource(&mut self, group: &str, resource: Resource, value: String, detail: Option<String>) {
        let key = if resource == Resource::Nobles { "nobleMetals" } else { resource.key() };
        let (label, icon) = self.material(key);
        self.push(group, Stat { label, value, detail, sentiment: "neutral".into(), icon, field: resource.key().into() });
    }

    fn space_links(&mut self, label: &str, ids: Vec<EntityId>) {
        if !ids.is_empty() {
            self.stats
                .links
                .push(StatLinks { label: label.into(), entities: ids.into_iter().map(|id| entity_ref(self.view, self.texts, id, self.spoilers)).collect() });
        }
    }

    fn space_names(&mut self, label: &str, template: &str, names: &[String]) {
        self.space_links(label, names.iter().filter_map(|n| self.view.find(template, n)).collect());
    }
}
