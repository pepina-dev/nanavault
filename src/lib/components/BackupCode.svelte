<script lang="ts">
  import { onMount } from "svelte";
  import { app, go } from "../store.svelte";
  import { generateBackupCode } from "../api";
  import Icon from "./Icon.svelte";
  import FlowHeader from "./FlowHeader.svelte";
  import ReassureNote from "./ReassureNote.svelte";

  let loading = $state(true);
  let genError = $state("");
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
    <div class="note" style="margin-top:20px; padding:12px 16px;">
      <div style="display:flex; justify-content:flex-end;">
        <button type="button" class="copy-link" onclick={copy}>
          <Icon name={copied ? "check" : "copy"} size={13} />
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <div class="mnemonic" style="margin-top:4px;">{app.backupCode}</div>
    </div>
    {#if copyFailed}
      <p class="muted" style="margin:8px 0 0; font-size:0.82rem;">
        Couldn't copy automatically — tap the code above to select it, then copy
        by hand.
      </p>
    {/if}

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
  .copy-link {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 0;
    background: none;
    border: none;
    color: var(--fg-muted);
    font-weight: 700;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    cursor: pointer;
  }
  .copy-link:hover {
    color: var(--fg);
    text-decoration: underline;
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
