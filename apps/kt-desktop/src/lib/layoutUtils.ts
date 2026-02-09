/**
 * Pure functions for manipulating the layout tree
 *
 * These functions take a layout state and return a new layout state,
 * making them easy to use with Zustand and test in isolation.
 */

import type {
  PaneLayout,
  LayoutNode,
  PaneLeaf,
  SplitContainer,
  SplitDirection,
  DropPosition,
} from "../types/layout";

/**
 * Generate a unique pane ID using crypto.randomUUID().
 *
 * Uses UUID instead of incrementing counters to:
 * - Avoid unbounded counter growth across sessions
 * - Ensure uniqueness across browser tabs/windows
 * - Simplify ID management without reset logic
 */
export function generatePaneId(): string {
  return `pane-${crypto.randomUUID()}`;
}

/**
 * Generate a unique split ID using crypto.randomUUID().
 *
 * Uses UUID instead of incrementing counters to:
 * - Avoid unbounded counter growth across sessions
 * - Ensure uniqueness across browser tabs/windows
 * - Simplify ID management without reset logic
 */
export function generateSplitId(): string {
  return `split-${crypto.randomUUID()}`;
}

/** Create a new pane leaf */
export function createPane(tabId: string): PaneLeaf {
  return {
    type: "pane",
    id: generatePaneId(),
    tabId,
  };
}

/** Create a new split container */
export function createSplit(
  direction: SplitDirection,
  children: LayoutNode[],
  sizes?: number[]
): SplitContainer {
  return {
    type: "split",
    id: generateSplitId(),
    direction,
    children,
    sizes: sizes || children.map(() => 100 / children.length),
  };
}

/** Find a pane by ID in the layout tree */
export function findPane(
  node: LayoutNode | null,
  paneId: string
): PaneLeaf | null {
  if (!node) return null;

  if (node.type === "pane") {
    return node.id === paneId ? node : null;
  }

  for (const child of node.children) {
    const found = findPane(child, paneId);
    if (found) return found;
  }

  return null;
}

/** Find a pane by tab ID in the layout tree */
export function findPaneByTabId(
  node: LayoutNode | null,
  tabId: string
): PaneLeaf | null {
  if (!node) return null;

  if (node.type === "pane") {
    return node.tabId === tabId ? node : null;
  }

  for (const child of node.children) {
    const found = findPaneByTabId(child, tabId);
    if (found) return found;
  }

  return null;
}

/** Find the first pane in the layout tree (DFS) */
export function findFirstPane(node: LayoutNode | null): PaneLeaf | null {
  if (!node) return null;

  if (node.type === "pane") return node;

  for (const child of node.children) {
    const found = findFirstPane(child);
    if (found) return found;
  }

  return null;
}

/** Get all panes in the layout tree */
export function getAllPanes(node: LayoutNode | null): PaneLeaf[] {
  if (!node) return [];

  if (node.type === "pane") return [node];

  return node.children.flatMap(getAllPanes);
}

/** Get the next pane in the layout (for focus cycling) */
export function getNextPane(
  node: LayoutNode | null,
  currentPaneId: string,
  direction: 1 | -1 = 1
): PaneLeaf | null {
  const panes = getAllPanes(node);
  if (panes.length === 0) return null;

  const currentIndex = panes.findIndex((p) => p.id === currentPaneId);
  if (currentIndex === -1) return panes[0];

  const nextIndex = (currentIndex + direction + panes.length) % panes.length;
  return panes[nextIndex];
}

/**
 * Split an existing pane, creating a new split container
 *
 * @param layout - Current layout state
 * @param targetPaneId - ID of the pane to split
 * @param direction - Direction of the split
 * @param newTabId - Tab ID for the new pane
 * @param insertBefore - If true, new pane goes before (left/top); if false, after (right/bottom)
 * @returns New layout state
 */
export function splitPaneInLayout(
  layout: PaneLayout,
  targetPaneId: string,
  direction: SplitDirection,
  newTabId: string,
  insertBefore: boolean = false
): PaneLayout {
  if (!layout.root) return layout;

  const newPane = createPane(newTabId);

  function transform(node: LayoutNode): LayoutNode {
    if (node.type === "pane") {
      if (node.id === targetPaneId) {
        // Replace this pane with a split containing original + new
        const children = insertBefore ? [newPane, node] : [node, newPane];
        return createSplit(direction, children, [50, 50]);
      }
      return node;
    }

    // Recurse into split children
    return {
      ...node,
      children: node.children.map(transform),
    };
  }

  return {
    root: transform(layout.root),
    activePaneId: newPane.id, // Focus the new pane
  };
}

/**
 * Close a pane and clean up empty splits
 *
 * When a pane is closed:
 * - If it's the only child, remove the parent split
 * - If sibling exists, promote sibling to take parent's place
 * - Recursively clean up until layout is minimal
 */
