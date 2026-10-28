import { invoke } from "@tauri-apps/api/core";
import { app } from "../stores/app.svelte.js";
import { t } from "../i18n/index.js";
import type { ActiveConnectionDto, ConnectionOpenResultDto } from "../types/index.js";

const pending = new Map<string, Promise<ActiveConnectionDto | null>>();

/** Share an in-flight connection and its SSH trust prompt across both entry points. */
export function connectSavedProfile(profileId: string): Promise<ActiveConnectionDto | null> {
  const existing = pending.get(profileId);
  if (existing) return existing;
  const task = (async () => {
    let result = await invoke<ConnectionOpenResultDto>("connect_connection_profile", { profileId, trustCurrentFingerprint: false });
    if (result.requiresTrust && result.fingerprint) {
      const profile = app.connectionProfiles.find(p => p.id === profileId);
      if (!await app.promptFingerprint(result.fingerprint, profile?.label ?? profile?.host ?? profileId)) return null;
      result = await invoke<ConnectionOpenResultDto>("connect_connection_profile", { profileId, trustCurrentFingerprint: true });
    }
    if (!result.connected || !result.connection) throw new Error(result.message ?? t("connectionManager.couldNotConnect"));
    app.setActiveConnections([...app.activeConnections.filter(s => s.profileId !== profileId), result.connection]);
    return result.connection;
  })().finally(() => pending.delete(profileId));
  pending.set(profileId, task);
  return task;
}
