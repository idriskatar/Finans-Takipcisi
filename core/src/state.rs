use crate::storage::Storage;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

const GROUP_ORDER: [&str; 6] = ["altin", "sarrafiye", "doviz", "parite", "maden", "diger"];

#[derive(Clone, Serialize)]
pub struct SourceHealth {
    pub ok: bool,
    pub connected: bool,
    pub last_data_at: Option<String>,
    pub error: Option<String>,
    pub from_cache: bool,
}

impl Default for SourceHealth {
    fn default() -> Self {
        Self { ok: false, connected: false, last_data_at: None, error: None, from_cache: false }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub prices: Arc<RwLock<Vec<Value>>>,
    pub price_map: Arc<RwLock<HashMap<String, Value>>>,
    pub tcmb: Arc<RwLock<Value>>,
    pub bist: Arc<RwLock<Value>>,
    pub kripto: Arc<RwLock<Value>>,
    pub harem_tx: broadcast::Sender<String>,
    pub kripto_tx: broadcast::Sender<String>,
    pub last_event: Arc<AtomicU64>,
    pub connected: Arc<AtomicBool>,
    pub storage: Storage,
    pub health: Arc<RwLock<HashMap<String, SourceHealth>>>,
}

impl AppState {
    pub fn new(storage: Storage) -> Self {
        let (harem_tx, _) = broadcast::channel(32);
        let (kripto_tx, _) = broadcast::channel(32);
        Self {
            prices: Arc::new(RwLock::new(Vec::new())),
            price_map: Arc::new(RwLock::new(HashMap::new())),
            tcmb: Arc::new(RwLock::new(serde_json::json!({}))),
            bist: Arc::new(RwLock::new(serde_json::json!({}))),
            kripto: Arc::new(RwLock::new(serde_json::json!({}))),
            harem_tx,
            kripto_tx,
            last_event: Arc::new(AtomicU64::new(0)),
            connected: Arc::new(AtomicBool::new(false)),
            storage,
            health: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Kaynak durumunu günceller. ok=true ise son veri zamanı şimdi olur.
    pub async fn mark_source(&self, name: &str, ok: bool, connected: bool, error: Option<String>) {
        let mut h = self.health.write().await;
        let e = h.entry(name.to_string()).or_default();
        e.ok = ok;
        e.connected = connected;
        e.error = error;
        e.from_cache = false;
        if ok { e.last_data_at = Some(chrono::Utc::now().to_rfc3339()); }
    }

    pub async fn health_snapshot(&self) -> HashMap<String, SourceHealth> {
        self.health.read().await.clone()
    }

    /// Açılışta diskdeki son veriyi yükler; ağ verisi gelene kadar ekran boş kalmaz.
    pub async fn restore_cache(&self) {
        let now = chrono::Utc::now().to_rfc3339();
        if let Some(w) = self.storage.load_cache("harem").await {
            if let Some(data) = w.get("data").and_then(Value::as_array) {
                *self.prices.write().await = data.clone();
                let mut h = self.health.write().await;
                let e = h.entry("harem".into()).or_default();
                e.ok = true; e.from_cache = true;
                e.last_data_at = Some(w.get("at").and_then(Value::as_str).unwrap_or(&now).to_string());
            }
        }
        for (name, target) in [
            ("tcmb", &self.tcmb),
            ("bist", &self.bist),
            ("kripto", &self.kripto),
        ] {
            if let Some(w) = self.storage.load_cache(name).await {
                if let Some(data) = w.get("data") {
                    *target.write().await = data.clone();
                    let mut h = self.health.write().await;
                    let e = h.entry(name.to_string()).or_default();
                    e.ok = true; e.from_cache = true;
                    e.last_data_at = Some(w.get("at").and_then(Value::as_str).unwrap_or(&now).to_string());
                }
            }
        }
    }

    pub async fn flush_cache(&self) {
        let now = chrono::Utc::now().to_rfc3339();
        {
            let p = self.prices.read().await;
            if !p.is_empty() {
                let _ = self.storage.save_cache("harem", &Value::Array(p.clone())).await;
                let mut h = self.health.write().await;
                if let Some(e) = h.get_mut("harem") { e.last_data_at = Some(now.clone()); }
            }
        }
        for (name, target) in [
            ("tcmb", &self.tcmb),
            ("bist", &self.bist),
            ("kripto", &self.kripto),
        ] {
            let v = target.read().await.clone();
            let has = v.get("items").and_then(Value::as_array).map(|a| !a.is_empty())
                .or_else(|| v.get("today").and_then(|t| t.get("items")).and_then(Value::as_object).map(|o| !o.is_empty()))
                .or_else(|| v.get("count").and_then(Value::as_u64).map(|n| n > 0))
                .unwrap_or(false);
            if has {
                let _ = self.storage.save_cache(name, &v).await;
                let mut h = self.health.write().await;
                if let Some(e) = h.get_mut(name) { e.last_data_at = Some(now.clone()); }
            }
        }
    }

    pub fn set_connected(&self, v: bool) {
        self.connected.store(v, Ordering::Relaxed);
    }

    fn rank(group: &str) -> usize {
        GROUP_ORDER.iter().position(|g| *g == group).unwrap_or(GROUP_ORDER.len())
    }

    fn sorted(map: &HashMap<String, Value>) -> Vec<Value> {
        let mut v: Vec<Value> = map.values().cloned().collect();
        v.sort_by(|a, b| {
            let ga = a.get("group").and_then(Value::as_str).unwrap_or("");
            let gb = b.get("group").and_then(Value::as_str).unwrap_or("");
            Self::rank(ga)
                .cmp(&Self::rank(gb))
                .then_with(|| {
                    let na = a.get("name").and_then(Value::as_str).unwrap_or("");
                    let nb = b.get("name").and_then(Value::as_str).unwrap_or("");
                    na.locale_compare(nb)
                })
        });
        v
    }

    pub async fn apply_prices(&self, changed: Vec<Value>) {
        {
            let mut map = self.price_map.write().await;
            for item in changed {
                if let Some(code) = item.get("code").and_then(Value::as_str) {
                    map.insert(code.to_string(), item);
                }
            }
            let list = Self::sorted(&map);
            *self.prices.write().await = list.clone();
            self.last_event.store(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                Ordering::Relaxed,
            );
            if let Ok(s) = serde_json::to_string(&list) {
                let _ = self.harem_tx.send(s);
            }
        }
    }

    pub async fn set_tcmb(&self, v: Value) {
        *self.tcmb.write().await = v;
    }

    pub async fn set_bist(&self, v: Value) {
        *self.bist.write().await = v;
    }

    pub async fn set_kripto(&self, v: Value) {
        let snapshot = match serde_json::to_string(&v) {
            Ok(s) => s,
            Err(_) => return,
        };
        *self.kripto.write().await = v;
        let _ = self.kripto_tx.send(snapshot);
    }
}

// Türkçe alfabe için basit karşılaştırma (locale'den bağımsız, Türkçe harfleri korur).
trait LocaleCompare {
    fn locale_compare(&self, other: &str) -> std::cmp::Ordering;
}
impl LocaleCompare for str {
    fn locale_compare(&self, other: &str) -> std::cmp::Ordering {
        const TR: [(char, char); 6] = [
            ('ç', 'c'), ('ğ', 'g'), ('ı', 'i'), ('ö', 'o'), ('ş', 's'), ('ü', 'u'),
        ];
        let key = |s: &str| -> String {
            s.chars()
                .map(|c| {
                    let low = c.to_lowercase().next().unwrap_or(c);
                    TR.iter().find(|(t, _)| *t == low).map(|(_, r)| *r).unwrap_or(low)
                })
                .collect()
        };
        let a = key(self);
        let b = key(other);
        a.cmp(&b).then_with(|| self.cmp(other))
    }
}
