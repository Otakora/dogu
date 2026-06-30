<script lang="ts">
  import { app } from "../../stores/app.svelte.js";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";

  let logsExpanded = $state(false);
  let logsEl = $state<HTMLElement | undefined>(undefined);

  const p = $derived(app.progress);

  $effect(() => {
    if (logsEl && p?.logs.length) {
      logsEl.scrollTop = logsEl.scrollHeight;
    }
  });
</script>

{#if p && p.showDialog}
  <Modal
    title={p.title}
    width="520px"
    onclose={p.done ? () => app.clearProgress() : undefined}
  >
    {#snippet children()}
      <div class="pd-body">
        <!-- Progress bar -->
        <div class="pd-bar-wrap" role="progressbar" aria-valuenow={p.progress} aria-valuemin={0} aria-valuemax={100}>
          <div class="pd-bar" style="width:{p.progress}%" class:pd-bar--done={p.done} class:pd-bar--error={p.done && !p.success}></div>
        </div>

        <!-- Message -->
        <p class="pd-message">
          {#if p.done}
            {p.resultMessage || (p.success ? "Completed." : "Failed.")}
          {:else}
            {p.message || "Working…"}
          {/if}
        </p>

        <!-- Status icon when done -->
        {#if p.done}
          <div class="pd-status" class:pd-status--ok={p.success} class:pd-status--err={!p.success}>
            {#if p.success}
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>
              {p.statusMessageOnSuccess ?? "Done"}
            {:else}
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
              {p.statusMessageOnFailure ?? "Error"}
            {/if}
          </div>
        {/if}

        <!-- Log output (collapsible) -->
        {#if p.logs.length > 0}
          <button class="pd-log-toggle" onclick={() => (logsExpanded = !logsExpanded)}>
            {logsExpanded ? "▲" : "▼"} Output ({p.logs.length} lines)
          </button>
          {#if logsExpanded}
            <div class="pd-logs scrollbar-thin" bind:this={logsEl}>
              {#each p.logs as line}
                <div class="pd-log-line">{line}</div>
              {/each}
            </div>
          {/if}
        {/if}
      </div>
    {/snippet}

    {#snippet footer()}
      {#if p.done}
        <Button variant="primary" onclick={() => app.clearProgress()}>Close</Button>
      {:else}
        <span class="pd-running">
          <span class="pd-spinner"></span> Running…
        </span>
      {/if}
    {/snippet}
  </Modal>
{/if}

<style>
  .pd-body {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }

  .pd-bar-wrap {
    height: 6px;
    background: var(--line);
    border-radius: 3px;
    overflow: hidden;
  }

  .pd-bar {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.3s ease;
  }

  .pd-bar--done { background: var(--accent); }
  .pd-bar--error { background: var(--danger); }

  .pd-message {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
    min-height: 18px;
    word-break: break-all;
  }

  .pd-status {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    font-weight: 500;
    padding: 6px 10px;
    border-radius: 6px;
  }

  .pd-status--ok  { background: var(--accent-soft); color: var(--accent); border: 1px solid var(--accent-border); }
  .pd-status--err { background: var(--danger-soft); color: var(--danger); border: 1px solid var(--danger-border); }

  .pd-log-toggle {
    background: none;
    border: none;
    font: inherit;
    font-size: 12px;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0;
    text-align: left;

    &:hover { color: var(--text); }
  }

  .pd-logs {
    max-height: 200px;
    overflow-y: auto;
    background: var(--surface-alt);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 8px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-muted);
    line-height: 1.6;
  }

  .pd-log-line { white-space: pre-wrap; word-break: break-all; }

  .pd-running {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .pd-spinner {
    display: inline-block;
    width: 14px;
    height: 14px;
    border: 2px solid var(--line-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
</style>
