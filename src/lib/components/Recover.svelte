<script lang="ts">
  import { app, go } from "../store.svelte";
  import { pickSaveAs } from "../api";
  import Icon from "./Icon.svelte";
  import FlowHeader from "./FlowHeader.svelte";
  import ReassureNote from "./ReassureNote.svelte";
  import SecretField from "./SecretField.svelte";

  let error = $state("");

  function setMode(m: "shares" | "password") {
    app.recoverMode = m;
    error = "";
  }

  const sharesReady = $derived(
    app.shareEntries.filter((s) => s.trim().length > 0).length >= 2,
  );
  const passwordReady = $derived(
    app.recoverNsec.trim().length > 3 && app.recoverPassword.length > 0,
  );
  const canRecover = $derived(
    app.recoverMode === "shares" ? sharesReady : passwordReady,
  );

  async function start() {
    error = "";
    try {
      const out = await pickSaveAs("recovered-file");
      if (!out) return; // cancelled the save dialog
      app.outputPath = out;
      go("recovering");
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="card">
  <FlowHeader label="Recover a file" onBack={() => go("home")} />

  <div class="icon-badge"><Icon name="recover" size={26} /></div>
  <h2>Bring your file back</h2>

  <!-- mode toggle -->
  <div class="seg" style="margin-top:16px;">
    <button class="seg-btn {app.recoverMode === 'shares' ? 'on' : ''}" onclick={() => setMode("shares")}>
      <Icon name="users" size={16} /> With my people's keys
    </button>
    <button class="seg-btn {app.recoverMode === 'password' ? 'on' : ''}" onclick={() => setMode("password")}>
      <Icon name="key" size={16} /> With my key & password
    </button>
  </div>

  {#if app.recoverMode === "shares"}
    <p class="lead" style="margin-top:18px;">
      Ask <strong>2 of your 3 people</strong> for their recovery keys and paste them
      in below. You don't need everyone — just any two.
    </p>

    <div style="margin-top:16px;">
      <ReassureNote icon="heart" color="var(--rose)">
        One person's key can't open anything. It takes <b>any 2</b> together to bring
        your file back.
      </ReassureNote>
    </div>

    <div class="stack" style="gap: 14px; margin-top: 20px;">
      {#each app.shareEntries as _entry, i}
        <div>
          <label for={`share-${i}`} style="display:flex; align-items:center; gap:8px;">
            Friend {i + 1}'s recovery key
            {#if app.shareEntries[i].trim().length > 0}
              <span class="pill pill-success" style="display:inline-flex;align-items:center;gap:4px;">
                <Icon name="check" size={12} stroke={2.6} /> got it
              </span>
            {/if}
          </label>
          <textarea
            id={`share-${i}`}
            class="input"
            rows="2"
            placeholder="Paste their recovery key (a list of words)…"
            style="resize:none; line-height:1.5; font-family:ui-monospace,monospace; font-size:0.95rem;"
            bind:value={app.shareEntries[i]}
          ></textarea>
        </div>
      {/each}
    </div>
  {:else}
    <p class="lead" style="margin-top:18px;">
      Enter the same <strong>secret key</strong> and <strong>password</strong> you
      sealed the file with, and I'll bring it back on my own.
    </p>
    <div style="margin-top: 18px; display:flex; flex-direction:column; gap:4px;">
      <SecretField id="r-nsec" label="Your secret key" placeholder="nsec1…" bind:value={app.recoverNsec} />
      <SecretField id="r-pw" label="Your password" placeholder="Something only you know" bind:value={app.recoverPassword} />
    </div>
  {/if}

  {#if error}<div class="error">{error}</div>{/if}

  <button class="btn" style="margin-top: 16px;" disabled={!canRecover} onclick={start}>
    <Icon name="sparkles" /> Bring my file back
  </button>
</div>

<style>
  .seg {
    display: flex;
    gap: 6px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    padding: 4px;
  }
  .seg-btn {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    padding: 10px 8px;
    border: none;
    background: transparent;
    border-radius: calc(var(--r-md) - 4px);
    font-weight: 700;
    font-size: 0.88rem;
    color: var(--fg-muted);
    cursor: pointer;
  }
  .seg-btn.on {
    background: var(--fg);
    color: var(--bg);
  }
</style>
