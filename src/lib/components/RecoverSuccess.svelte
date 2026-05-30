<script lang="ts">
  import { app, go, resetRecover } from "../store.svelte";
  import { baseName } from "../api";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import Icon from "./Icon.svelte";
  import NanaLogo from "./NanaLogo.svelte";
  import FileChip from "./FileChip.svelte";

  let error = $state("");

  const savedPath = app.recoveredTo;

  async function reveal() {
    error = "";
    try {
      await revealItemInDir(savedPath);
    } catch (e) {
      error = String(e);
    }
  }

  function home() {
    resetRecover();
    go("home");
  }
</script>

<div class="card">
  <div class="center">
    <div class="seal-mark pop" style="margin-bottom:14px;">
      <NanaLogo size={88} />
      <span
        style="position:absolute; right:0; bottom:0; width:30px; height:30px; border-radius:50%; background:var(--success); color:#fff; display:flex; align-items:center; justify-content:center; box-shadow:0 0 0 3px var(--bg);"
      >
        <Icon name="check" size={17} stroke={2.6} />
      </span>
    </div>
    <h1>Got it back!</h1>
    <p class="lead">Your file is unsealed and saved to your computer.</p>
    <div style="display:flex; justify-content:center; margin:18px 0;">
      <FileChip name={baseName(savedPath)} />
    </div>
  </div>

  <div class="flabel">Saved to</div>
  <div class="note" style="word-break:break-all; display:flex; align-items:center; gap:8px;">
    <span class="inline-ic"><Icon name="download" size={18} /></span> {savedPath}
  </div>

  {#if error}<div class="error">{error}</div>{/if}

  <div class="stack" style="margin-top: 26px;">
    <button class="btn" onclick={reveal}>
      <Icon name="search" /> Show me where it is
    </button>
    <button class="btn btn-ghost" onclick={home}><Icon name="home" /> Back to home</button>
  </div>
</div>
