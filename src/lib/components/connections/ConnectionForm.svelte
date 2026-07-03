<script lang="ts">
  import { untrack } from "svelte";
  import type { ConnectionProfileDto, ConnectionProfilePayload } from "../../types/index.js";
  import Button from "../ui/Button.svelte";
  import { t } from "../../i18n/index.js";

  type Props = {
    profile?: ConnectionProfileDto | null;
    isSaving?: boolean;
    isTesting?: boolean;
    onSave: (payload: ConnectionProfilePayload) => void;
    onTest: (payload: ConnectionProfilePayload) => void;
    onCancel: () => void;
  };

  let {
    profile = null,
    isSaving = false,
    isTesting = false,
    onSave,
    onTest,
    onCancel,
  }: Props = $props();

  type Protocol = "ssh" | "smb" | "ftp" | "ftps";

  // ── Form state ──────────────────────────────────────────
  // Snapshot the prop: the form owns its state after mount, changes to the
  // parent `profile` prop don't flow in (form is recreated via keyed {#key}).
  const init = untrack(() => profile);
  let label = $state(init?.label ?? "");
  let protocol = $state<Protocol>((init?.protocol as Protocol) ?? "ssh");
  let host = $state(init?.host ?? "");
  let portStr = $state(init ? String(init.port) : "");
  let username = $state(init?.username ?? "");
  let password = $state(init?.password ?? "");
  let showPassword = $state(false);
  let share = $state(init?.share ?? "");
  let workgroup = $state(init?.workgroup ?? "");
  let startPath = $state(init?.startPath ?? "");
  let sshMode = $state<"sftp" | "scp">(init?.sshMode ?? "sftp");
  let ftpMode = $state<"passive" | "active">(init?.ftpMode ?? "passive");
  let ftpSecureImplicit = $state(init?.ftpSecureImplicit ?? false);
  let ftpAcceptInvalidCerts = $state(init?.ftpAcceptInvalidCertificates ?? false);
  let ftpAcceptInvalidHostnames = $state(init?.ftpAcceptInvalidHostnames ?? false);

  let errors = $state<Record<string, string>>({});

  // ── Derived ──────────────────────────────────────────────
  const defaultPort = $derived((): number => {
    switch (protocol) {
      case "ssh": return 22;
      case "smb": return 445;
      case "ftps": return ftpSecureImplicit ? 990 : 21;
      default: return 21;
    }
  });

  const portPlaceholder = $derived(String(defaultPort()));

  // Auto-update port when protocol or implicit mode changes, only if user hasn't set a custom port.
  // untrack() on portStr reads prevents this effect from re-running when the user types.
  $effect(() => {
    const def = defaultPort();
    const current = untrack(() => parseInt(portStr, 10));
    if (!untrack(() => portStr) || [21, 22, 445, 990].includes(current)) {
      portStr = String(def);
    }
  });

  // ── Helpers ──────────────────────────────────────────────
  function buildPayload(): ConnectionProfilePayload {
    const portNum = portStr ? parseInt(portStr, 10) : null;
    return {
      id: profile?.id ?? null,
      label: label.trim(),
      protocol,
      host: host.trim(),
      port: portNum && !isNaN(portNum) ? portNum : null,
      username: username.trim(),
      password,
      share: share.trim(),
      workgroup: workgroup.trim(),
      startPath: startPath.trim(),
      sshMode,
      ftpMode,
      ftpSecureImplicit,
      ftpAcceptInvalidCertificates: ftpAcceptInvalidCerts,
      ftpAcceptInvalidHostnames: ftpAcceptInvalidHostnames,
    };
  }

  function validate(): boolean {
    const e: Record<string, string> = {};
    if (!host.trim()) e.host = t("connectionForm.hostRequired");
    if (protocol === "smb" && !share.trim()) e.share = t("connectionForm.shareRequired");
    if (portStr) {
      const n = parseInt(portStr, 10);
      if (isNaN(n) || n < 1 || n > 65535) e.port = t("connectionForm.portRange");
    }
    errors = e;
    return Object.keys(e).length === 0;
  }

  function handleSave() {
    if (!validate()) return;
    onSave(buildPayload());
  }

  function handleTest() {
    if (!validate()) return;
    onTest(buildPayload());
  }

  const isSSH = $derived(protocol === "ssh");
  const isSMB = $derived(protocol === "smb");
  const isFTP = $derived(protocol === "ftp" || protocol === "ftps");
  const isFTPS = $derived(protocol === "ftps");
  const busy = $derived(isSaving || isTesting);
