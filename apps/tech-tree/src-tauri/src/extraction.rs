//! In-app extraction of the game data with `ti-extract`: finding the install, running one
//! extraction at a time, and deleting old builds of the data root.
//!
//! `ti-extract` calls are wrapped in `catch_panic`, so a bug there fails the command instead of
//! taking the app down (game detection also runs at startup, for the update check).

use std::panic::AssertUnwindSafe;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use ti_extract::{ExtractOptions, GameInstall, Images, Progress, Stage};

use crate::dto::{BuildSize, ExtractArgs, ExtractEvent, ExtractResult, ExtractStage, ExtractStatus, GameInfo, ImageFormat};
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

/// Managed Tauri state: the running extraction, and the guard of every operation that changes
/// the data folder. Cheap to clone.
///
/// Switching the data folder or build and deleting a build hold the guard (`exclusive`) and are
/// refused while an extraction runs. An extraction is registered first (`start`, before any
/// work, so an immediate cancel is not lost); it then takes the guard to pick its target and
/// again for its final reload, so nothing can change the data folder in between.
#[derive(Clone, Default)]
pub struct Extractor(Arc<Inner>);

#[derive(Default)]
struct Inner {
    ops: Mutex<()>,
    running: Mutex<Option<Arc<Run>>>,
}

#[derive(Default)]
struct Run {
    cancel: AtomicBool,
    status: Mutex<ExtractStatus>,
}

/// A registered extraction; unregisters when dropped.
pub struct Running {
    extractor: Extractor,
    run: Arc<Run>,
}

impl Drop for Running {
    fn drop(&mut self) {
        *self.extractor.running() = None;
    }
}

impl Extractor {
    fn running(&self) -> MutexGuard<'_, Option<Arc<Run>>> {
        self.0.running.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn lock_ops(&self) -> MutexGuard<'_, ()> {
        self.0.ops.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Exclusive use of the data folder for `action` (e.g. "switching builds") while the guard
    /// is held; refused while an extraction runs.
    pub fn exclusive(&self, action: &str) -> CmdResult<MutexGuard<'_, ()>> {
        let ops = self.lock_ops();
        if self.is_running() {
            return Err(CmdError::msg(format!("Wait for the extraction to finish before {action}.")));
        }
        Ok(ops)
    }

    /// Register an extraction (one at a time); `Running::extract` then does the work.
    pub fn start(&self) -> CmdResult<Running> {
        let mut running = self.running();
        if running.is_some() {
            return Err(CmdError::msg("An extraction is already running."));
        }
        let run = Arc::new(Run::default());
        *running = Some(run.clone());
        Ok(Running { extractor: self.clone(), run })
    }

    pub fn is_running(&self) -> bool {
        self.running().is_some()
    }

    /// Progress of the running extraction, if any.
    pub fn status(&self) -> Option<ExtractStatus> {
        let run = self.running().clone()?;
        let status = run.status.lock().unwrap_or_else(|e| e.into_inner()).clone();
        Some(status)
    }

    /// Ask the running extraction to stop; it then fails with `CANCELLED`.
    pub fn cancel(&self) {
        if let Some(run) = &*self.running() {
            run.cancel.store(true, Ordering::Relaxed);
        }
    }
}

impl Running {
    fn cancelled(&self) -> bool {
        self.run.cancel.load(Ordering::Relaxed)
    }

    fn record(&self, event: &ExtractEvent) {
        let mut status = self.run.status.lock().unwrap_or_else(|e| e.into_inner());
        match event {
            ExtractEvent::Stage { stage } => {
                *status = ExtractStatus {
                    stage: Some(*stage),
                    ..Default::default()
                }
            }
            ExtractEvent::Step { done, total, label } => {
                (status.done, status.total) = (*done, *total);
                if label.is_some() {
                    status.label = label.clone();
                }
            }
            ExtractEvent::Log { message } => status.label = Some(message.clone()),
        }
    }

