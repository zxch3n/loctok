#!/usr/bin/env node
/*
  Downloads the prebuilt loctok binary for the current platform
  into vendor/ and makes it executable. Used by npm postinstall.
*/
const fs = require('fs');
const path = require('path');
const https = require('https');

const REPO = 'zxch3n/loctok';

function pkgVersion() {
  const pkg = require('../package.json');
  return pkg.version;
}

function detectTarget() {
  const { platform, arch } = process;
  if (platform === 'darwin') {
    if (arch === 'arm64') return 'aarch64-apple-darwin';
    if (arch === 'x64') return 'x86_64-apple-darwin';
  } else if (platform === 'linux') {
    if (arch === 'x64') return 'x86_64-unknown-linux-gnu';
    if (arch === 'arm64') return 'aarch64-unknown-linux-gnu';
  } else if (platform === 'win32') {
    if (arch === 'x64') return 'x86_64-pc-windows-msvc';
  }
  return null;
}

function makeUrl(version, target) {
  // Prefer mirror if provided, else GitHub Releases.
  const base = process.env.LOCTOK_DOWNLOAD_BASE || `https://github.com/${REPO}/releases/download`;
  const filename = process.platform === 'win32'
    ? `loctok-v${version}-${target}.exe`
    : `loctok-v${version}-${target}`;
  return `${base}/v${version}/${filename}`;
}

function ensureDir(p) {
  fs.mkdirSync(p, { recursive: true });
}

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const doReq = (u, redirectsLeft = 5) => {
      const req = https.get(u, { headers: { 'User-Agent': 'loctok-installer' } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          if (redirectsLeft === 0) return reject(new Error('Too many redirects'));
          return doReq(res.headers.location, redirectsLeft - 1);
        }
        if (res.statusCode !== 200) {
          return reject(new Error(`HTTP ${res.statusCode} for ${u}`));
        }
        const file = fs.createWriteStream(dest);
        res.pipe(file);
        file.on('finish', () => file.close(resolve));
        file.on('error', reject);
      });
      req.on('error', reject);
    };
    doReq(url);
  });
}

async function main() {
  const version = pkgVersion();
  const target = detectTarget();
  if (!target) {
    console.error(`[loctok] Unsupported platform: ${process.platform} ${process.arch}`);
    console.error('Please install from source: cargo install loctok');
    process.exit(1);
  }

  const url = makeUrl(version, target);
  const vendor = path.join(__dirname, '..', 'vendor');
  ensureDir(vendor);
  const exeName = process.platform === 'win32' ? 'loctok.exe' : 'loctok';
  const dest = path.join(vendor, exeName);

  console.log(`[loctok] downloading ${url}`);
  try {
    await download(url, dest);
  } catch (err) {
    console.error('[loctok] download failed:', err.message);
    console.error('You can try again or install from source: cargo install loctok');
    process.exit(1);
  }

  if (process.platform !== 'win32') {
    try {
      fs.chmodSync(dest, 0o755);
    } catch {}
  }
  console.log('[loctok] installed to', dest);
}

main().catch((e) => {
  console.error('[loctok] install error:', e);
  process.exit(1);
});
