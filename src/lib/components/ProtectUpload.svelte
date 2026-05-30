<script lang="ts">
  import { app, go } from "../store.svelte";
  import { pickFile, baseName } from "../api";
  import Icon from "./Icon.svelte";
  import FlowHeader from "./FlowHeader.svelte";
  import FileChip from "./FileChip.svelte";

  let error = $state("");

  async function choose() {
    error = "";
    try {
      const path = await pickFile();
      if (!path) return; // grandma cancelled the dialog
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

  const canContinue = $derived(app.filePath.length > 0);
</script>

<div class="card">
  <FlowHeader label="Keep something safe" total={4} current={0} onBack={() => go("home")} />

  <h2>What should we keep safe?</h2>
  <p class="lead">
    Pick one file that matters — a wallet backup, a password export, a will, the
    photos you can't lose.
  </p>

  <div style="margin-top: 22px;">
    {#if app.fileName}
      <div style="display:flex; align-items:center; gap:12px;">
        <FileChip name={app.fileName} />
        <button class="flow-back" aria-label="Remove file" onclick={clear} style="margin-left:auto;">
          <Icon name="x" size={19} />
        </button>
      </div>
      <p class="muted" style="display:flex; gap:7px; align-items:center; margin-top:12px;">
        <Icon name="lock" size={14} /> Stays on your computer until it's sealed.
      </p>
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
  </div>

  {#if error}<div class="error">{error}</div>{/if}

  <button class="btn" style="margin-top: 30px;" disabled={!canContinue} onclick={() => go("nsec")}>
    Continue <Icon name="arrow-right" />
  </button>
</div>
