// Başsız (penceresiz) mod: Rust veri kaynaklarını ve HTTP/SSE uç noktalarını test etmek için.
// Kullanım: cargo run --release --bin headless
use finans_core::{server, sources, state::AppState, storage::Storage};
use std::path::PathBuf;

const INDEX_HTML: &str = include_str!("../../../frontend/index.html");

#[tokio::main]
async fn main() {
    let state = AppState::new(Storage::new(PathBuf::from(".")));
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 4501)).await.unwrap();
    let router = server::build_router(state.clone(), INDEX_HTML);
    eprintln!("[headless] http://127.0.0.1:4501");
    tokio::spawn(async move { sources::start_all(state).await });
    axum::serve(listener, router).await.unwrap();
}
