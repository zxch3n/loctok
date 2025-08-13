#!/usr/bin/env node
const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');

function getBinaryPath() {
  const vendor = path.join(__dirname, '..', 'vendor');
  const exe = process.platform === 'win32' ? 'loctok.exe' : 'loctok';
  return path.join(vendor, exe);
}

function run() {
  const bin = getBinaryPath();
  if (!fs.existsSync(bin)) {
    console.error('[loctok] binary not found.');
    console.error('Tried:', bin);
    console.error('If install failed, try again:');
    console.error('  npm i loctok -g  # or use npx again');
    console.error('Or install from source: cargo install loctok');
    process.exit(1);
  }

  const child = spawn(bin, process.argv.slice(2), {
    stdio: 'inherit',
  });
  child.on('close', (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
    } else {
      process.exit(code);
    }
  });
}

run();

