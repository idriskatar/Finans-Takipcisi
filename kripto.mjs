// Kripto fiyatları: USD tarafı Binance (USDT çiftleri), TL tarafı BtcTurk (TRY çiftleri).
//
// - Binance REST:  https://api.binance.com/api/v3/ticker/24hr?symbols=[...]  (≤20 sembol/istek)
// - Binance WS:    wss://stream.binance.com:9443/stream?streams=btcusdt@miniTicker/...  (~1 msg/sn/sembol)
// - BtcTurk REST:  https://api.btcturk.com/api/v2/ticker  (389 çift tek istek)
// - BtcTurk WS:    wss://ws-feed-pro.btcturk.com/  ([151,{channel:"ticker",event:"BTC_TRY"}] aboneliği)
//
// WebSocket'ler canlı akışı taşır; REST 60 sn'de bir eksikleri tazeler. BtcTurk'te TRY çifti
// olmayan coinlerde (BNB, TON, BCH gibi) TL, USDT/TRY kuruyla tahmin edilir ve "≈" ile işaretlenir.
// Binance düşerse USD, BtcTurk'ün USDT çiftlerinden; BtcTurk düşerse TL, USDT/TRY ile tahmin edilir.

const UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36";

const COINS = [
  ["BTC", "Bitcoin"],
  ["ETH", "Ethereum"],
  ["BNB", "BNB"],
  ["SOL", "Solana"],
  ["XRP", "XRP"],
  ["ADA", "Cardano"],
  ["DOGE", "Dogecoin"],
  ["AVAX", "Avalanche"],
  ["LINK", "Chainlink"],
  ["TRX", "TRON"],
  ["DOT", "Polkadot"],
  ["LTC", "Litecoin"],
  ["SHIB", "Shiba Inu"],
  ["TON", "Toncoin"],
  ["SUI", "Sui"],
  ["NEAR", "NEAR Protocol"],
  ["APT", "Aptos"],
  ["ARB", "Arbitrum"],
  ["OP", "Optimism"],
  ["PEPE", "Pepe"],
  ["XLM", "Stellar"],
  ["BCH", "Bitcoin Cash"],
  ["HBAR", "Hedera"],
  ["UNI", "Uniswap"],
  ["ATOM", "Cosmos"],
  ["ETC", "Ethereum Classic"],
];

const COIN_SET = new Set(COINS.map(([code]) => code));
const BIN_SYMBOLS = COINS.map(([code]) => `${code}USDT`);

const BIN_REST = "https://api.binance.com/api/v3/ticker/24hr";
const BIN_WS = "wss://stream.binance.com:9443/stream?streams=";
const BT_REST = "https://api.btcturk.com/api/v2/ticker";
const BT_WS_HOSTS = ["wss://ws-feed-pro.btcturk.com/", "wss://ws.global.btcturk.com"];

const REST_TTL = 60_000; // REST tazeleme
const PUSH_MS = 1_000; // istemcilere en fazla saniyede bir anlık görüntü
const WATCH_MS = 15_000; // sessiz soketleri denetle
const SILENT_MS = 75_000; // bu süre mesaj gelmezse soketi yenile
const FRESH_MS = 5 * 60_000; // bu yaştan eski fiyat "bayat" sayılır
const TIMEOUT_MS = 15_000;
const BIN_CHUNK = 20; // ≤20 sembol: istek ağırlığı 2

const num = (v) => {
  if (v === null || v === undefined || v === "") return null;
  if (typeof v === "number") return Number.isFinite(v) ? v : null;
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
};

// ---------- durum ----------
/** @type {Map<string, object>} */
const state = new Map();
for (const [code, name] of COINS) {
  state.set(code, {
    code,
    name,
    bu: null, bp: null, bh: null, bl: null, bq: null, bat: 0, // Binance USDT
    uu: null, up: null, uat: 0, // BtcTurk USDT (yalnız yedek)
    tl: null, tp: null, th: null, tlow: null, tv: null, tat: 0, realTl: false, // BtcTurk TRY
  });
}

