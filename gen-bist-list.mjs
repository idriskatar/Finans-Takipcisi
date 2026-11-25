// bist100-list.mjs yedeğini Borsa İstanbul'un CSV'sinden yeniden üretir.
// İndirme ya da ayrıştırma başarısız olursa mevcut dosyaya DOKUNMAZ.
//
// Kullanım: node gen-bist-list.mjs

import { writeFile, rename } from "node:fs/promises";
import { parseConstituents } from "./bist100.mjs";

const CSV_URL = "https://www.borsaistanbul.com/datum/hisse_endeks_ds.csv";
const UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36";

const res = await fetch(CSV_URL, { headers: { "User-Agent": UA } });
if (!res.ok) {
  console.error(`CSV indirilemedi (HTTP ${res.status}); mevcut liste değiştirilmedi.`);
  process.exit(1);
}

const parsed = parseConstituents(await res.text());
if (!parsed || parsed.items.length < 10) {
  console.error("CSV çözümlenemedi; mevcut liste değiştirilmedi.");
  process.exit(1);
}

const items = parsed.items.slice().sort((a, b) => a.code.localeCompare(b.code, "tr"));
const body = items
  .map(({ code, name }) => `  [${JSON.stringify(code)}, ${JSON.stringify(name)}],`)
  .join("\n");

const out = `// BIST 100 (XU100) bileşenleri — yedek liste.
// Kaynak: ${CSV_URL}
// Tarih: ${parsed.date} · ${items.length} kayıt
// Sunucu bu listeyi 12 saatte bir aynı adresten günceller; bu dosya yalnızca indirme
// başarısız olduğunda (ör. ilk açılışta internet yokken) kullanılır.

export const FALLBACK_DATE = ${JSON.stringify(parsed.date)};

/** @type {Array<[string, string]>} [kod, BIST kısa adı] */
export const FALLBACK = [
${body}
];
`;

// Önce geçici dosyaya yaz, sonra yerine taşı: yarı yazılmış liste kalmasın.
const target = new URL("./bist100-list.mjs", import.meta.url);
const tmp = new URL("./bist100-list.mjs.tmp", import.meta.url);
await writeFile(tmp, out, "utf8");
await rename(tmp, target);

console.log("yazildi:", items.length, "kayit, tarih", parsed.date);
