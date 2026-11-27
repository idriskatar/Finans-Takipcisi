# Değişiklik kaydı

## 1.6.3 (2026-11-27)

### Düzeltildi
- Mobil: yeni sürüm yokken de üstte görünen "İndir" şeridi kaldırıldı. `hidden` niteliği artık `display` atayan sınıflar tarafından geçersiz kılınmıyor; sürüm güncel olduğunda, kontrol başarısız olduğunda veya APK bağlantısı bulunmadığında şerit kapalı kalıyor.

## 1.6.2 (2026-11-27)

### Değiştirildi
- Ekrana sığan sabit üst bar ve kayan barlar artık ortalanıyor; sola dayalı kalma giderildi.
- Dikey kaydırma çubuğu varsayılan olarak gizli; imleç sağ kenara geldiğinde veya sayfa kaydırıldığında belirip kısa süre sonra yeniden kayboluyor. Çubuk görünürken sayfa genişliği değişmiyor.

### Düzeltildi
- Üst barda sığma ölçümü yalnızca kayar alanı değil, sabit endeks kartı dahil tüm satırı baz alıyor.

## 1.6.1 (2026-11-27)

### Değiştirildi
- Sabit bar: BIST 100 endeksi her zaman sabit kalır; döviz ve altın kartları yalnızca ekrana sığmadığında kayar hale gelir.
- Diğer kayan barlar (yükselen/düşen hisseler, sarrafiye, kripto): içerik ekrana sığıyorsa sabit, sığmıyorsa kayar; karar pencere boyutu değiştiğinde yeniden hesaplanır ve kaydırma hızı içerik genişliğine göre ayarlanır.

## 1.6.0 (2026-11-27)

### Eklendi
- GitHub Releases üzerinden otomatik güncelleme: masaüstünde imzalı Tauri updater
  (`latest.json`), mobilde sürüm kontrolü + APK indirme şeridi.

### Değiştirildi
- Sürüm tüm platformlarda **1.6.0** olarak hizalandı.

## 1.1.0 (2026-11-25)

### Eklendi
- **Kaynak durumu paneli** (Ayarlar): Harem/TCMB/BIST/Kripto için bağlantı durumu,
  son veri saati ve hata mesajı; disk önbelleğinden gelen veriler "son kayıt" olarak işaretli.
- **Disk önbelleği**: son başarılı veri `app_data_dir/cache/*.json` içine yazılır; açılışta
  ağdan önce yüklenir, çevrimdışıyken de ekran dolu kalır.
- **Favori aktarımı**: Favoriler sekmesinde Dışa aktar / İçe aktar düğmeleri (JSON).
- **Fiyat alarmı**: her favori için eşik fiyat ve yön (↑/↓); eşik aşılınca uygulama içi
  bildirim, 5 dk aynı alarm tekrarı engellenir.
- **Yeni uygulama simgesi**; pencere başlık çubuğuna da açıkça uygulandı (`image-png`).

## 1.0.0 (2026-11-23)

### Eklendi
- Rust + Tauri v2 çok platformlu masaüstü uygulaması (`desktop/`)
- Dört veri kaynağının Rust portu: Harem Socket.IO, TCMB XML, BIST 100/Yahoo, Binance + BtcTurk
- Gömülü axum HTTP sunucusu ve SSE akışları
- Favoriler sekmesi ve tüm tablolara yıldız (☆/★) düğmeleri, kalıcı seçim
- Açık / koyu / sistem tema desteği
- Ayarlar sekmesi (tema, yenileme aralıkları, spread eşiği)
- Hakkında sekmesi (İdris KATAR, veri kaynakları, MIT)
- Üç durumlu sütun sıralaması (azalan → artan → sıralamasız)
- MIT lisansı, GitHub Actions çok platformlu derleme iş akışı, katkı ve kurulum rehberleri
- Windows NSIS kurulum paketi derleme betiği

### Düzeltildi
- Uygulama adı **Finans Takipçisi** olarak değiştirildi; Harem yalnızca kaynak olarak kaldı
- Kullanıcının sağladığı coin görseli uygulama simgesi yapıldı
- Ayarlar ve favoriler artık uygulama veri klasöründeki `user-data.json` dosyasına
  kaydediliyor; rastgele port kaynaklı `localStorage` kaybı giderildi, sabit port kullanılıyor
- TCMB/BIST verisi gelmediğinde otomatik yeniden deneme eklendi: hata/boş yanıt "başarılı"
  sayılmıyor; arayüz 5 sn'de bir, sunucu 5 sn'de bir tekrar deniyor, sekme açılınca da
  hemen yenileniyor. "Yenile" tuşuna basma ihtiyacı kalktı.
