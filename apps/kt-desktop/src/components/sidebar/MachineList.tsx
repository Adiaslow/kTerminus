import { useMachinesStore } from "../../stores/machines";
import { useTerminalsStore } from "../../stores/terminals";
import * as tauri from "../../lib/tauri";
import { clsx } from "clsx";
import type { Machine } from "../../types";

export function MachineList() {
  const machines = useMachinesStore((s) => s.machines);
  const selectedId = useMachinesStore((s) => s.selectedMachineId);
  const selectMachine = useMachinesStore((s) => s.selectMachine);

  const connectedMachines = machines.filter((m) => m.status === "connected");
  const disconnectedMachines = machines.filter((m) => m.status !== "connected");

  return (
    <div className="h-full flex flex-col">
      {/* Search */}
      <div className="p-2">
        <input
          type="text"
          placeholder="Search machines..."
          className="input w-full text-sm"
        />
      </div>

      {/* Machine list */}
      <div className="flex-1 overflow-auto">
        {machines.length === 0 ? (
          <div className="p-4 text-center text-terminal-fg/50 text-sm">
            No machines connected
          </div>
        ) : (
          <>
            {/* Connected machines */}
            {connectedMachines.length > 0 && (
              <div>
                <div className="px-3 py-1.5 text-xs text-terminal-fg/50 uppercase tracking-wide">
                  Connected ({connectedMachines.length})
                </div>
                {connectedMachines.map((machine) => (
                  <MachineItem
                    key={machine.id}
                    machine={machine}
                    selected={machine.id === selectedId}
                    onSelect={() => selectMachine(machine.id)}
                  />
                ))}
              </div>
            )}

            {/* Disconnected machines */}
            {disconnectedMachines.length > 0 && (
              <div className="mt-2">
                <div className="px-3 py-1.5 text-xs text-terminal-fg/50 uppercase tracking-wide">
                  Disconnected ({disconnectedMachines.length})
                </div>
                {disconnectedMachines.map((machine) => (
                  <MachineItem
                    key={machine.id}
                    machine={machine}
                    selected={machine.id === selectedId}
                    onSelect={() => selectMachine(machine.id)}
                  />
                ))}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}

function MachineItem({
  machine,
  selected,
  onSelect,
}: {
  machine: Machine;
  selected: boolean;
  onSelect: () => void;
}) {
  const addTab = useTerminalsStore((s) => s.addTab);
  const addSession = useTerminalsStore((s) => s.addSession);

  const handleConnect = async (e: React.MouseEvent) => {
    e.stopPropagation();

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

  const isConnected = machine.status === "connected";

  return (
    <div
      onClick={onSelect}
      className={clsx(
        "group flex items-center gap-2 px-3 py-2 cursor-pointer transition-colors",
        selected ? "bg-sidebar-active" : "hover:bg-sidebar-hover"
      )}
    >
      {/* Status indicator */}
      <div
        className={clsx(
          "w-2 h-2 rounded-full flex-shrink-0",
          isConnected
            ? "bg-terminal-green"
            : machine.status === "connecting"
            ? "bg-terminal-yellow animate-pulse"
            : "bg-terminal-red"
        )}
      />

      {/* Machine info */}
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium truncate">
          {machine.alias || machine.hostname}
        </div>
        <div className="text-xs text-terminal-fg/50 truncate">
          {machine.os}/{machine.arch}
          {machine.sessionCount > 0 && ` • ${machine.sessionCount} sessions`}
        </div>
      </div>

      {/* Connect button */}
      {isConnected && (
        <button
          onClick={handleConnect}
          className="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-terminal-blue/20 transition-all"
          title="New session"
        >
          <svg
            className="w-4 h-4 text-terminal-blue"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M12 4v16m8-8H4"
            />
          </svg>
        </button>
      )}
    </div>
  );
}
