//! Application state: the loaded dataset plus per-language caches, the persisted
//! data settings, and switching the data folder or build at runtime.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

use serde::{Deserialize, Serialize};
use ti_data::{DataRoot, Dataset, SearchIndex, Texts};

use crate::dto::DataDirSource;
use crate::error::{CmdError, CmdResult};

/// Environment variable overriding the data directory (the one containing `index.json`).
pub const DATA_DIR_ENV: &str = "TI_DATA_DIR";

/// Default data root inside the app data directory.
pub const DATA_SUBDIR: &str = "data";

/// Name of the settings file in the app config directory.
pub const CONFIG_FILE: &str = "settings.json";

/// Language whose texts and search index are built right after loading.
const PREWARM_LANGUAGE: &str = "en";

/// Sorted wiki entry lists kept per (scope, language, template, spoiler).
const WIKI_CACHE_SIZE: usize = 4;

/// Persisted data settings (`<app config dir>/settings.json`), e.g.
/// `{"dataDir": "/home/me/ti-data", "build": "25216249", "buildRoot": "/home/me/ti-data"}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataConfig {
    /// Data root chosen in the app (the directory containing `index.json`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<PathBuf>,
    /// Pinned build id; `None` = the latest build of the data root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    /// Data root the pinned build belongs to; the pin is ignored for any other root
    /// (e.g. when `TI_DATA_DIR` points elsewhere).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_root: Option<PathBuf>,
}

impl DataConfig {
    /// The settings in `path`; defaults when the file is missing or unreadable.
    pub fn load(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                eprintln!("ti-tech-tree: ignoring invalid settings {}: {e}", path.display());
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// The pinned build, if it was pinned for the data root `dir`.
    pub fn build_for(&self, dir: &Path) -> Option<&str> {
        let same = |root: &PathBuf| root == dir || root.canonicalize().is_ok_and(|r| dir.canonicalize().is_ok_and(|d| r == d));
        self.build.as_deref().filter(|_| self.build_root.as_ref().is_some_and(same))
    }

    /// Write the settings (via a temporary file, so a crash cannot leave half a file).
    pub fn save(&self, path: &Path) -> CmdResult<()> {
        let fail = |e: std::io::Error| CmdError::msg(format!("Could not save settings to {}: {e}", path.display()));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(fail)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).expect("settings serialize")).map_err(fail)?;
        std::fs::rename(&tmp, path).map_err(fail)
    }
}

/// Where the data may come from; `from_env` for the app, explicit values in tests.
#[derive(Debug, Clone)]
pub struct Locator {
    /// Value of `TI_DATA_DIR`, if set and nonempty.
    pub env: Option<PathBuf>,
    /// Settings file; `None` = settings are neither read nor saved.
    pub config_path: Option<PathBuf>,
    /// Default data root, `<app data dir>/data`: where extraction writes when no data is loaded.
    pub default_dir: Option<PathBuf>,
    /// Fallback search for a data root.
    pub search: fn() -> Option<PathBuf>,
}

impl Locator {
    pub fn from_env(config_path: Option<PathBuf>, default_dir: Option<PathBuf>) -> Self {
        let env = std::env::var_os(DATA_DIR_ENV).filter(|d| !d.is_empty()).map(PathBuf::from);
        Self {
            env,
            config_path,
            default_dir,
            search: find_ancestor_data_dir,
        }
    }

    pub fn config(&self) -> DataConfig {
        self.config_path.as_deref().map(DataConfig::load).unwrap_or_default()
    }

    fn save(&self, config: &DataConfig) -> CmdResult<()> {
        match &self.config_path {
            Some(path) => config.save(path),
            None => Ok(()),
        }
    }
}

/// `TI_DATA_DIR`, else the persisted data folder, else the app's default data root (both only
/// if they have an `index.json`), else `search`.
pub fn resolve_data_dir(
    env: Option<&Path>,
    config: &DataConfig,
    default_dir: Option<&Path>,
    search: impl FnOnce() -> Option<PathBuf>,
) -> Option<(PathBuf, DataDirSource)> {
    if let Some(dir) = env {
        return Some((dir.to_path_buf(), DataDirSource::Env));
    }
    if let Some(dir) = config.data_dir.as_ref().filter(|d| d.join("index.json").is_file()) {
        return Some((dir.clone(), DataDirSource::Settings));
    }
    if let Some(dir) = default_dir.filter(|d| d.join("index.json").is_file()) {
        return Some((dir.to_path_buf(), DataDirSource::Default));
    }
    search().map(|dir| (dir, DataDirSource::Found))
}

