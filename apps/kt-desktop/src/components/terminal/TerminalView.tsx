import { useTerminalsStore } from "../../stores/terminals";
import { TerminalTabs } from "./TerminalTabs";
import { TerminalPane } from "./TerminalPane";

export function TerminalView() {
  const tabs = useTerminalsStore((s) => s.tabs);
  const activeTabId = useTerminalsStore((s) => s.activeTabId);
  const activeTab = tabs.find((t) => t.id === activeTabId);

  return (
    <div className="h-full flex flex-col">
      {/* Tab bar */}
      <TerminalTabs />

      {/* Terminal pane */}
      <div className="flex-1 overflow-hidden">
        {activeTab ? (
          <TerminalPane
            key={activeTab.sessionId}
            sessionId={activeTab.sessionId}
            machineId={activeTab.machineId}
          />
        ) : (
          <EmptyState />
        )}
      </div>
    </div>
  );
}

function EmptyState() {
  return (
    <div className="h-full flex items-center justify-center">
      <div className="text-center">
        <div className="text-6xl mb-4">⌨</div>
        <h2 className="text-xl font-medium mb-2">No Active Sessions</h2>
        <p className="text-terminal-fg/50 max-w-md">
          Select a machine from the sidebar and click the + button to start a
          new terminal session.
        </p>
      </div>
    </div>
  );
}
