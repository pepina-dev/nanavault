<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    id,
    label,
    placeholder = "",
    value = $bindable(""),
    hint = "",
  }: {
    id: string;
    label: string;
    placeholder?: string;
    value?: string;
    hint?: string;
  } = $props();

  let show = $state(false);
</script>

<label for={id}>{label}</label>
<div class="field-wrap">
  {#if show}
    <input {id} type="text" {placeholder} bind:value autocomplete="off" spellcheck="false" />
  {:else}
    <input {id} type="password" {placeholder} bind:value autocomplete="off" spellcheck="false" />
  {/if}
  <button
    type="button"
    class="field-reveal"
    aria-label={show ? "Hide" : "Show"}
    tabindex="-1"
    onclick={() => (show = !show)}
  >
    <Icon name={show ? "eye-off" : "eye"} size={19} />
  </button>
</div>
{#if hint}<p class="muted" style="margin-top:6px;">{hint}</p>{/if}
