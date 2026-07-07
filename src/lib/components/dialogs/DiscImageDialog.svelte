<script lang="ts">
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import { t } from "../../i18n/index.js";
  import type { DiscImageOptionsPayload } from "../../types/index.js";
  import { basenameOf, dirnameOf, joinPath, stemOf } from "../../utils/ghosts.js";

  export type DiscImageMode =
    | "cso-convert"
    | "cso-restore"
    | "xiso-pack"
    | "xiso-unpack"
    | "rvz-convert"
    | "rvz-restore";

  type Props = {
    mode: DiscImageMode;
    sources: string[];
    onclose: () => void;
    onConfirm: (sources: string[], options: DiscImageOptionsPayload) => void;
  };

  let { mode, sources, onclose, onConfirm }: Props = $props();

  let destinationMode = $state<"same" | "custom">("same");
  let destinationPath = $state("");
  let deleteOriginals = $state(false);
  let overwrite = $state(false);
  let compressionLevel = $state(9);

  const title = $derived(t(`discImage.${mode}.title`, { count: sources.length }));
  const action = $derived(t(`discImage.${mode}.action`));
  const showCompression = $derived(mode === "cso-convert" || mode === "rvz-convert");
  const outputKind = $derived(mode === "xiso-unpack" ? "folder" : "file");

  function xisoStem(path: string): string {
    const stem = stemOf(basenameOf(path));
    return stem.endsWith(".xiso") ? stem.slice(0, -5) : stem;
  }

  function previewFor(path: string): string {
    const baseDir = destinationMode === "custom" && destinationPath.trim()
      ? destinationPath.trim()
      : dirnameOf(path);
    if (mode === "cso-convert") return joinPath(baseDir, `${stemOf(basenameOf(path))}.cso`);
    if (mode === "cso-restore") return joinPath(baseDir, `${stemOf(basenameOf(path))}.iso`);
    if (mode === "rvz-convert") return joinPath(baseDir, `${stemOf(basenameOf(path))}.rvz`);
    if (mode === "rvz-restore") return joinPath(baseDir, `${stemOf(basenameOf(path))}.iso`);
    if (mode === "xiso-pack") return joinPath(baseDir, `${xisoStem(path)}.xiso.iso`);
    return joinPath(baseDir, xisoStem(path));
  }

  function confirm() {
    onConfirm(sources, {
      destinationMode,
      destinationPath: destinationMode === "custom" ? destinationPath.trim() || null : null,
      deleteOriginals,
      overwrite,
      renameOnConflict: !overwrite,
      compressionLevel,
    });
  }
</script>

<Modal {title} width="560px" {onclose}>
  {#snippet children()}
    <div class="disc-body">
      <div class="disc-hero">
        <div>
          <div class="disc-kicker">{t(`discImage.${mode}.kicker`)}</div>
          <p>{t(`discImage.${mode}.description`)}</p>
        </div>
        <span class="disc-count">{sources.length}</span>
      </div>

      <fieldset class="section">
        <legend>{t("discImage.destination")}</legend>
        <label class="choice">
          <input type="radio" bind:group={destinationMode} value="same" />
          <span>{t("discImage.sameFolder")}</span>
        </label>
        <label class="choice">
          <input type="radio" bind:group={destinationMode} value="custom" />
          <span>{t("discImage.customFolder")}</span>
        </label>
        {#if destinationMode === "custom"}
          <input class="path-input" bind:value={destinationPath} placeholder={t("discImage.customFolderPlaceholder")} />
        {/if}
      </fieldset>

      {#if showCompression}
        <fieldset class="section">
          <legend>{t("discImage.compression")}</legend>
          <div class="range-row">
            <input type="range" min="1" max="9" bind:value={compressionLevel} />
            <input class="level-input" type="number" min="1" max="9" bind:value={compressionLevel} />
          </div>
        </fieldset>
      {/if}

      <fieldset class="section section--inline">
        <label class="choice">
          <input type="checkbox" bind:checked={deleteOriginals} />
          <span>{t("discImage.deleteOriginals")}</span>
        </label>
        <label class="choice">
          <input type="checkbox" bind:checked={overwrite} />
          <span>{t("discImage.overwrite")}</span>
        </label>
      </fieldset>

      <div class="preview">
        <div class="preview-title">{t("discImage.preview")}</div>
        {#each sources.slice(0, 6) as source}
          <div class="preview-row">
            <span class="preview-kind">{outputKind}</span>
            <span class="preview-path">{previewFor(source)}</span>
          </div>
        {/each}
        {#if sources.length > 6}
          <div class="preview-more">{t("discImage.more", { count: sources.length - 6 })}</div>
        {/if}
      </div>
    </div>
  {/snippet}

  {#snippet footer()}
    <Button variant="ghost" onclick={onclose}>{t("common.cancel")}</Button>
    <Button variant="primary" onclick={confirm}>{action}</Button>
  {/snippet}
</Modal>

<style>
  .disc-body { display: flex; flex-direction: column; gap: 14px; }

  .disc-hero {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    padding: 14px;
    border: 1px solid color-mix(in srgb, var(--accent) 22%, var(--line));
    border-radius: 12px;
    background:
      radial-gradient(circle at top left, color-mix(in srgb, var(--accent) 14%, transparent), transparent 48%),
      var(--surface-alt);

    p {
      margin: 4px 0 0;
      color: var(--text-muted);
      font-size: 12px;
      line-height: 1.45;
    }
  }

  .disc-kicker {
    font-size: 12px;
    font-weight: 800;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--accent);
  }

  .disc-count {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 34px;
    height: 34px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--accent-contrast);
    font-weight: 800;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 12px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--surface);

    legend {
      padding: 0 5px;
      color: var(--text-muted);
      font-size: 11px;
      font-weight: 700;
      text-transform: uppercase;
      letter-spacing: 0.05em;
    }
  }

  .section--inline { gap: 10px; }

  .choice {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text);
  }

  .path-input,
  .level-input {
    border: 1px solid var(--line-strong);
    border-radius: 8px;
    background: var(--bg);
    color: var(--text);
    padding: 7px 9px;
    outline: none;
  }

  .range-row {
    display: grid;
    grid-template-columns: 1fr 64px;
    gap: 10px;
    align-items: center;
  }

  .preview {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px;
    border-radius: 10px;
    background: var(--surface-alt);
    border: 1px solid var(--line);
  }

  .preview-title {
    font-size: 11px;
    font-weight: 800;
    color: var(--text-subtle);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .preview-row {
    display: grid;
    grid-template-columns: 56px 1fr;
    gap: 8px;
    min-width: 0;
    font-size: 12px;
  }

  .preview-kind {
    color: var(--accent);
    font-weight: 700;
    text-transform: uppercase;
  }

  .preview-path {
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .preview-more {
    color: var(--text-subtle);
    font-size: 12px;
  }
</style>