</script>

<form class="conn-form" onsubmit={(e) => { e.preventDefault(); handleSave(); }}>

  <!-- ── Label ── -->
  <div class="field">
    <label class="field-label" for="cf-label">{t("connectionForm.label")} <span class="optional">{t("connectionForm.optional")}</span></label>
    <input id="cf-label" class="field-input" type="text" bind:value={label} placeholder="My server" />
  </div>

  <!-- ── Protocol + Host + Port ── -->
  <div class="field-row">
    <div class="field field--protocol">
      <label class="field-label" for="cf-protocol">{t("connectionForm.protocol")}</label>
      <select id="cf-protocol" class="field-select" bind:value={protocol}>
        <option value="ssh">SSH / SFTP</option>
        <option value="ftp">FTP</option>
        <option value="ftps">FTPS</option>
        <option value="smb">SMB / Samba</option>
      </select>
    </div>
    <div class="field field--host">
      <label class="field-label" for="cf-host">{t("connectionForm.host")} <span class="required">{t("connectionForm.required")}</span></label>
      <input
        id="cf-host"
        class="field-input"
        class:field-input--error={errors.host}
        type="text"
        bind:value={host}
        placeholder="192.168.1.1 or hostname"
        autocomplete="off"
        spellcheck="false"
      />
      {#if errors.host}<span class="field-error">{errors.host}</span>{/if}
    </div>
    <div class="field field--port">
      <label class="field-label" for="cf-port">{t("connectionForm.port")}</label>
      <input
        id="cf-port"
        class="field-input"
        class:field-input--error={errors.port}
        type="text"
        inputmode="numeric"
        pattern="[0-9]*"
        bind:value={portStr}
        placeholder={portPlaceholder}
      />
      {#if errors.port}<span class="field-error">{errors.port}</span>{/if}
    </div>
  </div>

  <!-- ── Credentials ── -->
  <div class="field-row">
    <div class="field field--flex">
      <label class="field-label" for="cf-user">{t("connectionForm.username")}</label>
      <input id="cf-user" class="field-input" type="text" bind:value={username} placeholder="anonymous" autocomplete="off" />
    </div>
    <div class="field field--flex">
      <label class="field-label" for="cf-pass">{t("connectionForm.password")}</label>
      <div class="field-password-wrap">
        <input
          id="cf-pass"
          class="field-input"
          type={showPassword ? "text" : "password"}
          bind:value={password}
          placeholder=""
          autocomplete="new-password"
        />
        <button
          type="button"
          class="show-pass-btn"
          onclick={() => (showPassword = !showPassword)}
          aria-label={showPassword ? t("connectionForm.hidePassword") : t("connectionForm.showPassword")}
        >
          {#if showPassword}
            <!-- eye-slash icon -->
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"/>
              <line x1="1" y1="1" x2="23" y2="23"/>
            </svg>
          {:else}
            <!-- eye icon -->
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
              <circle cx="12" cy="12" r="3"/>
            </svg>
          {/if}
        </button>
      </div>
    </div>
  </div>

  <!-- ── SMB-specific ── -->
  {#if isSMB}
    <div class="field-row">
      <div class="field field--flex">
        <label class="field-label" for="cf-share">{t("connectionForm.share")} <span class="required">{t("connectionForm.required")}</span></label>
        <input
          id="cf-share"
          class="field-input"
          class:field-input--error={errors.share}
          type="text"
          bind:value={share}
          placeholder="shared_folder"
          autocomplete="off"
          spellcheck="false"
        />
        {#if errors.share}<span class="field-error">{errors.share}</span>{/if}
      </div>
      <div class="field field--flex">
        <label class="field-label" for="cf-workgroup">{t("connectionForm.workgroup")} <span class="optional">{t("connectionForm.optional")}</span></label>
        <input id="cf-workgroup" class="field-input" type="text" bind:value={workgroup} placeholder="WORKGROUP" autocomplete="off" />
      </div>
    </div>
  {/if}

  <!-- ── Start path ── -->
  <div class="field">
    <label class="field-label" for="cf-start-path">{t("connectionForm.startPath")} <span class="optional">{t("connectionForm.optional")}</span></label>
    <input id="cf-start-path" class="field-input" type="text" bind:value={startPath} placeholder="/" autocomplete="off" spellcheck="false" />
  </div>

  <!-- ── SSH-specific ── -->
  {#if isSSH}
    <div class="field-section">
      <span class="section-label">{t("connectionForm.sshOptions")}</span>
      <div class="radio-group">
        <label class="radio-label">
          <input type="radio" bind:group={sshMode} value="sftp" />
          {t("connectionForm.sftp")} <span class="radio-hint">{t("connectionForm.sftpHint")}</span>
        </label>
        <label class="radio-label">
          <input type="radio" bind:group={sshMode} value="scp" />
          {t("connectionForm.scp")} <span class="radio-hint">{t("connectionForm.scpHint")}</span>
        </label>
      </div>
    </div>
  {/if}

  <!-- ── FTP/FTPS-specific ── -->
  {#if isFTP}
    <div class="field-section">
      <span class="section-label">{t("connectionForm.ftpOptions")}</span>
      <div class="radio-group">
        <label class="radio-label">
          <input type="radio" bind:group={ftpMode} value="passive" />
          {t("connectionForm.passiveMode")} <span class="radio-hint">{t("connectionForm.passiveHint")}</span>
        </label>
        <label class="radio-label">
          <input type="radio" bind:group={ftpMode} value="active" />
          {t("connectionForm.activeMode")}
        </label>
      </div>
      {#if isFTPS}
        <div class="checkbox-group">
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={ftpSecureImplicit} />
            {t("connectionForm.implicitTls")} <span class="radio-hint">{t("connectionForm.implicitTlsHint")}</span>
          </label>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={ftpAcceptInvalidCerts} />
            {t("connectionForm.acceptInvalidCerts")}
          </label>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={ftpAcceptInvalidHostnames} />
            {t("connectionForm.acceptInvalidHostnames")}
          </label>
        </div>
      {/if}
    </div>
  {/if}

  <!-- ── Actions ── -->
  <div class="form-actions">
    <Button variant="ghost" onclick={onCancel} disabled={busy}>{t("connectionForm.cancel")}</Button>
    <Button variant="outline" onclick={handleTest} disabled={busy}>
      {#if isTesting}
        <span class="spinner"></span> {t("connectionForm.testing")}
      {:else}
        {t("connectionForm.testConnection")}
      {/if}
    </Button>
    <Button variant="primary" type="submit" disabled={busy}>
      {#if isSaving}
        <span class="spinner"></span> {t("connectionForm.saving")}
      {:else}
        {profile ? t("connectionForm.save") : t("connectionForm.addConnection")}
      {/if}
    </Button>
  </div>
</form>

<style>
  .conn-form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field-row {
    display: flex;
    gap: 8px;
    align-items: flex-start;
  }

  .field--protocol { flex: 0 0 130px; }
  .field--host { flex: 1; min-width: 0; }
  .field--port { flex: 0 0 80px; }
  .field--flex { flex: 1; min-width: 0; }

  .field-label {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-muted);
  }

  .required { color: var(--danger); }
  .optional { color: var(--text-subtle); font-weight: 400; }

  .field-input,
  .field-select {
    height: 30px;
    padding: 0 8px;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    background: var(--surface);
    color: var(--text);
    font-size: 13px;
    width: 100%;
    transition: border-color 0.1s, box-shadow 0.1s;
    outline: none;

    &:focus {
      border-color: var(--accent);
      box-shadow: 0 0 0 2px var(--accent-soft);
    }
  }

  .field-input--error {
    border-color: var(--danger);
    &:focus { box-shadow: 0 0 0 2px var(--danger-soft); }
  }

  .field-error {
    font-size: 11px;
    color: var(--danger);
  }

  .field-password-wrap {
    position: relative;
    display: flex;
    align-items: center;
  }

  .field-password-wrap .field-input {
    padding-right: 32px;
  }

  .show-pass-btn {
    position: absolute;
    right: 6px;
    background: none;
    border: none;
    color: var(--text-muted);
    padding: 2px;
    display: flex;
    align-items: center;

    &:hover { color: var(--text); }
  }

  /* ── Sections ── */
  .field-section {
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    background: var(--surface-alt);
  }

  .section-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-subtle);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .radio-group,
  .checkbox-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .radio-label,
  .checkbox-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--text);
    cursor: pointer;

    input { accent-color: var(--accent); }
  }

  .radio-hint {
    font-size: 12px;
    color: var(--text-muted);
  }

  /* ── Actions ── */
  .form-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 4px;
    border-top: 1px solid var(--line);
    margin-top: 4px;
  }

  /* ── Spinner ── */
  .spinner {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 2px solid transparent;
    border-top-color: currentColor;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }
</style>
