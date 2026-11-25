# Katkı rehberi

Finans Takipçisi açık kaynaklıdır (MIT). Katkılar memnuniyetle karşılanır.

## Geliştirme ortamı

- Rust (stable, MSVC toolchain on Windows) — https://rustup.rs
- Tauri v2 prerequisite'leri — https://v2.tauri.app/start/prerequisites/
- Windows: WebView2 (Windows 11'de hazır), NSIS / WiX (paketleme için)

```bash
cd desktop/src-tauri
cargo tauri dev
```

## Kod yapısı

| Dizin / dosya | Açıklama |
|---|---|
| `desktop/src-tauri/src/sources/harem.rs` | Socket.IO (Engine.IO) istemcisi + veri normalizasyonu |
| `.../tcmb.rs` | TCMB XML ayrıştırma (gösterge + saat başı) |
| `.../bist.rs` | BIST listesi + Yahoo Finance (çerez/crumb + chart yedeği) |
| `.../kripto.rs` | Binance + BtcTurk REST/WebSocket, anlık görüntü |
| `.../server.rs` | axum REST + SSE uç noktaları |
| `.../state.rs` | Paylaşılan durum ve yayın (broadcast) kuyrukları |
| `desktop/frontend/index.html` | Arayüz, tema, ayarlar, favoriler, hakkında |

## Kurallar

- Veri kaynaklarına saygılı olun: istek aralıklarını kısaltmayın.
- Arayüz metinleri Türkçe, DOM yazımları `textContent` ile yapılmalı.
- Yeni bağımlılık eklemeden önce `cargo check` ve `cargo tauri build` başarısını doğrulayın.
- Lisans bildirimi (© 2026 İdris KATAR) korunmalıdır.

## Sorun bildirme

GitHub Issues üzerinden; platform, sürüm ve yeniden üretme adımlarını ekleyin.
Yatırım tavsiyesi içermeyen, bilgilendirme amaçlı bir araçtır.
