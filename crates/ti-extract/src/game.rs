//! Locating the game install and reading its data layers (port of `game.py`).

use std::path::{Path, PathBuf};

use crate::Result;

/// Steam app id of Terra Invicta.
pub const APP_ID: &str = "1176470";

/// A folder of template JSON files: the base game or one DLC folder.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Layer {
    /// `base`, or derived from the DLC folder (see `layer_id` in `game.py`).
    pub id: String,
    /// `base` or `dlc`.
    pub kind: String,
    pub templates_dir: PathBuf,
    pub dlc: Option<String>,
    pub folder: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInstall {
    pub root: PathBuf,
    /// Game version from the game files, e.g. `1.0.53b`.
    pub version: Option<String>,
    /// Steam build id from `appmanifest_<appid>.acf`; `None` outside Steam.
    pub build_id: Option<String>,
    /// Unix time of the last Steam update.
    pub last_updated: Option<i64>,
    pub layers: Vec<Layer>,
    pub bundle_dirs: Vec<PathBuf>,
}

impl GameInstall {
    pub fn streaming_assets(&self) -> PathBuf {
        self.root.join("TerraInvicta_Data/StreamingAssets")
    }

    /// Localization folder names, sorted (`en`, `deu`, ...).
    pub fn languages(&self) -> Vec<String> {
        todo!()
    }
}

/// The first Steam library containing Terra Invicta.
pub fn find_game_dir() -> Option<PathBuf> {
    todo!()
}

/// Read an install (errors if `root` has no `Templates` directory).
pub fn discover(root: &Path) -> Result<GameInstall> {
    todo!("{}", root.display())
}

/// `explicit`, else `TI_GAME_DIR`, else `find_game_dir`.
pub fn resolve_game_dir(explicit: Option<&Path>) -> Result<PathBuf> {
    todo!("{explicit:?}")
}
