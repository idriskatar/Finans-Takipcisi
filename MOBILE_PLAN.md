# Mobil sürüm planı (Android + iOS)

Mimari: **paylaşılan `core/` Rust motoru** + platforma özel kabuk.

```
Finance/
├── core/                         # Harem/TCMB/BIST/Kripto + önbellek + sağlık + storage
├── desktop/                      # Tauri masaüstü (core'u kullanır)
└── mobile/                       # Tauri mobil (core'u kullanır)
    ├── frontend/index.html       # mobil arayüz (alt navigasyon, invoke API)
    └── src-tauri/
        ├── tauri.conf.json       # identifier: com.idriskatar.finans-takipcisi.mobile
        ├── capabilities/mobile.json
        ├── icons/                # Android/iOS ikonları (cargo tauri icon ile üretildi)
        └── src/lib.rs            # invoke komutları: snapshot, get_user_data, save_user_data
```

## Neler hazır?

- ✅ `core/` crate'i: tüm veri kaynakları, disk önbelleği, kaynak sağlığı, favori/ayar dosyası.
- ✅ Masaüstü `core`'a geçirildi ve `cargo check` geçiyor.
- ✅ `mobile/` Tauri projesi: `cargo check` geçiyor.
- ✅ Mobil arayüz taslağı: 6 sekme (Favoriler/Harem/BIST/TCMB/Kripto/Ayarlar), favori yıldızları,
  tema seçimi, kaynak durumu, Hakkında (İdris KATAR + veri kaynakları + MIT).
- ✅ Android/iOS ikon seti üretildi (`cargo tauri icon ../icon-1024.png`).

## Bu ortamda yapılamayanlar

| Adım | Neden | Nerede yapılır |
|---|---|---|
| `cargo tauri android init` | Java / Android Studio jbr yok (JAVA_HOME tanımsız) | Java 17 + Android Studio kurulu makine |
| `cargo tauri android build` | Android SDK/NDK gerekir | aynı makine |
| `cargo tauri ios init` / `build` | macOS + Xcode gerekir | macOS makine |

## Android adımları (Java + Android Studio kurulu makinede)

```bash
set JAVA_HOME=C:\Program Files\Android\Android Studio\jbr
cd mobile\src-tauri
cargo tauri android init
cargo tauri android build          # APK → mobile/src-tauri/gen/android/app/build/outputs/
```

İlk init sonrası `gen/android/` klasörü oluşur. İzinler için
`capabilities/mobile.json` içine `http` permission'ları gerekirse eklenir;
`INTERNET` izni Tauri tarafından otomatik eklenir.

Bildirimler için (Android 13+) `POST_NOTIFICATIONS` izni ve bir Tauri notification
eklentisi gerekir — şu an alarm bildirimi uygulama içi toast olarak çalışır.

## iOS adımları (macOS + Xcode'lu makinede)

**CI'da imzasız simulator derlemesi** (Team ID gerekmez, `Build Mobile` workflow'u bunu yapar):

```bash
cd mobile/src-tauri
rustup target add aarch64-apple-ios-sim
cargo tauri ios init --ci
cargo tauri ios build --target aarch64-sim --ci
```

Bu çıktı simulator içinde çalışır; **cihaza kurulamaz / App Store'a yüklenemez**.

**Cihaz / App Store için imzalı IPA** (Apple Developer hesabı gerekir):

```bash
export APPLE_DEVELOPMENT_TEAM=XXXXXXXXXX        # Team ID
export APPLE_API_KEY=...                        # App Store Connect API key
export APPLE_API_ISSUER=...
export APPLE_API_KEY_PATH=/path/AuthKey_XXX.p8
cargo tauri ios build --export-method app-store-connect
```

veya `IOS_CERTIFICATE` / `IOS_CERTIFICATE_PASSWORD` / `IOS_MOBILE_PROVISION`
(değişkenleri base64) ile manuel imzalama. Bu değerleri GitHub **Repository secrets**
olarak ekleyip workflow'da env olarak vermeniz gerekir.

## Sonraki iyileştirmeler (mobil)

1. Arka plan veri yenileme: Android `WorkManager` / iOS Background Fetch.
2. Gerçek push bildirim: FCM (Android) + APNs (iOS) — sunucu tarafı gerektirir.
3. Çevrimdışı mod: `core` önbelleği zaten var; mobil arayüzde "son veri" damgası gösterilir.
4. UI polishing: pull-to-refresh, büyük dokunma alanları, güncelleme animasyonu.
5. Favori aktarımı ve fiyat alarmı: masaüstündeki mantık `core`'a taşınabilir / mobil UI'a eklenebilir.

## Lisans / kimlik

Geliştirici **İdris KATAR** · **MIT** · uygulama adı **Finans Takipçisi**.
Mobil identifier masüstünden ayrı: `com.idriskatar.finans-takipcisi.mobile`.
