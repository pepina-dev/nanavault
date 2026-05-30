<script lang="ts">
  let {
    name = "?",
    seed = 0,
    size = 42,
    ring,
  }: { name?: string; seed?: number; size?: number; ring?: string } = $props();

  // a small set of warm, distinct hues so the people read as individuals
  const HUES = ["#e84a8a", "#f0883e", "#3aa76d", "#7c6cf0", "#39a0c4"];
  const color = $derived(HUES[seed % HUES.length]);
  const initials = $derived(
    name.trim().split(/\s+/).map((w) => w[0]).slice(0, 2).join("").toUpperCase() ||
      "?",
  );
</script>

<span
  class="avatar"
  style="width:{size}px; height:{size}px; font-size:{size * 0.38}px; background:{color}; {ring
    ? `box-shadow: inset 0 0 0 2px rgba(255,255,255,.28), 0 0 0 3px ${ring}, 0 1px 2px rgba(0,0,0,.12);`
    : ''}"
>
  {initials}
</span>
