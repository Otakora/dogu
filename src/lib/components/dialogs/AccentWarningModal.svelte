<script lang="ts">
  import type { AccentIssue } from "../../utils/ascii.js";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import { t } from "../../i18n/index.js";

  type Props = {
    issues: AccentIssue[];
    /** Renames the fixable files and proceeds. Only shown when every issue is fixable. */
    onRename: () => void;
    onCancel: () => void;
    busy?: boolean;
  };

  let { issues, onRename, onCancel, busy = false }: Props = $props();

  const unfixable = $derived(issues.filter((i) => !i.fixable));
  const allFixable = $derived(unfixable.length === 0);
</script>

<Modal title={t("accents.title")} width="560px" onclose={onCancel}>
  {#snippet children()}
    <p class="intro">{t("accents.explain")}</p>

    {#if allFixable}
      <p class="intro">{t("accents.renamePrompt")}</p>
    {:else}
      <p class="intro danger">{t("accents.unfixable")}</p>
    {/if}

    <div class="rename-list scrollbar-thin">
      {#each issues as issue}
        <div class="rename-row" class:blocked={!issue.fixable}>
          <span class="old">{issue.oldName}</span>
          <span class="arrow">→</span>
          {#if issue.fixable}
            <span class="new">{issue.newName}</span>
          {:else}
            <span class="new blocked-tag">{t("accents.cannotRename")}</span>
          {/if}
        </div>
      {/each}
    </div>
  {/snippet}

  {#snippet footer()}
    <Button variant="ghost" onclick={onCancel} disabled={busy}>{t("common.cancel")}</Button>
    {#if allFixable}
      <Button variant="primary" onclick={onRename} disabled={busy}>
        {t("accents.renameAndContinue")}
      </Button>
    {/if}
  {/snippet}
</Modal>

<style>
  .intro {
    margin: 0 0 10px;
    font-size: 13px;
    color: var(--text);
    line-height: 1.55;

    &.danger { color: var(--danger); }
  }

  .rename-list {
    max-height: 240px;
    overflow-y: auto;
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 4px;
    scrollbar-width: thin;
    scrollbar-color: var(--line-strong) transparent;
  }

  .rename-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    font-size: 12px;
    font-family: var(--font-mono, monospace);
    border-radius: 4px;

    &:hover { background: var(--surface-hover); }
    &.blocked { color: var(--danger); }
  }

  .old { color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .arrow { color: var(--text-faint); flex-shrink: 0; }
  .new { color: var(--accent); font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .new.blocked-tag { color: var(--danger); font-weight: 500; font-style: italic; }
</style>
