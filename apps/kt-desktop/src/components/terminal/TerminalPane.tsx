import { useEffect, useRef, useCallback } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";

import * as tauri from "../../lib/tauri";

interface TerminalPaneProps {
  sessionId: string;
  machineId: string;
}

export function TerminalPane({ sessionId }: TerminalPaneProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);

  // Initialize terminal
  useEffect(() => {
    if (!containerRef.current) return;

    // Create terminal instance
    const terminal = new Terminal({
      cursorBlink: true,
      cursorStyle: "block",
      fontSize: 14,
      fontFamily: '"JetBrains Mono", "Fira Code", Monaco, Consolas, monospace',
      lineHeight: 1.2,
      theme: {
        background: "#1a1b26",
        foreground: "#c0caf5",
        cursor: "#c0caf5",
        cursorAccent: "#1a1b26",
        selectionBackground: "#33467c",
        selectionForeground: "#c0caf5",
        black: "#15161e",
        red: "#f7768e",
        green: "#9ece6a",
        yellow: "#e0af68",
        blue: "#7aa2f7",
        magenta: "#bb9af7",
        cyan: "#7dcfff",
        white: "#a9b1d6",
        brightBlack: "#414868",
        brightRed: "#f7768e",
        brightGreen: "#9ece6a",
        brightYellow: "#e0af68",
        brightBlue: "#7aa2f7",
        brightMagenta: "#bb9af7",
        brightCyan: "#7dcfff",
        brightWhite: "#c0caf5",
      },
      allowProposedApi: true,
    });

    // Create addons
    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);

    // Try to load WebGL addon (falls back gracefully if unavailable)
    try {
      const webglAddon = new WebglAddon();
      webglAddon.onContextLoss(() => {
        webglAddon.dispose();
      });
      terminal.loadAddon(webglAddon);
    } catch (e) {
      console.warn("WebGL addon not available:", e);
    }

    // Load web links addon for clickable URLs
    terminal.loadAddon(new WebLinksAddon());

    // Open terminal in container
    terminal.open(containerRef.current);
    fitAddon.fit();

    // Store refs
    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;

    // Handle input - send to backend
    terminal.onData((data) => {
      tauri.terminalWrite(sessionId, tauri.stringToBytes(data)).catch((err) => {
        console.error("Failed to write to terminal:", err);
      });
    });

    // Handle resize
    terminal.onResize(({ cols, rows }) => {
      tauri.terminalResize(sessionId, cols, rows).catch((err) => {
        console.error("Failed to resize terminal:", err);
      });
    });

    // Initial resize
    const { cols, rows } = terminal;
    tauri.terminalResize(sessionId, cols, rows).catch((err) => {
      console.error("Failed to initial resize:", err);
    });

    // Cleanup
    return () => {
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
    };
  }, [sessionId]);

  // Subscribe to terminal output from backend
  useEffect(() => {
    let unsubscribe: (() => void) | null = null;

    tauri.onTerminalOutput(sessionId, (data) => {
      if (terminalRef.current) {
        terminalRef.current.write(data);
      }
    }).then((unlisten) => {
      unsubscribe = unlisten;
    });

    return () => {
      if (unsubscribe) {
        unsubscribe();
      }
    };
  }, [sessionId]);

  // Handle container resize
  const handleResize = useCallback(() => {
    if (fitAddonRef.current && terminalRef.current) {
      fitAddonRef.current.fit();
    }
  }, []);

  // Set up resize observer
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const resizeObserver = new ResizeObserver(() => {
      // Debounce resize
      requestAnimationFrame(handleResize);
    });

    resizeObserver.observe(container);

    return () => {
      resizeObserver.disconnect();
    };
  }, [handleResize]);

  // Focus terminal when it becomes visible
  useEffect(() => {
    if (terminalRef.current) {
      terminalRef.current.focus();
    }
  }, []);

  return (
    <div
      ref={containerRef}
      className="h-full w-full bg-terminal-bg"
      style={{ padding: "4px" }}
    />
  );
}
