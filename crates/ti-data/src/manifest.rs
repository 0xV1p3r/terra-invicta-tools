//! `manifest.json`, `index.json`, `schema.json` and `assets/index.json` as written by `ti-extract`.

use std::collections::BTreeMap;

use serde::Deserialize;

/// Data format version this library understands (see `ti_extract.FORMAT_VERSION`).
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format_version: u32,
    pub extractor: ExtractorInfo,
    pub extracted_at: String,
    pub game: GameInfo,
    pub layers: Vec<LayerInfo>,
    pub languages: Vec<String>,
    pub has_assets: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractorInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    pub app_id: String,
    pub build_id: String,
    pub last_updated: Option<i64>,
    /// Version string shown by the game, e.g. `1.0.53b`.
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LayerInfo {
    pub id: String,
    /// `"base"` or `"dlc"`.
    pub kind: String,
    pub dlc: Option<String>,
    /// Source directory relative to `DLC_Content`, for DLC layers.
    pub folder: Option<String>,
    pub templates: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataIndex {
    pub latest: Option<String>,
    pub builds: Vec<BuildEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildEntry {
    pub dir: String,
    pub build_id: String,
    pub last_updated: Option<i64>,
    pub format_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Schema {
    pub ref_fields: Vec<RefField>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RefField {
    pub template: String,
    pub path: String,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct AssetIndex {
    #[serde(default)]
    pub resources: BTreeMap<String, AssetEntry>,
    /// Glyphs of the game's inline sprite sheet (`<sprite name="metal">`), by name.
    #[serde(default)]
    pub sprites: SpriteIndex,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct SpriteIndex {
    #[serde(default)]
    pub exported: BTreeMap<String, AssetEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetEntry {
    /// Path relative to the build's `assets/` directory.
    pub file: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub variants: Vec<String>,
}
