//! Image export from the game's asset bundles (port of `assets.py`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use serde_json::Value;

use crate::{ImageFormat, Progress, Result};

/// TextMeshPro sprite asset behind `<sprite name="…">` tags in localized text.
pub const INLINE_SPRITE_BUNDLE: &str = "ui";

#[derive(Debug, Clone)]
pub struct Bundle {
    /// Lower-case bundle name.
    pub name: String,
    pub path: PathBuf,
    /// Lower-case image stem -> original stem.
    pub images: BTreeMap<String, String>,
}

/// An earlier build's `assets/` dir and its parsed `index.json`, for reusing unchanged bundles.
#[derive(Debug, Clone)]
pub struct Previous {
    pub dir: PathBuf,
    pub index: Value,
}

/// `resources`, `bundles` and `missing` of `assets/index.json`.
#[derive(Debug, Clone, Default)]
pub struct AssetResult {
    pub resources: BTreeMap<String, Value>,
    pub bundles: BTreeMap<String, Value>,
    pub missing: Vec<String>,
}

pub struct AssetOptions<'a> {
    /// Lower-case bundle names not to export.
    pub skip_bundles: &'a [String],
    /// Newest first.
    pub previous: &'a [Previous],
    pub image_format: ImageFormat,
    pub progress: &'a mut dyn FnMut(Progress),
    pub cancel: &'a AtomicBool,
}

/// Bundles listed in the `*.manifest` files of `bundle_dirs`, by lower-case name.
pub fn read_bundle_manifests(bundle_dirs: &[PathBuf]) -> Result<BTreeMap<String, Bundle>> {
    todo!("{bundle_dirs:?}")
}

/// Image resources referenced by template strings, plus the hard-coded extras.
pub fn collect_resources<'s>(strings: impl IntoIterator<Item = &'s str>, bundles: &BTreeMap<String, Bundle>) -> BTreeSet<String> {
    let _ = (strings.into_iter().count(), bundles);
    todo!()
}

/// Export `resources` to `out_dir`.
pub fn extract(resources: &BTreeSet<String>, bundles: &BTreeMap<String, Bundle>, out_dir: &Path, opts: &mut AssetOptions<'_>) -> Result<AssetResult> {
    todo!("{resources:?} {bundles:?} {out_dir:?} {:?}", opts.image_format)
}

/// Export the inline sprite glyphs to `sprites/`; the `sprites` value of `assets/index.json`,
/// or `None` without the `ui` bundle.
pub fn extract_inline_sprites(bundles: &BTreeMap<String, Bundle>, out_dir: &Path, opts: &mut AssetOptions<'_>) -> Result<Option<Value>> {
    todo!("{bundles:?} {out_dir:?} {:?}", opts.image_format)
}
