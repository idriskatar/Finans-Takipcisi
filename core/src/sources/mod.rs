mod bist;
mod fallback;
mod harem;
mod kripto;
mod tcmb;

use crate::state::AppState;

pub async fn start_all(state: AppState) {
    let h = state.clone();
    tokio::spawn(async move { harem::run(h).await });
    let t = state.clone();
    tokio::spawn(async move { tcmb::run(t).await });
    let b = state.clone();
    tokio::spawn(async move { bist::run(b).await });
    let k = state.clone();
    tokio::spawn(async move { kripto::run(k).await });
    let c = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            c.flush_cache().await;
        }
    });
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
