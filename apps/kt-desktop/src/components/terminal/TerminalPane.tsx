import { useEffect, useRef, useCallback } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";

import * as tauri from "../../lib/tauri";
import { terminalTheme, terminalConfig } from "../../lib/theme";
import { shouldPassThroughTerminal } from "../../lib/keyboard";
import { toast } from "../../stores/toast";

interface TerminalPaneProps {
  sessionId: string;
  isActive?: boolean;
}

/** Track loaded addons for proper cleanup */
interface LoadedAddons {
  fit: FitAddon;
  webgl: WebglAddon | null;
  webLinks: WebLinksAddon;
}

export function TerminalPane({ sessionId, isActive = true }: TerminalPaneProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const addonsRef = useRef<LoadedAddons | null>(null);

  // Initialize terminal
  useEffect(() => {
    if (!containerRef.current) return;

    // Create terminal instance with centralized config
    const terminal = new Terminal({
      ...terminalConfig,
      theme: terminalTheme,
    });

    // Create and track addons for proper cleanup
    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);

    // Try to load WebGL addon (falls back gracefully if unavailable)
    let webglAddon: WebglAddon | null = null;
    try {
      webglAddon = new WebglAddon();
      webglAddon.onContextLoss(() => {
        webglAddon?.dispose();
      });
      terminal.loadAddon(webglAddon);
    } catch (e) {
      console.warn("WebGL addon not available:", e);
      webglAddon = null;
    }

    // Load web links addon for clickable URLs
    const webLinksAddon = new WebLinksAddon();
    terminal.loadAddon(webLinksAddon);

    // Store addon references for cleanup
    addonsRef.current = {
      fit: fitAddon,
      webgl: webglAddon,
      webLinks: webLinksAddon,
    };

    // Allow app shortcuts to pass through to the app
    // Mac: Cmd+key, Windows/Linux: Ctrl+Shift+key
    // Uses centralized keyboard utilities for consistency
    terminal.attachCustomKeyEventHandler((e) => {
      // Return false to let app handle it, true to let terminal handle it
      return !shouldPassThroughTerminal(e);
    });

    // Open terminal in container
    terminal.open(containerRef.current);

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

    // Defer initial fit to allow renderer to fully initialize
    // This prevents "dimensions undefined" errors with WebGL addon
    requestAnimationFrame(() => {
      if (terminalRef.current && fitAddonRef.current) {
        try {
          fitAddonRef.current.fit();
          // Send initial size after fit
          const { cols, rows } = terminalRef.current;
          tauri.terminalResize(sessionId, cols, rows).catch((err) => {
            console.error("Failed to initial resize:", err);
          });
        } catch (e) {
          console.warn("Initial fit failed, will retry on next resize:", e);
        }
      }
    });

    // Cleanup - dispose addons explicitly before terminal
    return () => {
      // Dispose addons in reverse order of importance
      // WebGL should be disposed first as it holds GPU resources
      if (addonsRef.current) {
        const { webgl, webLinks, fit } = addonsRef.current;
        try {
          webgl?.dispose();
        } catch (e) {
          console.warn("Failed to dispose WebGL addon:", e);
        }
        try {
          webLinks.dispose();
        } catch (e) {
          console.warn("Failed to dispose WebLinks addon:", e);
        }
        try {
          fit.dispose();
        } catch (e) {
          console.warn("Failed to dispose Fit addon:", e);
        }
        addonsRef.current = null;
      }
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
    };
  }, [sessionId]);

  // Subscribe to terminal output from backend
  useEffect(() => {
    let unsubscribe: (() => void) | null = null;
    let subscriptionSucceeded = false;
    let isMounted = true;

    // Use async IIFE to properly chain subscription and listener setup
    (async () => {
      // First, subscribe to the session
      try {
        await tauri.subscribeSession(sessionId);
        subscriptionSucceeded = true;
      } catch (err) {
        console.error("Failed to subscribe to session:", err);
        toast.error("Failed to connect to terminal session");
        // Early return - don't set up output listener if subscription failed
        return;
      }

      // Only set up output listener if subscription succeeded and component still mounted
      if (!isMounted) return;

      try {
        const unlisten = await tauri.onTerminalOutput(sessionId, (data) => {
          if (terminalRef.current) {
            terminalRef.current.write(data);
          }
        });

        if (isMounted) {
          unsubscribe = unlisten;
        } else {
          // Component unmounted before we got the unlisten function - call it immediately
          unlisten();
        }
      } catch (err) {
        console.error("Failed to set up terminal output listener:", err);
        toast.error("Failed to receive terminal output");
      }
    })();

    return () => {
      isMounted = false;

      // Only unsubscribe if subscription succeeded
      if (subscriptionSucceeded) {
        tauri.unsubscribeSession(sessionId).catch((err) => {
          console.error("Failed to unsubscribe from session:", err);
        });
      }

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

  // Focus terminal when it becomes visible or when isActive changes to true
  useEffect(() => {
    if (isActive && terminalRef.current) {
      terminalRef.current.focus();
    }
  }, [isActive]);

  return (
    <div
      ref={containerRef}
      className="h-full w-full bg-bg-void"
      style={{
        padding: "16px 20px",
        // Subtle mauve glow in corner — like moonlight on concrete
        backgroundImage: "radial-gradient(ellipse at 15% 85%, rgba(155, 116, 137, 0.04) 0%, transparent 50%)",
      }}
    />
  );
}
