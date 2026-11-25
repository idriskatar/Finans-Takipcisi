use crate::state::AppState;
use crate::sources::now_ms;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0 Safari/537.36";
const COINS: &[(&str, &str)] = &[
    ("BTC", "Bitcoin"), ("ETH", "Ethereum"), ("BNB", "BNB"), ("SOL", "Solana"),
    ("XRP", "XRP"), ("ADA", "Cardano"), ("DOGE", "Dogecoin"), ("AVAX", "Avalanche"),
    ("LINK", "Chainlink"), ("TRX", "TRON"), ("DOT", "Polkadot"), ("LTC", "Litecoin"),
    ("SHIB", "Shiba Inu"), ("TON", "Toncoin"), ("SUI", "Sui"), ("NEAR", "NEAR Protocol"),
    ("APT", "Aptos"), ("ARB", "Arbitrum"), ("OP", "Optimism"), ("PEPE", "Pepe"),
    ("XLM", "Stellar"), ("BCH", "Bitcoin Cash"), ("HBAR", "Hedera"), ("UNI", "Uniswap"),
    ("ATOM", "Cosmos"), ("ETC", "Ethereum Classic"),
];
const BIN_REST: &str = "https://api.binance.com/api/v3/ticker/24hr";
const BIN_WS: &str = "wss://stream.binance.com:9443/stream?streams=";
const BT_REST: &str = "https://api.btcturk.com/api/v2/ticker";
const BT_HOSTS: [&str; 2] = ["wss://ws-feed-pro.btcturk.com/", "wss://ws.global.btcturk.com"];
const FRESH_MS: u64 = 5 * 60_000;

#[derive(Default, Clone)]
struct Coin {
    bu: Option<f64>, bp: Option<f64>, bh: Option<f64>, bl: Option<f64>, bq: Option<f64>, bat: u64,
    uu: Option<f64>, up: Option<f64>, uat: u64,
    tl: Option<f64>, tp: Option<f64>, th: Option<f64>, tlow: Option<f64>, tv: Option<f64>, tat: u64,
    real_tl: bool,
}

#[derive(Default)]
struct Crypto {
    coins: HashMap<String, Coin>,
    usdt_try: Option<f64>, usdt_try_pct: Option<f64>, usdt_try_at: u64,
    binance_on: bool, btcturk_on: bool,
    binance_err: Option<String>, btcturk_err: Option<String>,
    dirty: bool,
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64().filter(|n| n.is_finite()),
        Value::String(s) if !s.is_empty() => s.parse::<f64>().ok().filter(|n| n.is_finite()),
        _ => None,
    }
}
fn round(v: Option<f64>, d: i32) -> Option<f64> {
    let k = 10f64.powi(d);
    v.map(|n| (n * k).round() / k)
}
fn coin_mut<'a>(c: &'a mut Crypto, code: &str) -> &'a mut Coin {
    c.coins.entry(code.to_string()).or_default()
}

fn apply_binance(c: &mut Crypto, q: &Value) {
    let Some(code) = q["symbol"].as_str().and_then(|s| s.strip_suffix("USDT")) else { return };
    let Some(price) = num(&q["lastPrice"]) else { return };
    let s = coin_mut(c, code);
    s.bu = Some(price);
    s.bp = num(&q["priceChangePercent"]);
    s.bh = num(&q["highPrice"]);
    s.bl = num(&q["lowPrice"]);
    s.bq = num(&q["quoteVolume"]);
    s.bat = num(&q["closeTime"]).map(|n| n as u64).unwrap_or_else(now_ms);
    c.dirty = true;
}

fn apply_bt_rest(c: &mut Crypto, t: &Value) {
    let ns = t["numeratorSymbol"].as_str().unwrap_or("").to_uppercase();
    let ds = t["denominatorSymbol"].as_str().unwrap_or("").to_uppercase();
    let Some(last) = num(&t["last"]) else { return };
    if ds == "TRY" && ns == "USDT" {
        c.usdt_try = Some(last);
        c.usdt_try_pct = num(&t["dailyPercent"]);
        c.usdt_try_at = num(&t["timestamp"]).map(|n| n as u64).unwrap_or_else(now_ms);
        c.dirty = true;
        return;
    }
    if ds == "USDT" && COINS.iter().any(|(code, _)| *code == ns) {
        let s = coin_mut(c, &ns);
        s.uu = Some(last);
        s.up = num(&t["dailyPercent"]);
        s.uat = num(&t["timestamp"]).map(|n| n as u64).unwrap_or_else(now_ms);
        c.dirty = true;
        return;
    }
    if ds != "TRY" || !COINS.iter().any(|(code, _)| *code == ns) { return; }
    let s = coin_mut(c, &ns);
    s.tl = Some(last);
    s.tp = num(&t["dailyPercent"]);
    if let Some(h) = num(&t["high"]) { s.th = Some(h); }
    if let Some(l) = num(&t["low"]) { s.tlow = Some(l); }
    if let (Some(v), Some(a)) = (num(&t["volume"]), num(&t["average"])) { s.tv = Some(v * a); }
    s.tat = num(&t["timestamp"]).map(|n| n as u64).unwrap_or_else(now_ms);
    s.real_tl = true;
    c.dirty = true;
}

