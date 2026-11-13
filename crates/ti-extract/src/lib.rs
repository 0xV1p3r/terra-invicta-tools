//! Extracts Terra Invicta game data into a stable, build-pinned dataset:
//! `<data dir>/<build id>/` plus `<data dir>/index.json`, read by `ti-data`.
//!
//! Port of the Python `extractor/` (`ti_extract`); the output format is the same
//! (`FORMAT_VERSION`), so builds written by either one load the same way.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

pub mod assets;
pub mod diff;
pub mod game;
pub mod jsonc;
pub mod locale;
pub mod output;
pub mod schema;
pub mod templates;

mod extract;

pub use game::{GameInstall, Layer, discover, find_game_dir, resolve_game_dir};

/// Bump on any incompatible change to the layout or content of an extracted build.
pub const FORMAT_VERSION: u32 = 1;

/// Environment variable naming the game install, overriding Steam detection.
pub const GAME_DIR_ENV: &str = "TI_GAME_DIR";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Terra Invicta install not found")]
    GameNotFound,
    #[error("not a Terra Invicta install: {0}")]
    NotAnInstall(String),
    #[error("cancelled")]
    Cancelled,
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Other(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Png,
    /// Lossless WebP.
    #[default]
    Webp,
}

impl ImageFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Webp => "webp",
        }
    }
}

/// Which images to export.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Images {
    /// No images (`--no-assets`).
    None,
    /// Every referenced image.
    #[default]
    All,
    /// Every referenced image except those in these bundles (lower-case names, `--skip-bundle`).
    Skip(Vec<String>),
}

/// Bundles holding the large artwork (about 1.5 GB of the 1.6 GB of images):
/// illustrations, councilor portraits and hab module renders.
pub const LARGE_ART_BUNDLES: &[&str] = &["illustrations", "characterportraits", "habmodules"];

#[derive(Debug, Clone)]
pub struct ExtractOptions {
    /// Data root: builds go to `<data_dir>/<build id>/`, the build list to `<data_dir>/index.json`.
    pub data_dir: PathBuf,
    /// Output directory name instead of the Steam build id. Must be a single non-empty
    /// normal path component: no leading `.`, `/`, `\\`, `:`, NUL, or control characters;
    /// `.` and `..` are forbidden.
    pub build_id: Option<String>,
    /// Replace an existing extraction of the same build. The old build is renamed to
    /// `.<id>.old` while the new one moves in and is deleted afterwards, so readers of the
    /// old build keep working until then (open files must allow deletion on Windows, which
    /// Rust's `File` does).
    pub force: bool,
    /// Treat template files that fail to parse as success. [`extract`] itself always writes
    /// the build and returns `Ok`, counting the failures in [`ExtractReport::parse_errors`]
    /// (details in `reports/issues.json`); callers decide what to do with them. The CLI exits
    /// with status 1 when this is off and `parse_errors > 0`, like the Python one.
    pub allow_parse_errors: bool,
    pub images: Images,
    pub image_format: ImageFormat,
}

impl ExtractOptions {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            build_id: None,
            force: false,
            allow_parse_errors: false,
            images: Images::All,
            image_format: ImageFormat::Webp,
        }
    }
}

/// Extraction phases, in order. `Images` may be skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    Templates,
    Locale,
    Schema,
    Images,
    Finish,
}

/// Progress reports passed to the `progress` callback of [`extract`].
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Progress {
    /// A phase started.
    Stage { stage: Stage },
    /// Work within the current phase: `done` of `total` units (files, images, ...), with an
    /// optional label such as the bundle being exported.
    Step { done: u64, total: u64, label: Option<String> },
    /// A human-readable log line (what the Python CLI prints).
    Log { message: String },
}

/// Outcome of a finished extraction.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractReport {
    /// Directory name of the build under the data dir.
    pub build_id: String,
    pub build_dir: PathBuf,
    /// `false` when the build already existed and `force` was off: nothing was written.
    pub extracted: bool,
    /// Entries in `reports/issues.json`.
    pub issues: usize,
    /// Template files that failed to parse. A build with parse errors is re-extracted on the
    /// next run even without `force`.
    pub parse_errors: usize,
    pub seconds: f64,
}

/// Extract `install` into `opts.data_dir`, then rewrite `<data_dir>/index.json`.
///
/// Writes into `<data_dir>/.<build id>.partial` and renames it into place at the end, so a
/// failed or cancelled run never leaves a half-written build. `cancel` is polled between
/// files and images; when set, the partial directory is removed and `Error::Cancelled` returned.
pub fn extract(install: &GameInstall, opts: &ExtractOptions, progress: &mut dyn FnMut(Progress), cancel: &AtomicBool) -> Result<ExtractReport> {
    extract::run(install, opts, progress, cancel)
}

/// Rewrite `<data_dir>/index.json` from the complete builds (those with a `manifest.json`).
pub fn write_index(data_dir: &Path) -> Result<()> {
    extract::write_index(data_dir)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::{Path, PathBuf};

    /// A temporary directory removed on drop.
    pub struct TempDir(tempfile::TempDir);

    impl TempDir {
        pub fn new() -> Self {
            Self(tempfile::tempdir().expect("temp dir"))
        }

        pub fn path(&self) -> &Path {
            self.0.path()
        }
    }

    /// Write `text` to `root/relative`, creating parent directories.
    pub fn write(root: &Path, relative: &str, text: &str) -> PathBuf {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
}
