// BIST 100 (XU100) bileşenlerini Borsa İstanbul'dan, fiyatları Yahoo Finance'ten çeker.
//
// - Bileşen listesi: https://www.borsaistanbul.com/datum/hisse_endeks_ds.csv (12 saat önbellek)
// - Fiyatlar:        Yahoo v7/finance/quote (100 sembol tek istek, çerez + crumb)
// - Yedek:           bist100-list.mjs (liste indirilemezse), v8/finance/chart (toplu uç düşerse)

import { FALLBACK, FALLBACK_DATE } from "./bist100-list.mjs";

const UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36";
const CSV_URL = "https://www.borsaistanbul.com/datum/hisse_endeks_ds.csv";
const QUOTE_URL = "https://query2.finance.yahoo.com/v7/finance/quote";
const CHART_URL = "https://query1.finance.yahoo.com/v8/finance/chart";
const CRUMB_URL = "https://query2.finance.yahoo.com/v1/test/getcrumb";
const INDEX_SYMBOL = "XU100.IS"; // BIST 100 endeksi (hisselerin dışında ayrı sembol)

const TIMEOUT_MS = 15_000;
const LIST_TTL = 12 * 3600_000;
const QUOTE_TTL = 20_000;
const CHART_TTL = 60_000; // sembol başına chart yedeği pahalı: daha uzun önbellek
const FAIL_TTL = 30_000;
const CHUNK = 100; // Yahoo tek istekte ~100 sembol kabul ediyor
const CHART_CONCURRENCY = 6; // yalnızca toplu uç düşerse kullanılan yedek yol

const num = (v) => (typeof v === "number" && Number.isFinite(v) ? v : null);

