<script lang="ts">
  import { app, go } from "../store.svelte";
  import Icon from "./Icon.svelte";
  import FlowHeader from "./FlowHeader.svelte";
  import SecretField from "./SecretField.svelte";
  import ReassureNote from "./ReassureNote.svelte";

  const canContinue = $derived(app.nsec.trim().length > 3);
</script>

<div class="card">
  <FlowHeader
    label="Keep something safe"
    total={5}
    current={2}
    onBack={() => go("mode")}
  />

  <h2>Add your secret key</h2>
  <p class="lead">Your nostr private key is used to encrypt this secret.</p>

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
      This key is only used to protect <b>this one secret</b>
      and then it is erased.
    </ReassureNote>
  </div>

  <button
    class="btn"
    style="margin-top: 28px;"
    disabled={!canContinue}
    onclick={() => go("password")}
  >
    Continue <Icon name="arrow-right" />
  </button>
</div>
