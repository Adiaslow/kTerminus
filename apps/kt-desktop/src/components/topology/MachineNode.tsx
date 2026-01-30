import { Handle, Position } from "@xyflow/react";
import { clsx } from "clsx";
import type { Machine } from "../../types";
import { useTerminalsStore } from "../../stores/terminals";
import * as tauri from "../../lib/tauri";

interface MachineNodeProps {
  data: { machine: Machine };
}

export function MachineNode({ data }: MachineNodeProps) {
  const { machine } = data;
  const addTab = useTerminalsStore((s) => s.addTab);
  const addSession = useTerminalsStore((s) => s.addSession);

  const isConnected = machine.status === "connected";
  const isConnecting = machine.status === "connecting";

  const handleDoubleClick = async () => {
    if (!isConnected) return;

    try {
      const session = await tauri.createSession(machine.id);
      addSession(session);
      addTab({
        id: `tab-${session.id}`,
        sessionId: session.id,
        machineId: machine.id,
        title: machine.alias || machine.hostname,
        active: true,
      });
    } catch (err) {
      console.error("Failed to create session:", err);
    }
  };

  return (
    <div
      onDoubleClick={handleDoubleClick}
      className={clsx(
        "px-4 py-3 rounded-lg border-2 bg-sidebar-bg min-w-[160px] cursor-pointer transition-all",
        "hover:shadow-lg hover:shadow-terminal-blue/20",
        isConnected && "border-terminal-green",
        isConnecting && "border-terminal-yellow",
        !isConnected && !isConnecting && "border-terminal-red opacity-60"
      )}
    >
      {/* Connection handle */}
      <Handle
        type="target"
        position={Position.Top}
        className="!bg-terminal-fg/30 !w-3 !h-3 !border-2 !border-sidebar-bg"
      />

      {/* Header */}
      <div className="flex items-center gap-2 mb-2">
        <div
          className={clsx(
            "w-2.5 h-2.5 rounded-full",
            isConnected && "bg-terminal-green",
            isConnecting && "bg-terminal-yellow animate-pulse",
            !isConnected && !isConnecting && "bg-terminal-red"
          )}
        />
        <span className="font-medium text-sm truncate">
          {machine.alias || machine.hostname}
        </span>
      </div>

      {/* Details */}
      <div className="text-xs text-terminal-fg/60 space-y-0.5">
        <div className="flex items-center gap-1">
          <span>💻</span>
          <span>{machine.os}/{machine.arch}</span>
        </div>
        {machine.sessionCount > 0 && (
          <div className="flex items-center gap-1">
            <span>📺</span>
            <span>{machine.sessionCount} sessions</span>
          </div>
        )}
        {machine.tags && machine.tags.length > 0 && (
          <div className="flex flex-wrap gap-1 mt-1">
            {machine.tags.map((tag) => (
              <span
                key={tag}
                className="px-1.5 py-0.5 bg-terminal-blue/20 rounded text-terminal-blue"
              >
                {tag}
              </span>
            ))}
          </div>
        )}
      </div>

      {/* Double-click hint */}
      {isConnected && (
        <div className="mt-2 text-xs text-terminal-fg/40 text-center">
          Double-click to connect
        </div>
      )}
    </div>
  );
}
