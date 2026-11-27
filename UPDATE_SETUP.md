# Otomatik güncelleme kurulumu

Repo: **https://github.com/idriskatar/Finans-Takipcisi**

Masaüstü uygulaması GitHub Releases üzerinden **imzalı** güncelleme alır.
Mobil uygulama GitHub Releases'ı kontrol eder ve yeni APK'yı tarayıcıda açar
(Play Store kullanılmaz; kurulum onayı kullanıcıda kalır).

## 1. GitHub Secrets (zorunlu)

Repo → **Settings → Secrets and variables → Actions → New repository secret**:

| Secret | Değer |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | Görüşmede paylaşılan özel anahtar (uzun base64 metin) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | `finans` |

> Özel anahtar **asla** depoya eklenmez. Zip içinde de yoktur.
> Daha önce eklediyseniz **eski `TAURI_SIGNING_PRIVATE_KEY` secret'ını yeni değerle
> güncelleyin** ve şifreyi `finans` yapın. Eski anahtar base64 çözümleme hatası veriyordu.
>
> Workflow secret değerindeki boşluk/tırnak/satır sonunu temizleyip dosyaya yazıyor
> (beklenen uzunluk **348 karakter**). Logda `anahtar uzunluğu: 348` görmelisiniz.
> Farklı bir sayı görürseniz secret değeri yanlış kopyalanmış demektir.

Genel anahtar `desktop/src-tauri/tauri.conf.json` içinde zaten tanımlı; değiştirmeyin.

## 2. Masaüstü akışı

- `cargo tauri build` sırasında Tauri, paketleri özel anahtarla imzalar ve
  `*.sig` dosyalarını üretir (`bundle.createUpdaterArtifacts: true`).
- `Build Desktop` workflow'u paketleri Actions Artifacts'a yükler.
- `release` job'u `scripts/generate-latest.mjs` ile **`latest.json`** üretir ve
  taslak Release'e ekler.
- Uygulama açılışta şu adresi kontrol eder:
  ```
  https://github.com/idriskatar/Finans-Takipcisi/releases/latest/download/latest.json
  ```
- Yeni sürüm varsa Tauri'nin yerel güncelleme penceresi çıkar; onayla birlikte
  indirir ve kurar.

İlk sürümde secret'lar tanımlı değilse `.sig` dosyaları oluşmaz, `latest.json`
boş platform listesiyle yayınlanır ve güncelleme çalışmaz. Secret'ları ekleyip
yeniden çalıştırmak yeterlidir.

## 3. Mobil akışı

- Uygulama açılışta ve 6 saatte bir GitHub API'sini çağırır
  (`check_update` invoke komutu).
- Yeni sürüm varsa üstte **"Yeni sürüm X hazır — İndir"** şeridi görünür.
- **İndir** düğmesi `open_url` komutuyla APK bağlantısını sistem tarayıcısında açar;
  kullanıcı indirip kurar.
- `tauri-plugin-opener` + `opener:default` izni kullanılır. Play Store gerekmez.

Bu yöntem "sessiz kurulum" değildir. Android'de sessiz kurulum için
`REQUEST_INSTALL_PACKAGES` izni + FileProvider + native Intent köprüsü gerekir;
bu ayrı bir iş olarak eklenebilir.

## 4. Sürüm yayınlama

```bash
git tag v1.6.0
git push origin v1.6.0
```

- **Build Desktop** → taslak Release (exe/dmg/AppImage + `latest.json`)
- **Build Mobile** → `android-install-apk` artifact'i (imzalı APK)

Sürüm **yayınlanır** (`draft: false`); aksi halde `releases/latest/download/...`
linkleri çalışmaz. Paket adları sürümden bağımsızdır:
`Finans-Takipcisi-Windows-setup.exe`, `Finans-Takipcisi-Android.apk` — böylece
README'deki `latest/download` linkleri her sürümde geçerli kalır.
