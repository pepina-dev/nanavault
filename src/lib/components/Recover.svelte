<script lang="ts">
  import { app, go, type RecoverMode } from "../store.svelte";
  import { THRESHOLD, SHARE_COUNT } from "../api";
  import Icon from "./Icon.svelte";
  import FlowHeader from "./FlowHeader.svelte";
  import ReassureNote from "./ReassureNote.svelte";
  import SecretField from "./SecretField.svelte";

  let error = $state("");

  function setMode(m: RecoverMode) {
    app.recoverMode = m;
    error = "";
  }

  const sharesReady = $derived(
    app.shareEntries.filter((s) => s.trim().length > 0).length >= THRESHOLD,
  );
  // Easy-mode codes may have no password, so only the code/key is required.
  const passwordReady = $derived(app.recoverNsec.trim().length > 3);
  const canRecover = $derived(
    app.recoverMode === "shares" ? sharesReady : passwordReady,
  );

  function start() {
    error = "";
    // We recover first and ask where to keep the file afterwards, so there's no
    // folder picker before the file even exists.
    go("recovering");
  }
</script>

<div class="card">
  <FlowHeader label="Recover a secret" onBack={() => go("home")} />

  <div class="icon-badge"><Icon name="recover" size={26} /></div>
  <h2>Bring your secret back</h2>

  <!-- mode toggle -->
  <div class="seg" style="margin-top:16px;">
    <button
      class="seg-btn {app.recoverMode === 'shares' ? 'on' : ''}"
      onclick={() => setMode("shares")}
    >
      <Icon name="users" size={16} /> With my people's keys
    </button>
    <button
      class="seg-btn {app.recoverMode === 'password' ? 'on' : ''}"
      onclick={() => setMode("password")}
    >
      <Icon name="key" size={16} /> With my backup code
    </button>
  </div>

  {#if app.recoverMode === "shares"}
    <p class="lead" style="margin-top:18px;">
      Ask <strong>{THRESHOLD} of your {SHARE_COUNT} people</strong> for their
      recovery codes and paste them in below. You don't need everyone — just any {THRESHOLD}.
    </p>

    <div style="margin-top:16px;">
      <ReassureNote icon="heart" color="var(--rose)">
        One person's key can't open anything. It takes <b>any {THRESHOLD}</b> together
        to bring your secret back.
      </ReassureNote>
    </div>

    <div class="stack" style="gap: 14px; margin-top: 20px;">
      {#each app.shareEntries as _entry, i}
        <div>
          <label
            for={`share-${i}`}
            style="display:flex; align-items:center; gap:8px;"
          >
            Person {i + 1}'s recovery code
            {#if app.shareEntries[i].trim().length > 0}
              <span
                class="pill pill-success"
                style="display:inline-flex;align-items:center;gap:4px;"
              >
                <Icon name="check" size={12} stroke={2.6} /> got it
              </span>
            {/if}
          </label>
          <textarea
            id={`share-${i}`}
            class="input"
            rows="2"
            placeholder="Paste their recovery code (a list of words)…"
            style="resize:none; line-height:1.5; font-family:ui-monospace,monospace; font-size:0.95rem;"
            bind:value={app.shareEntries[i]}
          ></textarea>
        </div>
      {/each}
    </div>
  {:else}
    <p class="lead" style="margin-top:18px;">
      Enter the <strong>backup code</strong> you saved (or your own nsec), plus
      its <strong>password</strong> and I'll bring the secret back on my own.
    </p>
    <div
      style="margin-top: 18px; display:flex; flex-direction:column; gap:14px;"
    >
      <div>
        <label for="r-nsec">Your backup code or secret key</label>
        <textarea
          id="r-nsec"
          class="input"
          rows="2"
          placeholder="Paste your backup code (a list of words), or your nsec…"
          style="resize:none; line-height:1.5; font-family:ui-monospace,monospace; font-size:0.95rem;"
          bind:value={app.recoverNsec}
        ></textarea>
      </div>
      <SecretField
        id="r-pw"
        label="Your password"
        placeholder="Enter your password here"
        bind:value={app.recoverPassword}
      />
    </div>
  {/if}

  {#if error}<div class="error">{error}</div>{/if}

  <button
    class="btn"
    style="margin-top: 16px;"
    disabled={!canRecover}
    onclick={start}
  >
    <Icon name="sparkles" /> Bring my file back
  </button>
</div>
