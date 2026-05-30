import type { BackupOutcome } from "./api";
import { SHARE_COUNT, THRESHOLD } from "./api";

export type Screen =
  | "home"
  | "protect" // step 1: pick the file
  | "mode" // step 2: choose a recovery mode (easy / advanced)
  | "nsec" // advanced step: your secret key
  | "password" // advanced step: master password
  | "backup-code" // easy step: show the generated backup code (+ optional password)
  | "seal" // animated: protecting/encrypting (runs backup)
  | "share" // hand out the shares to your people
  | "protect-success"
  | "recover" // collect shares (or key+password)
  | "recovering" // animated: bringing it back (runs recover)
  | "recover-success";

export type RecoverMode = "shares" | "password";

// How the user chooses to be able to recover on their own:
//  - "easy":     we mint a backup code (an nsec) for them; password optional.
//  - "advanced": they bring their own nostr nsec + a password.
// Either way, 2-of-3 guardians can always bring the file back.
export type RecoveryMode = "easy" | "advanced";

export interface Guardian {
  hint: string;
  saved: boolean;
  shared: boolean;
}

// One blank guardian per share, so the UI tracks exactly SHARE_COUNT people.
function freshGuardians(): Guardian[] {
  return Array.from({ length: SHARE_COUNT }, () => ({
    hint: "",
    saved: false,
    shared: false,
  }));
}

// One blank share input per share needed to recover (the threshold).
function freshShareEntries(): string[] {
  return Array.from({ length: THRESHOLD }, () => "");
}

export const app = $state({
  screen: "home" as Screen,

  // protect flow
  filePath: "",
  fileName: "",
  recoveryMode: "easy" as RecoveryMode,
  nsec: "",
  masterPassword: "",
  backupCode: "", // easy mode: the minted word backup code (also copied into nsec)
  backupCodeSaved: false, // easy mode: user confirmed they saved the code
  guardians: freshGuardians(),
  outcome: null as BackupOutcome | null,

  // recover flow
  recoverMode: "shares" as RecoverMode,
  shareEntries: freshShareEntries(),
  recoverNsec: "",
  recoverPassword: "",
  recoveredTo: "", // temp path the recovered file lands in before the user saves it
});

export function go(screen: Screen) {
  app.screen = screen;
}

export function resetProtect() {
  app.filePath = "";
  app.fileName = "";
  app.recoveryMode = "easy";
  app.nsec = "";
  app.masterPassword = "";
  app.backupCode = "";
  app.backupCodeSaved = false;
  app.guardians = freshGuardians();
  app.outcome = null;
}

export function resetRecover() {
  app.recoverMode = "shares";
  app.shareEntries = freshShareEntries();
  app.recoverNsec = "";
  app.recoverPassword = "";
  app.recoveredTo = "";
}

// Wipe just the secrets once a flow has finished with them, so they don't sit
// in this long-lived reactive store until the user happens to return home.
// (A fuller fix would keep secrets out of shared state entirely and hand them
// straight to `invoke` — out of scope for now, but worth a note.)
export function clearProtectSecrets() {
  app.nsec = "";
  app.masterPassword = "";
  app.backupCode = "";
}

export function clearRecoverSecrets() {
  app.recoverNsec = "";
  app.recoverPassword = "";
  app.shareEntries = freshShareEntries();
}
