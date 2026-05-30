# auntiejane

A cross-platform desktop application built with [Tauri 2](https://tauri.app) and a
[SvelteKit](https://svelte.dev/docs/kit) + TypeScript frontend, with a Rust backend.

The frontend runs as a single-page app: SvelteKit's
[`adapter-static`](https://svelte.dev/docs/kit/adapter-static) prerenders to plain
files that Tauri serves in the native webview — there is no Node.js server at runtime.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
- [Bun](https://bun.com)
- Platform-specific Tauri dependencies — see the
  [Tauri prerequisites guide](https://tauri.app/start/prerequisites/)

## Getting started

Install the frontend dependencies:

```sh
bun install
```

Run the app in development mode (hot-reloading frontend, native window):

```sh
bun run tauri dev
```

## Building

Produce an optimized, bundled application for the current platform:

```sh
bun run tauri build
```

## Type checking

Validate the Svelte + TypeScript frontend without building:

```sh
bun run check
```

## Project layout

```
.
├── src/                # SvelteKit + TypeScript frontend
│   ├── app.html        # HTML shell
│   └── routes/         # Pages and layouts
├── static/             # Static assets served as-is
├── src-tauri/          # Rust backend
│   ├── src/
│   │   ├── lib.rs      # App setup and commands
│   │   └── main.rs     # Binary entry point
│   ├── capabilities/   # Permission definitions
│   ├── tauri.conf.json # Tauri configuration
│   └── Cargo.toml
├── svelte.config.js
└── package.json
```

The frontend and backend communicate through Tauri's
[command](https://tauri.app/develop/calling-rust/) and
[event](https://tauri.app/develop/calling-frontend/) systems.
