<script lang="ts">
  import { app, go } from "../store.svelte";
  import { THRESHOLD, SHARE_COUNT } from "../api";
  import Icon from "./Icon.svelte";
  import FlowHeader from "./FlowHeader.svelte";
  import Avatar from "./Avatar.svelte";

  let open = $state(0); // which key card is expanded
  let copied = $state(-1);
  let copyFailed = $state(-1);

  const shares = $derived(app.outcome?.shares ?? []);

  function toggle(i: number) {
    open = open === i ? -1 : i;
  }

  async function copyShare(i: number) {
    copyFailed = -1;
    try {
      await navigator.clipboard.writeText(shares[i]);
      copied = i;
      setTimeout(() => (copied = copied === i ? -1 : copied), 1600);
    } catch {
      // clipboard unavailable — tell the user so they can copy it by hand
      copyFailed = i;
      setTimeout(() => (copyFailed = copyFailed === i ? -1 : copyFailed), 3000);
    }
  }

  // When a code is marked shared, collapse it and move on to the next unshared
  // person (or close everything once they're all done).
  function onShareToggle(i: number) {
    if (app.guardians[i].shared) {
      open = app.guardians.findIndex((g) => !g.shared);
    }
  }

  const handedOut = $derived(app.guardians.filter((g) => g.shared).length);
  // Naming each person is just a memory aid (avatar + label), never used in the
  // crypto — so handing out every code is all it takes to finish.
  const allShared = $derived(app.guardians.every((g) => g.shared));

  // Wording + step dots follow the chosen recovery mode.
  const protectVerb = $derived(
    app.recoveryMode === "easy" ? "protect" : "encrypt",
  );
  const total = 5;
  const secretScreen = $derived(
    app.recoveryMode === "easy" ? "backup-code" : "password",
  );
</script>

<div class="card">
  <FlowHeader
    label="Keep something safe"
    {total}
    current={total - 1}
    onBack={() => go(secretScreen)}
  />

  {#if shares.length < SHARE_COUNT}
    <div class="icon-badge"><Icon name="users" size={26} /></div>
    <h2>Let's {protectVerb} your file first</h2>
    <p class="lead">
      We need to {protectVerb} your file before handing out keys.
    </p>
    <button
      class="btn"
      style="margin-top:20px;"
      onclick={() => go(secretScreen)}
    >
      <Icon name="arrow-left" /> Go back
    </button>
  {:else}
    <h2>Hand out the keys</h2>
    <p class="lead">
      Each person gets one <strong>recovery code</strong> — a list of words.
      Read it out or copy it across however you trust most. Any
      <strong>{THRESHOLD} of {SHARE_COUNT}</strong> can bring your secret file back
      — one alone can't.
    </p>

    <div class="stack" style="gap: 11px; margin-top: 22px;">
      {#each app.guardians as g, i}
        <div class="keycard">
          <div class="keycard-head">
            <Avatar name={g.hint || "?"} seed={i} size={44} />
            {#if open === i}
              <!-- Editing inline reuses the name's slot, so the open card
                   doesn't need a separate field below. -->
              <input
                class="name-input"
                type="text"
                placeholder={`Person  ${i + 1}`}
                aria-label={`Name for person ${i + 1}`}
                bind:value={g.hint}
              />
            {:else}
              <button class="head-main" type="button" onclick={() => toggle(i)}>
                <span style="font-weight:700;">
                  {g.hint.trim() || `Person ${i + 1}`}
                </span>
                <span class="muted" style="font-size:0.82rem;">
                  Recovery code {i + 1} of {SHARE_COUNT}
                </span>
              </button>
            {/if}
            {#if g.shared}
              <span
                class="pill pill-success"
                style="display:inline-flex;align-items:center;gap:4px;"
              >
                <Icon name="check" size={13} stroke={2.5} /> shared
              </span>
            {:else}
              <span class="pill">not yet</span>
            {/if}
            <button
              class="chevron-btn"
              type="button"
              aria-label={open === i ? "Collapse" : "Expand"}
              onclick={() => toggle(i)}
            >
              <span
                class="ns-ic"
                style="color:var(--fg-hint); transform:rotate({open === i
                  ? 180
                  : 0}deg); transition:transform .2s;"
              >
                <Icon name="chevron-down" size={18} />
              </span>
            </button>
          </div>

          {#if open === i}
            <div class="keycard-body">
              <div class="note" style="padding:12px 16px;">
                <div style="display:flex; justify-content:flex-end;">
                  <button
                    type="button"
                    class="reveal-link"
                    onclick={() => copyShare(i)}
                  >
                    <Icon name={copied === i ? "check" : "copy"} size={13} />
                    {copied === i ? "Copied" : "Copy"}
                  </button>
                </div>
                <div class="mnemonic" style="margin-top:4px;">{shares[i]}</div>
              </div>
              {#if copyFailed === i}
                <p class="muted" style="margin:8px 0 0; font-size:0.82rem;">
                  Couldn't copy automatically — tap the words above to select
                  them, then copy by hand.
                </p>
              {/if}

              <label class="checkrow" style="margin-top:14px;">
                <input
                  type="checkbox"
                  bind:checked={g.shared}
                  onchange={() => onShareToggle(i)}
                />
                I've shared this recovery code
              </label>
            </div>
          {/if}
        </div>
      {/each}
    </div>

    <div class="note-sunken" style="align-items:center; margin-top:18px;">
      <div style="flex:1;">
        <div style="font-weight:700; color:var(--fg); margin-bottom:6px;">
          {handedOut} of {SHARE_COUNT} keys handed out
        </div>
        <div
          style="height:6px; border-radius:999px; background:var(--border); overflow:hidden;"
        >
          <div
            style="height:100%; width:{(handedOut / SHARE_COUNT) *
              100}%; background:{allShared
              ? 'var(--success)'
              : 'var(--rose)'}; border-radius:999px; transition:width .4s ease;"
          ></div>
        </div>
      </div>
      {#if allShared}<span class="ns-ic" style="color:var(--success);"
          ><Icon name="check" size={20} /></span
        >{/if}
    </div>

    <button
      class="btn"
      style="margin-top: 18px;"
      disabled={!allShared}
      onclick={() => go("protect-success")}
    >
      <Icon name="check" /> All done
    </button>
  {/if}
</div>

<style>
  .keycard {
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    background: var(--bg);
    overflow: hidden;
  }
  .keycard-head {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
  }
  .head-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    text-align: left;
    background: transparent;
    border: none;
    padding: 0;
    cursor: pointer;
  }
  .head-main > span {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name-input {
    flex: 1;
    min-width: 0;
    padding: 9px 12px;
    font-size: 0.95rem;
    font-weight: 700;
    border-radius: var(--r-md);
  }
  .chevron-btn {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    background: transparent;
    border: none;
    cursor: pointer;
  }
  .keycard-body {
    padding: 4px 16px 18px;
    animation: nv-fade-up 0.2s ease both;
  }
  .mnemonic {
    font-family: ui-monospace, monospace;
    font-size: 13px;
    line-height: 1.7;
    color: var(--fg);
    word-spacing: 2px;
    word-break: break-word;
    user-select: all;
  }
  .reveal-link {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 6px 0;
    background: none;
    border: none;
    color: var(--fg-muted);
    font-weight: 700;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    cursor: pointer;
  }
  .reveal-link:hover {
    color: var(--fg);
    text-decoration: underline;
  }
</style>
