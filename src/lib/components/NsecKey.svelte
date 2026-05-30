<script lang="ts">
  import { app, go } from "../store.svelte";
  import Icon from "./Icon.svelte";
  import FlowHeader from "./FlowHeader.svelte";
  import SecretField from "./SecretField.svelte";
  import ReassureNote from "./ReassureNote.svelte";

  const canContinue = $derived(app.nsec.trim().length > 3);
</script>

<div class="card">
  <FlowHeader label="Keep something safe" total={4} current={1} onBack={() => go("protect")} />

  <h2>Add your secret key</h2>
  <p class="lead">
    Your nostr private key helps seal this file. Paste it once — it's used here and
    kept safe right alongside your file.
  </p>

  <div style="margin-top: 20px;">
    <SecretField
      id="nsec"
      label="Your secret key"
      placeholder="nsec1…"
      bind:value={app.nsec}
      hint="We hide it as you type — tap the eye to peek."
    />
  </div>

  <div style="margin-top: 20px;">
    <ReassureNote icon="shield">
      This never leaves this app, dear. The key we make can only ever open
      <b>this one file</b> — never your whole identity.
    </ReassureNote>
  </div>

  <button class="btn" style="margin-top: 28px;" disabled={!canContinue} onclick={() => go("password")}>
    Continue <Icon name="arrow-right" />
  </button>
</div>
