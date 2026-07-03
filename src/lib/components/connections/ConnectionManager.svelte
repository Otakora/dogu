<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { app } from "../../stores/app.svelte.js";
  import type {
    ConnectionProfileDto,
    ConnectionProfilePayload,
    ActiveConnectionDto,
    ConnectionOpenResultDto,
  } from "../../types/index.js";
  import ConnectionForm from "./ConnectionForm.svelte";
  import Button from "../ui/Button.svelte";
  import { t } from "../../i18n/index.js";

  // ── State ────────────────────────────────────────────────
  let profiles = $state<ConnectionProfileDto[]>([]);
  let sessions = $state<ActiveConnectionDto[]>([]);
  let selectedProfileId = $state<string | null>(null);
  let editingProfile = $state<ConnectionProfileDto | null>(null);
  let isCreating = $state(false);
  let isSaving = $state(false);
  let isTesting = $state(false);
  let connectingId = $state<string | null>(null);
  let disconnectingId = $state<string | null>(null);
  let testResult = $state<{ ok: boolean; message: string; writeAccess?: boolean | null } | null>(null);
  let deleteConfirmId = $state<string | null>(null);
  let testRunId = 0;
  let connectRunId = 0;
  let disconnectRunId = 0;

  const REMOTE_OPERATION_TIMEOUT_MS = 25000;

  // ── Derived ──────────────────────────────────────────────
  const showForm = $derived(isCreating || editingProfile !== null);
  const formProfile = $derived(isCreating ? null : editingProfile);

  // ── Load on mount ─────────────────────────────────────────
  $effect(() => {
    loadAll();
  });

  async function loadAll() {
    try {
      const [p, s] = await Promise.all([
        invoke<ConnectionProfileDto[]>("list_connection_profiles"),
        invoke<ActiveConnectionDto[]>("list_active_connections"),
      ]);
      profiles = p;
      sessions = s;
      app.setConnectionProfiles(p);
      app.setActiveConnections(s);
    } catch (e) {
      app.notify("error", String(e));
    }
  }

  // ── Profile CRUD ──────────────────────────────────────────
  function startCreate() {
    isCreating = true;
    editingProfile = null;
    testResult = null;
  }

  function startEdit(profile: ConnectionProfileDto) {
    editingProfile = profile;
    isCreating = false;
    testResult = null;
    selectedProfileId = profile.id;
  }

  function cancelForm() {
    testRunId += 1;
    isTesting = false;
    isCreating = false;
    editingProfile = null;
    testResult = null;
  }

  function formatInvokeError(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  async function invokeWithTimeout<T>(
    command: string,
    args: Record<string, unknown>,
    timeoutMessage: string,
  ): Promise<T> {
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    try {
      return await Promise.race([
        invoke<T>(command, args),
        new Promise<never>((_, reject) => {
          timeoutId = setTimeout(() => reject(new Error(timeoutMessage)), REMOTE_OPERATION_TIMEOUT_MS);
        }),
      ]);
    } finally {
      if (timeoutId !== null) {
        clearTimeout(timeoutId);
      }
    }
  }

  async function saveProfile(payload: ConnectionProfilePayload) {
    isSaving = true;
    testResult = null;
    try {
      await invoke("save_connection_profile", { profile: payload });
      await loadAll();
      cancelForm();
      app.notify("success", payload.id ? t("connectionManager.profileSaved") : t("connectionManager.profileAdded"));
    } catch (e) {
      app.notify("error", String(e));
    } finally {
      isSaving = false;
    }
  }

  async function testProfile(payload: ConnectionProfilePayload) {
    const runId = ++testRunId;
    isTesting = true;
    testResult = null;
    try {
      const result = await invokeWithTimeout<ConnectionOpenResultDto>(
        "test_connection_profile_payload",
        { profile: payload, trustCurrentFingerprint: false },
        t("connectionManager.testTimedOut"),
      );
      if (runId !== testRunId) return;

      if (result.requiresTrust && result.fingerprint) {
        const trusted = await app.promptFingerprint(
          result.fingerprint,
          payload.label || payload.host
        );
        if (runId !== testRunId) return;
        if (!trusted) {
          testResult = { ok: false, message: t("connectionManager.testCancelledFingerprint") };
          return;
        }
        const retryResult = await invokeWithTimeout<ConnectionOpenResultDto>(
          "test_connection_profile_payload",
          { profile: payload, trustCurrentFingerprint: true },
          t("connectionManager.testTimedOut"),
        );
        if (runId !== testRunId) return;
        testResult = retryResult.connected
          ? { ok: true, message: t("connectionManager.connectionSuccessful"), writeAccess: retryResult.writeAccess }
          : { ok: false, message: retryResult.message ?? t("connectionManager.connectionFailed") };
        return;
      }

      testResult = result.connected
        ? { ok: true, message: t("connectionManager.connectionSuccessful"), writeAccess: result.writeAccess }
        : { ok: false, message: result.message ?? t("connectionManager.connectionFailed") };
    } catch (e) {
      if (runId !== testRunId) return;
      testResult = { ok: false, message: formatInvokeError(e) };
    } finally {
      if (runId === testRunId) {
        isTesting = false;
      }
    }
  }

  async function deleteProfile(id: string) {
    deleteConfirmId = null;
    try {
      await invoke("delete_connection_profile", { profileId: id });
      await loadAll();
      if (editingProfile?.id === id) cancelForm();
    } catch (e) {
      app.notify("error", String(e));
    }
  }

  // ── Connect ───────────────────────────────────────────────
  async function connectProfile(profileId: string) {
    const runId = ++connectRunId;
    connectingId = profileId;
    try {
      const result = await invokeWithTimeout<ConnectionOpenResultDto>(
        "connect_connection_profile",
        {
          profileId,
          trustCurrentFingerprint: false,
        },
        t("connectionManager.connectTimedOut"),
      );
      if (runId !== connectRunId) return;

      if (result.requiresTrust && result.fingerprint) {
        const profile = profiles.find((p) => p.id === profileId);
        const trusted = await app.promptFingerprint(
          result.fingerprint,
          profile?.label ?? profile?.host ?? profileId
        );
        if (runId !== connectRunId) return;
        if (trusted) {
          // Retry with trust granted
          const retryResult = await invokeWithTimeout<ConnectionOpenResultDto>(
            "connect_connection_profile",
            {
              profileId,
              trustCurrentFingerprint: true,
            },
            t("connectionManager.connectTimedOut"),
          );
          if (runId !== connectRunId) return;
          if (retryResult.connected && retryResult.connection) {
            sessions = [...sessions.filter((s) => s.profileId !== profileId), retryResult.connection];
            app.setActiveConnections(sessions);
            app.notify("success", t("connectionManager.connectedTo", { label: retryResult.connection.label }));
            app.navigate(retryResult.connection.rootPath);
            app.closeConnectionManager();
          } else {
            app.notify("error", retryResult.message ?? t("connectionManager.connectionFailedAfterTrust"));
          }
        }
        return;
      }

      if (result.connected && result.connection) {
        sessions = [...sessions.filter((s) => s.profileId !== profileId), result.connection];
        app.setActiveConnections(sessions);
        app.notify("success", t("connectionManager.connectedTo", { label: result.connection.label }));
        app.navigate(result.connection.rootPath);
        app.closeConnectionManager();
      } else {
        app.notify("error", result.message ?? t("connectionManager.couldNotConnect"));
      }
    } catch (e) {
      if (runId !== connectRunId) return;
      app.notify("error", formatInvokeError(e));
    } finally {
      if (runId === connectRunId) {
        connectingId = null;
      }
    }
  }

  // ── Disconnect ────────────────────────────────────────────
  async function disconnectSession(sessionId: string) {
    const runId = ++disconnectRunId;
    disconnectingId = sessionId;
    try {
      await invokeWithTimeout(
        "disconnect_connection",
        { sessionId },
        t("connectionManager.disconnectTimedOut"),
      );
      if (runId !== disconnectRunId) return;
      sessions = sessions.filter((s) => s.sessionId !== sessionId);
      app.setActiveConnections(sessions);
    } catch (e) {
      if (runId !== disconnectRunId) return;
      app.notify("error", formatInvokeError(e));
    } finally {
      if (runId === disconnectRunId) {
        disconnectingId = null;
      }
    }
  }

  // ── Protocol badge helpers ────────────────────────────────
  const PROTO_COLORS: Record<string, string> = {
    ssh: "#2a7db5",
    smb: "#7a5fb5",
    ftp: "#3f9a6a",
    ftps: "#2a8060",
  };

  function protoLabel(p: string): string {
    return p.toUpperCase();
  }
</script>

<aside class="cm-panel" aria-label={t("connectionManager.title")}>

  <!-- ── Header ── -->
  <div class="cm-header">
    <span class="cm-title">{t("connectionManager.title")}</span>
    <div class="cm-header-actions">
      <Button variant="primary" size="sm" onclick={startCreate} disabled={showForm}>
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
          <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
        </svg>
        {t("connectionManager.new")}
      </Button>
      <Button variant="ghost" size="sm" onclick={() => app.closeConnectionManager()} aria-label={t("connectionManager.close")}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </Button>
    </div>
  </div>

  <div class="cm-body">

    <!-- ── Profile form (create / edit) ── -->
    {#if showForm}
      <section class="cm-section">
        <div class="cm-section-title">
          {isCreating ? t("connectionManager.newConnection") : t("connectionManager.editConnection")}
        </div>
        {#if testResult}
          <div class="test-result" class:test-result--ok={testResult.ok} class:test-result--fail={!testResult.ok}>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
              {#if testResult.ok}
                <polyline points="20 6 9 17 4 12"/>
              {:else}
                <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
              {/if}
            </svg>
            {testResult.message}
          </div>
          {#if testResult.ok && testResult.writeAccess != null}
            <div class="test-write-access" class:test-write-access--ok={testResult.writeAccess} class:test-write-access--no={!testResult.writeAccess}>
              <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                {#if testResult.writeAccess}
                  <polyline points="20 6 9 17 4 12"/>
                {:else}
                  <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
                {/if}
              </svg>
              {#if testResult.writeAccess}
                {t("connectionManager.writeAccessOk")}
              {:else}
                {t("connectionManager.writeAccessDenied")}
              {/if}
            </div>
          {/if}
        {/if}
        <ConnectionForm
          profile={formProfile}
          {isSaving}
          {isTesting}
          onSave={saveProfile}
          onTest={testProfile}
          onCancel={cancelForm}
        />
      </section>
    {/if}

    <!-- ── Active sessions ── -->
    {#if sessions.length > 0}
      <section class="cm-section">
        <div class="cm-section-title">{t("connectionManager.activeSessions", { count: sessions.length })}</div>
        <div class="session-list">
          {#each sessions as session (session.sessionId)}
            <div class="session-card">
              <div class="session-icon" style="color:{PROTO_COLORS[session.protocol] ?? 'var(--accent)'}">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                  <rect x="2" y="3" width="20" height="14" rx="2"/>
                  <path d="M8 21h8M12 17v4"/>
                </svg>
              </div>
              <div class="session-info">
                <div class="session-label">{session.label}</div>
                <div class="session-detail">
                  <span class="proto-badge" style="background:{PROTO_COLORS[session.protocol]}20;color:{PROTO_COLORS[session.protocol]}">
                    {protoLabel(session.protocol)}
                  </span>
                  {session.host}
                </div>
                {#if session.displayPath}
                  <div class="session-path">{session.displayPath}</div>
                {/if}
              </div>
              <button
                class="session-disconnect"
                onclick={() => disconnectSession(session.sessionId)}
                disabled={disconnectingId === session.sessionId}
                title={t("connectionManager.disconnect")}
                aria-label="{t('connectionManager.disconnect')}: {session.label}"
              >
                {#if disconnectingId === session.sessionId}
                  <span class="spinner-sm"></span>
                {:else}
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/>
                    <polyline points="16 17 21 12 16 7"/>
                    <line x1="21" y1="12" x2="9" y2="12"/>
                  </svg>
                {/if}
              </button>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <!-- ── Saved profiles ── -->
    <section class="cm-section">
      <div class="cm-section-title">{t("connectionManager.savedConnections", { count: profiles.length })}</div>
      {#if profiles.length === 0}
        <div class="cm-empty">
          {t("connectionManager.noSavedConnections")}{" "}
          <button class="link-btn" onclick={startCreate}>{t("connectionManager.addOne")}</button>
        </div>
      {:else}
        <div class="profile-list">
          {#each profiles as profile (profile.id)}
            <div
              class="profile-item"
              class:profile-item--selected={selectedProfileId === profile.id}
              role="button"
              tabindex="0"
              onclick={() => startEdit(profile)}
              onkeydown={(e) => e.key === "Enter" && startEdit(profile)}
            >
              <div class="profile-item-left">
                <span class="proto-dot" style="background:{PROTO_COLORS[profile.protocol] ?? 'var(--accent)'}"></span>
                <div class="profile-item-info">
                  <span class="profile-item-label">{profile.label || profile.host}</span>
                  <span class="profile-item-sub">
                    <span class="proto-badge" style="background:{PROTO_COLORS[profile.protocol]}20;color:{PROTO_COLORS[profile.protocol]}">
                      {protoLabel(profile.protocol)}
                    </span>
                    {profile.host}{profile.username ? ` · ${profile.username}` : ""}
                  </span>
                </div>
              </div>

              <div class="profile-item-actions">
                <!-- Delete -->
                {#if deleteConfirmId === profile.id}
                  <button
                    class="action-btn action-btn--danger"
                    onclick={(e) => { e.stopPropagation(); deleteProfile(profile.id); }}
                    title={t("connectionManager.confirmDelete")}
                  >
                    {t("connectionManager.confirm")}
                  </button>
                  <button
                    class="action-btn"
                    onclick={(e) => { e.stopPropagation(); deleteConfirmId = null; }}
                    title={t("connectionManager.cancel")}
                  >
                    {t("connectionManager.cancel")}
                  </button>
                {:else}
                  <button
                    class="icon-btn icon-btn--danger"
                    onclick={(e) => { e.stopPropagation(); deleteConfirmId = profile.id; }}
                    title={t("connectionManager.deleteProfile")}
                    aria-label="{t('connectionManager.deleteProfile')}: {profile.label || profile.host}"
                  >
                    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <polyline points="3 6 5 6 21 6"/>
                      <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/>
                      <path d="M10 11v6M14 11v6M9 6V4h6v2"/>
                    </svg>
                  </button>
                {/if}

                <!-- Connect button -->
                <button
                  class="connect-btn"
                  onclick={(e) => { e.stopPropagation(); connectProfile(profile.id); }}
                  disabled={connectingId === profile.id}
                  title={t("connectionManager.connect")}
                  aria-label="{t('connectionManager.connect')}: {profile.label || profile.host}"
                >
                  {#if connectingId === profile.id}
                    <span class="spinner-sm"></span>
                  {:else}
                    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <polyline points="5 12 19 12 14 7"/><polyline points="14 17 19 12"/>
                    </svg>
                  {/if}
                  {connectingId === profile.id ? t("connectionManager.connecting") : t("connectionManager.connect")}
                </button>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </section>
  </div>
</aside>

<style>
  .cm-panel {
    display: flex;
    flex-direction: column;
    width: 420px;
    height: 100%;
    background: var(--surface);
    border-right: 1px solid var(--line-strong);
    overflow: hidden;
    flex-shrink: 0;
  }

  /* ── Header ── */
  .cm-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
    gap: 8px;
  }

  .cm-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
  }

  .cm-header-actions {
    display: flex;
    gap: 4px;
  }

  /* ── Body ── */
  .cm-body {
    flex: 1;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--line-strong) transparent;
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  /* ── Sections ── */
  .cm-section {
    padding: 10px 12px;
    border-bottom: 1px solid var(--line);
  }

  .cm-section-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-subtle);
    margin-bottom: 8px;
  }

  .cm-empty {
    font-size: 13px;
    color: var(--text-muted);
    padding: 4px 0;
  }

  .link-btn {
    background: none;
    border: none;
    color: var(--accent);
    font: inherit;
    padding: 0;
    text-decoration: underline;
    cursor: pointer;
  }

  /* ── Test result banner ── */
  .test-result {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 10px;
    border-radius: 6px;
    font-size: 12px;
    margin-bottom: 10px;
  }

  .test-result--ok {
    background: var(--accent-soft);
    color: var(--accent);
    border: 1px solid var(--accent-border);
  }

  .test-result--fail {
    background: var(--danger-soft);
    color: var(--danger);
    border: 1px solid var(--danger-border);
  }

  .test-write-access {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    border-radius: 6px;
    font-size: 11.5px;
    margin-top: -6px;
    margin-bottom: 10px;
  }

  .test-write-access--ok {
    background: var(--accent-soft);
    color: var(--accent);
    border: 1px solid var(--accent-border);
    opacity: 0.85;
  }

  .test-write-access--no {
    background: color-mix(in srgb, #f59e0b 12%, var(--surface));
    color: #b45309;
    border: 1px solid color-mix(in srgb, #f59e0b 35%, var(--line));
  }

  /* ── Active sessions ── */
  .session-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .session-card {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    background: var(--surface-alt);
    border: 1px solid var(--line);
    border-radius: 7px;
  }

  .session-icon {
    flex-shrink: 0;
  }

  .session-info {
    flex: 1;
    min-width: 0;
  }

  .session-label {
    font-size: 13px;
    font-weight: 500;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .session-detail {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 2px;
  }

  .session-path {
    font-size: 11px;
    color: var(--text-subtle);
    font-family: var(--font-mono);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: 2px;
  }

  .session-disconnect {
    background: none;
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    color: var(--text-muted);
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;

    &:hover:not(:disabled) {
      background: var(--danger-soft);
      border-color: var(--danger-border);
      color: var(--danger);
    }
  }

  /* ── Protocol badge ── */
  .proto-badge {
    display: inline-block;
    padding: 1px 5px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
  }

  /* ── Profile list ── */
  .profile-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .profile-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 10px;
    border-radius: 6px;
    border: 1px solid transparent;
    cursor: pointer;
    transition: background 0.1s, border-color 0.1s;

    &:hover {
      background: var(--surface-hover);
      border-color: var(--line);
    }
  }

  .profile-item--selected {
    background: var(--accent-soft);
    border-color: var(--accent-border);
  }

  .profile-item-left {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
  }

  .proto-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .profile-item-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .profile-item-label {
    font-size: 13px;
    font-weight: 500;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .profile-item-sub {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .profile-item-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  .icon-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    padding: 4px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;

    &:hover { background: var(--surface-hover); color: var(--text); }
  }

  .icon-btn--danger:hover {
    background: var(--danger-soft) !important;
    color: var(--danger) !important;
  }

  .action-btn {
    font-size: 11px;
    font-weight: 500;
    padding: 3px 7px;
    border: 1px solid var(--line-strong);
    border-radius: 4px;
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
    white-space: nowrap;

    &:hover { background: var(--surface-hover); }
  }

  .action-btn--danger {
    background: var(--danger-soft);
    border-color: var(--danger-border);
    color: var(--danger);

    &:hover { background: var(--danger); color: #fff; }
  }

  .connect-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    border-radius: 5px;
    font-size: 12px;
    font-weight: 500;
    border: 1px solid var(--accent-border);
    background: var(--accent-soft);
    color: var(--accent);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
    white-space: nowrap;

    &:hover:not(:disabled) {
      background: var(--accent);
      color: #fff;
    }

    &:disabled { opacity: 0.6; cursor: default; }
  }

  /* ── Spinners ── */
  .spinner-sm {
    display: inline-block;
    width: 11px;
    height: 11px;
    border: 1.5px solid transparent;
    border-top-color: currentColor;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }
</style>