fn apply_bt_ws(c: &mut Crypto, p: &Value) {
    let ns = p["NS"].as_str().unwrap_or("").to_uppercase();
    let ds = p["DS"].as_str().unwrap_or("").to_uppercase();
    let last = num(&p["LA"]).or_else(|| num(&p["A"]));
    let Some(last) = last else { return };
    if ds == "TRY" && ns == "USDT" {
        c.usdt_try = Some(last);
        c.usdt_try_pct = num(&p["DP"]);
        c.usdt_try_at = now_ms();
        c.dirty = true;
        return;
    }
    if ds != "TRY" || !COINS.iter().any(|(code, _)| *code == ns) { return; }
    let s = coin_mut(c, &ns);
    s.tl = Some(last);
    s.tp = num(&p["DP"]);
    if let Some(h) = num(&p["H"]) { s.th = Some(h); }
    if let Some(l) = num(&p["L"]) { s.tlow = Some(l); }
    if let (Some(v), Some(a)) = (num(&p["V"]), num(&p["AV"])) { s.tv = Some(v * a); }
    s.tat = now_ms();
    s.real_tl = true;
    c.dirty = true;
}

fn apply_binance_ws(c: &mut Crypto, d: &Value) {
    let Some(code) = d["s"].as_str().and_then(|s| s.strip_suffix("USDT")) else { return };
    let Some(price) = num(&d["c"]) else { return };
    let s = coin_mut(c, code);
    s.bu = Some(price);
    if let (Some(o), Some(cp)) = (num(&d["o"]), Some(price)) {
        if o != 0.0 { s.bp = Some((cp / o - 1.0) * 100.0); }
    }
    if let Some(h) = num(&d["h"]) { s.bh = Some(h); }
    if let Some(l) = num(&d["l"]) { s.bl = Some(l); }
    if let Some(q) = num(&d["q"]) { s.bq = Some(q); }
    s.bat = num(&d["E"]).map(|n| n as u64).unwrap_or_else(now_ms);
    c.dirty = true;
}

