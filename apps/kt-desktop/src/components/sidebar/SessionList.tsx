import { useTerminalsStore } from "../../stores/terminals";
import { useMachinesStore } from "../../stores/machines";
import * as tauri from "../../lib/tauri";
import { clsx } from "clsx";

export function SessionList() {
  const tabs = useTerminalsStore((s) => s.tabs);
  const activeTabId = useTerminalsStore((s) => s.activeTabId);
  const setActiveTab = useTerminalsStore((s) => s.setActiveTab);
  const removeTab = useTerminalsStore((s) => s.removeTab);
  const machines = useMachinesStore((s) => s.machines);

  const getMachineName = (machineId: string) => {
    const machine = machines.find((m) => m.id === machineId);
    return machine?.alias || machine?.hostname || machineId.slice(0, 8);
  };

  const handleKillSession = async (
    e: React.MouseEvent,
    sessionId: string,
    tabId: string
  ) => {
    e.stopPropagation();

    try {
      await tauri.killSession(sessionId);
      removeTab(tabId);
    } catch (err) {
      console.error("Failed to kill session:", err);
    }
  };

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-2 flex items-center justify-between">
        <span className="text-xs text-terminal-fg/50 uppercase tracking-wide">
          Active Sessions ({tabs.length})
        </span>
      </div>

      {/* Session list */}
      <div className="flex-1 overflow-auto">
        {tabs.length === 0 ? (
          <div className="p-4 text-center text-terminal-fg/50 text-sm">
            No active sessions
            <br />
            <span className="text-xs">
              Click + on a machine to start one
            </span>
          </div>
        ) : (
          tabs.map((tab) => (
            <div
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={clsx(
                "group flex items-center gap-2 px-3 py-2 cursor-pointer transition-colors",
                tab.id === activeTabId ? "bg-sidebar-active" : "hover:bg-sidebar-hover"
              )}
            >
              {/* Terminal icon */}
              <div className="w-5 h-5 flex items-center justify-center text-terminal-green">
                <svg
                  className="w-4 h-4"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"
                  />
                </svg>
              </div>

              {/* Session info */}
              <div className="flex-1 min-w-0">
                <div className="text-sm font-medium truncate">{tab.title}</div>
                <div className="text-xs text-terminal-fg/50 truncate">
                  {getMachineName(tab.machineId)}
                </div>
              </div>

              {/* Kill button */}
              <button
                onClick={(e) => handleKillSession(e, tab.sessionId, tab.id)}
                className="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-terminal-red/20 transition-all"
                title="Kill session"
              >
                <svg
                  className="w-4 h-4 text-terminal-red"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
