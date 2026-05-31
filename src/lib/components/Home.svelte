<script lang="ts">
  import { go, resetProtect, resetRecover } from "../store.svelte";
  import NanaLogo from "./NanaLogo.svelte";
  import Icon from "./Icon.svelte";

  function startProtect() {
    resetProtect();
    go("protect");
  }
  function startRecover() {
    resetRecover();
    go("recover");
  }
</script>

<div class="card center">
  <div class="brand"><NanaLogo size={92} /></div>
  <h1>NanaVault</h1>
  <p class="lead">
    Hello, dear! I keep your important secrets safe and I help you get them back
    if they ever go missing.
  </p>

  <div class="stack" style="margin-top: 26px;">
    <button class="btn" onclick={startProtect}>
      <Icon name="lock" /> Protect a secret
    </button>
    <button class="btn btn-ghost" onclick={startRecover}>
      <Icon name="recover" /> Recover a secret
    </button>
  </div>
</div>
