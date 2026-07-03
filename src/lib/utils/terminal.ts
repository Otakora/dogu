import { invoke } from "@tauri-apps/api/core";
import { app } from "../stores/app.svelte.js";

let _seq = 0;

function isRemotePath(path: string): boolean {
  return path.startsWith("remote://");
}

function tabTitleFromCwd(cwd: string | null | undefined): string {
  if (!cwd) return `Terminal ${++_seq}`;
  const name = cwd.replace(/[/\\]+$/, "").split(/[/\\]/).filter(Boolean).pop();
  return name || `Terminal ${++_seq}`;
}

export async function openTerminalAt(cwd: string, shell?: string): Promise<void> {
  if (isRemotePath(cwd)) {
    await openRemoteTerminalAt(cwd);
    return;
  }
  const id = `term-${Date.now()}-${++_seq}`;
  const title = tabTitleFromCwd(cwd);
  app.addTerminalTab({ id, title, cwd });
  try {
    await invoke("create_terminal", { id, cwd, shell: shell ?? null });
  } catch (e) {
    app.closeTerminalTab(id);
    app.notify("error", String(e));
  }
}

export async function openRemoteTerminalAt(virtualPath: string): Promise<void> {
  const id = `term-${Date.now()}-${++_seq}`;
  const remoteName = virtualPath.split("/").filter(Boolean).pop() ?? "SSH";
  const title = `SSH: ${remoteName}`;
  app.addTerminalTab({ id, title, cwd: virtualPath });
  try {
    await invoke("create_terminal_for_remote_session", { id, virtualPath });
  } catch (e) {
    app.closeTerminalTab(id);
    app.notify("error", String(e));
  }
}
