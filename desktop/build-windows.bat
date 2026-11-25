@echo off
setlocal
cd /d "%~dp0src-tauri"

echo ============================================
echo  Finans Takipcisi - Windows build
echo  Gelistirici: Idris KATAR - MIT Lisansı
echo ============================================

where cargo >nul 2>&1
if errorlevel 1 (
  echo HATA: Rust/cargo bulunamadi. https://rustup.rs adresinden kurun.
  exit /b 1
)

where cargo-tauri >nul 2>&1
if errorlevel 1 (
  echo Tauri CLI kuruluyor...
  cargo install tauri-cli --version "^2"
  if errorlevel 1 exit /b 1
)

echo Derleniyor (NSIS kurulum paketi)...
cargo tauri build --bundles nsis
if errorlevel 1 (
  echo HATA: Derleme basarisiz.
  exit /b 1
)

echo.
echo Tamamlandi. Kurulum paketi:
echo   %cd%\target\release\bundle\nsis\
endlocal
