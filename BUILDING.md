# Building the public GUI repository

Publish the contents of `gui/` as the GUI repository root. Its workflow is
`.github/workflows/build.yml` and all required sources, assets, Rust `xtask` builder,
fixtures, Cargo lockfile and Rust toolchain file are inside this repository.

The workflow runs on pushes to `main`, pull requests touching build inputs, and
manual **Run workflow** requests. Each job runs packaging tests, Cargo tests and
Clippy, then builds and uploads an artifact:

| Platform | Runner | GUI package |
|---|---|---|
| Windows x64 | `windows-2025` | `.exe` |
| Linux x64 | `ubuntu-22.04` | `.AppImage` |
| Linux ARM64 | `ubuntu-22.04-arm` | `.AppImage` |
| macOS x64 | `macos-15-intel` | `.dmg` containing `Flacora.app` |
| macOS ARM64 | `macos-15` | `.dmg` containing `Flacora.app` |

Artifacts are named `flacora-gui-<platform>` and retained for 14 days. Download them
from the workflow run. Extension sources and extension signing secrets belong to
the separate extension repository; this workflow builds the GUI from its own files.
It does not publish GitHub Releases. macOS bundles are ad-hoc signed and not notarized.

## Local packaging

Run these commands from this repository root with Rust 1.100.1, as pinned in `rust-toolchain.toml`:

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo xtask
```

The GUI builder is pure Rust (`xtask/`). Windows requires MSVC build tools;
macOS requires Xcode command line tools. Linux releases use Ubuntu 22.04 / glibc
2.35; dependency and AppImage tool installation commands are in the workflow.
Set `APPIMAGETOOL` if the tool is not on `PATH`. `APPIMAGETOOL_RUNTIME_FILE` can
provide a pre-downloaded runtime.

Packages go to `dist/<platform>/`. `--platform <name>` asserts the native target,
`--out <folder>` selects another output directory, and `--skip-build` packages an
existing release binary. Builds use locked dependencies, remapped source paths,
and static MSVC CRT on Windows.
