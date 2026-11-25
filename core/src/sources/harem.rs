use crate::state::AppState;
use crate::sources::now_ms;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::Duration;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

const SOCKET_URL: &str = "wss://hrmsocketonly.haremaltin.com/socket.io/?EIO=4&transport=websocket";
const ORIGIN: &str = "https://canlipiyasalar.haremaltin.com";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0 Safari/537.36";

const LABELS: &[(&str, &str, &str)] = &[
    ("USDTRY", "Dolar / TL", "doviz"), ("EURTRY", "Euro / TL", "doviz"),
    ("GBPTRY", "Sterlin / TL", "doviz"), ("CHFTRY", "İsviçre Frangı / TL", "doviz"),
    ("AUDTRY", "Avustralya Doları / TL", "doviz"), ("CADTRY", "Kanada Doları / TL", "doviz"),
    ("JPYTRY", "Japon Yeni / TL", "doviz"), ("SARTRY", "Suudi Riyali / TL", "doviz"),
    ("KWDTRY", "Kuveyt Dinarı / TL", "doviz"), ("JODTRY", "Ürdün Dinarı / TL", "doviz"),
    ("EURUSD", "Euro / Dolar", "parite"), ("GBPUSD", "Sterlin / Dolar", "parite"),
    ("USDCHF", "Dolar / Frank", "parite"), ("USDCAD", "Dolar / Kanada Doları", "parite"),
    ("AUDUSD", "Avustralya Doları / Dolar", "parite"), ("USDJPY", "Dolar / Yen", "parite"),
    ("ALTIN", "Altın (gram)", "altin"), ("AYAR22", "22 Ayar Altın (gram)", "altin"),
    ("AYAR14", "14 Ayar Altın (gram)", "altin"), ("KULCEALTIN", "Külçe Altın", "altin"),
    ("ONS", "Ons Altın (USD)", "altin"), ("USDKG", "Altın Dolar/Kg", "altin"),
    ("EURKG", "Altın Euro/Kg", "altin"),
    ("CEYREK_YENI", "Çeyrek (Yeni)", "sarrafiye"), ("CEYREK_ESKI", "Çeyrek (Eski)", "sarrafiye"),
    ("YARIM_YENI", "Yarım (Yeni)", "sarrafiye"), ("YARIM_ESKI", "Yarım (Eski)", "sarrafiye"),
    ("TEK_YENI", "Tam (Yeni)", "sarrafiye"), ("TEK_ESKI", "Tam (Eski)", "sarrafiye"),
    ("ATA_YENI", "Ata (Yeni)", "sarrafiye"), ("ATA_ESKI", "Ata (Eski)", "sarrafiye"),
    ("ATA5_YENI", "Ata 5'li (Yeni)", "sarrafiye"), ("ATA5_ESKI", "Ata 5'li (Eski)", "sarrafiye"),
    ("GREMESE_YENI", "Gremese (Yeni)", "sarrafiye"), ("GREMESE_ESKI", "Gremese (Eski)", "sarrafiye"),
    ("GUMUSTRY", "Gümüş (TL)", "maden"), ("GUMUSUSD", "Gümüş (USD)", "maden"),
    ("XAGUSD", "Ons Gümüş (USD)", "maden"), ("XPTUSD", "Ons Platin (USD)", "maden"),
    ("XPDUSD", "Ons Paladyum (USD)", "maden"), ("PLATIN", "Platin", "maden"),
    ("PALADYUM", "Paladyum", "maden"),
];

fn to_number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64().filter(|n| n.is_finite()),
        Value::String(s) => {
            let mut t = s.trim().replace(['\u{00a0}', '\u{202f}', ' '], "");
            if t.is_empty() { return None; }
            let dot = t.contains('.');
            let comma = t.contains(',');
            if dot && comma {
                t = if t.rfind(',').unwrap_or(0) > t.rfind('.').unwrap_or(0) {
                    t.replace('.', "").replace(',', ".")
                } else {
                    t.replace(',', "")
                };
            } else if comma {
                let parts: Vec<&str> = t.split(',').collect();
                t = if parts.len() == 2 {
                    format!("{}.{}", parts[0], parts[1])
                } else {
                    t.replace(',', "")
                };
            } else if dot && t.matches('.').count() > 1 {
                t = t.replace('.', "");
            }
            t.parse::<f64>().ok().filter(|n| n.is_finite())
        }
        _ => None,
    }
}

fn parse_tarih(v: &Value) -> Option<String> {
    let s = v.as_str()?;
    if s.trim().is_empty() { return None; }
    use chrono::{FixedOffset, NaiveDateTime, TimeZone};
    let off = FixedOffset::east_opt(3 * 3600)?;
    for fmt in ["%d-%m-%Y %H:%M:%S", "%d.%m.%Y %H:%M:%S", "%d/%m/%Y %H:%M:%S",
                "%d-%m-%Y %H:%M", "%d.%m.%Y %H:%M", "%d/%m/%Y %H:%M"] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(s.trim(), fmt) {
            return Some(off.from_local_datetime(&naive).single()?.to_rfc3339());
        }
    }
    None
}

