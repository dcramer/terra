# Our tools in a browser

2026-10-12. Question: can the sandbox's tools, starting with the model workbench, run in a web browser on the game's own renderer, opened and controlled by a link? The developers want many testing tools and would like them web-accessible, ideally as a separate app that shares the game's modules and loads what a link names.

## Answer

Yes, and it is a few days' work, not a rewrite. Bevy 0.19.1, our pinned version, builds for the web (wasm32) and draws through WebGL2 or, still experimentally, WebGPU. Our shared crates are already web-clean, and nothing we depend on has native code. What stands in the way is ours: a handful of places that read files straight from disk, use threads, or read the native clock, all known. No build was tried; the wasm target is not installed yet.

The separate app already exists: it is the sandbox, which is a different binary from the game that uses the same modules, with its tools as experiments chosen on the command line. A web build is the sandbox built for another target, its command-line options read from the page's address instead: `?experiment=workbench&prop=Forge&hold=stone_pickaxe` is `--experiment workbench --prop Forge --hold stone_pickaxe`. Every tool gets a link, and no tool is written twice.

## What the pinned Bevy provides

- Features `web`, `webgl2` and `webgpu` (`bevy-0.19.1/Cargo.toml`); its examples document building with wasm-bindgen and ship a page to host one (`examples/wasm/`).
- On the web, assets load over HTTP from the page's server (`bevy_asset`'s `HttpWasmAssetReader`), so everything that already goes through the `AssetServer`, such as the workbench's own models, works unchanged.
- Audio, the inspector (egui) and timing have web backends. Our shaders use nothing WebGL2 lacks (no storage buffers, no compute).

## What stands in the way, cheapest first

1. **Features.** `file_watcher` (hot reload) and the remote protocol's HTTP server do not compile for the web; both become native-only.
2. **The clock.** `std::time::Instant` panics in a browser; `debug.rs`, `shell.rs` and `forest.rs` switch to `bevy::platform::time::Instant`.
3. **Native-only extras**, cfg'd out on the web: remote HTTP, saves, saved settings, probe traces, `--shot`/`--sheet` files (or made downloads), the icon baker, the data files' hot reload.
4. **Options from the address.** The page's query string becomes the argument list `Options` already parses.
5. **Threads.** `terrain::first_view` and flora use `std::thread::scope`; on the web they run on Bevy's task pool, which runs in place there. First meshing is slower.
6. **Reading files.** The one real design choice. `models::read_model` (every prop, plant and the arrow) and `data::read` (the seven registries) read the disk synchronously while the app is built. Either a small `files::read` that reads the disk natively and, on the web, a table filled by fetching those files before the app starts; or proper asset loaders behind the loading screen, which is cleaner and the industry's way, and also what streaming many worlds will want (D127).
7. **Tooling.** The wasm target, `wasm-bindgen-cli` at the lockfile's version, a page, and a static server for `assets/` (27 MB): a `cargo xtask web` would build it and serve it.

## Agents

The native loop agents use (`--remote` over HTTP, `--sheet` writing pictures, `SANDBOX` lines on stdout) does not exist in a browser. It does not need to: an agent drives a page with agent-browser or Playwright, reads it through a small JavaScript interface the app exposes, and screenshots it, as it already does with the [model viewer](../tools/models/README.md) (`window.viewer`). The remote protocol's methods could answer through that interface instead of HTTP. Native stays the place to measure performance (AGENTS.md).

## Until then

[tools/models](../tools/models/README.md) is a three.js viewer for any model, controlled by its address, built the same day. It shows shape, size, orientation and colour regions, not the game's exact look; the web build would replace it for looks and keep the link convention.
