<script lang="ts">
  import Shell from "./lib/components/shell/Shell.svelte";
  import FingerprintDialog from "./lib/components/connections/FingerprintDialog.svelte";
  import NotificationStack from "./lib/components/ui/NotificationStack.svelte";
  import { app } from "./lib/stores/app.svelte.js";
</script>

<Shell />

{#if app.fingerprintPrompt}
  <FingerprintDialog
    fingerprint={app.fingerprintPrompt.fingerprint}
    profileLabel={app.fingerprintPrompt.profileLabel}
    onTrust={() => app.resolveFingerprint(true)}
    onReject={() => app.resolveFingerprint(false)}
  />
{/if}

<NotificationStack />
