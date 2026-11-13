//! In-app extraction of the game data with `ti-extract`: finding the install, running one
//! extraction at a time, and deleting old builds of the data root.
//!
//! `ti-extract` calls are wrapped in `catch_panic`, so a bug there fails the command instead of
//! taking the app down (game detection also runs at startup, for the update check).

use std::panic::AssertUnwindSafe;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use ti_extract::{ExtractOptions, GameInstall, Images, Progress, Stage};

use crate::dto::{BuildSize, ExtractArgs, ExtractEvent, ExtractResult, ExtractStage, GameInfo, ImageFormat};
use crate::error::{CmdError, CmdResult};
use crate::state::AppState;

/// Error message of a cancelled extraction.
pub const CANCELLED: &str = "Extraction cancelled.";

/// Run `f`, turning a panic into an error ("<what> failed unexpectedly: ...").
fn catch_panic<T>(what: &str, f: impl FnOnce() -> T) -> CmdResult<T> {
    std::panic::catch_unwind(AssertUnwindSafe(f)).map_err(|panic| {
        let detail = panic
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("unknown error");
        CmdError::msg(format!("{what} failed unexpectedly: {detail}"))
    })
}

fn game_info(install: &GameInstall) -> GameInfo {
    GameInfo {
        root: install.root.display().to_string(),
        version: install.version.clone(),
        build_id: install.build_id.clone(),
    }
}

/// Read the install at `dir`; also accepts its `TerraInvicta_Data` folder.
fn discover(dir: &Path) -> CmdResult<GameInstall> {
    let not_an_install = || {
        CmdError::msg(format!(
            "{} is not a Terra Invicta install. Choose the game folder, the one that contains TerraInvicta_Data.",
            dir.display()
        ))
    };
    let read = |dir: &Path| catch_panic("Reading the game install", || ti_extract::discover(dir));
    match read(dir)? {
        Ok(install) => Ok(install),
        Err(ti_extract::Error::NotAnInstall(_) | ti_extract::Error::GameNotFound) => {
            let parent = dir.parent().filter(|_| dir.file_name().is_some_and(|n| n == "TerraInvicta_Data"));
            match parent.map(read).transpose()? {
                Some(Ok(install)) => Ok(install),
                _ => Err(not_an_install()),
            }
        }
        Err(e) => Err(CmdError::msg(format!("Could not read the game install at {}: {e}", dir.display()))),
    }
}

/// The install at `saved` (a folder chosen earlier), else `TI_GAME_DIR` or Steam's. `None` when
/// there is none or detection fails: callers treat both as "not found".
pub fn detect_game(saved: Option<&Path>) -> Option<GameInfo> {
    if let Some(install) = saved.and_then(|dir| discover(dir).ok()) {
        return Some(game_info(&install));
    }
    let found = catch_panic("Game detection", || {
        ti_extract::resolve_game_dir(None).and_then(|dir| ti_extract::discover(&dir))
    });
    match found {
        Ok(Ok(install)) => Some(game_info(&install)),
        Ok(Err(ti_extract::Error::GameNotFound)) => None,
        Ok(Err(e)) => {
            eprintln!("ti-tech-tree: game detection: {e}");
            None
        }
        Err(e) => {
            eprintln!("ti-tech-tree: {e}");
            None
        }
    }
}

/// The install at `dir`, or why it is not one.
pub fn inspect_game(dir: &Path) -> CmdResult<GameInfo> {
    discover(dir).map(|install| game_info(&install))
}

/// Managed Tauri state: the cancel flag of the running extraction, if any.
#[derive(Default)]
pub struct Extractor {
    running: Mutex<Option<Arc<AtomicBool>>>,
}

/// Marks an extraction as running until dropped.
struct Running<'a> {
    extractor: &'a Extractor,
    cancel: Arc<AtomicBool>,
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        *self.extractor.running.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

impl Extractor {
    fn start(&self) -> CmdResult<Running<'_>> {
        let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        if running.is_some() {
            return Err(CmdError::msg("An extraction is already running."));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *running = Some(cancel.clone());
        Ok(Running { extractor: self, cancel })
    }

    pub fn is_running(&self) -> bool {
        self.running.lock().unwrap_or_else(|e| e.into_inner()).is_some()
    }

