import { useState, useMemo, useRef, useCallback, memo } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useMachinesStore } from "../../stores/machines";
import { useTerminalsStore } from "../../stores/terminals";
import { toast } from "../../stores/toast";
import * as tauri from "../../lib/tauri";
import { clsx } from "clsx";
import { PlusIcon, XIcon } from "../Icons";
import type { Machine } from "../../types";

// Row types for virtual list
type VirtualRow =
  | { type: "header"; label: string; count: number }
  | { type: "machine"; machine: Machine };

// Estimated heights for virtual list
const HEADER_HEIGHT = 32;
const MACHINE_HEIGHT = 76; // Base height for a machine item
const MACHINE_WITH_TAGS_HEIGHT = 96; // Height when tags are present

export function MachineList() {
  const machines = useMachinesStore((s) => s.machines);
  const selectedId = useMachinesStore((s) => s.selectedMachineId);
  const selectMachine = useMachinesStore((s) => s.selectMachine);
  const [searchTerm, setSearchTerm] = useState("");
  const [activeTag, setActiveTag] = useState<string | null>(null);
  const parentRef = useRef<HTMLDivElement>(null);

  // Handle clicking a tag to filter
  const handleTagClick = useCallback((tag: string) => {
    if (activeTag === tag) {
      // Clicking same tag clears it
      setActiveTag(null);
      setSearchTerm("");
    } else {
      setActiveTag(tag);
      setSearchTerm(tag);
    }
  }, [activeTag]);

  // Pre-index tags for O(1) lookup: Map<machineId, Set<lowercaseTag>>
  const machineTagIndex = useMemo(() => {
    const index = new Map<string, Set<string>>();
    for (const machine of machines) {
      if (machine.tags && machine.tags.length > 0) {
        index.set(machine.id, new Set(machine.tags.map((t) => t.toLowerCase())));
      }
    }
    return index;
  }, [machines]);

  // Memoize filtering to prevent recalculation on every render
  // Uses pre-indexed tags for O(1) lookup instead of O(m) array iteration
  const { connectedMachines, disconnectedMachines } = useMemo(() => {
    const filtered = machines.filter((m) => {
      if (!searchTerm) return true;
      const term = searchTerm.toLowerCase();
      // Check hostname and alias (these are single value lookups, already O(1))
      if (m.hostname.toLowerCase().includes(term)) return true;
      if (m.alias && m.alias.toLowerCase().includes(term)) return true;
      // Use pre-indexed tag Set for O(1) lookup instead of O(m) array iteration
      const tagSet = machineTagIndex.get(m.id);
      if (tagSet) {
        // For partial matching, we still need to iterate, but only over the Set entries
        // This is unavoidable for substring matching but Set iteration is more efficient
        for (const tag of tagSet) {
          if (tag.includes(term)) return true;
        }
      }
      return false;
    });
    return {
      connectedMachines: filtered.filter((m) => m.status === "connected"),
      disconnectedMachines: filtered.filter((m) => m.status !== "connected"),
    };
  }, [machines, searchTerm, machineTagIndex]);

  // Flatten into virtual rows
  const virtualRows = useMemo<VirtualRow[]>(() => {
    const rows: VirtualRow[] = [];

    if (connectedMachines.length > 0) {
      rows.push({
        type: "header",
        label: "Connected",
        count: connectedMachines.length,
      });
      connectedMachines.forEach((machine) => {
        rows.push({ type: "machine", machine });
      });
    }

    if (disconnectedMachines.length > 0) {
      rows.push({
        type: "header",
        label: "Disconnected",
        count: disconnectedMachines.length,
      });
      disconnectedMachines.forEach((machine) => {
        rows.push({ type: "machine", machine });
      });
    }

    return rows;
  }, [connectedMachines, disconnectedMachines]);

  // Estimate row size based on type
  const estimateSize = useCallback(
    (index: number) => {
      const row = virtualRows[index];
      if (row.type === "header") return HEADER_HEIGHT;
      // Account for tags taking extra space
      if (row.machine.tags && row.machine.tags.length > 0) {
        return MACHINE_WITH_TAGS_HEIGHT;
      }
      return MACHINE_HEIGHT;
    },
    [virtualRows]
  );

  const virtualizer = useVirtualizer({
    count: virtualRows.length,
    getScrollElement: () => parentRef.current,
    estimateSize,
    overscan: 5, // Render 5 extra items above/below viewport
  });

  const hasNoMachines = machines.length === 0;
  const hasNoResults =
    !hasNoMachines &&
    connectedMachines.length === 0 &&
    disconnectedMachines.length === 0;

  return (
    <div className="h-full flex flex-col">
      {/* Search */}
      <div className="p-3 space-y-2">
        <input
          type="text"
          placeholder="Search machines..."
          className="input w-full"
          value={searchTerm}
          onChange={(e) => {
            setSearchTerm(e.target.value);
            // Clear active tag if user types something different
            if (activeTag && e.target.value !== activeTag) {
              setActiveTag(null);
            }
          }}
          aria-label="Search machines by hostname, alias, or tag"
        />
        {/* Active tag filter indicator */}
        {activeTag && (
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-text-ghost">Filtering by:</span>
            <button
              onClick={() => {
                setActiveTag(null);
                setSearchTerm("");
              }}
              className="inline-flex items-center gap-1 text-[9px] font-semibold uppercase tracking-[0.5px] px-1.5 py-0.5 border border-mauve text-mauve rounded-sm hover:bg-mauve/20 transition-colors"
            >
              {activeTag}
              <XIcon className="w-2.5 h-2.5" />
            </button>
          </div>
        )}
      </div>

      {/* Machine list */}
      <div ref={parentRef} className="flex-1 overflow-auto px-3 pb-3">
        {hasNoMachines ? (
          <div className="py-8 text-center text-text-ghost text-xs">
            No machines connected
          </div>
        ) : hasNoResults ? (
          <div className="py-8 text-center text-text-ghost text-xs">
            No machines match "{searchTerm}"
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
              const row = virtualRows[virtualRow.index];
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
                  {row.type === "header" ? (
                    <div className="px-1 py-2 text-[10px] font-semibold text-text-ghost uppercase tracking-[1.5px]">
                      {row.label} ({row.count})
                    </div>
                  ) : (
                    <MachineItem
                      machine={row.machine}
                      selected={row.machine.id === selectedId}
                      onSelect={() => selectMachine(row.machine.id)}
                      onTagClick={handleTagClick}
                    />
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

// Memoized machine item to prevent unnecessary re-renders
const MachineItem = memo(function MachineItem({
  machine,
  selected,
  onSelect,
  onTagClick,
}: {
  machine: Machine;
  selected: boolean;
  onSelect: () => void;
  onTagClick: (tag: string) => void;
}) {
  const [isCreatingSession, setIsCreatingSession] = useState(false);
  const addTab = useTerminalsStore((s) => s.addTab);
  const addSession = useTerminalsStore((s) => s.addSession);
  const removeMachine = useMachinesStore((s) => s.removeMachine);

  const handleConnect = async (e: React.MouseEvent) => {
    e.stopPropagation();
    console.info("[MachineList] handleConnect at", Date.now(), "appReady:", window.__appReady, "event:", e.type, e.isTrusted);

    // Ignore replayed click events from bfcache/session restore
    if (!window.__appReady) {
      console.info("[MachineList] Ignoring click - app not ready yet");
      return;
    }

    // Prevent double-click while session is being created
    if (isCreatingSession) return;

    setIsCreatingSession(true);
    try {
      const session = await tauri.createSession(machine.id);
      addSession(session);
      addTab({
        id: `tab-${session.id}`,
        sessionId: session.id,
        machineId: machine.id,
        title: machine.alias || machine.hostname,
      });
    } catch (err) {
      console.error("Failed to create session:", err);
      toast.error(
        `Failed to create session on ${machine.alias || machine.hostname}`
      );
    } finally {
      setIsCreatingSession(false);
    }
  };

  const handleDisconnect = async (e: React.MouseEvent) => {
    e.stopPropagation();

    try {
      await tauri.disconnectMachine(machine.id);
      removeMachine(machine.id);
    } catch (err) {
      console.error("Failed to disconnect machine:", err);
      toast.error(`Failed to disconnect ${machine.alias || machine.hostname}`);
    }
  };

  const isConnected = machine.status === "connected";
  const isConnecting = machine.status === "connecting";

  return (
    <div
      onClick={onSelect}
      className={clsx(
        "group relative border-2 rounded-zen p-3 cursor-pointer transition-all mb-1.5",
        selected
          ? "border-mauve-deep bg-mauve/5"
          : "border-border-faint bg-bg-base hover:border-border hover:bg-bg-elevated"
      )}
    >
      {/* Light slit on selected */}
      {selected && (
        <div className="absolute left-0 top-2 bottom-2 w-0.5 bg-mauve rounded-sm opacity-70" />
      )}

      <div className="flex flex-col gap-1">
        {/* Header: status + name */}
        <div className="flex items-center gap-2">
          <div
            className={clsx(
              "w-1.5 h-1.5 rounded-full flex-shrink-0",
              isConnected && "bg-sage",
              isConnecting && "bg-ochre animate-breathe",
              !isConnected && !isConnecting && "bg-terracotta-dim"
            )}
          />
          <span className="text-xs font-medium text-text-primary truncate">
            {machine.alias || machine.hostname}
          </span>
        </div>

        {/* Meta info */}
        <div className="text-[10px] text-text-ghost ml-[14px]">
          {machine.os}/{machine.arch}
          {machine.sessionCount > 0 && ` · ${machine.sessionCount} sessions`}
          {isConnecting && " · reconnecting…"}
        </div>

        {/* Tags - clickable to filter */}
        {machine.tags && machine.tags.length > 0 && (
          <div className="flex flex-wrap gap-1 mt-1 ml-[14px]">
            {machine.tags.map((tag) => (
              <button
                key={tag}
                onClick={(e) => {
                  e.stopPropagation();
                  onTagClick(tag);
                }}
                className={clsx(
                  "text-[9px] font-semibold uppercase tracking-[0.5px] px-1.5 py-px border rounded-sm transition-colors",
                  selected
                    ? "text-mauve-mid border-mauve-deep hover:bg-mauve/20"
                    : "text-text-muted border-border hover:border-mauve hover:text-mauve"
                )}
                title={`Filter by tag: ${tag}`}
              >
                {tag}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Action buttons on hover */}
      {isConnected && (
        <div className="absolute right-2 top-1/2 -translate-y-1/2 flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
          {/* New session button with loading state */}
          <button
            onClick={handleConnect}
            disabled={isCreatingSession}
            className={clsx(
              "p-1.5 rounded-zen transition-colors",
              isCreatingSession
                ? "opacity-50 cursor-not-allowed"
                : "hover:bg-mauve/20"
            )}
            title={isCreatingSession ? "Creating session..." : "New session"}
            aria-label={
              isCreatingSession
                ? "Creating session..."
                : `Create new session on ${machine.alias || machine.hostname}`
            }
          >
            <PlusIcon
              className={clsx(
                "w-3.5 h-3.5 text-mauve",
                isCreatingSession && "animate-pulse"
              )}
            />
          </button>

          {/* Disconnect button */}
          <button
            onClick={handleDisconnect}
            className="p-1.5 rounded-zen hover:bg-terracotta/20 transition-colors"
            title="Disconnect"
            aria-label={`Disconnect ${machine.alias || machine.hostname}`}
          >
            <XIcon className="w-3.5 h-3.5 text-terracotta" />
          </button>
        </div>
      )}
    </div>
  );
});