fn snapshot(c: &Crypto) -> Value {
    let now = now_ms();
    let items: Vec<Value> = COINS.iter().map(|(code, name)| {
        let s = c.coins.get(*code).cloned().unwrap_or_default();
        let bin_fresh = s.bat > 0 && now - s.bat < FRESH_MS;
        let alt_fresh = s.uat > 0 && now - s.uat < FRESH_MS;
        let use_alt = s.uu.is_some() && !bin_fresh && (alt_fresh || s.uat > s.bat);

        let mut usd = if use_alt { s.uu } else { s.bu };
        let mut usd_pct = if use_alt { s.up } else { s.bp };
        let mut usd_high = if use_alt { None } else { s.bh };
        let mut usd_low = if use_alt { None } else { s.bl };
        let mut vol_usd = if use_alt { None } else { s.bq };
        let mut time_ms = if use_alt { s.uat } else { s.bat };
        let mut usd_src = if use_alt { "btcturk" } else if s.bu.is_some() { "binance" } else { "" };

        if usd.is_none() && s.bu.is_some() {
            usd = s.bu; usd_pct = s.bp; usd_high = s.bh; usd_low = s.bl;
            vol_usd = s.bq; time_ms = s.bat; usd_src = "binance";
        }

        let try_fresh = s.real_tl && s.tat > 0 && now - s.tat < FRESH_MS;
        let mut tl = s.tl;
        let mut tl_pct = s.tp;
        let mut tl_high = s.th;
        let mut tl_low = s.tlow;
        let mut vol_tl = s.tv;
        let mut estimated = false;
        if !try_fresh && usd.is_some() && c.usdt_try.is_some() {
            let rate = c.usdt_try.unwrap();
            tl = usd.map(|v| v * rate);
            tl_high = usd_high.map(|v| v * rate);
            tl_low = usd_low.map(|v| v * rate);
            tl_pct = usd_pct.map(|p| (1.0 + p / 100.0) * (1.0 + c.usdt_try_pct.unwrap_or(0.0) / 100.0) * 100.0 - 100.0);
            vol_tl = vol_usd.map(|v| v * rate);
            estimated = true;
            if time_ms == 0 { time_ms = c.usdt_try_at; }
        }
        let vol_tl_approx = vol_usd.is_some() && c.usdt_try.is_some();
        if vol_tl_approx { vol_tl = vol_usd.map(|v| v * c.usdt_try.unwrap()); }

        let time = if time_ms > 0 {
            chrono::DateTime::from_timestamp_millis(time_ms as i64).map(|d| d.to_rfc3339())
        } else { None };
        let age = if time_ms > 0 { Some(now.saturating_sub(time_ms) / 1000) } else { None };
        let tl_age = if s.real_tl && s.tat > 0 { Some(now.saturating_sub(s.tat) / 1000) } else { None };

        json!({
            "code": code, "name": name,
            "usd": round(usd, 8), "usdPct": round(usd_pct, 4),
            "usdHigh": round(usd_high, 8), "usdLow": round(usd_low, 8),
            "volUsd": round(vol_usd, 2),
            "tl": round(tl, 8), "tlPct": round(tl_pct, 4),
            "tlHigh": round(tl_high, 8), "tlLow": round(tl_low, 8),
            "volTl": round(vol_tl, 2),
            "volTlApprox": vol_tl_approx, "estimated": estimated,
            "usdSrc": if usd_src.is_empty() { Value::Null } else { json!(usd_src) },
            "tlSrc": if estimated { json!("tahmini") } else if s.real_tl { json!("btcturk") } else { Value::Null },
            "time": time, "ageSec": age, "tlAgeSec": tl_age,
        })
    }).collect();

    let newest = items.iter().filter_map(|x| x["time"].as_str())
        .filter_map(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.timestamp_millis() as u64).max().unwrap_or(0);
    let with_data = items.iter().filter(|x| !x["usd"].is_null() || !x["tl"].is_null()).count();
    let fresh_usd: Vec<&Value> = items.iter().filter(|x| {
        !x["usd"].is_null() && x["ageSec"].as_u64().is_some_and(|a| a <= 120)
    }).collect();
    let fresh_tl: Vec<&Value> = items.iter().filter(|x| {
        !x["tl"].is_null() && x["tlAgeSec"].as_u64().is_some_and(|a| a <= 120)
    }).collect();

    let usd_src = if fresh_usd.iter().any(|x| x["usdSrc"] == "binance") { Some("binance") }
        else if !fresh_usd.is_empty() { Some("btcturk") } else { None };
    let tl_src = if !fresh_tl.is_empty() { Some("btcturk") }
        else if c.usdt_try.is_some() { Some("tahmini") } else { None };

    json!({
        "fetchedAt": chrono::Utc::now().to_rfc3339(),
        "count": items.len(), "withData": with_data,
        "stale": newest == 0 || now - newest > FRESH_MS,
        "connected": { "binance": c.binance_on, "btcturk": c.btcturk_on },
        "sources": { "usd": usd_src, "tl": tl_src },
        "errors": { "binance": c.binance_err, "btcturk": c.btcturk_err },
        "meta": {
            "usdtTry": round(c.usdt_try, 6), "usdtTryPct": round(c.usdt_try_pct, 4),
            "usdtTryAgeSec": if c.usdt_try_at > 0 { Some(now.saturating_sub(c.usdt_try_at) / 1000) } else { None },
        },
        "items": items,
    })
}

type Shared = Arc<Mutex<Crypto>>;

pub async fn broadcast_now(app: &AppState) {
    // TCMB verisi geldiğinde kripto kartını da yeniden çizmek için son anlık görüntüyü yayınlar.
    let snap = app.kripto.read().await.clone();
    if snap.get("items").is_some() { app.set_kripto(snap).await; }
}

