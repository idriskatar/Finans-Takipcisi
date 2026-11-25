# Finans Takipçisi — Teslim raporu

**Geliştirici:** İdris KATAR  
**Lisans:** MIT  
**Sürüm:** 1.0.0  
**Platformlar:** Windows, macOS (Intel + Apple Silicon), Linux

## 1. Masaüstü uygulaması (Rust + Tauri v2)

`desktop/` klasöründe tamamen Rust tabanlı bir Tauri uygulaması bulunur:

- `src/sources/harem.rs` — Socket.IO/Engine.IO istemcisi (wss), veri normalizasyonu
- `src/sources/tcmb.rs` — TCMB XML (today.xml + reeskontkur), 22 + 4 kayıt
- `src/sources/bist.rs` — BIST 100 listesi + Yahoo Finance (çerez/crumb, chart yedeği), 100 hisse
- `src/sources/kripto.rs` — Binance + BtcTurk REST/WebSocket, 26 coin
- `src/server.rs` — gömülü axum HTTP sunucusu: `/health`, `/prices`, `/stream` (SSE),
  `/tcmb`, `/bist`, `/kripto`, `/kripto/stream`
- `src/state.rs` — paylaşılan durum ve broadcast kuyrukları
- `bin/headless.rs` — penceresiz test modu (`http://127.0.0.1:4501`)

Doğrulanan canlı sonuçlar (headless mod):

```
/health   200  connected=true  stale=false  count=48  kripto=26
/tcmb     200  today=22  hourly=4  hata yok
/bist     200  items=100  endeks=11104.38
/kripto   200  withData=26/26  usd=binance  tl=btcturk
/prices   200  items=48
```

## 2. Arayüz (`desktop/frontend/index.html`)

- 7 sekme: **Favoriler | HAREM | BIST 100 | TCMB | Kripto | Ayarlar | Hakkında**
- Tüm tabloların her satırında yıldız (☆/★) düğmesi; seçimler Favoriler'de canlı görünür,
  `localStorage`'da saklanır.
- Sütun sıralaması 3 durumlu: azalan → artan → gelen veri sırası.
- Tema desteği: Sistem / Açık / Koyu (`data-theme` CSS override, anında geçiş).
- Ayarlar: tema, BIST/TCMB yenileme süreleri, şüpheli spread eşiği, varsayılanlara dön.
- Hakkında: Geliştirici **İdris KATAR**, sürüm, tüm veri kaynakları ve MIT lisans bilgisi.
- Tarayıcı konsolunda hata yok; tema geçişi ve ayar kalıcılığı doğrulandı.

## 3. Dağıtım / GitHub

| Dosya | Açıklama |
|---|---|
| `LICENSE` | MIT Lisansı, © 2026 İdris KATAR |
| `.gitignore` | Node/Rust/derleme çıktıları |
| `README.md` | Masaüstü uygulaması + eski Node gateway dokümantasyonu |
| `GITHUB_SETUP.md` | Depo oluşturma, etiketleme, sürüm yükleme adımları |
| `CONTRIBUTING.md` | Katkı kuralları |
| `.github/workflows/build.yml` | Windows + macOS (x2) + Linux otomatik derleme ve taslak sürüm |
| `desktop/build-windows.bat` | Yerel NSIS kurulum paketi derleme betiği |

## 4. Windows kurulum paketi — ✅ DERLENDİ

```
desktop/src-tauri/target/release/bundle/nsis/Finans Takipçisi_1.0.0_x64-setup.exe
Boyut: 4.70 MiB · NSIS, x64 · Tarih: 23.11.2026
Ana çalıştırılabilir: desktop/src-tauri/target/release/finans-takipcisi.exe (13.7 MB)

Uygulama simgesi: `desktop/src-tauri/icons/` — kullanıcının sağladığı altın coin görselinden
üretilen `icon.png` (512), `128x128.png`, `32x32.png` ve `icon.ico`; kaynak kopya
`icon-source.png` dosyasında tutulur.

### Kalıcı veri
Ayarlar ve favoriler uygulama veri klasöründeki `user-data.json` dosyasına yazılır
(Windows: `%APPDATA%\com.idriskatar.finans-takipcisi\user-data.json`). Gömülü sunucu
sabit port kullanır (4510–4519 arası ilk boş port); `localStorage` yedek olarak tutulur.
Yeniden başlatma sonrası favoriler ve ayarlar geri yüklenir (headless modda test edildi).

### Kaynak durumu ve önbellek
`/health` artık kaynak bazlı `ok`, `connected`, `last_data_at`, `error`, `from_cache`
alanlarını döner; Ayarlar sekmesindeki **Kaynak durumu** kartı bunları gösterir.
Son başarılı veriler 30 sn'de bir `cache/{harem,tcmb,bist,kripto}.json` dosyalarına
yazılır ve açılışta ağ isteklerinden önce yüklenir.

### Favori aktarımı ve alarm
Favoriler sekmesinden JSON dışa/içe aktarma yapılabilir. Her favori satırında ↑/↓ yönü
ve eşik fiyatı girilerek fiyat alarmı kurulabilir; eşik aşılınca uygulama içi bildirim
gösterilir (aynı alarm 5 dk boyunca tekrarlanmaz).
```

Yeniden derlemek için:

```bash
cd desktop/src-tauri
cargo tauri build --bundles nsis
```

## 5. Eski Node sürümü

Kök klasördeki `gateway.mjs`, `index.html` ve veri modülleri referans/altyapı olarak korundu
(port 4500). Yeni uygulama bunların mantığını Rust'a taşır; bağımlılık yoktur.