const meta = { usdtTry: null, usdtTryPct: null, usdtTryAt: 0 };
const listeners = new Set();
const logAt = new Map();

let started = false;
let dirty = false;
let restPending = false;
let restTimer = null;
let pushTimer = null;
let watchTimer = null;

const bin = {
  ws: null,
  connected: false,
  retry: 2_000,
  lastMsg: 0,
  lastAttempt: 0,
  error: null,
};
const bt = {
  ws: null,
  connected: false,
  retry: 2_000,
  lastMsg: 0,
  lastAttempt: 0,
  host: 0,
  error: null,
};

function logOnce(key, msg) {
  const at = logAt.get(key) ?? 0;
  if (Date.now() - at < 10 * 60_000) return;
  logAt.set(key, Date.now());
  console.warn(msg);
}

function ensure(code) {
  return state.get(code);
}

async function getJson(url) {
  const res = await fetch(url, {
    headers: { "User-Agent": UA, Accept: "application/json" },
    signal: AbortSignal.timeout(TIMEOUT_MS),
  });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const body = await res.json();
  if (body && typeof body === "object" && !Array.isArray(body) && body.code && body.msg) {
    throw new Error(`${body.code}: ${body.msg}`);
  }
  return body;
}

// ---------- REST tohumlama ----------
function applyBinance(q) {
  const code = String(q.symbol ?? "").replace(/USDT$/, "");
  const s = COIN_SET.has(code) ? ensure(code) : null;
  if (!s) return;
  const c = num(q.lastPrice);
  if (c === null) return;
  s.bu = c;
  s.bp = num(q.priceChangePercent);
  s.bh = num(q.highPrice);
  s.bl = num(q.lowPrice);
  s.bq = num(q.quoteVolume);
  s.bat = num(q.closeTime) ?? Date.now();
  dirty = true;
}

async function binanceRest() {
  for (let i = 0; i < BIN_SYMBOLS.length; i += BIN_CHUNK) {
    const chunk = BIN_SYMBOLS.slice(i, i + BIN_CHUNK);
    const url = `${BIN_REST}?symbols=${encodeURIComponent(JSON.stringify(chunk))}`;
    const arr = await getJson(url);
    if (!Array.isArray(arr)) throw new Error("Binance yanıtı geçersiz");
    for (const q of arr) applyBinance(q);
  }
  bin.error = null;
}

function setTry(s, { last, pct, high, low, vol, avg, at }) {
  const price = num(last);
  if (price === null) return;
  s.tl = price;
  s.tp = num(pct);
  if (num(high) !== null) s.th = num(high);
  if (num(low) !== null) s.tlow = num(low);
  s.tv = num(vol) !== null && num(avg) !== null ? num(vol) * num(avg) : s.tv;
  s.tat = num(at) ?? Date.now();
  s.realTl = true;
  dirty = true;
}

function applyBtItem(t) {
  const ns = String(t.numeratorSymbol ?? "").toUpperCase();
  const ds = String(t.denominatorSymbol ?? "").toUpperCase();
  const last = num(t.last);
  if (last === null) return;

  if (ds === "TRY" && ns === "USDT") {
    meta.usdtTry = last;
    meta.usdtTryPct = num(t.dailyPercent);
    meta.usdtTryAt = num(t.timestamp) ?? Date.now();
    dirty = true;
    return;
  }
  if (ds === "USDT" && COIN_SET.has(ns)) {
    const s = ensure(ns);
    s.uu = last;
    s.up = num(t.dailyPercent);
    s.uat = num(t.timestamp) ?? Date.now();
    dirty = true;
    return;
  }
  if (ds !== "TRY" || !COIN_SET.has(ns)) return;
  setTry(ensure(ns), {
    last,
    pct: t.dailyPercent,
    high: t.high,
    low: t.low,
    vol: t.volume,
    avg: t.average,
    at: t.timestamp,
  });
}

async function btcturkRest() {
  const body = await getJson(BT_REST);
  const arr = body?.data;
  if (!Array.isArray(arr)) throw new Error("BtcTurk yanıtı geçersiz");
  for (const t of arr) applyBtItem(t);
  bt.error = null;
}

