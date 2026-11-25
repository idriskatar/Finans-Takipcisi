// Harem canlı fiyat akışını tek bağlantıyla dinler, bellekte tutar,
// mobil uygulamaya REST ve SSE ile dağıtır. TCMB kurlarını /tcmb ile,
// BIST 100 hisselerini /bist, kripto fiyatlarını /kripto ile sunar.
//
// Çalıştırma:  npm install && npm start        (ayrıntılı log: npm run debug)
// Uç noktalar: GET /health | /prices | /prices?group=sarrafiye
//              GET /prices/USDTRY | /stream (SSE) | /tcmb | /bist
//              GET /kripto | /kripto/stream (SSE)

import http from "node:http";
import { readFile } from "node:fs/promises";
import { io } from "socket.io-client";
import { normalizePayload } from "./normalize.mjs";
import { getTcmb } from "./tcmb.mjs";
import { getBist100 } from "./bist100.mjs";
import { getKripto, onKripto, startKripto, stopKripto } from "./kripto.mjs";

const PORT = Number(process.env.PORT ?? 4500);
const HOST = process.env.HOST ?? "0.0.0.0"; // mobil cihazlar erişebilsin diye tüm arayüzler
const CORS_ORIGIN = process.env.CORS_ORIGIN ?? "*";
const SOCKET_URL = "https://hrmsocketonly.haremaltin.com";
const ORIGIN = "https://canlipiyasalar.haremaltin.com";
const STALE_AFTER_MS = 60_000; // bu süre veri gelmezse /health "stale" der
const SSE_PING_MS = 20_000; // proxy'lerin sessiz bağlantıyı kesmesini önler
const SSE_MAX_BUFFER = 1_000_000; // yavaş istemcide bu kadar biriken veri varsa düşür
const TCMB_WARM_MS = 5 * 60_000;
const DEBUG = process.env.DEBUG === "1" || process.argv.includes("--debug");

const GROUP_ORDER = ["altin", "sarrafiye", "doviz", "parite", "maden", "diger"];

/** @type {Map<string, object>} */
const prices = new Map();
const sseClients = new Set();
const kriptoClients = new Set();
let connected = false;
let lastEventAt = 0; // son geçerli fiyat kaydı
let lastPayloadAt = 0; // son ham olay (boş payload dahil)
let indexHtml = null;

const groupRank = (g) => {
  const i = GROUP_ORDER.indexOf(g);
  return i === -1 ? GROUP_ORDER.length : i;
};

function sortedPrices() {
  return [...prices.values()].sort(
    (a, b) => groupRank(a.group) - groupRank(b.group) || a.name.localeCompare(b.name, "tr"),
  );
}

// ---------- Harem bağlantısı ----------
const socket = io(SOCKET_URL, {
  transports: ["websocket"],
  extraHeaders: { Origin: ORIGIN },
  reconnection: true,
  reconnectionDelay: 2_000,
  reconnectionDelayMax: 30_000,
});

socket.on("connect", () => {
  connected = true;
  console.log("[harem] bağlandı", socket.id);
});
socket.on("disconnect", (reason) => {
  connected = false;
  console.log("[harem] bağlantı koptu:", reason);
});
socket.on("connect_error", (e) => console.log("[harem] hata:", e.message));
socket.on("error", (e) => console.log("[harem] soket hatası:", e?.message ?? e));

if (DEBUG) {
  socket.onAny((event, ...args) =>
    console.log("[debug]", event, JSON.stringify(args).slice(0, 300)),
  );
}

socket.on("price_changed", (payload) => {
  // Burada fırlayan bir hata tüm süreci düşürmemeli.
  try {
    lastPayloadAt = Date.now();
    const changed = normalizePayload(payload);
    if (!changed.length) return;
    lastEventAt = Date.now();
    for (const item of changed) prices.set(item.code, item);
    broadcast(changed);
  } catch (e) {
    console.error("[harem] payload işlenemedi:", e);
  }
});

// ---------- SSE ----------
function dropClient(res) {
  sseClients.delete(res);
  try {
    res.end();
  } catch {}
  try {
    res.destroy();
  } catch {}
}

