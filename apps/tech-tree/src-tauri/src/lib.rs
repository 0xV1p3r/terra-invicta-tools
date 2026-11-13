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
//! * `fixtures` – (test only) writes mock JSON for browser-based UI work

mod assets;
mod commands;
mod dto;
mod effects;
mod error;
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
use state::{AppState, Backend, Locator};
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
#[tauri::command]
async fn set_build(state: State<'_, AppState>, build_id: Option<String>) -> CmdResult<()> {
    let state = AppState::clone(&state);
    spawn(move || state.set_build(build_id)).await
}

/// Load the latest build of another data folder (the one containing `index.json`) and remember it.
#[tauri::command]
async fn set_data_dir(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    let state = AppState::clone(&state);
    spawn(move || state.set_data_dir(path.into())).await
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
            app.manage(AppState::load_in_background(Locator::from_env(config_path)));
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
