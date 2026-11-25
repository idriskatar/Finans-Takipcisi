// TCMB verileri:
//  - Gösterge kurları:    https://www.tcmb.gov.tr/kurlar/today.xml  (Tarih_Date / Currency)
//  - Saat başı kur/altın: https://www.tcmb.gov.tr/reeskontkur/YYYYMM/DDMMYYYY-HHMM.xml
//    (tcmbVeri / doviz_kur_liste / kur; alış tek fiyat, ondalık virgülle)
//
// 23.11.2026 itibarıyla doğrulandı: reeskontkur dosyaları her gün 11:00 (döviz),
// 12:00/14:00/16:00 (altın XAU/XAS) saatlerinde yayımlanıyor; hepsi tek dosyada olmayabilir.

const BASE = "https://www.tcmb.gov.tr";
const TIMEOUT_MS = 20_000; // TCMB yanıtları yavaş olabiliyor (ölçülen: 3-9 sn)
const TODAY_TTL = 10 * 60_000;
const HOURLY_TTL = 5 * 60_000;
const FAIL_TTL = 60_000; // hata sonrası daha sık dene
const HOURS = [16, 15, 14, 13, 12, 11, 10]; // yeniden eskiye
const MAX_BUSINESS_DAYS_BACK = 5;
const CONCURRENCY = 3; // TCMB'yi yormamak için aynı anda en fazla 3 istek

// reeskontkur dosyalarında ad alanı yok, kodlar biliniyor
const HOURLY_NAMES = {
  USD: "ABD Doları",
  EUR: "Euro",
  GBP: "İngiliz Sterlini",
  CHF: "İsviçre Frangı",
  JPY: "Japon Yeni",
  XAU: "24 Ayar Altın (gram)",
  XAS: "Has Altın (gram)",
};

const NUM_RE = /^[+-]?(\d+(\.\d*)?|\.\d+)$/;

// TCMB ondalık ayırıcı olarak virgül kullanır; nokta/binlik de tolere edilir.
function num(v) {
  if (v === null || v === undefined) return null;
  let s = String(v).trim().replace(/[\s\u00a0]/g, "");
  if (!s) return null;
  if (s.includes(",") && s.includes(".")) {
    s = s.lastIndexOf(",") > s.lastIndexOf(".")
      ? s.replace(/\./g, "").replace(",", ".")
      : s.replace(/,/g, "");
  } else if (s.includes(",")) {
    s = s.replace(",", ".");
  }
  if (!NUM_RE.test(s)) return null;
  const n = Number(s);
  return Number.isFinite(n) ? n : null;
}

const ENTITIES = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " " };

function decodeEntities(s) {
  return s.replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);/gi, (m, e) => {
    if (e[0] === "#") {
      const cp =
        e[1] === "x" || e[1] === "X"
          ? parseInt(e.slice(2), 16)
          : parseInt(e.slice(1), 10);
      return Number.isFinite(cp) && cp > 0 && cp <= 0x10ffff ? String.fromCodePoint(cp) : m;
    }
    return ENTITIES[e.toLowerCase()] ?? m;
  });
}

const esc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const attr = (s, name) => new RegExp(`(?:^|\\s)${esc(name)}="([^"]*)"`).exec(s)?.[1] ?? null;
const tag = (s, name) => {
  const m = new RegExp(`<${esc(name)}>([^<]*)</${esc(name)}>`).exec(s);
  return m ? decodeEntities(m[1]).trim() : null;
};

// today.xml: <Tarih_Date ...><Currency Kod="USD"><Isim>..</Isim><ForexBuying>..</ForexBuying>...
export function parseCurrencies(xml) {
  const root = /<Tarih_Date\b([^>]*)>/.exec(xml);
  const items = {};
  for (const m of xml.matchAll(/<Currency\b([^>]*)>([\s\S]*?)<\/Currency>/g)) {
    const code = attr(m[1], "Kod") ?? attr(m[1], "CurrencyCode");
    if (!code) continue;
    items[code] = {
      code,
      name: tag(m[2], "Isim") ?? tag(m[2], "CurrencyName") ?? code,
      unit: num(tag(m[2], "Unit")) ?? 1,
      buy: num(tag(m[2], "ForexBuying")),
      sell: num(tag(m[2], "ForexSelling")),
      cashBuy: num(tag(m[2], "BanknoteBuying")),
      cashSell: num(tag(m[2], "BanknoteSelling")),
    };
  }
  return {
    date: root ? attr(root[1], "Tarih") : null,
    bulten: root ? attr(root[1], "Bulten_No") : null,
    items,
  };
}

// reeskontkur: <doviz_kur_liste gecerlilik_tarihi=".." saat=".."><kur><doviz_cinsi>USD</doviz_cinsi>
//   <birim>1</birim><alis>50,6079</alis></kur>...
export function parseHourly(xml) {
  const list = /<doviz_kur_liste\b([^>]*)>/.exec(xml);
  const items = {};
  for (const m of xml.matchAll(/<kur>([\s\S]*?)<\/kur>/g)) {
    const body = m[1];
    const code = tag(body, "doviz_cinsi");
    if (!code) continue;
    items[code] = {
      code,
      name: HOURLY_NAMES[code] ?? code,
      unit: num(tag(body, "birim")) ?? 1,
      buy: num(tag(body, "alis")),
      sell: null, // bu servis yalnızca alış fiyatı yayımlar
      cashBuy: null,
      cashSell: null,
    };
  }
  return {
    date: list ? attr(list[1], "gecerlilik_tarihi") : null,
    saat: list ? attr(list[1], "saat") : null,
    items,
  };
}

