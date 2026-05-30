<script lang="ts">
  import { app, go } from "../store.svelte";
  import Icon from "./Icon.svelte";
  import FlowHeader from "./FlowHeader.svelte";
  import Avatar from "./Avatar.svelte";

  let open = $state(0); // which key card is expanded
  let revealed = $state<boolean[]>([false, false, false]);
  let copied = $state(-1);

  const shares = $derived(app.outcome?.shares ?? []);

  function toggle(i: number) {
    open = open === i ? -1 : i;
  }

  async function copyShare(i: number) {
    try {
      await navigator.clipboard.writeText(shares[i]);
      copied = i;
      setTimeout(() => (copied = copied === i ? -1 : copied), 1600);
    } catch {
      /* clipboard unavailable */
    }
  }

  function give(i: number) {
    app.guardians[i].shared = true;
    const next = app.guardians.findIndex((g) => !g.shared);
    open = next;
  }

  const handedOut = $derived(app.guardians.filter((g) => g.shared).length);
  const allShared = $derived(
    app.guardians.every((g) => g.hint.trim().length > 0 && g.shared),
  );
</script>

<div class="card">
  <FlowHeader label="Keep something safe" total={4} current={3} onBack={() => go("password")} />

  {#if shares.length < 3}
    <div class="icon-badge"><Icon name="users" size={26} /></div>
    <h2>Let's seal your file first</h2>
    <p class="lead">We need to seal your file before handing out keys.</p>
    <button class="btn" style="margin-top:20px;" onclick={() => go("password")}>
      <Icon name="arrow-left" /> Go back
    </button>
  {:else}
    <h2>Hand out the keys</h2>
    <p class="lead">
      Each person gets one <strong>recovery key</strong> — a list of words. Read it
      out or copy it across however you trust most. Any <strong>2 of 3</strong> can
      bring your file back — one alone can't.
    </p>

    <div class="stack" style="gap: 11px; margin-top: 22px;">
      {#each app.guardians as g, i}
        <div class="keycard">
          <button class="keycard-head" type="button" onclick={() => toggle(i)}>
            <Avatar name={g.hint || "?"} seed={i} size={44} />
            <div style="flex:1; min-width:0; text-align:left;">
              <div style="font-weight:700;">{g.hint.trim() || `Person ${i + 1}`}</div>
              <div class="muted" style="font-size:0.82rem;">Recovery key {i + 1} of 3</div>
            </div>
            {#if g.shared}
              <span class="pill pill-success" style="display:inline-flex;align-items:center;gap:4px;">
                <Icon name="check" size={13} stroke={2.5} /> given
              </span>
            {:else}
              <span class="pill">not yet</span>
            {/if}
            <span class="ns-ic" style="color:var(--fg-hint); transform:rotate({open === i ? -90 : 90}deg); transition:transform .2s;">
              <Icon name="arrow-right" size={18} />
            </span>
          </button>

          {#if open === i}
            <div class="keycard-body">
              <label for={`hint-${i}`}>Nickname or hint</label>
              <input
                id={`hint-${i}`}
                type="text"
                placeholder="e.g. My daughter Rosa"
                bind:value={g.hint}
              />

              <div class="note" style="margin-top:14px; padding:14px 16px;">
                <div style="display:flex; align-items:center; justify-content:space-between; margin-bottom:10px;">
                  <span class="eyebrow">Recovery key</span>
                </div>
                {#if revealed[i]}
                  <div class="mnemonic">{shares[i]}</div>
                {:else}
                  <button class="words-hidden" onclick={() => (revealed[i] = true)}>
                    <Icon name="eye" size={18} /> Tap to show the recovery key
                  </button>
                {/if}
              </div>

              <p class="muted" style="display:flex; gap:7px; align-items:flex-start; margin:12px 0 14px;">
                <span style="display:inline-flex; margin-top:2px;"><Icon name="shield" size={14} /></span>
                One key alone opens nothing — it takes any 2 together. Safe to give to {g.hint.trim().split(" ")[0] || "them"}.
              </p>

              <button class="btn btn-dark" onclick={() => copyShare(i)}>
                <Icon name={copied === i ? "check" : "file"} size={18} />
                {copied === i ? "Copied" : "Copy key"}
              </button>

              {#if !g.shared}
                <button class="btn" style="margin-top:12px;" disabled={!revealed[i]} onclick={() => give(i)}>
                  <Icon name="check" /> Mark as given
                </button>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>

    <div class="note-sunken" style="align-items:center; margin-top:18px;">
      <div style="flex:1;">
        <div style="font-weight:700; color:var(--fg); margin-bottom:6px;">{handedOut} of 3 keys handed out</div>
        <div style="height:6px; border-radius:999px; background:var(--border); overflow:hidden;">
          <div style="height:100%; width:{(handedOut / 3) * 100}%; background:{allShared ? 'var(--success)' : 'var(--rose)'}; border-radius:999px; transition:width .4s ease;"></div>
        </div>
      </div>
      {#if allShared}<span class="ns-ic" style="color:var(--success);"><Icon name="check" size={20} /></span>{/if}
    </div>

    <button class="btn" style="margin-top: 18px;" disabled={!allShared} onclick={() => go("protect-success")}>
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
    gap: 14px;
    padding: 14px 16px;
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
  .words-hidden {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 16px;
    border: 1px dashed var(--border);
    border-radius: var(--r-md);
    background: var(--bg);
    color: var(--fg-muted);
    font-weight: 700;
    font-size: 0.9rem;
    cursor: pointer;
  }
  .words-hidden:hover { color: var(--rose); border-color: var(--rose); }
</style>
