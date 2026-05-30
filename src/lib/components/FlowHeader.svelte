<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    label,
    total = 1,
    current = 0,
    onBack,
  }: { label: string; total?: number; current?: number; onBack?: () => void } =
    $props();
</script>

<div class="flow-header">
  {#if onBack}
    <button class="flow-back" aria-label="Back" onclick={onBack}>
      <Icon name="arrow-left" size={19} />
    </button>
  {:else}
    <span style="width:40px; flex:none;"></span>
  {/if}
  <div class="flow-meta">
    <span class="eyebrow">{label}</span>
    {#if total > 1}
      <div class="step-dots">
        {#each Array(total) as _, i}
          <span class="s {i === current ? 'active' : i < current ? 'done' : ''}"></span>
        {/each}
      </div>
    {/if}
  </div>
</div>