async function fetchXml(url) {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), TIMEOUT_MS);
  try {
    const res = await fetch(url, {
      signal: ctl.signal,
      headers: { "User-Agent": "harem-gateway/0.2", Accept: "application/xml,text/xml,*/*" },
    });
    if (!res.ok) return { ok: false, status: res.status, url };
    const text = await res.text();
    if (!text.trimStart().startsWith("<")) return { ok: false, status: "XML değil", url };
    return { ok: true, text, url };
  } catch (e) {
    return { ok: false, status: e.name === "AbortError" ? "zaman aşımı" : e.message, url };
  } finally {
    clearTimeout(timer);
  }
}

async function mapLimit(list, limit, fn) {
  const out = new Array(list.length);
  let i = 0;
  const workers = Array.from({ length: Math.min(limit, list.length) }, async () => {
    while (i < list.length) {
      const idx = i++;
      out[idx] = await fn(list[idx]);
    }
  });
  await Promise.all(workers);
  return out;
}

async function loadToday() {
  const r = await fetchXml(`${BASE}/kurlar/today.xml`);
  if (!r.ok) return { error: `today.xml alınamadı (${r.status})`, url: r.url };
  const p = parseCurrencies(r.text);
  if (!Object.keys(p.items).length) return { error: "today.xml çözümlenemedi", url: r.url };
  return { url: r.url, ...p };
}

const pad = (n) => String(n).padStart(2, "0");

// Bir iş gününün yayımlanmış tüm saatlerini çeker; bulunanları tek listede birleştirir
// (aynı kod birden fazla saatte varsa en yeni saat kazanır).
async function loadDay(yyyy, mm, dd, hours) {
  const urls = hours.map(
    (h) => `${BASE}/reeskontkur/${yyyy}${mm}/${dd}${mm}${yyyy}-${pad(h)}00.xml`,
  );
  const results = await mapLimit(urls, CONCURRENCY, fetchXml);

  const found = [];
  results.forEach((r, i) => {
    if (!r.ok) return;
    const p = parseHourly(r.text);
    if (Object.keys(p.items).length) found.push({ h: hours[i], url: r.url, p });
  });
  if (!found.length) return null;

  found.sort((a, b) => b.h - a.h); // en yeni saat başta
  const items = {};
  for (const f of found) {
    for (const [code, v] of Object.entries(f.p.items)) {
      if (!(code in items)) items[code] = v;
    }
  }
  const newest = found[0];
  return {
    url: newest.url,
    date: newest.p.date ?? `${dd}.${mm}.${yyyy}`,
    time: `${dd}.${mm}.${yyyy} ${pad(newest.h)}:00`,
    hours: found.map((f) => `${pad(f.h)}:00`),
    items,
  };
}

// Saat başı dosyasını bulur: bugünden geriye, iş günlerini deneyerek
async function loadHourly() {
  const ist = new Date(Date.now() + 3 * 3600_000); // İstanbul = UTC+3 (sabit)
  let tried = 0;
  for (let back = 0; tried < MAX_BUSINESS_DAYS_BACK && back < 10; back++) {
    const d = new Date(ist.getTime() - back * 86_400_000);
    const dow = d.getUTCDay();
    if (dow === 0 || dow === 6) continue;
    tried++;

    const yyyy = d.getUTCFullYear(), mm = pad(d.getUTCMonth() + 1), dd = pad(d.getUTCDate());
    const hours = HOURS.filter((h) => back > 0 || h <= ist.getUTCHours());
    if (!hours.length) continue;

    const day = await loadDay(yyyy, mm, dd, hours);
    if (day) return day;
  }
  return { error: "son iş günlerinde saat başı dosyası bulunamadı" };
}

const state = {
  today: { at: 0, value: null, pending: null },
  hourly: { at: 0, value: null, pending: null },
};

// Aynı anda gelen çağrılar tek isteği paylaşır; hata sonrası FAIL_TTL kadar sonra yeniden denenir.
function cached(key, ttl, loader) {
  const s = state[key];
  if (s.pending) return s.pending;

  const limit = s.value?.error ? FAIL_TTL : ttl;
  if (s.value !== null && Date.now() - s.at < limit) return Promise.resolve(s.value);

  s.at = Date.now();
  s.pending = loader()
    .catch((e) => ({ error: e.message }))
    .then((v) => {
      s.value = v;
      return v;
    })
    .finally(() => {
      s.pending = null;
    });
  return s.pending;
}

export async function getTcmb() {
  const [today, hourly] = await Promise.all([
    cached("today", TODAY_TTL, loadToday),
    cached("hourly", HOURLY_TTL, loadHourly),
  ]);
  return { fetchedAt: new Date().toISOString(), today, hourly };
}
