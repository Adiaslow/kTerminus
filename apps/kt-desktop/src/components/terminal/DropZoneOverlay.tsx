/**
 * Visual overlay that shows drop zones when dragging a tab over a pane
 *
 * Shows indicators for:
 * - Top edge: Split vertical, new pane on top
 * - Bottom edge: Split vertical, new pane on bottom
 * - Left edge: Split horizontal, new pane on left
 * - Right edge: Split horizontal, new pane on right
 * - Center: Replace pane content
 */

import { useState, useCallback, memo } from "react";
import clsx from "clsx";
import type { DropPosition } from "../../types/layout";

interface DropZoneOverlayProps {
  /** Called when a tab is dropped */
  onDrop: (tabId: string, position: DropPosition) => void;
  /** Whether this pane is currently being dragged over */
  children: React.ReactNode;
}

export const DropZoneOverlay = memo(function DropZoneOverlay({
  onDrop,
  children,
}: DropZoneOverlayProps) {
  const [isDraggingOver, setIsDraggingOver] = useState(false);
  const [dropPosition, setDropPosition] = useState<DropPosition | null>(null);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";

    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const width = rect.width;
    const height = rect.height;

    // Calculate relative position (0-1)
    const relX = x / width;
    const relY = y / height;

    // Determine drop zone based on position
    // Edge zones are 25% of the dimension
    const edgeThreshold = 0.25;

    let position: DropPosition;
    if (relY < edgeThreshold) {
      position = "top";
    } else if (relY > 1 - edgeThreshold) {
      position = "bottom";
    } else if (relX < edgeThreshold) {
      position = "left";
    } else if (relX > 1 - edgeThreshold) {
      position = "right";
    } else {
      position = "center";
    }

    setDropPosition(position);
    setIsDraggingOver(true);
  }, []);

  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDraggingOver(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    // Only clear if leaving the container (not entering a child)
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX;
    const y = e.clientY;

    if (
      x < rect.left ||
      x > rect.right ||
      y < rect.top ||
      y > rect.bottom
    ) {
      setIsDraggingOver(false);
      setDropPosition(null);
    }
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      const tabId = e.dataTransfer.getData("text/plain");

      if (tabId && dropPosition) {
        onDrop(tabId, dropPosition);
      }

      setIsDraggingOver(false);
      setDropPosition(null);
    },
    [dropPosition, onDrop]
  );

  return (
    <div
      className="relative h-full w-full"
      onDragOver={handleDragOver}
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      {children}

      {/* Transparent capture layer - covers the terminal during drag to prevent
          xterm.js canvas from consuming drag events */}
      {isDraggingOver && (
        <div className="absolute inset-0 z-40" />
      )}

      {/* Drop zone indicators */}
      {isDraggingOver && (
        <div className="absolute inset-0 pointer-events-none z-50">
          {/* Top zone */}
          <div
            className={clsx(
              "absolute top-0 left-0 right-0 h-1/4 border-2 border-dashed transition-colors",
              dropPosition === "top"
                ? "border-mauve bg-mauve/20"
                : "border-transparent"
            )}
          />

          {/* Bottom zone */}
          <div
            className={clsx(
              "absolute bottom-0 left-0 right-0 h-1/4 border-2 border-dashed transition-colors",
              dropPosition === "bottom"
                ? "border-mauve bg-mauve/20"
                : "border-transparent"
            )}
          />

          {/* Left zone */}
          <div
            className={clsx(
              "absolute top-1/4 bottom-1/4 left-0 w-1/4 border-2 border-dashed transition-colors",
              dropPosition === "left"
                ? "border-mauve bg-mauve/20"
                : "border-transparent"
            )}
          />

          {/* Right zone */}
          <div
            className={clsx(
              "absolute top-1/4 bottom-1/4 right-0 w-1/4 border-2 border-dashed transition-colors",
              dropPosition === "right"
                ? "border-mauve bg-mauve/20"
                : "border-transparent"
            )}
          />

          {/* Center zone */}
          <div
            className={clsx(
              "absolute top-1/4 bottom-1/4 left-1/4 right-1/4 border-2 border-dashed transition-colors",
              dropPosition === "center"
                ? "border-sage bg-sage/20"
                : "border-transparent"
            )}
          />

          {/* Position label */}
          {dropPosition && (
            <div className="absolute inset-0 flex items-center justify-center">
              <span
                className={clsx(
                  "px-3 py-1.5 rounded-md text-xs font-medium",
                  dropPosition === "center"
                    ? "bg-sage text-bg-void"
                    : "bg-mauve text-bg-void"
                )}
              >
                {dropPosition === "center"
                  ? "Replace"
                  : `Split ${dropPosition}`}
              </span>
            </div>
          )}
        </div>
      )}
    </div>
  );
});
