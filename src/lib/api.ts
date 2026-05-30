// Thin marshalling layer over the Rust backend. No crypto, no storage, no
// share-matching — the webview only collects input, calls a command, and
// renders the result. Every secret operation happens in src-tauri/.

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

// --- backend types (mirror src-tauri) --------------------------------------

/** Exportable, secret-free pointer to where a backup lives. Treated as opaque
 *  by the UI: pass it straight back to `exportManifest` unmodified. */
export type Manifest = Record<string, unknown>;

export interface BackupOutcome {
  derived_npub: string;
  blob_sha256: string;
  shares: string[]; // real SLIP-0039 mnemonics
  manifest: Manifest;
}

// --- defaults the UI ships with --------------------------------------------
// TODO(backend): replace with the canonical, known-good relay + Blossom URLs.

export const DEFAULT_RELAYS = ["wss://relay.damus.io", "wss://nos.lol"];
export const DEFAULT_SERVERS = ["https://blossom.primal.net"];
export const THRESHOLD = 2;
export const SHARE_COUNT = 3;

// --- native pickers (replace the File API + base64 pipeline) ---------------

/** Pick one file to protect. Returns its path, or null if cancelled. */
export async function pickFile(): Promise<string | null> {
  const picked = await open({ multiple: false, directory: false });
  return typeof picked === "string" ? picked : null;
}

/** Pick the folder to write a recovered file into. The file keeps the name it
 *  was backed up under, so we choose a directory, not a full path. Returns the
 *  directory, or null if cancelled. */
export async function pickOutputDir(): Promise<string | null> {
  const picked = await open({ directory: true, multiple: false });
  return typeof picked === "string" ? picked : null;
}

/** The file name portion of a path, for display only. */
export function baseName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

// --- backend commands (one wrapper each; arg names match commands.rs) ------

export function backup(
  nsec: string,
  password: string,
  filePath: string,
): Promise<BackupOutcome> {
  return invoke<BackupOutcome>("backup", {
    nsec,
    password,
    filePath,
    relays: DEFAULT_RELAYS,
    servers: DEFAULT_SERVERS,
    threshold: THRESHOLD,
    shareCount: SHARE_COUNT,
  });
}

export function exportManifest(manifest: Manifest, path: string): Promise<void> {
  return invoke<void>("export_manifest", { manifest, path });
}

/** Recover into `outputDir`; the file keeps its original name. Returns the full
 *  path it was written to. */
export function recoverWithPassword(
  nsec: string,
  password: string,
  outputDir: string,
  manifestPath?: string,
): Promise<string> {
  return invoke<string>("recover_with_password", {
    nsec,
    password,
    outputDir,
    relays: DEFAULT_RELAYS,
    manifestPath: manifestPath ?? null,
  });
}

/** Recover into `outputDir`; the file keeps its original name. Returns the full
 *  path it was written to. */
export function recoverWithShares(
  shares: string[],
  outputDir: string,
  manifestPath?: string,
): Promise<string> {
  return invoke<string>("recover_with_shares", {
    shares,
    outputDir,
    relays: DEFAULT_RELAYS,
    manifestPath: manifestPath ?? null,
  });
}
