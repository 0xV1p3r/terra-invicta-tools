//! Tauri backend: exposes `ti-data` to the Svelte frontend. Command names and
//! payloads are the contract with `src/lib/api.ts` (see `dto.rs`).
//!
//! * `state`    – data loading, per-language text/search caches
//! * `scope`    – `ScopeArgs` → `ti_data::Scope`
//! * `commands` – command logic (no Tauri runtime needed; tested on real data)
//! * `layout`   – layered graph layout
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
mod state;
mod stats;

use dto::*;
use error::{CmdError, CmdResult};
use state::{AppState, Backend};
use tauri::{Manager, State};

/// Run command logic on the blocking pool so the UI thread stays responsive.
async fn run_blocking<T: Send + 'static>(
    state: State<'_, AppState>,
    f: impl FnOnce(&Backend) -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    let state = AppState::clone(&state);
    tauri::async_runtime::spawn_blocking(move || f(&*state.backend()?))
        .await
        .map_err(|e| CmdError::msg(format!("command failed: {e}")))?
}

#[tauri::command]
async fn get_meta(state: State<'_, AppState>) -> CmdResult<Meta> {
    run_blocking(state, commands::get_meta).await
}

#[tauri::command]
async fn get_tree(
    state: State<'_, AppState>,
    scope: ScopeArgs,
    filter: TreeFilter,
    layout: Option<FullLayout>,
) -> CmdResult<TreeGraph> {
    run_blocking(state, move |b| commands::get_tree(b, &scope, &filter, layout.unwrap_or_default())).await
}

#[tauri::command]
async fn get_subtree(state: State<'_, AppState>, scope: ScopeArgs, args: SubtreeArgs) -> CmdResult<TreeGraph> {
    run_blocking(state, move |b| commands::get_subtree(b, &scope, &args)).await
}

#[tauri::command]
async fn get_focus(state: State<'_, AppState>, scope: ScopeArgs, root: String) -> CmdResult<TreeGraph> {
    run_blocking(state, move |b| commands::get_focus(b, &scope, &root)).await
}

#[tauri::command]
async fn get_properties(state: State<'_, AppState>, scope: ScopeArgs, filter: TreeFilter) -> CmdResult<Vec<PropertyInfo>> {
    run_blocking(state, move |b| commands::get_properties(b, &scope, &filter)).await
}

#[tauri::command]
async fn get_entity(state: State<'_, AppState>, scope: ScopeArgs, key: String) -> CmdResult<EntityDetail> {
    run_blocking(state, move |b| commands::get_entity(b, &scope, &key)).await
}

#[tauri::command]
async fn search(state: State<'_, AppState>, scope: ScopeArgs, query: String, limit: usize) -> CmdResult<Vec<SearchHit>> {
    run_blocking(state, move |b| commands::search(b, &scope, &query, limit)).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(AppState::load_in_background());
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
        .invoke_handler(tauri::generate_handler![get_meta, get_tree, get_subtree, get_focus, get_properties, get_entity, search])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