export function closePaneInLayout(
  layout: PaneLayout,
  paneId: string
): PaneLayout {
  if (!layout.root) return layout;

  function transform(node: LayoutNode): LayoutNode | null {
    if (node.type === "pane") {
      return node.id === paneId ? null : node;
    }

    // Filter out removed children
    const newChildren = node.children
      .map(transform)
      .filter((c): c is LayoutNode => c !== null);

    // If only one child remains, promote it
    if (newChildren.length === 1) {
      return newChildren[0];
    }

    // If no children remain, remove this split
    if (newChildren.length === 0) {
      return null;
    }

    // Recalculate sizes proportionally
    const totalSize = node.sizes
      .filter((_, i) => {
        const child = node.children[i];
        return child && transform(child) !== null;
      })
      .reduce((a, b) => a + b, 0);

    const newSizes = newChildren.map((_, i) => {
      const originalIndex = node.children.findIndex(
        (c) => c.id === newChildren[i].id
      );
      if (originalIndex !== -1 && totalSize > 0) {
        return (node.sizes[originalIndex] / totalSize) * 100;
      }
      return 100 / newChildren.length;
    });

    return { ...node, children: newChildren, sizes: newSizes };
  }

  const newRoot = transform(layout.root);

  // Find new active pane if current was closed
  let newActivePaneId = layout.activePaneId;
  if (layout.activePaneId === paneId || !findPane(newRoot, layout.activePaneId || "")) {
    newActivePaneId = findFirstPane(newRoot)?.id || null;
  }

  return { root: newRoot, activePaneId: newActivePaneId };
}

/**
 * Update sizes of a split container's children
 */
export function updateSizesInLayout(
  layout: PaneLayout,
  splitId: string,
  sizes: number[]
): PaneLayout {
  if (!layout.root) return layout;

  function transform(node: LayoutNode): LayoutNode {
    if (node.type === "pane") return node;

    if (node.id === splitId) {
      return { ...node, sizes };
    }

    return {
      ...node,
      children: node.children.map(transform),
    };
  }

  return { ...layout, root: transform(layout.root) };
}

/**
 * Replace a pane's tab (for tab switching within a pane)
 */
export function replacePaneTab(
  layout: PaneLayout,
  paneId: string,
  newTabId: string
): PaneLayout {
  if (!layout.root) return layout;

  function transform(node: LayoutNode): LayoutNode {
    if (node.type === "pane") {
      if (node.id === paneId) {
        return { ...node, tabId: newTabId };
      }
      return node;
    }

    return {
      ...node,
      children: node.children.map(transform),
    };
  }

  return { ...layout, root: transform(layout.root) };
}

/**
 * Handle dropping a tab onto a pane
 *
 * Depending on the drop position:
 * - "center": Replace the pane's current tab
 * - "left"/"right": Create horizontal split
 * - "top"/"bottom": Create vertical split
 */
export function dropTabOnPaneInLayout(
  layout: PaneLayout,
  tabId: string,
  targetPaneId: string,
  position: DropPosition
): PaneLayout {
  if (position === "center") {
    // Replace the target pane's tab
    return replacePaneTab(layout, targetPaneId, tabId);
  }

  // Create a split
  const direction: SplitDirection =
    position === "left" || position === "right" ? "horizontal" : "vertical";
  const insertBefore = position === "left" || position === "top";

  return splitPaneInLayout(layout, targetPaneId, direction, tabId, insertBefore);
}

/**
 * Create initial layout from a tab ID
 */
export function createInitialLayout(tabId: string): PaneLayout {
  const pane = createPane(tabId);
  return {
    root: pane,
    activePaneId: pane.id,
  };
}

/**
 * Add a pane to the layout (when a new tab is created)
 * If no layout exists, creates initial layout.
 * If layout exists, adds pane to active split or creates new split.
 *
 * NOTE: This function is intentionally exported for future use in:
 * - Automated layout building (e.g., restoring a multi-pane layout)
 * - External layout manipulation APIs
 * - Testing layout operations
 * Currently the store uses splitPaneInLayout directly, but this provides
 * a higher-level API that may be useful for batch operations.
 */
export function addPaneToLayout(
  layout: PaneLayout,
  tabId: string
): PaneLayout {
  if (!layout.root) {
    return createInitialLayout(tabId);
  }

  // Add to active pane's location (split horizontally by default)
  if (layout.activePaneId) {
    return splitPaneInLayout(layout, layout.activePaneId, "horizontal", tabId);
  }

  // Fallback: split first pane
  const firstPane = findFirstPane(layout.root);
  if (firstPane) {
    return splitPaneInLayout(layout, firstPane.id, "horizontal", tabId);
  }

  return createInitialLayout(tabId);
}
