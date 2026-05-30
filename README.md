# NanaVault

Encrypted, recoverable file backups on [nostr](https://nostr.com) +
[Blossom](https://github.com/hzrd149/blossom), with social recovery.

> **Status: in development.** The repository is currently a Tauri 2 + SvelteKit
> scaffold. The design and the step-by-step build plan live in
> [`plan.md`](./plan.md). This README describes the intended product and is the
> source of truth for *what* we're building and *why*.

## What it does

NanaVault backs up a single file so that you can get it back even if you lose almost
everything:

1. You provide a nostr secret key (`nsec`) and a password.
2. From those two, the app derives a separate key. This derived key encrypts your file
   and is also the nostr identity that records where the backup lives.
3. The encrypted file is uploaded to one or more Blossom servers.
4. A small, **encrypted** pointer (the file's hash and the server URLs) is published to
   nostr relays.
5. To recover, you log in again with the same `nsec` + password; the app re-derives the
   key, finds the pointer, downloads the ciphertext, and decrypts it.

For the case where you lose the `nsec` *or* forget the password, the derived key is split
into Shamir secret shares (default **2-of-3**, configurable) using
[SLIP-0039](https://github.com/satoshilabs/slips/blob/master/slip-0039.md) mnemonics. Give
the shares to people you trust. Any quorum of them can reconstruct the derived key and
recover the file.

## Why the design is safe to share

The derived key is a **one-way** function of your `nsec` and password — it cannot be
turned back into your `nsec`. So the shares you hand to friends, and the derived key
itself, can only ever unlock **this backup**. They never expose your real nostr identity.
That is the whole point of the scheme: social recovery without surrendering your
identity.

Two independent paths reconstruct the derived key, and nothing else does:

- your master `nsec` **and** the password, or
- a quorum of the Shamir shares.

A note on trust: a 2-of-3 scheme means any **two** share-holders who collude can read the
backup. Choose your threshold accordingly — the app makes this explicit when you create
the shares.

## What it does *not* protect, and known boundaries

We document these honestly rather than hide them:

- **Backup, not identity.** The shares and derived key unlock the backup only.
- **Relay persistence.** Relays may drop events. The app publishes to several relays and
  lets you export a small, secret-free **recovery manifest** as an offline fallback, so a
  recovery still works even if every relay forgets you.
- **Webview boundary.** The `nsec` and password are typed into the UI and briefly cross
  into the Rust core through a Tauri command, so they pass through the webview's memory.
  The app collects them immediately before use, never persists or logs them, and wipes
  them from memory afterward.

For the cryptographic specifics (key derivation, the streaming AEAD used for files, the
metadata event format, and the SLIP-0039 parameters), see [`plan.md`](./plan.md).

## Tech stack

Built with [Tauri 2](https://tauri.app) and a [SvelteKit](https://svelte.dev/docs/kit) +
TypeScript frontend over a Rust backend. The Rust side owns all cryptography and secret
handling; the frontend is the view. The frontend runs as a single-page app:
SvelteKit's [`adapter-static`](https://svelte.dev/docs/kit/adapter-static) prerenders to
plain files that Tauri serves in the native webview — there is no Node.js server at
runtime.

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
├── plan.md             # Engineering plan and cryptographic specification
├── src/                # SvelteKit + TypeScript frontend (the view)
│   ├── app.html        # HTML shell
│   └── routes/         # Pages and layouts
├── static/             # Static assets served as-is
├── src-tauri/          # Rust backend (owns crypto and secrets)
│   ├── src/
│   │   ├── lib.rs      # App setup and commands
│   │   └── main.rs     # Binary entry point
│   ├── capabilities/   # Permission definitions
│   ├── tauri.conf.json # Tauri configuration
│   └── Cargo.toml
├── svelte.config.js
└── package.json
```

> The Rust module layout above reflects the current scaffold. The planned modules
> (`crypto/`, `nostr/`, `blossom/`, `backup.rs`, `recover.rs`, …) are described in
> [`plan.md`](./plan.md) and will land as the phases there are implemented.

The frontend and backend communicate through Tauri's
[command](https://tauri.app/develop/calling-rust/) and
[event](https://tauri.app/develop/calling-frontend/) systems.
