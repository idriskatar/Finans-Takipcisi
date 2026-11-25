// Harem "price_changed" olayındaki ham kayıtları temiz bir şemaya çevirir.

// Bu yüzdenin üzerindeki (veya negatif) spread şüpheli sayılır.
export const WIDE_SPREAD_PCT = 5;

// Bilinen kodların Türkçe adları. Listede olmayan kod, kendi koduyla görünür.
export const LABELS = {
  // TL kurları
  USDTRY: ["Dolar / TL", "doviz"],
  EURTRY: ["Euro / TL", "doviz"],
  GBPTRY: ["Sterlin / TL", "doviz"],
  CHFTRY: ["İsviçre Frangı / TL", "doviz"],
  AUDTRY: ["Avustralya Doları / TL", "doviz"],
  CADTRY: ["Kanada Doları / TL", "doviz"],
  JPYTRY: ["Japon Yeni / TL", "doviz"],
  SARTRY: ["Suudi Riyali / TL", "doviz"],
  KWDTRY: ["Kuveyt Dinarı / TL", "doviz"],
  JODTRY: ["Ürdün Dinarı / TL", "doviz"],
  // Pariteler
  EURUSD: ["Euro / Dolar", "parite"],
  GBPUSD: ["Sterlin / Dolar", "parite"],
  USDCHF: ["Dolar / Frank", "parite"],
  USDCAD: ["Dolar / Kanada Doları", "parite"],
  AUDUSD: ["Avustralya Doları / Dolar", "parite"],
  USDJPY: ["Dolar / Yen", "parite"],
  // Altın
  ALTIN: ["Altın (gram)", "altin"],
  AYAR22: ["22 Ayar Altın (gram)", "altin"],
  AYAR14: ["14 Ayar Altın (gram)", "altin"],
  KULCEALTIN: ["Külçe Altın", "altin"],
  ONS: ["Ons Altın (USD)", "altin"],
  USDKG: ["Altın Dolar/Kg", "altin"],
  EURKG: ["Altın Euro/Kg", "altin"],
  // Sarrafiye
  CEYREK_YENI: ["Çeyrek (Yeni)", "sarrafiye"],
  CEYREK_ESKI: ["Çeyrek (Eski)", "sarrafiye"],
  YARIM_YENI: ["Yarım (Yeni)", "sarrafiye"],
  YARIM_ESKI: ["Yarım (Eski)", "sarrafiye"],
  TEK_YENI: ["Tam (Yeni)", "sarrafiye"],
  TEK_ESKI: ["Tam (Eski)", "sarrafiye"],
  ATA_YENI: ["Ata (Yeni)", "sarrafiye"],
  ATA_ESKI: ["Ata (Eski)", "sarrafiye"],
  ATA5_YENI: ["Ata 5'li (Yeni)", "sarrafiye"],
  ATA5_ESKI: ["Ata 5'li (Eski)", "sarrafiye"],
  GREMESE_YENI: ["Gremese (Yeni)", "sarrafiye"],
  GREMESE_ESKI: ["Gremese (Eski)", "sarrafiye"],
  // Diğer madenler
  GUMUSTRY: ["Gümüş (TL)", "maden"],
  GUMUSUSD: ["Gümüş (USD)", "maden"],
  XAGUSD: ["Ons Gümüş (USD)", "maden"],
  XPTUSD: ["Ons Platin (USD)", "maden"],
  XPDUSD: ["Ons Paladyum (USD)", "maden"],
  PLATIN: ["Platin", "maden"],
  PALADYUM: ["Paladyum", "maden"],
};

const NUM_RE = /^[+-]?(\d+(\.\d*)?|\.\d+)$/;