/// A few recently used values, most recent last.
struct RecentCache<K, V> {
    entries: Vec<(K, V)>,
    capacity: usize,
}

impl<K: Eq, V: Clone> RecentCache<K, V> {
    fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    fn get_or_try_insert<E>(&mut self, key: K, build: impl FnOnce() -> Result<V, E>) -> Result<V, E> {
        if let Some(i) = self.entries.iter().position(|(k, _)| *k == key) {
            let entry = self.entries.remove(i);
            self.entries.push(entry);
        } else {
            let value = build()?;
            if self.entries.len() >= self.capacity {
                self.entries.remove(0);
            }
            self.entries.push((key, value));
        }
        Ok(self.entries.last().expect("just inserted").1.clone())
    }
}

/// Key of a cached wiki entry list: scope, language, template, spoiler section.
pub type WikiKey = (ti_data::Scope, String, String, bool);

/// One wiki list row plus its lowercased filter text.
pub struct WikiRow {
    pub haystack: String,
    pub entry: crate::dto::WikiEntry,
}

/// A loaded build with lazily filled caches. Cheap to share via `Arc`.
pub struct Backend {
    pub ds: Arc<Dataset>,
    /// Data root (directory with `index.json`).
    pub data_dir: PathBuf,
    pub data_dir_source: DataDirSource,
    /// The data root's build list as of loading.
    pub root: DataRoot,
    /// Build id the settings pin (`None` = latest).
    pub pinned_build: Option<String>,
    /// Canonical `<data root>/<build>/assets`, if it exists on disk.
    pub assets_dir: Option<PathBuf>,
    texts: Mutex<HashMap<String, Arc<Texts>>>,
    spoilers: Mutex<HashMap<ti_data::Scope, Arc<HashSet<ti_data::EntityId>>>>,
    search: Mutex<HashMap<String, Arc<SearchIndex>>>,
    /// Languages whose search index is built or being built.
    warming: Mutex<HashSet<String>>,
    wiki: Mutex<RecentCache<WikiKey, Arc<Vec<WikiRow>>>>,
}

impl Backend {
    /// The latest build of a data root.
    #[cfg(test)]
    pub fn open(data_dir: impl Into<PathBuf>) -> CmdResult<Self> {
        Self::load(data_dir.into(), None, DataDirSource::Found)
    }

    /// Build `build` (`None` = latest) of a data root.
    pub fn load(data_dir: PathBuf, build: Option<&str>, data_dir_source: DataDirSource) -> CmdResult<Self> {
        let root = DataRoot::open(&data_dir)?;
        let ds = match build {
            Some(id) => root.build(id)?,
            None => root.latest()?,
        };
        let assets_dir = ds.asset_path("").and_then(|p| p.canonicalize().ok()).filter(|p| p.is_dir());
        Ok(Self {
            ds: Arc::new(ds),
            data_dir,
            data_dir_source,
            root,
            pinned_build: build.map(str::to_string),
            assets_dir,
            texts: Mutex::default(),
            search: Mutex::default(),
            spoilers: Mutex::default(),
            warming: Mutex::default(),
            wiki: Mutex::new(RecentCache::new(WIKI_CACHE_SIZE)),
        })
    }

    /// Build id of the data root's latest build.
    pub fn latest_build_id(&self) -> Option<&str> {
        let latest = self.root.index.latest.as_deref()?;
        Some(self.root.index.builds.iter().find(|b| b.dir == latest).map_or(latest, |b| b.build_id.as_str()))
    }

