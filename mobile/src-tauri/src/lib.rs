use finans_core::{sources, state::AppState, storage::Storage};
use serde_json::{json, Value};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

const RELEASES_API: &str = "https://api.github.com/repos/idriskatar/Finans-Takipcisi/releases/latest";

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

/// GitHub Releases üzerinden yeni sürüm kontrolü (Play Store kullanılmaz).
#[tauri::command]
async fn check_update() -> Value {
    let client = match reqwest::Client::builder()
        .user_agent("Finans-Takipcisi-Updater")
        .build()
    {
        Ok(c) => c,
        Err(_) => return json!({ "available": false }),
    };
    let release: Value = match client.get(RELEASES_API).send().await {
        Ok(r) if r.status().is_success() => match r.json().await {
            Ok(v) => v,
            Err(_) => return json!({ "available": false }),
        },
        _ => return json!({ "available": false }),
    };

    let tag = release["tag_name"].as_str().unwrap_or("").trim_start_matches('v');
    let current = env!("CARGO_PKG_VERSION");
    let parse = |s: &str| -> Vec<u64> {
        s.split('.').map(|x| x.parse::<u64>().unwrap_or(0)).collect()
    };
    let newer = parse(tag) > parse(current);
    let apk = release["assets"].as_array().and_then(|a| {
        a.iter().find(|x| {
            x["name"].as_str().map(|n| n.to_lowercase().ends_with(".apk")).unwrap_or(false)
        })
    });

    json!({
        "available": newer && apk.is_some(),
        "version": tag,
        "current": current,
        "notes": release["body"].as_str().unwrap_or(""),
        "url": apk.and_then(|a| a["browser_download_url"].as_str()).unwrap_or(""),
    })
}

/// APK bağlantısını sistem tarayıcısında açar (kullanıcı indirip kurar).
#[tauri::command]
fn open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    app.opener().open_url(&url, None::<&str>).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
            check_update,
            open_url
        ])
        .run(tauri::generate_context!())
        .expect("mobil uygulama başlatılamadı");
}