    /// Extract the install at `args.game_dir` into the extraction target of `state`, then load
    /// the new build and remember its data root.
    pub fn extract(self, state: &AppState, args: &ExtractArgs, progress: &mut dyn FnMut(ExtractEvent)) -> CmdResult<ExtractResult> {
        if self.cancelled() {
            return Err(CmdError::msg(CANCELLED));
        }
        // Waits for a switch or deletion that started before this run was registered.
        let data_dir = {
            let _ops = self.extractor.lock_ops();
            state.extract_target()?
        };
        let install = discover(Path::new(&args.game_dir))?;
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
        let mut forward = |p: Progress| {
            let event = event(p);
            self.record(&event);
            progress(event);
        };
        let report = catch_panic("Extraction", || ti_extract::extract(&install, &opts, &mut forward, &self.run.cancel))?.map_err(|e| match e {
            ti_extract::Error::Cancelled => CmdError::msg(CANCELLED),
            e => CmdError::msg(e.to_string()),
        })?;
        self.finish(state, data_dir, &report.build_id)?;
        Ok(ExtractResult {
            build_id: report.build_id,
            extracted: report.extracted,
            issues: report.issues,
            parse_errors: report.parse_errors,
            seconds: report.seconds,
        })
    }

    /// Load the extracted build, then unregister, both under the guard.
    fn finish(self, state: &AppState, data_dir: PathBuf, build_id: &str) -> CmdResult<()> {
        let extractor = self.extractor.clone();
        let ops = extractor.lock_ops();
        let loaded = state
            .load_extracted(data_dir, build_id)
            .map_err(|e| CmdError::msg(format!("Extracted build {build_id}, but could not load it: {e}")));
        drop(self);
        drop(ops);
        loaded
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
    let _ops = extractor.exclusive("deleting builds")?;
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
        // Hidden folders (e.g. a replaced build set aside by `ti-extract`) are never builds.
        .filter(|b| !b.dir.starts_with('.'))
        .map(|b| BuildSize {
            dir: b.dir.clone(),
            bytes: dir_size(&root.path.join(&b.dir)),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tests::{TempDir, data_root, loaded_build, locator};

    #[test]
    fn panics_become_errors() {
        let err = catch_panic("Detection", || -> u32 { panic!("not yet") }).unwrap_err().to_string();
        assert_eq!(err, "Detection failed unexpectedly: not yet");
        assert_eq!(catch_panic("x", || 3).unwrap(), 3);
    }

    #[test]
    fn one_extraction_at_a_time() {
        let extractor = Extractor::default();
        let running = extractor.start().unwrap();
        assert!(extractor.is_running());
        assert!(extractor.start().is_err());
        extractor.cancel();
        assert!(running.cancelled());
        drop(running);
        assert!(!extractor.is_running());
        assert!(!extractor.start().unwrap().cancelled());
    }

    /// A loaded state with the test data root (builds `1` and the latest `2`, dirs `b1`, `b2`).
    fn loaded_state(tmp: &TempDir) -> (AppState, PathBuf) {
        let root = tmp.0.join("root");
        data_root(&root);
        let state = AppState::load_in_background(locator(Some(root.clone()), tmp.0.join("settings.json")));
        assert_eq!(loaded_build(&state), "2");
        (state, root)
    }

    fn args(game_dir: &Path) -> ExtractArgs {
        ExtractArgs {
            game_dir: game_dir.display().to_string(),
            large_art: false,
            image_format: ImageFormat::Webp,
            force: false,
        }
    }

    #[test]
    fn data_operations_are_refused_while_an_extraction_runs() {
        let tmp = TempDir::new("ops-refused");
        let (state, _root) = loaded_state(&tmp);
        let extractor = Extractor::default();

        let running = extractor.start().unwrap();
        let err = extractor.exclusive("switching builds").unwrap_err().to_string();
        assert_eq!(err, "Wait for the extraction to finish before switching builds.");
        let err = delete_build(&state, &extractor, "b1").unwrap_err().to_string();
        assert!(err.contains("before deleting builds"), "{err}");
        drop(running);

        assert!(extractor.exclusive("switching builds").is_ok());
        // Deleting the loaded build is refused; another one is deleted and leaves the index.
        let err = delete_build(&state, &extractor, "b2").unwrap_err().to_string();
        assert!(err.contains("loaded build"), "{err}");
        assert!(tmp.0.join("root/b2").is_dir());
        delete_build(&state, &extractor, "b1").unwrap();
        assert!(!tmp.0.join("root/b1").exists());
        let root = ti_data::DataRoot::open(tmp.0.join("root")).unwrap();
        assert_eq!(root.index.builds.iter().map(|b| b.dir.as_str()).collect::<Vec<_>>(), ["b2"]);
    }

    #[test]
    fn cancel_before_the_work_starts_is_not_lost() {
        let tmp = TempDir::new("cancel-early");
        let (state, _root) = loaded_state(&tmp);
        let extractor = Extractor::default();
        let running = extractor.start().unwrap();
        extractor.cancel();
        // The game folder does not exist: only the cancel check can produce this error.
        let err = running.extract(&state, &args(&tmp.0.join("no-game")), &mut |_| {}).unwrap_err().to_string();
        assert_eq!(err, CANCELLED);
        assert!(!extractor.is_running());
        assert_eq!(loaded_build(&state), "2");
    }

    #[test]
    fn the_final_reload_waits_for_a_switch_in_progress() {
        let tmp = TempDir::new("ops-order");
        let (state, root) = loaded_state(&tmp);
        let extractor = Extractor::default();

        // A switch holds the guard; an extraction registers meanwhile and reaches its reload.
        let switch = extractor.exclusive("switching builds").unwrap();
        let running = extractor.start().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let finisher = {
            let (state, root) = (state.clone(), root.clone());
            std::thread::spawn(move || {
                tx.send(()).unwrap();
                running.finish(&state, root, "1")
            })
        };
        rx.recv().unwrap();
        state.set_build(Some("2".into())).unwrap();
        // The extraction cannot unregister (or reload) before the switch releases the guard.
        assert!(extractor.is_running());
        assert_eq!(loaded_build(&state), "2");
        drop(switch);

        finisher.join().unwrap().unwrap();
        assert!(!extractor.is_running());
        let b = state.backend().unwrap();
        assert_eq!((b.ds.build_id(), b.pinned_build.as_deref()), ("1", Some("1")));
        assert!(extractor.exclusive("switching builds").is_ok());
    }

    #[test]
    fn status_follows_the_progress() {
        let extractor = Extractor::default();
        assert_eq!(extractor.status(), None);
        let running = extractor.start().unwrap();
        assert_eq!(extractor.status(), Some(ExtractStatus::default()));
        running.record(&ExtractEvent::Stage { stage: ExtractStage::Images });
        running.record(&ExtractEvent::Step {
            done: 3,
            total: 9,
            label: Some("icons".into()),
        });
        running.record(&ExtractEvent::Step {
            done: 4,
            total: 9,
            label: None,
        });
        let status = ExtractStatus {
            stage: Some(ExtractStage::Images),
            done: 4,
            total: 9,
            label: Some("icons".into()),
        };
        assert_eq!(extractor.status(), Some(status));
        running.record(&ExtractEvent::Stage { stage: ExtractStage::Finish });
        assert_eq!(extractor.status().unwrap().label, None);
        drop(running);
        assert_eq!(extractor.status(), None);
    }

    #[test]
    fn build_sizes_skip_hidden_folders() {
        let tmp = TempDir::new("sizes");
        let (state, root) = loaded_state(&tmp);
        let index = std::fs::read_to_string(root.join("index.json")).unwrap();
        let mut index: serde_json::Value = serde_json::from_str(&index).unwrap();
        let mut hidden = index["builds"][0].clone();
        hidden["dir"] = ".b1.old".into();
        index["builds"].as_array_mut().unwrap().push(hidden);
        std::fs::write(root.join("index.json"), index.to_string()).unwrap();
        let dirs: Vec<_> = build_sizes(&state).unwrap().into_iter().map(|b| b.dir).collect();
        assert_eq!(dirs, ["b1", "b2"]);
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
