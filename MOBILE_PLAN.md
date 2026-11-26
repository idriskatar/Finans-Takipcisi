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
- ✅ Mobil arayüz: 6 sekme (Favoriler/Harem/BIST/TCMB/Kripto/Ayarlar) ve masaüstüyle
  **özellik eşitliği**: kategori sekmeleri + arama + şüpheli filtresi (Harem), BIST sıralama
  ve endeks kartı, TCMB saat başı + gösterge + birim/nakit kurlar, kripto özet kartları +
  alt sekmeler + arama + sıralama + 24s aralık çubuğu + hacim, favorilerde **fiyat alarmı**
  ve JSON dışa/içe aktarım, tema + yenileme aralığı + spread eşiği ayarları, kaynak durumu,
  Hakkında (İdris KATAR + veri kaynakları + MIT). Tüm detaylar genişletilebilir kartlarda.
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

### APK neden kurulamıyordu?
`cargo tauri android build` varsayılan olarak **imzasız** APK üretir; Android ise
imzasız APK'yı yüklemeyi reddeder (`INSTALL_PARSE_FAILED_NO_CERTIFICATES`).
"İmzasız ama kurulabilir" diye bir şey yoktur — en azından debug anahtarıyla
imzalanmış olması gerekir.

**CI çözümü (`Build Mobile` workflow'u bunu yapar):** geçici bir debug keystore
üretir, `zipalign` + `apksigner` ile APK'yı imzalar ve doğrular. Secret gerekmez.
Artifact adı: **`android-install-apk`** → `Finans-Takipcisi-1.0.0-debug-signed.apk`.
Telefonda "Bilinmeyen kaynaklardan yükleme" iznini açıp bu APK'yı kurabilirsiniz.

**Yerel imzalama (opsiyonel):**
```bash
# Debug keystore zaten varsa: ~/.android/debug.keystore (şifre: android, alias: androiddebugkey)
keytool -genkeypair -v -keystore ~/.android/debug.keystore \
  -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 \
  -storepass android -keypass android -dname "CN=Android Debug,O=Android,C=US"
```
Sonra `gen/android/keystore.properties` oluşturun:
```
storeFile=/home/<kullanici>/.android/debug.keystore
storePassword=android
keyAlias=androiddebugkey
keyPassword=android
```
veya CI'daki gibi `apksigner sign --ks ~/.android/debug.keystore ...` kullanın.

⚠️ Debug anahtarıyla imzalanmış APK ile yayın anahtarlı sürüm aynı paket adında
yüklenemez; güncellemede önce mevcut uygulamayı kaldırın. Play Store için kendi
upload keystore'ınızı ve secret'ları kullanın.

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
4. Durum çubuğu: `viewport-fit=cover` + `env(safe-area-inset-top/bottom)` ölçümüyle
   başlık ve alt navigasyon Android bildirim ikonları / gesture bar ile çakışmaz
   (`--safe-top` / `--safe-bottom`; env 0 döndürürse Android için 28px yedek).
4. UI polishing: pull-to-refresh, büyük dokunma alanları, güncelleme animasyonu.
5. Favori aktarımı ve fiyat alarmı: masaüstündeki mantık `core`'a taşınabilir / mobil UI'a eklenebilir.

## Lisans / kimlik

Geliştirici **İdris KATAR** · **MIT** · uygulama adı **Finans Takipçisi**.
Mobil identifier masüstünden ayrı: `com.idriskatar.finans-takipcisi.mobile`.
