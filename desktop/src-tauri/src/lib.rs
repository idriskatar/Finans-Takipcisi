use finans_core::{server, sources, state::AppState, storage::Storage};
use tauri::Manager;

const INDEX_HTML: &str = include_str!("../../frontend/index.html");

pub fn run_app() {
    tauri::Builder::default()
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
