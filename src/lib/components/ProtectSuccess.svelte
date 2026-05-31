<script lang="ts">
  import { app, go, resetProtect } from "../store.svelte";
  import { THRESHOLD, SHARE_COUNT } from "../api";
  import Icon from "./Icon.svelte";
  import Avatar from "./Avatar.svelte";
  import SealMark from "./SealMark.svelte";

  const hints = $derived(
    app.guardians.map((g) => g.hint.trim()).filter(Boolean),
  );
  const isText = $derived(app.protectInput === "text");
  // The noun for the protected thing, and the bolded subject of the success
  // line: a file has its name; a typed secret has none, so it gets a stand-in.
  const subjectNoun = $derived(isText ? "secret" : "file");
  const subject = $derived(isText ? "Your secret" : app.fileName);

  // Easy mode "protects"; advanced mode "encrypts".
  const sealedAdj = $derived(
    app.recoveryMode === "easy" ? "Protected" : "Encrypted",
  );
  const keyNoun = $derived(app.recoveryMode === "easy" ? "backup code" : "key");

  function home() {
    resetProtect();
    go("home");
  }
</script>

<div class="card center">
  <SealMark size={92} check />

  <span class="badge-success"
    ><Icon name="check" size={13} stroke={2.6} /> {sealedAdj} &amp; safe</span
  >

  <h1 style="margin-top:14px;">Your {subjectNoun} is safe.</h1>
  <p class="lead">
    <strong style="color:var(--fg);">{subject}</strong>
    is {sealedAdj.toLowerCase()} and split into
    {SHARE_COUNT} keys shared with people you trust.
  </p>

  <div class="card card-inset" style="margin:20px 0;">
    <div class="meta-row">
      <span class="mr-ic"><Icon name="lock" size={18} /></span>
      <span class="mr-label">{sealedAdj} with your {keyNoun}</span>
      <span class="mr-val">Only you</span>
    </div>
    <div class="meta-row">
      <span class="mr-ic"><Icon name="users" size={18} /></span>
      <span class="mr-label">Shared with</span>
      <span class="mr-val">{SHARE_COUNT} people</span>
    </div>
    <div class="meta-row">
      <span class="mr-ic"><Icon name="recover" size={18} /></span>
      <span class="mr-label">To recover, you need</span>
      <span class="mr-val">Any {THRESHOLD} of {SHARE_COUNT}</span>
    </div>
  </div>

  {#if hints.length}
    <div
      class="avatar-stack"
      style="justify-content:center; margin-bottom:18px;"
    >
      {#each hints as h, i}
        <Avatar name={h} seed={i} size={40} ring="var(--bg)" />
      {/each}
    </div>
  {/if}

  <div class="note-sunken" style="text-align:left;">
    <span class="ns-ic" style="color:var(--rose);"
      ><Icon name="recover" size={20} /></span
    >
    <span>
      If you ever lose your {subjectNoun}, just ask any
      <b>{THRESHOLD} of those {SHARE_COUNT} people</b> for the key you gave
      them, then follow the <b>“Recover secret”</b> path. I'll put it back together
      for you.
    </span>
  </div>

  <button class="btn" style="margin-top: 24px;" onclick={home}
    ><Icon name="home" /> Back to home</button
  >
</div>
