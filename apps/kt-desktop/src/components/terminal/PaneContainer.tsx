/**
 * Recursive container that renders the layout tree
 *
 * - For pane nodes: renders TerminalPaneWrapper
 * - For split nodes: renders Group with Panel children
 */

import { memo, Fragment, useCallback } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";
import { TerminalPaneWrapper } from "./TerminalPaneWrapper";
import { useLayoutStore } from "../../stores/layout";
import type { LayoutNode, SplitContainer } from "../../types/layout";

interface PaneContainerProps {
  node: LayoutNode;
}

export const PaneContainer = memo(function PaneContainer({
  node,
}: PaneContainerProps) {
  const updateSizes = useLayoutStore((s) => s.updateSizes);

  // Leaf node - render the terminal pane
  if (node.type === "pane") {
    return <TerminalPaneWrapper paneId={node.id} tabId={node.tabId} />;
  }

  // Split node - render panel group with children
  return (
    <SplitGroup node={node} updateSizes={updateSizes} />
  );
});

interface SplitGroupProps {
  node: SplitContainer;
  updateSizes: (splitId: string, sizes: number[]) => void;
}

const SplitGroup = memo(function SplitGroup({ node, updateSizes }: SplitGroupProps) {
  // Convert layout map back to array of sizes in child order
  const handleLayoutChanged = useCallback(
    (layout: { [panelId: string]: number }) => {
      const sizes = node.children.map((child) => layout[child.id] ?? 50);
      updateSizes(node.id, sizes);
    },
    [node.id, node.children, updateSizes]
  );

  return (
    <Group
      orientation={node.direction}
      onLayoutChanged={handleLayoutChanged}
    >
      {node.children.map((child, index) => (
        <Fragment key={child.id}>
          {index > 0 && (
            <Separator
              className={
                node.direction === "horizontal"
                  ? "w-1 bg-border hover:bg-mauve/50 transition-colors cursor-col-resize"
                  : "h-1 bg-border hover:bg-mauve/50 transition-colors cursor-row-resize"
              }
            />
          )}
          <Panel
            defaultSize={node.sizes[index]}
            minSize={10}
            id={child.id}
          >
            <PaneContainer node={child} />
          </Panel>
        </Fragment>
      ))}
    </Group>
  );
});
