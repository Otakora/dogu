<script lang="ts">
  import DestinationPickerModal from "./DestinationPickerModal.svelte";
  import Button from "../ui/Button.svelte";
  import { t } from "../../i18n/index.js";

  type Props = {
    /** Currently selected path (local or remote://...). Null = nothing selected yet. */
    path: string | null;
    /** Human-readable label for the path. */
    label: string | null;
    onchange: (path: string, label: string) => void;
    disabled?: boolean;
  };

  let { path, label, onchange, disabled = false }: Props = $props();

  let showPicker = $state(false);

  function isRemotePath(p: string) {
    return p.startsWith("remote://");
  }

  function iconFor(p: string) {
    if (isRemotePath(p)) return "🌐";
    return "📁";
  }

  function truncatePath(p: string) {
    if (isRemotePath(p)) {
      // Show only the logical portion after session-id
      const rest = p.replace(/^remote:\/\/[^/]+/, "");
      return rest || "/";
    }
    // Show last two segments on long paths
    const parts = p.replace(/\\/g, "/").split("/").filter(Boolean);
    if (parts.length <= 2) return p.replace(/\\/g, "/");
    return "…/" + parts.slice(-2).join("/");
  }
</script>

<div class="dest-field" class:disabled>
  {#if path}
    <span class="dest-icon">{iconFor(path)}</span>
    <div class="dest-info">
      {#if label}<span class="dest-label">{label}</span>{/if}
      <span class="dest-path">{truncatePath(path)}</span>
    </div>
  {:else}
    <span class="dest-icon">📁</span>
    <span class="dest-placeholder">{t("common.selectDestination")}</span>
  {/if}

  <Button
    size="sm"
    variant="outline"
    {disabled}
    onclick={() => (showPicker = true)}
  >
    {t("destPicker.change")}
  </Button>
</div>

{#if showPicker}
  <DestinationPickerModal
    onselect={(result) => { onchange(result.path, result.label); showPicker = false; }}
    onclose={() => (showPicker = false)}
  />
{/if}

<style>
  .dest-field {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    background: var(--surface);
    min-height: 36px;

    &.disabled { opacity: 0.5; pointer-events: none; }
  }

  .dest-icon { font-size: 14px; flex-shrink: 0; }

  .dest-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    gap: 1px;
  }

  .dest-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dest-path {
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: var(--font-mono, monospace);
  }

  .dest-placeholder {
    flex: 1;
    font-size: 12px;
    color: var(--text-faint);
    font-style: italic;
  }
</style>
