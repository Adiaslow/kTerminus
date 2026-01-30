import { Handle, Position } from "@xyflow/react";
import { clsx } from "clsx";
import type { OrchestratorStatus } from "../../types";

interface OrchestratorNodeProps {
  data: { status: OrchestratorStatus };
}

export function OrchestratorNode({ data }: OrchestratorNodeProps) {
  const { status } = data;

  const formatUptime = (secs: number) => {
    const days = Math.floor(secs / 86400);
    const hours = Math.floor((secs % 86400) / 3600);
    const mins = Math.floor((secs % 3600) / 60);

    if (days > 0) return `${days}d ${hours}h`;
    if (hours > 0) return `${hours}h ${mins}m`;
    return `${mins}m`;
  };

  return (
    <div
      className={clsx(
        "px-5 py-4 rounded-xl border-2 bg-sidebar-bg min-w-[180px]",
        "shadow-lg shadow-terminal-blue/20",
        status.running ? "border-terminal-blue" : "border-terminal-red"
      )}
    >
      {/* Connection handle */}
      <Handle
        type="source"
        position={Position.Bottom}
        className="!bg-terminal-blue !w-3 !h-3 !border-2 !border-sidebar-bg"
      />

      {/* Header */}
      <div className="flex items-center gap-2 mb-3">
        <div className="text-2xl">🎛️</div>
        <div>
          <div className="font-semibold text-terminal-blue">Orchestrator</div>
          <div className="text-xs text-terminal-fg/50">v{status.version}</div>
        </div>
      </div>

      {/* Status */}
      <div className="flex items-center gap-2 mb-3">
        <div
          className={clsx(
            "w-2 h-2 rounded-full",
            status.running ? "bg-terminal-green" : "bg-terminal-red"
          )}
        />
        <span className="text-sm">
          {status.running ? "Running" : "Stopped"}
        </span>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-2 gap-2 text-xs">
        <div className="bg-terminal-bg rounded p-2">
          <div className="text-terminal-fg/50">Uptime</div>
          <div className="font-medium">{formatUptime(status.uptimeSecs)}</div>
        </div>
        <div className="bg-terminal-bg rounded p-2">
          <div className="text-terminal-fg/50">Machines</div>
          <div className="font-medium">{status.machineCount}</div>
        </div>
        <div className="bg-terminal-bg rounded p-2 col-span-2">
          <div className="text-terminal-fg/50">Active Sessions</div>
          <div className="font-medium">{status.sessionCount}</div>
        </div>
      </div>
    </div>
  );
}
