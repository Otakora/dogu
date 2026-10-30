<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  type Direction = "North" | "South" | "East" | "West" | "NorthEast" | "NorthWest" | "SouthEast" | "SouthWest";
  const directions: Direction[] = ["North", "South", "East", "West", "NorthEast", "NorthWest", "SouthEast", "SouthWest"];
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
</script>

{#if !maximized}
  {#each directions as direction}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="resize-handle" class:n={direction === "North"} class:s={direction === "South"} class:e={direction === "East"} class:w={direction === "West"} class:ne={direction === "NorthEast"} class:nw={direction === "NorthWest"} class:se={direction === "SouthEast"} class:sw={direction === "SouthWest"}
      aria-hidden="true" onmousedown={(event) => { if (event.button === 0) { event.preventDefault(); void getCurrentWindow().startResizeDragging(direction).catch(() => {}); } }}></div>
  {/each}
{/if}

<style>
  .resize-handle { position: fixed; z-index: 9999; }
  .n, .s { left: 8px; right: 8px; height: 5px; cursor: ns-resize; }
  .n { top: 0; } .s { bottom: 0; }
  .e, .w { top: 8px; bottom: 8px; width: 5px; cursor: ew-resize; }
  .e { right: 0; } .w { left: 0; }
  .ne, .nw, .se, .sw { width: 9px; height: 9px; }
  .ne { top: 0; right: 0; cursor: nesw-resize; }
  .sw { bottom: 0; left: 0; cursor: nesw-resize; }
  .nw { top: 0; left: 0; cursor: nwse-resize; }
  .se { bottom: 0; right: 0; cursor: nwse-resize; }
</style>