async function fetchT(url, options = {}) {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), TIMEOUT_MS);
  try {
    return await fetch(url, { ...options, signal: ctl.signal });
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

// ---------- Yahoo oturumu (çerez + crumb) ----------
const session = { cookie: "", crumb: "", at: 0 };

async function ensureSession(force = false) {
  if (!force && session.crumb && Date.now() - session.at < 30 * 60_000) return session;

  const c = await fetchT("https://fc.yahoo.com", { headers: { "User-Agent": UA } });
  const setCookies = c.headers.getSetCookie?.() ?? [c.headers.get("set-cookie")].filter(Boolean);
  const cookie = setCookies.map((x) => x.split(";")[0]).join("; ");

  const cr = await fetchT(CRUMB_URL, { headers: { "User-Agent": UA, Cookie: cookie } });
  const crumb = (await cr.text()).trim();
  if (!cookie || !crumb || crumb.length > 64) throw new Error("Yahoo oturumu kurulamadı");

  Object.assign(session, { cookie, crumb, at: Date.now() });
  return session;
}

async function quoteChunk(symbols, retry = true) {
  const s = await ensureSession();
  const url = `${QUOTE_URL}?symbols=${encodeURIComponent(symbols.join(","))}&crumb=${encodeURIComponent(s.crumb)}`;
  const res = await fetchT(url, {
    headers: { "User-Agent": UA, Cookie: s.cookie, Accept: "application/json" },
  });

  if (retry && (res.status === 401 || res.status === 403 || res.status === 429)) {
    await ensureSession(true);
    return quoteChunk(symbols, false);
  }
  if (!res.ok) throw new Error(`Yahoo quote ${res.status}`);
  const j = await res.json();
  return j.quoteResponse?.result ?? [];
}

// Toplu uç çalışmazsa sembol sembol chart ucundan oku (kimlik doğrulama istemez).
async function chartQuote(symbol) {
  const res = await fetchT(`${CHART_URL}/${encodeURIComponent(symbol)}?interval=1d&range=1d`, {
    headers: { "User-Agent": UA, Accept: "application/json" },
  });
  if (!res.ok) return null;
  const j = await res.json();
  const m = j.chart?.result?.[0]?.meta;
  if (!m) return null;
  return {
    symbol,
    currency: m.currency,
    regularMarketPrice: m.regularMarketPrice,
    regularMarketDayHigh: m.regularMarketDayHigh,
    regularMarketDayLow: m.regularMarketDayLow,
    regularMarketVolume: m.regularMarketVolume,
    regularMarketTime: m.regularMarketTime,
    regularMarketPreviousClose: m.chartPreviousClose,
    regularMarketChangePercent:
      m.chartPreviousClose && m.regularMarketPrice
        ? ((m.regularMarketPrice - m.chartPreviousClose) / m.chartPreviousClose) * 100
        : null,
    shortName: m.shortName,
    longName: m.longName,
  };
}

// ---------- Bileşen listesi ----------
const lstate = { at: 0, value: null, pending: null };

export function parseConstituents(csv) {
  const lines = csv.split(/\r?\n/).filter(Boolean);
  const rows = [];
  for (const line of lines.slice(1)) {
    const c = line.split(";");
    if (c[2] !== "XU100") continue;
    const code = (c[0] ?? "").trim().replace(/\.E$/, "");
    if (!code) continue;
    rows.push({ code, name: (c[1] ?? code).trim(), date: (c[5] ?? "").trim() });
  }
  if (!rows.length) return null;

  // TARIH(GG/AA/YYYY) — en yeni tarihli bileşen kümesini al
  const stamp = (s) => {
    const [d, m, y] = s.split("/").map(Number);
    return y && m && d ? Date.UTC(y, m - 1, d) : 0;
  };
  let latest = rows[0].date, latestAt = stamp(latest);
  for (const r of rows) {
    const at = stamp(r.date);
    if (at > latestAt) {
      latest = r.date;
      latestAt = at;
    }
  }

  const items = rows
    .filter((r) => r.date === latest)
    .map(({ code, name }) => ({ code, name, symbol: `${code}.IS` }));

  const seen = new Set();
  const unique = items.filter((x) => !seen.has(x.code) && seen.add(x.code));
  return { date: latest, items: unique, source: CSV_URL };
}

async function loadList() {
  try {
    const res = await fetchT(CSV_URL, { headers: { "User-Agent": UA } });
    if (!res.ok) throw new Error(`liste ${res.status}`);
    const parsed = parseConstituents(await res.text());
    if (!parsed || parsed.items.length < 10) throw new Error("liste çözümlenemedi");
    return { ...parsed, fallback: false };
  } catch (e) {
    console.warn("[bist] bileşen listesi alınamadı, yedek liste kullanılıyor:", e.message);
    return {
      date: FALLBACK_DATE,
      source: "bist100-list.mjs (yedek)",
      fallback: true,
      items: FALLBACK.map(([code, name]) => ({ code, name, symbol: `${code}.IS` })),
    };
  }
}

async function getList() {
  if (lstate.pending) return lstate.pending;

  const limit = lstate.value?.fallback ? FAIL_TTL : LIST_TTL;
  if (lstate.value && Date.now() - lstate.at < limit) return lstate.value;

  lstate.at = Date.now();
  lstate.pending = loadList()
    .then((v) => {
      lstate.value = v;
      return v;
    })
    .finally(() => {
      lstate.pending = null;
    });
  return lstate.pending;
}

// ---------- Fiyatlar ----------
const qstate = { at: 0, result: null, lastGood: null, pending: null, error: false, chart: false };

async function fetchQuotes(symbols) {
  const chunks = [];
  for (let i = 0; i < symbols.length; i += CHUNK) chunks.push(symbols.slice(i, i + CHUNK));

  const all = [];
  // Toplu uç HTTP hatası (401/429/ağ) verirse chart yedeğine düşebilmek için yut.
  try {
    for (const chunk of chunks) all.push(...(await quoteChunk(chunk)));
  } catch (e) {
    console.warn("[bist] toplu quote başarısız, chart yedeğine geçilecek:", e.message);
  }

  // Endeks ayrı sembol; toplu uç başarısız olsa bile üst barda gösterilebilsin.
  let index = null;
  try {
    index = (await quoteChunk([INDEX_SYMBOL]))[0] ?? null;
  } catch (e) {
    console.warn("[bist] endeks alınamadı:", e.message);
  }

  if (all.length) return { items: all, index, chart: false };

  console.warn("[bist] toplu uç kullanılamadı, sembol başına chart yedeğine geçiliyor");
  const single = await mapLimit(symbols, CHART_CONCURRENCY, chartQuote);
  if (!index) index = await chartQuote(INDEX_SYMBOL);
  return { items: single.filter(Boolean), index, chart: true };
}

async function getQuotes(symbols) {
  if (qstate.pending) return qstate.pending;

  const limit = qstate.error ? FAIL_TTL : qstate.chart ? CHART_TTL : QUOTE_TTL;
  if (qstate.result && Date.now() - qstate.at < limit) return qstate.result;

  qstate.pending = (async () => {
    try {
      const data = await fetchQuotes(symbols);
      if (!data.items.length) throw new Error("Yahoo boş yanıt döndü");
      qstate.error = false;
      qstate.chart = data.chart;
      qstate.lastGood = data;
      qstate.result = { items: data.items, index: data.index, stale: false, error: null };
    } catch (e) {
      qstate.error = true;
      // son iyi veriyi bayat olarak sun; hiç yoksa boş
      qstate.result = qstate.lastGood
        ? { ...qstate.lastGood, stale: true, error: e.message }
        : { items: [], index: null, stale: true, error: e.message };
    } finally {
      qstate.at = Date.now();
      qstate.pending = null;
    }
    return qstate.result;
  })();
  return qstate.pending;
}

function normalizeQuote(q) {
  const price = num(q.regularMarketPrice);
  const volume = num(q.regularMarketVolume);
  const symbol = String(q.symbol ?? "");
  const code = symbol.replace(/\.IS$/, "");
  const name = q.shortName || q.longName || code;

  return {
    code,
    symbol,
    name,
    longName: q.longName || name,
    price,
    high: num(q.regularMarketDayHigh),
    low: num(q.regularMarketDayLow),
    volume, // işlem adedi
    value: price !== null && volume !== null ? price * volume : null, // ≈ işlem hacmi (TL)
    prevClose: num(q.regularMarketPreviousClose),
    change: num(q.regularMarketChange),
    changePct: num(q.regularMarketChangePercent),
    currency: q.currency ?? "TRY",
    state: q.marketState ?? null,
    delayedBy: Number.isFinite(q.exchangeDataDelayedBy) ? q.exchangeDataDelayedBy : null,
    time: q.regularMarketTime ? new Date(q.regularMarketTime * 1000).toISOString() : null,
  };
}

export async function getBist100() {
  const list = await getList();
  const quotes = await getQuotes(list.items.map((x) => x.symbol));
  const bySymbol = new Map(quotes.items.map((q) => [q.symbol, q]));

  const items = list.items.map((c) => {
    const raw = bySymbol.get(c.symbol);
    if (!raw) {
      return {
        code: c.code,
        symbol: c.symbol,
        name: c.name,
        longName: c.name,
        price: null,
        high: null,
        low: null,
        volume: null,
        value: null,
        prevClose: null,
        change: null,
        changePct: null,
        currency: "TRY",
        state: null,
        delayedBy: null,
        time: null,
      };
    }
    const item = normalizeQuote(raw);
    if (!item.name || item.name === item.code) item.name = c.name;
    return item;
  });

  items.sort(
    (a, b) => (b.value ?? -1) - (a.value ?? -1) || a.code.localeCompare(b.code, "tr"),
  );

  const withData = items.filter((x) => x.price !== null).length;
  const open = items.some((x) => x.state === "REGULAR");
  const delayedBy = items.reduce((m, x) => Math.max(m, x.delayedBy ?? 0), 0);

  const idx = quotes.index ? normalizeQuote(quotes.index) : null;
  const indexQuote = idx
    ? {
        symbol: idx.symbol,
        price: idx.price,
        changePct: idx.changePct,
        high: idx.high,
        low: idx.low,
        time: idx.time,
        delayedBy: idx.delayedBy,
      }
    : null;

  return {
    fetchedAt: new Date().toISOString(),
    index: "XU100",
    indexName: "BIST 100",
    indexQuote,
    date: list.date,
    source: list.source,
    fallback: list.fallback,
    stale: quotes.stale,
    error: quotes.error,
    count: items.length,
    withData,
    marketOpen: open,
    delayedBy: delayedBy || null,
    items,
  };
}
