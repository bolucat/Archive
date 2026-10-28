# Changelog

Generated from the commit history by [git-cliff](https://git-cliff.org).

## [0.1.5](https://github.com/libnyanpasu/enable-loopback/releases/tag/v0.1.5) - 2026-10-28

- Start on Windows 10 by bootstrapping the framework without Windows 11-only imports ([2d403f6](https://github.com/libnyanpasu/enable-loopback/commit/2d403f6eabb03b4637278bae098da4753325b2a5))
- Publish the Windows App SDK bootstrapper with each release ([a8e4146](https://github.com/libnyanpasu/enable-loopback/commit/a8e41461c23806106addecec161d1006141c0404))

## [0.1.4](https://github.com/libnyanpasu/enable-loopback/releases/tag/v0.1.4) - 2026-10-22

- License the project under MIT ([a68bbb1](https://github.com/libnyanpasu/enable-loopback/commit/a68bbb15b9667dafabcacd728e62996cd1bd205f))
- Center the window on the monitor under the cursor ([be9e97f](https://github.com/libnyanpasu/enable-loopback/commit/be9e97fcbeb869f0f9e74b667c289c597a48f07d))
- Start the Binaries column at 200 DIP ([4a23be3](https://github.com/libnyanpasu/enable-loopback/commit/4a23be33a9e6fca898f25331ddfd3bc816498582))
- Scroll the table in its list so both scroll bars stay reachable ([b676214](https://github.com/libnyanpasu/enable-loopback/commit/b6762145a0785cc526fbec5aa8c47d56266d2369))
- Let rows follow a column resize while dragging ([3191842](https://github.com/libnyanpasu/enable-loopback/commit/31918424f02562b8366b7bd50302fc4ad601f31d))
- Keep the table's scroll position when its rows are replaced ([f6adeec](https://github.com/libnyanpasu/enable-loopback/commit/f6adeec373fb328cc191c83ea0c22b76e2afa822))
- Build table rows only near the list's viewport ([5174e44](https://github.com/libnyanpasu/enable-loopback/commit/5174e443aa44b3f7ca69e3d7048ae98fef82f354))
- Create cell tooltips only for the hovered row ([21a257a](https://github.com/libnyanpasu/enable-loopback/commit/21a257af34f4552f15d1eb9d42be4bc3a741311b))
- Generate the changelog and release notes with git-cliff ([a7190f5](https://github.com/libnyanpasu/enable-loopback/commit/a7190f5aa8a972ffbc084bdf7bec1524f33ef691))

## [0.1.3](https://github.com/libnyanpasu/enable-loopback/releases/tag/v0.1.3) - 2026-10-21

- Link the C runtime statically ([7fdcd62](https://github.com/libnyanpasu/enable-loopback/commit/7fdcd62d260f51c008e3df630f9d377d69144522))
- Offer to install Windows App Runtime from the Microsoft Store ([ab92f4e](https://github.com/libnyanpasu/enable-loopback/commit/ab92f4e88df86b4d3b28c8508129611f8bd57c92))
- Opt the executable into the segment heap ([1a8de54](https://github.com/libnyanpasu/enable-loopback/commit/1a8de54613f35964f942563510c49f3048f75878))

## [0.1.2](https://github.com/libnyanpasu/enable-loopback/releases/tag/v0.1.2) - 2026-10-21

- Perf reduce release executable size ([#3](https://github.com/libnyanpasu/enable-loopback/pull/3)) ([71f9cf3](https://github.com/libnyanpasu/enable-loopback/commit/71f9cf3ecf8c6bf93029ad0a0b4b1bb54fb54729))

## [0.1.1](https://github.com/libnyanpasu/enable-loopback/releases/tag/v0.1.1) - 2026-10-20

- Open the row context menu on right-button release ([71c28de](https://github.com/libnyanpasu/enable-loopback/commit/71c28de866e1eb7869ccf9bad3ebde943d86363b))

## [0.1.0](https://github.com/libnyanpasu/enable-loopback/releases/tag/v0.1.0) - 2026-10-20

- Scaffold the localized WinUI loopback tool ([4ddb6d0](https://github.com/libnyanpasu/enable-loopback/commit/4ddb6d08b428ef2c9367c6c79fa19f2adb1c958e))
- Check formatting, Clippy, and tests on Windows CI ([f463b63](https://github.com/libnyanpasu/enable-loopback/commit/f463b637202807d146f2e796389fcaed9c3713b2))
- Implement owned NetIso FFI and registry enumeration ([b2e28d3](https://github.com/libnyanpasu/enable-loopback/commit/b2e28d30c796afe708d7615030f1a13fccaaed5e))
- Implement pure list rendering and exemption rules ([dfb38bd](https://github.com/libnyanpasu/enable-loopback/commit/dfb38bd8af0d4b02b2201ea2ea325ca6226dbc80))
- Add SID-based list selection state ([a727edb](https://github.com/libnyanpasu/enable-loopback/commit/a727edb8bed4418d581b7db7ad5c9d4afecd8487))
- Execute CLI exemption commands without WinAppSDK ([69d3c7b](https://github.com/libnyanpasu/enable-loopback/commit/69d3c7be905fbbf0904d9e37fee97b0ce219eb38))
- Enforce GUI single instance and publish its window handle ([b5e6603](https://github.com/libnyanpasu/enable-loopback/commit/b5e66035a075d0c777467c550e5a9e4fc0c4cccd))
- Build the AppContainer list and asynchronous refresh flow ([89cb09f](https://github.com/libnyanpasu/enable-loopback/commit/89cb09f47789987635bd44fe4171bc1b3b05cfc4))
- Wire row selection and context menu clipboard actions ([92c0e69](https://github.com/libnyanpasu/enable-loopback/commit/92c0e69ad6e1ce3abb4820c988dc6bec8929913b))
- Save checked exemptions without losing failed edits ([8a30beb](https://github.com/libnyanpasu/enable-loopback/commit/8a30beb81576e7737ac3ff990d86c3ffdd89f95a))
- Finish close confirmation, keyboard shortcuts and help ([f408ba7](https://github.com/libnyanpasu/enable-loopback/commit/f408ba7b34d4fc6b142bf7bf7b016d54aa38048b))
- Publish x64 and arm64 executables with checksums on tags ([88633e9](https://github.com/libnyanpasu/enable-loopback/commit/88633e9d85b320a8671dfc57aa95a5f2567af093))
- Build against the vendored libnyanpasu windows-rs fork ([725bd80](https://github.com/libnyanpasu/enable-loopback/commit/725bd80be02c1520b29f90aec54fa00d6fb5df80))
- Rework the window around a TableView component with search ([94fad55](https://github.com/libnyanpasu/enable-loopback/commit/94fad55c5e1734e8d64515f03448f4dc9a361d49))
- Mark the design as implemented ([9f87f6a](https://github.com/libnyanpasu/enable-loopback/commit/9f87f6aa86cf76b61d311ccae19c67f90821857e))

