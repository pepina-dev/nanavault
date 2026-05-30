<script lang="ts">
  import { onMount } from "svelte";
  import {
    app,
    go,
    clearProtectSecrets,
    clearRecoverSecrets,
  } from "../store.svelte";
  import { backup, recoverWithShares, recoverWithPassword } from "../api";
  import Icon, { type IconName } from "./Icon.svelte";
  import NanaLogo from "./NanaLogo.svelte";

  let { mode }: { mode: "seal" | "recover" } = $props();

  type Task = { icon: IconName; label: string; sub: string };

  // Wording softens for easy/shares paths and gets precise for advanced ones:
  //   seal:    easy → "Protecting",  advanced → "Encrypting"
  //   recover: shares → "Recovering", key+password → "Decrypting"
  const easyProtect = $derived(app.recoveryMode === "easy");
  const sealVerb = $derived(easyProtect ? "Protecting" : "Encrypting");
  const decrypting = $derived(app.recoverMode === "password");
  const recoverVerb = $derived(decrypting ? "Decrypting" : "Recovering");

  const SEAL: Task[] = $derived([
    {
      icon: "key",
      label: "Making your key",
      sub: easyProtect
        ? "From your backup code"
        : "Mixing your secret key and password",
    },
    {
      icon: "lock",
      label: `${sealVerb} your file`,
      sub: "Locking it so only your key opens it",
    },
    {
      icon: "users",
      label: "Splitting it into 3 keys",
      sub: "So your circle can help you back",
    },
  ]);
  const RECOVER: Task[] = $derived([
    {
      icon: "key",
      label: "Putting the keys together",
      sub: decrypting ? "From your code and password" : "Combining your friends' pieces",
    },
    {
      icon: "search",
      label: "Finding your file",
      sub: "Reading the protected note",
    },
    {
      icon: "lock",
      label: `${recoverVerb} it for you`,
      sub: "Only your keys could do this",
    },
  ]);

  const tasks = $derived(mode === "seal" ? SEAL : RECOVER);
  const title = $derived(
    mode === "seal"
      ? `${sealVerb} your file…`
      : `${recoverVerb} your file…`,
  );

  // The step animation below is cosmetic — the backend runs as one opaque call
  // (a multi-second Argon2id derivation among other things), so the steps
  // reassure rather than report real progress. We drive the animation purely on
  // a timer and only navigate once BOTH it and the real call have finished.
  let done = $state(0);
  let error = $state("");

  let backendDone = false;
  let cancelled = false; // set on unmount so stray timers never touch dead state
  const timers: ReturnType<typeof setTimeout>[] = [];

  function later(fn: () => void, ms: number) {
    timers.push(setTimeout(() => !cancelled && fn(), ms));
  }

  function friendly(e: unknown): string {
    const detail = e instanceof Error ? e.message : String(e);
    const lead =
      mode === "seal"
        ? `I couldn't finish ${sealVerb.toLowerCase()} your file, dear.`
        : "I couldn't put your file back together, dear.";
    return `${lead} (${detail})`;
  }

  async function runBackend() {
    try {
      if (mode === "seal") {
        const outcome = await backup(
          app.nsec,
          app.masterPassword,
          app.filePath,
        );
        if (cancelled) return;
        app.outcome = outcome;
        clearProtectSecrets();
      } else if (app.recoverMode === "shares") {
        app.recoveredTo = await recoverWithShares(
          app.shareEntries.map((s) => s.trim()).filter(Boolean),
        );
        if (cancelled) return;
        clearRecoverSecrets();
      } else {
        app.recoveredTo = await recoverWithPassword(
          app.recoverNsec,
          app.recoverPassword,
        );
        if (cancelled) return;
        clearRecoverSecrets();
      }
      backendDone = true;
      maybeFinish();
    } catch (e) {
      if (cancelled) return;
      console.error(e); // keep the raw error for debugging
      error = friendly(e);
    }
  }

  // Walk the cosmetic checklist forward on a timer, independent of the backend.
  function animate() {
    if (cancelled || error) return;
    if (done < tasks.length) {
      later(
        () => {
          done += 1;
          animate();
        },
        done === 0 ? 700 : 950,
      );
    } else {
      maybeFinish();
    }
  }

  // Navigate on once the call has resolved AND the animation has caught up.
  function maybeFinish() {
    if (cancelled || error) return;
    if (backendDone && done >= tasks.length) {
      later(() => go(mode === "seal" ? "share" : "recover-success"), 400);
    }
  }

  onMount(() => {
    runBackend();
    animate();
    return () => {
      cancelled = true;
      timers.forEach(clearTimeout);
    };
  });
</script>

<div class="card center">
  {#if error}
    <div class="icon-badge" style="background:#fef2f2; color:var(--danger);">
      <Icon name="alert" size={26} />
    </div>
    <h2>
      {mode === "seal" ? "Something went wrong" : "We couldn't put it together"}
    </h2>
    <div class="error" style="text-align:left;">{error}</div>
    <button
      class="btn btn-ghost"
      style="margin-top:18px;"
      onclick={() =>
        go(
          mode === "seal"
            ? easyProtect
              ? "backup-code"
              : "password"
            : "recover",
        )}
    >
      <Icon name="arrow-left" /> Go back and try again
    </button>
  {:else}
    <div class="seal-mark">
      <span class="pulse-ring"></span>
      <NanaLogo size={88} />
    </div>
    <h2>{title}</h2>
    <p class="lead" style="margin-bottom:24px;">
      This takes a moment. You don't need to do anything.
    </p>

    <div class="card card-inset">
      {#each tasks as t, i}
        {@const state = i < done ? "done" : i === done ? "run" : "wait"}
        <div class="check-row" style="opacity:{state === 'wait' ? 0.5 : 1};">
          <div class="check-mark {state}">
            {#if state === "done"}
              <Icon name="check" size={16} stroke={2.6} />
            {:else if state === "run"}
              <span class="spinner"></span>
            {:else}
              <Icon name={t.icon} size={15} />
            {/if}
          </div>
          <div style="flex:1;">
            <div
              style="font-weight:700; font-size:0.95rem; color:{state === 'wait'
                ? 'var(--fg-hint)'
                : 'var(--fg)'};"
            >
              {t.label}
            </div>
            <div class="muted" style="font-size:0.82rem;">{t.sub}</div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
