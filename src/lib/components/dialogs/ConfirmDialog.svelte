<script lang="ts">
  import { app } from "../../stores/app.svelte.js";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import { t } from "../../i18n/index.js";

  const dialog = $derived(app.confirmDialog);
</script>

{#if dialog}
  <Modal title={dialog.title} width="420px" onclose={() => app.closeConfirm()}>
    {#snippet children()}
      <p class="confirm-message">{dialog.message}</p>
    {/snippet}

    {#snippet footer()}
      <Button variant="ghost" onclick={() => app.closeConfirm()}>{t("confirmDialog.cancel")}</Button>
      <Button
        variant="danger"
        onclick={() => { dialog.onConfirm(); app.closeConfirm(); }}
      >
        {dialog.confirmLabel ?? t("confirmDialog.confirm")}
      </Button>
    {/snippet}
  </Modal>
{/if}

<style>
  .confirm-message {
    margin: 0;
    font-size: 13px;
    color: var(--text);
    line-height: 1.6;
  }
</style>