fn dir_of(v: Option<&Value>) -> Option<&'static str> {
    match v {
        Some(Value::String(s)) if s == "up" || s == "1" => Some("up"),
        Some(Value::String(s)) if s == "down" || s == "-1" => Some("down"),
        Some(Value::Number(n)) if n.as_f64() == Some(1.0) => Some("up"),
        Some(Value::Number(n)) if n.as_f64() == Some(-1.0) => Some("down"),
        _ => None,
    }
}

fn normalize_item(key: &str, raw: &Value) -> Option<Value> {
    if !raw.is_object() { return None; }
    let buy = to_number(&raw["alis"]);
    let sell = to_number(&raw["satis"]);
    if (buy.is_none() || buy == Some(0.0)) && (sell.is_none() || sell == Some(0.0)) {
        return None;
    }
    let code = raw["code"].as_str().unwrap_or(key).trim().to_uppercase();
    if code.is_empty() { return None; }
    let (name, group) = LABELS
        .iter()
        .find(|(c, _, _)| *c == code)
        .map(|(_, n, g)| (*n, *g))
        .unwrap_or((code.as_str(), "diger"));

    let mut low = to_number(&raw["dusuk"]);
    let mut high = to_number(&raw["yuksek"]);
    if let (Some(l), Some(h)) = (low, high) {
        if l > h { std::mem::swap(&mut low, &mut high); }
    }
    let spread = match (buy, sell) {
        (Some(b), Some(s)) if b != 0.0 && s > 0.0 => Some(((s - b) / s * 100.0 * 100.0).round() / 100.0),
        _ => None,
    };
    let suspect = spread.is_some_and(|s| s < 0.0 || s > 5.0);

    Some(json!({
        "code": code,
        "name": name,
        "group": group,
        "buy": buy,
        "sell": sell,
        "low": low,
        "high": high,
        "prevClose": to_number(&raw["kapanis"]),
        "buyDir": dir_of(raw["dir"].get("alis_dir")),
        "sellDir": dir_of(raw["dir"].get("satis_dir")),
        "spreadPct": spread,
        "suspect": suspect,
        "updatedAt": parse_tarih(&raw["tarih"]),
    }))
}

fn normalize_payload(payload: Option<&Value>) -> Vec<Value> {
    let mut out = Vec::new();
    if let Some(data) = payload.and_then(|p| p.get("data")).and_then(Value::as_object) {
        for (key, raw) in data {
            if let Some(item) = normalize_item(key, raw) { out.push(item); }
        }
    }
    out
}

pub async fn run(state: AppState) {
    let mut retry = 2u64;
    loop {
        match connect_once(&state).await {
            Ok(()) => retry = 2,
            Err(e) => {
                eprintln!("[harem] bağlantı hatası: {e}");
                state.mark_source("harem", false, false, Some(e.to_string())).await;
            }
        }
        state.set_connected(false);
        tokio::time::sleep(Duration::from_secs(retry)).await;
        retry = (retry * 2).min(30);
    }
}

async fn connect_once(state: &AppState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut request = SOCKET_URL.into_client_request()?;
    request.headers_mut().insert("Origin", ORIGIN.parse()?);
    request.headers_mut().insert("User-Agent", UA.parse()?);
    let (mut ws, _) = tokio_tungstenite::connect_async(request).await?;

    // Engine.IO açılış paketini bekle (0{...})
    while let Some(msg) = ws.next().await {
        let text = msg?.into_text()?;
        if text.starts_with('0') { break; }
    }
    // Socket.IO CONNECT (varsayılan namespace)
    ws.send(Message::Text("40".into())).await?;
    state.set_connected(true);
    state.mark_source("harem", true, true, None).await;
    eprintln!("[harem] bağlandı");

    while let Some(msg) = ws.next().await {
        match msg? {
            Message::Text(text) => {
                if text == "2" {
                    ws.send(Message::Text("3".into())).await?;
                    continue;
                }
                if let Some(payload) = text.strip_prefix("42") {
                    if let Ok(arr) = serde_json::from_str::<Vec<Value>>(payload) {
                        if arr.first().and_then(Value::as_str) == Some("price_changed") {
                            let changed = normalize_payload(arr.get(1));
                            if !changed.is_empty() { state.apply_prices(changed).await; }
                        }
                    }
                }
            }
            Message::Ping(p) => ws.send(Message::Pong(p)).await?,
            Message::Close(_) => break,
            _ => {}
        }
    }
    let _ = now_ms();
    Ok(())
}
