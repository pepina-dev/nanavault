<script lang="ts">
  // The NanaVault logo with an optional green "done" check badge. Extracted
  // because the protect- and recover-success screens used the exact same block.
  import NanaLogo from "./NanaLogo.svelte";
  import Icon from "./Icon.svelte";

  let { size = 90, check = false }: { size?: number; check?: boolean } = $props();
</script>

<div class="seal-mark pop" style="margin-bottom:14px;">
  <NanaLogo {size} />
  {#if check}
    <span class="seal-check">
      <Icon name="check" size={17} stroke={2.6} />
    </span>
  {/if}
</div>

<style>
  .seal-check {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: var(--success);
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 0 0 3px var(--bg);
  }
</style>
