/**
 * Wrapper around TerminalPane that adds header, focus handling, and drop zones
 *
 * This component connects a pane in the layout tree to its terminal.
 */

import { memo, useCallback } from "react";
import clsx from "clsx";
import { TerminalPane } from "./TerminalPane";
import { PaneHeader } from "./PaneHeader";
import { DropZoneOverlay } from "./DropZoneOverlay";
import { useTerminalsStore } from "../../stores/terminals";
import { useLayoutStore } from "../../stores/layout";
import type { DropPosition } from "../../types/layout";

interface TerminalPaneWrapperProps {
  paneId: string;
  tabId: string;
}

export const TerminalPaneWrapper = memo(function TerminalPaneWrapper({
  paneId,
  tabId,
}: TerminalPaneWrapperProps) {
  const tab = useTerminalsStore((s) => s.tabs.find((t) => t.id === tabId));
  const activePaneId = useLayoutStore((s) => s.layout.activePaneId);
  const setActivePane = useLayoutStore((s) => s.setActivePane);
  const splitPane = useLayoutStore((s) => s.splitPane);
  const closePane = useLayoutStore((s) => s.closePane);
  const dropTabOnPane = useLayoutStore((s) => s.dropTabOnPane);
  const setActiveTab = useTerminalsStore((s) => s.setActiveTab);

  const isActive = activePaneId === paneId;

  // Handle click to focus this pane
  const handleClick = useCallback(() => {
    if (!isActive) {
      setActivePane(paneId);
      // Also update the active tab in terminals store for tab bar highlighting
      if (tabId) {
        setActiveTab(tabId);
      }
    }
  }, [isActive, paneId, tabId, setActivePane, setActiveTab]);

  // Handle split actions - need to create a new tab first
  // For now, we'll split with the same tab (user can then switch)
  // In Phase 2, we'll add proper new session creation
  const handleSplitHorizontal = useCallback(() => {
    // For now, split with same tab - this creates a duplicate view
    // which isn't ideal but shows the split working
    // TODO: In Phase 2, prompt for new session or show tab picker
    splitPane(paneId, "horizontal", tabId);
  }, [paneId, tabId, splitPane]);

  const handleSplitVertical = useCallback(() => {
    splitPane(paneId, "vertical", tabId);
  }, [paneId, tabId, splitPane]);

  const handleClose = useCallback(() => {
    closePane(paneId);
  }, [paneId, closePane]);

  // Handle dropping a tab onto this pane
  const handleDrop = useCallback(
    (droppedTabId: string, position: DropPosition) => {
      // Don't do anything if dropping the same tab on center
      if (droppedTabId === tabId && position === "center") {
        return;
      }
      dropTabOnPane(droppedTabId, paneId, position);
    },
    [paneId, tabId, dropTabOnPane]
  );

  if (!tab) {
    // Log warning for debugging - this indicates a stale layout or race condition
    console.warn(
      `[TerminalPaneWrapper] Tab not found for pane. paneId=${paneId}, tabId=${tabId}. ` +
      `This may indicate a stale layout referencing a closed tab.`
    );
    return (
      <div className="h-full flex items-center justify-center bg-bg-deep text-text-muted">
        <span className="text-sm">Tab not found</span>
      </div>
    );
  }

  return (
    <DropZoneOverlay onDrop={handleDrop}>
      <div
        className={clsx(
          "h-full flex flex-col overflow-hidden",
          isActive && "ring-1 ring-inset ring-mauve/30"
        )}
        onClick={handleClick}
      >
        <PaneHeader
          tab={tab}
          isActive={isActive}
          onSplitHorizontal={handleSplitHorizontal}
          onSplitVertical={handleSplitVertical}
          onClose={handleClose}
        />
        <div className="flex-1 overflow-hidden">
          <TerminalPane sessionId={tab.sessionId} isActive={isActive} />
        </div>
      </div>
    </DropZoneOverlay>
  );
});
