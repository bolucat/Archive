# boxplayer-mpv-texture

N-API/libmpv renderer for BoxPlayer. The renderers below are implemented in source;
Windows/Linux GPU playback still requires target-machine acceptance. Compiling a
backend, or falling back successfully, does not verify that its GPU path works.

| Platform | Preferred renderer | Fallback | Runtime |
| --- | --- | --- | --- |
| macOS x64 / arm64 | OpenGL FBO backed by IOSurface | GPU readback, then software rendering if initialization fails | `libmpv.dylib` |
| Windows x64 | OpenGL/D3D11 interop, DXGI NT shared texture | GPU readback or software rendering | `libmpv-2.dll` |
| Linux x64 / arm64 | EGL texture exported as DMA-BUF | GPU readback or software rendering | `libmpv.so.2` |

Windows arm64 is not currently supported. Windows probes adapters against the WGL
context instead of choosing by vendor name. Unsupported interop falls back. Linux
requires EGL/OpenGL development headers at build time and compatible runtime
drivers. It probes `EGL_MESA_image_dma_buf_export` and accepts packed RGBA/BGRA
exports only. GPU import failure in Electron requests CPU readback.

`BOXPLAYER_MPV_RENDERER=software` forces software rendering for diagnostics.
Software rendering uses a bounded buffer pool, a stable maximum 1280×720 target,
and copy-back decoding. It remains a compatibility path, not full-resolution
4K presentation. GPU rendering uses source dimensions. Viewport-adaptive software
resolution and asynchronous GPU fences are follow-up performance work.

## Ownership and threading

- `create()` and `destroy()` return promises. Always await them. Initialize the
  render context, render frames and release GL/EGL resources on the same native
  render thread. Electron's main thread must not wait for network or thread joins.
- Playback commands use asynchronous libmpv APIs. Track/status queries read
  event-maintained caches. External track commands resolve on `COMMAND_REPLY`,
  are cancelled on a new load/destroy and have a 15-second deadline.
- Render code does not acquire a lock held by a synchronous client API call.
  Loading a remote subtitle must never suspend video rendering.
- Exported GPU frames carry an idempotent JS `release()` lease. Retain it until
  Electron's `allReferencesReleased` callback. Release dropped/unimported frames
  immediately. Occupied slots cannot be overwritten; a saturated pool drops frames.
- Resizing deletes obsolete GL objects, while outstanding leases retain the
  underlying IOSurface/D3D/DMA-BUF resource. There is no unbounded retired pool.
- GPU exporters currently use a conservative completion fence. This avoids
  producer/consumer races, but it is not a claim of an entirely asynchronous or
  end-to-end zero-copy pipeline.

## Linux process boundary

libmpv stays in `mpv-node-host` to avoid Chromium/FFmpeg symbol collisions.
`mpv_transport.node` contains only the Unix descriptor transport; it does not link
libmpv and is safe to load in Electron. The host passes DMA-BUF descriptors through
`SCM_RIGHTS` on a private local socket, rather than sending process-local fd numbers
through Node IPC. Frame metadata travels with the descriptors. Electron duplicates
received descriptors during import; our receiver retains its originals until the
frame lease is released and then acknowledges the producing host.

Every child and request belongs to one host session. An old exit/message cannot
clear a replacement child or reject its requests. Commands and startup are bounded.
Ordinary status updates are event-driven; track lists are cached, not polled from
libmpv on every UI refresh.

## Build

Use pnpm and an architecture-matched SDK. On Windows, provide
`deps/mpv/win32/x64/mpv.lib` and `libmpv-2.dll`. Linux uses
`deps/mpv/linux/<arch>/libmpv.so`. The non-macOS default build is a stub;
`build:libmpv` enables the real GPU renderer and its software fallback.

```bash
# macOS: use a supported-system SDK; do not ship newer Homebrew dylibs to older macOS.
LIBMPV_PREFIX=/opt/homebrew pnpm --dir native/boxplayer-mpv-texture run sync:mpv:mac
pnpm --dir native/boxplayer-mpv-texture run build:libmpv
pnpm --dir native/boxplayer-mpv-texture run bundle:mac
pnpm --dir native/boxplayer-mpv-texture run check:mac-minos

# Native Windows x64 (MSYS2 CLANG64 source build) or Linux x64/arm64:
pnpm --dir native/boxplayer-mpv-texture run source:win:x64
# OR: pnpm --dir native/boxplayer-mpv-texture run source:linux
pnpm --dir native/boxplayer-mpv-texture run build:libmpv
pnpm --dir native/boxplayer-mpv-texture run bundle:software
```

The legacy `bundle:software` script name now packages both render paths. Linux
bundles must include `mpv_transport.node`, `mpv-node-host` and `mpv-host.cjs` as well
as libmpv and the rendering addon. Release packaging downloads both Linux
architectures. Candidate workflows build from pinned upstream mpv source and keep
platform acceptance separate from compilation.

## Verification

These native checks do not launch Electron or access a user profile:

```bash
node native/boxplayer-mpv-texture/scripts/smoke-control.mjs
node native/boxplayer-mpv-texture/scripts/smoke-playback.mjs
node native/boxplayer-mpv-texture/scripts/smoke-lifecycle.mjs
# Linux descriptor transport:
node native/boxplayer-mpv-texture/scripts/smoke-transport.cjs
```

Use `BOXPLAYER_MPV_AUDIO_OUTPUT=null` for checks that should not produce sound.
`smoke-lifecycle` holds a local subtitle response and checks event-loop responsiveness,
continued frames, bounded texture ownership, cancellation, and close/recreate.
Set `BOXPLAYER_MPV_REQUIRE_GPU=1` to reject software fallback during a GPU acceptance run.
Native checks do not replace visible Electron acceptance. Follow repository
`AGENTS.md`: attach to the user's existing dev app by default; do not launch a new
Electron app or execute production E2E without an explicit E2E request.
