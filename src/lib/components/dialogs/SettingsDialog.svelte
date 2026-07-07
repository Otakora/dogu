<script lang="ts">
  import { app } from "../../stores/app.svelte.js";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import AboutDialog from "./AboutDialog.svelte";
  import { t } from "../../i18n/index.js";
  import type { Locale } from "../../types/index.js";

  let aboutOpen = $state(false);
</script>

{#if app.settingsOpen}
  <Modal title={t("settingsDialog.title")} width="520px" onclose={() => app.closeSettings()}>
    {#snippet children()}
      <div class="settings-body">

        <!-- Theme -->
        <div class="setting-group">
          <div class="setting-label">{t("settingsDialog.appearance")}</div>
          <div class="setting-row">
            <span class="setting-name">{t("settingsDialog.theme")}</span>
            <div class="toggle-group" role="group" aria-label={t("settingsDialog.theme")}>
              <button
                class="toggle-btn"
                class:toggle-btn--active={app.theme === "light"}
                onclick={() => app.setTheme("light")}
                aria-pressed={app.theme === "light"}
              >
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="5"/>
                  <line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/>
                  <line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/>
                  <line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/>
                  <line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/>
                </svg>
                {t("settingsDialog.light")}
              </button>
              <button
                class="toggle-btn"
                class:toggle-btn--active={app.theme === "dark"}
                onclick={() => app.setTheme("dark")}
                aria-pressed={app.theme === "dark"}
              >
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>
                </svg>
                {t("settingsDialog.dark")}
              </button>
            </div>
          </div>

          <div class="setting-row">
            <span class="setting-name">{t("settingsDialog.language")}</span>
            <select
              class="select-input"
              value={app.settings.locale}
              onchange={(e) => app.updateSettings({ locale: (e.target as HTMLSelectElement).value as Locale })}
              aria-label={t("settingsDialog.language")}
            >
              <option value="en">English</option>
              <option value="es">Español</option>
            </select>
          </div>

          <div class="setting-row">
            <span class="setting-name">{t("settingsDialog.fontScale")}</span>
            <div class="setting-row-right">
              <input
                type="range"
                min="0.75"
                max="1.5"
                step="0.05"
                value={app.settings.fontScale}
                oninput={(e) => {
                  const v = parseFloat((e.target as HTMLInputElement).value);
                  app.updateSettings({ fontScale: v });
                  document.documentElement.style.setProperty("--app-font-scale", String(v));
                }}
                class="range-input"
                aria-label={t("settingsDialog.fontScale")}
              />
              <span class="range-val">{Math.round(app.settings.fontScale * 100)}%</span>
            </div>
          </div>

          <div class="setting-row">
            <span class="setting-name">{t("settingsDialog.compactUi")}</span>
            <input
              type="checkbox"
              checked={app.settings.compactUi}
              onchange={(e) => app.updateSettings({ compactUi: (e.target as HTMLInputElement).checked })}
              class="checkbox"
              aria-label={t("settingsDialog.compactUi")}
            />
          </div>
        </div>

        <!-- Files -->
        <div class="setting-group">
          <div class="setting-label">{t("settingsDialog.fileOperations")}</div>
          <div class="setting-row">
            <span class="setting-name">{t("settingsDialog.confirmBeforeDelete")}</span>
            <input
              type="checkbox"
              checked={app.settings.confirmDelete}
              onchange={(e) => app.updateSettings({ confirmDelete: (e.target as HTMLInputElement).checked })}
              class="checkbox"
              aria-label={t("settingsDialog.confirmBeforeDelete")}
            />
          </div>
          <div class="setting-row">
            <span class="setting-name">{t("settingsDialog.defaultView")}</span>
            <select
              class="select-input"
              value={app.settings.defaultViewMode}
              onchange={(e) => app.updateSettings({ defaultViewMode: (e.target as HTMLSelectElement).value as "list" | "grid" })}
              aria-label={t("settingsDialog.defaultView")}
            >
              <option value="list">{t("settingsDialog.list")}</option>
              <option value="grid">{t("settingsDialog.grid")}</option>
            </select>
          </div>
        </div>

        <!-- File conflicts -->
        <div class="setting-group">
          <div class="setting-label">{t("settingsDialog.fileConflicts")}</div>
          <div class="setting-row">
            <div class="setting-name-block">
              <span class="setting-name">{t("settingsDialog.defaultOverwrite")}</span>
              <span class="setting-hint">{t("settingsDialog.defaultOverwriteHint")}</span>
            </div>
            <input
              type="checkbox"
              checked={app.settings.defaultOverwriteOnConflict}
              onchange={(e) => app.updateSettings({ defaultOverwriteOnConflict: (e.target as HTMLInputElement).checked })}
              class="checkbox"
              aria-label={t("settingsDialog.defaultOverwrite")}
            />
          </div>
          <div class="setting-row">
            <div class="setting-name-block">
              <span class="setting-name">{t("settingsDialog.renameOnConflict")}</span>
              <span class="setting-hint">{t("settingsDialog.renameOnConflictHint")}</span>
            </div>
            <input
              type="checkbox"
              checked={app.settings.renameOnConflict}
              onchange={(e) => app.updateSettings({ renameOnConflict: (e.target as HTMLInputElement).checked })}
              class="checkbox"
              aria-label={t("settingsDialog.renameOnConflict")}
            />
          </div>
        </div>

        <!-- Queue -->
        <div class="setting-group">
          <div class="setting-label">{t("settingsDialog.queue")}</div>
          <div class="queue-card">
            <div class="setting-row setting-row--top">
              <div class="setting-name-block">
                <span class="setting-name">{t("settingsDialog.queueMaxConcurrent")}</span>
                <span class="setting-hint">{t("settingsDialog.queueMaxConcurrentHint")}</span>
              </div>
              <span class="queue-badge">{t("settingsDialog.queueMaxConcurrentValue", { count: app.queueMaxConcurrent })}</span>
            </div>
            <div class="queue-slider-row">
              <input
                type="range"
                min="1"
                max="6"
                step="1"
                value={app.queueMaxConcurrent}
                oninput={(e) => app.updateSettings({ queueMaxConcurrent: parseInt((e.target as HTMLInputElement).value) || 3 })}
                class="range-input range-input--wide"
                aria-label={t("settingsDialog.queueMaxConcurrent")}
              />
              <input
                type="number"
                min="1"
                max="6"
                class="number-input"
                value={app.queueMaxConcurrent}
                oninput={(e) => app.updateSettings({ queueMaxConcurrent: parseInt((e.target as HTMLInputElement).value) || 3 })}
                aria-label={t("settingsDialog.queueMaxConcurrent")}
              />
            </div>
            <div class="queue-scale" aria-hidden="true">
              <span>1</span><span>2</span><span>3</span><span>4</span><span>5</span><span>6</span>
            </div>
          </div>
        </div>

        <!-- Weight / summary -->
        <div class="setting-group">
          <div class="setting-label">{t("settingsDialog.properties")}</div>
          <div class="setting-row">
            <span class="setting-name">{t("settingsDialog.maxDepth")}</span>
            <input
              type="number"
              min="1"
              max="20"
              class="number-input"
              value={app.settings.selectionWeightMaxDepth}
              oninput={(e) => app.updateSettings({ selectionWeightMaxDepth: parseInt((e.target as HTMLInputElement).value) || 5 })}
              aria-label={t("settingsDialog.maxDepth")}
            />
          </div>
        </div>

        <!-- CHD -->
        <div class="setting-group">
          <div class="setting-label">{t("settingsDialog.chdConversion")}</div>
          <div class="setting-row">
            <span class="setting-name">{t("settingsDialog.chdScanDepth")}</span>
            <div class="setting-row-right">
              <input
                type="number"
                min="1"
                max="10"
                class="number-input"
                value={app.settings.chdScanDepth}
                oninput={(e) => app.updateSettings({ chdScanDepth: Math.min(10, Math.max(1, parseInt((e.target as HTMLInputElement).value) || 3)) })}
                aria-label={t("settingsDialog.chdScanDepth")}
              />
            </div>
          </div>
        </div>
      </div>
    {/snippet}

    {#snippet footer()}
      <div class="footer-left">
        <Button variant="ghost" onclick={() => (aboutOpen = true)}>{t("settingsDialog.credits")}</Button>
      </div>
      <Button variant="primary" onclick={() => app.closeSettings()}>{t("settingsDialog.done")}</Button>
    {/snippet}
  </Modal>
{/if}

{#if aboutOpen}
  <AboutDialog onclose={() => (aboutOpen = false)} />
{/if}

<style>
  .settings-body {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .setting-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .setting-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-subtle);
    padding-bottom: 4px;
    border-bottom: 1px solid var(--line);
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 28px;
  }

  .setting-row--top {
    align-items: flex-start;
  }

  .setting-name {
    font-size: 13px;
    color: var(--text);
  }

  .setting-name-block {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    padding-right: 12px;
  }

  .setting-hint {
    font-size: 11px;
    color: var(--text-subtle);
    line-height: 1.35;
  }

  .setting-row-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .toggle-group {
    display: flex;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    overflow: hidden;
  }

  .toggle-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    background: var(--surface);
    border: none;
    font-size: 12px;
    color: var(--text-muted);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;

    &:hover { background: var(--surface-hover); color: var(--text); }
  }

  .toggle-btn--active {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 500;
  }

  .range-input {
    width: 120px;
    accent-color: var(--accent);
  }

  .range-input--wide {
    width: 100%;
    min-width: 160px;
  }

  .range-val {
    font-size: 12px;
    color: var(--text-muted);
    min-width: 36px;
    text-align: right;
  }

  .checkbox {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
    cursor: pointer;
  }

  .select-input {
    height: 28px;
    padding: 0 8px;
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
  }

  .number-input {
    width: 60px;
    height: 28px;
    padding: 0 8px;
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 13px;
    text-align: right;
  }

  .queue-card {
    display: flex;
    flex-direction: column;
    gap: 9px;
    padding: 10px 11px;
    border-radius: 9px;
    background:
      radial-gradient(circle at 8% 0%, color-mix(in srgb, var(--accent) 14%, transparent), transparent 38%),
      linear-gradient(135deg, color-mix(in srgb, var(--accent) 7%, var(--surface-alt)), var(--surface-alt));
    border: 1px solid color-mix(in srgb, var(--accent) 22%, var(--line));
  }

  .queue-badge {
    flex-shrink: 0;
    padding: 3px 8px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
    border: 1px solid color-mix(in srgb, var(--accent) 28%, transparent);
    font-size: 11px;
    font-weight: 750;
    white-space: nowrap;
  }

  .queue-slider-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 64px;
    gap: 10px;
    align-items: center;
  }

  .queue-scale {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 4px;
    color: var(--text-subtle);
    font-size: 10px;
    font-weight: 700;
    padding: 0 72px 0 2px;
    text-align: center;
  }

  .footer-left {
    flex: 1;
  }
</style>
