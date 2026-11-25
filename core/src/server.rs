use crate::state::AppState;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use futures_util::stream::{Stream, StreamExt};
use serde_json::json;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;

/// Arayüz HTML'si platform tarafından sağlanır (masaüstü/mobil farklı olabilir).
pub fn build_router(state: AppState, index_html: &'static str) -> Router {
    Router::new()
        .route("/", get(move || async move { axum::response::Html(index_html) }))
        .route("/health", get(health))
        .route("/prices", get(prices))
        .route("/prices/{code}", get(price_one))
        .route("/stream", get(harem_stream))
        .route("/tcmb", get(tcmb))
        .route("/bist", get(bist))
        .route("/kripto", get(kripto))
        .route("/kripto/stream", get(kripto_stream))
        .route("/user-data", get(get_user_data).post(post_user_data))
        .with_state(state)
}

async fn health(State(st): State<AppState>) -> Json<serde_json::Value> {
    let age = if st.last_event.load(std::sync::atomic::Ordering::Relaxed) == 0 {
        None
    } else {
        Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64
                - st.last_event.load(std::sync::atomic::Ordering::Relaxed),
        )
    };
    let stale = age.is_none_or(|a| a > 60_000);
    let k = st.kripto.read().await.clone();
    Json(json!({
        "connected": st.connected.load(std::sync::atomic::Ordering::Relaxed),
        "stale": stale,
        "lastEventAgeMs": age,
        "count": st.prices.read().await.len(),
        "kripto": {
            "usd": k["sources"]["usd"],
            "tl": k["sources"]["tl"],
            "stale": k["stale"],
            "count": k["withData"],
        },
        "uptimeSec": 0,
        "fetchedAt": chrono::Utc::now().to_rfc3339(),
        "sources": st.health_snapshot().await,
    }))
}

async fn prices(State(st): State<AppState>) -> Json<Vec<serde_json::Value>> {
    Json(st.prices.read().await.clone())
}

async fn price_one(
    State(st): State<AppState>,
    axum::extract::Path(code): axum::extract::Path<String>,
) -> impl IntoResponse {
    let code = code.trim().to_uppercase();
    let found = st.prices.read().await.iter().find(|p| p["code"] == code).cloned();
    match found {
        Some(v) => Json(v).into_response(),
        None => (axum::http::StatusCode::NOT_FOUND, Json(json!({"error": "bulunamadı"}))).into_response(),
    }
}

async fn get_user_data(State(st): State<AppState>) -> Json<serde_json::Value> {
    Json(st.storage.load().await)
}

async fn post_user_data(
    State(st): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    match st.storage.save(&body).await {
        Ok(()) => (axum::http::StatusCode::OK, Json(json!({"ok": true}))).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        ).into_response(),
    }
}

async fn tcmb(State(st): State<AppState>) -> Json<serde_json::Value> {
    Json(st.tcmb.read().await.clone())
}

async fn bist(State(st): State<AppState>) -> Json<serde_json::Value> {
    Json(st.bist.read().await.clone())
}

async fn kripto(State(st): State<AppState>) -> Json<serde_json::Value> {
    Json(st.kripto.read().await.clone())
}

fn sse_from(rx: tokio::sync::broadcast::Receiver<String>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(rx).filter_map(|msg| async move {
        match msg {
            Ok(data) => Some(Ok(Event::default().data(data))),
            Err(_) => None,
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn harem_stream(State(st): State<AppState>) -> impl IntoResponse {
    sse_from(st.harem_tx.subscribe())
}

async fn kripto_stream(State(st): State<AppState>) -> impl IntoResponse {
    sse_from(st.kripto_tx.subscribe())
}
