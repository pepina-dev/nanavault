<script lang="ts">
  import { app, go, type RecoveryMode } from "../store.svelte";
  import { THRESHOLD, SHARE_COUNT } from "../api";
  import Icon from "./Icon.svelte";
  import FlowHeader from "./FlowHeader.svelte";

  // Easy is the default and the safe choice for anyone unsure.
  function pick(m: RecoveryMode) {
    app.recoveryMode = m;
  }

  const total = 5;

  function next() {
    // Both modes make a password next; easy then shows the generated code.
    go(app.recoveryMode === "easy" ? "password" : "nsec");
  }
</script>

<div class="card">
  <FlowHeader
    label="Keep something safe"
    {total}
    current={1}
    onBack={() => go("protect")}
  />

  <h2>Do you have a NOSTR identity?</h2>
  <div class="stack" style="gap: 12px; margin-top: 22px;">
    <button
      type="button"
      class="mode-card {app.recoveryMode === 'easy' ? 'on' : ''}"
      onclick={() => pick("easy")}
    >
      <span class="mode-ic"><Icon name="heart" size={22} /></span>
      <span class="mode-body">
        <span class="mode-title"> I have no idea what NOSTR is </span>
        <span class="mode-text">
          Don't worry, we can still protect your secret and give you a recovery
          mode.
        </span>
      </span>
      <span class="mode-check">
        {#if app.recoveryMode === "easy"}<Icon
            name="check"
            size={15}
            stroke={2.6}
          />{/if}
      </span>
    </button>

    <button
      type="button"
      class="mode-card {app.recoveryMode === 'advanced' ? 'on' : ''}"
      onclick={() => pick("advanced")}
    >
      <span class="mode-ic"><Icon name="key" size={22} /></span>
      <span class="mode-body">
        <span class="mode-title">Yes, I have one</span>
        <span class="mode-text">
          Great! If you don't want to enter your nsec, pretend you don't have
          idea what NOSTR is
        </span>
      </span>
      <span class="mode-check">
        {#if app.recoveryMode === "advanced"}<Icon
            name="check"
            size={15}
            stroke={2.6}
          />{/if}
      </span>
    </button>
  </div>

  <button class="btn" style="margin-top: 26px;" onclick={next}>
    Continue <Icon name="arrow-right" />
  </button>
</div>

<style>
  .mode-card {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    width: 100%;
    text-align: left;
    padding: 16px;
    border: 1.5px solid var(--border);
    border-radius: var(--r-lg);
    background: var(--bg);
    cursor: pointer;
    transition:
      border-color 0.15s,
      background 0.15s;
  }
  .mode-card.on {
    border-color: var(--rose);
    background: var(--card);
  }
  .mode-ic {
    display: inline-flex;
    flex: none;
    width: 44px;
    height: 44px;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: var(--card);
    color: var(--rose);
  }
  .mode-card.on .mode-ic {
    background: var(--rose);
    color: var(--bg);
  }
  .mode-body {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex: 1;
    min-width: 0;
  }
  .mode-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.88rem;
    font-weight: 700;
    color: var(--fg);
  }
  .mode-text {
    font-size: 0.88rem;
    line-height: 1.5;
    color: var(--fg-muted);
  }
  .mode-check {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    margin-top: 2px;
    border-radius: 50%;
    color: var(--rose);
  }
</style>
