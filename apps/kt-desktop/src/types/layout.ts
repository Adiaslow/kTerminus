/**
 * Layout types for terminal pane splitting/tiling
 *
 * The layout is represented as a tree where:
 * - Leaf nodes (PaneLeaf) contain a single terminal
 * - Split nodes (SplitContainer) contain two or more children arranged horizontally or vertically
 *
 * This enables recursive splitting to create arbitrary grid layouts.
 */

/** Direction of a split container */
export type SplitDirection = "horizontal" | "vertical";

/** A leaf node containing a single terminal pane */
export interface PaneLeaf {
  type: "pane";
  /** Unique pane identifier */
  id: string;
  /** Reference to the TerminalTab this pane displays */
  tabId: string;
}

/** A split container with two or more child panes/splits */
export interface SplitContainer {
  type: "split";
  /** Unique container identifier */
  id: string;
  /** Whether children are arranged horizontally (side-by-side) or vertically (stacked) */
  direction: SplitDirection;
  /** Child nodes (can be panes or nested splits) */
  children: LayoutNode[];
  /** Percentage sizes for each child (must sum to 100) */
  sizes: number[];
}

/** Union type for all layout tree nodes */
export type LayoutNode = PaneLeaf | SplitContainer;

/** Root layout structure */
export interface PaneLayout {
  /** Root node of the layout tree (null = no panes open) */
  root: LayoutNode | null;
  /** Currently focused pane ID */
  activePaneId: string | null;
}

/** Position for dropping a tab onto a pane */
export type DropPosition = "left" | "right" | "top" | "bottom" | "center";
