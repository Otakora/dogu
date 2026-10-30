<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { t } from "../../i18n/index.js";

  let maximized = $state(false);
  onMount(() => {
    const window = getCurrentWindow();
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const refresh = () => void window.isMaximized().then(value => { if (!disposed) maximized = value; }).catch(() => {});
    refresh();
    void window.onResized(refresh).then(stop => { if (disposed) stop(); else unlisten = stop; }).catch(() => {});
    return () => { disposed = true; unlisten?.(); };
  });

  function run(action: "minimize" | "maximize" | "close") {
    const window = getCurrentWindow();
    const result = action === "minimize" ? window.minimize() : action === "maximize" ? window.toggleMaximize() : window.close();
    void result.catch(error => console.error("Window control failed", error));
  }
</script>

<div class="window-controls" role="group" aria-label={t("window.controls")}>
  <button title={t("window.minimize")} aria-label={t("window.minimize")} onclick={() => run("minimize")}>
    <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8h10"/></svg>
  </button>
  <button title={t(maximized ? "window.restore" : "window.maximize")} aria-label={t(maximized ? "window.restore" : "window.maximize")} onclick={() => run("maximize")}>
    {#if maximized}<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M5 4V2.5h8v8h-1.5M3 5h8v8H3z"/></svg>
    {:else}<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 3h10v10H3z"/></svg>{/if}
  </button>
  <button class="close" title={t("window.close")} aria-label={t("window.close")} onclick={() => run("close")}>
    <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8"/></svg>
  </button>
</div>

<style>
  .window-controls { display: flex; flex: none; align-items: stretch; height: 34px; }
  button { width: 42px; display: grid; place-items: center; border: 0; padding: 0; color: var(--text-muted); background: transparent; cursor: pointer; }
  button:hover, button:focus-visible { color: var(--text); background: var(--surface-hover); outline: none; }
  button.close:hover, button.close:focus-visible { background: #c42b1c; color: white; }
  svg { width: 15px; height: 15px; fill: none; stroke: currentColor; stroke-width: 1.2; }
</style>
