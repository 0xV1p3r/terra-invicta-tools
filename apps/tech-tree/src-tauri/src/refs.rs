//! Entity keys (`"<Template>:<dataName>"`) and `EntityRef`s.

use ti_data::model::{Kind, PartKind};
use ti_data::{EntityId, Texts, View};

use crate::dto::EntityRef;
use crate::error::{CmdError, CmdResult};

pub fn key(view: &View<'_>, id: EntityId) -> String {
    let e = view.dataset().entity(id);
    format!("{}:{}", e.template, e.data_name)
}

/// Entity of a key, if it exists in the dataset (any scope).
pub fn parse_key(view: &View<'_>, key: &str) -> CmdResult<EntityId> {
    let (template, data_name) =
        key.split_once(':').ok_or_else(|| CmdError::msg(format!("Malformed entity key {key:?}")))?;
    view.dataset().find(template, data_name).ok_or_else(|| CmdError::msg(format!("Unknown entity {key:?}")))
}

pub fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Tech => "tech",
        Kind::Project => "project",
        Kind::Effect => "effect",
        Kind::HabModule => "habModule",
        Kind::ShipPart(_) => "shipPart",
        Kind::Org => "org",
        Kind::Trait => "trait",
        Kind::Faction => "faction",
        Kind::Objective => "objective",
        Kind::Mission => "mission",
        Kind::Codex => "codex",
        Kind::Other => "other",
    }
}

/// lowerCamel `PartKind` name (`drive`, `powerPlant`, `magneticGun`, …).
pub fn part_kind_name(kind: PartKind) -> String {
    let name = format!("{kind:?}");
    let mut chars = name.chars();
    chars.next().map(|c| c.to_ascii_lowercase().to_string() + chars.as_str()).unwrap_or_default()
}

/// `techCategory` of a tech/project record in this scope.
pub fn category(view: &View<'_>, id: EntityId) -> Option<String> {
    if !view.kind(id).is_research() {
        return None;
    }
    view.record(id)?.get("techCategory")?.as_str().map(str::to_string)
}

/// Asset file path (relative to the build's assets dir) of a resource string.
pub fn asset_file(view: &View<'_>, resource: &str) -> Option<String> {
    view.dataset().asset(resource).map(|a| a.file.clone())
}

pub fn entity_ref(view: &View<'_>, texts: &Texts, id: EntityId, spoilers: &std::collections::HashSet<EntityId>) -> EntityRef {
    let e = view.dataset().entity(id);
    let kind = view.kind(id);
    EntityRef {
        spoiler: spoilers.contains(&id),
        key: key(view, id),
        template: e.template.to_string(),
        data_name: e.data_name.to_string(),
        name: view.name(texts, id),
        kind: kind_name(kind).into(),
        subtype: match kind {
            Kind::ShipPart(p) => Some(part_kind_name(p)),
            _ => None,
        },
        category: category(view, id),
        icon: view.icon_resource(id).and_then(|r| asset_file(view, &r)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_kind_names() {
        assert_eq!(part_kind_name(PartKind::Drive), "drive");
        assert_eq!(part_kind_name(PartKind::PowerPlant), "powerPlant");
        assert_eq!(part_kind_name(PartKind::MagneticGun), "magneticGun");
    }
}
