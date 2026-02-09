import { useAppStore, type ViewMode } from "../../stores/app";
import { clsx } from "clsx";
import { useState, useCallback } from "react";
import { MenuIcon } from "../Icons";

const viewModes: { id: ViewMode; label: string }[] = [
  { id: "terminals", label: "Terminals" },
  { id: "topology", label: "Topology" },
  { id: "health", label: "Health" },
  { id: "logs", label: "Logs" },
];

export function Header() {
  const viewMode = useAppStore((s) => s.viewMode);
  const setViewMode = useAppStore((s) => s.setViewMode);
  const toggleSidebar = useAppStore((s) => s.toggleSidebar);
  const showSidebar = useAppStore((s) => s.showSidebar);
  const isConnected = useAppStore((s) => s.isConnected);
  const status = useAppStore((s) => s.orchestratorStatus);
  const [copiedCode, setCopiedCode] = useState(false);

  const copyPairingCode = useCallback(() => {
    if (status?.pairingCode) {
      navigator.clipboard.writeText(status.pairingCode);
      setCopiedCode(true);
      setTimeout(() => setCopiedCode(false), 2000);
    }
  }, [status?.pairingCode]);

  return (
    <header className="h-[46px] flex items-center bg-bg-surface border-b border-border px-4 gap-2">
      {/* Sidebar toggle */}
      <button
        onClick={toggleSidebar}
        className="p-1.5 rounded-zen hover:bg-bg-hover transition-colors text-text-muted hover:text-text-secondary"
        title={showSidebar ? "Hide sidebar" : "Show sidebar"}
        aria-label={showSidebar ? "Hide sidebar" : "Show sidebar"}
      >
        <MenuIcon className="w-4 h-4" />
      </button>

      {/* Logo — k in mauve, - in ghost, Terminus in primary */}
      <div className="flex items-center gap-2 px-3">
        <span className="text-sm font-bold tracking-tight">
          <span className="text-mauve-mid">k</span>
          <span className="text-text-ghost font-light">-</span>
          <span className="text-text-primary">Terminus</span>
        </span>
      </div>

      {/* Divider */}
      <div className="w-px h-4 bg-border" />

      {/* View mode tabs */}
      <nav className="flex items-center gap-1">
        {viewModes.map((mode) => (
          <button
            key={mode.id}
            onClick={() => setViewMode(mode.id)}
            className={clsx(
              "relative px-4 py-1.5 text-xs font-medium rounded-zen transition-colors",
              viewMode === mode.id
                ? "bg-bg-elevated text-text-primary"
                : "text-text-muted hover:text-text-secondary hover:bg-bg-hover"
            )}
          >
            {mode.label}
            {/* Light slit under active tab */}
            {viewMode === mode.id && (
              <span className="absolute bottom-0 left-1/4 right-1/4 h-px bg-mauve opacity-60 rounded" />
            )}
          </button>
        ))}
      </nav>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Pairing code */}
      {isConnected && status?.pairingCode && (
        <>
          <button
            onClick={copyPairingCode}
            className="flex items-center gap-2 px-3 py-1 text-xs rounded-zen bg-bg-elevated hover:bg-bg-hover border border-border-faint transition-colors group"
            title="Pairing code - click to copy"
            aria-label={`Copy pairing code ${status.pairingCode}`}
          >
            <span className="text-text-ghost uppercase text-[10px] tracking-wide">
              Code
            </span>
            <span className="font-mono font-bold tracking-wider text-mauve-mid">
              {status.pairingCode}
            </span>
            <span className="text-[10px] text-text-ghost group-hover:text-text-muted">
              {copiedCode ? "Copied!" : ""}
            </span>
          </button>
          <div className="w-px h-4 bg-border" />
        </>
      )}

      {/* Connection status */}
      <div className="flex items-center gap-2 px-2 text-xs">
        <div
          className={clsx(
            "w-[7px] h-[7px] rounded-full",
            isConnected ? "bg-sage" : "bg-terracotta-dim"
          )}
        />
        <span className="text-text-muted">
          {isConnected
            ? `${status?.machineCount ?? 0} machines`
            : "Disconnected"}
        </span>
      </div>
    </header>
  );
}
