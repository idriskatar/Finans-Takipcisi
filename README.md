# Finans Takipçisi

Çok platformlu finans takip uygulaması: **Rust + Tauri**.
**Geliştirici: İdris KATAR** · **Lisans: MIT**

## ⬇️ Son sürümü indir

**Güncel sürüm: v1.6.3** · [Tüm sürümler / en yenisi](https://github.com/idriskatar/Finans-Takipcisi/releases/latest)

| Platform | İndirme |
|---|---|
| **Windows** (kurulum paketi) | [Finans-Takipcisi-Windows-setup.exe](https://github.com/idriskatar/Finans-Takipcisi/releases/latest/download/Finans-Takipcisi-Windows-setup.exe) |
| **Android** (telefona kurulabilir APK) | [Finans-Takipcisi-Android.apk](https://github.com/idriskatar/Finans-Takipcisi/releases/latest/download/Finans-Takipcisi-Android.apk) |
| **macOS / Linux** | [Releases sayfasından indirin](https://github.com/idriskatar/Finans-Takipcisi/releases/latest) |

> Uygulama açılışta GitHub'dan yeni sürüm kontrolü yapar. Masaüstünde güncelleme onayla birlikte kurulur; mobilde **"Yeni sürüm hazır · İndir"** şeridi APK bağlantısını açar. Ayrıntılar: `UPDATE_SETUP.md`.

## Proje yapısı

| Klasör | Açıklama |
|---|---|
| `core/` | Paylaşılan Rust veri motoru: Harem/TCMB/BIST/Kripto, önbellek, kaynak sağlığı, kalıcı veri |
| `desktop/` | Windows/macOS/Linux masaüstü uygulaması (NSIS kurulum paketi hazır) |
| `mobile/` | Android/iOS Tauri projesi + mobil arayüz — bkz. `MOBILE_PLAN.md` |

Harem Altın, BIST 100, TCMB ve Kripto piyasaları aynı ekranda izlenir.

## Masaüstü uygulaması (Rust + Tauri)

`desktop/` klasörü uygulamanın tamamını içerir:

- **Rust** (axum + tokio + reqwest + tokio-tungstenite): dört veri kaynağını doğrudan çeker,
  gömülü HTTP sunucusunda REST + SSE olarak yayınlar.
- **Tauri v2**: yerel webview penceresi, yerel paketleyici (Windows NSIS/MSI, macOS DMG, Linux AppImage/deb).
- **Arayüz**: `desktop/frontend/index.html` — Favoriler, HAREM, BIST 100, TCMB, Kripto,
  Ayarlar ve Hakkında sekmeleri; açık/koyu/sistem teması; 3 durumlu sütun sıralaması.

### Geliştirme

```bash
cd desktop/src-tauri
cargo tauri dev
```

### Windows kurulum paketi üretme

```bash
cargo install tauri-cli --version '^2'
cd desktop/src-tauri
cargo tauri build --bundles nsis      # .exe kurulum paketi
cargo tauri build --bundles msi       # .msi (WiX gerektirir)
```

Çıktı: `desktop/src-tauri/target/release/bundle/nsis/`

### Başsız mod (sunucuyu pencere olmadan çalıştırır)

```bash
cd desktop/src-tauri
cargo run --release --bin headless    # http://127.0.0.1:4501
```

### GitHub'a yükleme

`.github/workflows/build.yml` dosyası `v*` etiketiyle tetiklenir ve üç platform için
taslak sürüm oluşturur. Ayrıntılar: `GITHUB_SETUP.md`, katkı kuralları: `CONTRIBUTING.md`.

---

## Eski sürüm (Node gateway)

Harem Altın canlı fiyat akışını tek bir WebSocket bağlantısıyla dinler, bellekte tutar;
tarayıcıya ve mobil uygulamaya REST + SSE ile dağıtır. Ayrıca TCMB gösterge kurlarını ve
saat başı kur/altın kayıtlarını `/tcmb`, BIST 100 hisselerini `/bist`, kripto fiyatlarını
`/kripto` üzerinden sunar.

## Çalıştırma

```bash
npm install
npm start          # http://localhost:4500
npm run debug      # ayrıntılı log (--debug)
```

### Ayarlar (ortam değişkenleri)

| Değişken | Varsayılan | Açıklama |
|---|---|---|
| `PORT` | `4500` | HTTP portu |
| `HOST` | `0.0.0.0` | Dinlenecek arayüz; yalnız yerel için `127.0.0.1` |
| `CORS_ORIGIN` | `*` | `Access-Control-Allow-Origin` değeri |
| `DEBUG` | – | `1` ise ham socket olaylarını loglar |

## Uç noktalar

| Yol | Açıklama |
|---|---|
| `GET /` | Arayüz (`index.html`) |
| `GET /health` | Bağlantı durumu; veri bayatsa `503` |
| `GET /prices` | Tüm fiyatlar (grup + ada göre sıralı) |
| `GET /prices?group=sarrafiye` | Grup filtresi (`doviz`, `altin`, `sarrafiye`, `parite`, `maden`, `diger`) |
| `GET /prices/USDTRY` | Tek kayıt |
| `GET /tcmb` | TCMB gösterge kurları + saat başı kur/altın |
| `GET /bist` | BIST 100 hisseleri + `indexQuote` (XU100 endeksi: son, değişim, saat) |
| `GET /kripto` | Kripto anlık görüntüsü: 26 coin, USD + TL, 24s değişim/yüksek/düşük/hacim |
| `GET /stream` | SSE akışı (`data: [...]`, 20 sn'de bir `: ping`) |
| `GET /kripto/stream` | Kripto SSE akışı (`data: {items, meta, ...}`, saniyede en fazla bir güncelleme) |

SSE istemcisi bağlanınca önce mevcut anlık görüntüyü, sonra değişiklikleri alır.

## Arayüz

Sayfanın üstünde dört bar vardır:

1. **Sabit bar** — BIST 100 (XU100) endeksi, TCMB saat başı döviz kurları (yalnız alış),
   Harem gram/ons altın, çeyrek, ata ve tam altın (alış/satış alt alta kutucuklarda).
2. **Kayan bar** — BIST 100 içinden en çok yükselen 10 ve düşen 10 hisse
   (`KOD → son fiyat → %`; fiyat koyu, yüzde açık renk).
3. **Kayan bar** — 12 sarrafiye ürünü, alış/satış alt alta.
4. **Kripto barı** — 26 coin; USD (Binance) ve TL (BtcTurk) fiyatı, 24s değişim.
   Mobilde TL fiyatı gizlenir.

Dört bar birlikte yapışır: sayfa kaydırılırken hepsi görünür kalır.
Barlar fare üzerine gelince durur; `prefers-reduced-motion` açıksa animasyon yerine
yatay kaydırma kullanılır. Veriler 30 sn'de bir (BIST) / 60 sn'de bir (TCMB) ve SSE ile
(Harem, kripto) yenilenir.

Barların altında **Favoriler / HAREM / BIST 100 / TCMB / Kripto** sekmeleri vardır; her sekme
kendi kartını gösterir (aynı anda tek bölüm açık). Seçilen sekme `localStorage`'da tutulur,
ok tuşlarıyla da gezinilebilir. HAREM kartının içinde kategori alt sekmeleri
(Tümü, Altın, Sarrafiye, Döviz, Parite, Maden, Diğer), arama ve şüpheli spread filtresi
bulunur. Kripto kartında BTC/ETH/USDT-TRY özet kartları, Tümü / Yükselenler / Düşenler /
Hacim alt sekmeleri, arama, sütun sıralaması ve 24s aralık çubuğu vardır.

Her tablodaki (HAREM, BIST, TCMB, Kripto) veri satırının başında bir yıldız (☆/★) düğmesi
vardır. Tıklanan kayıtlar **Favoriler** sekmesinde listelenir; değerler canlı güncellenir,
seçim tarayıcıda saklanır. Favorilerden çıkarınca kaynak tablodaki yıldız da söner;
"Tümünü temizle" tüm seçimleri siler.

Sütun başlıkları 3 durumlu sıralama yapar: 1. tık azalan, 2. tık artan, 3. tık sıralamayı
kaldırıp gelen veri sırasına döner. Tablo başlıkları yapışkan barların hemen altında
(`--bar-h`, JS ile ölçülür) sabitlenir; mobilde (≤720px) tablolar yatay kayar ve başlık
normal akışta kalır.

## Veri kaynakları

- **Harem**: `wss://hrmsocketonly.haremaltin.com` → `price_changed` olayı.
  `normalize.mjs` ham kayıtları ortak şemaya çevirir (sayı/tarih ayrıştırma, Türkçe adlar,
  spread ve şüpheli veri işareti).
- **TCMB gösterge kurları**: `https://www.tcmb.gov.tr/kurlar/today.xml`
- **TCMB saat başı kur/altın**: `https://www.tcmb.gov.tr/reeskontkur/YYYYMM/DDMMYYYY-HHMM.xml`
  - 23.11.2026 itibarıyla doğrulandı: dosyalar 11:00 (döviz) ile 12:00/14:00/16:00 (altın,
    `XAU`/`XAS`) saatlerinde yayımlanıyor; her gün hepsi birden olmayabilir.
  - Bu servis **yalnızca alış** fiyatı verir, ondalık ayırıcı virgüldür, değerler `birim`
    başınadır.
  - Sonuç 5 dk, gösterge kurları 10 dk önbelleklenir; hata sonrası 1 dk'da yeniden denenir.
- **BIST 100**: bileşenler `https://www.borsaistanbul.com/datum/hisse_endeks_ds.csv`
  (XU100 satırları, 12 saat önbellek), fiyatlar Yahoo Finance `v7/finance/quote`
  (çerez + crumb, 100 sembol tek istek + XU100.IS endeksi, 20 sn önbellek). Toplu uç HTTP
  hatası verir ya da boş dönerse sembol başına `v8/finance/chart` yedeğine geçer (o modda
  60 sn önbellek); o da olmazsa son iyi veri "bayat" olarak sunulur.
  - Hacim, Yahoo'nun verdiği işlem **adedi × son fiyat** yaklaşımıdır; tam işlem tutarı değildir.
  - Liste indirilemezse `bist100-list.mjs` (23.11.2026 tarihli yedek) kullanılır.
  - Yedek listeyi yenilemek için: `node gen-bist-list.mjs`
- **Kripto**: USD tarafı Binance, TL tarafı BtcTurk.
  - Binance REST `api.binance.com/api/v3/ticker/24hr` (≤20 sembollük parçalar, 60 sn) +
    WebSocket `stream.binance.com:9443` `@miniTicker` (26 çift, saniyede bir).
  - BtcTurk REST `api.btcturk.com/api/v2/ticker` (389 çift tek istek, 60 sn) +
    WebSocket `ws-feed-pro.btcturk.com` `ticker` kanalı (TRY çiftleri + USDTTRY).
  - USD fiyatı Binance'ten, TL fiyatı BtcTurk'ten gelir; ikisi bağımsız yedeklenir
    (Binance düşerse BtcTurk USDT çiftleri, BtcTurk düşerse USDT/TRY ile tahmin).
  - BtcTurk'te TRY çifti olmayan coinlerde (BNB, TON, BCH) TL **tahminidir** ve `≈` ile
    işaretlenir.
  - SSE anlık görüntüsü saniyede en fazla bir kez itilir; REST yalnız eksik alanları tazeler.

## Notlar

- `/tcmb` ilk istekte yavaş olabilir (TCMB yanıtları 3-9 sn); sunucu açılışta önbelleği
  kendisi doldurur.
- Fiyatlar resmî değildir, işlem kararı için kullanılmamalıdır.
