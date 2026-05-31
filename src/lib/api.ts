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

/** What a content update returns: the new blob hash and the refreshed manifest.
 *  No shares — re-saving keeps the same identity, so the originals still work. */
export interface UpdateOutcome {
  blob_sha256: string;
  manifest: Manifest;
}

/** What a recovery produced. A file backup is written to a temp path (then saved
 *  to a chosen folder via `saveRecovered`); a text backup comes back in memory for
 *  the app to display, edit, and optionally save via `saveText`. */
export type Recovered =
  | { kind: "file"; path: string }
  | { kind: "text"; filename: string; content: string };

// --- defaults the UI ships with --------------------------------------------
// TODO(backend): replace with the canonical, known-good relay + Blossom URLs.

export const DEFAULT_RELAYS = [
  "wss://relay.damus.io",
  "wss://nos.lol",
  "wss://relay.primal.net",
];
export const DEFAULT_SERVERS = ["https://blossom.primal.net"];
export const THRESHOLD = 2;
export const SHARE_COUNT = 3;

/** Largest typed-text secret accepted for a text backup, in UTF-8 bytes. Mirrors
 *  the backend's `MAX_TEXT_BYTES`, which is the authoritative limit. */
export const MAX_TEXT_BYTES = 10 * 1024 * 1024;

/** The UTF-8 byte length of a string — the unit the size limit is measured in. */
export function textBytes(text: string): number {
  return new TextEncoder().encode(text).length;
}

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

/** Easy mode: mint a fresh backup code on the backend — a SLIP-0039 word
 *  mnemonic (the same format as the guardians' recovery codes). Key generation
 *  is a secret operation, so it lives in Rust. The caller shows the code to the
 *  user and passes it straight back to `backup` in place of an nsec. */
export function generateBackupCode(): Promise<string> {
  return invoke<string>("generate_backup_code");
}

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

/** Text-input counterpart to `backup`: encrypt typed text under the fixed
 *  text-backup name, with the same relay/server/threshold defaults. */
export function backupText(
  nsec: string,
  password: string,
  text: string,
): Promise<BackupOutcome> {
  return invoke<BackupOutcome>("backup_text", {
    nsec,
    password,
    text,
    relays: DEFAULT_RELAYS,
    servers: DEFAULT_SERVERS,
    threshold: THRESHOLD,
    shareCount: SHARE_COUNT,
  });
}

export function exportManifest(manifest: Manifest, path: string): Promise<void> {
  return invoke<void>("export_manifest", { manifest, path });
}

/** Recover a backup. A file backup comes back as a temp path (then saved to a
 *  chosen folder via `saveRecovered`); a text backup comes back in memory. */
export function recoverWithPassword(
  nsec: string,
  password: string,
  manifestPath?: string,
): Promise<Recovered> {
  return invoke<Recovered>("recover_with_password", {
    nsec,
    password,
    relays: DEFAULT_RELAYS,
    manifestPath: manifestPath ?? null,
  });
}

/** Recover a backup from a quorum of shares. A file backup comes back as a temp
 *  path (saved via `saveRecovered`); a text backup comes back in memory. */
export function recoverWithShares(
  shares: string[],
  manifestPath?: string,
): Promise<Recovered> {
  return invoke<Recovered>("recover_with_shares", {
    shares,
    relays: DEFAULT_RELAYS,
    manifestPath: manifestPath ?? null,
  });
}

/** Re-encrypt edited text and republish the pointer under the same identity,
 *  reached with the master key and password. The shares stay valid. */
export function resaveTextWithPassword(
  nsec: string,
  password: string,
  text: string,
): Promise<UpdateOutcome> {
  return invoke<UpdateOutcome>("resave_text_with_password", {
    nsec,
    password,
    text,
    relays: DEFAULT_RELAYS,
    servers: DEFAULT_SERVERS,
  });
}

/** Re-encrypt edited text and republish the pointer under the same identity,
 *  reached with a quorum of shares. */
export function resaveTextWithShares(
  shares: string[],
  text: string,
): Promise<UpdateOutcome> {
  return invoke<UpdateOutcome>("resave_text_with_shares", {
    shares,
    text,
    relays: DEFAULT_RELAYS,
    servers: DEFAULT_SERVERS,
  });
}

/** Move a just-recovered file from its temp location into the folder the user
 *  chose, keeping its name. Returns the final path. */
export function saveRecovered(
  sourcePath: string,
  outputDir: string,
): Promise<string> {
  return invoke<string>("save_recovered", { sourcePath, outputDir });
}

/** Write recovered or edited text into a chosen folder under the fixed
 *  text-backup name (never overwriting an existing file). Returns the path. */
export function saveText(content: string, outputDir: string): Promise<string> {
  return invoke<string>("save_text", { content, outputDir });
}
