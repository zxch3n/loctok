# loctok (npm)

Run the Rust-powered `loctok` CLI via npm.

## Quick start

```bash
npx loctok            # run without installing
# or
npm i -g loctok       # install globally
```

The package downloads a prebuilt binary from GitHub Releases during install.

## Environment

- `LOCTOK_DOWNLOAD_BASE`: override the download host (for mirrors),
  defaults to `https://github.com/zxch3n/loctok/releases/download`.

If your platform is not supported or the download fails, install from source:

```bash
cargo install loctok
```