async function refreshRest() {
  if (restPending) return;
  restPending = true;
  try {
    const [b, t] = await Promise.allSettled([binanceRest(), btcturkRest()]);
    if (b.status === "rejected") {
      bin.error = b.reason?.message ?? "hata";
      logOnce("bin-rest", `[kripto] Binance REST hatası: ${bin.error}`);
    }
    if (t.status === "rejected") {
      bt.error = t.reason?.message ?? "hata";
      logOnce("bt-rest", `[kripto] BtcTurk REST hatası: ${bt.error}`);
    }
    dirty = true;
    broadcast();
  } finally {
    restPending = false;
  }
}

// ---------- Binance WebSocket ----------
function applyBinanceTicker(d) {
  const code = String(d.s ?? "").replace(/USDT$/, "");
  const s = COIN_SET.has(code) ? ensure(code) : null;
  if (!s) return;
  const c = num(d.c);
  if (c === null) return;
  const o = num(d.o);
  s.bu = c;
  if (o !== null && o !== 0) s.bp = (c / o - 1) * 100;
  if (num(d.h) !== null) s.bh = num(d.h);
  if (num(d.l) !== null) s.bl = num(d.l);
  if (num(d.q) !== null) s.bq = num(d.q);
  s.bat = num(d.E) ?? Date.now();
  dirty = true;
}

function connectBinance() {
  if (!started || bin.ws) return;
  const streams = COINS.map(([code]) => `${code.toLowerCase()}usdt@miniTicker`).join("/");
  let ws;
  try {
    ws = new WebSocket(BIN_WS + streams);
  } catch (e) {
    bin.error = e.message;
    logOnce("bin-ws-create", `[kripto] Binance WS açılamadı: ${e.message}`);
    setTimeout(connectBinance, bin.retry);
    return;
  }
  bin.ws = ws;
  bin.lastAttempt = Date.now();

  ws.onopen = () => {
    bin.connected = true;
    bin.retry = 2_000;
    bin.lastMsg = Date.now();
    bin.error = null;
  };
  ws.onmessage = (ev) => {
    bin.lastMsg = Date.now();
    try {
      const msg = JSON.parse(ev.data);
      const d = msg?.data;
      if (d && d.e === "24hrMiniTicker") applyBinanceTicker(d);
    } catch {}
  };
  ws.onerror = () => {
    bin.error = "WebSocket hatası";
  };
  ws.onclose = () => {
    bin.connected = false;
    bin.ws = null;
    if (!started) return;
    const wait = bin.retry;
    bin.retry = Math.min(bin.retry * 2, 30_000);
    logOnce("bin-ws-close", `[kripto] Binance akışı koptu, ${wait / 1000} sn sonra yeniden`);
    setTimeout(connectBinance, wait);
  };
}

// ---------- BtcTurk WebSocket ----------
function applyBtTicker(p) {
  const ns = String(p.NS ?? "").toUpperCase();
  const ds = String(p.DS ?? "").toUpperCase();
  const last = num(p.LA) ?? num(p.A);
  if (last === null) return;

  if (ds === "TRY" && ns === "USDT") {
    meta.usdtTry = last;
    meta.usdtTryPct = num(p.DP);
    meta.usdtTryAt = Date.now();
    dirty = true;
    return;
  }
  if (ds !== "TRY" || !COIN_SET.has(ns)) return;

  const s = ensure(ns);
  setTry(s, {
    last,
    pct: p.DP,
    high: p.H,
    low: p.L,
    vol: p.V,
    avg: p.AV,
    at: Date.now(),
  });
}