// "50.7380" (string), 50.738 (number) veya "1.234,56" -> number.
// Boş/geçersiz değer -> null. Kısmen sayı olan metin ("50.7abc") kabul edilmez.
export function toNumber(v) {
  if (v === null || v === undefined || v === "") return null;
  if (typeof v === "number") return Number.isFinite(v) ? v : null;
  if (typeof v !== "string") return null;

  let s = v.trim().replace(/[\s\u00a0\u202f]/g, "");
  if (!s) return null;

  const hasDot = s.includes(".");
  const hasComma = s.includes(",");
  if (hasDot && hasComma) {
    // sonra gelen ayırıcı ondalık ayırıcıdır
    s =
      s.lastIndexOf(",") > s.lastIndexOf(".")
        ? s.replace(/\./g, "").replace(",", ".")
        : s.replace(/,/g, "");
  } else if (hasComma) {
    const parts = s.split(",");
    // Türkçe kaynak: tek virgül her zaman ondalık ("50,738" -> 50.738);
    // birden fazla virgül binlik ayırıcıdır ("1,234,567" -> 1234567).
    s = parts.length === 2 ? `${parts[0]}.${parts[1]}` : s.replace(/,/g, "");
  } else if (hasDot && s.split(".").length > 2) {
    s = s.replace(/\./g, ""); // "1.234.567" binlik
  }

  if (!NUM_RE.test(s)) return null;
  const n = Number(s);
  return Number.isFinite(n) ? n : null;
}

const pad = (n) => String(n).padStart(2, "0");
const TARIH_RE =
  /^(\d{1,2})[-./](\d{1,2})[-./](\d{4})(?:[ T](\d{1,2}):(\d{2})(?::(\d{2}))?)?$/;

let warnedTarih = false;

// "23-11-2026 11:02:01" (İstanbul saati, UTC+3 sabit) -> ISO 8601.
// "23.11.2026 11:02" gibi varyantlar da kabul edilir; tanınmayan biçim bir kez uyarı verir.
export function parseTarih(s) {
  if (typeof s !== "string" || !s.trim()) return null;
  const m = TARIH_RE.exec(s.trim());
  if (!m) {
    if (!warnedTarih) {
      warnedTarih = true;
      console.warn("[normalize] tanınmayan tarih biçimi:", JSON.stringify(s));
    }
    return null;
  }
  const [, d, mo, y, h = "0", mi = "0", se = "0"] = m;
  const date = new Date(`${y}-${pad(mo)}-${pad(d)}T${pad(h)}:${pad(mi)}:${pad(se)}+03:00`);
  return Number.isNaN(date.getTime()) ? null : date.toISOString();
}

// Harem'in yön alanı "up"/"down", 1/-1 ya da "1"/"-1" olabilir.
function dirOf(v) {
  if (v === 1 || v === "1" || v === "up") return "up";
  if (v === -1 || v === "-1" || v === "down") return "down";
  return null;
}

// Tek bir ham kaydı normalize eder. Boş/kullanılamaz kayıt için null döner.
export function normalizeItem(key, raw) {
  if (!raw || typeof raw !== "object") return null;

  const buy = toNumber(raw.alis);
  const sell = toNumber(raw.satis);

  // alis=0, satis=0, tarih="" olan kayıtlar (örn. OMRUSD) veri taşımıyor
  if ((buy === null || buy === 0) && (sell === null || sell === 0)) return null;

  const code = String(raw.code || key || "").trim().toUpperCase();
  if (!code) return null;

  const [name, group] = LABELS[code] ?? [code, "diger"];

  let low = toNumber(raw.dusuk);
  let high = toNumber(raw.yuksek);
  if (low !== null && high !== null && low > high) [low, high] = [high, low];

  const spreadPct =
    buy !== null && buy !== 0 && sell !== null && sell > 0
      ? +(((sell - buy) / sell) * 100).toFixed(2)
      : null;

  return {
    code,
    name,
    group,
    buy,
    sell,
    low,
    high,
    prevClose: toNumber(raw.kapanis),
    buyDir: dirOf(raw.dir?.alis_dir), // "up" | "down" | null
    sellDir: dirOf(raw.dir?.satis_dir),
    spreadPct,
    // negatif spread (alış > satış) da veri hatasıdır
    suspect: spreadPct !== null && (spreadPct < 0 || spreadPct > WIDE_SPREAD_PCT),
    updatedAt: parseTarih(raw.tarih),
  };
}

// price_changed gövdesindeki ({meta, data}) tüm kayıtları normalize eder
export function normalizePayload(payload) {
  const out = [];
  const data = payload?.data;
  if (!data || typeof data !== "object") return out;
  for (const [key, raw] of Object.entries(data)) {
    const item = normalizeItem(key, raw);
    if (item) out.push(item);
  }
  return out;
}
