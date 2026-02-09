import { Handle, Position } from "@xyflow/react";
import { clsx } from "clsx";
import { useState, useCallback } from "react";
import type { OrchestratorStatus } from "../../types";
import { formatUptime } from "../../lib/utils";
import { OrchestratorIcon } from "../Icons";

interface OrchestratorNodeProps {
  data: { status: OrchestratorStatus };
}

export function OrchestratorNode({ data }: OrchestratorNodeProps) {
  const { status } = data;
  const [copied, setCopied] = useState(false);

  const copyPairingCode = useCallback(() => {
    if (status.pairingCode) {
      navigator.clipboard.writeText(status.pairingCode);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  }, [status.pairingCode]);

  return (
    <div
      className={clsx(
        "relative px-5 py-4 rounded-zen border-2 bg-bg-surface min-w-[180px]",
        status.running
          ? "border-mauve-deep shadow-lg shadow-mauve/10"
          : "border-terracotta-dim shadow-lg shadow-terracotta/10"
      )}
    >
      {/* Light slit accent — top edge */}
      <div
        className={clsx(
          "absolute top-0 left-4 right-4 h-px opacity-50",
          status.running ? "bg-mauve" : "bg-terracotta"
        )}
      />

      {/* Connection handle */}
      <Handle
        type="source"
        position={Position.Bottom}
        className={clsx(
          "!w-3 !h-3 !border-2 !border-bg-surface !rounded-sm",
          status.running ? "!bg-mauve" : "!bg-terracotta-dim"
        )}
      />

      {/* Header */}
      <div className="flex items-center gap-3 mb-3">
        {/* Orchestrator icon — abstract node symbol */}
        <div className="w-6 h-6 flex items-center justify-center">
          <OrchestratorIcon
            className={clsx(
              "w-5 h-5",
              status.running ? "text-mauve" : "text-terracotta-dim"
            )}
          />
        </div>
        <div>
          <div
            className={clsx(
              "font-medium text-sm",
              status.running ? "text-mauve-mid" : "text-terracotta"
            )}
          >
            Orchestrator
          </div>
          <div className="text-[10px] text-text-ghost">v{status.version}</div>
        </div>
      </div>

      {/* Status */}
      <div className="flex items-center gap-2 mb-3">
        <div
          className={clsx(
            "w-1.5 h-1.5 rounded-full",
            status.running ? "bg-sage" : "bg-terracotta-dim"
          )}
        />
        <span className="text-xs text-text-secondary">
          {status.running ? "Running" : "Stopped"}
        </span>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-2 gap-1.5 text-[10px]">
        <div className="bg-bg-deep rounded-sm p-2">
          <div className="text-text-ghost uppercase tracking-wide">Uptime</div>
          <div className="font-medium text-text-primary">
            {formatUptime(status.uptimeSecs)}
          </div>
        </div>
        <div className="bg-bg-deep rounded-sm p-2">
          <div className="text-text-ghost uppercase tracking-wide">Machines</div>
          <div className="font-medium text-text-primary">
            {status.machineCount}
          </div>
        </div>
        <div className="bg-bg-deep rounded-sm p-2 col-span-2">
          <div className="text-text-ghost uppercase tracking-wide">
            Active Sessions
          </div>
          <div className="font-medium text-text-primary">
            {status.sessionCount}
          </div>
        </div>
      </div>

      {/* Pairing Code */}
      {status.pairingCode && (
        <div className="mt-3 pt-3 border-t border-border-faint">
          <div className="text-[10px] text-text-ghost uppercase tracking-wide mb-1.5">
            Pairing Code
          </div>
          <button
            onClick={copyPairingCode}
            className="flex items-center justify-between w-full bg-bg-deep hover:bg-bg-elevated rounded-sm px-3 py-2 transition-colors group"
            title="Click to copy"
          >
            <span className="font-mono font-bold text-sm tracking-wider text-mauve-mid">
              {status.pairingCode}
            </span>
            <span className="text-[10px] text-text-ghost group-hover:text-text-muted transition-colors">
              {copied ? "Copied!" : "Copy"}
            </span>
          </button>
        </div>
      )}
    </div>
  );
}
