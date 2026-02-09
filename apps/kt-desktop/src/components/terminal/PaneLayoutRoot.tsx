/**
 * Root component for the pane layout system
 *
 * Provides:
 * - Keyboard shortcuts for splitting/closing panes
 * - Empty state when no panes exist
 * - Focus management
 * - Error boundary to catch xterm.js crashes
 */

import { useEffect, useCallback, Component, type ReactNode } from "react";
import { PaneContainer } from "./PaneContainer";
import { useLayoutStore } from "../../stores/layout";
import { useTerminalsStore } from "../../stores/terminals";
import { AlertTriangleIcon } from "../Icons";
import { isAppShortcut, getShortcutKey, isMac } from "../../lib/keyboard";

/**
 * Error boundary specifically for the terminal/pane view.
 * Catches xterm.js errors and provides recovery options without crashing the entire app.
 */
interface TerminalErrorBoundaryProps {
  children: ReactNode;
  onReset?: () => void;
}

interface TerminalErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

class TerminalErrorBoundary extends Component<TerminalErrorBoundaryProps, TerminalErrorBoundaryState> {
  constructor(props: TerminalErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): TerminalErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error("Terminal view error:", error, errorInfo);
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: null });
  };

  handleResetLayout = () => {
    this.setState({ hasError: false, error: null });
    this.props.onReset?.();
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="h-full flex items-center justify-center bg-bg-void">
          <div className="max-w-md text-center p-8">
            {/* Error icon */}
            <div className="w-16 h-16 mx-auto mb-6 rounded-full bg-terracotta/10 flex items-center justify-center">
              <AlertTriangleIcon className="w-8 h-8 text-terracotta" />
            </div>

            {/* Error message */}
            <h2 className="text-xl font-semibold mb-2 text-text-primary">
              Terminal View Error
            </h2>
            <p className="text-text-muted mb-6">
              The terminal encountered an unexpected error. You can try again or reset the layout.
            </p>

            {/* Error details (collapsed by default) */}
            {this.state.error && (
              <details className="mb-6 text-left">
                <summary className="cursor-pointer text-sm text-text-ghost hover:text-text-muted">
                  Technical details
                </summary>
                <pre className="mt-2 p-3 bg-bg-surface rounded-zen text-xs text-terracotta overflow-auto max-h-32">
                  {this.state.error.message}
                </pre>
              </details>
            )}

            {/* Action buttons */}
            <div className="flex gap-3 justify-center">
              <button
                onClick={this.handleRetry}
                className="px-4 py-2 text-sm font-medium rounded-zen bg-bg-elevated hover:bg-bg-hover border border-border text-text-secondary transition-colors"
              >
                Try Again
              </button>
              <button
                onClick={this.handleResetLayout}
                className="px-4 py-2 text-sm font-medium rounded-zen bg-mauve hover:bg-mauve-mid text-white transition-colors"
              >
                Reset Layout
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

export function PaneLayoutRoot() {
  const layout = useLayoutStore((s) => s.layout);
  const splitActivePane = useLayoutStore((s) => s.splitActivePane);
  const closeActivePane = useLayoutStore((s) => s.closeActivePane);
  const focusNextPane = useLayoutStore((s) => s.focusNextPane);
  const resetLayout = useLayoutStore((s) => s.resetLayout);
  const activeTabId = useTerminalsStore((s) => s.activeTabId);
  const tabCount = useTerminalsStore((s) => s.tabs.length);

  // Keyboard shortcuts
  // Mac: Cmd+key (matches iTerm2)
  // Windows/Linux: Ctrl+Shift+key (avoids terminal conflicts like Ctrl+C, Ctrl+D)
  // Uses centralized keyboard utilities for consistency
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      // Don't intercept if typing in an input
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement
      ) {
        return;
      }

      if (!isAppShortcut(e)) return;

      const key = getShortcutKey(e);
      const macPlatform = isMac();

      // Split horizontally (side by side)
      // Mac: Cmd+D, Win/Linux: Ctrl+Shift+D
      if (key === "d" && (macPlatform ? !e.shiftKey : true)) {
        e.preventDefault();
        if (activeTabId) {
          splitActivePane("horizontal", activeTabId);
        }
        return;
      }

      // Split vertically (stacked)
      // Mac: Cmd+Shift+D, Win/Linux: Ctrl+Shift+Alt+D
      if (key === "d" && e.shiftKey && (macPlatform || e.altKey)) {
        e.preventDefault();
        if (activeTabId) {
          splitActivePane("vertical", activeTabId);
        }
        return;
      }

      // Close active pane
      // Mac: Cmd+W, Win/Linux: Ctrl+Shift+W
      if (key === "w") {
        e.preventDefault();
        closeActivePane();
        return;
      }

      // Focus next pane
      // Mac: Cmd+], Win/Linux: Ctrl+Shift+]
      if (e.key === "]") {
        e.preventDefault();
        focusNextPane(1);
        return;
      }

      // Focus previous pane
      // Mac: Cmd+[, Win/Linux: Ctrl+Shift+[
      if (e.key === "[") {
        e.preventDefault();
        focusNextPane(-1);
        return;
      }
    },
    [activeTabId, splitActivePane, closeActivePane, focusNextPane]
  );

  useEffect(() => {
    // Use capture phase to intercept before xterm gets the event
    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, [handleKeyDown]);

  // Empty state - also show if no tabs exist (stale layout from HMR)
  if (!layout.root || tabCount === 0) {
    return <EmptyState />;
  }

  return (
    <TerminalErrorBoundary onReset={resetLayout}>
      <div className="h-full w-full">
        <PaneContainer node={layout.root} />
      </div>
    </TerminalErrorBoundary>
  );
}

function EmptyState() {
  return (
    <div className="h-full flex items-center justify-center bg-bg-void">
      <div className="text-center">
        <div className="text-6xl mb-4 opacity-30">⌨</div>
        <h2 className="text-xl font-medium mb-2 text-text-secondary">
          No Active Sessions
        </h2>
        <p className="text-text-muted max-w-md">
          Select a machine from the sidebar and click the + button to start a
          new terminal session.
        </p>
      </div>
    </div>
  );
}
