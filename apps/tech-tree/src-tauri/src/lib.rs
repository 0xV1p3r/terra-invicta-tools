//! Tauri backend: exposes `ti-data` to the Svelte frontend. Command names and
//! payloads are the contract with `src/lib/api.ts` (see `dto.rs`).
//!
//! * `state`    – data loading and switching, settings file, per-language text/search caches
//! * `scope`    – `ScopeArgs` → `ti_data::Scope`
//! * `commands` – command logic (no Tauri runtime needed; tested on real data)
//! * `layout`   – layered graph layout
//! * `wiki`     – wiki section labels and group order
//! * `refs`     – entity keys and `EntityRef`s
//! * `effects`  – effect descriptions
//! * `properties` – filterable tech/project properties (effects, unlocks)
//! * `assets`   – `tiasset://` protocol for images
//! * `extraction` – in-app extraction with `ti-extract`, game detection, deleting builds
//! * `fixtures` – (test only) writes mock JSON for browser-based UI work

mod assets;
mod commands;
mod dto;
mod effects;
mod error;
mod extraction;
#[cfg(test)]
mod fixtures;
mod layout;
mod properties;
mod refs;
mod scope;
mod spoilers;
mod state;
mod stats;
mod wiki;

use dto::*;
use error::{CmdError, CmdResult};
use extraction::Extractor;
use state::{AppState, Backend, Locator};
use tauri::ipc::Channel;
use tauri::{Manager, State};

/// Run `f` on the blocking pool so the UI thread stays responsive.
async fn spawn<T: Send + 'static>(f: impl FnOnce() -> CmdResult<T> + Send + 'static) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| CmdError::msg(format!("command failed: {e}")))?
}

/// Run command logic on the current backend (kept for the whole command, even if a reload swaps it).
async fn run_blocking<T: Send + 'static>(state: State<'_, AppState>, f: impl FnOnce(&Backend) -> CmdResult<T> + Send + 'static) -> CmdResult<T> {
    let state = AppState::clone(&state);
    spawn(move || f(&*state.backend()?)).await
}

/// Like `run_blocking`, and starts building the search index of the command's language in the background.
async fn run_in_language<T: Send + 'static>(
    state: State<'_, AppState>,
    lang: String,
    f: impl FnOnce(&Backend) -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    let state = AppState::clone(&state);
    spawn(move || {
        let backend = state.backend()?;
        backend.warm_search(&lang);
        f(&backend)
    })
    .await
}

#[tauri::command]
async fn get_meta(state: State<'_, AppState>, lang: Option<String>) -> CmdResult<Meta> {
    let lang = lang.unwrap_or_else(|| "en".into());
    run_in_language(state, lang.clone(), move |b| commands::get_meta(b, &lang)).await
}

#[tauri::command]
async fn list_builds(state: State<'_, AppState>) -> CmdResult<Vec<BuildInfo>> {
    run_blocking(state, commands::list_builds).await
}

/// Load another build of the data folder (`None` = always the latest) and remember it.
/// Refused while an extraction runs.
#[tauri::command]
async fn set_build(state: State<'_, AppState>, extractor: State<'_, Extractor>, build_id: Option<String>) -> CmdResult<()> {
    let (state, extractor) = (AppState::clone(&state), Extractor::clone(&extractor));
    spawn(move || {
        let _ops = extractor.exclusive("switching builds")?;
        state.set_build(build_id)
    })
    .await
}

/// Load the latest build of another data folder (the one containing `index.json`) and remember it.
/// Refused while an extraction runs.
#[tauri::command]
async fn set_data_dir(state: State<'_, AppState>, extractor: State<'_, Extractor>, path: String) -> CmdResult<()> {
    let (state, extractor) = (AppState::clone(&state), Extractor::clone(&extractor));
    spawn(move || {
        let _ops = extractor.exclusive("choosing another data folder")?;
        state.set_data_dir(path.into())
    })
    .await
}

/// Disk usage of the data root's builds (walks the build folders; call when needed).
#[tauri::command]
async fn get_build_sizes(state: State<'_, AppState>) -> CmdResult<Vec<BuildSize>> {
    let state = AppState::clone(&state);
    spawn(move || extraction::build_sizes(&state)).await
}

/// Delete a build (by directory) of the loaded data root; not the loaded one.
#[tauri::command]
async fn delete_build(app: tauri::AppHandle, dir: String) -> CmdResult<()> {
    spawn(move || extraction::delete_build(&app.state::<AppState>(), &app.state::<Extractor>(), &dir)).await
}

/// The game install at `saved` (chosen earlier), else the detected one; `None` if there is none.
#[tauri::command]
async fn detect_game(saved: Option<String>) -> CmdResult<Option<GameInfo>> {
    spawn(move || Ok(extraction::detect_game(saved.as_deref().map(std::path::Path::new)))).await
}

/// The game install at `path`, or an error saying why it is not one.
#[tauri::command]
async fn inspect_game(path: String) -> CmdResult<GameInfo> {
    spawn(move || extraction::inspect_game(path.as_ref())).await
}

