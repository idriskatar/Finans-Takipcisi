use finans_core::{sources, state::AppState, storage::Storage};
use serde_json::{json, Value};
use tauri::Manager;

#[tauri::command]
fn snapshot(state: tauri::State<'_, AppState>) -> Value {
    let health = tauri::async_runtime::block_on(state.health_snapshot());
    json!({
        "prices": tauri::async_runtime::block_on(async { state.prices.read().await.clone() }),
        "tcmb": tauri::async_runtime::block_on(async { state.tcmb.read().await.clone() }),
        "bist": tauri::async_runtime::block_on(async { state.bist.read().await.clone() }),
        "kripto": tauri::async_runtime::block_on(async { state.kripto.read().await.clone() }),
        "health": health,
    })
}

#[tauri::command]
fn get_user_data(state: tauri::State<'_, AppState>) -> Value {
    tauri::async_runtime::block_on(state.storage.load())
}

#[tauri::command]
fn save_user_data(state: tauri::State<'_, AppState>, body: Value) -> Result<(), String> {
    tauri::async_runtime::block_on(state.storage.save(&body)).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let state = AppState::new(Storage::new(data_dir));
            tauri::async_runtime::block_on(state.restore_cache());
            app.manage(state.clone());
            tauri::async_runtime::spawn(async move { sources::start_all(state).await });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![snapshot, get_user_data, save_user_data])
        .run(tauri::generate_context!())
        .expect("mobil uygulama başlatılamadı");
}
