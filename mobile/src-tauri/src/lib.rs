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

/// Kullanıcının seçtiği JSON dosyasını okur (içe aktarma).
#[tauri::command]
fn read_json_file(path: String) -> Result<String, String> {
    if !path.to_lowercase().ends_with(".json") {
        return Err("Yalnızca .json dosyaları desteklenir".into());
    }
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

/// Kullanıcının seçtiği yola JSON yazar (dışa aktarma).
#[tauri::command]
fn write_json_file(path: String, contents: String) -> Result<(), String> {
    if !path.to_lowercase().ends_with(".json") {
        return Err("Yalnızca .json dosyaları desteklenir".into());
    }
    std::fs::write(&path, contents).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_user_data(state: tauri::State<'_, AppState>, body: Value) -> Result<(), String> {
    tauri::async_runtime::block_on(state.storage.save(&body)).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let state = AppState::new(Storage::new(data_dir));
            tauri::async_runtime::block_on(state.restore_cache());
            app.manage(state.clone());
            tauri::async_runtime::spawn(async move { sources::start_all(state).await });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            get_user_data,
            save_user_data,
            read_json_file,
            write_json_file
        ])
        .run(tauri::generate_context!())
        .expect("mobil uygulama başlatılamadı");
}
