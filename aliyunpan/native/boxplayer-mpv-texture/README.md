# boxplayer-mpv-texture

macOS-only N-API addon scaffold for BoxPlayer embedded MPV.

Current status:

- Builds a `.node` addon shape for the Electron 40 `sharedTexture` route.
- Exports `mpvTexture`, matching `electron/main/mpv/embeddedMpvNativeAddon.ts`.
- Routes the N-API entrypoint through `src/mpv_context.{h,cpp}` so the real libmpv backend can replace the current stub without changing the JavaScript API.
- Adds a small `IOSurfaceTexture` render-target wrapper so the future `mpv_render_context` path has a stable texture handle boundary.
- Links Objective-C++ translation units against `IOSurface` to validate the native toolchain path.
- The opt-in libmpv backend creates and controls an mpv context, and passes load options such as start position and HTTP headers into `loadfile`.
- The opt-in libmpv backend exposes non-blocking event polling so Electron can read mpv status without calling JavaScript from libmpv-owned threads.
- The opt-in libmpv backend creates a libmpv render context and renders frames into IOSurface-backed OpenGL FBOs for Electron `sharedTexture`.
- The default stub backend has been verified to build into `build/Release/boxplayer-mpv-texture.node` and export the expected JavaScript API shape.
- `mpvTexture.load()` currently rejects with `boxplayer-mpv-texture libmpv backend is not implemented yet`.

Expected packaged output:

```text
static/engine/darwin/arm64/mpv-texture/
  boxplayer-mpv-texture.node
  libmpv.dylib
  *.dylib
```

Manual build sketch:

```bash
cd native/boxplayer-mpv-texture
pnpm install
pnpm run build
```

The default build uses `src/mpv_context_stub.cpp` and does not link libmpv.

When node-gyp cannot write to the default user cache, point it at a writable devdir and local Node headers:

```bash
NODE_PATH=/path/to/root/node_modules/.pnpm/node-addon-api@5.1.0/node_modules \
npm_config_devdir=/private/tmp/boxplayer-node-gyp-cache \
NODE_DIR=/path/to/node \
pnpm run build:libmpv
```

The package scripts call `scripts/node-gyp-rebuild.sh`, which uses the root
repo's pinned `node-gyp@12.4.0` and defaults to `$HOME/.hermes/node` for Node
headers. Override `NODE_GYP_BIN` or `NODE_DIR` only when needed.

To sync a local libmpv SDK into the project-local dependency layout:

```bash
cd native/boxplayer-mpv-texture
LIBMPV_PREFIX=/opt/homebrew pnpm run sync:mpv:mac
```

This creates the sbtlTV-style local layout:

```text
deps/mpv/
  include/mpv/*.h
  macos/libmpv.dylib
```

`LIBMPV_PREFIX` must contain `include/mpv/client.h`, `include/mpv/render.h`,
`include/mpv/render_gl.h`, and a libmpv dylib. The source may be Homebrew for
development or a custom lower-deployment-target libmpv build for release.

To build the libmpv-backed context from the project-local deps:

```bash
cd native/boxplayer-mpv-texture
pnpm run check:libmpv
pnpm run build:libmpv
```

The native package defaults to `deps/mpv` when `npm_config_libmpv_prefix` is
not set. Override `npm_config_libmpv_prefix` and `npm_config_libmpv_lib_dir`
only when testing an external SDK.

Manual bundle sketch:

```bash
cd native/boxplayer-mpv-texture
ARCH=arm64 pnpm run bundle:mac
```

The bundler:

- copies `build/Release/boxplayer-mpv-texture.node`;
- copies local `libmpv.dylib` from `deps/mpv/macos`, Homebrew, or `LIBMPV_SOURCE`;
- recursively copies non-system dylib dependencies;
- rewrites load commands to `@loader_path/...`;
- ad-hoc signs copied `.node` and `.dylib` files for local development;
- writes `static/engine/darwin/<arch>/mpv-texture/mpv-bundle-manifest.json` with sha256 checksums and source notes.

Do not wire this package into the root install/build pipeline until packaged
Electron playback has been verified. The root Electron app must keep running
without this addon.

Development loader smoke:

```bash
cd native/boxplayer-mpv-texture
scripts/install-dev-stub.sh
```

This copies the default stub `.node` to `static/engine/darwin/<arch>/mpv-texture/boxplayer-mpv-texture.node`, the same dev resource path used by `electron/main/mpv/embeddedMpvNativeAddon.ts`. The copied stub is gitignored because it cannot render video.

Next implementation step:

1. Verify packaged Electron can load the `.node` and dylibs outside the Codex sandbox.
2. Play a local MP4 through the macOS embedded MPV surface.
3. Validate Dolby Vision Profile 5 samples.
4. Replace the Homebrew-sourced deps with a custom lower-deployment-target libmpv build before release.
