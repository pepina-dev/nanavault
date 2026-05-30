<script lang="ts">
  import { onMount } from "svelte";
  import { app, go } from "../store.svelte";
  import { backup, recoverWithShares, recoverWithPassword } from "../api";
  import Icon from "./Icon.svelte";
  import NanaLogo from "./NanaLogo.svelte";

  let { mode }: { mode: "seal" | "recover" } = $props();

  type Task = { icon: string; label: string; sub: string };

  const SEAL: Task[] = [
    { icon: "key", label: "Making your key", sub: "Mixing your secret key and password" },
    { icon: "lock", label: "Sealing your file", sub: "Locking it so only your key opens it" },
    { icon: "users", label: "Splitting it into 3 keys", sub: "So your circle can help you back" },
  ];
  const RECOVER: Task[] = [
    { icon: "key", label: "Putting the keys together", sub: "Combining your friends' pieces" },
    { icon: "search", label: "Finding your file", sub: "Reading the sealed note" },
    { icon: "lock", label: "Unsealing it for you", sub: "Only your keys could do this" },
  ];

  const tasks = $derived(mode === "seal" ? SEAL : RECOVER);
  const title = $derived(mode === "seal" ? "Sealing your file…" : "Bringing your file back…");

  // The animation below is cosmetic — the backend runs as one opaque call (a
  // multi-second Argon2id derivation among other things), so the steps reassure
  // rather than report real progress.
  let done = $state(0);
  let runDone = $state(false);
  let error = $state("");

  async function doRun() {
    try {
      if (mode === "seal") {
        app.outcome = await backup(app.nsec, app.masterPassword, app.filePath);
      } else if (app.recoverMode === "shares") {
        await recoverWithShares(
          app.shareEntries.map((s) => s.trim()).filter(Boolean),
          app.outputPath,
        );
        app.recoveredTo = app.outputPath;
      } else {
        await recoverWithPassword(
          app.recoverNsec,
          app.recoverPassword,
          app.outputPath,
        );
        app.recoveredTo = app.outputPath;
      }
      runDone = true;
      finish();
    } catch (e) {
      error = String(e);
    }
  }

  function step() {
    if (error) return;
    if (done < tasks.length) {
      setTimeout(() => {
        done += 1;
        step();
      }, done === 0 ? 700 : 950);
    } else {
      finish();
    }
  }

  function finish() {
    if (error) return;
    if (runDone && done >= tasks.length) {
      setTimeout(() => go(mode === "seal" ? "share" : "recover-success"), 500);
    }
  }

  onMount(() => {
    doRun();
    step();
  });
</script>

<div class="card center">
  {#if error}
    <div class="icon-badge" style="background:#fef2f2; color:var(--danger);">
      <Icon name="alert" size={26} />
    </div>
    <h2>{mode === "seal" ? "Something went wrong" : "We couldn't put it together"}</h2>
    <div class="error" style="text-align:left;">{error}</div>
    <button
      class="btn btn-ghost"
      style="margin-top:18px;"
      onclick={() => go(mode === "seal" ? "password" : "recover")}
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

    <div class="card" style="text-align:left; padding:6px 20px; box-shadow:none; border-color:var(--border); animation:none;">
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
            <div style="font-weight:700; font-size:0.95rem; color:{state === 'wait' ? 'var(--fg-hint)' : 'var(--fg)'};">
              {t.label}
            </div>
            <div class="muted" style="font-size:0.82rem;">{t.sub}</div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
