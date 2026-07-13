# macOS embedded MPV native assets

BoxPlayer's macOS embedded MPV route expects native assets under:

```text
static/engine/darwin/<arch>/mpv-texture/
  boxplayer-mpv-texture.node
  libmpv.dylib
  *.dylib
  mpv-bundle-manifest.json
```

At runtime these files are resolved from the packaged resources path:

```text
engine/darwin/<arch>/mpv-texture/boxplayer-mpv-texture.node
```

Notes:

- `<arch>` is `arm64` first; `x64` must be added only after a separate verification pass.
- `boxplayer-mpv-texture.node` must export `mpvTexture`.
- `mpvTexture` must expose `create`, `load`, `destroy`, `onFrame`, `onStatus`, and `onError` at minimum.
- `onFrame` must report IOSurface-backed texture handles compatible with Electron 40 `sharedTexture.importSharedTexture`.
- All dylib install names and transitive dependencies must be rewritten so the packaged app can load them after signing and notarization.
- `mpv-bundle-manifest.json` records source paths, architecture, file sizes, and SHA-256 checksums.

Use `native/boxplayer-mpv-texture/scripts/bundle-dylibs-mac.sh` to prepare this directory from a local native addon build and local Homebrew/libmpv installation. The script does not download binaries.
