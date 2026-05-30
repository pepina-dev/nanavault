import type { BackupOutcome } from "./api";

export type Screen =
  | "home"
  | "protect" // step 1: pick the file
  | "nsec" // step 2: your secret key
  | "password" // step 3: master password
  | "seal" // animated: sealing (runs backup)
  | "share" // step 4: hand out the shares
  | "protect-success"
  | "recover" // collect shares (or key+password)
  | "recovering" // animated: bringing it back (runs recover)
  | "recover-success";

export type RecoverMode = "shares" | "password";

export interface Guardian {
  hint: string;
  saved: boolean;
  shared: boolean;
}

function freshGuardians(): Guardian[] {
  return [
    { hint: "", saved: false, shared: false },
    { hint: "", saved: false, shared: false },
    { hint: "", saved: false, shared: false },
  ];
}

export const app = $state({
  screen: "home" as Screen,

  // protect flow
  filePath: "",
  fileName: "",
  nsec: "",
  masterPassword: "",
  guardians: freshGuardians(),
  outcome: null as BackupOutcome | null,

  // recover flow
  recoverMode: "shares" as RecoverMode,
  shareEntries: ["", ""] as string[],
  recoverNsec: "",
  recoverPassword: "",
  outputDir: "",
  recoveredTo: "",
});

export function go(screen: Screen) {
  app.screen = screen;
}

export function resetProtect() {
  app.filePath = "";
  app.fileName = "";
  app.nsec = "";
  app.masterPassword = "";
  app.guardians = freshGuardians();
  app.outcome = null;
}

export function resetRecover() {
  app.recoverMode = "shares";
  app.shareEntries = ["", ""];
  app.recoverNsec = "";
  app.recoverPassword = "";
  app.outputDir = "";
  app.recoveredTo = "";
}