function connectBtcTurk() {
  if (!started || bt.ws) return;
  const host = BT_WS_HOSTS[bt.host % BT_WS_HOSTS.length];
  let ws;
  try {
    ws = new WebSocket(host);
  } catch (e) {
    bt.error = e.message;
    logOnce("bt-ws-create", `[kripto] BtcTurk WS açılamadı: ${e.message}`);
    setTimeout(connectBtcTurk, bt.retry);
    return;
  }
  bt.ws = ws;
  bt.lastAttempt = Date.now();

  ws.onopen = () => {
    bt.connected = true;
    bt.retry = 2_000;
    bt.lastMsg = Date.now();
    bt.error = null;
    // TRY çiftleri + kripto dolar kuru
    const events = COINS.map(([code]) => `${code}TRY`);
    events.push("USDTTRY");
    for (const event of events) {
      try {
        ws.send(JSON.stringify([151, { type: 151, channel: "ticker", event, join: true }]));
      } catch {}
    }
  };
  ws.onmessage = (ev) => {
    bt.lastMsg = Date.now();
    try {
      const arr = JSON.parse(ev.data);
      if (Array.isArray(arr) && arr[0] === 402 && arr[1]) applyBtTicker(arr[1]);
    } catch {}
  };
  ws.onerror = () => {
    bt.error = "WebSocket hatası";
  };
  ws.onclose = () => {
    bt.connected = false;
    bt.ws = null;
    if (!started) return;
    // Hiç açılamadıysa yedek adrese geç
    bt.host = (bt.host + 1) % BT_WS_HOSTS.length;
    const wait = bt.retry;
    bt.retry = Math.min(bt.retry * 2, 30_000);
    logOnce("bt-ws-close", `[kripto] BtcTurk akışı koptu, ${wait / 1000} sn sonra yeniden`);
    setTimeout(connectBtcTurk, wait);
  };
}

function watchSockets() {
  const now = Date.now();
  if (bin.connected && bin.lastMsg && now - bin.lastMsg > SILENT_MS) {
    logOnce("bin-silent", "[kripto] Binance akışı sessiz, yenileniyor");
    try {
      bin.ws?.close();
    } catch {}
  }
  if (bt.connected && bt.lastMsg && now - bt.lastMsg > SILENT_MS) {
    logOnce("bt-silent", "[kripto] BtcTurk akışı sessiz, yenileniyor");
    try {
      bt.ws?.close();
    } catch {}
  }
  if (!bin.ws && now - bin.lastAttempt > 10_000) connectBinance();
  if (!bt.ws && now - bt.lastAttempt > 10_000) connectBtcTurk();
}