    /// Ask the running extraction to stop; it then fails with `CANCELLED`.
    pub fn cancel(&self) {
        if let Some(cancel) = &*self.running.lock().unwrap_or_else(|e| e.into_inner()) {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Extract the install at `args.game_dir` into the extraction target of `state`, then load
    /// the new build and remember its data root. One extraction at a time.
    pub fn run(&self, state: &AppState, args: &ExtractArgs, progress: &mut dyn FnMut(ExtractEvent)) -> CmdResult<ExtractResult> {
        let running = self.start()?;
        let install = discover(Path::new(&args.game_dir))?;
        let data_dir = state.extract_target()?;
        let opts = ExtractOptions {
            force: args.force,
            images: if args.large_art {
                Images::All
            } else {
                Images::Skip(ti_extract::LARGE_ART_BUNDLES.iter().map(|b| b.to_string()).collect())
            },
            image_format: match args.image_format {
                ImageFormat::Png => ti_extract::ImageFormat::Png,
                ImageFormat::Webp => ti_extract::ImageFormat::Webp,
            },
            ..ExtractOptions::new(&data_dir)
        };
        let mut forward = |p: Progress| progress(event(p));
        let report = catch_panic("Extraction", || ti_extract::extract(&install, &opts, &mut forward, &running.cancel))?.map_err(|e| match e {
            ti_extract::Error::Cancelled => CmdError::msg(CANCELLED),
            e => CmdError::msg(e.to_string()),
        })?;
        drop(running);
        state
            .load_extracted(data_dir.clone())
            .map_err(|e| CmdError::msg(format!("Extracted build {}, but could not load it: {e}", report.build_id)))?;
        Ok(ExtractResult {
            build_id: report.build_id,
            extracted: report.extracted,
            issues: report.issues,
            parse_errors: report.parse_errors,
            seconds: report.seconds,
        })
    }
}

fn event(progress: Progress) -> ExtractEvent {
    match progress {
        Progress::Stage { stage } => ExtractEvent::Stage {
            stage: match stage {
                Stage::Templates => ExtractStage::Templates,
                Stage::Locale => ExtractStage::Locale,
                Stage::Schema => ExtractStage::Schema,
                Stage::Images => ExtractStage::Images,
                Stage::Finish => ExtractStage::Finish,
            },
        },
        Progress::Step { done, total, label } => ExtractEvent::Step { done, total, label },
        Progress::Log { message } => ExtractEvent::Log { message },
    }
}

/// The directory of build `dir` of the data root at `root`, if it may be deleted: listed in
/// `index.json`, not the loaded build, and a real directory directly inside the data root.
pub fn deletable_build_dir(root: &ti_data::DataRoot, loaded_build: &str, dir: &str) -> CmdResult<PathBuf> {
    let entry = root
        .index
        .builds
        .iter()
        .find(|b| b.dir == dir)
        .ok_or_else(|| CmdError::msg(format!("{dir} is not a build of this data folder.")))?;
    if entry.build_id == loaded_build {
        return Err(CmdError::msg("The loaded build cannot be deleted. Switch to another build first."));
    }
    let mut parts = Path::new(dir).components();
    if !matches!((parts.next(), parts.next()), (Some(Component::Normal(_)), None)) || dir.starts_with('.') {
        return Err(CmdError::msg(format!("Refusing to delete {dir}: not a build folder name.")));
    }
    let outside = || CmdError::msg(format!("Refusing to delete {dir}: it is not a folder inside the data folder."));
    let root_dir = root.path.canonicalize().map_err(|_| outside())?;
    let path = root_dir.join(dir);
    // `symlink_metadata`: a symlinked build is never followed out of the data root.
    let is_dir = path.symlink_metadata().is_ok_and(|m| m.is_dir());
    if !is_dir || path.canonicalize().ok().as_deref().and_then(Path::parent) != Some(root_dir.as_path()) {
        return Err(outside());
    }
    Ok(path)
}

/// Delete build `dir` of the loaded data root and rewrite its `index.json`.
pub fn delete_build(state: &AppState, extractor: &Extractor, dir: &str) -> CmdResult<()> {
    if extractor.is_running() {
        return Err(CmdError::msg("Wait for the extraction to finish before deleting builds."));
    }
    let backend = state.backend()?;
    let root = ti_data::DataRoot::open(&backend.data_dir)?;
    let path = deletable_build_dir(&root, backend.ds.build_id(), dir)?;
    std::fs::remove_dir_all(&path).map_err(|e| CmdError::msg(format!("Could not delete {}: {e}", path.display())))?;
    eprintln!("ti-tech-tree: deleted build {}", path.display());
    catch_panic("Updating index.json", || ti_extract::write_index(&backend.data_dir))?
        .map_err(|e| CmdError::msg(format!("Deleted {dir}, but could not update index.json: {e}")))
}

/// Bytes of the regular files below `path` (symlinks are not followed).
fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else { return 0 };
    entries
        .flatten()
        .map(|entry| match entry.metadata() {
            Ok(m) if m.is_dir() => dir_size(&entry.path()),
            Ok(m) if m.is_file() => m.len(),
            _ => 0,
        })
        .sum()
}

/// Disk usage of every build of the loaded data root.
pub fn build_sizes(state: &AppState) -> CmdResult<Vec<BuildSize>> {
    let backend = state.backend()?;
    let root = ti_data::DataRoot::open(&backend.data_dir).unwrap_or_else(|_| backend.root.clone());
    Ok(root
        .index
        .builds
        .iter()
        .map(|b| BuildSize {
            dir: b.dir.clone(),
            bytes: dir_size(&root.path.join(&b.dir)),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panics_become_errors() {
        let err = catch_panic("Detection", || -> u32 { panic!("not yet") }).unwrap_err().to_string();
        assert_eq!(err, "Detection failed unexpectedly: not yet");
        assert_eq!(catch_panic("x", || 3).unwrap(), 3);
    }

    /// Detection runs at startup (update check): whatever `ti-extract` does, it must not panic.
    #[test]
    fn detection_does_not_panic() {
        let missing = std::env::temp_dir().join("ti-tech-tree-no-such-install");
        let _ = detect_game(Some(&missing));
        assert!(inspect_game(&missing).is_err());
    }

    #[test]
    fn one_extraction_at_a_time() {
        let extractor = Extractor::default();
        let running = extractor.start().unwrap();
        assert!(extractor.is_running());
        assert!(extractor.start().is_err());
        extractor.cancel();
        assert!(running.cancel.load(Ordering::Relaxed));
        drop(running);
        assert!(!extractor.is_running());
        assert!(!extractor.start().unwrap().cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn dir_size_sums_nested_files() {
        let dir = std::env::temp_dir().join(format!("ti-tech-tree-{}-size", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("a/b")).unwrap();
        std::fs::write(dir.join("x"), [0; 10]).unwrap();
        std::fs::write(dir.join("a/b/y"), [0; 5]).unwrap();
        assert_eq!(dir_size(&dir), 15);
        assert_eq!(dir_size(&dir.join("missing")), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
