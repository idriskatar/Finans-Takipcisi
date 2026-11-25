use crate::state::AppState;
use crate::sources::now_ms;
use chrono::{Datelike, Duration as ChronoDuration, Timelike, Utc};
use regex::Regex;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde_json::{json, Map, Value};
use std::time::Duration;

const BASE: &str = "https://www.tcmb.gov.tr";
const UA: &str = "finans-takipcisi/1.0";
const HOURS: [u32; 7] = [16, 15, 14, 13, 12, 11, 10];

const HOURLY_NAMES: &[(&str, &str)] = &[
    ("USD", "ABD Doları"), ("EUR", "Euro"), ("GBP", "İngiliz Sterlini"),
    ("CHF", "İsviçre Frangı"), ("JPY", "Japon Yeni"),
    ("XAU", "24 Ayar Altın (gram)"), ("XAS", "Has Altın (gram)"),
];

fn decode_entities(s: &str) -> String {
    let mut out = s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
        .replace("&quot;", "\"").replace("&apos;", "'").replace("&nbsp;", " ");
    // sayısal varlıklar
    let re = Regex::new(r"&#(x[0-9a-fA-F]+|[0-9]+);").unwrap();
    out = re.replace_all(&out, |caps: &regex::Captures| {
        let raw = &caps[1];
        let cp = if let Some(hex) = raw.strip_prefix('x').or_else(|| raw.strip_prefix('X')) {
            u32::from_str_radix(hex, 16).ok()
        } else {
            raw.parse::<u32>().ok()
        };
        cp.and_then(char::from_u32).map(|c| c.to_string()).unwrap_or_else(|| caps[0].to_string())
    }).into_owned();
    out
}

fn num(v: Option<&str>) -> Option<f64> {
    let s = v?.trim().replace(['\u{00a0}', ' '], "");
    if s.is_empty() { return None; }
    let s = if s.contains(',') && s.contains('.') {
        if s.rfind(',').unwrap_or(0) > s.rfind('.').unwrap_or(0) {
            s.replace('.', "").replace(',', ".")
        } else {
            s.replace(',', "")
        }
    } else if s.contains(',') {
        s.replace(',', ".")
    } else {
        s
    };
    s.parse::<f64>().ok().filter(|n| n.is_finite())
}

