import { useMemo, useCallback, useRef, memo } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useTerminalsStore } from "../../stores/terminals";
import { useMachinesStore } from "../../stores/machines";
import { toast } from "../../stores/toast";
import * as tauri from "../../lib/tauri";
import { clsx } from "clsx";
import { TerminalIcon, XIcon } from "../Icons";
import type { TerminalTab } from "../../types";

// Row height for virtual list
const SESSION_ROW_HEIGHT = 52;

export function SessionList() {
  const tabs = useTerminalsStore((s) => s.tabs);
  const activeTabId = useTerminalsStore((s) => s.activeTabId);
  const setActiveTab = useTerminalsStore((s) => s.setActiveTab);
  const removeTab = useTerminalsStore((s) => s.removeTab);
  const machines = useMachinesStore((s) => s.machines);
  const parentRef = useRef<HTMLDivElement>(null);

  // Create O(1) lookup map for machines
  const machineMap = useMemo(
    () => new Map(machines.map((m) => [m.id, m])),
    [machines]
  );

  const getMachineName = useCallback(
    (machineId: string) => {
      const machine = machineMap.get(machineId);
      return machine?.alias || machine?.hostname || machineId.slice(0, 8);
    },
    [machineMap]
  );

  const virtualizer = useVirtualizer({
    count: tabs.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => SESSION_ROW_HEIGHT,
    overscan: 5, // Render 5 extra items above/below viewport
  });

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-2 flex items-center justify-between">
        <span className="text-xs text-text-ghost uppercase tracking-wider">
          Active Sessions ({tabs.length})
        </span>
      </div>

      {/* Session list */}
      <div ref={parentRef} className="flex-1 overflow-auto">
        {tabs.length === 0 ? (
          <div className="p-4 text-center text-text-muted text-sm">
            No active sessions
            <br />
            <span className="text-xs text-text-ghost">
              Click + on a machine to start one
            </span>
          </div>
        ) : (
          <div
            style={{
              height: `${virtualizer.getTotalSize()}px`,
              width: "100%",
              position: "relative",
            }}
          >
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const tab = tabs[virtualRow.index];
              return (
                <div
                  key={virtualRow.key}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    height: `${virtualRow.size}px`,
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                >
                  <SessionItem
                    tab={tab}
                    isActive={tab.id === activeTabId}
                    machineName={getMachineName(tab.machineId)}
                    onSelect={() => setActiveTab(tab.id)}
                    onKill={() => removeTab(tab.id)}
                  />
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

// Memoized session item to prevent unnecessary re-renders
const SessionItem = memo(function SessionItem({
  tab,
  isActive,
  machineName,
  onSelect,
  onKill,
}: {
  tab: TerminalTab;
  isActive: boolean;
  machineName: string;
  onSelect: () => void;
  onKill: () => void;
}) {
  const handleKillSession = async (e: React.MouseEvent) => {
    e.stopPropagation();

    try {
      await tauri.killSession(tab.sessionId);
      onKill();
    } catch (err) {
      console.error("Failed to kill session:", err);
      toast.error("Failed to kill session");
    }
  };

  return (
    <div
      onClick={onSelect}
      className={clsx(
        "group flex items-center gap-2 px-3 py-2 cursor-pointer transition-colors",
        isActive
          ? "bg-mauve/5 border-l-2 border-mauve"
          : "hover:bg-bg-hover border-l-2 border-transparent"
      )}
    >
      {/* Terminal icon */}
      <div className="w-5 h-5 flex items-center justify-center text-sage">
        <TerminalIcon className="w-4 h-4" />
      </div>

      {/* Session info */}
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium truncate text-text-primary">{tab.title}</div>
        <div className="text-xs text-text-ghost truncate">
          {machineName}
        </div>
      </div>

      {/* Kill button */}
      <button
        onClick={handleKillSession}
        className="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-terracotta/20 transition-all"
        title="Kill session"
        aria-label={`Kill session ${tab.title}`}
      >
        <XIcon className="w-4 h-4 text-terracotta" />
      </button>
    </div>
  );
});
