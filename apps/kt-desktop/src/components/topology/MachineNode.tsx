import { Handle, Position } from "@xyflow/react";
import { clsx } from "clsx";
import type { Machine } from "../../types";
import { useTerminalsStore } from "../../stores/terminals";
import { useAppStore } from "../../stores/app";
import { toast } from "../../stores/toast";
import * as tauri from "../../lib/tauri";

interface MachineNodeProps {
  data: { machine: Machine };
}

export function MachineNode({ data }: MachineNodeProps) {
  const { machine } = data;
  const addTab = useTerminalsStore((s) => s.addTab);
  const addSession = useTerminalsStore((s) => s.addSession);
  const setViewMode = useAppStore((s) => s.setViewMode);

  const isConnected = machine.status === "connected";
  const isConnecting = machine.status === "connecting";

  const handleDoubleClick = async () => {
    if (!isConnected) return;

    // Ignore replayed events from bfcache/session restore
    if (!window.__appReady) {
      console.info("[MachineNode] Ignoring double-click - app not ready yet");
      return;
    }

    try {
      const session = await tauri.createSession(machine.id);
      addSession(session);
      addTab({
        id: `tab-${session.id}`,
        sessionId: session.id,
        machineId: machine.id,
        title: machine.alias || machine.hostname,
      });
      // Switch to Terminals view to show the new session
      setViewMode("terminals");
    } catch (err) {
      console.error("Failed to create session:", err);
      toast.error(`Failed to create session on ${machine.alias || machine.hostname}`);
    }
  };

  return (
    <div
      onDoubleClick={handleDoubleClick}
      className={clsx(
        "px-4 py-3 rounded-zen border-2 bg-bg-surface min-w-[150px] cursor-pointer transition-all",
        isConnected && "border-border hover:border-sage/50",
        isConnecting && "border-ochre-dim",
        !isConnected && !isConnecting && "border-border-faint opacity-50"
      )}
    >
      {/* Connection handle */}
      <Handle
        type="target"
        position={Position.Top}
        className="!bg-border !w-2.5 !h-2.5 !border-2 !border-bg-surface !rounded-sm"
      />

      {/* Header */}
      <div className="flex items-center gap-2 mb-1">
        <div
          className={clsx(
            "w-1.5 h-1.5 rounded-full flex-shrink-0",
            isConnected && "bg-sage",
            isConnecting && "bg-ochre animate-breathe",
            !isConnected && !isConnecting && "bg-terracotta-dim"
          )}
        />
        <span className="font-medium text-xs text-text-primary truncate">
          {machine.alias || machine.hostname}
        </span>
      </div>

      {/* Details */}
      <div className="text-[10px] text-text-ghost space-y-0.5 ml-[14px]">
        <div>{machine.os}/{machine.arch}</div>
        {machine.sessionCount > 0 && (
          <div>{machine.sessionCount} sessions</div>
        )}
      </div>

      {/* Tags */}
      {machine.tags && machine.tags.length > 0 && (
        <div className="flex flex-wrap gap-1 mt-2 ml-[14px]">
          {machine.tags.map((tag) => (
            <span
              key={tag}
              className="text-[9px] font-medium uppercase tracking-[0.5px] px-1.5 py-px border border-border rounded-sm text-text-muted"
            >
              {tag}
            </span>
          ))}
        </div>
      )}

      {/* Double-click hint */}
      {isConnected && (
        <div className="mt-2 text-[10px] text-text-ghost text-center">
          Double-click to connect
        </div>
      )}
    </div>
  );
}