// ---------- anlık görüntü ----------
function buildSnapshot() {
  const now = Date.now();
  const round = (v, d) =>
    v === null || v === undefined || !Number.isFinite(v) ? null : Math.round(v * 10 ** d) / 10 ** d;
  const items = COINS.map(([code, name]) => {
    const s = ensure(code);

    const binFresh = s.bat > 0 && now - s.bat < FRESH_MS;
    const altFresh = s.uat > 0 && now - s.uat < FRESH_MS;
    const useAlt =
      s.uu !== null && !binFresh && (altFresh || s.uat > s.bat);

    let usd = useAlt ? s.uu : s.bu;
    let usdPct = useAlt ? s.up : s.bp;
    let usdHigh = useAlt ? null : s.bh;
    let usdLow = useAlt ? null : s.bl;
    let volUsd = useAlt ? null : s.bq;
    let timeMs = useAlt ? s.uat : s.bat;
    let usdSrc = useAlt ? "btcturk" : s.bu !== null ? "binance" : null;

    if (usd === null && s.bu !== null) {
      usd = s.bu;
      usdPct = s.bp;
      usdHigh = s.bh;
      usdLow = s.bl;
      volUsd = s.bq;
      timeMs = s.bat;
      usdSrc = "binance";
    }

    // TL: BtcTurk kotasyonu taze ise onu kullan; değilse USDT/TRY ile tahmin et.
    const tryFresh = s.realTl && s.tat > 0 && now - s.tat < FRESH_MS;
    let tl = s.tl;
    let tlPct = s.tp;
    let tlHigh = s.th;
    let tlLow = s.tlow;
    let volTl = s.tv;
    let estimated = false;

    if (!tryFresh && usd !== null && meta.usdtTry !== null) {
      tl = usd * meta.usdtTry;
      tlHigh = usdHigh !== null ? usdHigh * meta.usdtTry : null;
      tlLow = usdLow !== null ? usdLow * meta.usdtTry : null;
      tlPct =
        usdPct !== null
          ? ((1 + usdPct / 100) * (1 + (meta.usdtTryPct ?? 0) / 100) - 1) * 100
          : null;
      volTl = volUsd !== null ? volUsd * meta.usdtTry : null;
      estimated = true;
      if (timeMs === 0) timeMs = meta.usdtTryAt;
    }

    // Hacimde USD (Binance) ana kaynaktır; TL karşılığı yaklaşıktır.
    const volTlApprox = volUsd !== null && meta.usdtTry !== null;
    if (volTlApprox) volTl = volUsd * meta.usdtTry;

    return {
      code,
      name,
      usd: round(usd, 8),
      usdPct: round(usdPct, 4),
      usdHigh: round(usdHigh, 8),
      usdLow: round(usdLow, 8),
      volUsd: round(volUsd, 2),
      tl: round(tl, 8),
      tlPct: round(tlPct, 4),
      tlHigh: round(tlHigh, 8),
      tlLow: round(tlLow, 8),
      volTl: round(volTl, 2),
      volTlApprox,
      estimated,
      usdSrc,
      tlSrc: estimated ? "tahmini" : s.realTl ? "btcturk" : null,
      time: timeMs ? new Date(timeMs).toISOString() : null,
      ageSec: timeMs ? Math.max(0, Math.round((now - timeMs) / 1000)) : null,
      tlAgeSec:
        s.realTl && s.tat > 0 ? Math.max(0, Math.round((now - s.tat) / 1000)) : null,
    };
  });

  const newest = items.reduce((m, x) => Math.max(m, x.time ? Date.parse(x.time) : 0), 0);
  const withData = items.filter((x) => x.usd !== null || x.tl !== null).length;
  const freshUsd = items.filter((x) => x.usd !== null && x.ageSec !== null && x.ageSec <= 120);
  const freshTl = items.filter((x) => x.tl !== null && x.tlAgeSec !== null && x.tlAgeSec <= 120);

  return {
    fetchedAt: new Date().toISOString(),
    count: items.length,
    withData,
    stale: !newest || now - newest > FRESH_MS,
    connected: { binance: bin.connected, btcturk: bt.connected },
    sources: {
      usd: freshUsd.some((x) => x.usdSrc === "binance")
        ? "binance"
        : freshUsd.length
          ? "btcturk"
          : null,
      tl: freshTl.length ? "btcturk" : meta.usdtTry !== null ? "tahmini" : null,
    },
    errors: { binance: bin.error, btcturk: bt.error },
    meta: {
      usdtTry: round(meta.usdtTry, 6),
      usdtTryPct: round(meta.usdtTryPct, 4),
      usdtTryAgeSec: meta.usdtTryAt
        ? Math.max(0, Math.round((now - meta.usdtTryAt) / 1000))
        : null,
    },
    items,
  };
}

function broadcast() {
  if (!listeners.size) return;
  const snapshot = buildSnapshot();
  for (const fn of listeners) {
    try {
      fn(snapshot);
    } catch {}
  }
}

export function getKripto() {
  return buildSnapshot();
}

export function onKripto(fn) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export function startKripto() {
  if (started) return;
  started = true;
  refreshRest();
  connectBinance();
  connectBtcTurk();

  restTimer = setInterval(refreshRest, REST_TTL);
  pushTimer = setInterval(() => {
    if (!dirty) return;
    dirty = false;
    broadcast();
  }, PUSH_MS);
  watchTimer = setInterval(watchSockets, WATCH_MS);
  for (const t of [restTimer, pushTimer, watchTimer]) t.unref?.();
}

export function stopKripto() {
  started = false;
  clearInterval(restTimer);
  clearInterval(pushTimer);
  clearInterval(watchTimer);
  try {
    bin.ws?.close();
  } catch {}
  try {
    bt.ws?.close();
  } catch {}
  bin.ws = null;
  bt.ws = null;
  bin.connected = false;
  bt.connected = false;
  listeners.clear();
}
