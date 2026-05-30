<script lang="ts">
  import { onMount } from "svelte";
  import { app, go } from "../store.svelte";
  import { generateBackupCode } from "../api";
  import Icon from "./Icon.svelte";
  import FlowHeader from "./FlowHeader.svelte";
  import ReassureNote from "./ReassureNote.svelte";

  let loading = $state(true);
  let genError = $state("");
  let revealed = $state(false);
  let copied = $state(false);
  let copyFailed = $state(false);

  async function mint() {
    loading = true;
    genError = "";
    try {
      const code = await generateBackupCode();
      app.backupCode = code;
      app.nsec = code; // the backend accepts this code in place of an nsec
    } catch (e) {
      console.error(e);
      genError = String(e);
    } finally {
      loading = false;
    }
  }

  async function copy() {
    copyFailed = false;
    try {
      await navigator.clipboard.writeText(app.backupCode);
      copied = true;
      setTimeout(() => (copied = false), 1600);
    } catch {
      copyFailed = true;
      setTimeout(() => (copyFailed = false), 3000);
    }
  }

  function next() {
    go("seal"); // the password was already set on the previous step
  }

  onMount(mint);
</script>

<div class="card">
  <FlowHeader
    label="Keep something safe"
    total={5}
    current={3}
    onBack={() => go("password")}
  />

  <h2>Here is your backup code</h2>
  <p class="lead">
    This is a list of words — the key to your file, just like the codes your
    people get. Keep it somewhere safe: write it down or save it in a password
    manager. If you ever lose it, your people can still bring the file back.
  </p>

  {#if loading}
    <div class="note" style="margin-top:20px; text-align:center;">
      <span class="spinner"></span> Making your backup code…
    </div>
  {:else if genError}
    <div class="error" style="margin-top:20px;">
      I couldn't make your backup code just now, dear. ({genError})
    </div>
    <button class="btn" style="margin-top:18px;" onclick={mint}>
      <Icon name="recover" /> Try again
    </button>
    <button
      class="btn btn-ghost"
      style="margin-top:10px;"
      onclick={() => go("mode")}
    >
      <Icon name="arrow-left" /> Back to recovery modes
    </button>
  {:else}
    <div class="note" style="margin-top:20px; padding:16px;">
      <div
        style="display:flex; align-items:center; justify-content:space-between; margin-bottom:10px;"
      >
        <span class="eyebrow">Your backup code</span>
      </div>
      {#if revealed}
        <div class="mnemonic">{app.backupCode}</div>
      {:else}
        <button
          type="button"
          class="words-hidden"
          onclick={() => (revealed = true)}
        >
          <Icon name="eye" size={18} /> Tap to show your backup code
        </button>
      {/if}

      <button
        type="button"
        class="btn btn-dark"
        style="margin-top:14px;"
        onclick={copy}
      >
        <Icon name={copied ? "check" : "file"} size={18} />
        {copied ? "Copied" : "Copy backup code"}
      </button>
      {#if copyFailed}
        <p class="muted" style="margin:8px 0 0; font-size:0.82rem;">
          Couldn't copy automatically — tap the code above to select it, then
          copy by hand.
        </p>
      {/if}
    </div>

    <label class="confirm-row">
      <input type="checkbox" bind:checked={app.backupCodeSaved} />
      <span>I've saved my backup code somewhere safe.</span>
    </label>

    <div style="margin-top:16px;">
      <ReassureNote icon="shield">
        I never keep this code. Once your file is protected, it's wiped from the
        app — only your copy remains.
      </ReassureNote>
    </div>

    <button
      class="btn"
      style="margin-top: 22px;"
      disabled={!app.backupCodeSaved}
      onclick={next}
    >
      Protect my file <Icon name="lock" />
    </button>
  {/if}
</div>

<style>
  .mnemonic {
    font-family: ui-monospace, monospace;
    font-size: 13px;
    line-height: 1.7;
    color: var(--fg);
    word-spacing: 2px;
    word-break: break-word;
    user-select: all;
  }
  .words-hidden {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 16px;
    border: 1px dashed var(--border);
    border-radius: var(--r-md);
    background: var(--bg);
    color: var(--fg-muted);
    font-weight: 700;
    font-size: 0.9rem;
    cursor: pointer;
  }
  .words-hidden:hover {
    color: var(--rose);
    border-color: var(--rose);
  }
  .confirm-row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 18px;
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--fg);
    cursor: pointer;
  }
  .confirm-row input {
    width: 18px;
    height: 18px;
    accent-color: var(--rose);
    flex: none;
  }
</style>
