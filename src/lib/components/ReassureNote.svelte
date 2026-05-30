<script lang="ts">
  import Icon, { type IconName } from "./Icon.svelte";
  import type { Snippet } from "svelte";

  let {
    icon = "shield",
    color = "var(--success)",
    children,
  }: { icon?: IconName; color?: string; children: Snippet } = $props();
</script>

<div class="note-sunken">
  <span class="ns-ic" style="color:{color};"><Icon name={icon} size={20} /></span>
  <span>{@render children()}</span>
</div>