pub async fn run(app: AppState) {
    let shared: Shared = Arc::new(Mutex::new(Crypto::default()));
    for (code, _) in COINS {
        shared.lock().await.coins.insert(code.to_string(), Coin::default());
    }

    let rest = shared.clone();
    tokio::spawn(async move { rest_loop(rest).await });
    let b = shared.clone();
    tokio::spawn(async move { binance_ws(b).await });
    let t = shared.clone();
    tokio::spawn(async move { btcturk_ws(t).await });
    let p = shared.clone();
    let a = app.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let mut c = p.lock().await;
            if !c.dirty { continue; }
            c.dirty = false;
            let snap = snapshot(&c);
            let with_data = snap["withData"].as_u64().unwrap_or(0);
            let connected = c.binance_on || c.btcturk_on;
            let err = c.binance_err.clone().or_else(|| c.btcturk_err.clone());
            drop(c);
            a.mark_source("kripto", with_data > 0, connected, err).await;
            a.set_kripto(snap).await;
        }
    });
}

async fn rest_loop(c: Shared) {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(15)).build().unwrap();
    loop {
        // Binance (≤20 sembol/istek)
        for chunk in COINS.chunks(20) {
            let symbols: Vec<String> = chunk.iter().map(|(code, _)| format!("{code}USDT")).collect();
            let url = format!("{}?symbols={}", BIN_REST, urlencode(&format!("[\"{}\"]", symbols.join("\",\""))));
            match client.get(&url).header("User-Agent", UA).send().await {
                Ok(r) if r.status().is_success() => {
                    if let Ok(arr) = r.json::<Vec<Value>>().await {
                        let mut g = c.lock().await;
                        for q in &arr { apply_binance(&mut g, q); }
                        g.binance_err = None;
                    }
                }
                Ok(r) => { c.lock().await.binance_err = Some(format!("HTTP {}", r.status())); }
                Err(e) => { c.lock().await.binance_err = Some(e.to_string()); }
            }
        }
        // BtcTurk
        match client.get(BT_REST).header("User-Agent", UA).send().await {
            Ok(r) if r.status().is_success() => {
                if let Ok(body) = r.json::<Value>().await {
                    if let Some(arr) = body["data"].as_array() {
                        let mut g = c.lock().await;
                        for t in arr { apply_bt_rest(&mut g, t); }
                        g.btcturk_err = None;
                    }
                }
            }
            Ok(r) => { c.lock().await.btcturk_err = Some(format!("HTTP {}", r.status())); }
            Err(e) => { c.lock().await.btcturk_err = Some(e.to_string()); }
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { out.push(b as char); }
        else { out.push_str(&format!("%{:02X}", b)); }
    }
    out
}

async fn binance_ws(c: Shared) {
    let streams = COINS.iter().map(|(code, _)| format!("{}usdt@miniTicker", code.to_lowercase()))
        .collect::<Vec<_>>().join("/");
    let url = format!("{BIN_WS}{streams}");
    loop {
        match tokio_tungstenite::connect_async(&url).await {
            Ok((mut ws, _)) => {
                c.lock().await.binance_on = true;
                while let Some(msg) = ws.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                                if let Some(d) = v.get("data") { apply_binance_ws(&mut *c.lock().await, d); }
                            }
                        }
                        Ok(Message::Ping(p)) => { let _ = ws.send(Message::Pong(p)).await; }
                        Ok(Message::Close(_)) | Err(_) => break,
                        _ => {}
                    }
                }
                c.lock().await.binance_on = false;
            }
            Err(e) => { c.lock().await.binance_err = Some(e.to_string()); }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn btcturk_ws(c: Shared) {
    let mut host = 0usize;
    loop {
        let url = BT_HOSTS[host % BT_HOSTS.len()];
        match tokio_tungstenite::connect_async(url).await {
            Ok((mut ws, _)) => {
                c.lock().await.btcturk_on = true;
                for (code, _) in COINS {
                    let msg = json!([151, {"type": 151, "channel": "ticker", "event": format!("{code}TRY"), "join": true}]);
                    let _ = ws.send(Message::Text(msg.to_string())).await;
                }
                let msg = json!([151, {"type": 151, "channel": "ticker", "event": "USDTTRY", "join": true}]);
                let _ = ws.send(Message::Text(msg.to_string())).await;

                while let Some(msg) = ws.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Ok(arr) = serde_json::from_str::<Vec<Value>>(&text) {
                                if arr.first().and_then(Value::as_u64) == Some(402) {
                                    if let Some(p) = arr.get(1) { apply_bt_ws(&mut *c.lock().await, p); }
                                }
                            }
                        }
                        Ok(Message::Ping(p)) => { let _ = ws.send(Message::Pong(p)).await; }
                        Ok(Message::Close(_)) | Err(_) => break,
                        _ => {}
                    }
                }
                c.lock().await.btcturk_on = false;
                host += 1;
            }
            Err(e) => {
                c.lock().await.btcturk_err = Some(e.to_string());
                host += 1;
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
