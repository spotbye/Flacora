# Building Flacora

Requires Rust 1.100.1 (pinned in `rust-toolchain.toml`).

## Local build

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo xtask
```

Packages are written to `dist/<platform>/`:

- **Windows x64**: `.exe` (requires MSVC build tools)
- **Linux x64 / ARM64**: `.AppImage` (Ubuntu 22.04 / glibc 2.35; see [`.github/workflows/build.yml`](.github/workflows/build.yml) for dependencies and `appimagetool`)
- **macOS x64 / ARM64**: `.dmg` containing `Flacora.app` (requires Xcode command line tools; ad-hoc signed, not notarized)

Optional flags: `--platform <name>`, `--out <folder>`, `--skip-build`.

## GitHub Actions

Run [`.github/workflows/build.yml`](.github/workflows/build.yml) manually from **Actions → Run workflow** to build all five targets (`windows-x64`, `linux-x64`, `linux-arm64`, `macos-x64`, `macos-arm64`) and create a draft GitHub Release (`v<version>`) with all five assets attached.
