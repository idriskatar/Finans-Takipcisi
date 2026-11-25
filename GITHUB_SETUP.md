# GitHub'a yükleme (GitHub Desktop ile)

`finans-takipcisi.zip` dosyasını istediğiniz bir klasöre çıkarın. Klasör şu şekilde görünmeli:

```
finans-takipcisi/
├── .github/workflows/   → build-desktop.yml, build-mobile.yml
├── core/                → paylaşılan Rust veri motoru
├── desktop/             → masaüstü uygulaması
├── mobile/              → Android/iOS uygulaması
├── .gitignore  LICENSE  README.md  MOBILE_PLAN.md  CHANGELOG.md
└── ... (eski Node referans dosyaları)
```

Derleme çıktıları (`target/`, `gen/`, `node_modules/`) zip'e dahil **edilmez**; zaten `.gitignore` ile de hariç tutulurlar.

## GitHub Desktop adımları

1. GitHub Desktop → **File > Add Local Repository** → çıkarılan klasörü seçin.
   (Klasörde `.git` yoksa GitHub Desktop "Create a repository" seçeneğini gösterir;
   `git init` zaten yapıldıysa doğrudan açılır.)
2. **Publish repository** → repo adı: `finans-takipcisi` → public/private seçin → **Publish**.
3. İlk commit zaten hazır; "Publish" ile birlikte gönderilir.

Komut satırıyla eşdeğeri:

```bash
cd finans-takipcisi
git remote add origin https://github.com/<kullanici>/finans-takipcisi.git
git branch -M main
git push -u origin main
```

## Otomatik derleme

`v1.0.0` gibi bir etiket gönderdiğinizde iki workflow tetiklenir:

| Workflow | Job'lar | Çıktı |
|---|---|---|
| `Build Desktop` | Windows (NSIS **+ MSI**), macOS ARM/Intel (DMG), Linux (AppImage + deb) | GitHub **Releases** (taslak) **ve** Actions **Artifacts** |
| `Build Mobile` | Android (APK + AAB), iOS (imzasız simulator `.app`) | GitHub **Actions > Artifacts** |

Etiket göndermek:

```bash
git tag v1.0.0
git push origin v1.0.0
```

Ardından **Actions** sekmesinden derlemeleri izleyin. Desktop paketleri **Releases** altında
taslak olarak oluşur (gözden geçirip *Publish release* yapın). Mobil paketler ilgili
workflow sayfasındaki **Artifacts** bölümünden indirilir.

### Notlar

- iOS job'u `macos-latest` üzerinde çalışır ve **imzasız simulator `.app`** üretir
  (`--target aarch64-sim --ci`). Team ID / sertifika olmadan cihaz/ipa derlenemediği için
  bu tercih edildi. İmzalı IPA için `MOBILE_PLAN.md` içindeki Apple secret'larını ekleyin.
- Android job'u Java 17 + runner'ın hazır Android SDK'sını kullanır; `setup-android@v3`
  kaldırıldı (deprecated `tools` paketi hatası).
- Masaüstü paketleri hem Releases'e hem Actions Artifacts'a yüklenir; Windows job'u
  NSIS + MSI üretir.
- İlk derleme Rust bağımlılıkları nedeniyle uzun sürebilir; `swatinem/rust-cache` sonraki
  derlemeleri hızlandırır.

## Yapı

Paylaşılan `core/` sayesinde masaüstü ve mobil aynı veri motorunu kullanır. Yeni özellik
eklerken veri katmanını `core/` içine, platforma özel kodu `desktop/` veya `mobile/` içine
koyun. Ayrıntılar: `README.md`, `MOBILE_PLAN.md`, `CONTRIBUTING.md`.

Lisans: **MIT**, © 2026 İdris KATAR.