function sseWrite(res, chunk) {
  if (res.writableEnded || res.destroyed) {
    sseClients.delete(res);
    return false;
  }
  if (res.writableLength > SSE_MAX_BUFFER) {
    console.log("[sse] yavaş istemci düşürüldü");
    dropClient(res);
    return false;
  }
  try {
    res.write(chunk);
    return true;
  } catch (e) {
    console.log("[sse] yazma hatası:", e.message);
    dropClient(res);
    return false;
  }
}

function broadcast(items) {
  const msg = `data: ${JSON.stringify(items)}\n\n`;
  for (const res of sseClients) sseWrite(res, msg);
}

// Kripto anlık görüntüsü ayrı istemcilere itilir (Harem fiyatlarıyla karışmaz).
onKripto((snapshot) => {
  if (!kriptoClients.size) return;
  const msg = `data: ${JSON.stringify(snapshot)}\n\n`;
  for (const res of kriptoClients) sseWrite(res, msg);
});

const heartbeat = setInterval(() => {
  const ping = ": ping\n\n";
  for (const res of sseClients) sseWrite(res, ping);
  for (const res of kriptoClients) sseWrite(res, ping);
}, SSE_PING_MS);
heartbeat.unref();

// ---------- HTTP ----------
function json(res, status, body) {
  const buf = Buffer.from(JSON.stringify(body));
  res.writeHead(status, {
    "Content-Type": "application/json; charset=utf-8",
    "Content-Length": buf.length,
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
    "Access-Control-Allow-Origin": CORS_ORIGIN,
  });
  res.end(buf);
}

function safeDecode(s) {
  try {
    return decodeURIComponent(s);
  } catch {
    return null;
  }
}

async function serveIndex(res) {
  try {
    if (!indexHtml) indexHtml = await readFile(new URL("./index.html", import.meta.url));
  } catch (e) {
    console.error("[gateway] index.html okunamadı:", e.message);
    return json(res, 500, { error: "index.html okunamadı" });
  }
  res.writeHead(200, {
    "Content-Type": "text/html; charset=utf-8",
    "Content-Length": indexHtml.length,
    "Cache-Control": "no-cache",
    "X-Content-Type-Options": "nosniff",
  });
  res.end(indexHtml);
}

async function handle(req, res) {
  const method = req.method ?? "GET";

  if (method === "OPTIONS") {
    res.writeHead(204, {
      "Access-Control-Allow-Origin": CORS_ORIGIN,
      "Access-Control-Allow-Methods": "GET, HEAD, OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type",
      "Access-Control-Max-Age": "86400",
    });
    return res.end();
  }
  if (method !== "GET" && method !== "HEAD") {
    return json(res, 405, { error: "yalnızca GET destekleniyor" });
  }

  let url;
  try {
    url = new URL(req.url ?? "/", `http://${req.headers.host ?? "localhost"}`);
  } catch {
    return json(res, 400, { error: "geçersiz istek" });
  }

  const path = url.pathname;

  if (path === "/" || path === "/index.html") return serveIndex(res);

  if (path === "/health") {
    const age = lastEventAt ? Date.now() - lastEventAt : null;
    const stale = age === null || age > STALE_AFTER_MS;
    const kripto = getKripto();
    return json(res, stale ? 503 : 200, {
      connected,
      stale,
      lastEventAgeMs: age,
      lastPayloadAgeMs: lastPayloadAt ? Date.now() - lastPayloadAt : null,
      count: prices.size,
      kripto: {
        usd: kripto.sources.usd,
        tl: kripto.sources.tl,
        stale: kripto.stale,
        count: kripto.withData,
      },
      uptimeSec: Math.round(process.uptime()),
    });
  }

  if (path === "/prices") {
    const group = url.searchParams.get("group");
    const list = sortedPrices().filter((p) => !group || p.group === group);
    return json(res, 200, list);
  }

  if (path.startsWith("/prices/")) {
    const raw = safeDecode(path.slice(8));
    if (raw === null) return json(res, 400, { error: "geçersiz kod" });
    const item = prices.get(raw.trim().toUpperCase());
    return item ? json(res, 200, item) : json(res, 404, { error: "bulunamadı" });
  }

  if (path === "/tcmb") {
    const data = await getTcmb();
    const ok = !data.today?.error || !data.hourly?.error;
    return json(res, ok ? 200 : 502, data);
  }

  if (path === "/bist") {
    const data = await getBist100();
    return json(res, data.withData ? 200 : 502, data);
  }

  if (path === "/kripto") {
    const data = getKripto();
    return json(res, data.withData ? 200 : 502, data);
  }

  if (path === "/kripto/stream") {
    res.writeHead(200, {
      "Content-Type": "text/event-stream; charset=utf-8",
      "Cache-Control": "no-cache, no-transform",
      Connection: "keep-alive",
      "X-Accel-Buffering": "no",
      "Access-Control-Allow-Origin": CORS_ORIGIN,
    });
    res.write("retry: 3000\n\n");

    const cleanup = () => kriptoClients.delete(res);
    req.on("close", cleanup);
    req.on("error", cleanup);
    res.on("error", cleanup);

    kriptoClients.add(res);
    sseWrite(res, `data: ${JSON.stringify(getKripto())}\n\n`);
    return;
  }

  if (path === "/stream") {
    res.writeHead(200, {
      "Content-Type": "text/event-stream; charset=utf-8",
      "Cache-Control": "no-cache, no-transform",
      Connection: "keep-alive",
      "X-Accel-Buffering": "no",
      "Access-Control-Allow-Origin": CORS_ORIGIN,
    });
    res.write("retry: 3000\n\n");

    const cleanup = () => sseClients.delete(res);
    req.on("close", cleanup);
    req.on("error", cleanup);
    res.on("error", cleanup);

    sseClients.add(res);
    sseWrite(res, `data: ${JSON.stringify(sortedPrices())}\n\n`); // ilk anlık görüntü
    return;
  }

  json(res, 404, { error: "geçersiz yol" });
}

