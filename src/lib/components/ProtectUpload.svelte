<script lang="ts">
  import { app, go, type ProtectInput } from "../store.svelte";
  import { pickFile, baseName, MAX_TEXT_BYTES, textBytes } from "../api";
  import Icon from "./Icon.svelte";
  import FlowHeader from "./FlowHeader.svelte";
  import FileChip from "./FileChip.svelte";

  let error = $state("");

  function setInput(source: ProtectInput) {
    app.protectInput = source;
    error = "";
  }

  async function choose() {
    error = "";
    try {
      const path = await pickFile();
      if (!path) return;
      app.filePath = path;
      app.fileName = baseName(path);
    } catch (e) {
      error = `I couldn't open the file picker, dear. (${String(e)})`;
    }
  }

  function clear() {
    app.filePath = "";
    app.fileName = "";
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      choose();
    }
  }

  // Text is measured in UTF-8 bytes, the unit the backend's limit uses.
  const textTooLong = $derived(textBytes(app.secretText) > MAX_TEXT_BYTES);
  const canContinue = $derived(
    app.protectInput === "file"
      ? app.filePath.length > 0
      : app.secretText.trim().length > 0 && !textTooLong,
  );

  const lead = $derived(
    app.protectInput === "file"
      ? "Pick one file that matters to you: your family recipes, a will, or any important information you don't want to lose."
      : "Type something you can't lose — a password, a recovery phrase, a private note. We'll keep it as safe as a file.",
  );

  // Advanced collects key + password (5 steps); easy shows one code screen (4).
  const total = 5;
</script>

<div class="card">
  <FlowHeader
    label="Keep something safe"
    {total}
    current={0}
    onBack={() => go("home")}
  />

  <h2>What do you want to keep safe?</h2>
  <p class="lead">{lead}</p>

  <div class="seg" style="margin-top:18px;">
    <button
      class="seg-btn {app.protectInput === 'file' ? 'on' : ''}"
      onclick={() => setInput("file")}
    >
      <Icon name="file" size={16} /> Keep a file
    </button>
    <button
      class="seg-btn {app.protectInput === 'text' ? 'on' : ''}"
      onclick={() => setInput("text")}
    >
      <Icon name="note" size={16} /> Type it in
    </button>
  </div>

  <div style="margin-top: 22px;">
    {#if app.protectInput === "file"}
      {#if app.fileName}
        <div style="display:flex; align-items:center; gap:12px;">
          <FileChip name={app.fileName} />
          <button
            class="flow-back"
            aria-label="Remove file"
            onclick={clear}
            style="margin-left:auto;"
          >
            <Icon name="x" size={19} />
          </button>
        </div>
      {:else}
        <div
          class="dropzone"
          role="button"
          tabindex="0"
          onclick={choose}
          onkeydown={onKey}
        >
          <span class="dz-icon"><Icon name="upload" size={26} /></span>
          <span class="dz-title">Click to choose a file</span>
          <span class="muted">One file at a time.</span>
        </div>
      {/if}
    {:else}
      <label for="secret-text">Your secret</label>
      <textarea
        id="secret-text"
        class="input"
        rows="7"
        placeholder="Type or paste what you want to keep safe…"
        style="resize:vertical; line-height:1.6;"
        bind:value={app.secretText}
      ></textarea>
      <p class="muted" style="margin-top:6px;">
        {#if textTooLong}
          <span style="color:var(--danger); font-weight:600;"
            >That's a bit too long — please keep it under 10 MB.</span
          >
        {:else}
          Up to 10 MB of text. It's saved as <b>nanavault-secret-file.txt</b>.
        {/if}
      </p>
    {/if}
  </div>

  {#if error}<div class="error">{error}</div>{/if}

  <button
    class="btn"
    style="margin-top: 30px;"
    disabled={!canContinue}
    onclick={() => go("mode")}
  >
    Continue <Icon name="arrow-right" />
  </button>
</div>
