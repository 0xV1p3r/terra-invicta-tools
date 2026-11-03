//! Application state: the loaded dataset plus per-language caches.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use ti_data::{DataRoot, Dataset, SearchIndex, Texts};

use crate::error::{CmdError, CmdResult};

/// Environment variable overriding the data directory (the one containing `index.json`).
pub const DATA_DIR_ENV: &str = "TI_DATA_DIR";

/// A loaded build with lazily filled caches. Cheap to share via `Arc`.
pub struct Backend {
    pub ds: Arc<Dataset>,
    /// Data root (directory with `index.json`).
    pub data_dir: PathBuf,
    /// Canonical `<data root>/<build>/assets`, if it exists on disk.
    pub assets_dir: Option<PathBuf>,
    texts: Mutex<HashMap<String, Arc<Texts>>>,
    search: Mutex<HashMap<String, Arc<SearchIndex>>>,
}

impl Backend {
    pub fn open(data_dir: impl Into<PathBuf>) -> CmdResult<Self> {
        let data_dir = data_dir.into();
        let ds = DataRoot::open(&data_dir)?.latest()?;
        Ok(Self::new(ds, data_dir))
    }

    pub fn new(ds: Dataset, data_dir: PathBuf) -> Self {
        let assets_dir = ds.asset_path("").and_then(|p| p.canonicalize().ok()).filter(|p| p.is_dir());
        Self { ds: Arc::new(ds), data_dir, assets_dir, texts: Mutex::default(), search: Mutex::default() }
    }

    /// Language code actually used for `lang`: itself if part of the build, else `en`.
    pub fn language<'a>(&self, lang: &'a str) -> &'a str {
        if self.ds.languages().iter().any(|l| l == lang) { lang } else { "en" }
    }

    /// Texts of a language (with English fallback), loaded on first use.
    pub fn texts(&self, lang: &str) -> CmdResult<Arc<Texts>> {
        let lang = self.language(lang);
        let mut cache = self.texts.lock().map_err(|_| CmdError::msg("text cache poisoned"))?;
        if let Some(t) = cache.get(lang) {
            return Ok(t.clone());
        }
        let t = Arc::new(Texts::load(&self.ds, lang)?);
        cache.insert(lang.to_string(), t.clone());
        Ok(t)
    }

    /// Search index of a language, built on first use (~165 ms) and cached.
    pub fn search_index(&self, lang: &str) -> CmdResult<Arc<SearchIndex>> {
        let lang = self.language(lang);
        let texts = self.texts(lang)?;
        // Held while building so concurrent first searches do not build twice.
        let mut cache = self.search.lock().map_err(|_| CmdError::msg("search cache poisoned"))?;
        if let Some(i) = cache.get(lang) {
            return Ok(i.clone());
        }
        let index = Arc::new(SearchIndex::build(&self.ds, &texts));
        cache.insert(lang.to_string(), index.clone());
        Ok(index)
    }
}

/// Managed Tauri state: the backend, or why it could not be loaded. Loading runs
/// on a background thread so the window opens immediately; users block until done.
#[derive(Clone, Default)]
pub struct AppState(Arc<OnceLock<Result<Arc<Backend>, String>>>);

impl AppState {
    /// Start locating and loading the data; failures are kept and reported by every command.
    pub fn load_in_background() -> Self {
        let state = Self::default();
        let cell = state.0.clone();
        std::thread::spawn(move || {
            let _ = cell.set(load());
        });
        state
    }

    /// The backend once loaded (blocks while loading; call off the main thread).
    pub fn backend(&self) -> CmdResult<Arc<Backend>> {
        self.0.wait().clone().map_err(CmdError::Other)
    }
}

fn load() -> Result<Arc<Backend>, String> {
    let started = std::time::Instant::now();
    let result = match find_data_dir() {
        Some(dir) => Backend::open(&dir)
            .map(Arc::new)
            .map_err(|e| format!("Could not load game data from {}: {e}", dir.display())),
        None => Err(format!(
            "No extracted game data found (looked for data/index.json above the current and the \
             executable's directory). Run ti-extract or set {DATA_DIR_ENV}."
        )),
    };
    match &result {
        Ok(b) => eprintln!("ti-tech-tree: loaded build {} from {} in {:?}", b.ds.build_id(), b.data_dir.display(), started.elapsed()),
        Err(e) => eprintln!("ti-tech-tree: {e}"),
    }
    result
}

/// `TI_DATA_DIR`, else the first `data/` with an `index.json` above the current dir or the executable.
pub fn find_data_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(DATA_DIR_ENV).filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    let starts = [std::env::current_dir().ok(), std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf))];
    starts.into_iter().flatten().find_map(|start| {
        start.ancestors().map(|d| d.join("data")).find(|d| d.join("index.json").is_file())
    })
}
