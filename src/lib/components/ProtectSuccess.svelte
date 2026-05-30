<script lang="ts">
  import { app, go, resetProtect } from "../store.svelte";
  import Icon from "./Icon.svelte";
  import NanaLogo from "./NanaLogo.svelte";
  import Avatar from "./Avatar.svelte";

  const hints = $derived(app.guardians.map((g) => g.hint.trim()).filter(Boolean));
  const fileName = app.fileName;

  function home() {
    resetProtect();
    go("home");
  }
</script>

<div class="card center">
  <div class="seal-mark pop" style="margin-bottom:14px;">
    <NanaLogo size={92} />
    <span
      style="position:absolute; right:0; bottom:0; width:30px; height:30px; border-radius:50%; background:var(--success); color:#fff; display:flex; align-items:center; justify-content:center; box-shadow:0 0 0 3px var(--bg);"
    >
      <Icon name="check" size={17} stroke={2.6} />
    </span>
  </div>

  <span class="badge-success"><Icon name="check" size={13} stroke={2.6} /> Sealed &amp; safe</span>

  <h1 style="margin-top:14px;">Your file is safe.</h1>
  <p class="lead">
    <strong style="color:var(--fg);">{fileName}</strong> is sealed and split into 3
    keys — findable only by you.
  </p>

  <div class="card" style="text-align:left; padding:6px 20px; box-shadow:none; border-color:var(--border); margin:20px 0; animation:none;">
    <div class="meta-row">
      <span class="mr-ic"><Icon name="lock" size={18} /></span>
      <span class="mr-label">Sealed with your key</span>
      <span class="mr-val">Only you</span>
    </div>
    <div class="meta-row">
      <span class="mr-ic"><Icon name="users" size={18} /></span>
      <span class="mr-label">Shared with</span>
      <span class="mr-val">{hints.length} people</span>
    </div>
    <div class="meta-row">
      <span class="mr-ic"><Icon name="recover" size={18} /></span>
      <span class="mr-label">To recover, you need</span>
      <span class="mr-val">Any 2 of 3</span>
    </div>
  </div>

  {#if hints.length}
    <div class="avatar-stack" style="justify-content:center; margin-bottom:18px;">
      {#each hints as h, i}
        <Avatar name={h} seed={i} size={40} ring="var(--bg)" />
      {/each}
    </div>
  {/if}

  <div class="note-sunken" style="text-align:left;">
    <span class="ns-ic" style="color:var(--rose);"><Icon name="recover" size={20} /></span>
    <span>
      If you ever lose your file, just ask any <b>2 of those 3 people</b> for the key
      you gave them, then follow the <b>“Recover a lost secret”</b> path. I'll put it
      back together for you.
    </span>
  </div>

  <button class="btn" style="margin-top: 24px;" onclick={home}><Icon name="home" /> Back to home</button>
</div>
