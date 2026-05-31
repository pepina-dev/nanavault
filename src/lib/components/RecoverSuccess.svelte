<script lang="ts">
  import { app, go, resetRecover } from "../store.svelte";
  import { baseName, pickOutputDir, saveRecovered } from "../api";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import Icon from "./Icon.svelte";
  import FileChip from "./FileChip.svelte";
  import SealMark from "./SealMark.svelte";
  import TextSecretView from "./TextSecretView.svelte";

  const recovered = $derived(app.recovered);

  let error = $state("");
  let savedPath = $state(""); // the permanent home, once the user picks one
  let busy = $state(false);

  // A file recovery lands in a temp spot; we show its real name until it's saved
  // somewhere permanent. (A text recovery is handled by TextSecretView instead.)
  const tempPath = $derived(recovered?.kind === "file" ? recovered.path : "");
  const fileName = $derived(baseName(savedPath || tempPath));
  const broughtBack = $derived(
    app.recoverMode === "password" ? "decrypted" : "recovered",
  );

  async function chooseAndSave() {
    error = "";
    busy = true;
    try {
      const dir = await pickOutputDir();
      if (!dir) return; // cancelled — leave it in temp, let them try again
      savedPath = await saveRecovered(tempPath, dir);
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

  function home() {
    resetRecover();
    go("home");
  }
</script>

{#if recovered?.kind === "text"}
  <TextSecretView />
{:else if recovered?.kind === "file"}
  <div class="card">
    <div class="center">
      <SealMark size={88} check />
      {#if savedPath}
        <span class="badge-success"
          ><Icon name="check" size={13} stroke={2.6} /> Saved to your computer</span
        >
        <h1 style="margin-top:14px;">All set — it's saved!</h1>
        <p class="lead">Your {broughtBack} file is saved and ready to open.</p>
      {:else}
        <h1>Got it back!</h1>
        <p class="lead">Your file is {broughtBack} and ready.</p>
      {/if}
      <div style="display:flex; justify-content:center; margin:18px 0;">
        <FileChip name={fileName} />
      </div>
    </div>

    {#if !savedPath}
      <p class="lead" style="text-align:center;">
        Where would you like to keep it?
      </p>

      {#if error}<div class="error">{error}</div>{/if}

      <div class="stack" style="margin-top: 22px;">
        <button class="btn" disabled={busy} onclick={chooseAndSave}>
          <Icon name="download" />
          {busy ? "Saving…" : "Choose a folder & save"}
        </button>
        <button class="btn btn-ghost" onclick={home}>
          <Icon name="home" /> Back to home
        </button>
      </div>
    {:else}
      {#if error}<div class="error">{error}</div>{/if}

      <div class="stack" style="margin-top: 26px;">
        <button class="btn btn-success" onclick={reveal}>
          <Icon name="search" /> Show me where it is
        </button>
        <button class="btn btn-ghost" onclick={home}>
          <Icon name="home" /> Back to home
        </button>
      </div>
    {/if}
  </div>
{/if}
