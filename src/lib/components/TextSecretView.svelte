<script lang="ts">
  import { app, go, resetRecover } from "../store.svelte";
  import {
    MAX_TEXT_BYTES,
    textBytes,
    pickOutputDir,
    saveText,
    resaveTextWithPassword,
    resaveTextWithShares,
  } from "../api";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import Icon from "./Icon.svelte";
  import SealMark from "./SealMark.svelte";

  // Only shown for a text recovery, so `app.recovered` is the text variant here.
  // Seed the editor once from the recovered content; edits live locally until the
  // user saves them. (A new recovery remounts this component, re-seeding it.)
  const seed = app.recovered?.kind === "text" ? app.recovered.content : "";
  let text = $state(seed);
  let baseline = $state(seed); // the last-saved content, to detect unsaved edits

  let busy = $state(false);
  let error = $state("");
  let savedPath = $state(""); // path of an on-disk copy, once the user saves one
  let changesSaved = $state(false); // brief confirmation after "save changes"
  let confirmingLeave = $state(false);

  const dirty = $derived(text !== baseline);
  const tooLong = $derived(textBytes(text) > MAX_TEXT_BYTES);
  const canSaveChanges = $derived(
    dirty && text.trim().length > 0 && !tooLong && !busy,
  );

  async function saveChanges() {
    error = "";
    busy = true;
    try {
      if (app.recoverMode === "shares") {
        await resaveTextWithShares(
          app.shareEntries.map((s) => s.trim()).filter(Boolean),
          text,
        );
      } else {
        await resaveTextWithPassword(
          app.recoverNsec,
          app.recoverPassword,
          text,
        );
      }
      baseline = text; // the edits are now the saved version
      changesSaved = true;
    } catch (e) {
      console.error(e);
      error = `I couldn't save your changes, dear. (${String(e)})`;
    } finally {
      busy = false;
    }
  }

  async function saveCopy() {
    error = "";
    busy = true;
    try {
      const dir = await pickOutputDir();
      if (!dir) return; // cancelled
      savedPath = await saveText(text, dir);
    } catch (e) {
      console.error(e);
      error = `I couldn't save it there, dear. (${String(e)})`;
    } finally {
      busy = false;
    }
  }

  async function reveal() {
    error = "";
    try {
      await revealItemInDir(savedPath);
    } catch (e) {
      console.error(e);
      error = `I couldn't open the folder, dear. (${String(e)})`;
    }
  }

  function leave() {
    resetRecover();
    go("home");
  }
</script>

<div class="card">
  <div class="center">
    <SealMark size={88} check />
    <h1>Here's your secret.</h1>
    <p class="lead">
      Read it, or change it and save. It only lives here until you save it.
    </p>
  </div>

  <label for="recovered-text">Your secret</label>
  <textarea
    id="recovered-text"
    class="input"
    rows="8"
    style="resize:vertical; line-height:1.6;"
    bind:value={text}
  ></textarea>
  {#if tooLong}
    <p class="muted" style="margin-top:6px;">
      <span style="color:var(--danger); font-weight:600;"
        >That's a bit too long — please keep it under 10 MB to save.</span
      >
    </p>
  {/if}

  {#if changesSaved && !dirty}
    <div style="display:flex; justify-content:center; margin-top:16px;">
      <span class="badge-success">
        <Icon name="check" size={13} stroke={2.6} /> Changes saved
      </span>
    </div>
  {/if}

  {#if savedPath}
    <div class="note" style="margin-top:16px; display:flex; gap:10px; align-items:center;">
      <span class="inline-ic" style="color:var(--success);">
        <Icon name="check-circle" size={18} />
      </span>
      <span style="flex:1;">Saved a copy to your computer.</span>
      <button
        class="btn btn-ghost"
        style="width:auto; padding:8px 14px;"
        onclick={reveal}
      >
        <Icon name="search" size={16} /> Show me
      </button>
    </div>
  {/if}

  {#if error}<div class="error">{error}</div>{/if}

  {#if confirmingLeave}
    <div class="note" style="margin-top:22px;">
      <p>You have unsaved changes. Leave without saving them?</p>
    </div>
    <div class="row" style="margin-top:12px;">
      <button class="btn btn-ghost" onclick={() => (confirmingLeave = false)}>
        Keep editing
      </button>
      <button class="btn btn-dark" onclick={leave}>Discard &amp; leave</button>
    </div>
  {:else}
    <div class="stack" style="margin-top: 22px;">
      <button class="btn" disabled={!canSaveChanges} onclick={saveChanges}>
        <Icon name="lock" />
        {busy ? "Saving…" : "Save changes"}
      </button>
      <button class="btn btn-ghost" disabled={busy} onclick={saveCopy}>
        <Icon name="download" /> Save a copy to my computer
      </button>
      <button
        class="btn btn-ghost"
        onclick={() => (dirty ? (confirmingLeave = true) : leave())}
      >
        <Icon name="home" /> Done
      </button>
    </div>
  {/if}
</div>
