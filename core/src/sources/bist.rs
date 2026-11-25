use crate::state::AppState;
use crate::sources::fallback::{FALLBACK, FALLBACK_DATE};
use reqwest::header::{ACCEPT, COOKIE, SET_COOKIE, USER_AGENT};
use serde_json::{json, Map, Value};
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36";
const CSV_URL: &str = "https://www.borsaistanbul.com/datum/hisse_endeks_ds.csv";
const QUOTE_URL: &str = "https://query2.finance.yahoo.com/v7/finance/quote";
const CHART_URL: &str = "https://query1.finance.yahoo.com/v8/finance/chart";
const CRUMB_URL: &str = "https://query2.finance.yahoo.com/v1/test/getcrumb";
const INDEX_SYMBOL: &str = "XU100.IS";

fn f(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64).filter(|n| n.is_finite())
}

struct Session { cookie: String, crumb: String }

async fn ensure_session(client: &reqwest::Client) -> Option<Session> {
    let c = client.get("https://fc.yahoo.com").send().await.ok()?;
    let cookie = c.headers().get_all(SET_COOKIE).iter()
        .filter_map(|v| v.to_str().ok())
        .map(|s| s.split(';').next().unwrap_or("").to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>().join("; ");
    let cr = client.get(CRUMB_URL).header(COOKIE, &cookie).send().await.ok()?;
    let crumb = cr.text().await.ok()?.trim().to_string();
    if cookie.is_empty() || crumb.is_empty() || crumb.len() > 64 { return None; }
    Some(Session { cookie, crumb })
}

async fn quote_chunk(client: &reqwest::Client, s: &Session, symbols: &[String]) -> Option<Vec<Value>> {
    let url = format!("{}?symbols={}&crumb={}", QUOTE_URL,
        urlencode(&symbols.join(",")), urlencode(&s.crumb));
    let res = client.get(url).header(COOKIE, &s.cookie).send().await.ok()?;
    if !res.status().is_success() { return None; }
    let body: Value = res.json().await.ok()?;
    body["quoteResponse"]["result"].as_array().cloned()
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

async fn chart_quote(client: &reqwest::Client, symbol: &str) -> Option<Value> {
    let url = format!("{}/{}?interval=1d&range=1d", CHART_URL, urlencode(symbol));
    let res = client.get(url).send().await.ok()?;
    if !res.status().is_success() { return None; }
    let body: Value = res.json().await.ok()?;
    let m = body["chart"]["result"][0]["meta"].as_object()?;
    let price = f(m.get("regularMarketPrice"));
    let prev = f(m.get("chartPreviousClose"));
    Some(json!({
        "symbol": symbol,
        "currency": m.get("currency").cloned().unwrap_or(json!("TRY")),
        "regularMarketPrice": price,
        "regularMarketDayHigh": f(m.get("regularMarketDayHigh")),
        "regularMarketDayLow": f(m.get("regularMarketDayLow")),
        "regularMarketVolume": f(m.get("regularMarketVolume")),
        "regularMarketTime": f(m.get("regularMarketTime")),
        "regularMarketPreviousClose": prev,
        "regularMarketChangePercent": match (price, prev) {
            (Some(p), Some(c)) if c != 0.0 => Some((p - c) / c * 100.0),
            _ => None,
        },
        "shortName": m.get("shortName").cloned(),
        "longName": m.get("longName").cloned(),
    }))
}

fn normalize_quote(q: &Value) -> Value {
    let price = f(q.get("regularMarketPrice"));
    let volume = f(q.get("regularMarketVolume"));
    let symbol = q["symbol"].as_str().unwrap_or("").to_string();
    let code = symbol.trim_end_matches(".IS").to_string();
    let short = q["shortName"].as_str().filter(|s| !s.is_empty());
    let long = q["longName"].as_str().filter(|s| !s.is_empty());
    let name = short.or(long).unwrap_or(code.as_str()).to_string();
    let time = f(q.get("regularMarketTime")).map(|s| {
        chrono::DateTime::from_timestamp(s as i64, 0)
            .map(|d| d.to_rfc3339()).unwrap_or_default()
    });
    json!({
        "code": code,
        "symbol": symbol,
        "name": name,
        "longName": long.unwrap_or(name.as_str()),
        "price": price,
        "high": f(q.get("regularMarketDayHigh")),
        "low": f(q.get("regularMarketDayLow")),
        "volume": volume,
        "value": match (price, volume) { (Some(p), Some(v)) => Some(p * v), _ => None },
        "prevClose": f(q.get("regularMarketPreviousClose")),
        "change": f(q.get("regularMarketChange")),
        "changePct": f(q.get("regularMarketChangePercent")),
        "currency": q["currency"].as_str().unwrap_or("TRY"),
        "state": q["marketState"].as_str(),
        "delayedBy": f(q.get("exchangeDataDelayedBy")),
        "time": time,
    })
}

async fn load_list(client: &reqwest::Client) -> (Vec<(String, String, String)>, String, bool) {
    if let Ok(res) = client.get(CSV_URL).send().await {
        if res.status().is_success() {
            if let Ok(text) = res.text().await {
                let mut rows: Vec<(String, String, String)> = Vec::new();
                for line in text.lines().skip(1) {
                    let c: Vec<&str> = line.split(';').collect();
                    if c.len() < 6 || c[2].trim() != "XU100" { continue; }
                    let code = c[0].trim().trim_end_matches(".E").to_string();
                    if code.is_empty() { continue; }
                    rows.push((code, c[1].trim().to_string(), c[5].trim().to_string()));
                }
                if rows.len() >= 10 {
                    let stamp = |s: &str| -> i64 {
                        let p: Vec<&str> = s.split('/').collect();
                        if p.len() != 3 { return 0; }
                        let (d, m, y) = (p[0].parse::<i64>().unwrap_or(0),
                            p[1].parse::<i64>().unwrap_or(0), p[2].parse::<i64>().unwrap_or(0));
                        if y == 0 || m == 0 || d == 0 { 0 } else { y * 10000 + m * 100 + d }
                    };
                    let latest = rows.iter().map(|r| r.2.clone())
                        .max_by_key(|s| stamp(s)).unwrap_or_default();
                    let items: Vec<_> = rows.into_iter().filter(|r| r.2 == latest).collect();
                    let mut seen = std::collections::HashSet::new();
                    let unique: Vec<_> = items.into_iter()
                        .filter(|r| seen.insert(r.0.clone())).collect();
                    return (unique, latest, false);
                }
            }
        }
    }
    let items: Vec<_> = FALLBACK.iter()
        .map(|(c, n)| (c.to_string(), n.to_string(), FALLBACK_DATE.to_string())).collect();
    (items, FALLBACK_DATE.to_string(), true)
}

async fn load_quotes(client: &reqwest::Client, symbols: &[String]) -> (Vec<Value>, Option<Value>, bool) {
    let mut all = Vec::new();
    let mut index = None;
    if let Some(s) = ensure_session(client).await {
        for chunk in symbols.chunks(100) {
            if let Some(rows) = quote_chunk(client, &s, chunk).await { all.extend(rows); }
        }
        if let Some(rows) = quote_chunk(client, &s, &[INDEX_SYMBOL.to_string()]).await {
            index = rows.into_iter().next();
        }
    }
    if !all.is_empty() { return (all, index, false); }

    // chart yedeği (kimlik doğrulama gerektirmez)
    for chunk in symbols.chunks(6) {
        let mut futs = Vec::new();
        for sym in chunk {
            let c = client.clone();
            let s = sym.clone();
            futs.push(async move { chart_quote(&c, &s).await });
        }
        for q in futures_util::future::join_all(futs).await.into_iter().flatten() { all.push(q); }
    }
    if index.is_none() { index = chart_quote(client, INDEX_SYMBOL).await; }
    (all, index, true)
}

pub async fn run(state: AppState) {
    let client = reqwest::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(15))
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(USER_AGENT, UA.parse().unwrap());
            h.insert(ACCEPT, "application/json,text/plain,*/*".parse().unwrap());
            h
        })
        .build()
        .unwrap();

    loop {
        let (list, date, fallback) = load_list(&client).await;
        let symbols: Vec<String> = list.iter().map(|r| format!("{}.IS", r.0)).collect();
        let (quotes, index, chart) = load_quotes(&client, &symbols).await;
        let by_symbol: Map<String, Value> = quotes.iter()
            .filter_map(|q| q["symbol"].as_str().map(|s| (s.to_string(), q.clone())))
            .collect();

        let mut items: Vec<Value> = list.iter().map(|(code, name, _)| {
            let symbol = format!("{}.IS", code);
            match by_symbol.get(&symbol) {
                Some(raw) => {
                    let mut item = normalize_quote(raw);
                    if item["name"] == *code || item["name"].as_str().unwrap_or("").is_empty() {
                        item["name"] = json!(name);
                    }
                    item
                }
                None => json!({
                    "code": code, "symbol": symbol, "name": name, "longName": name,
                    "price": null, "high": null, "low": null, "volume": null, "value": null,
                    "prevClose": null, "change": null, "changePct": null,
                    "currency": "TRY", "state": null, "delayedBy": null, "time": null,
                }),
            }
        }).collect();

        items.sort_by(|a, b| {
            let va = f(a.get("value")).unwrap_or(-1.0);
            let vb = f(b.get("value")).unwrap_or(-1.0);
            vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a["code"].as_str().unwrap_or("").cmp(b["code"].as_str().unwrap_or("")))
        });

        let with_data = items.iter().filter(|x| f(x.get("price")).is_some()).count();
        let market_open = items.iter().any(|x| x["state"] == "REGULAR");
        let delayed = items.iter().filter_map(|x| f(x.get("delayedBy"))).fold(0.0f64, f64::max);
        let index_quote = index.as_ref().map(|raw| {
            let q = normalize_quote(raw);
            json!({
                "symbol": q["symbol"], "price": q["price"], "changePct": q["changePct"],
                "high": q["high"], "low": q["low"], "time": q["time"], "delayedBy": q["delayedBy"],
            })
        });

        let snapshot = json!({
            "fetchedAt": chrono::Utc::now().to_rfc3339(),
            "date": date,
            "source": if fallback { "yedek liste" } else { CSV_URL },
            "fallback": fallback,
            "chart": chart,
            "withData": with_data,
            "count": items.len(),
            "stale": with_data == 0,
            "marketOpen": market_open,
            "delayedBy": delayed,
            "indexQuote": index_quote,
            "items": items,
        });
        // Fiyat gelmediyse 30 sn bekleme: 5 sn sonra yeniden dene.
        let retry = if with_data == 0 { 5 } else { 30 };
        let err = snapshot.get("error").and_then(Value::as_str).map(str::to_string);
        state.mark_source("bist", with_data > 0, true, err).await;
        state.set_bist(snapshot).await;
        tokio::time::sleep(Duration::from_secs(retry)).await;
    }
}
