// @refresh reset
/**
 * Layout store for terminal pane splitting/tiling
 *
 * Manages the layout tree state and provides actions for:
 * - Splitting panes horizontally/vertically
 * - Closing panes
 * - Updating pane sizes
 * - Handling drag-and-drop
 * - Focus management
 *
 * NOTE: Layout is intentionally NOT persisted because terminal sessions
 * are not persisted. Persisting layout without sessions causes stale
 * pane references on app restart.
 */

import { create } from "zustand";
import type { PaneLayout, SplitDirection, DropPosition } from "../types/layout";
import {
  splitPaneInLayout,
  closePaneInLayout,
  updateSizesInLayout,
  dropTabOnPaneInLayout,
  createInitialLayout,
  findPaneByTabId,
  getNextPane,
  replacePaneTab,
} from "../lib/layoutUtils";

interface LayoutState {
  /** Current layout tree */
  layout: PaneLayout;

  // --- Actions ---

  /** Set the active (focused) pane */
  setActivePane: (paneId: string) => void;

  /** Split a pane in the given direction */
  splitPane: (paneId: string, direction: SplitDirection, newTabId: string) => void;

  /** Split the active pane (convenience for keyboard shortcuts) */
  splitActivePane: (direction: SplitDirection, newTabId: string) => void;

  /** Close a pane */
  closePane: (paneId: string) => void;

  /** Close the active pane */
  closeActivePane: () => void;

  /** Update sizes after resize */
  updateSizes: (splitId: string, sizes: number[]) => void;

  /** Handle dropping a tab onto a pane */
  dropTabOnPane: (tabId: string, targetPaneId: string, position: DropPosition) => void;

  /** Replace a pane's tab (for switching tabs within a pane) */
  setPaneTab: (paneId: string, tabId: string) => void;

  /** Focus next/previous pane (for cycling) */
  focusNextPane: (direction?: 1 | -1) => void;

  /** Initialize layout with a tab (called when first tab is created) */
  initializeWithTab: (tabId: string) => void;

  /** Add a new tab to the layout (creates pane in active location) */
  addTabToLayout: (tabId: string) => void;

  /** Remove a tab from layout (closes its pane if any) */
  removeTabFromLayout: (tabId: string) => void;

  /** Reset layout to empty */
  resetLayout: () => void;

  // --- Selectors ---

  /** Get the active pane ID */
  getActivePaneId: () => string | null;

  /** Check if a tab is visible in any pane */
  isTabInLayout: (tabId: string) => boolean;

  /** Get the pane displaying a specific tab */
  getPaneForTab: (tabId: string) => string | null;
}

export const useLayoutStore = create<LayoutState>()((set, get) => ({
  layout: { root: null, activePaneId: null },

  setActivePane: (paneId) =>
    set((state) => ({
      layout: { ...state.layout, activePaneId: paneId },
    })),

  splitPane: (paneId, direction, newTabId) =>
    set((state) => ({
      layout: splitPaneInLayout(state.layout, paneId, direction, newTabId),
    })),

  splitActivePane: (direction, newTabId) => {
    const { layout } = get();
    if (layout.activePaneId) {
      set({
        layout: splitPaneInLayout(layout, layout.activePaneId, direction, newTabId),
      });
    }
  },

  closePane: (paneId) =>
    set((state) => ({
      layout: closePaneInLayout(state.layout, paneId),
    })),

  closeActivePane: () => {
    const { layout } = get();
    if (layout.activePaneId) {
      set({
        layout: closePaneInLayout(layout, layout.activePaneId),
      });
    }
  },

  updateSizes: (splitId, sizes) =>
    set((state) => ({
      layout: updateSizesInLayout(state.layout, splitId, sizes),
    })),

  dropTabOnPane: (tabId, targetPaneId, position) =>
    set((state) => ({
      layout: dropTabOnPaneInLayout(state.layout, tabId, targetPaneId, position),
    })),

  setPaneTab: (paneId, tabId) =>
    set((state) => ({
      layout: replacePaneTab(state.layout, paneId, tabId),
    })),

  focusNextPane: (direction = 1) => {
    const { layout } = get();
    const nextPane = getNextPane(
      layout.root,
      layout.activePaneId || "",
      direction
    );
    if (nextPane) {
      set({
        layout: { ...layout, activePaneId: nextPane.id },
      });
    }
  },

  initializeWithTab: (tabId) =>
    set({
      layout: createInitialLayout(tabId),
    }),

  addTabToLayout: (tabId) => {
    const { layout } = get();
    if (!layout.root) {
      // No layout yet, create initial
      set({ layout: createInitialLayout(tabId) });
    }
    // If layout exists, don't auto-add (user can drag or split manually)
    // This prevents every new tab from auto-splitting
  },

  removeTabFromLayout: (tabId) => {
    const { layout } = get();
    const pane = findPaneByTabId(layout.root, tabId);
    if (pane) {
      set({ layout: closePaneInLayout(layout, pane.id) });
    }
  },

  resetLayout: () =>
    set({
      layout: { root: null, activePaneId: null },
    }),

  // Selectors
  getActivePaneId: () => get().layout.activePaneId,

  isTabInLayout: (tabId) => {
    const { layout } = get();
    return findPaneByTabId(layout.root, tabId) !== null;
  },

  getPaneForTab: (tabId) => {
    const { layout } = get();
    const pane = findPaneByTabId(layout.root, tabId);
    return pane?.id || null;
  },
}));

// Reset layout on HMR to prevent stale state
// This is only relevant during development
if (import.meta.hot) {
  import.meta.hot.accept(() => {
    useLayoutStore.getState().resetLayout();
  });
}
