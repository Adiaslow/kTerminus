import { useTerminalsStore } from "../../stores/terminals";
import { clsx } from "clsx";
import * as tauri from "../../lib/tauri";

export function TerminalTabs() {
  const tabs = useTerminalsStore((s) => s.tabs);
  const activeTabId = useTerminalsStore((s) => s.activeTabId);
  const setActiveTab = useTerminalsStore((s) => s.setActiveTab);
  const removeTab = useTerminalsStore((s) => s.removeTab);

  if (tabs.length === 0) {
    return null;
  }

  const handleCloseTab = async (
    e: React.MouseEvent,
    tabId: string,
    sessionId: string
  ) => {
    e.stopPropagation();

    try {
      await tauri.terminalClose(sessionId);
    } catch (err) {
      console.error("Failed to close terminal:", err);
    }

    removeTab(tabId);
  };

  return (
    <div className="flex items-center bg-sidebar-bg border-b border-sidebar-active overflow-x-auto">
      {tabs.map((tab) => (
        <div
          key={tab.id}
          onClick={() => setActiveTab(tab.id)}
          className={clsx(
            "group flex items-center gap-2 px-3 py-1.5 cursor-pointer transition-colors border-r border-sidebar-active",
            tab.id === activeTabId
              ? "bg-terminal-bg text-terminal-fg"
              : "text-terminal-fg/60 hover:text-terminal-fg hover:bg-sidebar-hover"
          )}
        >
          {/* Terminal icon */}
          <svg
            className="w-4 h-4 text-terminal-green flex-shrink-0"
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

          {/* Tab title */}
          <span className="text-sm truncate max-w-32">{tab.title}</span>

          {/* Close button */}
          <button
            onClick={(e) => handleCloseTab(e, tab.id, tab.sessionId)}
            className={clsx(
              "p-0.5 rounded transition-all",
              tab.id === activeTabId
                ? "opacity-60 hover:opacity-100 hover:bg-terminal-red/20"
                : "opacity-0 group-hover:opacity-60 hover:!opacity-100 hover:bg-terminal-red/20"
            )}
          >
            <svg
              className="w-3.5 h-3.5"
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
      ))}
    </div>
  );
}