    pub fn spoilers(&self, view: &ti_data::View<'_>) -> CmdResult<Arc<HashSet<ti_data::EntityId>>> {
        // "Can any human faction get it?" ignores the selected faction: classify per scenario only.
        let scope = ti_data::Scope { faction: None, ..view.scope() };
        let mut cache = self.spoilers.lock().map_err(|_| CmdError::msg("spoiler cache poisoned"))?;
        Ok(cache
            .entry(scope)
            .or_insert_with(|| Arc::new(crate::spoilers::classify(&self.ds.view(scope))))
            .clone())
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

    /// Search index of a language, built on first use (~300 ms) and cached.
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

    /// Mark a language's search index as wanted; true the first time.
    fn start_warming(&self, lang: &str) -> bool {
        let lang = self.language(lang);
        self.warming.lock().is_ok_and(|mut w| w.insert(lang.to_string()))
    }

    /// Build a language's texts and search index on a background thread, once.
    pub fn warm_search(self: &Arc<Self>, lang: &str) {
        if self.start_warming(lang) {
            let (b, lang) = (self.clone(), lang.to_string());
            std::thread::spawn(move || b.warm_now(&lang));
        }
    }

    fn warm_now(&self, lang: &str) {
        let started = std::time::Instant::now();
        match self.search_index(lang) {
            Ok(_) => eprintln!("ti-tech-tree: search index ({}) ready in {:?}", self.language(lang), started.elapsed()),
            Err(e) => eprintln!("ti-tech-tree: search index ({lang}) failed: {e}"),
        }
    }

    /// Cached wiki rows for `key`, built by `build` on a miss.
    pub fn wiki_rows(&self, key: WikiKey, build: impl FnOnce() -> CmdResult<Vec<WikiRow>>) -> CmdResult<Arc<Vec<WikiRow>>> {
        // Held while building: the next page of the same list waits instead of building again.
        let mut cache = self.wiki.lock().map_err(|_| CmdError::msg("wiki cache poisoned"))?;
        cache.get_or_try_insert(key, || build().map(Arc::new))
    }
}

/// The current backend, or why none could be loaded.
type Loaded = Result<Arc<Backend>, String>;

struct Shared {
    locator: Locator,
    /// `None` until the initial load finishes.
    current: Mutex<Option<Loaded>>,
    loaded: Condvar,
    /// Serializes reloads.
    reloading: Mutex<()>,
}

/// Managed Tauri state: the backend, or why it could not be loaded. Loading runs
/// on a background thread so the window opens immediately; users block until done.
/// `set_build` / `set_data_dir` swap the backend; commands already running keep theirs.
#[derive(Clone)]
pub struct AppState(Arc<Shared>);

impl AppState {
    fn new(locator: Locator) -> Self {
        Self(Arc::new(Shared {
            locator,
            current: Mutex::new(None),
            loaded: Condvar::new(),
            reloading: Mutex::new(()),
        }))
    }

    /// Start locating and loading the data; failures are kept and reported by every command.
    /// Then build the default search index on the same thread.
    pub fn load_in_background(locator: Locator) -> Self {
        let state = Self::new(locator);
        let loader = state.clone();
        std::thread::spawn(move || {
            let loaded = initial_load(&loader.0.locator);
            let backend = loaded.as_ref().ok().cloned();
            // A folder chosen while this was loading wins: its result is already published.
            if !loader.publish_initial(loaded) {
                return;
            }
            if let Some(b) = backend.filter(|b| b.start_warming(PREWARM_LANGUAGE)) {
                b.warm_now(PREWARM_LANGUAGE);
            }
        });
        state
    }

    /// Publish the initial load unless a reload already published; true if it was used.
    fn publish_initial(&self, loaded: Loaded) -> bool {
        let mut current = self.0.current.lock().unwrap_or_else(|e| e.into_inner());
        let first = current.is_none();
        if first {
            *current = Some(loaded);
            self.0.loaded.notify_all();
        }
        first
    }

    fn publish(&self, loaded: Loaded) {
        *self.0.current.lock().unwrap_or_else(|e| e.into_inner()) = Some(loaded);
        self.0.loaded.notify_all();
    }

    /// The backend once loaded (blocks while loading; call off the main thread).
    pub fn backend(&self) -> CmdResult<Arc<Backend>> {
        let current = self.0.current.lock().unwrap_or_else(|e| e.into_inner());
        let current = self.0.loaded.wait_while(current, |c| c.is_none()).unwrap_or_else(|e| e.into_inner());
        current.clone().expect("loaded").map_err(CmdError::Other)
    }

