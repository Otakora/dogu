<script lang="ts">
  import { setContext, untrack } from "svelte";
  import { app } from "../../stores/app.svelte.js";
  import type { PaneView } from "../../stores/app.svelte.js";
  import TabBar from "./TabBar.svelte";
  import Toolbar from "./Toolbar.svelte";
  import ContentPanel from "../content/ContentPanel.svelte";

  type Props = {
    paneIdx: number;
    onDelete: (paths: string[]) => void;
    onCopy: (paths: string[]) => void;
    onCut: (paths: string[]) => void;
    onPaste: () => void;
    onExtractHere: (paths: string[]) => void;
    onExtractToFolder: (paths: string[]) => void;
    onExtractTo: (paths: string[]) => void;
    onCompressQuick: (paths: string[]) => void;
    onCompress: (paths: string[]) => void;
    onChd: (paths: string[]) => void;
    onM3u: (paths: string[]) => void;
    onProperties: (paths: string[]) => void;
    onOpenWith: (path: string) => void;
  };

  let {
    paneIdx,
    onDelete, onCopy, onCut, onPaste,
    onExtractHere, onExtractToFolder, onExtractTo,
    onCompressQuick, onCompress, onChd, onM3u,
    onProperties, onOpenWith,
  }: Props = $props();

  // Set per-pane context for all children (paneIdx is stable — untrack is correct here)
  const paneView: PaneView = untrack(() => app.getPaneView(paneIdx));
  setContext<PaneView>("pane", paneView);

  // ContentPanel ref for imperative calls from Pane
  let contentPanel = $state<ReturnType<typeof ContentPanel> | undefined>(undefined);

  // Exported methods so Shell can call them
  export function refresh()            { contentPanel?.refresh(); }
  export function createFolder()       { contentPanel?.createFolder(); }
  export function createFile()         { contentPanel?.createFile(); }
  export function search(q: string)    { contentPanel?.search(q); }
  export function clearSearch()        { contentPanel?.clearSearch(); }

  const isFocused = $derived(app.focusedPaneIdx === paneIdx);
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="pane"
  class:pane--focused={isFocused}
  class:pane--split={app.isSplit}
  class:pane--left={paneIdx === 0}
  class:pane--right={paneIdx === 1}
  style="--content-scale: {paneView.contentZoom}"
  role="region"
  aria-label="Pane {paneIdx + 1}"
  onmousedown={() => app.focusPane(paneIdx)}
>
  <TabBar />

  <Toolbar
    onRefresh={() => refresh()}
    onNewFolder={() => createFolder()}
    onNewFile={() => createFile()}
    onSearch={(q) => search(q)}
    onClearSearch={() => clearSearch()}
  />

  <div class="pane-content">
    <ContentPanel
      bind:this={contentPanel}
      {onDelete}
      {onCopy}
      {onCut}
      {onPaste}
      {onExtractHere}
      {onExtractToFolder}
      {onExtractTo}
      {onCompressQuick}
      {onCompress}
      {onChd}
      {onM3u}
      {onProperties}
      {onOpenWith}
    />
  </div>
</div>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    position: relative;
  }

  .pane--split.pane--focused::before {
    content: "";
    position: absolute;
    inset: 0;
    border: 1px solid color-mix(in srgb, var(--accent) 66%, transparent);
    box-shadow:
      inset 0 1px 0 color-mix(in srgb, var(--accent) 20%, transparent),
      inset var(--focus-edge-x, 3px) 0 0 color-mix(in srgb, var(--accent) 72%, transparent);
    pointer-events: none;
    z-index: 10;
    border-radius: 0;
  }

  .pane--right {
    --focus-edge-x: -3px;
  }

  .pane-content {
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
  }
</style>
