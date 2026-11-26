// GitHub Releases için Tauri updater latest.json dosyasını üretir.
// Kullanım: node scripts/generate-latest.mjs <artifacts-dir> <version> <tag>
import fs from 'node:fs';
import path from 'node:path';

const [dir, version, tag] = process.argv.slice(2);
const repo = process.env.GITHUB_REPOSITORY ?? 'idriskatar/Finans-Takipcisi';
const base = `https://github.com/${repo}/releases/download/${tag}/`;

const files = [];
function walk(d) {
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, e.name);
    if (e.isDirectory()) walk(p);
    else files.push(p);
  }
}
walk(dir);
const byName = (name) => files.find((p) => path.basename(p) === name);

const latest = {
  version,
  notes: `Finans Takipçisi ${version} — yeni sürüm yayınlandı.`,
  pub_date: new Date().toISOString(),
  platforms: {},
};

function add(platform, filename) {
  const sig = byName(filename + '.sig');
  if (!sig) return;
  latest.platforms[platform] = {
    signature: fs.readFileSync(sig, 'utf8').trim(),
    url: base + encodeURIComponent(filename),
  };
}

for (const f of files) {
  const name = path.basename(f);
  if (name.endsWith('.sig')) continue;
  if (/windows|win32/i.test(name) || name.endsWith('-setup.exe')) {
    if (/x86|32-bit/i.test(name)) add('windows-x86', name);
    else add('windows-x86_64', name);
  } else if (name.endsWith('.dmg')) {
    if (/aarch64|arm64|apple/i.test(name)) add('darwin-aarch64', name);
    else if (/x64|x86_64|intel/i.test(name)) add('darwin-x86_64', name);
  } else if (name.endsWith('.AppImage')) {
    add('linux-x86_64', name);
  }
}

if (!Object.keys(latest.platforms).length) {
  console.error('Uyarı: imzalı paket bulunamadı, latest.json boş platform listesiyle yazılacak.');
}
const out = path.join(dir, 'latest.json');
fs.writeFileSync(out, JSON.stringify(latest, null, 2));
console.log('Yazıldı:', out, Object.keys(latest.platforms).join(', '));