    /// Load another build (`None` = latest) of the current data root and pin it.
    pub fn set_build(&self, build: Option<String>) -> CmdResult<()> {
        let _reload = self.0.reloading.lock().unwrap_or_else(|e| e.into_inner());
        let current = self
            .backend()
            .map_err(|_| CmdError::msg("No game data is loaded; choose a data folder first."))?;
        let next = Backend::load(current.data_dir.clone(), build.as_deref(), current.data_dir_source)?;
        let build_root = build.is_some().then(|| current.data_dir.clone());
        self.0.locator.save(&DataConfig {
            build,
            build_root,
            ..self.0.locator.config()
        })?;
        self.swap(next);
        Ok(())
    }

    /// Load the latest build of another data root and remember the folder (clears a pinned build).
    pub fn set_data_dir(&self, dir: PathBuf) -> CmdResult<()> {
        let _reload = self.0.reloading.lock().unwrap_or_else(|e| e.into_inner());
        // Absolute and symlink-free, so the saved folder does not depend on the working directory.
        let dir = dir.canonicalize().unwrap_or(dir);
        if !dir.join("index.json").is_file() {
            return Err(CmdError::msg(format!(
                "{} has no index.json. Choose the data folder written by ti-extract: it contains index.json and one folder per game build.",
                dir.display()
            )));
        }
        let next = Backend::load(dir.clone(), None, DataDirSource::Settings)
            .map_err(|e| CmdError::msg(format!("Could not load game data from {}: {e}", dir.display())))?;
        self.0.locator.save(&DataConfig {
            data_dir: Some(dir),
            build: None,
            build_root: None,
        })?;
        self.swap(next);
        Ok(())
    }

    /// Data root an extraction writes to: `TI_DATA_DIR`, else the loaded data root, else the
    /// app's default data root.
    pub fn extract_target(&self) -> CmdResult<PathBuf> {
        let locator = &self.0.locator;
        if let Some(dir) = &locator.env {
            return Ok(dir.clone());
        }
        if let Ok(b) = self.backend() {
            return Ok(b.data_dir.clone());
        }
        locator
            .default_dir
            .clone()
            .ok_or_else(|| CmdError::msg("Could not determine the app data folder to extract into."))
    }

    /// Load build `build` (id or directory) of a data root an extraction just wrote to. It is
    /// pinned unless it is the root's latest build (e.g. the game was rolled back). `TI_DATA_DIR`
    /// overrides the settings anyway, so only the pin is saved for it; any other folder is remembered.
    pub fn load_extracted(&self, dir: PathBuf, build: &str) -> CmdResult<()> {
        let _reload = self.0.reloading.lock().unwrap_or_else(|e| e.into_inner());
        let env = self.0.locator.env.as_deref() == Some(dir.as_path());
        let (dir, source) = if env {
            (dir, DataDirSource::Env)
        } else {
            (dir.canonicalize().unwrap_or(dir), DataDirSource::Settings)
        };
        let mut next = Backend::load(dir.clone(), Some(build), source)?;
        let latest = next.latest_build_id() == Some(next.ds.build_id());
        next.pinned_build = (!latest).then(|| next.ds.build_id().to_string());
        let config = self.0.locator.config();
        self.0.locator.save(&DataConfig {
            data_dir: if env { config.data_dir } else { Some(dir.clone()) },
            build: next.pinned_build.clone(),
            build_root: next.pinned_build.is_some().then_some(dir),
        })?;
        self.swap(next);
        Ok(())
    }

