import { useCallback, useMemo, memo } from "react";
import { useTerminalsStore } from "../../stores/terminals";
import { useLayoutStore } from "../../stores/layout";
import { clsx } from "clsx";
import * as tauri from "../../lib/tauri";
import { XIcon } from "../Icons";
import type { TerminalTab } from "../../types";

// Memoized TabItem component to prevent unnecessary re-renders
const TabItem = memo(function TabItem({
  tab,
  isActive,
  onSelect,
  onClose,
}: {
  tab: TerminalTab;
  isActive: boolean;
  onSelect: () => void;
  onClose: (e: React.MouseEvent) => void;
}) {
  return (
    <div
      onClick={onSelect}
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData("text/plain", tab.id);
        e.dataTransfer.effectAllowed = "move";
      }}
      className={clsx(
        "group relative flex items-center gap-2 px-4 py-2.5 cursor-pointer transition-colors select-none",
        isActive
          ? "text-text-secondary"
          : "text-text-ghost hover:text-text-muted"
      )}
    >
      {/* Status indicator */}
      <div
        className={clsx(
          "w-[5px] h-[5px] rounded-full flex-shrink-0",
          isActive ? "bg-sage" : "bg-sage-dim"
        )}
      />

      {/* Tab title */}
      <span className="text-xs truncate max-w-32">{tab.title}</span>

      {/* Close button */}
      <button
        onClick={onClose}
        className={clsx(
          "p-0.5 rounded-sm transition-all text-text-ghost",
          isActive
            ? "opacity-40 hover:opacity-100 hover:text-terracotta"
            : "opacity-0 group-hover:opacity-40 hover:!opacity-100 hover:text-terracotta"
        )}
        title="Close tab"
        aria-label={`Close tab ${tab.title}`}
      >
        <XIcon className="w-3.5 h-3.5" />
      </button>

      {/* Light slit under active tab */}
      {isActive && (
        <span className="absolute bottom-0 left-4 right-4 h-px bg-mauve opacity-40" />
      )}
    </div>
  );
});

export function TerminalTabs() {
  const tabs = useTerminalsStore((s) => s.tabs);
  const activeTabId = useTerminalsStore((s) => s.activeTabId);
  const setActiveTab = useTerminalsStore((s) => s.setActiveTab);
  const removeTab = useTerminalsStore((s) => s.removeTab);
  const removeTabFromLayout = useLayoutStore((s) => s.removeTabFromLayout);

  const getPaneForTab = useLayoutStore((s) => s.getPaneForTab);
  const setActivePane = useLayoutStore((s) => s.setActivePane);
  const setPaneTab = useLayoutStore((s) => s.setPaneTab);
  const getActivePaneId = useLayoutStore((s) => s.getActivePaneId);

  // Memoize close handler factory to prevent creating new functions on each render
  const createCloseHandler = useCallback(
    (tabId: string, sessionId: string) => async (e: React.MouseEvent) => {
      e.stopPropagation();
      try {
        await tauri.terminalClose(sessionId);
      } catch (err) {
        console.error("Failed to close terminal:", err);
      }
      removeTabFromLayout(tabId);
      removeTab(tabId);
    },
    [removeTab, removeTabFromLayout]
  );

  // Memoize select handler factory
  // When clicking a tab:
  // - If it's already in a pane, focus that pane
  // - If it's not in any pane, replace the active pane's content
  const createSelectHandler = useCallback(
    (tabId: string) => () => {
      setActiveTab(tabId);

      // Check if this tab already has a pane
      const existingPaneId = getPaneForTab(tabId);
      if (existingPaneId) {
        // Tab is already displayed - just focus its pane
        setActivePane(existingPaneId);
      } else {
        // Tab is not in any pane - replace the active pane's tab
        const activePaneId = getActivePaneId();
        if (activePaneId) {
          setPaneTab(activePaneId, tabId);
        }
      }
    },
    [setActiveTab, getPaneForTab, setActivePane, setPaneTab, getActivePaneId]
  );

  // Memoize handlers for each tab
  const tabHandlers = useMemo(
    () =>
      tabs.map((tab) => ({
        tabId: tab.id,
        onSelect: createSelectHandler(tab.id),
        onClose: createCloseHandler(tab.id, tab.sessionId),
      })),
    [tabs, createSelectHandler, createCloseHandler]
  );

  if (tabs.length === 0) {
    return null;
  }

  return (
    <div className="flex items-center bg-bg-surface border-b border-border-faint overflow-x-auto px-1">
      {tabs.map((tab, index) => (
        <TabItem
          key={tab.id}
          tab={tab}
          isActive={tab.id === activeTabId}
          onSelect={tabHandlers[index].onSelect}
          onClose={tabHandlers[index].onClose}
        />
      ))}
    </div>
  );
}
