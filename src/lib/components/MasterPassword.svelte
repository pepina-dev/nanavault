<script lang="ts">
  import { app, go } from "../store.svelte";
  import Icon from "./Icon.svelte";
  import FlowHeader from "./FlowHeader.svelte";
  import SecretField from "./SecretField.svelte";
  import ReassureNote from "./ReassureNote.svelte";

  let pw = $state(app.masterPassword);
  let repeat = $state("");

  const tooShort = $derived(pw.length > 0 && pw.length < 4);
  const mismatch = $derived(repeat.length > 0 && pw !== repeat);
  const canContinue = $derived(pw.length >= 4 && pw === repeat);

  function next() {
    app.masterPassword = pw;
    go("seal");
  }
</script>

<div class="card">
  <FlowHeader label="Keep something safe" total={4} current={2} onBack={() => go("nsec")} />

  <h2>Make your password</h2>
  <p class="lead">
    Your secret key and this password combine into the one key that seals your file.
    Pick something only you know.
  </p>

  <div style="margin-top: 20px; display:flex; flex-direction:column; gap:4px;">
    <SecretField id="pw" label="A password" placeholder="Something only you know" bind:value={pw} />
    <SecretField id="pw2" label="Type it once more" placeholder="Just to be sure" bind:value={repeat} />
  </div>

  {#if tooShort}
    <div class="error">Let's make it at least 4 characters, dear.</div>
  {:else if mismatch}
    <div class="error">Oh dear, the two passwords don't match yet.</div>
  {/if}

  <div style="margin-top: 18px;">
    <ReassureNote icon="shield">
      You'll need both your key and this password to recover the file on your own —
      or any <b>2 of your 3 people</b> can help.
    </ReassureNote>
  </div>

  <button class="btn" style="margin-top: 26px;" disabled={!canContinue} onclick={next}>
    Seal it up <Icon name="lock" />
  </button>
</div>
