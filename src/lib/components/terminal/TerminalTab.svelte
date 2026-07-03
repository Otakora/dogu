<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import "@xterm/xterm/css/xterm.css";
  import { t } from "../../i18n/index.js";
  import { app } from "../../stores/app.svelte.js";

  type Props = {
    id: string;
    active: boolean;
  };

  let { id, active }: Props = $props();

  let containerEl: HTMLDivElement | undefined = $state();
  let term: Terminal | undefined;
  let fitAddon: FitAddon | undefined;
  let unlistenData: (() => void) | undefined;
  let unlistenExit: (() => void) | undefined;
  let unlistenCwd:  (() => void) | undefined;
  let exited = false;

  onMount(async () => {
    term = new Terminal({
      theme: {
        background: "#0d0d0d",
        foreground: "#cccccc",
        cursor: "#cccccc",
        cursorAccent: "#0d0d0d",
        selectionBackground: "#264f78",
        black: "#1e1e1e",
        red: "#f44747",
        green: "#4ec994",
        yellow: "#dcdcaa",
        blue: "#569cd6",
        magenta: "#c678dd",
        cyan: "#4ec9b0",
        white: "#cccccc",
        brightBlack: "#666666",
        brightRed: "#f44747",
        brightGreen: "#4ec994",
        brightYellow: "#dcdcaa",
        brightBlue: "#569cd6",
        brightMagenta: "#c678dd",
        brightCyan: "#4ec9b0",
        brightWhite: "#ffffff",
      },
      fontFamily: '"Cascadia Code", "Cascadia Mono", Consolas, "Courier New", monospace',
      fontSize: 13,
      lineHeight: 1.35,
      cursorBlink: true,
      scrollback: 5000,
      allowTransparency: false,
    });

    fitAddon = new FitAddon();
    term.loadAddon(fitAddon);

    if (containerEl) {
      term.open(containerEl);
      fitAddon.fit();
      syncSize();
    }

    term.onData((data) => {
      invoke("terminal_input", { id, data }).catch(() => {});
    });

    unlistenData = await listen<{ id: string; data: string }>(
      "terminal-data",
      ({ payload }) => {
        if (payload.id === id) term?.write(payload.data);
      }
    );

    unlistenExit = await listen<{ id: string }>(
      "terminal-exit",
      ({ payload }) => {
        if (payload.id === id && !exited) {
          exited = true;
          term?.writeln(`\r\n\x1b[2m${t("terminal.processExited")}\x1b[0m`);
        }
      }
    );

    unlistenCwd = await listen<{ id: string; cwd: string }>(
      "terminal-cwd",
      ({ payload }) => {
        if (payload.id === id) app.updateTerminalTabCwd(id, payload.cwd);
      }
    );
  });

  onDestroy(() => {
    unlistenData?.();
    unlistenExit?.();
    unlistenCwd?.();
    term?.dispose();
  });

  function syncSize() {
    if (!fitAddon || !term) return;
    const dims = fitAddon.proposeDimensions();
    if (dims) {
      invoke("resize_terminal", { id, cols: dims.cols, rows: dims.rows }).catch(() => {});
    }
  }

  export function fit() {
    if (!fitAddon || !containerEl) return;
    fitAddon.fit();
    syncSize();
    term?.focus();
  }

  $effect(() => {
    if (active) {
      setTimeout(fit, 20);
    }
  });
</script>

<div
  bind:this={containerEl}
  class="terminal-container"
  class:hidden={!active}
></div>

<style>
  .terminal-container {
    width: 100%;
    height: 100%;
    background: #0d0d0d;
    overflow: hidden;
  }

  .hidden {
    display: none;
  }

  :global(.xterm) {
    height: 100%;
    padding: 6px 8px;
  }

  :global(.xterm-viewport) {
    background-color: transparent !important;
  }

  :global(.xterm-screen) {
    height: 100% !important;
  }
</style>