    fn swap(&self, next: Backend) {
        let next = Arc::new(next);
        eprintln!("ti-tech-tree: switched to build {} from {}", next.ds.build_id(), next.data_dir.display());
        self.publish(Ok(next.clone()));
        next.warm_search(PREWARM_LANGUAGE);
    }
}

fn initial_load(locator: &Locator) -> Loaded {
    let started = std::time::Instant::now();
    let config = locator.config();
    let result = match resolve_data_dir(locator.env.as_deref(), &config, locator.default_dir.as_deref(), locator.search) {
        Some((dir, source)) => load_pinned(locator, &config, &dir, source)
            .map(Arc::new)
            .map_err(|e| format!("Could not load game data from {}: {e}", dir.display())),
        None => Err(format!(
            "No extracted game data found. Extract it from your Terra Invicta install, choose a data \
             folder, or set {DATA_DIR_ENV}."
        )),
    };
    match &result {
        Ok(b) => eprintln!(
            "ti-tech-tree: loaded build {} from {} in {:?}",
            b.ds.build_id(),
            b.data_dir.display(),
            started.elapsed()
        ),
        Err(e) => eprintln!("ti-tech-tree: {e}"),
    }
    result
}

/// The build pinned for `dir`, or the latest one. A pin whose build is no longer in the
/// data root is dropped from the settings, so it cannot come back unannounced.
fn load_pinned(locator: &Locator, config: &DataConfig, dir: &Path, source: DataDirSource) -> CmdResult<Backend> {
    match Backend::load(dir.to_path_buf(), config.build_for(dir), source) {
        Err(CmdError::Data(ti_data::Error::UnknownBuild(id))) => {
            eprintln!("ti-tech-tree: pinned build {id} is gone; loading the latest build");
            if let Err(e) = locator.save(&DataConfig {
                build: None,
                build_root: None,
                ..config.clone()
            }) {
                eprintln!("ti-tech-tree: {e}");
            }
            Backend::load(dir.to_path_buf(), None, source)
        }
        result => result,
    }
}

/// The first `data/` with an `index.json` above the current dir or the executable.
pub fn find_ancestor_data_dir() -> Option<PathBuf> {
    let starts = [
        std::env::current_dir().ok(),
        std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)),
    ];
    starts
        .into_iter()
        .flatten()
        .find_map(|start| start.ancestors().map(|d| d.join("data")).find(|d| d.join("index.json").is_file()))
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::json;

    use super::*;

    /// A fresh directory under the system temp dir, removed on drop.
    pub(crate) struct TempDir(pub(crate) PathBuf);

    impl TempDir {
        pub(crate) fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("ti-tech-tree-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir.canonicalize().unwrap())
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write(path: &Path, value: serde_json::Value) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
    }

    /// A data root with two empty builds, `1` (dir `b1`) and the latest `2` (dir `b2`).
    pub(crate) fn data_root(dir: &Path) {
        let entry = |id: &str| json!({"dir": format!("b{id}"), "buildId": id, "lastUpdated": 1, "formatVersion": ti_data::manifest::FORMAT_VERSION});
        write(&dir.join("index.json"), json!({"latest": "b2", "builds": [entry("1"), entry("2")]}));
        for id in ["1", "2"] {
            let build = dir.join(format!("b{id}"));
            write(
                &build.join("manifest.json"),
                json!({
                    "formatVersion": ti_data::manifest::FORMAT_VERSION,
                    "extractor": {"name": "test", "version": "0"},
                    "extractedAt": "",
                    "game": {"appId": "0", "buildId": id, "lastUpdated": 1, "version": format!("1.0.{id}")},
                    "layers": [{"id": "base", "kind": "base", "dlc": null, "folder": null, "templates": []}],
                    "languages": ["en"],
                    "hasAssets": false,
                }),
            );
            write(&build.join("schema.json"), json!({"refFields": []}));
            write(&build.join("locale/en/strings.json"), json!({}));
        }
    }

    fn no_search() -> Option<PathBuf> {
        None
    }

    pub(crate) fn locator(env: Option<PathBuf>, config_path: PathBuf) -> Locator {
        Locator {
            env,
            config_path: Some(config_path),
            default_dir: None,
            search: no_search,
        }
    }

    pub(crate) fn loaded_build(state: &AppState) -> String {
        state.backend().unwrap().ds.build_id().to_string()
    }

    #[test]
    fn data_dir_resolution_order() {
        let tmp = TempDir::new("resolve");
        let (saved, default) = (tmp.0.join("saved"), tmp.0.join("default"));
        data_root(&saved);
        data_root(&default);
        let found = || Some(PathBuf::from("/found"));
        let config = DataConfig {
            data_dir: Some(saved.clone()),
            ..Default::default()
        };
        let env = PathBuf::from("/env");
        let resolve = |env: Option<&Path>, config: &DataConfig, default: Option<&Path>| resolve_data_dir(env, config, default, found);

        // TI_DATA_DIR, then the settings folder, then the app's default root, then the search.
        assert_eq!(resolve(Some(&env), &config, Some(&default)), Some((env.clone(), DataDirSource::Env)));
        assert_eq!(resolve(None, &config, Some(&default)), Some((saved.clone(), DataDirSource::Settings)));
        assert_eq!(
            resolve(None, &DataConfig::default(), Some(&default)),
            Some((default.clone(), DataDirSource::Default))
        );
        assert_eq!(
            resolve(None, &DataConfig::default(), None),
            Some((PathBuf::from("/found"), DataDirSource::Found))
        );
        // A saved folder that lost its index.json falls through to the default root, then the search.
        let gone = DataConfig {
            data_dir: Some(tmp.0.join("gone")),
            ..Default::default()
        };
        assert_eq!(resolve(None, &gone, Some(&default)), Some((default.clone(), DataDirSource::Default)));
        // So does a default root without index.json (nothing extracted yet).
        let empty = tmp.0.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert_eq!(resolve(None, &gone, Some(&empty)), Some((PathBuf::from("/found"), DataDirSource::Found)));
        assert_eq!(resolve_data_dir(None, &gone, Some(&empty), no_search), None);
    }

    #[test]
    fn startup_falls_back_to_the_default_root() {
        let tmp = TempDir::new("default");
        let default = tmp.0.join("default");
        data_root(&default);
        let locator = Locator {
            default_dir: Some(default.clone()),
            ..locator(None, tmp.0.join("settings.json"))
        };
        let state = AppState::load_in_background(locator);
        let b = state.backend().unwrap();
        assert_eq!((b.data_dir.as_path(), b.data_dir_source), (default.as_path(), DataDirSource::Default));
        assert_eq!(state.extract_target().unwrap(), default);
    }

    #[test]
    fn extraction_target_and_reload() {
        let tmp = TempDir::new("target");
        let (env, default) = (tmp.0.join("env"), tmp.0.join("default"));
        let config = tmp.0.join("settings.json");
        let with_default = |env: Option<PathBuf>| Locator {
            default_dir: Some(default.clone()),
            ..locator(env, config.clone())
        };

        // Nothing loaded: extract into the default root, then load and remember it.
        let state = AppState::load_in_background(with_default(None));
        assert!(state.backend().is_err());
        assert_eq!(state.extract_target().unwrap(), default);
        data_root(&default);
        state.load_extracted(default.clone(), "2").unwrap();
        assert_eq!(state.backend().unwrap().data_dir, default);
        assert_eq!(DataConfig::load(&config).data_dir, Some(default.clone()));

        // TI_DATA_DIR is the target even when it fails to load; reloading keeps it unsaved.
        let state = AppState::load_in_background(with_default(Some(env.clone())));
        assert!(state.backend().is_err());
        assert_eq!(state.extract_target().unwrap(), env);
        data_root(&env);
        state.load_extracted(env.clone(), "2").unwrap();
        let b = state.backend().unwrap();
        assert_eq!((b.data_dir.as_path(), b.data_dir_source), (env.as_path(), DataDirSource::Env));
        assert_eq!(DataConfig::load(&config).data_dir, Some(default));
    }

    #[test]
    fn loads_the_extracted_build_even_if_it_is_not_the_latest() {
        let tmp = TempDir::new("extracted");
        let root = tmp.0.join("root");
        data_root(&root);
        let config = tmp.0.join("settings.json");
        let state = AppState::new(locator(None, config.clone()));

        // The game was rolled back: build 1 was extracted, the root's latest is still 2.
        state.load_extracted(root.clone(), "1").unwrap();
        let b = state.backend().unwrap();
        assert_eq!((b.ds.build_id(), b.pinned_build.as_deref()), ("1", Some("1")));
        let pinned = DataConfig {
            data_dir: Some(root.clone()),
            build: Some("1".into()),
            build_root: Some(root.clone()),
        };
        assert_eq!(DataConfig::load(&config), pinned);

        // Extracting the latest build keeps following the latest one.
        state.load_extracted(root.clone(), "2").unwrap();
        let b = state.backend().unwrap();
        assert_eq!((b.ds.build_id(), b.pinned_build.as_deref()), ("2", None));
        assert_eq!(
            DataConfig::load(&config),
            DataConfig {
                data_dir: Some(root.clone()),
                ..Default::default()
            }
        );

        // TI_DATA_DIR: only the pin is saved.
        let state = AppState::new(locator(Some(root.clone()), config.clone()));
        state.load_extracted(root.clone(), "1").unwrap();
        assert_eq!(state.backend().unwrap().data_dir_source, DataDirSource::Env);
        assert_eq!(DataConfig::load(&config), pinned);
    }

    #[test]
    fn only_other_builds_inside_the_root_can_be_deleted() {
        use crate::extraction::deletable_build_dir;
        let tmp = TempDir::new("delete");
        let root_dir = tmp.0.join("root");
        data_root(&root_dir);
        let root = DataRoot::open(&root_dir).unwrap();
        let err = |loaded: &str, dir: &str| deletable_build_dir(&root, loaded, dir).unwrap_err().to_string();

        assert_eq!(deletable_build_dir(&root, "2", "b1").unwrap(), root_dir.join("b1"));
        assert!(err("1", "b1").contains("loaded build"));
        assert!(err("2", "b9").contains("not a build"));
        assert!(err("2", "..").contains("not a build"));

        // Listed in index.json, but pointing outside the data root.
        let outside = tmp.0.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let entry = |dir: &str| json!({"dir": dir, "buildId": dir, "lastUpdated": 1, "formatVersion": ti_data::manifest::FORMAT_VERSION});
        write(
            &root_dir.join("index.json"),
            json!({"latest": "b2", "builds": [entry("b2"), entry("../outside"), entry(".hidden"), entry("link"), entry("gone")]}),
        );
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root_dir.join("link")).unwrap();
        let root = DataRoot::open(&root_dir).unwrap();
        for dir in ["../outside", ".hidden", "link", "gone"] {
            assert!(deletable_build_dir(&root, "b2", dir).is_err(), "{dir} must not be deletable");
        }
        assert!(outside.is_dir());
    }

    #[test]
    fn config_file_round_trip() {
        let tmp = TempDir::new("config");
        let path = tmp.0.join("nested/settings.json");
        assert_eq!(DataConfig::load(&path), DataConfig::default());
        let config = DataConfig {
            data_dir: Some("/data".into()),
            build: Some("2".into()),
            build_root: Some("/data".into()),
        };
        config.save(&path).unwrap();
        assert_eq!(DataConfig::load(&path), config);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"dataDir\"") && text.contains("\"build\""), "{text}");
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(DataConfig::load(&path), DataConfig::default());
    }

    #[test]
    fn startup_uses_settings_and_pinned_build() {
        let tmp = TempDir::new("startup");
        let root = tmp.0.join("root");
        data_root(&root);
        let config = tmp.0.join("settings.json");

        let pinned = |build: &str| DataConfig {
            data_dir: Some(root.clone()),
            build: Some(build.into()),
            build_root: Some(root.clone()),
        };
        pinned("1").save(&config).unwrap();
        let state = AppState::load_in_background(locator(None, config.clone()));
        let b = state.backend().unwrap();
        assert_eq!(
            (b.ds.build_id(), b.pinned_build.as_deref(), b.data_dir_source),
            ("1", Some("1"), DataDirSource::Settings)
        );
        assert_eq!(b.latest_build_id(), Some("2"));

        // A pinned build that disappeared falls back to the latest one.
        pinned("9").save(&config).unwrap();
        let state = AppState::load_in_background(locator(None, config.clone()));
        let b = state.backend().unwrap();
        assert_eq!((b.ds.build_id(), b.pinned_build.as_deref()), ("2", None));
        assert_eq!(
            DataConfig::load(&config),
            DataConfig {
                data_dir: Some(root.clone()),
                ..Default::default()
            }
        );

        // A pin made for one data root is not applied to another (here: the env folder).
        let other = tmp.0.join("other");
        data_root(&other);
        pinned("1").save(&config).unwrap();
        let state = AppState::load_in_background(locator(Some(other), config.clone()));
        assert_eq!(loaded_build(&state), "2");
        assert_eq!(DataConfig::load(&config), pinned("1"));

        let state = AppState::load_in_background(locator(None, tmp.0.join("missing.json")));
        assert!(state.backend().err().unwrap().to_string().contains("No extracted game data found"));
    }

    #[test]
    fn switching_builds_and_folders() {
        let tmp = TempDir::new("switch");
        let (first, second) = (tmp.0.join("first"), tmp.0.join("second"));
        data_root(&first);
        data_root(&second);
        let config = tmp.0.join("settings.json");
        let state = AppState::load_in_background(locator(Some(first.clone()), config.clone()));

        let before = state.backend().unwrap();
        assert_eq!((before.ds.build_id(), before.data_dir_source), ("2", DataDirSource::Env));
        state.set_build(Some("1".into())).unwrap();
        // A command that started before the switch keeps its backend.
        assert_eq!(before.ds.build_id(), "2");
        let after = state.backend().unwrap();
        assert!(!Arc::ptr_eq(&before, &after));
        assert_eq!(
            (after.ds.build_id(), after.data_dir.as_path(), after.data_dir_source),
            ("1", first.as_path(), DataDirSource::Env)
        );
        // The env folder is not written to the settings; the build is.
        assert_eq!(
            DataConfig::load(&config),
            DataConfig {
                data_dir: None,
                build: Some("1".into()),
                build_root: Some(first.clone())
            }
        );

        let err = state.set_build(Some("9".into())).unwrap_err().to_string();
        assert!(err.contains("\"9\""), "{err}");
        assert_eq!(loaded_build(&state), "1");

        state.set_build(None).unwrap();
        assert_eq!(loaded_build(&state), "2");
        assert_eq!(DataConfig::load(&config), DataConfig::default());

        let err = state.set_data_dir(tmp.0.join("empty")).unwrap_err().to_string();
        assert!(err.contains("has no index.json"), "{err}");
        assert_eq!(state.backend().unwrap().data_dir, first);

        state.set_build(Some("1".into())).unwrap();
        state.set_data_dir(second.clone()).unwrap();
        let b = state.backend().unwrap();
        assert_eq!(
            (b.ds.build_id(), b.data_dir.as_path(), b.data_dir_source, b.pinned_build.as_deref()),
            ("2", second.as_path(), DataDirSource::Settings, None)
        );
        assert_eq!(
            DataConfig::load(&config),
            DataConfig {
                data_dir: Some(second),
                ..Default::default()
            }
        );
    }

    #[test]
    fn choosing_a_folder_recovers_from_a_failed_start() {
        let tmp = TempDir::new("recover");
        let root = tmp.0.join("root");
        data_root(&root);
        let state = AppState::load_in_background(locator(None, tmp.0.join("settings.json")));
        assert!(state.backend().is_err());
        assert!(state.set_build(None).is_err());
        state.set_data_dir(root.clone()).unwrap();
        assert_eq!(state.backend().unwrap().data_dir, root);
    }

    #[test]
    fn a_reload_wins_over_a_slower_initial_load() {
        let tmp = TempDir::new("race");
        let root = tmp.0.join("root");
        data_root(&root);
        let state = AppState::new(locator(None, tmp.0.join("settings.json")));
        state.set_data_dir(root.clone()).unwrap();
        // The initial load finishes late, with an error or with other data: both are discarded.
        assert!(!state.publish_initial(Err("late failure".into())));
        assert!(
            !state.publish_initial(
                Backend::load(root.clone(), Some("1"), DataDirSource::Found)
                    .map(Arc::new)
                    .map_err(|e| e.to_string())
            )
        );
        assert_eq!(loaded_build(&state), "2");
        assert!(AppState::new(locator(None, tmp.0.join("settings.json"))).publish_initial(Err("failure".into())));
    }

    #[test]
    fn relative_data_dirs_are_saved_absolute() {
        let tmp = TempDir::new("relative");
        let root = tmp.0.join("root");
        data_root(&root);
        let config = tmp.0.join("settings.json");
        let state = AppState::new(locator(None, config.clone()));
        state.set_data_dir(root.join("b1").join("..")).unwrap();
        assert_eq!(DataConfig::load(&config).data_dir, Some(root.clone()));
        assert_eq!(state.backend().unwrap().data_dir, root);
    }

    #[test]
    fn recent_cache_evicts_least_recently_used() {
        let mut cache = RecentCache::new(2);
        let builds = std::cell::Cell::new(0);
        let get = |cache: &mut RecentCache<u32, u32>, k: u32| {
            cache
                .get_or_try_insert(k, || {
                    builds.set(builds.get() + 1);
                    Ok::<_, ()>(k * 10)
                })
                .unwrap()
        };
        assert_eq!(get(&mut cache, 1), 10);
        get(&mut cache, 2);
        get(&mut cache, 1);
        get(&mut cache, 3); // evicts 2
        get(&mut cache, 1);
        assert_eq!(builds.get(), 3);
        get(&mut cache, 2);
        assert_eq!(builds.get(), 4);
        assert!(cache.get_or_try_insert(5, || Err(())).is_err());
    }
}
