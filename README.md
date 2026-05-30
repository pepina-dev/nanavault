# NanaVault

Encrypted, recoverable file backups on [nostr](https://nostr.com) +
[Blossom](https://github.com/hzrd149/blossom), with social recovery.

> **Status.** The Rust backend is implemented and covered by a test suite that
> includes the complete official SLIP-0039 vector set. The desktop UI is not
> built yet, so there is not yet an end-user app to click through — the backend
> is driven through Tauri commands and exercised by `cargo test`.

## What it does

NanaVault backs up a single file so you can get it back even if you lose almost
everything:

1. You provide a nostr secret key (`nsec`) and a password.
2. From those two, the app derives a *separate* key. This derived key encrypts
   your file and is also the nostr identity that records where the backup lives.
3. The encrypted file is uploaded to one or more Blossom servers.
4. A small, **encrypted** pointer (the file's hash and the server URLs) is
   published to nostr relays.
5. To recover, you log in again with the same `nsec` + password; the app
   re-derives the key, finds the pointer, downloads the ciphertext, verifies it,
   and decrypts it.

If you lose the `nsec` *or* forget the password, the derived key is also split
into [SLIP-0039](https://github.com/satoshilabs/slips/blob/master/slip-0039.md)
mnemonic shares (default **2-of-3**, configurable). Give the shares to people you
trust; any quorum of them reconstructs the derived key and recovers the file.

## Why the design is safe to share

The derived key is a **one-way** function of your `nsec` and password — it cannot
be turned back into your `nsec`. So the shares you hand to friends, and the
derived key itself, can only ever unlock **this backup**. They never expose your
real nostr identity. That is the whole point: social recovery without
surrendering your identity.

Exactly two paths reconstruct the derived key, and nothing else does:

- your master `nsec` **and** the password, or
- a quorum of the Shamir shares.

A note on trust: a 2-of-3 scheme means any **two** share-holders who collude can
read the backup. Choose your threshold accordingly.

## What it does *not* protect, and known boundaries

Stated honestly rather than hidden:

- **Backup, not identity.** The shares and the derived key unlock the backup
  only — never your `nsec`.
- **Relay persistence.** Relays may drop events. The app publishes to several
  relays *and* lets you export a small, secret-free **recovery manifest** as an
  offline fallback, so recovery still works even if every relay forgets you.
- **Blossom availability.** A server may drop a blob. The app uploads to several
  servers and, on recovery, tries each until one returns a blob whose hash
  matches.
- **Webview boundary.** The `nsec` and password are typed into the UI and briefly
  cross into the Rust core through a Tauri command, so they pass through the
  webview's memory. The Rust side holds them in zeroizing wrappers, never
  persists or logs them, and wipes them on return.

## How the cryptography works

All of this lives in the Rust backend; nothing security-sensitive runs in the
webview.

- **Key derivation.** `salt = SHA-256("nanavault/kdf/v1" ‖ master_xonly_pubkey)`;
  `pw_key = Argon2id(password, salt)` with `m = 256 MiB, t = 4, p = 1`;
  `seed = HKDF-SHA256(ikm = master_secret, salt = pw_key, info = "nanavault/derived-key/v1")`,
  mapped to a valid secp256k1 scalar. The Argon2id parameters are a fixed
  constant (recovery has to re-derive the key before it can read anything, so
  there is nowhere to read tunable parameters from first); they are still
  recorded in the pointer and manifest for transparency.
- **File key.** A dedicated symmetric key, `HKDF-SHA256` from the derived key
  (`info = "nanavault/file-key/v1"`), so the signing key and the encryption key
  are never the same bytes.
- **File encryption.** XChaCha20-Poly1305 in the STREAM construction, in 1 MiB
  chunks. The blob is `magic ‖ version ‖ 19-byte nonce prefix ‖ AEAD chunks`. Its
  SHA-256 is both its Blossom address and an integrity tag that is verified
  before decryption.
- **Pointer event.** A replaceable nostr event (kind `10909`) authored by the
  derived key, whose content is a NIP-44 self-encrypted JSON record of the blob
  hash, size, servers, cipher, and KDF parameters. A relay observer learns only
  that some key published one small encrypted event.
- **Shamir sharing.** SLIP-0039, implemented from scratch and validated against
  the complete official test vectors: a single `threshold`-of-`count` group with
  an empty passphrase (the share path must never need a remembered secret).
- **Recovery manifest.** A secret-free JSON file (relay list + the pointer's
  descriptor) that lets recovery proceed even if every relay has dropped the
  pointer. The key still comes from the password or the shares.

## Architecture

The Rust backend owns all logic and secret handling; Tauri commands are a thin
boundary; the (forthcoming) SvelteKit frontend is only the view. A small
ports-and-adapters seam wraps the two external systems — nostr relays and Blossom
servers — so the orchestration is unit-testable against in-memory fakes, with no
network.

```
src-tauri/src/
├── lib.rs            # Tauri builder; registers the commands
├── commands.rs       # thin #[tauri::command] handlers
├── error.rs          # one error type, serialized to a message at the boundary
├── secret.rs         # zeroizing newtypes (Password, MasterKey, DerivedKey, FileKey)
├── crypto/
│   ├── kdf.rs        # Argon2id + HKDF → derived key; HKDF → file key
│   ├── cipher.rs     # XChaCha20-Poly1305 STREAM blob format; BlobHash
│   └── slip39/       # SLIP-0039 from scratch (GF(256), RS1024, Feistel, mnemonics)
├── metadata.rs       # the NIP-44 encrypted pointer event
├── relay.rs          # MetadataStore port + nostr-sdk adapter
├── blossom.rs        # BlobStore port + nostr-blossom adapter
├── manifest.rs       # the secret-free recovery manifest
├── backup.rs         # orchestration: encrypt → upload → publish → split → manifest
└── recover.rs        # orchestration: (derive | combine) → locate → download → decrypt
```

## Backend API (Tauri commands)

- `backup(nsec, password, file_path, relays, servers, threshold, share_count)` →
  `{ derived_npub, blob_sha256, shares, manifest }`
- `export_manifest(manifest, path)` — write the recovery manifest to disk.
- `recover_with_password(nsec, password, output_path, relays, manifest_path?)`
- `recover_with_shares(shares, output_path, relays, manifest_path?)`

The `shares` returned by `backup` are secret — distribute them, don't log them.

## Tech stack

Built with [Tauri 2](https://tauri.app) and a [SvelteKit](https://svelte.dev/docs/kit)
+ TypeScript frontend over a Rust backend. The frontend runs as a single-page
app: SvelteKit's [`adapter-static`](https://svelte.dev/docs/kit/adapter-static)
prerenders to plain files that Tauri serves in the native webview — there is no
Node.js server at runtime.

Key Rust dependencies: [`nostr`](https://crates.io/crates/nostr) /
[`nostr-sdk`](https://crates.io/crates/nostr-sdk) (keys, NIP-44, relay client),
[`nostr-blossom`](https://crates.io/crates/nostr-blossom) (Blossom client),
`argon2`, `hkdf`, `sha2`, `chacha20poly1305`, `pbkdf2`, and `zeroize`. Versions
are pinned to stable releases.

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

## Testing the backend

From `src-tauri/`:

```sh
cargo test      # unit + orchestration tests, including the SLIP-0039 vectors
cargo clippy --all-targets
cargo fmt --check
```

The frontend type-checks with:

```sh
bun run check
```

## Project layout

```
.
├── src/                # SvelteKit + TypeScript frontend (the view)
│   ├── app.html        # HTML shell
│   └── routes/         # Pages and layouts
├── static/             # Static assets served as-is
├── src-tauri/          # Rust backend (owns crypto and secrets)
│   ├── src/            # modules (see Architecture above)
│   ├── capabilities/   # Tauri permission definitions
│   ├── tauri.conf.json # Tauri configuration
│   └── Cargo.toml
├── svelte.config.js
└── package.json
```

The frontend and backend communicate through Tauri's
[command](https://tauri.app/develop/calling-rust/) and
[event](https://tauri.app/develop/calling-frontend/) systems.

## Roadmap

- **Done** — the full Rust backend: cryptography, relay and Blossom adapters,
  backup and recovery orchestration, and the Tauri commands, with an extensive
  offline test suite.
- **Next** — the desktop UI (backup and both recovery flows, share display,
  manifest export), and end-to-end integration tests against a live relay and
  Blossom server.
