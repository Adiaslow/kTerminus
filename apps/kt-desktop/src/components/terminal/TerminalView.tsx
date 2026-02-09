/**
 * Main terminal view container
 *
 * Renders the tab bar and the pane layout system.
 */

import { useEffect } from "react";
import { useTerminalsStore } from "../../stores/terminals";
import { useLayoutStore } from "../../stores/layout";
import { TerminalTabs } from "./TerminalTabs";
import { PaneLayoutRoot } from "./PaneLayoutRoot";

export function TerminalView() {
  const tabs = useTerminalsStore((s) => s.tabs);
  const activeTabId = useTerminalsStore((s) => s.activeTabId);
  const layout = useLayoutStore((s) => s.layout);
  const initializeWithTab = useLayoutStore((s) => s.initializeWithTab);
  const removeTabFromLayout = useLayoutStore((s) => s.removeTabFromLayout);
  const resetLayout = useLayoutStore((s) => s.resetLayout);
  const setPaneTab = useLayoutStore((s) => s.setPaneTab);
  const isTabInLayout = useLayoutStore((s) => s.isTabInLayout);

  const setActiveTab = useTerminalsStore((s) => s.setActiveTab);

  // Reactively clean up stale state whenever we detect it
  // This handles HMR restoring state after mount effects run
  useEffect(() => {
    if (tabs.length === 0) {
      if (layout.root) {
        resetLayout();
      }
      if (activeTabId) {
        setActiveTab(null);
      }
    }
  }, [tabs.length, layout.root, activeTabId, resetLayout, setActiveTab]);

  // Initialize layout when first tab is created, or swap active pane for subsequent tabs
  useEffect(() => {
    // Only act if the tab actually exists (prevents stale HMR state issues)
    const tabExists = tabs.some((t) => t.id === activeTabId);
    if (!activeTabId || !tabExists) return;

    if (!layout.root) {
      // No layout yet - create initial layout with this tab
      initializeWithTab(activeTabId);
    } else if (!isTabInLayout(activeTabId) && layout.activePaneId) {
      // Tab is not in any pane - replace the active pane's content
      // This handles new tab creation (e.g., double-clicking a machine)
      setPaneTab(layout.activePaneId, activeTabId);
    }
  }, [activeTabId, tabs, layout.root, layout.activePaneId, initializeWithTab, isTabInLayout, setPaneTab]);

  // Clean up layout when tabs are removed
  useEffect(() => {
    // Find tabs that are in the layout but no longer exist
    const tabIds = new Set(tabs.map((t) => t.id));
    const layoutTabIds = getLayoutTabIds(layout.root);

    for (const tabId of layoutTabIds) {
      if (!tabIds.has(tabId)) {
        removeTabFromLayout(tabId);
      }
    }
  }, [tabs, layout.root, removeTabFromLayout]);

  return (
    <div className="h-full flex flex-col">
      {/* Tab bar */}
      <TerminalTabs />

      {/* Pane layout */}
      <div className="flex-1 overflow-hidden">
        <PaneLayoutRoot />
      </div>
    </div>
  );
}

/** Extract all tab IDs from the layout tree */
function getLayoutTabIds(node: import("../../types/layout").LayoutNode | null): string[] {
  if (!node) return [];
  if (node.type === "pane") return [node.tabId];
  return node.children.flatMap(getLayoutTabIds);
}
