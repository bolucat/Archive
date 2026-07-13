# macOS arm64 MPV texture runtime

This directory is the expected development and packaged resource location for the macOS embedded MPV native addon.

Expected final files:

```text
boxplayer-mpv-texture.node
libmpv.dylib
*.dylib
mpv-bundle-manifest.json
```

During development, `native/boxplayer-mpv-texture/scripts/install-dev-stub.sh` can copy the default stub `.node` here after a local node-gyp build. That stub only validates the Electron/native loader path and does not render video.
