<script lang="ts">
  let { name, sub = "" }: { name: string; sub?: string } = $props();
  const ext = $derived((name.split(".").pop() || "file").slice(0, 4));
</script>

<div class="chip">
  <div class="chip-icon"><span class="ext">{ext}</span></div>
  <div style="display:flex; flex-direction:column; gap:2px; min-width:0;">
    <span class="chip-name">{name}</span>
    {#if sub}<span class="muted">{sub}</span>{/if}
  </div>
</div>