fn attr(block: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"(?:^|\s){}="([^"]*)""#, regex::escape(name))).ok()?;
    re.captures(block).map(|c| c[1].to_string())
}

fn tag(block: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!("<{}>([^<]*)</{}>", regex::escape(name), regex::escape(name))).ok()?;
    re.captures(block).map(|c| decode_entities(c[1].trim()))
}

fn parse_currencies(xml: &str) -> (Option<String>, Option<String>, Map<String, Value>) {
    let root = Regex::new(r"<Tarih_Date\b([^>]*)>").ok()
        .and_then(|r| r.captures(xml).map(|c| c[1].to_string()));
    let re = Regex::new(r"<Currency\b([^>]*)>([\s\S]*?)</Currency>").unwrap();
    let mut items = Map::new();
    for caps in re.captures_iter(xml) {
        let code = attr(&caps[1], "Kod").or_else(|| attr(&caps[1], "CurrencyCode"));
        let Some(code) = code else { continue };
        let body = &caps[2];
        items.insert(code.clone(), json!({
            "code": code,
            "name": tag(body, "Isim").or_else(|| tag(body, "CurrencyName")).unwrap_or(code),
            "unit": num(tag(body, "Unit").as_deref()).unwrap_or(1.0),
            "buy": num(tag(body, "ForexBuying").as_deref()),
            "sell": num(tag(body, "ForexSelling").as_deref()),
            "cashBuy": num(tag(body, "BanknoteBuying").as_deref()),
            "cashSell": num(tag(body, "BanknoteSelling").as_deref()),
        }));
    }
    let date = root.as_deref().and_then(|r| attr(r, "Tarih"));
    let bulten = root.as_deref().and_then(|r| attr(r, "Bulten_No"));
    (date, bulten, items)
}

fn parse_hourly(xml: &str) -> (Option<String>, Option<String>, Map<String, Value>) {
    let list = Regex::new(r"<doviz_kur_liste\b([^>]*)>").ok()
        .and_then(|r| r.captures(xml).map(|c| c[1].to_string()));
    let re = Regex::new(r"<kur>([\s\S]*?)</kur>").unwrap();
    let mut items = Map::new();
    for caps in re.captures_iter(xml) {
        let body = &caps[1];
        let Some(code) = tag(body, "doviz_cinsi") else { continue };
        let name = HOURLY_NAMES.iter().find(|(c, _)| *c == code)
            .map(|(_, n)| *n).unwrap_or(code.as_str());
        items.insert(code.clone(), json!({
            "code": code,
            "name": name,
            "unit": num(tag(body, "birim").as_deref()).unwrap_or(1.0),
            "buy": num(tag(body, "alis").as_deref()),
            "sell": Value::Null,
            "cashBuy": Value::Null,
            "cashSell": Value::Null,
        }));
    }
    let date = list.as_deref().and_then(|r| attr(r, "gecerlilik_tarihi"));
    let saat = list.as_deref().and_then(|r| attr(r, "saat"));
    (date, saat, items)
}

async fn fetch_xml(client: &reqwest::Client, url: &str) -> Option<String> {
    let res = client.get(url).timeout(Duration::from_secs(20)).send().await.ok()?;
    if !res.status().is_success() { return None; }
    let text = res.text().await.ok()?;
    if !text.trim_start().starts_with('<') { return None; }
    Some(text)
}

async fn load_today(client: &reqwest::Client) -> Value {
    let url = format!("{BASE}/kurlar/today.xml");
    match fetch_xml(client, &url).await {
        Some(text) => {
            let (date, bulten, items) = parse_currencies(&text);
            if items.is_empty() {
                json!({"error": "today.xml çözümlenemedi", "url": url})
            } else {
                json!({"url": url, "date": date, "bulten": bulten, "items": items})
            }
        }
        None => json!({"error": "today.xml alınamadı", "url": url}),
    }
}

async fn load_day(client: &reqwest::Client, yyyy: u32, mm: u32, dd: u32, hours: &[u32]) -> Option<Value> {
    let mut found: Vec<(u32, String, String, Map<String, Value>)> = Vec::new();
    for chunk in hours.chunks(3) {
        let mut futs = Vec::new();
        for h in chunk {
            let url = format!("{BASE}/reeskontkur/{yyyy:04}{mm:02}/{dd:02}{mm:02}{yyyy:04}-{h:02}00.xml");
            let c = client.clone();
            futs.push(async move { (*h, url.clone(), fetch_xml(&c, &url).await) });
        }
        for (h, url, text) in futures_util::future::join_all(futs).await {
            if let Some(text) = text {
                let (date, _, items) = parse_hourly(&text);
                if !items.is_empty() {
                    let date = date.unwrap_or_else(|| format!("{dd:02}.{mm:02}.{yyyy:04}"));
                    found.push((h, url, date, items));
                }
            }
        }
    }
    if found.is_empty() { return None; }
    found.sort_by(|a, b| b.0.cmp(&a.0));
    let mut items = Map::new();
    for f in &found {
        for (k, v) in &f.3 {
            items.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
    let newest = &found[0];
    Some(json!({
        "url": newest.1,
        "date": newest.2.clone(),
        "time": format!("{dd:02}.{mm:02}.{yyyy:04} {:02}:00", newest.0),
        "hours": found.iter().map(|f| format!("{:02}:00", f.0)).collect::<Vec<_>>(),
        "items": items,
    }))
}

async fn load_hourly(client: &reqwest::Client) -> Value {
    let ist = Utc::now() + ChronoDuration::hours(3);
    let mut tried = 0;
    for back in 0..10 {
        let d = ist - ChronoDuration::days(back);
        let dow = d.weekday().num_days_from_sunday();
        if dow == 0 || dow == 6 { continue; }
        tried += 1;
        let hours: Vec<u32> = HOURS.iter().copied()
            .filter(|h| back > 0 || *h <= ist.hour()).collect();
        if hours.is_empty() { continue; }
        if let Some(day) = load_day(client, d.year() as u32, d.month() as u32, d.day() as u32, &hours).await {
            return day;
        }
        if tried >= 5 { break; }
    }
    json!({"error": "son iş günlerinde saat başı dosyası bulunamadı"})
}

pub async fn run(state: AppState) {
    let client = reqwest::Client::builder()
        .user_agent(UA)
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(ACCEPT, "application/xml,text/xml,*/*".parse().unwrap());
            h.insert(USER_AGENT, UA.parse().unwrap());
            h
        })
        .build()
        .unwrap();

    loop {
        let (today, hourly) = tokio::join!(load_today(&client), load_hourly(&client));
        // Veri yoksa 60 sn bekleme: 5 sn sonra tekrar dene, geldiğinde normal ritme dön.
        let today_err = today.get("error").and_then(Value::as_str).map(str::to_string);
        let hourly_err = hourly.get("error").and_then(Value::as_str).map(str::to_string);
        let failed = today_err.is_some() || hourly_err.is_some();
        let err = today_err.clone().or(hourly_err.clone());
        state.mark_source("tcmb", !failed, true, err).await;
        let snapshot = json!({
            "fetchedAt": chrono::Utc::now().to_rfc3339(),
            "today": today,
            "hourly": hourly,
        });
        state.set_tcmb(snapshot).await;
        // Kripto kartındaki TCMB dolar karşılaştırması yeniden çizilir
        crate::sources::kripto::broadcast_now(&state).await;
        tokio::time::sleep(Duration::from_secs(if failed { 5 } else { 60 })).await;
    }
}

#[allow(dead_code)]
fn touch() { let _ = now_ms(); }
