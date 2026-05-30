<script lang="ts">
  import { app, go } from "../store.svelte";
  import Icon from "./Icon.svelte";
  import FlowHeader from "./FlowHeader.svelte";
  import SecretField from "./SecretField.svelte";
  import ReassureNote from "./ReassureNote.svelte";

  let pw = $state(app.masterPassword);
  let repeat = $state("");

  // Easy mode reuses this very screen, just ahead of the backup code. The
  // password is required in both modes.
  const easy = $derived(app.recoveryMode === "easy");

  const tooShort = $derived(pw.length > 0 && pw.length < 4);
  const mismatch = $derived(repeat.length > 0 && pw !== repeat);
  const canContinue = $derived(pw.length >= 4 && pw === repeat);

  function next() {
    app.masterPassword = pw;
    go(easy ? "backup-code" : "seal");
  }
</script>

<div class="card">
  <FlowHeader
    label="Keep something safe"
    total={5}
    current={easy ? 2 : 3}
    onBack={() => go(easy ? "mode" : "nsec")}
  />

  <h2>Make your password</h2>
  {#if easy}
    <p class="lead">
      This password adds an extra lock on top of the backup code I'm about to
      give you. Pick something only you know and will remember.
    </p>
  {:else}
    <p class="lead">
      Your secret key and this password combine into the one key that encrypts
      your file. Pick something only you know and will remember.
    </p>
  {/if}

  <div style="margin-top: 20px; display:flex; flex-direction:column; gap:4px;">
    <SecretField
      id="pw"
      label="A password"
      placeholder="Something only you know"
      bind:value={pw}
    />
    <SecretField
      id="pw2"
      label="Type it once more"
      placeholder="Just to be sure"
      bind:value={repeat}
    />
  </div>

  {#if tooShort}
    <div class="error">Let's make it at least 4 characters, dear.</div>
  {:else if mismatch}
    <div class="error">Oh dear, the two passwords don't match yet.</div>
  {/if}

  <div style="margin-top: 18px;">
    <ReassureNote icon="shield">
      {#if easy}
        You'll need both your backup code and this password to recover the file
        on your own — or any <b>2 of your 3 people</b> can help.
      {:else}
        You'll need both your key and this password to recover the file on your
        own — or any <b>2 of your 3 people</b> can help.
      {/if}
    </ReassureNote>
  </div>

  <button
    class="btn"
    style="margin-top: 26px;"
    disabled={!canContinue}
    onclick={next}
  >
    {#if easy}
      Continue <Icon name="arrow-right" />
    {:else}
      Encrypt it <Icon name="lock" />
    {/if}
  </button>
</div>