/// Extract the game data, streaming progress to `on_progress`, then load the new build.
#[tauri::command]
async fn extract_game_data(
    state: State<'_, AppState>,
    extractor: State<'_, Extractor>,
    args: ExtractArgs,
    on_progress: Channel<ExtractEvent>,
) -> CmdResult<ExtractResult> {
    // Registered before the blocking work starts, so a cancel right after cannot be lost.
    let running = extractor.start()?;
    let state = AppState::clone(&state);
    spawn(move || {
        let mut send = |event: ExtractEvent| {
            // The window may be gone (reloaded); the extraction finishes anyway.
            let _ = on_progress.send(event);
        };
        running.extract(&state, &args, &mut send)
    })
    .await
}

/// Stop the running extraction (it then fails with "Extraction cancelled.").
#[tauri::command]
fn cancel_extraction(extractor: State<'_, Extractor>) {
    extractor.cancel();
}

/// Progress of the running extraction; `None` when none runs (e.g. to recover after a reload).
#[tauri::command]
fn extraction_status(extractor: State<'_, Extractor>) -> Option<ExtractStatus> {
    extractor.status()
}

#[tauri::command]
async fn get_wiki_index(state: State<'_, AppState>, scope: ScopeArgs, spoilers: Option<bool>) -> CmdResult<Vec<WikiSection>> {
    run_in_language(state, scope.lang.clone(), move |b| commands::get_wiki_index(b, &scope, spoilers)).await
}

#[tauri::command]
async fn get_wiki_entries(
    state: State<'_, AppState>,
    scope: ScopeArgs,
    template: String,
    spoiler: Option<bool>,
    filter: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
) -> CmdResult<WikiPage> {
    run_in_language(state, scope.lang.clone(), move |b| {
        commands::get_wiki_entries(b, &scope, &template, spoiler, filter.as_deref().unwrap_or_default(), offset.unwrap_or(0), limit)
    })
    .await
}

#[tauri::command]
async fn get_tree(
    state: State<'_, AppState>,
    scope: ScopeArgs,
    filter: TreeFilter,
    layout: Option<FullLayout>,
    merge_edges: Option<bool>,
) -> CmdResult<TreeGraph> {
    run_in_language(state, scope.lang.clone(), move |b| {
        commands::get_tree(b, &scope, &filter, layout.unwrap_or_default(), merge_edges.unwrap_or(true))
    })
    .await
}

#[tauri::command]
async fn get_subtree(state: State<'_, AppState>, scope: ScopeArgs, args: SubtreeArgs, merge_edges: Option<bool>) -> CmdResult<TreeGraph> {
    run_in_language(state, scope.lang.clone(), move |b| {
        commands::get_subtree(b, &scope, &args, merge_edges.unwrap_or(true))
    })
    .await
}

#[tauri::command]
async fn get_focus(state: State<'_, AppState>, scope: ScopeArgs, root: String) -> CmdResult<TreeGraph> {
    run_in_language(state, scope.lang.clone(), move |b| commands::get_focus(b, &scope, &root)).await
}

#[tauri::command]
async fn get_properties(state: State<'_, AppState>, scope: ScopeArgs, filter: TreeFilter) -> CmdResult<Vec<PropertyInfo>> {
    run_in_language(state, scope.lang.clone(), move |b| commands::get_properties(b, &scope, &filter)).await
}

#[tauri::command]
async fn get_entity(state: State<'_, AppState>, scope: ScopeArgs, key: String) -> CmdResult<EntityDetail> {
    run_in_language(state, scope.lang.clone(), move |b| commands::get_entity(b, &scope, &key)).await
}

#[tauri::command]
async fn search(
    state: State<'_, AppState>,
    scope: ScopeArgs,
    query: String,
    limit: usize,
    all: Option<bool>,
    kinds: Option<Vec<String>>,
) -> CmdResult<Vec<SearchHit>> {
    run_in_language(state, scope.lang.clone(), move |b| {
        commands::search(b, &scope, &query, limit, all.unwrap_or(false), &kinds.unwrap_or_default())
    })
    .await
}

#[tauri::command]
async fn get_solar_system(state: State<'_, AppState>, scope: ScopeArgs) -> CmdResult<SolarSystem> {
    run_in_language(state, scope.lang.clone(), move |b| commands::get_solar_system(b, &scope)).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_path = app.path().app_config_dir().ok().map(|dir| dir.join(state::CONFIG_FILE));
            let default_dir = app.path().app_data_dir().ok().map(|dir| dir.join(state::DATA_SUBDIR));
            app.manage(AppState::load_in_background(Locator::from_env(config_path, default_dir)));
            app.manage(Extractor::default());
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol(assets::SCHEME, |ctx, request, responder| {
            let state = ctx.app_handle().try_state::<AppState>().map(|s| AppState::clone(&s));
            tauri::async_runtime::spawn_blocking(move || {
                let backend = state.and_then(|s| s.backend().ok());
                let assets_dir = backend.as_ref().and_then(|b| b.assets_dir.as_deref());
                responder.respond(assets::respond(assets_dir, &request));
            });
        })
        .invoke_handler(tauri::generate_handler![
            get_wiki_index,
            get_wiki_entries,
            get_meta,
            list_builds,
            set_build,
            set_data_dir,
            get_build_sizes,
            delete_build,
            detect_game,
            inspect_game,
            extract_game_data,
            cancel_extraction,
            extraction_status,
            get_tree,
            get_subtree,
            get_focus,
            get_properties,
            get_entity,
            search,
            get_solar_system
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
