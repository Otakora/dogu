<script lang="ts">
  import { openUrl as tauriOpenUrl } from "@tauri-apps/plugin-opener";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import { t } from "../../i18n/index.js";
  import pkg from "../../../../package.json";

  type Props = { onclose: () => void };
  let { onclose }: Props = $props();

  const version: string = pkg.version;

  function openUrl(url: string) {
    tauriOpenUrl(url).catch(() => {});
  }

  const tools = [
    {
      name: "chdman",
      version: "v0.288",
      copyright: "© 1997-2026 MAMEdev and contributors",
      license: "GPL-2.0",
      url: "https://www.mamedev.org/",
      urlLabel: "mamedev.org",
      icon: `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="10"/>
        <circle cx="12" cy="12" r="3"/>
        <path d="M12 2v4M12 18v4M2 12h4M18 12h4"/>
      </svg>`,
    },
    {
      name: "7-Zip",
      version: "v26.01",
      copyright: "© 1999-2026 Igor Pavlov",
      license: "LGPL-2.1",
      url: "https://www.7-zip.org/",
      urlLabel: "7-zip.org",
      icon: `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
        <polyline points="3.27 6.96 12 12.01 20.73 6.96"/>
        <line x1="12" y1="22.08" x2="12" y2="12"/>
      </svg>`,
    },
  ] as const;
</script>

<Modal title={t("about.title")} width="500px" {onclose}>
  {#snippet children()}
    <div class="about-body">

      <!-- Hero -->
      <div class="hero">
        <div class="hero-kanji" aria-hidden="true">道具</div>
        <div class="hero-info">
          <span class="hero-name">Dogu</span>
          <span class="hero-version">v{version}</span>
        </div>
        <p class="hero-tagline">{t("about.tagline")}</p>
      </div>

      <!-- Third-party tools -->
      <div class="section">
        <div class="section-label">{t("about.thirdParty")}</div>
        <div class="tools-grid">
          {#each tools as tool}
            <div class="tool-card">
              <div class="tool-header">
                <span class="tool-icon">{@html tool.icon}</span>
                <div class="tool-title">
                  <span class="tool-name">{tool.name}</span>
                  <span class="tool-ver">{tool.version}</span>
                </div>
              </div>
              <p class="tool-copyright">{tool.copyright}</p>
              <div class="tool-footer">
                <span class="license-badge">{tool.license}</span>
                <button class="link-btn" onclick={() => openUrl(tool.url)}>
                  {tool.urlLabel}
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
                    <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>
                    <polyline points="15 3 21 3 21 9"/>
                    <line x1="10" y1="14" x2="21" y2="3"/>
                  </svg>
                </button>
              </div>
            </div>
          {/each}
        </div>
        <p class="licenses-note">{t("about.licensesNote")}</p>
      </div>

      <!-- Source -->
      <div class="section section--source">
        <span class="source-label">{t("about.sourceCode")}</span>
        <button class="link-btn" onclick={() => openUrl("https://github.com/Otakora/dogu")}>
          github.com/Otakora/dogu
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
            <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>
            <polyline points="15 3 21 3 21 9"/>
            <line x1="10" y1="14" x2="21" y2="3"/>
          </svg>
        </button>
      </div>

    </div>
  {/snippet}

  {#snippet footer()}
    <Button variant="primary" onclick={onclose}>{t("about.close")}</Button>
  {/snippet}
</Modal>

<style>
  .about-body {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  /* ── Hero ── */
  .hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 20px 16px 4px;
    text-align: center;
  }

  .hero-kanji {
    font-size: 52px;
    line-height: 1;
    color: var(--accent);
    opacity: 0.85;
    font-weight: 400;
    letter-spacing: -0.02em;
    background: var(--accent-soft);
    width: 80px;
    height: 80px;
    border-radius: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid color-mix(in srgb, var(--accent) 20%, transparent);
  }

  .hero-info {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .hero-name {
    font-size: 22px;
    font-weight: 700;
    color: var(--text);
    letter-spacing: -0.01em;
  }

  .hero-version {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-muted);
    background: var(--surface-alt);
    border: 1px solid var(--line);
    padding: 2px 7px;
    border-radius: 10px;
  }

  .hero-tagline {
    font-size: 13px;
    color: var(--text-muted);
    max-width: 340px;
    line-height: 1.5;
    margin: 0;
  }

  /* ── Section ── */
  .section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .section-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-subtle);
    padding-bottom: 6px;
    border-bottom: 1px solid var(--line);
  }

  /* ── Tool cards ── */
  .tools-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .tool-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--surface-alt);
  }

  .tool-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .tool-icon {
    color: var(--text-muted);
    display: flex;
    flex-shrink: 0;
  }

  .tool-title {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .tool-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
  }

  .tool-ver {
    font-size: 11px;
    color: var(--text-muted);
  }

  .tool-copyright {
    font-size: 11px;
    color: var(--text-subtle);
    margin: 0;
    line-height: 1.4;
  }

  .tool-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    margin-top: 2px;
  }

  .license-badge {
    font-size: 10px;
    font-weight: 600;
    font-family: monospace;
    padding: 2px 6px;
    border-radius: 4px;
    background: var(--accent-soft);
    color: var(--accent);
    border: 1px solid color-mix(in srgb, var(--accent) 20%, transparent);
    letter-spacing: 0.03em;
  }

  /* ── Link button ── */
  .link-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: none;
    border: none;
    padding: 0;
    font-size: 12px;
    color: var(--accent);
    cursor: pointer;
    text-decoration: none;
    line-height: 1;

    &:hover {
      text-decoration: underline;
      color: var(--accent-light);
    }
  }

  /* ── Licenses note ── */
  .licenses-note {
    font-size: 11px;
    color: var(--text-subtle);
    margin: 0;
    line-height: 1.5;
  }

  /* ── Source row ── */
  .section--source {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    background: var(--surface-alt);
    border: 1px solid var(--line);
    border-radius: 8px;
    gap: 8px;
  }

  .source-label {
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
