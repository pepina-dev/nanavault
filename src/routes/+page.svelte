<script lang="ts">
  import { app } from "$lib/store.svelte";
  import Home from "$lib/components/Home.svelte";
  import ProtectUpload from "$lib/components/ProtectUpload.svelte";
  import RecoveryModePick from "$lib/components/RecoveryModePick.svelte";
  import BackupCode from "$lib/components/BackupCode.svelte";
  import NsecKey from "$lib/components/NsecKey.svelte";
  import MasterPassword from "$lib/components/MasterPassword.svelte";
  import Working from "$lib/components/Working.svelte";
  import ShareGuardians from "$lib/components/ShareGuardians.svelte";
  import ProtectSuccess from "$lib/components/ProtectSuccess.svelte";
  import Recover from "$lib/components/Recover.svelte";
  import RecoverSuccess from "$lib/components/RecoverSuccess.svelte";
</script>

<main class="app-shell">
  {#if app.screen === "home"}
    <Home />
  {:else if app.screen === "protect"}
    <ProtectUpload />
  {:else if app.screen === "mode"}
    <RecoveryModePick />
  {:else if app.screen === "backup-code"}
    <BackupCode />
  {:else if app.screen === "nsec"}
    <NsecKey />
  {:else if app.screen === "password"}
    <MasterPassword />
  {:else if app.screen === "seal"}
    <Working mode="seal" />
  {:else if app.screen === "share"}
    <ShareGuardians />
  {:else if app.screen === "protect-success"}
    <ProtectSuccess />
  {:else if app.screen === "recover"}
    <Recover />
  {:else if app.screen === "recovering"}
    <Working mode="recover" />
  {:else if app.screen === "recover-success"}
    <RecoverSuccess />
  {/if}
</main>
