use finans_core::{server, sources, state::AppState, storage::Storage};
use serde_json::{json, Value};
use tauri::Manager;
use tauri_plugin_updater::UpdaterExt;

const INDEX_HTML: &str = include_str!("../../frontend/index.html");

/// GitHub `latest.json` üzerinden yeni sürüm kontrolü (açılışta ve Ayarlar'da çağrılır).
#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<Value, String> {
    let updater = app.updater_builder().build().map_err(|e| e.to_string())?;
    match updater.check().await.map_err(|e| e.to_string())? {
        Some(update) => Ok(json!({
            "available": true,
            "version": update.version,
            "current": update.current_version,
            "notes": update.body.unwrap_or_default(),
        })),
        None => Ok(json!({ "available": false })),
    }
}

/// İmzalı paketi indirir, kurar ve uygulamayı yeniden başlatır.
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    let updater = app.updater_builder().build().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Yeni sürüm bulunamadı".to_string())?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())
}

pub fn run_app() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![check_update, install_update])
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let state = AppState::new(Storage::new(data_dir));
            // Son veriyi diskden geri yükle; ağ verisi gelene kadar boş ekran olmasın.
            tauri::async_runtime::block_on(state.restore_cache());

            // Sabit port: WebView origin'i her açılışta aynı olur (localStorage yedeği için).
            let port = tauri::async_runtime::block_on(async {
                let mut bound = None;
                for candidate in 4510..4520u16 {
                    if let Ok(l) = tokio::net::TcpListener::bind(("127.0.0.1", candidate)).await {
                        bound = Some(l);
                        break;
                    }
                }
                let listener = match bound {
                    Some(l) => l,
                    None => tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?,
                };
                let port = listener.local_addr()?.port();
                let router = server::build_router(state.clone(), INDEX_HTML);
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = axum::serve(listener, router).await {
                        eprintln!("[sunucu] {e}");
                    }
                });
                Ok::<u16, std::io::Error>(port)
            })?;

            app.manage(state.clone());

            let url: tauri::Url = format!("http://127.0.0.1:{port}").parse()?;
            tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::External(url))
                .title("Finans Takipçisi")
                .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?)?
                .maximized(true)
                .resizable(true)
                .build()?;

            let s = state.clone();
            tauri::async_runtime::spawn(async move { sources::start_all(s).await; });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("uygulama başlatılamadı");
}
