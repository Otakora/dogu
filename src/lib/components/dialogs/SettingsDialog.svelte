<script lang="ts">
  import { app } from "../../stores/app.svelte.js";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
</script>

{#if app.settingsOpen}
  <Modal title="Settings" width="440px" onclose={() => app.closeSettings()}>
    {#snippet children()}
      <div class="settings-body">

        <!-- Theme -->
        <div class="setting-group">
          <div class="setting-label">Appearance</div>
          <div class="setting-row">
            <span class="setting-name">Theme</span>
            <div class="toggle-group" role="group" aria-label="Theme">
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
                Light
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
                Dark
              </button>
            </div>
          </div>

          <div class="setting-row">
            <span class="setting-name">Font scale</span>
            <div class="setting-row-right">
              <input
                type="range"
                min="0.8"
                max="1.3"
                step="0.05"
                value={app.settings.fontScale}
                oninput={(e) => {
                  const v = parseFloat((e.target as HTMLInputElement).value);
                  app.updateSettings({ fontScale: v });
                  document.documentElement.style.setProperty("--app-font-scale", String(v));
                }}
                class="range-input"
                aria-label="Font scale"
              />
              <span class="range-val">{Math.round(app.settings.fontScale * 100)}%</span>
            </div>
          </div>

          <div class="setting-row">
            <span class="setting-name">Compact UI</span>
            <input
              type="checkbox"
              checked={app.settings.compactUi}
              onchange={(e) => app.updateSettings({ compactUi: (e.target as HTMLInputElement).checked })}
              class="checkbox"
              aria-label="Compact UI"
            />
          </div>
        </div>

        <!-- Files -->
        <div class="setting-group">
          <div class="setting-label">File operations</div>
          <div class="setting-row">
            <span class="setting-name">Confirm before delete</span>
            <input
              type="checkbox"
              checked={app.settings.confirmDelete}
              onchange={(e) => app.updateSettings({ confirmDelete: (e.target as HTMLInputElement).checked })}
              class="checkbox"
              aria-label="Confirm delete"
            />
          </div>
          <div class="setting-row">
            <span class="setting-name">Default view</span>
            <select
              class="select-input"
              value={app.settings.defaultViewMode}
              onchange={(e) => app.updateSettings({ defaultViewMode: (e.target as HTMLSelectElement).value as "list" | "grid" })}
              aria-label="Default view mode"
            >
              <option value="list">List</option>
              <option value="grid">Grid</option>
            </select>
          </div>
        </div>

        <!-- Weight / summary -->
        <div class="setting-group">
          <div class="setting-label">Properties</div>
          <div class="setting-row">
            <span class="setting-name">Max depth for size calculation</span>
            <input
              type="number"
              min="1"
              max="20"
              class="number-input"
              value={app.settings.selectionWeightMaxDepth}
              oninput={(e) => app.updateSettings({ selectionWeightMaxDepth: parseInt((e.target as HTMLInputElement).value) || 5 })}
              aria-label="Max depth"
            />
          </div>
        </div>
      </div>
    {/snippet}

    {#snippet footer()}
      <Button variant="primary" onclick={() => app.closeSettings()}>Done</Button>
    {/snippet}
  </Modal>
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

  .setting-name {
    font-size: 13px;
    color: var(--text);
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
</style>