const server = http.createServer((req, res) => {
  handle(req, res).catch((e) => {
    console.error("[gateway] istek hatası:", e);
    if (res.headersSent) {
      try {
        res.end();
      } catch {}
    } else {
      json(res, 500, { error: "sunucu hatası" });
    }
  });
});

// ---------- TCMB önbelleği ----------
async function warmTcmb() {
  try {
    const d = await getTcmb();
    if (DEBUG) {
      console.log(
        "[tcmb] önbellek hazır:",
        d.today?.error ? `today: ${d.today.error}` : `today ${Object.keys(d.today.items).length} kayıt`,
        d.hourly?.error ? `hourly: ${d.hourly.error}` : `hourly ${Object.keys(d.hourly.items).length} kayıt`,
      );
    }
  } catch (e) {
    console.warn("[tcmb] önbellek hatası:", e.message);
  }
}

const tcmbTimer = setInterval(warmTcmb, TCMB_WARM_MS);
tcmbTimer.unref();

// ---------- BIST 100 ----------
async function warmBist() {
  try {
    const d = await getBist100();
    if (DEBUG) {
      console.log(
        `[bist] ${d.withData}/${d.count} kayıt, bileşen ${d.date}` +
          (d.error ? `, hata: ${d.error}` : ""),
      );
    }
  } catch (e) {
    console.warn("[bist] önbellek hatası:", e.message);
  }
}

// ---------- Yaşam döngüsü ----------
function shutdown(sig) {
  console.log(`[gateway] ${sig} alındı, kapatılıyor`);
  clearInterval(heartbeat);
  clearInterval(tcmbTimer);
  stopKripto();
  for (const res of [...sseClients, ...kriptoClients]) {
    try {
      res.end();
    } catch {}
  }
  sseClients.clear();
  kriptoClients.clear();
  socket.close();
  server.close(() => process.exit(0));
  setTimeout(() => process.exit(0), 3_000).unref();
}

process.on("SIGINT", () => shutdown("SIGINT"));
process.on("SIGTERM", () => shutdown("SIGTERM"));
process.on("uncaughtException", (e) => console.error("[gateway] yakalanmayan hata:", e));
process.on("unhandledRejection", (e) => console.error("[gateway] işlenmeyen promise:", e));

server.listen(PORT, HOST, () => {
  console.log(`[gateway] http://${HOST === "0.0.0.0" ? "localhost" : HOST}:${PORT}`);
  warmTcmb();
  warmBist();
  startKripto();
});
