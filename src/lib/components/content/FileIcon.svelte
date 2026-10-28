<script lang="ts">
  import type { EntryDto } from "../../types/index.js";
  import { fileIconKind } from "../../utils/file-icons.js";
  let { entry, size = 16 }: { entry: EntryDto; size?: number } = $props();
  const kind = $derived(fileIconKind(entry));
</script>

<svg class="entry-icon entry-icon--{kind}" class:entry-icon--ghost={entry.isGhost}
  width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor"
  stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  {#if kind === "folder"}
    <path d="M3 6a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>
    <path d="M3 10h18"/>
  {:else if kind === "archive"}
    <rect x="4" y="3" width="16" height="18" rx="2"/><path d="M9 3v3h2v3H9v3h2v3H9v3h3v-3"/><path d="M14 7h3M14 10h3"/>
  {:else if kind === "text"}
    <path d="M6 3h8l4 4v14H6z"/><path d="M14 3v5h4M9 12h6M9 15h6M9 18h4"/>
  {:else if kind === "disc"}
    <circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="2.4"/><path d="M6 9a7 7 0 0 1 4-3M18 15a7 7 0 0 1-4 3"/>
  {:else if kind === "game"}
    <path d="M5 4h14v16H5z"/><path d="M8 4v6h8V4M8 15h8M9 18h6"/>
  {:else if kind === "image"}
    <rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="8" cy="9" r="1.5"/><path d="m4 17 5-5 3 3 3-3 5 5"/>
  {:else if kind === "audio"}
    <path d="M10 18V6l9-2v12"/><circle cx="7" cy="18" r="2.5"/><circle cx="16" cy="16" r="2.5"/>
  {:else}
    <rect x="5" y="4" width="14" height="16" rx="2"/><path d="M8 8h8M8 12h3M8 16h8"/><circle cx="15" cy="12" r="1"/>
  {/if}
</svg>

<style>
  .entry-icon { flex-shrink: 0; color: var(--text-muted); }
  .entry-icon--folder { color: #d99a27; }
  .entry-icon--archive { color: #b780db; }
  .entry-icon--text { color: #6599c5; }
  .entry-icon--disc { color: #45a8a0; }
  .entry-icon--game { color: #d18555; }
  .entry-icon--image { color: #8cae59; }
  .entry-icon--audio { color: #ce7f9c; }
  .entry-icon--ghost { stroke-dasharray: 3 2; opacity: .82; }
</style>
